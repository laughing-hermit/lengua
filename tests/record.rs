//! Recording spec.md: the agent writes it from the user's request and records it with the user's
//! own words, without waiting for an approval; lengua.lock keeps the words, and every change after
//! them is shown.

mod common;

use common::{ADD_SPEC, Program, Protocol, REQUEST, Session};
use lengua::hash::Sha256Hex;
use serde_json::{Value, json};

fn label(text: &str) -> String {
    Sha256Hex::of(text.as_bytes()).label()
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

async fn status(session: &Session, program: &Program) -> Value {
    session.call_ok("status", program.arguments()).await
}

#[tokio::test]
async fn lists_record_spec_with_its_hints() {
    let session = Session::start(Protocol::Discover).await;
    let tools = session.service.list_all_tools().await.expect("list tools");
    let tool = tools
        .iter()
        .find(|tool| tool.name == "record_spec")
        .expect("record_spec is listed");
    let hints = tool.annotations.as_ref().expect("annotations");
    assert_eq!(hints.read_only_hint, Some(false));
    assert_eq!(hints.destructive_hint, Some(false));
    assert_eq!(hints.idempotent_hint, Some(true));
    let description = tool.description.as_deref().unwrap_or_default();
    assert!(
        description.contains("don't wait for an approval"),
        "{description}"
    );
    assert!(description.contains("never made up"), "{description}");
    let inputs = serde_json::to_value(&tool.input_schema).expect("schema");
    assert!(
        inputs["required"].as_array().is_some_and(|required| {
            required.contains(&json!("path")) && required.contains(&json!("request"))
        }),
        "{inputs}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn records_the_users_words_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::with_add_spec();
        let report = session.record(&program).await;
        assert_eq!(report["recorded"], true, "{protocol:?}: {report}");
        assert_eq!(report["requests"], json!([REQUEST]), "{report}");
        assert_eq!(report["spec_label"], label(ADD_SPEC), "{report}");
        assert!(report.get("spec_changed").is_none(), "{report}");
        assert!(
            text(&report["message"]).contains("lengua recorded spec.md for \"add\""),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).contains("write-tests skill"),
            "{report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("words = [\"{REQUEST}\"]\n")),
            "{lock}"
        );
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        assert_eq!(report["program"]["requests"], json!([REQUEST]), "{report}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_path_may_name_the_spec_itself() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    let spec_path = program.file("spec.md");
    let report = session
        .call_ok(
            "record_spec",
            json!({
                "path": spec_path.to_str().expect("UTF-8 path"),
                "request": REQUEST,
            }),
        )
        .await;
    assert_eq!(report["recorded"], true, "{report}");
    assert!(program.read("lengua.lock").is_some());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_users_words_are_required() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    let empty = session
        .call_error("record_spec", program.request(" \n "))
        .await;
    assert!(empty.contains("Never make them up"), "{empty}");
    let missing = session.call_error("record_spec", program.arguments()).await;
    assert!(missing.contains("request"), "{missing}");
    let long = session
        .call_error("record_spec", program.request(&"make it ".repeat(300)))
        .await;
    assert!(long.contains("more than 2000 characters"), "{long}");
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn recording_the_same_spec_again_changes_nothing() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    session.record(&program).await;
    let lock = program.read("lengua.lock").expect("the lock");
    let again = session.record(&program).await;
    assert!(
        text(&again["message"]).contains("already recorded"),
        "{again}"
    );
    assert_eq!(program.read("lengua.lock").as_deref(), Some(lock.as_str()));
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_invalid_spec_is_reported_before_it_is_recorded() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let invalid = ADD_SPEC.replace("exits with code `1`", "exits with code `134`");
    program.write("spec.md", &invalid);
    // status catches the problem first.
    let report = status(&session, &program).await;
    assert_eq!(report["program"]["state"], "spec_unrecorded", "{report}");
    let step = text(&report["next_step"]);
    assert!(
        step.starts_with("Next: fix") && step.contains("134 is reserved"),
        "{report}"
    );
    // record_spec refuses it too.
    let error = session
        .call_error("record_spec", program.request(REQUEST))
        .await;
    assert!(
        error.contains("has 1 problem to fix before lengua can record it"),
        "{error}"
    );
    assert!(error.contains("call record_spec again"), "{error}");
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn paths_must_be_absolute_and_hold_a_spec() {
    let session = Session::start(Protocol::Discover).await;
    let relative = session
        .call_error("record_spec", json!({ "path": "add", "request": REQUEST }))
        .await;
    assert!(relative.contains("absolute path"), "{relative}");
    let status = session.call_error("status", json!({ "path": "add" })).await;
    assert!(status.contains("absolute path"), "{status}");
    let empty = Program::new();
    let missing = session
        .call_error("record_spec", empty.request(REQUEST))
        .await;
    assert!(missing.contains("There's no spec.md"), "{missing}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn status_follows_a_program_through_recording() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "no_spec", "{report}");
        assert!(
            text(&report["next_step"]).contains("write-spec skill"),
            "{report}"
        );

        program.write("spec.md", ADD_SPEC);
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "spec_unrecorded", "{report}");
        assert_eq!(report["program"]["lock"]["state"], "missing", "{report}");
        assert_eq!(report["program"]["spec_label"], label(ADD_SPEC), "{report}");
        assert!(
            text(&report["next_step"]).contains("call record_spec"),
            "{report}"
        );

        session.record(&program).await;
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        assert!(
            text(&report["next_step"]).contains(&format!("spec = \"{}\"", label(ADD_SPEC))),
            "{report}"
        );

        // An edited spec is recorded again, and the change since the request is shown.
        let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
        program.write("spec.md", &edited);
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "spec_unrecorded", "{report}");
        assert_eq!(
            report["program"]["files"]["spec"]["stamped"], false,
            "{report}"
        );
        let record = session.record(&program).await;
        assert_eq!(record["spec_label"], label(&edited), "{record}");
        let changed = text(&record["spec_changed"]);
        assert!(
            changed.contains("- 10: ## B1: Adds two numbers")
                && changed.contains("+ 10: ## B1: Adds two integers"),
            "{record}"
        );
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["spec_changed"], record["spec_changed"]);

        // A lock edited by hand can't be used, and recording starts over.
        let lock = program.read("lengua.lock").expect("the lock exists");
        program.write("lengua.lock", &lock.replace("Make me", "Make us"));
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "lock_invalid", "{report}");
        assert_eq!(report["program"]["lock"]["state"], "invalid", "{report}");
        assert!(
            text(&report["program"]["lock"]["problem"]).contains("signature doesn't match"),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).contains("Call record_spec"),
            "{report}"
        );
        let record = session.record(&program).await;
        assert_eq!(record["recorded"], true, "{record}");
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn a_build_keeps_its_requests_and_its_first_spec() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    session.record(&program).await;
    // Until code passes, new words join the build rather than start a new one, so changes are
    // still shown from the spec the build started with.
    for number in 1..=7 {
        let spec = ADD_SPEC.replace("Adds two numbers", &format!("Adds two numbers ({number})"));
        program.write("spec.md", &spec);
        let words = format!("Change {number}");
        let record = session
            .call_ok("record_spec", program.request(&words))
            .await;
        assert!(
            text(&record["spec_changed"]).contains("- 10: ## B1: Adds two numbers\n"),
            "{record}"
        );
        let requests = record["requests"].as_array().expect("requests");
        assert_eq!(requests.first(), Some(&json!(REQUEST)), "{record}");
        assert_eq!(requests.last(), Some(&json!(words)), "{record}");
        assert!(requests.len() <= 6, "{record}");
    }
    session.service.cancel().await.expect("close the session");
}
