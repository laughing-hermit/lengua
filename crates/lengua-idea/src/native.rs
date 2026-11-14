use std::fs;
use std::path::{Path, PathBuf};

use crate::constraints::{Constraints, Grant, Part, Reason};
use crate::lock::{self, Kind};
use crate::{Finding, Shape, generate, library, sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Native {
    pub path: String,
    pub name: String,
    pub label: String,
    pub lock: String,
    pub lock_sha256: String,
    pub binary: PathBuf,
    pub binary_sha256: String,
    pub because: String,
    pub description: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub native: Option<String>,
    pub desktop: bool,
}

pub const HOST: &str = ".native";

fn finding(line: usize, code: &'static str, message: String) -> Finding {
    Finding {
        file: "idea.md".to_string(),
        line,
        code,
        message,
    }
}

pub(crate) fn declares_native(idea: &str) -> bool {
    idea.lines().any(|l| {
        l.strip_prefix("- Native: ")
            .is_some_and(|rest| !rest.trim_start().starts_with('`') && !rest.contains("Electron"))
    })
}

const INPUTS: [Grant; 2] = [Grant::StandardInput, Grant::Arguments];

pub(crate) fn rules(
    top: &Constraints,
    shape: Shape,
    screens: bool,
    findings: &mut Vec<Finding>,
) -> Place {
    let mut place = Place::default();
    for line in &top.natives {
        let Some(reason) = line.reason else {
            findings.push(finding(
                line.line,
                "native-reason",
                format!(
                    "`{}` is not a reason for native code; a part is native only for large AI models, the fastest local inference, training or fine-tuning, heavy compute with shared-memory threads, a device with no host grant, or the Electron shell of a desktop app",
                    line.because
                ),
            ));
            continue;
        };
        match (&line.part, reason) {
            (Part::Folder(_), Reason::Desktop) => findings.push(finding(
                line.line,
                "native-reason",
                "the Electron shell is the window of the app itself: write `- Native: a desktop window, for the Electron shell of a desktop app`".to_string(),
            )),
            (Part::Own(_), Reason::Desktop) => {
                if !screens {
                    findings.push(finding(
                        line.line,
                        "native-desktop",
                        "a desktop app shows screens, and the idea has none in `ideas/ui/`; add the screens, or remove the line".to_string(),
                    ));
                }
                place.desktop = true;
            }
            (Part::Own(_), _) => place.native = Some(line.because.clone()),
            (Part::Folder(_), _) => {}
        }
    }
    let Some(own) = top
        .natives
        .iter()
        .find(|l| matches!(l.part, Part::Own(_)) && l.reason.is_some_and(|r| r != Reason::Desktop))
    else {
        return place;
    };
    if shape != Shape::Command || place.desktop {
        findings.push(finding(
            own.line,
            "native-shape",
            "a native part is a program that reads its standard input and writes its standard output, not a server or a library; the WebAssembly program that names it serves and calls it".to_string(),
        ));
    }
    let reached = top.grants.iter().any(|g| !INPUTS.contains(g))
        || !top.components.is_empty()
        || !top.secrets.is_empty();
    if reached {
        findings.push(finding(
            own.line,
            "native-grant",
            "a native part runs outside the sandbox, so no grant can hold it: its idea grants only `- Reads: standard input` and `- Reads: arguments`, and its reason says what more it reaches".to_string(),
        ));
    }
    place
}

fn binary_line(lock_text: &str) -> Option<(String, String)> {
    lock_text.lines().find_map(|l| {
        let rest = l.strip_prefix("component ")?;
        let (hash, path) = rest.split_once(' ')?;
        Some((hash.to_string(), path.to_string()))
    })
}

fn one(
    folder: &Path,
    (path, because, line): (&str, &str, usize),
    key: Option<&lock::Key>,
) -> Result<Native, Finding> {
    let place = folder.join(path);
    let missing = || {
        finding(
            line,
            "native-missing",
            format!("the native part `{path}` has no `idea.md`; name the folder of a native part"),
        )
    };
    let idea = fs::read_to_string(place.join("idea.md")).map_err(|_| missing())?;
    if !declares_native(&idea) {
        return Err(finding(
            line,
            "native-missing",
            format!(
                "the idea of `{path}` has no `- Native:` line of its own, so it is not a native part; name a native part, or use the folder as a library"
            ),
        ));
    }
    let name = fs::canonicalize(&place)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .filter(|n| generate::package_name(n))
        .ok_or_else(missing)?;
    let unverified = |message: String| {
        finding(
            line,
            "native-lock",
            format!(
                "the native part `{path}` is not checked as it is: {message}. Call check on its folder first"
            ),
        )
    };
    let key = key.ok_or_else(|| unverified("lengua could not read the user's key".to_string()))?;
    let lock_path = place.join(lock::LOCK);
    let verified = lock::verify(&place, &lock_path, Kind::Native, key).map_err(&unverified)?;
    let bytes = fs::read(&lock_path).unwrap_or_default();
    let (binary_sha256, binary) = binary_line(&String::from_utf8_lossy(&bytes))
        .ok_or_else(|| unverified("its lock names no program".to_string()))?;
    let trimmed = path.trim_end_matches('/');
    Ok(Native {
        path: trimmed.to_string(),
        name,
        label: verified.label,
        lock: format!("{trimmed}/{}", lock::LOCK),
        lock_sha256: sha256::hex(&bytes),
        binary: place.join(binary),
        binary_sha256,
        because: because.to_string(),
        description: library::description(&idea),
        line,
    })
}

pub(crate) fn resolve(
    folder: &Path,
    top: &Constraints,
    findings: &mut Vec<Finding>,
) -> Vec<Native> {
    let wanted: Vec<(&str, &str, usize)> = top
        .natives
        .iter()
        .filter(|l| l.reason.is_some_and(|r| r != Reason::Desktop))
        .filter_map(|l| match &l.part {
            Part::Folder(path) => Some((path.as_str(), l.because.as_str(), l.line)),
            Part::Own(_) => None,
        })
        .collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let key = lock::user_key().ok();
    let mut out: Vec<Native> = Vec::new();
    for item in wanted {
        match one(folder, item, key.as_ref()) {
            Ok(found) if out.iter().any(|n| n.name == found.name) => findings.push(finding(
                item.2,
                "native-duplicate",
                format!("the native part `{}` is named two times", found.name),
            )),
            Ok(found) => out.push(found),
            Err(problem) => findings.push(problem),
        }
    }
    out
}

pub(crate) fn undeclared(file: &str, text: &str) -> Vec<Finding> {
    let needs = [
        ("thread::spawn", "threads that share memory"),
        ("thread::scope", "threads that share memory"),
        ("process::Command", "a process of the system"),
    ];
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if let Some((call, what)) = needs.iter().find(|(call, _)| line.contains(call)) {
            out.push(Finding {
                file: file.to_string(),
                line: number + 1,
                code: "native-undeclared",
                message: format!(
                    "`{call}` needs {what}, which a WebAssembly component does not have; move the work to a native part, declared with `- Native: `<folder>`, for <reason>`, or do it without `{call}`"
                ),
            });
        }
    }
    out
}

#[must_use]
pub fn where_it_runs(shape: Shape, place: &Place, natives: &[Native], screens: bool) -> String {
    let base = if let Some(because) = &place.native {
        format!(
            "A native program, built for this computer's system and CPU, for {because}. It runs outside the sandbox: it can do what the user's account can."
        )
    } else if place.desktop {
        "An Electron app: the same front end in a window, and the same WebAssembly file run by `lengua-run` beside it. Runs on Linux, macOS and Windows.".to_string()
    } else if screens {
        "One WebAssembly file that serves its own front end. Opens in any current browser. The server reaches only what the idea grants; the browser's sandbox holds the front end.".to_string()
    } else if shape == Shape::Library {
        "A lengua library, built into each program that uses it, as WebAssembly. It reaches nothing of its own.".to_string()
    } else {
        "One WebAssembly file. Runs on any OS and CPU. Sandboxed: it reaches only what the idea grants.".to_string()
    };
    if natives.is_empty() {
        return base;
    }
    let names: Vec<String> = natives
        .iter()
        .map(|n| format!("`{}` (for {})", n.name, n.because))
        .collect();
    format!(
        "{base} Native, beside it: {}, built for this computer and outside the sandbox.",
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::{Place, declares_native, undeclared, where_it_runs};
    use crate::Shape;

    #[test]
    fn a_thread_or_a_process_in_webassembly_is_an_undeclared_native_need() {
        let found = undeclared(
            "src/main.rs",
            "fn main() {\n    let t = std::thread::spawn(work);\n}\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].code, "native-undeclared");
        assert_eq!(undeclared("src/main.rs", "fn main() {}\n"), []);
    }

    #[test]
    fn the_line_says_where_each_kind_runs() {
        let plain = where_it_runs(Shape::Command, &Place::default(), &[], false);
        assert!(plain.starts_with("One WebAssembly file. Runs on any OS and CPU."));
        let native = Place {
            native: Some("the fastest local inference".to_string()),
            desktop: false,
        };
        assert!(where_it_runs(Shape::Command, &native, &[], false).contains("outside the sandbox"));
        let desktop = Place {
            native: None,
            desktop: true,
        };
        assert!(where_it_runs(Shape::Server, &desktop, &[], true).starts_with("An Electron app"));
        assert!(declares_native(
            "# Constraints\n\n- Native: a model, for large AI models\n"
        ));
        assert!(!declares_native(
            "- Native: `../model`, for large AI models\n"
        ));
    }
}
