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
                let path = skills_dir().join(&folder).join(target);
                assert!(path.is_file(), "{folder} links to missing {target}");
            }
            rest = &after[end..];
        }
    }
}

#[test]
fn the_spec_template_is_a_valid_spec() {
    let text = read(&skills_dir().join("write-spec/assets/spec-template.md"));
    if let Err(problems) = lengua::spec::parse(&text) {
        panic!("the template has problems: {problems:#?}");
    }
}

#[test]
fn specs_shown_in_skills_are_valid() {
    let mut checked = 0;
    for (folder, text) in skills() {
        let mut block: Option<Vec<&str>> = None;
        for line in text.lines() {
            match (&mut block, line) {
                (None, "```markdown") => block = Some(Vec::new()),
                (Some(lines), "```") => {
                    let spec = lines.join("\n") + "\n";
                    if spec.contains("type: lengua/program-spec") {
                        if let Err(problems) = lengua::spec::parse(&spec) {
                            panic!("{folder} shows an invalid spec: {problems:#?}");
                        }
                        checked += 1;
                    }
                    block = None;
                }
                (Some(lines), line) => lines.push(line),
                (None, _) => {}
            }
        }
    }
    assert!(checked > 0, "no example specs found");
}
