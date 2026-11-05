//! The inventory that keeps the test suite one suite for every target, so that a gap between two
//! targets is a failing test rather than something an audit finds.
//!
//! lengua is one product on every target, tested by the same cases (`PerTarget` tables in
//! `tests/red_team.rs`, `tests/every_program.rs`, `tests/limits.rs` and `src/checker/tests.rs`).
//! Two things keep it so:
//!
//! - A ratchet on test files made for one target: a file that is compiled for one CPU only
//!   (`#![cfg(target_arch = ...)]`) or that names a target in its own name has a twin missing on
//!   the others, so it must be listed in [`DEBT`] with its reason and what the other targets lack.
//!   A new such file fails; a listed file that stops being for one target must leave the list,
//!   so the debt only shrinks.
//! - A native test runner for every target: each target has a job in `.github/workflows` that
//!   runs `cargo test` on a runner of its own CPU, since a table's cells for a target run
//!   natively on that target's runner. A target added to lengua without a runner row fails.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lengua::platform::Profile;
use lengua::targets::PerTarget;
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
/// on one target only. Paying one (a `PerTarget` table that both runners run) means deleting its
/// entry, and the ratchet fails until it is gone.
const DEBT: &[Debt] = &[
    Debt {
        file: "tests/aarch64.rs",
        reason: "the AArch64 templates built and run alone, with no server: one machine \
                 instruction per source line, the template against its spec's examples, fill \
                 and assert_fail as a program of their own, and every word of the templates \
                 read by lengua's decoder as LLVM's disassembler reads it",
        missing: "x86-64 has none of the three standalone tests: its template runs through the \
                  server rather than alone against its spec's examples, fill and assert_fail \
                  are not a program of their own there, and its decoder (iced-x86) is not held \
                  to LLVM's disassembler on every template word",
    },
    Debt {
        file: "tests/aarch64_session.rs",
        reason: "AArch64 programs through the server: the calc, hypotenuse, vector, stream, \
                 tally, server, terminal, echo, fetch and parallel programs, the benchmark's \
                 own programs, proofs, the cut only the proof tries, a closed standard output, \
                 a computer an argument names, a raw read",
        missing: "tests/check_code.rs has the x86-64 side of most of these and about 25 more \
                  (see its own entry), which ARM64 lacks",
    },
    Debt {
        file: "tests/check_code.rs",
        reason: "x86-64 programs through the server: the vector, stream, tally, parallel and \
                 jump-table programs, small ranges, decimals, files, folders, the environment, \
                 sleep, the network, programs built on programs, the proofs of the examples and \
                 what they find in a cut or a closed output, the AVX-512 programs of the \
                 benchmarks",
        missing: "ARM64 has no session test for: small ranges that run every value, decimals, \
                  files, folders, listing, sleep, the environment, dice, TLS, programs built on \
                  programs, and a server that waits for its client",
    },
];

/// The names of a target in a file's name, besides the CPUs and OSes of the targets lengua has.
const ALIASES: &[&str] = &[
    "x86", "x64", "amd64", "arm64", "arm", "macos", "darwin", "windows", "win",
];

/// Each target's native test runner: the label of the GitHub runner on its own CPU.
const RUNNERS: PerTarget<&str> = PerTarget {
    linux_x86_64: "ubuntu-24.04",
    linux_aarch64: "ubuntu-24.04-arm",
};

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
                 table with a cell for each target (lengua::targets::PerTarget), or list the \
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

/// The runners a job's `runs-on` names: the label itself, or, for `${{ matrix.key }}`, every
/// value `key` takes in the job's matrix, listed or included.
fn runners_of(job: &Value, runs_on: &str) -> Vec<String> {
    let Some(key) = runs_on
        .strip_prefix("${{ matrix.")
        .and_then(|rest| rest.strip_suffix(" }}"))
    else {
        return vec![runs_on.to_owned()];
    };
    let matrix = &job["strategy"]["matrix"];
    let listed = matrix[key].as_array().into_iter().flatten();
    let included = matrix["include"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| &entry[key]);
    listed
        .chain(included)
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

/// Every job of the workflow `text` that runs `cargo test`: its name, as `file: job`, and the
/// runner it runs on, once for each runner a matrix names.
fn test_jobs_of(file: &str, text: &str) -> Vec<(String, String)> {
    let workflow: Value =
        serde_saphyr::from_str(text).unwrap_or_else(|error| panic!("{file}: not YAML: {error}"));
    let mut found = Vec::new();
    for (name, job) in workflow["jobs"].as_object().expect("the workflow's jobs") {
        let runs_tests = job["steps"].as_array().is_some_and(|steps| {
            steps.iter().any(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|script| script.contains("cargo test"))
            })
        });
        if runs_tests && let Some(runs_on) = job["runs-on"].as_str() {
            for runner in runners_of(job, runs_on) {
                found.push((format!("{file}: {name}"), runner));
            }
        }
    }
    found
}

/// Every job of every workflow in the repository that runs `cargo test`.
fn test_jobs() -> Vec<(String, String)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&folder).expect("the workflows") {
        let path = entry.expect("an entry").path();
        if !path.extension().is_some_and(|extension| extension == "yml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 name");
        found.extend(test_jobs_of(file, &text));
    }
    found
}

/// A runner a matrix names counts, listed or included, and a job that doesn't run the tests
/// doesn't.
#[test]
fn a_runner_a_matrix_names_is_read() {
    let workflow = "\
jobs:
  tests:
    runs-on: ${{ matrix.runner }}
    strategy:
      matrix:
        runner: [ubuntu-24.04, ubuntu-24.04-arm]
        include:
          - { runner: macos-15, flavor: x }
    steps:
      - run: cargo test --release
  build:
    runs-on: windows-2025
    steps:
      - run: cargo build
";
    let found = test_jobs_of("w.yml", workflow);
    let runners: Vec<&str> = found.iter().map(|(_, runner)| runner.as_str()).collect();
    assert_eq!(runners, ["ubuntu-24.04", "ubuntu-24.04-arm", "macos-15"]);
}

/// Every target has a job that runs `cargo test` on a runner of its own CPU, so the cells of
/// every table run natively on every target: a target added without a runner row fails.
#[test]
fn every_target_has_a_native_test_runner() {
    let jobs = test_jobs();
    assert!(jobs.len() >= 2, "{jobs:#?}");
    for (target, label) in RUNNERS.cells() {
        assert!(
            jobs.iter().any(|(_, runs_on)| runs_on == label),
            "{} has no job that runs `cargo test` on {label} in .github/workflows: add its \
             runner (the jobs found: {jobs:#?})",
            target.name()
        );
    }
}
