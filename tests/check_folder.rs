//! One program folder's check, for writing a program by hand on a machine that isn't the
//! developer's: `LENGUA_CHECK_FOLDER` names the folder, which holds spec.md and main.s, and the
//! test prints what `check` reports (rules, tests, proofs) and fails unless the job ends done.
//! GitHub's runners run it (the workflow's "ignored" mode with this test's name as the filter).
//!
//! It checks the folder as this computer's own target, natively. `LENGUA_CHECK_TARGET` names
//! another target (`linux-aarch64` on an x86-64 computer, say) to check it through QEMU's user
//! mode, where that is set up; the test then ends with a note in the run's summary.

mod common;
mod flow;
mod per_target;

use common::Program;
use lengua::platform::Profile;
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
    let target = match std::env::var("LENGUA_CHECK_TARGET")
        .ok()
        .filter(|name| !name.is_empty())
    {
        Some(name) => Profile::from_name(&name)
            .unwrap_or_else(|| panic!("LENGUA_CHECK_TARGET names no target lengua has: {name}")),
        None => {
            let Some(target) = per_target::this_target() else {
                per_target::runs_programs();
                return;
            };
            target
        }
    };
    let Some((session, _)) = flow::proving(target, Some(60)).await else {
        return;
    };
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
