//! Every x86-64 example and fixture goes through the server: its rules, its tests and, with no
//! time for proofs, nothing more. It is the mirror of `every_aarch64_twin_passes_its_checks`
//! (tests/aarch64_session.rs), so neither CPU has a program the other CPU's suite doesn't check:
//! one product on both Linux targets.

// Its programs are x86-64 code, which runs only on an x86-64 computer; the AArch64 mirror runs
// on an ARM64 one (or under QEMU).
#![cfg(target_arch = "x86_64")]

mod common;

use common::{Program, Protocol, Session};
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// Every program folder of `root`'s `folders`, each with a main.s.
fn programs(root: &std::path::Path, folders: &[&str]) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for folder in folders {
        for entry in std::fs::read_dir(root.join(folder)).expect("a folder") {
            let path = entry.expect("an entry").path();
            if path.join("main.s").is_file() {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Every x86-64 program follows every rule and passes its tests through the server.
#[tokio::test]
async fn every_x86_64_program_passes_its_checks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let session = Session::start(Protocol::Discover).await;
    for folder in programs(root, &["examples", "tests/fixtures"]) {
        let name = folder
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a name")
            .to_owned();
        let program = Program::new();
        for file in ["spec.md", "main.s"] {
            let contents = std::fs::read_to_string(folder.join(file))
                .unwrap_or_else(|error| panic!("{name}/{file}: {error}"));
            program.write(file, &contents);
        }
        session.record(&program).await;
        let report = session.settle(&program).await;
        if name == "second" {
            // Wrong by design, as its AArch64 twin is: one fill reads both lines only when no
            // read ends right after the first, and the tests' own pieces of standard input find
            // it.
            assert_eq!(report["state"], "tests_failed", "{name}: {report}");
            continue;
        }
        assert_eq!(report["state"], "done", "{name}: {report}");
        assert!(
            any_line(&report["tests"], "follows every rule and passed all"),
            "{name}: {report}"
        );
    }
    session.service.cancel().await.expect("close the session");
}
