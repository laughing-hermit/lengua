//! Approving spec.md: the dialog in both protocol generations, the permission-prompt fallback for
//! clients without dialogs, and every way an approval can fail.

mod common;

use std::collections::BTreeMap;

use common::{ADD_SPEC, Answer, Program, Protocol, Session, TestClient};
use lengua::hash::Sha256Hex;
use rmcp::model::CallToolResponse;
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
    let session = Session::start(Protocol::Discover, TestClient::without_elicitation()).await;
    let tools = session.service.list_all_tools().await.expect("list tools");
    let tool = tools
        .iter()
        .find(|tool| tool.name == "approve_spec")
        .expect("approve_spec is listed");
    let hints = tool.annotations.as_ref().expect("annotations");
    assert_eq!(hints.read_only_hint, Some(false));
    assert_eq!(hints.destructive_hint, Some(false));
    assert_eq!(hints.idempotent_hint, Some(true));
    assert!(
        tool.output_schema.is_some(),
        "approve_spec has an output schema"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn approves_in_a_dialog_in_both_protocols() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol, TestClient::with_elicitation(Answer::Approve)).await;
        let program = Program::with_add_spec();
        let report = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(report["approved"], true, "{protocol:?}: {report}");
        assert_eq!(
            report["approved_via"], "elicitation",
            "{protocol:?}: {report}"
        );
        assert_eq!(report["spec_label"], label(ADD_SPEC));
        assert!(
            text(&report["next_step"]).contains("write-tests skill"),
            "{report}"
        );

        let hash = Sha256Hex::of(ADD_SPEC.as_bytes());
        let lock = program.read("lengua.lock").expect("the lock is written");
        assert!(lock.contains(&format!("sha256 = \"{hash}\"")), "{lock}");

        let prompts = session.client.prompts();
        assert_eq!(prompts.len(), 1, "{protocol:?}: {prompts:?}");
        for expected in [
            "Approve the spec for \"add\"?",
            "B1: Adds two numbers",
            "B2: Missing arguments",
            "Allowed system calls: read, write, exit",
            hash.short(),
        ] {
            assert!(
                prompts[0].contains(expected),
                "{protocol:?}: {expected:?} is missing from the dialog:\n{}",
                prompts[0]
            );
        }

        let status = status(&session, &program).await;
        let program_report = &status["program"];
        assert_eq!(program_report["state"], "tests_missing", "{status}");
        assert_eq!(program_report["files"]["spec"]["stamped"], true, "{status}");
        assert_eq!(program_report["lock"]["state"], "valid", "{status}");
        assert_eq!(
            program_report["lock"]["approved_via"], "elicitation",
            "{status}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn an_approved_spec_needs_no_second_dialog() {
    let session = Session::start(
        Protocol::Discover,
        TestClient::with_elicitation(Answer::Approve),
    )
    .await;
    let program = Program::with_add_spec();
    session.call_ok("approve_spec", program.arguments()).await;
    let again = session.call_ok("approve_spec", program.arguments()).await;
    assert_eq!(again["approved"], true, "{again}");
    assert!(
        text(&again["message"]).contains("already approved"),
        "{again}"
    );
    assert_eq!(session.client.prompts().len(), 1);
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn declining_cancelling_or_unchecking_approves_nothing() {
    for protocol in Protocol::ALL {
        for (answer, words) in [
            (Answer::Decline, "declined"),
            (Answer::Cancel, "closed the dialog"),
            (Answer::Reject, "unchecked"),
        ] {
            let session = Session::start(protocol, TestClient::with_elicitation(answer)).await;
            let program = Program::with_add_spec();
            let report = session.call_ok("approve_spec", program.arguments()).await;
            assert_eq!(
                report["approved"], false,
                "{protocol:?} {answer:?}: {report}"
            );
            assert!(text(&report["message"]).contains(words), "{report}");
            assert!(
                text(&report["next_step"]).contains("ask the user what to change"),
                "{report}"
            );
            assert!(
                program.read("lengua.lock").is_none(),
                "{protocol:?} {answer:?}"
            );
            session.service.cancel().await.expect("close the session");
        }
    }
}

#[tokio::test]
async fn clients_without_dialogs_approve_through_the_permission_prompt() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol, TestClient::without_elicitation()).await;
        let program = Program::with_add_spec();
        let report = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(report["approved"], true, "{protocol:?}: {report}");
        assert_eq!(report["approved_via"], "tool-permission", "{report}");
        assert!(session.client.prompts().is_empty());
        let lock = program.read("lengua.lock").expect("the lock is written");
        assert!(
            lock.contains("approved_via = \"tool-permission\""),
            "{lock}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn a_spec_edited_while_the_dialog_is_open_is_not_approved() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol, TestClient::with_elicitation(Answer::Approve)).await;
        let program = Program::with_add_spec();
        let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
        session
            .client
            .edit_during_next_dialog(program.file("spec.md"), edited.clone());
        let report = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(report["approved"], false, "{protocol:?}: {report}");
        assert!(
            text(&report["message"]).contains("changed while the approval dialog was open"),
            "{report}"
        );
        assert_eq!(report["spec_label"], label(&edited), "{report}");
        assert!(program.read("lengua.lock").is_none(), "{protocol:?}");

        let report = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(report["approved"], true, "{protocol:?}: {report}");
        let prompts = session.client.prompts();
        assert!(
            prompts
                .last()
                .is_some_and(|prompt| prompt.contains("B1: Adds two integers")),
            "{prompts:?}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn nonces_are_single_use_and_bound_to_their_folder() {
    let session = Session::start(
        Protocol::Discover,
        TestClient::with_elicitation(Answer::Approve),
    )
    .await;
    let program = Program::with_add_spec();
    let answer = |action: Value| BTreeMap::from([("approve_spec".to_owned(), action)]);
    let approve = answer(json!({"action": "accept", "content": {"approve": true}}));

    // A nonce the server never issued shows a new dialog.
    let response = session
        .call_once(
            "approve_spec",
            program.arguments(),
            Some("not-a-real-nonce"),
            Some(approve.clone()),
        )
        .await;
    assert!(
        matches!(response, CallToolResponse::InputRequired(_)),
        "{response:?}"
    );

    // A nonce issued for another folder can't approve this one.
    let other = Program::with_add_spec();
    let CallToolResponse::InputRequired(dialog) = session
        .call_once("approve_spec", other.arguments(), None, None)
        .await
    else {
        panic!("approve_spec should show a dialog");
    };
    let foreign = dialog.request_state.expect("a nonce");
    let response = session
        .call_once(
            "approve_spec",
            program.arguments(),
            Some(&foreign),
            Some(approve.clone()),
        )
        .await;
    assert!(
        matches!(response, CallToolResponse::InputRequired(_)),
        "{response:?}"
    );
    assert!(program.read("lengua.lock").is_none());

    // A nonce answers once: after a decline, it can't be reused to approve.
    let CallToolResponse::InputRequired(dialog) = session
        .call_once("approve_spec", program.arguments(), None, None)
        .await
    else {
        panic!("approve_spec should show a dialog");
    };
    let nonce = dialog.request_state.expect("a nonce");
    let declined = session
        .call_once(
            "approve_spec",
            program.arguments(),
            Some(&nonce),
            Some(answer(json!({"action": "decline"}))),
        )
        .await;
    assert!(
        matches!(&declined, CallToolResponse::Complete(result) if result.structured_content.as_ref().is_some_and(|report| report["approved"] == false)),
        "{declined:?}"
    );
    let reused = session
        .call_once(
            "approve_spec",
            program.arguments(),
            Some(&nonce),
            Some(approve.clone()),
        )
        .await;
    assert!(
        matches!(reused, CallToolResponse::InputRequired(_)),
        "{reused:?}"
    );
    assert!(program.read("lengua.lock").is_none());

    // A fresh nonce approves its own folder.
    let CallToolResponse::InputRequired(dialog) = session
        .call_once("approve_spec", program.arguments(), None, None)
        .await
    else {
        panic!("approve_spec should show a dialog");
    };
    let nonce = dialog.request_state.expect("a nonce");
    let approved = session
        .call_once(
            "approve_spec",
            program.arguments(),
            Some(&nonce),
            Some(approve),
        )
        .await;
    assert!(
        matches!(&approved, CallToolResponse::Complete(result) if result.structured_content.as_ref().is_some_and(|report| report["approved"] == true)),
        "{approved:?}"
    );
    assert!(program.read("lengua.lock").is_some());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_invalid_spec_is_reported_without_a_dialog() {
    let session = Session::start(
        Protocol::Discover,
        TestClient::with_elicitation(Answer::Approve),
    )
    .await;
    let program = Program::new();
    program.write(
        "spec.md",
        &ADD_SPEC.replace("exits with code `1`", "exits with code `134`"),
    );
    let error = session
        .call_error("approve_spec", program.arguments())
        .await;
    assert!(error.contains("has 1 problem to fix"), "{error}");
    assert!(error.contains("134 is reserved"), "{error}");
    assert!(session.client.prompts().is_empty());
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn paths_must_be_absolute_and_hold_a_spec() {
    let session = Session::start(
        Protocol::Discover,
        TestClient::with_elicitation(Answer::Approve),
    )
    .await;
    let relative = session
        .call_error("approve_spec", json!({ "path": "add" }))
        .await;
    assert!(relative.contains("absolute path"), "{relative}");
    let status = session.call_error("status", json!({ "path": "add" })).await;
    assert!(status.contains("absolute path"), "{status}");
    let empty = Program::new();
    let missing = session.call_error("approve_spec", empty.arguments()).await;
    assert!(missing.contains("There's no spec.md"), "{missing}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn status_follows_a_program_through_approval() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol, TestClient::with_elicitation(Answer::Approve)).await;
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
            text(&report["next_step"]).contains("call approve_spec"),
            "{report}"
        );

        session.call_ok("approve_spec", program.arguments()).await;
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
        let approval = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(approval["spec_label"], label(&edited), "{approval}");
        assert_eq!(session.client.prompts().len(), 2, "{protocol:?}");

        // A lock edited by hand can't be trusted, and approving starts over.
        let lock = program.read("lengua.lock").expect("the lock exists");
        program.write(
            "lengua.lock",
            &lock.replace("\"elicitation\"", "\"tool-permission\""),
        );
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "lock_invalid", "{report}");
        assert_eq!(report["program"]["lock"]["state"], "invalid", "{report}");
        assert!(
            text(&report["program"]["lock"]["problem"]).contains("signature doesn't match"),
            "{report}"
        );
        assert!(
            text(&report["next_step"]).starts_with("Blocked:"),
            "{report}"
        );
        let approval = session.call_ok("approve_spec", program.arguments()).await;
        assert_eq!(approval["approved"], true, "{approval}");
        let report = status(&session, &program).await;
        assert_eq!(report["program"]["state"], "tests_missing", "{report}");
        session.service.cancel().await.expect("close the session");
    }
}
