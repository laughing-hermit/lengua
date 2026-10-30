//! The shipped skills follow the Agent Skills format (agentskills.io), and every spec they show
//! is one lengua accepts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

fn skills_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin/skills")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Every skill as (folder name, SKILL.md text).
fn skills() -> Vec<(String, String)> {
    let mut skills: Vec<_> = std::fs::read_dir(skills_dir())
        .expect("list plugin/skills")
        .map(|entry| entry.expect("skill folder").path())
        .filter(|path| path.is_dir())
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 folder name")
                .to_owned();
            (name, read(&path.join("SKILL.md")))
        })
        .collect();
    skills.sort();
    skills
}

/// The YAML frontmatter and the body.
fn split(text: &str) -> (&str, &str) {
    let rest = text
        .strip_prefix("---\n")
        .expect("SKILL.md starts with frontmatter");
    let end = rest.find("\n---\n").expect("frontmatter is closed");
    (&rest[..end], &rest[end + 5..])
}

/// The fields the Agent Skills specification allows.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
#[allow(dead_code)]
struct Frontmatter {
    name: String,
    description: String,
    license: Option<String>,
    compatibility: Option<String>,
    metadata: Option<BTreeMap<String, String>>,
    allowed_tools: Option<String>,
}

fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[test]
fn every_skill_follows_the_agent_skills_format() {
    let skills = skills();
    assert!(!skills.is_empty(), "the plugin ships skills");
    for (folder, text) in skills {
        let (yaml, body) = split(&text);
        let frontmatter: Frontmatter = serde_saphyr::from_str(yaml)
            .unwrap_or_else(|error| panic!("{folder}: bad frontmatter: {error}"));
        assert_eq!(frontmatter.name, folder, "name must match the folder");
        assert!(valid_name(&frontmatter.name), "{folder}: invalid name");
        let description = frontmatter.description.chars().count();
        assert!(
            (1..=1024).contains(&description),
            "{folder}: description is {description} chars"
        );
        if let Some(compatibility) = &frontmatter.compatibility {
            assert!(
                compatibility.chars().count() <= 500,
                "{folder}: compatibility too long"
            );
        }
        assert!(
            body.lines().count() <= 500,
            "{folder}: keep SKILL.md under 500 lines"
        );
        assert!(
            lengua::server::INSTRUCTIONS.contains(&folder),
            "the server instructions don't mention {folder}"
        );
    }
}

#[test]
fn skills_link_only_to_files_that_exist() {
    for (folder, text) in skills() {
        let mut rest = text.as_str();
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let end = after.find(')').expect("closed link");
            let target = &after[..end];
            if !target.contains("://") && !target.starts_with('#') {
                let file = target.split('#').next().unwrap_or(target);
                let path = skills_dir().join(&folder).join(file);
                assert!(path.is_file(), "{folder} links to missing {target}");
            }
            rest = &after[end..];
        }
    }
}

#[test]
fn the_assembly_template_is_a_complete_lengua_program() {
    let assets = skills_dir().join("compile/assets");
    let spec_text = read(&assets.join("template-spec.md"));
    let spec = lengua::spec::parse(&spec_text)
        .unwrap_or_else(|problems| panic!("the template's spec has problems: {problems:#?}"));
    if let Err(problems) = lengua::testfile::check(&spec) {
        panic!("the template's tests fail check: {problems:#?}");
    }
    let code = read(&assets.join("main-template.s"));
    assert_eq!(
        code.lines().next(),
        Some(".intel_syntax noprefix"),
        "main-template.s starts with its syntax, with no label to copy"
    );
}

#[test]
fn the_spec_template_is_a_valid_spec() {
    let text = read(&skills_dir().join("write-spec/assets/spec-template.md"));
    let spec = lengua::spec::parse(&text)
        .unwrap_or_else(|problems| panic!("the template has problems: {problems:#?}"));
    if let Err(problems) = lengua::testfile::check(&spec) {
        panic!("the template's tests fail check: {problems:#?}");
    }
}

/// Every markdown file under plugin/skills, as (path relative to the skills folder, text).
fn documents() -> Vec<(String, String)> {
    fn walk(folder: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(folder).expect("list a skills folder") {
            let path = entry.expect("folder entry").path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                found.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    walk(&skills_dir(), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(skills_dir())
                .expect("inside the skills folder")
                .display()
                .to_string();
            (name, read(&path))
        })
        .collect()
}

/// The fenced code blocks of a markdown text, as (language, content). A block ends at a fence at
/// least as long as the one that opened it, as markdown reads them.
fn code_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, String, Vec<&str>)> = None;
    for line in text.lines() {
        let ticks = line.len() - line.trim_start_matches('`').len();
        let rest = line[ticks..].trim();
        match &mut open {
            None if ticks >= 3 => open = Some((ticks, rest.to_owned(), Vec::new())),
            Some((fence, ..)) if ticks >= *fence && rest.is_empty() => {
                let (_, language, lines) = open.take().expect("an open block");
                blocks.push((language, lines.join("\n") + "\n"));
            }
            Some((_, _, lines)) => lines.push(line),
            None => {}
        }
    }
    blocks
}

/// Each spec the skills show whole is one lengua accepts, its tests included.
#[test]
fn examples_in_the_skills_pass_lenguas_own_checks() {
    let mut specs = 0;
    for (document, text) in documents() {
        for (language, block) in code_blocks(&text) {
            if language == "markdown" && block.contains("type: lengua/program-spec") {
                let spec = lengua::spec::parse(&block).unwrap_or_else(|problems| {
                    panic!("{document} shows an invalid spec: {problems:#?}")
                });
                if let Err(problems) = lengua::testfile::check(&spec) {
                    panic!("{document} shows a spec whose tests fail check: {problems:#?}");
                }
                specs += 1;
            }
        }
    }
    assert!(specs >= 4, "found only {specs} example specs");
}
