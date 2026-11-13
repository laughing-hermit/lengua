use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const NOV_13_2026: u64 = 1_794_528_000;

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a folder");
    for entry in fs::read_dir(from).expect("the fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("a copy");
        }
    }
}

fn fixture(name: &str, test: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let to = std::env::temp_dir().join(format!(
        "lengua-idea-{}-{test}-{name}",
        std::process::id()
    ));
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    to
}

fn read(folder: &Path, file: &str) -> String {
    fs::read_to_string(folder.join(file)).expect("the file")
}

#[test]
fn a_good_program_has_no_findings_and_check_writes_the_wiki() {
    let folder = fixture("good", "wiki");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(report.findings, []);
    assert_eq!(
        report.written,
        ["idea.md", "ideas/words.md", "ideas/ui/summary.md", "index.md", "log.md"]
    );
    let top = read(&folder, "idea.md");
    assert!(top.starts_with("---\ntype: lengua Program\ndescription: \"The program counts the words in a text.\"\nmodule: main\nsource: src/main.rs\nuses: [ideas/words, ideas/ui/summary]\nused_by: []\nlabel: unchecked\nidea_sha256: "));
    assert!(top.contains("\nsource_sha256: none\n---\n\n# Purpose\n"));
    let words = read(&folder, "ideas/words.md");
    assert!(words.contains("type: lengua Module\n"));
    assert!(words.contains("used_by: [idea, ideas/ui/summary]\n"));
    assert!(words.contains("source_sha256: none\ntitle: Words\ntags: [text]\n---\n"));
    assert!(!words.contains("label: checked"));
    let index = read(&folder, "index.md");
    assert!(index.starts_with("---\nokf_version: \"0.2\"\n---\n\n# Program\n\n* ["));
    assert!(index.contains("](idea.md) - The program counts the words in a text. Label: `unchecked`.\n"));
    assert!(index.contains("\n# Modules\n\n* [words](ideas/words.md) - The module finds the words in a text. Label: `unchecked`.\n"));
    assert!(index.contains("\n# Screens\n\n* [ui/summary](ideas/ui/summary.md) - "));
    let log = read(&folder, "log.md");
    assert!(log.starts_with("# Log\n\n## 2026-11-13\n\n* **Check** at 00:00:00Z: no problems; label `unchecked`.\n"));
    assert!(log.contains("  * Idea [words](ideas/words.md), new: `"));
}

#[test]
fn a_second_check_changes_only_the_log() {
    let folder = fixture("good", "again");
    let first = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(first.findings, []);
    let second = lengua_idea::check(&folder, NOV_13_2026 + 60);
    assert_eq!(second.findings, []);
    assert_eq!(second.written, ["log.md"]);
    let log = read(&folder, "log.md");
    assert!(log.starts_with("# Log\n\n## 2026-11-13\n\n* **Check** at 00:01:00Z: no problems; label `unchecked`; nothing changed.\n* **Check** at 00:00:00Z"));
}

#[test]
fn code_that_changes_without_its_idea_gives_a_note() {
    let folder = fixture("good", "drift");
    fs::create_dir_all(folder.join("src")).expect("src");
    fs::write(folder.join("src/words.rs"), "fn count_words() {}\n").expect("a file");
    assert_eq!(lengua_idea::check(&folder, NOV_13_2026).findings, []);
    fs::write(folder.join("src/words.rs"), "fn count_words() { }\n").expect("a file");
    let report = lengua_idea::check(&folder, NOV_13_2026 + 1);
    assert_eq!(report.findings, []);
    assert_eq!(report.notes.len(), 1);
    assert_eq!(report.notes[0].file, "ideas/words.md");
    assert!(report.notes[0].message.contains("`src/words.rs` changed"));
    assert!(read(&folder, "log.md").contains("  * Code `src/words.rs`, changed: `"));
}

const EVERY_KIND: [&str; 26] = [
    "behaviour-example-missing",
    "behaviour-order",
    "behaviour-then-missing",
    "case-range",
    "case-range-unused",
    "case-variable-unknown",
    "constraint-memory-missing",
    "constraint-unknown",
    "constraint-value",
    "example-exit-code-missing",
    "example-key",
    "example-string",
    "frontmatter-line",
    "idea-name",
    "module-missing",
    "module-unlisted",
    "purpose-empty",
    "section-unknown",
    "ste-passive",
    "ste-sentence-length",
    "ste-word",
    "ste-word-unapproved",
    "structure-main-missing",
    "structure-name",
    "structure-one-sentence",
    "uses-unknown",
];

#[test]
fn a_bad_program_gives_every_problem_at_once_in_order() {
    let folder = fixture("bad", "every");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    let found: BTreeSet<&str> = report.findings.iter().map(|f| f.code).collect();
    let wanted: BTreeSet<&str> = EVERY_KIND.into_iter().collect();
    let missing: Vec<&&str> = wanted.difference(&found).collect();
    assert!(missing.is_empty(), "missing {missing:?} in {:#?}", report.findings);
    let places: Vec<(&str, usize)> = report
        .findings
        .iter()
        .map(|f| (f.file.as_str(), f.line))
        .collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    assert_eq!(places, sorted);
    assert!(report.written.is_empty());
    assert!(!folder.join("index.md").exists());
    assert!(!folder.join("log.md").exists());
}

#[test]
fn a_folder_without_an_idea_is_one_finding() {
    let folder = std::env::temp_dir().join(format!("lengua-idea-{}-empty", std::process::id()));
    fs::create_dir_all(&folder).expect("a folder");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].code, "idea-missing");
}
