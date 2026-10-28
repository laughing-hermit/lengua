//! Checking main.s through the server: preconditions, the spec label, building, and the rules.

mod common;

use std::path::PathBuf;

use common::{Answer, Program, Protocol, Session, TestClient};
use serde_json::Value;

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/write-assembly/assets")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// A program folder holding the template's spec (approved) and tests (checked).
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let approval = session.call_ok("approve_spec", program.arguments()).await;
    assert_eq!(approval["approved"], true, "{approval}");
    program.write("tests.toml", &asset("template-tests.toml"));
    let tests = session.call_ok("check_tests", program.arguments()).await;
    assert_eq!(tests["checked"], true, "{tests}");
    program
}

async fn session() -> Session {
    Session::start(
        Protocol::Discover,
        TestClient::with_elicitation(Answer::Approve),
    )
    .await
}

#[tokio::test]
async fn the_template_passes_the_rules() {
    let session = session().await;
    let program = checked_template(&session).await;
    program.write("main.s", &asset("main-template.s"));
    let report: Value = session.call_ok("check_code", program.arguments()).await;
    assert_eq!(report["rules_passed"], true, "{report}");
    assert_eq!(report["functions"], 4, "{report}");
    assert_eq!(report["assertions"], 11, "{report}");
    assert!(
        report["max_stack_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes >= 48),
        "{report}"
    );
    assert_eq!(
        report["unsafe_blocks"].as_array().map(Vec::len),
        Some(0),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn violations_come_with_rule_line_and_fix() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s").replacen(
        "    mov rax, qword ptr [rsp]\n",
        "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
        1,
    );
    program.write("main.s", &code);
    let error = session.call_error("check_code", program.arguments()).await;
    let line = code
        .lines()
        .position(|line| line == "    and rsp, -16")
        .expect("the inserted line")
        + 1;
    assert!(error.contains("so nothing ran"), "{error}");
    assert!(
        error.contains(&format!(
            "rule 3 (fixed resources) main.s:{line}: `and` changes rsp"
        )),
        "{error}"
    );
    assert!(
        error.contains("Fix them with the write-assembly skill"),
        "{error}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn main_s_carries_the_current_spec_label_and_builds() {
    let session = session().await;
    let program = checked_template(&session).await;
    let code = asset("main-template.s");
    let wrong_label = format!(
        "# lengua: spec sha256:{}\n{}",
        "0".repeat(64),
        code.split_once('\n').map_or("", |(_, rest)| rest)
    );
    program.write("main.s", &wrong_label);
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(
        error.contains("first line must be `# lengua: spec sha256:"),
        "{error}"
    );

    program.write(
        "main.s",
        &code.replacen("    xor r9d, r9d\n", "    mov r9d, rbx\n", 1),
    );
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(
        error.contains("doesn't build: assembling failed"),
        "{error}"
    );
    assert!(
        error.contains("main.s:"),
        "the assembler's message names the line: {error}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn code_needs_checked_tests_first() {
    let session = session().await;
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    session.call_ok("approve_spec", program.arguments()).await;
    program.write("main.s", &asset("main-template.s"));
    let error = session.call_error("check_code", program.arguments()).await;
    assert!(error.contains("approved spec and checked tests"), "{error}");
    session.service.cancel().await.expect("close the session");
}
