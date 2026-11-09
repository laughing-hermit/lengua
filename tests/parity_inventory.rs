//! The inventory that keeps the test suite one suite for every target, so that a gap between two
//! targets is a failing test rather than something an audit finds.
//!
//! lengua is one product on every target: its programs are WebAssembly, which runs the same way on
//! each of the five runners lengua-mcp is built for, and every test file runs on every one of
//! them. Two things keep it so:
//!
//! - No test file is made for one target: a file that is compiled for some targets only
//!   (`#![cfg(target_arch = ...)]`) or that names a target in its own name fails. A case that only
//!   some systems can take, such as one that starts `sh`, is a test of its own gated where it
//!   stands, in a file every runner runs.
//! - A test runner for every target: `scripts/ci/targets.json` lists the GitHub runner of each of
//!   the five targets, and `ci.yml`'s one test job runs `cargo test` on every runner the list
//!   names. A target without a runner in the list fails, and so does a second workflow that runs
//!   the suite.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The names of a target in a file's name, besides the systems and CPUs of the five targets.
const ALIASES: &[&str] = &[
    "x86", "x64", "amd64", "arm64", "arm", "macos", "darwin", "windows", "win",
];

/// The words of a file's name, split at every character that isn't a letter or a digit, that
/// name a target.
fn target_words(file_name: &str) -> Vec<String> {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let mut names: Vec<String> = ALIASES.iter().map(|alias| (*alias).to_owned()).collect();
    for target in lengua::gate::TARGETS {
        names.extend(target.split('-').map(str::to_owned));
    }
    stem.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| names.iter().any(|name| name == word))
        .map(str::to_owned)
        .collect()
}

/// Whether `text` has an inner attribute that compiles the whole file for some targets only,
/// such as `#![cfg(target_arch = "x86_64")]`.
fn gated_to_a_target(text: &str) -> bool {
    text.lines().map(str::trim_start).any(|line| {
        line.starts_with("#![cfg(")
            && [
                "target_arch",
                "target_os",
                "target_family",
                "unix",
                "windows",
            ]
            .iter()
            .any(|key| line.contains(key))
    })
}

/// Why a test file is for one target, if it is: its name, or the attribute that gates it.
fn why_for_one_target(file_name: &str, text: &str) -> Option<String> {
    let words = target_words(file_name);
    if !words.is_empty() {
        return Some(format!("its name says {}", words.join(", ")));
    }
    gated_to_a_target(text).then(|| "it is compiled for some targets only".to_owned())
}

/// Every `.rs` file under `dir`, but not programs and schemas, which are data.
fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a folder of tests") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            let name = path.file_name().and_then(|name| name.to_str());
            if !matches!(name, Some("fixtures" | "schemas")) {
                rust_files(&path, found);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Every test file that is for one target, with why, by its path relative to the repository.
fn files_for_one_target() -> BTreeMap<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("tests"), &mut files);
    assert!(files.len() >= 15, "{} test files found", files.len());
    let mut found = BTreeMap::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 name");
        if let Some(why) = why_for_one_target(name, &text) {
            let relative = path
                .strip_prefix(root)
                .expect("inside the repository")
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative, why);
        }
    }
    found
}

/// No test file is for one target: every runner runs every file.
#[test]
fn no_test_file_is_for_one_target() {
    let problems: Vec<String> = files_for_one_target()
        .into_iter()
        .map(|(file, why)| {
            format!(
                "{file} is for one target ({why}): every runner runs every test file, so write \
                 its cases for every target, and gate a case only some systems can take where it \
                 stands"
            )
        })
        .collect();
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The ratchet sees a file made for one target by its name and by its attribute, and passes the
/// files made for all of them.
#[test]
fn the_ratchet_sees_a_file_for_one_target() {
    assert!(why_for_one_target("adversarial_aarch64.rs", "").is_some());
    assert!(why_for_one_target("aarch64.rs", "").is_some());
    assert!(why_for_one_target("x86-64-things.rs", "").is_some());
    assert!(why_for_one_target("macos_runner.rs", "").is_some());
    assert!(why_for_one_target("windows.rs", "").is_some());
    assert!(why_for_one_target("linux_only.rs", "").is_some());
    let gated = "//! Docs.\n\n#![cfg(target_arch = \"x86_64\")]\n\nmod common;\n";
    assert!(why_for_one_target("e2e.rs", gated).is_some());
    let gated_by_os = "#![cfg(all(target_os = \"linux\", unix))]\n";
    assert!(why_for_one_target("sandbox.rs", gated_by_os).is_some());
    // A comment, a use in a function, a gated test and a name with no target in it are not a
    // gate.
    let commented = "// #![cfg(target_arch = \"x86_64\")]\nfn f() {}\n";
    assert!(why_for_one_target("wasm_red_team.rs", commented).is_none());
    let used = "fn f() { if cfg!(target_arch = \"x86_64\") {} }\n";
    assert!(why_for_one_target("wasm_red_team.rs", used).is_none());
    let one_test = "#[cfg(unix)]\n#[test]\nfn f() {}\n";
    assert!(why_for_one_target("workflow.rs", one_test).is_none());
    assert!(why_for_one_target("wasm_every_program.rs", "").is_none());
    assert!(why_for_one_target("wasm_limits.rs", "").is_none());
    assert!(why_for_one_target("parity_inventory.rs", "").is_none());
}

/// The runners `scripts/ci/targets.json` lists: the label of the GitHub runner of each target,
/// by the target's name.
fn hosts() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/ci/targets.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let list: Value = serde_json::from_str(&text).expect("targets.json is JSON");
    list.as_array()
        .expect("targets.json is a list")
        .iter()
        .map(|entry| {
            let field = |name: &str| {
                entry[name]
                    .as_str()
                    .unwrap_or_else(|| panic!("an entry of targets.json has no {name}: {entry}"))
                    .to_owned()
            };
            (field("target"), field("runner"))
        })
        .collect()
}

/// The workflows of the repository, by file name, as text.
fn workflows() -> BTreeMap<String, String> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    std::fs::read_dir(&folder)
        .expect("the workflows")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "yml"))
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a UTF-8 name")
                .to_owned();
            (name, text)
        })
        .collect()
}

/// Whether `job` has a step whose script contains `needle`.
fn runs(job: &Value, needle: &str) -> bool {
    job["steps"].as_array().is_some_and(|steps| {
        steps
            .iter()
            .any(|step| step["run"].as_str().is_some_and(|run| run.contains(needle)))
    })
}

/// Every one of the five targets has a runner in the list, the list names no other, and
/// `ci.yml`'s test job runs `cargo test` on every runner it names: a target added without a
/// runner fails.
#[test]
fn every_target_has_a_test_runner() {
    let hosts = hosts();
    for target in lengua::gate::TARGETS {
        let label = hosts.get(target).unwrap_or_else(|| {
            panic!("{target} has no runner in scripts/ci/targets.json: add its GitHub runner")
        });
        assert!(!label.is_empty(), "{target}: an empty runner");
    }
    let listed: Vec<&str> = hosts.keys().map(String::as_str).collect();
    let mut targets = lengua::gate::TARGETS.to_vec();
    targets.sort_unstable();
    assert_eq!(listed, targets, "the targets scripts/ci/targets.json lists");
    let workflows = workflows();
    let ci: Value = serde_saphyr::from_str(workflows.get("ci.yml").expect("ci.yml"))
        .unwrap_or_else(|error| panic!("ci.yml: not YAML: {error}"));
    // The plan reads the list, and the test job runs on what the plan made of it.
    assert!(
        runs(&ci["jobs"]["plan"], "scripts/ci/targets.json"),
        "ci.yml's plan job doesn't read scripts/ci/targets.json"
    );
    let test = &ci["jobs"]["test"];
    assert_eq!(test["runs-on"], "${{ matrix.runner }}");
    assert_eq!(
        test["strategy"]["matrix"],
        "${{ fromJSON(needs.plan.outputs.matrix) }}"
    );
    assert!(runs(test, "cargo test"), "ci.yml's test job runs no tests");
}

/// One workflow runs the suite: a second would be a second definition of how the targets are
/// tested, and the two would drift.
#[test]
fn one_workflow_runs_the_suite() {
    let running: Vec<String> = workflows()
        .into_iter()
        .filter(|(_, text)| text.contains("cargo test"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(running, ["ci.yml"], "the workflows that run `cargo test`");
}
