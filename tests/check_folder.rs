//! One program folder's check, for writing a program by hand on a machine that isn't the
//! developer's: `LENGUA_CHECK_FOLDER` names the folder, which holds idea.md and main.wat, and the
//! test prints what `check` reports (rules, tests, proofs) and fails unless the job ends done.
//! GitHub's runners run it (the workflow's "ignored" mode with this test's name as the filter).

mod common;

use common::{Program, Protocol, Session};
use serde_json::Value;

/// How long each folder's proofs may take, in seconds.
const PROOF_SECONDS: u64 = 60;

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
    let session = Session::proving_for(Protocol::Discover, PROOF_SECONDS).await;
    let mut failed = Vec::new();
    // Several folders, separated by commas, share one session.
    for folder in folder.split(',') {
        let folder = std::path::PathBuf::from(folder);
        let program = Program::new();
        // The parts the folder builds on stand beside it, each checked first.
        if let Err(problem) = session.settle_parts(&program, &folder).await {
            println!("PORT folder: {}", folder.display());
            println!("PORT problems: {problem}");
            failed.push(folder.display().to_string());
            continue;
        }
        for file in ["idea.md", "main.wat"] {
            let text = std::fs::read_to_string(folder.join(file)).expect("the file");
            program.write(file, &text);
        }
        session.record(&program).await;
        let report = session.settle(&program).await;
        println!("PORT folder: {}", folder.display());
        println!("PORT state: {}", report["state"]);
        for field in ["problems", "tests", "proofs", "parts", "contract", "memory"] {
            let lines = match &report[field] {
                Value::String(line) => vec![line.as_str()],
                lines => lines.as_array().into_iter().flatten().map(text).collect(),
            };
            for line in lines {
                println!("PORT {field}: {line}");
            }
        }
        if report["state"] != "done" {
            failed.push(folder.display().to_string());
        }
    }
    session.service.cancel().await.expect("close the session");
    assert!(failed.is_empty(), "not done: {failed:?}");
}
