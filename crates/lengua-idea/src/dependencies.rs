use std::fs;
use std::path::{Path, PathBuf};

use crate::sha256;

const CACHE: &str = "target/lengua/crates.txt";
const NATIVE_BUILDS: [&str; 5] = ["cc", "cmake", "pkg-config", "bindgen", "vcpkg"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Count {
    pub declared: Vec<(String, String)>,
    pub brought: Option<usize>,
    pub unsafe_code: Vec<String>,
    pub native: Vec<String>,
    pub unread: Vec<String>,
}

fn value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    Some(rest.trim().trim_matches('"').to_string())
}

fn packages(lock: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current: (Option<String>, Option<String>, bool) = (None, None, false);
    let mut finish = |current: &mut (Option<String>, Option<String>, bool)| {
        if let (Some(name), Some(version), true) = std::mem::take(current) {
            out.push((name, version));
        }
    };
    for line in lock.lines() {
        if line.trim() == "[[package]]" {
            finish(&mut current);
        } else if let Some(name) = value(line, "name") {
            current.0 = Some(name);
        } else if let Some(version) = value(line, "version") {
            current.1 = Some(version);
        } else if let Some(source) = value(line, "source") {
            current.2 = source.starts_with("registry+") || source.starts_with("git+");
        }
    }
    finish(&mut current);
    out.sort();
    out.dedup();
    out
}

fn cargo_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".cargo"))
}

fn source_dir(home: &Path, name: &str, version: &str) -> Option<PathBuf> {
    let registries = fs::read_dir(home.join("registry").join("src")).ok()?;
    registries
        .flatten()
        .map(|entry| entry.path().join(format!("{name}-{version}")))
        .find(|dir| dir.join("Cargo.toml").is_file())
}

fn word_at(line: &str, at: usize, word: &str) -> bool {
    let before = line[..at].chars().next_back();
    let after = line[at + word.len()..].chars().next();
    let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    edge(before) && edge(after)
}

fn has_unsafe(text: &str) -> bool {
    text.lines().any(|line| {
        let code = line.split("//").next().unwrap_or_default();
        code.match_indices("unsafe")
            .any(|(at, word)| word_at(code, at, word))
    })
}

fn uses_unsafe(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(place) = stack.pop() {
        let Ok(entries) = fs::read_dir(&place) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && fs::read_to_string(&path).is_ok_and(|text| has_unsafe(&text))
            {
                return true;
            }
        }
    }
    false
}

fn links_a_library(dir: &Path) -> bool {
    fs::read_to_string(dir.join("build.rs")).is_ok_and(|script| {
        script.contains("rustc-link-lib") || script.contains("rustc-link-search")
    })
}

fn not_pure(dir: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    let mut build_section = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            build_section = line
                .trim_matches(['[', ']'])
                .ends_with("build-dependencies");
            continue;
        }
        if value(line, "links").is_some() && links_a_library(dir) {
            return true;
        }
        let key = line
            .split(['=', '.', ' '])
            .next()
            .unwrap_or_default()
            .trim_matches('"');
        if build_section && NATIVE_BUILDS.contains(&key) {
            return true;
        }
    }
    false
}

fn read_cache(folder: &Path, hash: &str) -> Option<Count> {
    let text = fs::read_to_string(folder.join(CACHE)).ok()?;
    let mut lines = text.lines();
    if lines.next()? != hash {
        return None;
    }
    let mut count = Count {
        brought: Some(0),
        ..Count::default()
    };
    for line in lines {
        match line.split_once(' ') {
            Some(("brought", n)) => count.brought = n.parse().ok(),
            Some(("unsafe", name)) => count.unsafe_code.push(name.to_string()),
            Some(("native", name)) => count.native.push(name.to_string()),
            Some(("unread", name)) => count.unread.push(name.to_string()),
            _ => {}
        }
    }
    Some(count)
}

fn write_cache(folder: &Path, hash: &str, count: &Count) {
    let mut text = format!("{hash}\nbrought {}\n", count.brought.unwrap_or(0));
    for (tag, names) in [
        ("unsafe", &count.unsafe_code),
        ("native", &count.native),
        ("unread", &count.unread),
    ] {
        for name in names {
            text.push_str(tag);
            text.push(' ');
            text.push_str(name);
            text.push('\n');
        }
    }
    let path = folder.join(CACHE);
    let _ = path.parent().map(fs::create_dir_all);
    let _ = fs::write(path, text);
}

pub(crate) fn count(folder: &Path, declared: &[(String, String)]) -> Count {
    let mut count = Count {
        declared: declared.to_vec(),
        ..Count::default()
    };
    if declared.is_empty() {
        count.brought = Some(0);
        return count;
    }
    let Ok(lock) = fs::read_to_string(folder.join("Cargo.lock")) else {
        return count;
    };
    let hash = sha256::hex(format!("crates 2\0{lock}").as_bytes());
    if let Some(cached) = read_cache(folder, &hash) {
        return Count {
            declared: count.declared,
            ..cached
        };
    }
    let external = packages(&lock);
    count.brought = Some(
        external
            .iter()
            .filter(|(name, _)| !declared.iter().any(|(d, _)| d == name))
            .count(),
    );
    let home = cargo_home();
    for (name, version) in &external {
        let label = format!("{name} {version}");
        let Some(dir) = home.as_deref().and_then(|h| source_dir(h, name, version)) else {
            count.unread.push(label);
            continue;
        };
        if uses_unsafe(&dir) {
            count.unsafe_code.push(label.clone());
        }
        if not_pure(&dir) {
            count.native.push(label);
        }
    }
    if count.unread.is_empty() {
        write_cache(folder, &hash, &count);
    }
    count
}

fn names(list: &[String]) -> String {
    let named: Vec<String> = list
        .iter()
        .map(|item| match item.split_once(' ') {
            Some((name, version)) => format!("`{name}` {version}"),
            None => format!("`{item}`"),
        })
        .collect();
    named.join(", ")
}

impl Count {
    pub(crate) fn lines(&self) -> Vec<String> {
        if self.declared.is_empty() {
            return vec![
                "* Declared in the ideas: none; the code uses the Rust standard library alone.\n"
                    .to_string(),
            ];
        }
        let declared: Vec<String> = self
            .declared
            .iter()
            .map(|(name, version)| format!("{name} {version}"))
            .collect();
        let mut out = vec![format!(
            "* Declared in the ideas: {}: {}.\n",
            declared.len(),
            names(&declared)
        )];
        let Some(brought) = self.brought else {
            out.push(
                "* Brought in by them: not known until the first build writes `Cargo.lock`.\n"
                    .to_string(),
            );
            return out;
        };
        out.push(format!("* Brought in by them: {brought}.\n"));
        let groups = [
            ("With `unsafe` in their Rust code", &self.unsafe_code),
            (
                "Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies)",
                &self.native,
            ),
            (
                "Not read (their source is not in Cargo's folder)",
                &self.unread,
            ),
        ];
        for (what, list) in groups {
            if list.is_empty() && what.starts_with("Not read") {
                continue;
            }
            let text = if list.is_empty() {
                "none".to_string()
            } else {
                format!("{}: {}", list.len(), names(list))
            };
            out.push(format!("* {what}: {text}.\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{has_unsafe, packages};

    #[test]
    fn the_external_packages_come_from_the_lock() {
        let lock = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"wasip2\"\nversion = \"2.0.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"ab\"\n";
        assert_eq!(
            packages(lock),
            [("wasip2".to_string(), "2.0.1".to_string())]
        );
    }

    #[test]
    fn unsafe_is_a_word_outside_comments() {
        assert!(has_unsafe(&["fn f() { ", "unsafe", " { g() } }"].concat()));
        assert!(!has_unsafe("// unsafe here\nlet not_unsafe_at_all = 1;"));
    }
}
