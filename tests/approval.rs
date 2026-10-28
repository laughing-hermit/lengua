//! Approving spec.md: the user approves it in their own words, the agent passes those words to
//! `approve_spec`, and lengua.lock records them.

mod common;

use common::{ADD_SPEC, Program, Protocol, Session};
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
async fn lists_approve_spec_with_its_hints() {
    let session = Session::start(Protocol::Discover).await;
    let tools = session.service.list_all_tools().await.expect("list tools");
    let tool = tools
        .iter()
        .find(|tool| tool.name == "approve_spec")
        .expect("approve_spec is listed");
    let hints = tool.annotations.as_ref().expect("annotations");
    assert_eq!(hints.read_only_hint, Some(false));
    assert_eq!(hints.destructive_hint, Some(false));
    assert_eq!(hints.idempotent_hint, Some(true));
    let description = tool.description.as_deref().unwrap_or_default();
    assert!(description.contains("in their own words"), "{description}");
    let inputs = serde_json::to_value(&tool.input_schema).expect("schema");
    assert!(
        inputs["required"].as_array().is_some_and(|required| {
            required.contains(&json!("path")) && required.contains(&json!("approval"))
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
        let report = session.approve(&program).await;
        assert_eq!(report["approved"], true, "{protocol:?}: {report}");
        assert_eq!(report["approval"], program.approval_words(), "{report}");
        assert_eq!(report["spec_label"], label(ADD_SPEC), "{report}");
        assert!(
            text(&report["message"]).contains("The user approved spec.md for \"add\""),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).contains("write-tests skill"),
            "{report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("approval = \"{}\"\n", program.approval_words())),
            "{lock}"
        );
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        assert_eq!(
            report["program"]["lock"]["approval"],
            program.approval_words(),
            "{report}"
        );
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
            "approve_spec",
            json!({
                "path": spec_path.to_str().expect("UTF-8 path"),
                "approval": "I approve it",
            }),
        )
        .await;
    assert_eq!(report["approved"], true, "{report}");
    assert!(program.read("lengua.lock").is_some());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn only_the_users_words_approve() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    let empty = session
        .call_error("approve_spec", program.approval(" \n "))
        .await;
    assert!(
        empty.contains("Only the user can approve a spec"),
        "{empty}"
    );
    let missing = session
        .call_error("approve_spec", program.arguments())
        .await;
    assert!(missing.contains("approval"), "{missing}");
    let long = session
        .call_error("approve_spec", program.approval(&"approve ".repeat(100)))
        .await;
    assert!(long.contains("more than 500 characters"), "{long}");
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn approving_the_same_spec_again_changes_nothing() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    session.approve(&program).await;
    let lock = program.read("lengua.lock").expect("the lock");
    let again = session
        .call_ok("approve_spec", program.approval("yes, approved"))
        .await;
    assert!(
        text(&again["message"]).contains("already approved"),
        "{again}"
    );
    assert_eq!(again["approval"], program.approval_words(), "{again}");
    assert_eq!(program.read("lengua.lock").as_deref(), Some(lock.as_str()));
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_invalid_spec_is_reported_before_the_user_is_asked() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let invalid = ADD_SPEC.replace("exits with code `1`", "exits with code `134`");
    program.write("spec.md", &invalid);
    // status catches the problem first, so the agent fixes it before asking the user.
    let report = status(&session, &program).await;
    assert_eq!(report["program"]["state"], "spec_not_approved", "{report}");
    let step = text(&report["next_step"]);
    assert!(
        step.starts_with("Next: fix") && step.contains("134 is reserved"),
        "{report}"
    );
    // approve_spec refuses it too.
    let error = session
        .call_error("approve_spec", program.approval("I approve"))
        .await;
    assert!(error.contains("has 1 problem to fix"), "{error}");
    assert!(error.contains("ask them to approve it again"), "{error}");
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn paths_must_be_absolute_and_hold_a_spec() {
    let session = Session::start(Protocol::Discover).await;
    let relative = session
        .call_error(
            "approve_spec",
            json!({ "path": "add", "approval": "I approve" }),
        )
        .await;
    assert!(relative.contains("absolute path"), "{relative}");
    let status = session.call_error("status", json!({ "path": "add" })).await;
    assert!(status.contains("absolute path"), "{status}");
    let empty = Program::new();
    let missing = session
        .call_error("approve_spec", empty.approval("I approve"))
        .await;
    assert!(missing.contains("There's no spec.md"), "{missing}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn status_follows_a_program_through_approval() {
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
        assert_eq!(report["program"]["state"], "spec_not_approved", "{report}");
        assert_eq!(report["program"]["lock"]["state"], "missing", "{report}");
        assert_eq!(report["program"]["spec_label"], label(ADD_SPEC), "{report}");
        assert!(
            text(&report["next_step"]).contains("wait until they say they approve it"),
            "{report}"
        );

        session.approve(&program).await;
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        assert!(
            text(&report["next_step"]).contains(&format!("spec = \"{}\"", label(ADD_SPEC))),
            "{report}"
        );

        // Any edit to the spec needs a new approval.
        let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
        program.write("spec.md", &edited);
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "spec_not_approved", "{report}");
        assert_eq!(
            report["program"]["files"]["spec"]["stamped"], false,
            "{report}"
        );
        let approval = session.approve(&program).await;
        assert_eq!(approval["spec_label"], label(&edited), "{approval}");

        // A lock edited by hand can't be used, and approving starts over.
        let lock = program.read("lengua.lock").expect("the lock exists");
        program.write(
            "lengua.lock",
            &lock.replace("approval = \"I approve", "approval = \"We approve"),
        );
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "lock_invalid", "{report}");
        assert_eq!(report["program"]["lock"]["state"], "invalid", "{report}");
        assert!(
            text(&report["program"]["lock"]["problem"]).contains("signature doesn't match"),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).contains("ask them to approve"),
            "{report}"
        );
        let approval = session.approve(&program).await;
        assert_eq!(approval["approved"], true, "{approval}");
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        session.service.cancel().await.expect("close the session");
    }
}
