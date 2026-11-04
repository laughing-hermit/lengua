//! One program folder's check, for writing a program by hand on a machine that isn't the
//! developer's: `LENGUA_CHECK_FOLDER` names the folder, which holds spec.md and main.s, and the
//! test prints what `check` reports (rules, tests, proofs) and fails unless the job ends done.
//! GitHub's runners run it (the targets workflow's "ignored" switch with this test's name as the
//! filter); `check_one_aarch64_folder` is its AArch64 twin.

mod common;

use common::{Program, Protocol, Session};
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[tokio::test]
#[ignore = "checks the folder LENGUA_CHECK_FOLDER names: by hand"]
async fn check_one_folder() {
    let Some(folder) = std::env::var("LENGUA_CHECK_FOLDER")
        .ok()
        .filter(|folder| !folder.is_empty())
    else {
        return;
    };
    let session = Session::proving_for(Protocol::Discover, 60).await;
    let mut failed = Vec::new();
    // Several folders, separated by commas, share one session.
    for folder in folder.split(',') {
        let folder = std::path::PathBuf::from(folder);
        let program = Program::new();
        for file in ["spec.md", "main.s"] {
            let text = std::fs::read_to_string(folder.join(file)).expect("the file");
            program.write(file, &text);
        }
        session.record(&program).await;
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
    assert!(failed.is_empty(), "not done: {failed:?}");
}
