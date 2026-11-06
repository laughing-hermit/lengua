//! The inventory that keeps the test suite one suite for every target, so that a gap between two
//! targets is a failing test rather than something an audit finds.
//!
//! lengua is one product on every target, tested by the same cases (`PerCpu` tables in
//! `tests/red_team.rs`, `tests/every_program.rs`, `tests/limits.rs` and `src/checker/tests.rs`).
//! Two things keep it so:
//!
//! - A ratchet on test files made for one target: a file that is compiled for one CPU only
//!   (`#![cfg(target_arch = ...)]`) or that names a target in its own name has a twin missing on
//!   the others, so it must be listed in [`DEBT`] with its reason and what the other targets lack.
//!   A new such file fails; a listed file that stops being for one target must leave the list,
//!   so the debt only shrinks.
//! - A native test runner for every target: `scripts/ci/targets.json` lists the GitHub runner of
//!   each target's own CPU and system, and `ci.yml`'s one test job runs `cargo test` on every
//!   runner the list names, since a table's cells for a target run natively on that target's
//!   runner. A target added to lengua without a runner in the list fails, and so does a second
//!   workflow that runs the suite. The routes (WSL 2 and a Lima VM, in `routes.yml`) run the same
//!   suite through `scripts/ci/suite.sh`, its one definition.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lengua::platform::Profile;
use serde_json::Value;

/// A test file for one target: why it is, and what the other targets lack because of it.
struct Debt {
    /// The file, relative to the repository.
    file: &'static str,
    /// Why it is for one target.
    reason: &'static str,
    /// The counterpart the other targets are missing.
    missing: &'static str,
}

/// Every test file that is for one target today, which is the debt: what the suite still tests
/// on one target only. Paying one (a `PerCpu` table that both runners run) means deleting its
/// entry, and the ratchet fails until it is gone.
const DEBT: &[Debt] = &[];

/// The names of a target in a file's name, besides the CPUs and OSes of the targets lengua has.
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
    for target in Profile::ALL {
        names.push(target.cpu().to_owned());
        names.extend(target.name().split('-').map(str::to_owned));
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

/// A new test file for one target fails; so does a listed one that no longer is, and a listed
/// one that doesn't say why and what is missing.
#[test]
fn no_new_test_file_is_for_one_target() {
    let found = files_for_one_target();
    let mut problems = Vec::new();
    for (file, why) in &found {
        if !DEBT.iter().any(|debt| debt.file == file) {
            problems.push(format!(
                "{file} is for one target ({why}) and is not in DEBT: write the cases once, as a \
                 table with a cell for each target (lengua::targets::PerCpu), or list the \
                 file with its reason and the counterpart the other targets lack"
            ));
        }
    }
    for debt in DEBT {
        if !found.contains_key(debt.file) {
            problems.push(format!(
                "{} is in DEBT but is not a file for one target any more: delete its entry",
                debt.file
            ));
        }
        if debt.reason.len() < 40 || debt.missing.len() < 40 {
            problems.push(format!(
                "{}: say why it is for one target, and what the others lack",
                debt.file
            ));
        }
    }
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
    let gated = "//! Docs.\n\n#![cfg(target_arch = \"x86_64\")]\n\nmod common;\n";
    assert!(why_for_one_target("e2e.rs", gated).is_some());
    let gated_by_os = "#![cfg(all(target_os = \"linux\", unix))]\n";
    assert!(why_for_one_target("sandbox.rs", gated_by_os).is_some());
    // A comment, a use in a function and a name with no target in it are not a gate.
    let commented = "// #![cfg(target_arch = \"x86_64\")]\nfn f() {}\n";
    assert!(why_for_one_target("red_team.rs", commented).is_none());
    let used = "fn f() { if cfg!(target_arch = \"x86_64\") {} }\n";
    assert!(why_for_one_target("red_team.rs", used).is_none());
    assert!(why_for_one_target("every_program.rs", "").is_none());
    assert!(why_for_one_target("limits.rs", "").is_none());
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

/// Every target has a runner in the list, and `ci.yml`'s test job runs `cargo test` on every
/// runner the list names, so the cells of every table run natively on every target: a target
/// added without a runner fails.
#[test]
fn every_target_has_a_native_test_runner() {
    let hosts = hosts();
    for target in Profile::ALL {
        let label = hosts.get(target.name()).unwrap_or_else(|| {
            panic!(
                "{} has no runner in scripts/ci/targets.json: add its GitHub runner",
                target.name()
            )
        });
        assert!(!label.is_empty(), "{}: an empty runner", target.name());
    }
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

/// The suite has one definition, `scripts/ci/suite.sh`: `ci.yml`'s test job runs it on the
/// runners, and `routes.yml` runs it inside WSL 2 and a Lima VM through `scripts/ci/guest.sh`, so
/// the routes for Windows and a Mac are tested by the targets' own suite.
#[test]
fn the_routes_run_the_targets_suite() {
    let workflows = workflows();
    let ci: Value = serde_saphyr::from_str(workflows.get("ci.yml").expect("ci.yml"))
        .unwrap_or_else(|error| panic!("ci.yml: not YAML: {error}"));
    assert!(
        runs(&ci["jobs"]["test"], "scripts/ci/suite.sh"),
        "ci.yml's test job doesn't run scripts/ci/suite.sh"
    );
    let routes: Value = serde_saphyr::from_str(workflows.get("routes.yml").expect("routes.yml"))
        .unwrap_or_else(|error| panic!("routes.yml: not YAML: {error}"));
    for job in ["wsl", "lima"] {
        assert!(
            runs(&routes["jobs"][job], "scripts/ci/guest.sh"),
            "routes.yml's {job} job doesn't run scripts/ci/guest.sh"
        );
    }
    let guest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/ci/guest.sh"))
            .expect("scripts/ci/guest.sh");
    assert!(
        guest.contains("scripts/ci/suite.sh"),
        "scripts/ci/guest.sh doesn't run scripts/ci/suite.sh"
    );
}
