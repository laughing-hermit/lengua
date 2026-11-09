//! The server's flow on the compile skill's main template and the WebAssembly fixtures, the same on
//! every system: how `check` tests and proves a program, what it says to each way of going wrong,
//! and what `run` allows after it. tests/wasm_check.rs holds the rest of main.wat's path: a rule
//! broken on its line, a failed assertion on its line, a proof that a restarted server takes from
//! the lock, and a planted bug a proof finds and a real run confirms.

mod common;

use std::path::Path;

use common::{Program, Protocol, REQUEST, Session};
use serde_json::{Value, json};

/// A program that always prints -7: right for the example, wrong for the rest. It still reads
/// its number, which it multiplies by zero, and its 7 is a named constant, as rule 8 asks.
const HARDCODED: [(&str, &str); 2] = [
    (
        "  (global $TEN i64 (i64.const 10))\n",
        "  (global $TEN i64 (i64.const 10))\n  (global $SEVEN i64 (i64.const 7))\n",
    ),
    (
        "(call $write_i64 (i64.sub (i64.const 0) (local.get $n)))",
        "(call $write_i64 (i64.sub (i64.mul (local.get $n) (i64.const 0)) (global.get $SEVEN)))",
    ),
];

/// A change that leaves what a program does as it was, and makes it a new build: a `nop` in cat's
/// `$main`, after its local. The tests and the proofs must be made again for it, and a proof of
/// the old build must not land on it.
const NOP: (&str, &str) = ("    (local $got i32)\n", "    (local $got i32)\n    nop\n");

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// The arguments of `run` for `program` with `args` for the program.
fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// A file of the repository, by its path from the repository's root.
fn read(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The compile skill's main template, main.wat.
fn template() -> String {
    read("plugin/skills/compile/assets/main-template.wat")
}

/// The main template's idea.
fn template_idea() -> String {
    read("plugin/skills/compile/assets/main-template-idea.md")
}

/// `code` with `edits` made in order. Each `from` must be in the text as the edits before it left
/// it.
fn changed(code: &str, edits: &[(&str, &str)]) -> String {
    let mut code = code.to_owned();
    for &(from, to) in edits {
        assert!(code.contains(from), "the text has no {from:?}");
        code = code.replacen(from, to, 1);
    }
    code
}

/// A program folder holding the template's idea, tests included, recorded: the next step is its
/// code.
async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("idea.md", &template_idea());
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// A program folder holding the fixture `name`'s idea.md and main.wat, from tests/fixtures, its
/// idea recorded.
async fn shipped(session: &Session, name: &str) -> Program {
    let program = Program::new();
    for file in ["idea.md", "main.wat"] {
        program.write(file, &read(&format!("tests/fixtures/{name}/{file}")));
    }
    session.record(&program).await;
    program
}

/// Checks until the job ends, which must be done: the tests passed and every proof is final and
/// recorded in the lock. Once the grace for the proofs is over, a reply can say done while the
/// job is still writing the last proof to the lock, and says so ("still running"); a case that
/// reads the lock or restarts the server afterwards must wait for the job to end, which the
/// next check, a moment later, shows.
async fn done(session: &Session, program: &Program) -> Value {
    let mut report = session.settle(program).await;
    for _ in 0..600 {
        if !text(&report["next_step"]).contains("still running") {
            break;
        }
        report = session.settle(program).await;
    }
    assert_eq!(report["state"], "done", "{report}");
    assert!(
        !text(&report["next_step"]).contains("still running"),
        "the job never ended: {report}"
    );
    report
}

/// Checks until the job ends, which must be with failed tests; returns the problems.
async fn failed(session: &Session, program: &Program) -> String {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "tests_failed", "{report}");
    report["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .map(|problem| text(problem).to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Whether the solvers are installed; a note in the run's summary says it when they are not.
fn solvers_installed() -> bool {
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
    }
    installed
}

/// The module's hash a table of lengua.lock names, `[code]` or `[proofs]`.
fn module_in(lock: &str, table: &str) -> Option<String> {
    lock.split(&format!("[{table}]"))
        .nth(1)
        .and_then(|rest| rest.split("module_sha256 = \"").nth(1))
        .and_then(|rest| rest.split('"').next())
        .map(str::to_owned)
}

/// A program that prints the same answer for every number passes its example and fails the
/// generated tests, with the seed that found it, and is not stamped.
#[tokio::test]
async fn hardcoded_answers_fail_generated_tests() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked_template(&session).await;
    program.write("main.wat", &changed(&template(), &HARDCODED));
    let problems = failed(&session, &program).await;
    assert!(problems.contains("generated B1"), "{problems}");
    assert!(problems.contains("(seed "), "{problems}");
    assert!(problems.contains("but was \"-7\\n\""), "{problems}");
    assert!(
        !program
            .read("lengua.lock")
            .expect("the lock")
            .contains("[code]")
    );
    // Calling again with nothing changed runs the tests again, with new inputs.
    assert!(failed(&session, &program).await.contains("(seed "));
    session.service.cancel().await.expect("close the session");
}

/// `run` needs the files `check` tested and the module it tested, and refuses a lock that was
/// edited.
#[tokio::test]
async fn run_needs_matching_files_and_the_tested_build() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked_template(&session).await;
    let code = template();
    program.write("main.wat", &code);
    let error = session.call_error("run", program.arguments()).await;
    assert!(error.contains("doesn't"), "run refuses unchecked code: {error}");
    done(&session, &program).await;

    let report = session.call_ok("run", with_args(&program, &["7"])).await;
    assert_eq!(report["stdout"], "-7\n", "{report}");
    assert_eq!(report["exit_code"], 0, "{report}");
    assert_eq!(report["ended"], "exit code 0", "{report}");
    let report = session.call_ok("run", with_args(&program, &[])).await;
    assert_eq!(report["stderr"], "usage: negate <n>\n", "{report}");
    assert_eq!(report["exit_code"], 1, "{report}");

    // A module that differs from the tested one (as after lengua itself changed) doesn't run.
    let key_bytes: [u8; 32] = std::fs::read(session.data_dir.path().join("key"))
        .expect("the signing key")
        .try_into()
        .expect("32 bytes");
    let key = lengua::lock::Key::from_bytes(key_bytes);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    let stamp = lock.code.as_mut().expect("code stamp");
    stamp.module_sha256 = lengua::hash::Sha256Hex::of(b"another module");
    lock.proofs = None;
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(
        error.contains("differs from the module check tested"),
        "{error}"
    );
    done(&session, &program).await;
    session.call_ok("run", with_args(&program, &["7"])).await;

    // Any edit stops run until it's checked again.
    program.write("main.wat", &format!("{code}\n"));
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(error.contains("call check"), "{error}");
    program.write("main.wat", &code);
    let lock_text = program.read("lengua.lock").expect("the lock");
    program.write(
        "lengua.lock",
        &lock_text.replace("max_stack_bytes = ", "max_stack_bytes = 1"),
    );
    let error = session.call_error("run", with_args(&program, &["7"])).await;
    assert!(error.contains("can't be used"), "{error}");
    session.service.cancel().await.expect("close the session");
}

/// Code waits for an idea whose tests check: an idea with a test that cannot run is refused
/// first, and nothing is locked.
#[tokio::test]
async fn code_waits_for_an_idea_whose_tests_check() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write(
        "idea.md",
        &changed(&template_idea(), &[("| 50 |", "| 0 |")]),
    );
    program.write("main.wat", &template());
    let report = session.record(&program).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    assert!(
        any_line(&report["problems"], "runs must be from 1"),
        "{report}"
    );
    assert!(program.read("lengua.lock").is_none());
    session.service.cancel().await.expect("close the session");
}

/// Tests can be fixed after code ran: the change is checked like any version of the idea, shown
/// with lengua's own marks on what tests less, and approved by nobody.
#[tokio::test]
async fn tests_can_be_fixed_after_code_ran() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked_template(&session).await;
    program.write("main.wat", &template());
    let report = done(&session, &program).await;
    assert!(report.get("changes").is_none(), "{report}");
    let idea = template_idea();

    program.write("idea.md", &changed(&idea, &[("| 200 |", "| 10 |")]));
    let report = done(&session, &program).await;
    let changes = text(&report["changes"]["idea_changed"]);
    assert!(changes.contains("- ") && changes.contains("+ "), "{report}");
    assert!(changes.contains("| 10 |"), "{report}");
    assert_eq!(
        report["changes"]["tests_less"],
        json!(["generated B1: fewer runs, 10 instead of 200"]),
        "{report}"
    );

    // A change that tests more gets no mark, and going back to the first version shows nothing.
    program.write("idea.md", &changed(&idea, &[("| 200 |", "| 300 |")]));
    let report = done(&session, &program).await;
    assert!(report["changes"].get("tests_less").is_none(), "{report}");
    assert!(report["changes"]["idea_changed"].is_string(), "{report}");
    program.write("idea.md", &idea);
    let report = done(&session, &program).await;
    assert!(report.get("changes").is_none(), "{report}");
    session.service.cancel().await.expect("close the session");
}

/// New words in the middle of a build keep the first idea as what changes are shown against.
#[tokio::test]
async fn new_words_in_the_middle_of_a_build_never_hide_a_change() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked_template(&session).await;
    // The build isn't finished: no code has passed yet. Changing the idea and recording it with
    // other words keeps the first idea as what changes are shown against.
    let edited = changed(&template_idea(), &[("# Purpose\n", "# Purpose\nIt is quick. ")]);
    program.write("idea.md", &edited);
    session
        .call_ok("check", program.request("Also make it quick"))
        .await;
    program.write("main.wat", &template());
    let report = done(&session, &program).await;
    assert_eq!(
        report["changes"]["requests"],
        json!([REQUEST, "Also make it quick"]),
        "{report}"
    );
    assert!(
        text(&report["changes"]["idea_changed"]).contains("It is quick."),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// Done comes once the tests pass, while the proofs go on, and the program runs meanwhile.
#[tokio::test]
async fn done_comes_once_the_tests_pass_while_the_proofs_go_on() {
    if !solvers_installed() {
        return;
    }
    // add's proof of every pair of numbers takes minutes, so done comes first, with the proofs
    // still running.
    let session = Session::proving_with_grace(Protocol::Discover, 0).await;
    let program = shipped(&session, "add").await;
    let report = session.check(&program).await;
    assert_eq!(report["state"], "done", "{report}");
    assert!(any_line(&report["proofs"], ": proving"), "{report}");
    assert!(
        text(&report["next_step"]).contains("still running"),
        "{report}"
    );
    // The program runs while its proofs go on.
    let run = session
        .call_ok("run", with_args(&program, &["2", "3"]))
        .await;
    assert_eq!(run["stdout"], "5\n", "{run}");
    session.service.cancel().await.expect("close the session");
}

/// The last line of a proof goes final only once lengua.lock records the proofs: a caller that
/// sees every proof settled, or a server restarted at that moment, finds them there. The write
/// is made slow (600 ms) and each check waits 50 ms at most, so a check lands in that moment.
#[tokio::test]
async fn the_proofs_are_in_the_lock_before_the_last_line_goes_final() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_slow_to_record(Protocol::Discover, 50, 600).await;
    let program = shipped(&session, "cat").await;
    let started = std::time::Instant::now();
    loop {
        let report = session.check(&program).await;
        assert_ne!(report["state"], "tests_failed", "{report}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(600),
            "the proofs never settled: {report}"
        );
        let settled = report["proofs"]
            .as_array()
            .is_some_and(|lines| !lines.is_empty())
            && !any_line(&report["proofs"], ": proving")
            && !any_line(&report["proofs"], "waiting for its turn");
        if settled {
            let lock = program.read("lengua.lock").unwrap_or_default();
            assert!(
                lock.contains("[proofs]"),
                "all proof lines final, but the lock had no proofs: {report}\n{lock}"
            );
            break;
        }
    }
    done(&session, &program).await;
    session.service.cancel().await.expect("close the session");
}

/// An edit in the middle of a proof starts it over, and the old proof never lands on the new
/// build.
#[tokio::test]
async fn an_edit_mid_proof_starts_over_and_the_old_proof_never_lands() {
    if !solvers_installed() {
        return;
    }
    // Each check waits 50 ms at most, so it sees the proof running: cat's, of every text its
    // idea allows, takes over a second even on an idle computer.
    let session = Session::proving_waiting(Protocol::Discover, 50).await;
    let program = shipped(&session, "cat").await;
    loop {
        let report = session.check(&program).await;
        match report["state"].as_str() {
            Some("proving") => break,
            Some("testing") => {}
            _ => panic!("{report}"),
        }
    }
    program.write(
        "main.wat",
        &changed(&read("tests/fixtures/cat/main.wat"), &[NOP]),
    );
    let report = done(&session, &program).await;
    assert!(
        any_line(&report["proofs"], "proven for every input"),
        "{report}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(module_in(&lock, "code").is_some(), "{lock}");
    assert_eq!(module_in(&lock, "code"), module_in(&lock, "proofs"), "{lock}");
    session.service.cancel().await.expect("close the session");
}
