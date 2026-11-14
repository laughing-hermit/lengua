use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const NOV_13_2026: u64 = 1_794_528_000;
const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            "index.md",
            "log.md",
            "Cargo.toml",
            "wit/world.wit"
        ]
    );
    let top = read(&folder, "idea.md");
    assert!(top.starts_with("---\ntype: lengua Program\ndescription: \"The program counts the words in a text.\"\nmodule: main\nsource: src/main.rs\nuses: [ideas/words]\nused_by: []\nlabel: unchecked\nidea_sha256: "));
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
    assert!(words.contains(
        "used_by: [idea]
"
    ));
    assert!(words.contains(&format!(
        "source_sha256: none
generated: {{ by: lengua-mcp/{VERSION}, at: 2026-11-13T00:00:00Z }}
title: Words
tags: [text]
status: draft
---
"
    )));
    assert!(!words.contains("verified:"));
    assert!(
        words.contains(
            "
# Links

* Used by: ["
        ),
        "{words}"
    );
    assert!(
        words.contains(
            "](../idea.md).
* Index: ["
        ),
        "{words}"
    );
    assert!(
        words.ends_with(
            "](../index.md).
"
        ),
        "{words}"
    );
    assert!(
        top.contains(
            "
# Links

* Uses: [Words](ideas/words.md).
"
        ),
        "{top}"
    );
    assert!(
        top.contains(
            "
title: lengua-idea-"
        ),
        "{top}"
    );
    assert!(!words.contains("label: checked"));
    let index = read(&folder, "index.md");
    assert!(index.starts_with("---\nokf_version: \"0.2\"\n---\n\n# Back end\n\n## Program\n\n* ["));
    assert!(
        index
            .contains("](idea.md) - The program counts the words in a text. Label: `unchecked`.\n")
    );
    assert!(index.contains(
        "
## Modules

* [Words](ideas/words.md) - The module finds the words in a text. Label: `unchecked`.
"
    ));
    let log = read(&folder, "log.md");
    assert!(log.starts_with(
        "# Log\n\n## 2026-11-13\n\n* **Check** at 00:00:00Z: no problems; label `unchecked`.\n"
    ));
    assert!(log.contains("  * Idea [Words](ideas/words.md), new: `"));
    assert!(log.contains("  * Generated `Cargo.toml`, new: `"));
    assert!(log.contains("  * Generated `wit/world.wit`, new: `"));
}

#[test]
fn a_second_check_that_changes_nothing_writes_nothing() {
    let folder = fixture("good", "again");
    let first = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(first.findings, []);
    let log = read(&folder, "log.md");
    let top = read(&folder, "idea.md");
    assert!(read(&folder, "index.md").contains("\n## Crates\n\n* Declared in the ideas: none;"));
    let second = lengua_idea::check(&folder, NOV_13_2026 + 60);
    assert_eq!(second.findings, []);
    assert_eq!(second.written, Vec::<String>::new());
    assert_eq!(read(&folder, "log.md"), log);
    assert_eq!(read(&folder, "idea.md"), top);
}

#[test]
fn a_check_whose_job_is_not_done_keeps_the_label_of_the_same_files() {
    use lengua_idea::{Label, Trust};
    let folder = fixture("good", "pending");
    let checked = Trust {
        label: Label::Checked,
        score: Some((9, 10)),
    };
    let first = lengua_idea::check_with(&folder, NOV_13_2026, |_| Some(checked));
    assert_eq!(first.findings, []);
    let top = read(&folder, "idea.md");
    assert!(
        top.contains("\nverified:\n  - { by: process:lengua-check, at: 2026-11-13T00:00:00Z }\n")
    );
    let pending = lengua_idea::check_with(&folder, NOV_13_2026 + 60, |_| None);
    assert_eq!(pending.written, Vec::<String>::new());
    assert_eq!(read(&folder, "idea.md"), top);
    fs::create_dir_all(folder.join("src")).expect("src");
    fs::write(folder.join("src/words.rs"), "fn count_words() {}\n").expect("a file");
    let changed = lengua_idea::check_with(&folder, NOV_13_2026 + 120, |_| None);
    assert_eq!(changed.findings, []);
    let top = read(&folder, "idea.md");
    assert!(top.contains("\nlabel: unchecked\n"), "{top}");
    assert!(!top.contains("process:lengua-check"), "{top}");
    assert!(top.contains("at: 2026-11-13T00:02:00Z }"), "{top}");
    assert!(read(&folder, "log.md").starts_with(
        "# Log\n\n## 2026-11-13\n\n* **Check** at 00:02:00Z: no problems; label `unchecked`.\n"
    ));
}

#[test]
fn the_links_section_is_check_s_own_and_outside_the_idea_s_hash() {
    let folder = fixture("good", "links");
    assert_eq!(lengua_idea::check(&folder, NOV_13_2026).findings, []);
    let words = read(&folder, "ideas/words.md");
    let edited = words.replace("* Index: [", "* Other: [");
    fs::write(folder.join("ideas/words.md"), &edited).expect("an edit");
    let again = lengua_idea::check(&folder, NOV_13_2026 + 1);
    assert_eq!(again.findings, []);
    assert_eq!(
        read(&folder, "ideas/words.md").replace("00:00:01Z", "00:00:00Z"),
        words
    );
    assert!(!read(&folder, "log.md").contains("Idea [Words](ideas/words.md), changed"));
}

#[test]
fn agent_written_okf_values_are_checked_and_kept() {
    use lengua_idea::{Label, Trust};
    let folder = fixture("good", "okf");
    let words = read(&folder, "ideas/words.md");
    let bad = words.replace(
        "label: checked\n",
        "status: stable\nverified: { by: ana, at: yesterday }\nstale_after: 2026-13-01T00:00:00Z\n",
    );
    fs::write(folder.join("ideas/words.md"), bad).expect("a file");
    let found: Vec<(String, &str)> = lengua_idea::check(&folder, NOV_13_2026)
        .findings
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect();
    for code in ["okf-status", "okf-actor", "okf-time"] {
        assert!(
            found.contains(&("ideas/words.md".to_string(), code)),
            "{code} in {found:?}"
        );
    }
    let good = words.replace(
        "label: checked\n",
        "status: deprecated\nverified: { by: human:ana, at: 2026-11-12T10:00:00+01:00 }\n",
    );
    fs::write(folder.join("ideas/words.md"), good).expect("a file");
    let report = lengua_idea::check_with(&folder, NOV_13_2026, |_| {
        Some(Trust {
            label: Label::Tested,
            score: Some((1, 4)),
        })
    });
    assert_eq!(report.findings, []);
    let written = read(&folder, "ideas/words.md");
    assert!(written.contains("\nverified:\n  - { by: human:ana, at: 2026-11-12T10:00:00+01:00 }\n  - { by: process:lengua-check, at: 2026-11-13T00:00:00Z }\n"), "{written}");
    assert!(written.contains("\nstatus: deprecated\n"), "{written}");
    assert!(!written.contains("status: draft"), "{written}");
}

#[test]
fn every_page_in_the_folder_is_an_okf_concept() {
    let folder = fixture("good", "conformance");
    fs::write(folder.join("notes.md"), "# Notes\n").expect("a file");
    let found: Vec<(String, &str)> = lengua_idea::check(&folder, NOV_13_2026)
        .findings
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect();
    assert_eq!(found, [("notes.md".to_string(), "okf-type")]);
    fs::write(folder.join("notes.md"), "---\ntype: Note\n---\n\n# Notes\n").expect("a file");
    assert_eq!(lengua_idea::check(&folder, NOV_13_2026).findings, []);
}

#[test]
fn a_purpose_that_names_a_module_its_constraints_do_not_list_is_a_finding() {
    let folder = fixture("good", "named");
    let words = read(&folder, "ideas/words.md");
    fs::write(
        folder.join("ideas/words.md"),
        words.replace(
            "A word is a group of characters with no space in it.",
            "The library `json` can write the words.",
        ),
    )
    .expect("a file");
    let found: Vec<(String, &str)> = lengua_idea::program(&folder)
        .findings
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect();
    assert!(
        found.contains(&("ideas/words.md".to_string(), "purpose-unlisted")),
        "{found:?}"
    );
    let top = read(&folder, "idea.md");
    fs::write(
        folder.join("idea.md"),
        top.replace(
            "It reads the text from the standard input.",
            "It reads the text from the standard input, and the module `words` finds the words.",
        ),
    )
    .expect("a file");
    let found: Vec<(String, &str)> = lengua_idea::program(&folder)
        .findings
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect();
    assert!(
        !found.contains(&("idea.md".to_string(), "purpose-unlisted")),
        "{found:?}"
    );
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
        .filter(|e| e.path().join("idea.md").is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert!(names.len() >= 8, "{names:?}");
    for name in names {
        let idea = read(&examples.join(&name), "idea.md");
        if idea.contains("- Uses: `../") || idea.contains("- Uses component:") {
            continue;
        }
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
    let report = lengua_idea::check_with(&folder, NOV_13_2026, |_| {
        Some(Trust {
            label: Label::Checked,
            score: Some((9, 10)),
        })
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
    let tested = lengua_idea::check_with(&folder, NOV_13_2026 + 1, |_| {
        Some(Trust {
            label: Label::Tested,
            score: Some((1, 4)),
        })
    });
    assert_eq!(
        tested.written,
        ["idea.md", "ideas/words.md", "index.md", "log.md"]
    );
    assert!(read(&folder, "log.md").starts_with("# Log\n\n## 2026-11-13\n\n* **Check** at 00:00:01Z: no problems; label `tested`, mutation score 1 of 4 (25%).\n"));
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
            ("ideas/words.md", "two words", false)
        ]
    );
    assert_eq!(build.sources, ["src/main.rs", "src/words.rs"]);
    assert_eq!(build.tests[1].ranges.len(), 1);
    assert_eq!(build.tests[2].stdout.as_deref(), Some("0\n"));
    assert_eq!(build.tests_hash.len(), 64);
    let again = lengua_idea::program(&folder).build.expect("a build");
    assert_eq!(again.inputs, build.inputs);
}

fn example_copy(name: &str, test: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    let to = std::env::temp_dir()
        .join(format!("lengua-idea-{}-{test}", std::process::id()))
        .join(name);
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    to
}

fn codes_of(folder: &Path) -> Vec<(String, &'static str)> {
    lengua_idea::program(folder)
        .findings
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect()
}

#[test]
fn a_program_of_three_modules_is_one_build_of_every_file() {
    let folder = example_copy("tally", "modules");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(report.findings, []);
    let build = report.build.expect("a build");
    assert_eq!(
        build.sources,
        [
            "src/main.rs",
            "src/lines.rs",
            "src/report.rs",
            "src/words.rs"
        ]
    );
    let files: BTreeSet<&str> = build.tests.iter().map(|t| t.file.as_str()).collect();
    assert_eq!(
        files,
        [
            "idea.md",
            "ideas/lines.md",
            "ideas/report.md",
            "ideas/words.md"
        ]
        .into_iter()
        .collect()
    );
    let cargo = read(&folder, "Cargo.toml");
    assert!(cargo.contains(
        "include = [\"src/main.rs\", \"src/lines.rs\", \"src/report.rs\", \"src/words.rs\"]"
    ));
    let index = read(&folder, "index.md");
    assert!(index.contains("* [words](ideas/words.md) - "), "{index}");
}

#[test]
fn a_module_without_its_idea_or_its_mod_line_is_refused() {
    let folder = example_copy("tally", "no-idea");
    fs::remove_file(folder.join("ideas/report.md")).expect("the idea removed");
    let found = codes_of(&folder);
    assert!(
        found.contains(&("src/report.rs".to_string(), "rule-12-source-unlisted")),
        "{found:?}"
    );
    assert!(
        found.contains(&("idea.md".to_string(), "module-missing")),
        "{found:?}"
    );
    let folder = example_copy("tally", "no-mod");
    let main = read(&folder, "src/main.rs");
    fs::write(
        folder.join("src/main.rs"),
        main.replace("mod report;\n", ""),
    )
    .expect("a file");
    assert!(
        codes_of(&folder).contains(&("src/main.rs".to_string(), "module-undeclared")),
        "{:?}",
        codes_of(&folder)
    );
}

const LIBRARY: &str = "# Purpose

The library gives two times a number.

# Behaviours

## Give two times a number

Given the number 2,
When the program calls `double`,
Then it gives 4.

### Example: two

- Call: `double(2)`
- Expect value: `4`

# Constraints

- Memory: 16 MiB
- Time: 2 s
- Exports: `double`

# Structure

- `double`: gives two times a number.
";

#[test]
fn a_library_is_tested_through_calls_and_reaches_nothing() {
    let folder = std::env::temp_dir()
        .join(format!("lengua-idea-{}-library", std::process::id()))
        .join("twice");
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(folder.join("src")).expect("a folder");
    fs::write(folder.join("idea.md"), LIBRARY).expect("the idea");
    fs::write(
        folder.join("src/main.rs"),
        "#[must_use]\npub fn double(number: u64) -> u64 {\n    debug_assert!(number < u64::MAX / 2);\n    debug_assert!(number <= u64::MAX);\n    number * 2\n}\n",
    )
    .expect("the code");
    let report = lengua_idea::check(&folder, NOV_13_2026);
    assert_eq!(report.findings, []);
    let build = report.build.expect("a build");
    assert!(build.library);
    assert_eq!(build.artifact, "target/wasm32-wasip2/release/libtwice.rlib");
    assert!(read(&folder, "Cargo.toml").contains("[lib]\nname = \"twice\"\n"));
    assert!(read(&folder, "idea.md").contains("type: lengua Library\n"));
    let harness = build.harness();
    assert_eq!(harness.package, "twice-harness");
    assert!(harness.grants.reach(lengua_idea::Reach::Arguments));
    let body = |text: &str| fs::write(folder.join("idea.md"), text).expect("the idea");
    body(&LIBRARY.replace("- Exports: `double`", "- Exports: `double`, `triple`"));
    assert!(codes_of(&folder).contains(&("idea.md".to_string(), "export-missing")));
    body(&LIBRARY.replace("- Time: 2 s\n", "- Time: 2 s\n- Reads: arguments\n"));
    assert!(codes_of(&folder).contains(&("idea.md".to_string(), "library-grant")));
    body(&LIBRARY.replace(
        "- Call: `double(2)`\n- Expect value: `4`\n",
        "- Standard input: \"2\"\n- Expect exit code: 0\n",
    ));
    assert!(codes_of(&folder).contains(&("idea.md".to_string(), "example-library-call")));
    body(&LIBRARY.replace(
        "- Time: 2 s\n",
        "- Time: 2 s\n- Serves: HTTP on TCP port 8080\n",
    ));
    assert!(codes_of(&folder).contains(&("idea.md".to_string(), "constraint-kind")));
}

#[test]
fn a_library_that_is_not_checked_is_refused_by_the_program_that_uses_it() {
    let root = std::env::temp_dir().join(format!("lengua-idea-{}-uses", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let library = root.join("libraries/twice");
    fs::create_dir_all(library.join("src")).expect("a folder");
    fs::write(library.join("idea.md"), LIBRARY).expect("the idea");
    let program = root.join("app");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"),
        &program,
    );
    let idea = read(&program, "idea.md");
    fs::write(
        program.join("idea.md"),
        idea.replace(
            "- Time: 2 s\n",
            "- Time: 2 s\n- Uses: `../libraries/twice`\n",
        ),
    )
    .expect("the idea");
    let found = codes_of(&program);
    assert!(
        found.contains(&("idea.md".to_string(), "library-lock")),
        "{found:?}"
    );
    fs::remove_dir_all(&library).expect("removed");
    assert!(codes_of(&program).contains(&("idea.md".to_string(), "library-missing")));
}

#[test]
fn the_label_in_the_frontmatter_does_not_change_the_build_s_inputs() {
    use lengua_idea::{Label, Trust};
    let folder = fixture("good", "inputs");
    let first = lengua_idea::check(&folder, NOV_13_2026);
    let before = first.build.expect("a build").inputs;
    let checked = lengua_idea::check_with(&folder, NOV_13_2026 + 1, |_| {
        Some(Trust {
            label: Label::Checked,
            score: Some((9, 10)),
        })
    });
    assert!(read(&folder, "idea.md").contains("\nverified:\n"));
    assert_eq!(checked.build.expect("a build").inputs, before);
}
