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
    let to = std::env::temp_dir().join(format!("lengua-idea-{}-{test}-{name}", std::process::id()));
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
        [
            "idea.md",
            "ideas/words.md",
            "ideas/ui/summary.md",
            "index.md",
            "log.md",
            "Cargo.toml",
            "wit/world.wit"
        ]
    );
    let top = read(&folder, "idea.md");
    assert!(top.starts_with("---\ntype: lengua Program\ndescription: \"The program counts the words in a text.\"\nmodule: main\nsource: src/main.rs\nuses: [ideas/words, ideas/ui/summary]\nused_by: []\nlabel: unchecked\nidea_sha256: "));
    assert!(top.contains("\nsource_sha256: none\ncargo_sha256: "));
    assert!(top.contains("\nworld_sha256: "));
    assert!(top.contains("\n---\n\n# Purpose\n"));
    let cargo = read(&folder, "Cargo.toml");
    assert!(cargo.starts_with("[package]\nname = \"lengua-idea-"));
    assert!(cargo.contains("[[bin]]\n"));
    assert!(read(&folder, "wit/world.wit").contains("  import wasi:cli/stdin@0.2.12;\n"));
    assert_eq!(report.build.as_ref().map(|b| b.code), Some(false));
    let words = read(&folder, "ideas/words.md");
    assert!(words.contains("type: lengua Module\n"));
    assert!(words.contains("used_by: [idea, ideas/ui/summary]\n"));
    assert!(words.contains("source_sha256: none\ntitle: Words\ntags: [text]\n---\n"));
    assert!(!words.contains("label: checked"));
    let index = read(&folder, "index.md");
    assert!(index.starts_with("---\nokf_version: \"0.2\"\n---\n\n# Program\n\n* ["));
    assert!(
        index
            .contains("](idea.md) - The program counts the words in a text. Label: `unchecked`.\n")
    );
    assert!(index.contains("\n# Modules\n\n* [words](ideas/words.md) - The module finds the words in a text. Label: `unchecked`.\n"));
    assert!(index.contains("\n# Screens\n\n* [ui/summary](ideas/ui/summary.md) - "));
    let log = read(&folder, "log.md");
    assert!(log.starts_with(
        "# Log\n\n## 2026-11-13\n\n* **Check** at 00:00:00Z: no problems; label `unchecked`.\n"
    ));
    assert!(log.contains("  * Idea [words](ideas/words.md), new: `"));
    assert!(log.contains("  * Generated `Cargo.toml`, new: `"));
    assert!(log.contains("  * Generated `wit/world.wit`, new: `"));
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
fn a_hand_edit_of_a_generated_file_is_a_finding() {
    let folder = fixture("good", "held");
    assert_eq!(lengua_idea::check(&folder, NOV_13_2026).findings, []);
    let cargo = read(&folder, "Cargo.toml");
    let edited = cargo.replace("opt-level = 3", "opt-level = \"z\"");
    fs::write(folder.join("Cargo.toml"), &edited).expect("a file");
    let report = lengua_idea::check(&folder, NOV_13_2026 + 1);
    let found: Vec<(&str, usize, &str)> = report
        .findings
        .iter()
        .map(|f| (f.file.as_str(), f.line, f.code))
        .collect();
    let line = cargo
        .lines()
        .position(|l| l == "opt-level = 3")
        .expect("the line")
        + 1;
    assert_eq!(found, [("Cargo.toml", line, "generated-edited")]);
    assert_eq!(report.written, Vec::<String>::new());
    assert_eq!(report.build, None);
    fs::remove_file(folder.join("Cargo.toml")).expect("the file removed");
    let again = lengua_idea::check(&folder, NOV_13_2026 + 2);
    assert_eq!(again.findings, []);
    assert_eq!(again.written, ["log.md", "Cargo.toml"]);
    assert_eq!(read(&folder, "Cargo.toml"), cargo);
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
    assert_eq!(missing, Vec::<&&str>::new(), "in {:#?}", report.findings);
    let places: Vec<(&str, usize)> = report
        .findings
        .iter()
        .map(|f| (f.file.as_str(), f.line))
        .collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    assert_eq!(places, sorted);
    assert_eq!(report.written, Vec::<String>::new());
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

#[test]
fn the_example_in_the_write_idea_skill_has_no_findings() {
    let skill = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugin/skills/write-idea/SKILL.md"),
    )
    .expect("the skill")
    .replace("\r\n", "\n");
    let idea = skill
        .split("\n````\n")
        .nth(1)
        .expect("an idea between two four-backtick lines");
    let folder = std::env::temp_dir().join(format!("lengua-idea-{}-skill", std::process::id()));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("an old copy removed");
    }
    fs::create_dir_all(&folder).expect("a folder");
    fs::write(folder.join("idea.md"), format!("{idea}\n")).expect("the idea");
    assert_eq!(lengua_idea::check(&folder, NOV_13_2026).findings, []);
}

#[test]
fn the_hello_example_has_no_findings_and_gets_its_build_files() {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello");
    let to = std::env::temp_dir()
        .join(format!("lengua-idea-{}-example", std::process::id()))
        .join("hello");
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    let report = lengua_idea::check(&to, NOV_13_2026);
    assert_eq!(report.findings, []);
    let build = report.build.expect("a build");
    assert_eq!(build.package, "hello");
    assert_eq!(build.artifact, "target/wasm32-wasip2/release/hello.wasm");
    assert_eq!(build.crates, 0);
    assert!(build.code);
    assert!(read(&to, "Cargo.toml").starts_with("[package]\nname = \"hello\"\n"));
    let world = read(&to, "wit/world.wit");
    assert!(world.contains("  import wasi:cli/stdin@0.2.12;\n"));
    assert!(world.contains("  import wasi:random/insecure-seed@0.2.12;\n"));
    for ungranted in [
        "wasi:random/random",
        "wasi:random/insecure@",
        "wasi:filesystem",
        "wasi:sockets",
        "wall-clock",
    ] {
        assert!(!world.contains(ungranted), "{ungranted} in {world}");
    }
}

#[test]
fn every_example_keeps_every_rule() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut names: Vec<String> = fs::read_dir(&examples)
        .expect("the examples")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert!(names.len() >= 8, "{names:?}");
    for name in names {
        let to = std::env::temp_dir()
            .join(format!("lengua-idea-{}-rules", std::process::id()))
            .join(&name);
        if to.exists() {
            fs::remove_dir_all(&to).expect("an old copy removed");
        }
        copy(&examples.join(&name), &to);
        let report = lengua_idea::program(&to);
        assert_eq!(report.findings, [], "{name}");
    }
}

#[test]
fn code_that_breaks_the_rules_is_refused_with_the_idea_s_findings() {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello");
    let to = std::env::temp_dir()
        .join(format!("lengua-idea-{}-broken", std::process::id()))
        .join("hello");
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    fs::write(
        to.join("src/main.rs"),
        "// a comment\nfn main() {\n    helper();\n}\n\nfn helper() {\n    main();\n}\n",
    )
    .expect("the code");
    fs::write(to.join("src/extra.rs"), "fn extra() {}\n").expect("a file");
    fs::write(to.join("build.rs"), "fn main() {}\n").expect("a build script");
    let report = lengua_idea::check(&to, NOV_13_2026);
    let found: Vec<(&str, usize, &str)> = report
        .findings
        .iter()
        .map(|f| (f.file.as_str(), f.line, f.code))
        .collect();
    assert_eq!(
        found,
        [
            ("build.rs", 1, "rule-8-build-script"),
            ("src/extra.rs", 1, "rule-12-source-unlisted"),
            ("src/main.rs", 1, "rule-11-comment"),
            ("src/main.rs", 2, "rule-1-recursion"),
            ("src/main.rs", 6, "rule-1-recursion"),
            ("src/main.rs", 6, "rule-12-unnamed"),
            ("src/main.rs", 6, "rule-5-assertions"),
        ]
    );
    assert_eq!(report.written, Vec::<String>::new());
    assert_eq!(report.build, None);
}

#[test]
fn the_label_and_the_score_go_into_the_wiki() {
    use lengua_idea::{Label, Trust};
    let folder = fixture("good", "label");
    let report = lengua_idea::check_with(&folder, NOV_13_2026, |_| Trust {
        label: Label::Checked,
        score: Some((9, 10)),
    });
    assert_eq!(report.findings, []);
    assert_eq!(report.trust.label, Label::Checked);
    assert!(read(&folder, "idea.md").contains("\nlabel: checked\n"));
    assert!(read(&folder, "ideas/words.md").contains("\nlabel: checked\n"));
    assert!(read(&folder, "index.md").contains(" Label: `checked`.\n"));
    assert!(!read(&folder, "index.md").contains("`unchecked`"));
    assert!(
        read(&folder, "log.md")
            .contains("no problems; label `checked`, mutation score 9 of 10 (90%).\n")
    );
    let tested = lengua_idea::check_with(&folder, NOV_13_2026 + 1, |_| Trust {
        label: Label::Tested,
        score: Some((1, 4)),
    });
    assert_eq!(
        tested.written,
        [
            "idea.md",
            "ideas/words.md",
            "ideas/ui/summary.md",
            "index.md",
            "log.md"
        ]
    );
    assert!(read(&folder, "log.md").starts_with("# Log\n\n## 2026-11-13\n\n* **Check** at 00:00:01Z: no problems; label `tested`, mutation score 1 of 4 (25%); nothing changed.\n"));
}

#[test]
fn the_build_carries_every_test_of_every_idea() {
    let folder = fixture("good", "tests");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    let build = report.build.expect("a build");
    let names: Vec<(&str, &str, bool)> = build
        .tests
        .iter()
        .map(|t| (t.file.as_str(), t.name.as_str(), t.screen))
        .collect();
    assert_eq!(
        names,
        [
            ("idea.md", "two words", false),
            ("idea.md", "one word", false),
            ("idea.md", "empty input", false),
            ("ideas/words.md", "two words", false),
            ("ideas/ui/summary.md", "two words", true)
        ]
    );
    assert_eq!(
        build.sources,
        ["src/main.rs", "src/words.rs", "src/ui/summary.rs"]
    );
    assert_eq!(build.tests[1].ranges.len(), 1);
    assert_eq!(build.tests[2].stdout.as_deref(), Some("0\n"));
    assert_eq!(build.tests_hash.len(), 64);
    let again = lengua_idea::program(&folder).build.expect("a build");
    assert_eq!(again.inputs, build.inputs);
}
