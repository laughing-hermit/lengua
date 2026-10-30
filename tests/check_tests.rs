//! The tests inside spec.md: checked with the spec, their problems reported with its own and the
//! user's words kept until they're fixed, and an old tests.toml moved into the spec.

mod common;

use common::{ADD_SPEC, Program, Protocol, REQUEST, Session};
use lengua::hash::Sha256Hex;
use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[tokio::test]
async fn records_a_spec_with_its_tests() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::with_add_spec();
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{protocol:?}: {report}");
        assert!(
            text(&report["next_step"]).contains("compile skill"),
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
        let hash = Sha256Hex::of(ADD_SPEC.as_bytes()).to_string();
        assert!(
            lock.contains(&format!("[spec]\nsha256 = \"{hash}\"")),
            "{lock}"
        );
        assert!(!lock.contains("[tests]"), "{lock}");
        let stored = session.data_dir.path().join("stamped").join(&hash);
        assert_eq!(
            std::fs::read_to_string(&stored).expect("the recorded spec is kept"),
            ADD_SPEC
        );

        let again = session.check(&program).await;
        assert_eq!(again["state"], "write_code", "{again}");
        assert_eq!(program.read("lengua.lock").as_deref(), Some(lock.as_str()));
        session.service.cancel().await.expect("close the session");
    }
}

#[tokio::test]
async fn tests_can_be_fixed_under_the_same_request() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    session.record(&program).await;
    // A fix to the tests is an edit of the spec, recorded under the words it already has.
    let fixed = ADD_SPEC.replace("| 200 |", "| 300 |");
    program.write("spec.md", &fixed);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains(&Sha256Hex::of(fixed.as_bytes()).to_string()),
        "{lock}"
    );
    assert!(
        lock.contains(&format!("words = [\"{REQUEST}\"]\n")),
        "{lock}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn problems_are_reported_and_the_words_wait_for_the_fix() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("spec.md", &ADD_SPEC.replace("{a + b}", "{a - b}"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "spec_problems", "{report}");
    assert!(
        report["problems"].as_array().is_some_and(|problems| {
            problems.len() == 1 && text(&problems[0]).contains("formula disagrees with the spec")
        }),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).contains("write-spec skill"),
        "{report}"
    );
    assert!(program.read("lengua.lock").is_none(), "nothing is recorded");
    // The words came with the spec, so the fixed spec is recorded under them without asking.
    program.write("spec.md", ADD_SPEC);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains(&format!("words = [\"{REQUEST}\"]\n")),
        "{lock}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn an_old_tests_file_goes_into_the_spec() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    program.write(
        "tests.toml",
        "[[example]]\nid = \"B1\"\nargs = [\"2\", \"3\"]\n",
    );
    let report = session.record(&program).await;
    assert_eq!(report["state"], "spec_problems", "{report}");
    assert!(
        text(&report["problems"][0]).contains("the tests live in spec.md now"),
        "{report}"
    );
    std::fs::remove_file(program.file("tests.toml")).expect("delete tests.toml");
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn a_changed_spec_checks_its_tests_again() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_spec();
    session.record(&program).await;

    // The tests still fit the edited spec, so one call records it.
    let edited = ADD_SPEC.replace("Adds two numbers", "Adds two integers");
    program.write("spec.md", &edited);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains(&format!(
            "[spec]\nsha256 = \"{}\"\n",
            Sha256Hex::of(edited.as_bytes())
        )),
        "{lock}"
    );

    // When they don't fit, they're the spec's problems to fix.
    let narrower = edited.replace("`-1000000000`", "`-1000`");
    program.write("spec.md", &narrower);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "spec_problems", "{report}");
    assert!(
        report["problems"]
            .to_string()
            .contains("Purpose doesn't state"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}
