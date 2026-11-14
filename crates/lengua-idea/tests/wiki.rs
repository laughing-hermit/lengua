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

fn tree(test: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("lengua-wiki-{}-{test}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("an old copy removed");
    }
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/good"),
        &root.join("apps/good"),
    );
    root
}

fn codes(root: &Path) -> Vec<(String, &'static str)> {
    lengua_idea::wiki_findings(root)
        .into_iter()
        .map(|f| (f.file, f.code))
        .collect()
}

#[test]
fn a_tree_of_folders_gets_its_root_indexes_and_keeps_to_okf() {
    let root = tree("indexes");
    assert_eq!(
        lengua_idea::check(&root.join("apps/good"), NOV_13_2026).findings,
        []
    );
    fs::write(
        root.join("README.md"),
        "---\ntype: Overview\ndescription: A tree of lengua folders.\n---\n\n# Tree\n\nSee [the apps](apps/index.md).\n",
    )
    .expect("a file");
    let before = codes(&root);
    for wanted in [
        ("README.md", "wiki-link"),
        ("apps/index.md", "wiki-index-stale"),
        ("index.md", "wiki-index-stale"),
    ] {
        assert!(
            before.contains(&(wanted.0.to_string(), wanted.1)),
            "{wanted:?} in {before:?}"
        );
    }
    let written = lengua_idea::write_wiki_indexes(&root).expect("the indexes");
    assert_eq!(written, ["index.md", "apps/index.md"]);
    assert_eq!(codes(&root), []);
    let index = fs::read_to_string(root.join("index.md")).expect("the index");
    assert_eq!(
        index,
        "---\nokf_version: \"0.2\"\n---\n\n# Documents\n\n* [README](README.md) - A tree of lengua folders.\n\n# Folders\n\n* [apps](apps/index.md) - 1 lengua folder: 1 program.\n"
    );
    let apps = fs::read_to_string(root.join("apps/index.md")).expect("the index");
    assert_eq!(
        apps,
        "# Programs\n\n* [good](good/index.md) - The program counts the words in a text. Label: `unchecked`.\n"
    );
    assert_eq!(
        lengua_idea::write_wiki_indexes(&root).expect("the indexes"),
        Vec::<String>::new()
    );
}

#[test]
fn a_stale_page_a_broken_link_and_a_page_with_no_type_are_findings() {
    let root = tree("stale");
    assert_eq!(
        lengua_idea::check(&root.join("apps/good"), NOV_13_2026).findings,
        []
    );
    lengua_idea::write_wiki_indexes(&root).expect("the indexes");
    assert_eq!(codes(&root), []);
    let words = root.join("apps/good/ideas/words.md");
    let text = fs::read_to_string(&words).expect("the idea");
    fs::write(&words, text.replace("two words", "2 words")).expect("a change");
    fs::write(
        root.join("notes.md"),
        "# Notes\n\nSee [nothing](nothing.md).\n",
    )
    .expect("a file");
    let found = codes(&root);
    for wanted in [
        ("apps/good/ideas/words.md", "wiki-stale"),
        ("notes.md", "okf-type"),
        ("notes.md", "wiki-link"),
        ("index.md", "wiki-index-stale"),
    ] {
        assert!(
            found.contains(&(wanted.0.to_string(), wanted.1)),
            "{wanted:?} in {found:?}"
        );
    }
}

#[test]
fn the_repository_s_wiki_keeps_to_okf_and_is_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let found: Vec<String> = lengua_idea::wiki_findings(&root)
        .into_iter()
        .map(|f| format!("{}:{}: {}: {}", f.file, f.line, f.code, f.message))
        .collect();
    assert!(
        found.is_empty(),
        "the committed wiki is not current; dispatch ci.yml with the mode `wiki` and commit the files of its artifact `wiki-<run>`:\n{}",
        found.join("\n")
    );
}

fn principle_rows(text: &str, heading: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split_once(heading)
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with('#'))
        .filter(|l| l.starts_with("| ") && l[2..].starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_vision_skill_s_principles_are_the_design_s() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let design = fs::read_to_string(root.join("Lengua3.md")).expect("the design");
    let page = fs::read_to_string(root.join("plugin/skills/vision/references/principles.md"))
        .expect("the principles");
    let wanted = principle_rows(&design, "### The vision: the best lengua solution");
    assert!(wanted.len() >= 9, "{wanted:?}");
    assert_eq!(principle_rows(&page, "## The principles"), wanted);
}
