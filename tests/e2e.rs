//! The whole workflow through the MCP server, in both protocol generations, on the `add` example:
//! spec → approval → tests → code → run, and every way lengua refuses or recovers.

mod common;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
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

async fn state(session: &Session, program: &Program) -> String {
    let status = session.call_ok("status", program.arguments()).await;
    text(&status["program"]["state"]).to_owned()
}

fn run_arguments(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

#[tokio::test]
async fn the_server_offers_exactly_five_tools() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let tools = session.service.list_all_tools().await.expect("list tools");
        let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["approve_spec", "check_code", "check_tests", "run", "status"],
            "{protocol:?}"
        );
        let mut titles: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool.title.as_deref())
            .collect();
        titles.sort_unstable();
        assert_eq!(
            titles,
            [
                "Approve spec",
                "Check code",
                "Check tests",
                "Run program",
                "Status"
            ],
            "{protocol:?}"
        );
        for tool in &tools {
            let hints = tool.annotations.as_ref().expect("annotations");
            let description = tool.description.as_deref().unwrap_or_default();
            assert!(
                description.contains("path"),
                "{} explains its path",
                tool.name
            );
            match tool.name.as_ref() {
                "status" => assert_eq!(hints.read_only_hint, Some(true)),
                "run" => assert_eq!(hints.read_only_hint, Some(false)),
                _ => {
                    assert_eq!(hints.read_only_hint, Some(false), "{}", tool.name);
                    assert_eq!(hints.idempotent_hint, Some(true), "{}", tool.name);
                    assert_eq!(hints.destructive_hint, Some(false), "{}", tool.name);
                }
            }
        }
        session.service.cancel().await.expect("close the session");
    }
}

#[test]
fn the_example_carries_its_spec_label() {
    let label = Sha256Hex::of(example("spec.md").as_bytes()).label();
    assert!(example("tests.toml").contains(&format!("spec = \"{label}\"")));
    assert_eq!(
        example("main.s").lines().next(),
        Some(format!("# lengua: spec {label}").as_str())
    );
}

#[tokio::test]
async fn add_goes_from_spec_to_run_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        for name in ["spec.md", "tests.toml", "main.s"] {
            program.write(name, &example(name));
        }
        assert_eq!(
            state(&session, &program).await,
            "spec_not_approved",
            "{protocol:?}"
        );

        let approval = session.approve(&program).await;
        assert_eq!(approval["approved"], true, "{protocol:?}: {approval}");
        assert_eq!(state(&session, &program).await, "tests_unchecked");
        let tests = session.call_ok("check_tests", program.arguments()).await;
        assert_eq!(tests["checked"], true, "{tests}");
        assert_eq!(state(&session, &program).await, "code_unchecked");
        let code = session.call_ok("check_code", program.arguments()).await;
        assert_eq!(code["stamped"], true, "{code}");
        assert_eq!(code["test_runs"], 3 + 200 + 50, "{code}");
        assert_eq!(state(&session, &program).await, "ready");

        let sum = session
            .call_ok("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert_eq!(sum["stdout"], "5\n", "{protocol:?}: {sum}");
        assert_eq!(sum["exit_code"], 0, "{sum}");
        let usage = session.call_ok("run", run_arguments(&program, &[])).await;
        assert_eq!(usage["stderr"], "usage: add <a> <b>\n", "{usage}");
        assert_eq!(usage["exit_code"], 1, "{usage}");
        let invalid = session
            .call_ok("run", run_arguments(&program, &["2", "x"]))
            .await;
        assert_eq!(invalid["stderr"], "error: not a number\n", "{invalid}");
        assert_eq!(invalid["exit_code"], 2, "{invalid}");

        // Edited code doesn't run until it's checked again.
        program.write("main.s", &format!("{}\n", example("main.s")));
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(error.contains("call check_code"), "{error}");
        program.write("main.s", &example("main.s"));
        assert_eq!(state(&session, &program).await, "ready");

        // Checked tests are frozen.
        program.write(
            "tests.toml",
            &example("tests.toml").replace("runs = 200", "runs = 20"),
        );
        let error = session.call_error("check_tests", program.arguments()).await;
        assert!(error.contains("tests stay frozen"), "{error}");
        program.write("tests.toml", &example("tests.toml"));
        assert_eq!(state(&session, &program).await, "ready");

        // A lock edited by hand can't be trusted; approving again starts over.
        let lock = program.read("lengua.lock").expect("the lock");
        program.write(
            "lengua.lock",
            &lock.replace("unsafe_blocks = 0", "unsafe_blocks = 1"),
        );
        assert_eq!(state(&session, &program).await, "lock_invalid");
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(error.contains("can't be used"), "{error}");
        session.approve(&program).await;
        session.call_ok("check_tests", program.arguments()).await;
        session.call_ok("check_code", program.arguments()).await;
        assert_eq!(state(&session, &program).await, "ready");

        // An edited spec stops everything until the user approves it in their own words.
        let edited = example("spec.md").replace("Adds two numbers", "Adds two integers");
        program.write("spec.md", &edited);
        assert_eq!(state(&session, &program).await, "spec_not_approved");
        let error = session
            .call_error("run", run_arguments(&program, &["2", "3"]))
            .await;
        assert!(
            error.contains("wait until they say they approve it"),
            "{error}"
        );
        let error = session
            .call_error("approve_spec", program.approval("  "))
            .await;
        assert!(
            error.contains("Only the user can approve a spec"),
            "{error}"
        );

        // Approving it records their words and resets the tests and code checks.
        let approval = session.approve(&program).await;
        assert_eq!(approval["approval"], program.approval_words(), "{approval}");
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("approval = \"{}\"", program.approval_words())),
            "{lock}"
        );
        assert!(
            !lock.contains("[tests]") && !lock.contains("[code]"),
            "{lock}"
        );
        assert_eq!(state(&session, &program).await, "tests_unchecked");
        let error = session.call_error("check_tests", program.arguments()).await;
        assert!(
            error.contains("[meta] says spec"),
            "the tests name the old spec: {error}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn code_that_breaks_a_rule_runs_no_tests() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        program.write("spec.md", &example("spec.md"));
        program.write("tests.toml", &example("tests.toml"));
        program.write(
            "main.s",
            &example("main.s").replacen(
                "    mov rax, qword ptr [rsp]\n",
                "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
                1,
            ),
        );
        session.approve(&program).await;
        session.call_ok("check_tests", program.arguments()).await;
        let error = session.call_error("check_code", program.arguments()).await;
        assert!(error.contains("so nothing ran"), "{protocol:?}: {error}");
        assert!(error.contains("rule 3 (fixed resources)"), "{error}");
        assert_eq!(state(&session, &program).await, "code_unchecked");
        session.service.cancel().await.expect("close the session");
    }
}
