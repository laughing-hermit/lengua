use std::fs;
use std::path::Path;

use crate::lock::{self, Kind};
use crate::{Finding, generate, sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    pub path: String,
    pub name: String,
    pub crate_name: String,
    pub label: String,
    pub lock: String,
    pub lock_sha256: String,
    pub description: String,
    pub line: usize,
}

fn finding(line: usize, code: &'static str, message: String) -> Finding {
    Finding {
        file: "idea.md".to_string(),
        line,
        code,
        message,
    }
}

pub(crate) fn description(idea: &str) -> String {
    let mut lines = idea.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return String::new();
    }
    for line in lines {
        if line.trim_end() == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("description:") {
            let value = value.trim();
            let inner = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            return inner.replace("\\\"", "\"").replace("\\\\", "\\");
        }
    }
    String::new()
}

fn one(
    folder: &Path,
    path: &str,
    line: usize,
    key: Option<&lock::Key>,
) -> Result<Library, Finding> {
    let place = folder.join(path);
    let missing = || {
        finding(
            line,
            "library-missing",
            format!("the library `{path}` has no `idea.md`; name the folder of a lengua library"),
        )
    };
    let idea = fs::read_to_string(place.join("idea.md")).map_err(|_| missing())?;
    let name = fs::canonicalize(&place)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .ok_or_else(missing)?;
    if !generate::package_name(&name) {
        return Err(finding(
            line,
            "library-name",
            format!(
                "the folder of the library `{path}` is not a package name: lowercase letters, digits, `-` and `_`"
            ),
        ));
    }
    let Some(key) = key else {
        return Err(finding(
            line,
            "library-lock",
            format!(
                "lengua could not read the user's key, so it can't verify the lock of the library `{path}`; call check"
            ),
        ));
    };
    let lock_path = place.join(lock::LOCK);
    let verified = lock::verify(&place, &lock_path, Kind::Library, key).map_err(|message| {
        finding(
            line,
            "library-lock",
            format!("the library `{path}` is not checked as it is: {message}. Call check on the library's folder first"),
        )
    })?;
    let bytes = fs::read(&lock_path).unwrap_or_default();
    Ok(Library {
        path: path.trim_end_matches('/').to_string(),
        crate_name: name.replace('-', "_"),
        name,
        label: verified.label,
        lock: format!("{}/{}", path.trim_end_matches('/'), lock::LOCK),
        lock_sha256: sha256::hex(&bytes),
        description: description(&idea),
        line,
    })
}

pub(crate) fn resolve(
    folder: &Path,
    uses: &[(String, usize)],
    findings: &mut Vec<Finding>,
) -> Vec<Library> {
    if uses.is_empty() {
        return Vec::new();
    }
    let key = lock::user_key().ok();
    let mut out: Vec<Library> = Vec::new();
    for (path, line) in uses {
        match one(folder, path, *line, key.as_ref()) {
            Ok(found) if out.iter().any(|l| l.name == found.name) => findings.push(finding(
                *line,
                "library-duplicate",
                format!("the library `{}` is in `- Uses:` two times", found.name),
            )),
            Ok(found) => out.push(found),
            Err(problem) => findings.push(problem),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::description;

    #[test]
    fn the_description_comes_from_the_frontmatter() {
        let idea = "---\ntype: lengua Library\ndescription: \"The library reads \\\"JSON\\\".\"\n---\n\n# Purpose\n";
        assert_eq!(description(idea), "The library reads \"JSON\".");
        assert_eq!(description("# Purpose\n"), "");
    }
}
