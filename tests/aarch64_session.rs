//! The by-hand port tool for linux-aarch64: `check_one_aarch64_folder` checks the folders
//! `LENGUA_CHECK_FOLDER` names, natively on an ARM64 computer and through QEMU's user mode on an
//! x86-64 one, and prints what a port needs to see. Every other case of the server on AArch64
//! code is written once for both targets (tests/sessions.rs, tests/workflow.rs, tests/programs.rs,
//! tests/network.rs, tests/template.rs, tests/standalone.rs).

mod common;

use common::{Program, Protocol, Session};
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// A program folder holding `files`, its spec recorded.
async fn written(session: &Session, files: [(&str, String); 2]) -> Program {
    let program = Program::new();
    for (name, text) in files {
        program.write(name, &text);
    }
    session.record(&program).await;
    program
}

/// One folder's check, for porting a program: `LENGUA_CHECK_FOLDER` names it, and the test fails
/// unless the job ends done, as `check_one_folder` does for x86-64.
#[tokio::test]
#[ignore = "checks the folder LENGUA_CHECK_FOLDER names: by hand"]
async fn check_one_aarch64_folder() {
    let Some(folder) = std::env::var("LENGUA_CHECK_FOLDER")
        .ok()
        .filter(|folder| !folder.is_empty())
    else {
        return;
    };
    // LENGUA_CHECK_NATIVE checks it as this computer's own program: an x86-64 twin on x86-64.
    let native = std::env::var_os("LENGUA_CHECK_NATIVE").is_some();
    let session = if cfg!(target_arch = "aarch64") || native {
        Session::proving_for(Protocol::Discover, 60).await
    } else if common::qemu_runs_aarch64() {
        Session::emulating_for(Protocol::Discover, "linux-aarch64", 60).await
    } else {
        return;
    };
    let mut failed = Vec::new();
    // Several folders, separated by commas, share one session.
    for folder in folder.split(',') {
        let folder = std::path::PathBuf::from(folder);
        let read = |file: &str| std::fs::read_to_string(folder.join(file)).expect("the file");
        let program = written(
            &session,
            [("spec.md", read("spec.md")), ("main.s", read("main.s"))],
        )
        .await;
        let report = session.settle(&program).await;
        println!("PORT folder: {}", folder.display());
        println!("PORT state: {}", report["state"]);
        for field in ["problems", "tests", "proofs"] {
            for line in report[field].as_array().into_iter().flatten() {
                println!("PORT {field}: {}", text(line));
            }
        }
        if report["state"] != "done" {
            failed.push(folder.display().to_string());
        }
    }
    session.service.cancel().await.expect("close the session");
    // As its x86-64 twin does: the job must end done.
    assert!(failed.is_empty(), "not done: {failed:?}");
}
