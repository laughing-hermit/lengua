//! Every idea file in the repository is written in ASD-STE100 Simplified Technical English, as
//! `check` holds an idea to it: no sentence over 25 words, no paragraph over six sentences, no
//! -ing form but lengua's technical names, and every step in lengua's STE phrasings.

use std::path::{Path, PathBuf};

/// The folders that hold idea files, from the repository's root.
const FOLDERS: [&str; 4] = [
    "examples",
    "tests/fixtures",
    "tests/fixtures-wasm",
    "plugin/skills",
];

/// Every idea file under `folder`, in order: each `idea.md`, each `*-idea.md`, and the write-idea
/// skill's `idea-template.md`.
fn ideas(folder: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("list {}: {error}", folder.display()))
        .map(|entry| entry.expect("a folder entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            ideas(&path, found);
        } else if name == "idea.md" || name.ends_with("-idea.md") || name == "idea-template.md" {
            found.push(path);
        }
    }
}

#[test]
fn every_idea_file_is_written_in_ste() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for folder in FOLDERS {
        ideas(&root.join(folder), &mut files);
    }
    assert!(files.len() >= 10, "found only {} idea files", files.len());
    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let Err(problems) = lengua::idea::parse(&text) else {
            continue;
        };
        let shown = path.strip_prefix(&root).unwrap_or(path).display();
        for problem in problems {
            if problem.message.starts_with("STE:") {
                failures.push(format!("{shown}: {problem}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "idea files that aren't in STE:\n{}",
        failures.join("\n")
    );
}
