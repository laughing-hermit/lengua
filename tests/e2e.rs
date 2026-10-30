//! The whole workflow through the MCP server, in both protocol generations, on the `add` example:
//! from the request to done through check alone, then run, and every way lengua refuses or
//! recovers.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, REQUEST, Session};
use lengua::hash::Sha256Hex;
use serde_json::{Value, json};

fn example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/add")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn run_arguments(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// lengua's line in main.s once a check has passed: what it was compiled from and for.
fn header_of(program: &Program) -> String {
    let code = program.read("main.s").expect("main.s");
    code.lines().nth(1).unwrap_or_default().to_owned()
}

fn compiled_from(spec: &str) -> String {
    format!(
        "# lengua: compiled from spec.md sha256:{} for linux-x86_64",
        Sha256Hex::of(spec.as_bytes())
    )
}

#[test]
fn the_example_is_plain_assembly() {
    // lengua writes its line where main.s is checked; the example in the repository has none.
    let code = example("main.s");
    assert_eq!(code.lines().next(), Some(".intel_syntax noprefix"));
    assert!(!code.contains("# lengua:"), "{code}");
}

#[tokio::test]
async fn add_goes_from_request_to_done_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_spec", "{protocol:?}: {report}");

        // One call after each file: the spec, tests included, with the user's words, then the
        // code.
        program.write("spec.md", &example("spec.md"));
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{report}");
        program.write("main.s", &example("main.s"));
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{protocol:?}: {done}");
        // Code that passed carries lengua's line, which leaves what was checked as it was.
        assert_eq!(header_of(&program), compiled_from(&example("spec.md")));
        assert_eq!(
            program.read("main.s").expect("main.s").lines().count(),
            example("main.s").lines().count() + 1
        );
        assert_eq!(
            done["tests"][0], "main.s follows every rule and passed all 253 test runs",
            "{done}"
        );
        assert!(
            done["memory"]
                .as_str()
                .is_some_and(|memory| memory.starts_with("the rules (M1 to M8) hold")),
            "{done}"
        );
        assert_eq!(
            done["example"],
            "example B1: given arguments [\"2\", \"3\"], the program printed \"5\\n\", exited \
             with code 0",
            "{done}"
        );
        assert!(done["proofs"].is_array(), "{done}");
        assert!(
            done["performance"][0]
                .as_str()
                .is_some_and(|line| line.starts_with("example B1: 1 run")),
            "{done}"
        );
        for gone in ["machine", "changes", "problems"] {
            assert!(done.get(gone).is_none(), "{gone}: {done}");
        }
        assert!(text(&done["next_step"]).starts_with("Done:"), "{done}");

        let sum = session
            .call_ok("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert_eq!(sum["stdout"], "5\n", "{protocol:?}: {sum}");
        assert_eq!(sum["exit_code"], 0, "{sum}");
        assert!(sum.get("fingerprint").is_none(), "{sum}");
        let usage = session.call_ok("run", run_arguments(&program, &[])).await;
        assert_eq!(usage["stderr"], "usage: add <a> <b>\n", "{usage}");
        assert_eq!(usage["exit_code"], 1, "{usage}");
        let invalid = session
            .call_ok("run", run_arguments(&program, &["2", "x"]))
            .await;
        assert_eq!(invalid["stderr"], "error: not a number\n", "{invalid}");
        assert_eq!(invalid["exit_code"], 2, "{invalid}");
        let long = session
            .call_error(
                "run",
                json!({ "path": program.arguments()["path"], "timeout_seconds": 600 }),
            )
            .await;
        assert!(long.contains("goes from 1 to 40"), "{long}");

        // Edited code doesn't run until it's checked again.
        program.write("main.s", &format!("{}\n", example("main.s")));
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(error.contains("call check"), "{error}");
        program.write("main.s", &example("main.s"));
        assert_eq!(session.settle(&program).await["state"], "done");

        // Without its line, the checked code is still the checked code, and gets it back.
        assert_eq!(header_of(&program), compiled_from(&example("spec.md")));

        // Tests can be fixed after code ran: the change is shown, lengua marks what tests less,
        // and main.s's line names the spec it now passed.
        let fewer = example("spec.md").replace("| 200 |", "| 20 |");
        program.write("spec.md", &fewer);
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{done}");
        assert!(
            text(&done["changes"]["spec_changed"]).contains("| 20 |"),
            "{done}"
        );
        assert_eq!(header_of(&program), compiled_from(&fewer));
        assert!(
            done["changes"]["tests_less"][0]
                .as_str()
                .is_some_and(|mark| mark.contains("fewer runs, 20 instead of 200")),
            "{done}"
        );
        assert!(text(&done["next_step"]).contains("changes lists"), "{done}");
        program.write("spec.md", &example("spec.md"));
        let done = session.settle(&program).await;
        assert!(done.get("changes").is_none(), "{done}");

        // A lock edited by hand can't be trusted; the user's words record the spec again.
        let lock = program.read("lengua.lock").expect("the lock");
        program.write(
            "lengua.lock",
            &lock.replace("unsafe_blocks = 0", "unsafe_blocks = 1"),
        );
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(error.contains("can't be used"), "{error}");
        let report = session.check(&program).await;
        assert_eq!(report["state"], "needs_request", "{report}");
        session.record(&program).await;
        assert_eq!(session.settle(&program).await["state"], "done");

        // An edited spec is recorded under the same words by the next check, and shown.
        let edited = example("spec.md").replace("Adds two numbers", "Adds two integers");
        program.write("spec.md", &edited);
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(error.contains("call check"), "{error}");
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{done}");
        let changed = text(&done["changes"]["spec_changed"]);
        assert!(
            changed.contains("Adds two numbers") && changed.contains("Adds two integers"),
            "{done}"
        );
        assert_eq!(done["changes"]["requests"], json!([REQUEST]), "{done}");

        // A new request after a finished build starts a new one with the user's words.
        let done = session
            .call_ok("check", program.request("Call them integers"))
            .await;
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(lock.contains("words = [\"Call them integers\"]"), "{lock}");
        assert!(done["state"].is_string(), "{done}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn code_that_breaks_a_rule_runs_no_tests() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        program.write("spec.md", &example("spec.md"));
        program.write(
            "main.s",
            &example("main.s").replacen(
                "    mov rax, qword ptr [rsp]\n",
                "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
                1,
            ),
        );
        let report = session.record(&program).await;
        assert_eq!(report["state"], "code_problems", "{protocol:?}: {report}");
        assert!(
            report["problems"]
                .as_array()
                .is_some_and(|problems| problems
                    .iter()
                    .any(|problem| text(problem).contains("rule 3 (fixed resources)"))),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).contains("compile skill"),
            "{report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(!lock.contains("[code]"), "{lock}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn every_call_returns_within_the_window_and_the_job_goes_on() {
    let session = Session::start_waiting(Protocol::Discover, 1).await;
    let program = Program::new();
    for name in ["spec.md", "main.s"] {
        program.write(name, &example(name));
    }
    let began = std::time::Instant::now();
    let report = session.record(&program).await;
    assert!(
        began.elapsed() < std::time::Duration::from_secs(5),
        "{:?}: {report}",
        began.elapsed()
    );
    // The same files: later calls wait on the same job, and nothing needs to change.
    let mut replies = vec![report];
    while !matches!(
        replies.last().and_then(|reply| reply["state"].as_str()),
        Some("done")
    ) {
        let began = std::time::Instant::now();
        replies.push(session.check(&program).await);
        assert!(began.elapsed() < std::time::Duration::from_secs(5));
        assert!(replies.len() < 400, "{replies:?}");
    }
    assert!(
        replies.iter().any(|reply| reply["state"] == "testing"),
        "{replies:?}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_shared_program_is_checked_again_where_it_lands() {
    // One user's lengua builds add; another user, with their own key, gets its two files.
    let first = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("spec.md", &example("spec.md"));
    program.write("main.s", &example("main.s"));
    first.record(&program).await;
    assert_eq!(first.settle(&program).await["state"], "done");
    let shared = Program::new();
    for name in ["spec.md", "main.s"] {
        shared.write(name, &program.read(name).expect("a file"));
    }
    assert!(
        shared.read("lengua.lock").is_none(),
        "the lock stays behind"
    );
    let second = Session::start(Protocol::Discover).await;
    let refused = second
        .call_error("run", run_arguments(&shared, &["2", "3"]))
        .await;
    assert!(refused.contains("call check"), "{refused}");
    // Checked from scratch there, with that user's words, it runs.
    second.record(&shared).await;
    assert_eq!(second.settle(&shared).await["state"], "done");
    let sum = second
        .call_ok("run", run_arguments(&shared, &["2", "3"]))
        .await;
    assert_eq!(sum["stdout"], "5\n", "{sum}");
    first.service.cancel().await.expect("close the session");
    second.service.cancel().await.expect("close the session");
}
