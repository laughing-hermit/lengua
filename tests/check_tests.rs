//! Checking tests.toml: the checks, fixing tests at any time, and the stored copy that later
//! changes are shown against.

mod common;

use common::{ADD_SPEC, Program, Protocol, Session, add_tests, label};
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

async fn recorded_program(session: &Session) -> Program {
    let program = Program::with_add_spec();
    let report = session.record(&program).await;
    assert_eq!(report["recorded"], true, "{report}");
    program
}

async fn state(session: &Session, program: &Program) -> Value {
    let status = session.call_ok("status", program.arguments()).await;
    status["program"]["state"].clone()
}

#[tokio::test]
async fn checks_and_records_valid_tests() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = recorded_program(&session).await;
        let tests = add_tests(ADD_SPEC);
        program.write("tests.toml", &tests);

        let report = session.call_ok("check_tests", program.arguments()).await;
        assert_eq!(report["checked"], true, "{protocol:?}: {report}");
        assert_eq!(report["examples"], 2, "{report}");
        assert_eq!(report["generated_blocks"], 1, "{report}");
        assert_eq!(report["generated_runs"], 200, "{report}");
        assert!(
            text(&report["message"]).contains("covers every behavior"),
            "{report}"
        );
        assert!(report.get("tests_changed").is_none(), "{report}");
        assert!(
            text(&report["next_step"]).contains(&format!("# lengua: spec {}", label(ADD_SPEC))),
            "{report}"
        );

        let status = session.call_ok("status", program.arguments()).await;
        assert_eq!(status["program"]["state"], "code_missing", "{status}");
        assert_eq!(
            status["program"]["files"]["tests"]["stamped"], true,
            "{status}"
        );
        let stored = session
            .data_dir
            .path()
            .join("stamped")
            .join(text(&report["tests_sha256"]));
        assert_eq!(
            std::fs::read_to_string(&stored).expect("the checked tests are kept"),
            tests
        );

        let again = session.call_ok("check_tests", program.arguments()).await;
        assert!(
            text(&again["message"]).contains("already checked"),
            "{again}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn tests_can_be_fixed_before_code_runs() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    let tests = add_tests(ADD_SPEC);
    program.write("tests.toml", &tests);
    session.call_ok("check_tests", program.arguments()).await;

    // No code has run against them yet, so a fix is simply checked again.
    let fixed = tests.replace("runs = 200", "runs = 300");
    program.write("tests.toml", &fixed);
    assert_eq!(state(&session, &program).await, "tests_unchecked");
    let report = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(report["generated_runs"], 300, "{report}");
    // Before code runs, nothing is counted as a change after it.
    assert!(report.get("tests_changed").is_none(), "{report}");
    assert_eq!(state(&session, &program).await, "code_missing");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn problems_are_reported_and_nothing_is_recorded() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write(
        "tests.toml",
        &add_tests(ADD_SPEC).replace("{a + b}", "{a - b}"),
    );
    let error = session.call_error("check_tests", program.arguments()).await;
    assert!(
        error.contains("has 1 problem, so it wasn't checked"),
        "{error}"
    );
    assert!(error.contains("formula disagrees with the spec"), "{error}");
    assert!(
        error.contains("Fix them with the write-tests skill"),
        "{error}"
    );
    let lock = program.read("lengua.lock").expect("the lock exists");
    assert!(!lock.contains("[tests]"), "{lock}");
    assert_eq!(state(&session, &program).await, "tests_unchecked");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn tests_need_a_recorded_spec_and_a_file() {
    let session = Session::start(Protocol::Discover).await;
    let unrecorded = Program::with_add_spec();
    unrecorded.write("tests.toml", &add_tests(ADD_SPEC));
    let error = session
        .call_error("check_tests", unrecorded.arguments())
        .await;
    assert!(error.contains("recorded spec"), "{error}");
    assert!(error.contains("call record_spec"), "{error}");

    let without_tests = recorded_program(&session).await;
    let error = session
        .call_error("check_tests", without_tests.arguments())
        .await;
    assert!(error.contains("There's no tests.toml"), "{error}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_changed_spec_needs_its_tests_checked_again() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write("tests.toml", &add_tests(ADD_SPEC));
    session.call_ok("check_tests", program.arguments()).await;

    let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
    program.write("spec.md", &edited);
    let record = session.record(&program).await;
    assert_eq!(record["recorded"], true, "{record}");
    // Recorded again under the same request, the change is shown.
    assert!(
        text(&record["spec_changed"]).contains("Adds two integers"),
        "{record}"
    );
    assert_eq!(state(&session, &program).await, "tests_unchecked");

    let error = session.call_error("check_tests", program.arguments()).await;
    assert!(error.contains("[meta] says spec"), "{error}");
    program.write("tests.toml", &add_tests(&edited));
    let report = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(report["checked"], true, "{report}");
    session.service.cancel().await.expect("close the session");
}
