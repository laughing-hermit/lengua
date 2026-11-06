//! Recording idea.md: the agent writes it from the user's request and calls check with the user's
//! own words, without waiting for an approval; lengua.lock keeps the words, later edits are
//! recorded under them, and every change after them is shown.

mod common;
mod per_target;

use common::{ADD_IDEA, Program, Protocol, REQUEST, Session};
use lengua::hash::Sha256Hex;
use serde_json::{Value, json};

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn hash(text: &str) -> String {
    Sha256Hex::of(text.as_bytes()).to_string()
}

#[tokio::test]
async fn lists_check_with_its_hints() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let tools = session.service.list_all_tools().await.expect("list tools");
    let tool = tools
        .iter()
        .find(|tool| tool.name == "check")
        .expect("check is listed");
    let hints = tool.annotations.as_ref().expect("annotations");
    assert_eq!(hints.read_only_hint, Some(false));
    assert_eq!(hints.destructive_hint, Some(false));
    assert_eq!(hints.idempotent_hint, Some(true));
    let description = tool.description.as_deref().unwrap_or_default();
    assert!(description.contains("never made up"), "{description}");
    assert!(description.contains("call it again"), "{description}");
    let inputs = serde_json::to_value(&tool.input_schema).expect("schema");
    assert_eq!(inputs["required"], json!(["path"]), "{inputs}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn records_the_users_words_in_both_protocols() {
    if !per_target::runs_programs() {
        return;
    }
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::with_add_idea();
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{protocol:?}: {report}");
        assert!(
            text(&report["next_step"]).contains("compile skill"),
            "{report}"
        );
        assert!(
            text(&report["machine"]).contains("core"),
            "the machine shows until main.s is written: {report}"
        );
        assert!(report.get("changes").is_none(), "{report}");
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("words = [\"{REQUEST}\"]\n")),
            "{lock}"
        );
        assert!(
            lock.contains(&format!("[idea]\nsha256 = \"{}\"", hash(ADD_IDEA))),
            "{lock}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn the_path_may_name_the_idea_itself() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    let idea_path = program.file("idea.md");
    let report = session
        .call_ok(
            "check",
            json!({
                "path": idea_path.to_str().expect("UTF-8 path"),
                "request": REQUEST,
            }),
        )
        .await;
    assert_eq!(report["state"], "write_code", "{report}");
    assert!(program.read("lengua.lock").is_some());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn the_users_words_are_needed_the_first_time() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    for arguments in [program.arguments(), program.request(" \n ")] {
        let report = session.call_ok("check", arguments).await;
        assert_eq!(report["state"], "needs_request", "{report}");
        assert!(
            text(&report["next_step"]).contains("as request, the user's words"),
            "{report}"
        );
    }
    let long = session
        .call_error("check", program.request(&"make it ".repeat(300)))
        .await;
    assert!(long.contains("more than 2000 characters"), "{long}");
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn checking_the_same_idea_again_changes_nothing() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    session.record(&program).await;
    let lock = program.read("lengua.lock").expect("the lock");
    for arguments in [program.request(REQUEST), program.arguments()] {
        let again = session.call_ok("check", arguments).await;
        assert_eq!(again["state"], "write_code", "{again}");
        assert_eq!(program.read("lengua.lock").as_deref(), Some(lock.as_str()));
    }
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_invalid_idea_is_reported_before_it_is_recorded() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let invalid = ADD_IDEA.replace("exits with code `1`", "exits with code `134`");
    program.write("idea.md", &invalid);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    assert!(
        report["problems"].as_array().is_some_and(
            |problems| problems.len() == 1 && text(&problems[0]).contains("134 is reserved")
        ),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).starts_with("Next: fix"),
        "{report}"
    );
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn paths_must_be_absolute_and_a_new_folder_asks_for_an_idea() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let relative = session
        .call_error("check", json!({ "path": "add", "request": REQUEST }))
        .await;
    assert!(relative.contains("absolute path"), "{relative}");
    let empty = Program::new();
    let report = session.record(&empty).await;
    assert_eq!(report["state"], "write_idea", "{report}");
    assert!(
        text(&report["next_step"]).contains("write-idea skill"),
        "{report}"
    );
    assert!(empty.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_edited_idea_is_recorded_under_the_same_words() {
    if !per_target::runs_programs() {
        return;
    }
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::with_add_idea();
        session.record(&program).await;
        let edited = ADD_IDEA.replace("Adds two numbers", "Adds two integers");
        program.write("idea.md", &edited);
        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_code", "{protocol:?}: {report}");
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("words = [\"{REQUEST}\"]\n")),
            "the same request: {lock}"
        );
        assert!(
            lock.contains(&format!("idea = \"{}\"", hash(ADD_IDEA))),
            "changes are counted from the first idea: {lock}"
        );
        assert!(
            lock.contains(&format!("[idea]\nsha256 = \"{}\"", hash(&edited))),
            "{lock}"
        );

        // A lock edited by hand can't be used: the user's words are needed again.
        program.write("lengua.lock", &lock.replace("Make me", "Make us"));
        let report = session.check(&program).await;
        assert_eq!(report["state"], "needs_request", "{report}");
        assert!(
            text(&report["next_step"]).contains("can't be used"),
            "{report}"
        );
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{report}");
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn a_build_keeps_its_requests_and_its_first_idea() {
    if !per_target::runs_programs() {
        return;
    }
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    session.record(&program).await;
    // Until code passes, new words join the build rather than start a new one, so changes are
    // still shown from the idea the build started with.
    for number in 1..=7 {
        let idea = ADD_IDEA.replace("Adds two numbers", &format!("Adds two numbers ({number})"));
        program.write("idea.md", &idea);
        let words = format!("Change {number}");
        session.call_ok("check", program.request(&words)).await;
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains(&format!("words = [\"{REQUEST}\", ")),
            "the first request stays: {lock}"
        );
        assert!(
            lock.contains(&format!("\"{words}\"]\n")),
            "the latest request is last: {lock}"
        );
        assert!(
            lock.contains(&format!("idea = \"{}\"", hash(ADD_IDEA))),
            "{lock}"
        );
        assert!(lock.matches("Change ").count() <= 5, "{lock}");
    }
    session.service.cancel().await.expect("close the session");
}
