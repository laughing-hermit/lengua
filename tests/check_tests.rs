//! Checking tests.toml: the checks, fixing tests at any time, and the stored copy that later
//! changes are shown against.

mod common;

use common::{ADD_SPEC, ADD_TESTS, Program, Protocol, Session};
use lengua::hash::Sha256Hex;
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

async fn recorded_program(session: &Session) -> Program {
    let program = Program::with_add_spec();
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_tests", "{report}");
    program
}

#[tokio::test]
async fn checks_and_records_valid_tests() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = recorded_program(&session).await;
        program.write("tests.toml", ADD_TESTS);

        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_code", "{protocol:?}: {report}");
        assert!(
            text(&report["next_step"]).contains("write-assembly skill"),
            "{report}"
        );
        assert!(
            !text(&report["next_step"]).contains("sha256"),
            "no label to copy: {report}"
        );
        assert!(
            text(&report["machine"]).contains("core"),
            "the machine shows while main.s is missing: {report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        let hash = Sha256Hex::of(ADD_TESTS.as_bytes()).to_string();
        assert!(
            lock.contains(&format!("[tests]\nsha256 = \"{hash}\"")),
            "{lock}"
        );
        let stored = session.data_dir.path().join("stamped").join(&hash);
        assert_eq!(
            std::fs::read_to_string(&stored).expect("the checked tests are kept"),
            ADD_TESTS
        );

        let again = session.check(&program).await;
        assert_eq!(again["state"], "write_code", "{again}");
        assert_eq!(program.read("lengua.lock").as_deref(), Some(lock.as_str()));
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn tests_can_be_fixed_before_code_runs() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write("tests.toml", ADD_TESTS);
    session.check(&program).await;

    // No code has run against them yet, so a fix is simply checked again.
    let fixed = ADD_TESTS.replace("runs = 200", "runs = 300");
    program.write("tests.toml", &fixed);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains(&Sha256Hex::of(fixed.as_bytes()).to_string()),
        "{lock}"
    );
    // Before code runs, nothing is counted as a change after it.
    assert!(!lock.contains("\ntests = "), "{lock}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn problems_are_reported_and_nothing_is_recorded() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write("tests.toml", &ADD_TESTS.replace("{a + b}", "{a - b}"));
    let report = session.check(&program).await;
    assert_eq!(report["state"], "test_problems", "{report}");
    assert!(
        report["problems"].as_array().is_some_and(|problems| {
            problems.len() == 1 && text(&problems[0]).contains("formula disagrees with the spec")
        }),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).contains("write-tests skill"),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock exists");
    assert!(!lock.contains("[tests]"), "{lock}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_leftover_spec_label_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write(
        "tests.toml",
        &format!("[meta]\nspec = \"sha256:1111\"\n\n{ADD_TESTS}"),
    );
    let report = session.check(&program).await;
    assert_eq!(report["state"], "test_problems", "{report}");
    assert!(
        text(&report["problems"][0]).contains("delete the [meta] table"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn tests_wait_for_a_recorded_spec() {
    let session = Session::start(Protocol::Discover).await;
    let unrecorded = Program::with_add_spec();
    unrecorded.write("tests.toml", ADD_TESTS);
    let report = session.check(&unrecorded).await;
    assert_eq!(report["state"], "needs_request", "{report}");
    let lock = unrecorded.read("lengua.lock");
    assert!(lock.is_none(), "{lock:?}");
    // With the user's words, the spec is recorded and the tests are checked in the same call.
    let report = session.record(&unrecorded).await;
    assert_eq!(report["state"], "write_code", "{report}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_changed_spec_checks_its_tests_again() {
    let session = Session::start(Protocol::Discover).await;
    let program = recorded_program(&session).await;
    program.write("tests.toml", ADD_TESTS);
    session.check(&program).await;

    // The tests still fit the edited spec, so the one call records the spec and checks them
    // again: nothing in tests.toml had to change.
    let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
    program.write("spec.md", &edited);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains(&format!(
            "[spec]\nsha256 = \"{}\"\n\n[tests]",
            Sha256Hex::of(edited.as_bytes())
        )),
        "{lock}"
    );

    // When they don't fit, they're the next thing to fix.
    let narrower = edited.replace("`-1000000000`", "`-1000`");
    program.write("spec.md", &narrower);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "test_problems", "{report}");
    session.service.cancel().await.expect("close the session");
}
