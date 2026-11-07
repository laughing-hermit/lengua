//! main.wat's path end to end, the same on every system: the add program, an Attested
//! Computation, from the user's request to done through the MCP session, then run with its
//! arguments, run from a shell with `lengua-mcp run`, and attested with `lengua-mcp check`; its
//! lock's stamp, format 6; lengua's entry in the idea's `verified`, written at done and taken away
//! once the idea changes; and how check answers a folder with both code files, a rule broken on
//! its line, and failing tests. Nothing here needs LLVM, a Linux kernel or a CPU target.

mod common;

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use common::{Program, Protocol, Session};
use lengua::hash::Sha256Hex;
use lengua::trust;
use serde_json::{Value, json};

/// The add program's idea, an Attested Computation.
const IDEA: &str = include_str!("fixtures-wasm/add/idea.md");

/// Its code, in WebAssembly text.
const CODE: &str = include_str!("fixtures-wasm/add/main.wat");

/// A value of a reply as text, or none.
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// The line, from 1, that `needle` is first on in `text`.
fn line_of(text: &str, needle: &str) -> usize {
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} isn't in the text"));
    text[..at].matches('\n').count() + 1
}

/// `text` with the first `from` in it made `to`.
fn changed(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from:?} isn't in the text");
    text.replacen(from, to, 1)
}

/// `run`'s arguments for `program` with `args` for the program.
fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// `lengua-mcp` run with `args`, from a shell.
fn lengua_mcp(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .args(args)
        .output()
        .expect("lengua-mcp runs")
}

/// `lengua-mcp run` of the program in `folder`, with the session's data folder and so its key,
/// and `args` after the folder.
fn run_from_shell(session: &Session, folder: &Path, args: &[&str]) -> Output {
    let mut all: Vec<&OsStr> = vec![
        OsStr::new("--data-dir"),
        session.data_dir.path().as_os_str(),
        OsStr::new("run"),
        folder.as_os_str(),
    ];
    all.extend(args.iter().map(OsStr::new));
    lengua_mcp(&all)
}

/// `lengua-mcp check` of the program in `folder`: its exit code, and what it printed.
fn attest(folder: &Path) -> (Option<i32>, String) {
    let attested = lengua_mcp(&[OsStr::new("check"), folder.as_os_str()]);
    (
        attested.status.code(),
        String::from_utf8_lossy(&attested.stdout).into_owned(),
    )
}

/// A program folder holding add's idea, recorded with the user's words, and its main.wat.
async fn add(session: &Session) -> Program {
    let program = Program::new();
    program.write("idea.md", IDEA);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", CODE);
    program
}

/// add from the user's request to done, in both protocol generations: the machine line names
/// the engine and no LLVM, the tests pass and say tested, not proven, the lock stamps main.wat
/// in format 6, the idea's `verified` holds lengua's entry, and `run` runs it.
#[tokio::test]
async fn add_goes_from_its_request_to_done_and_runs() {
    for protocol in Protocol::ALL {
        let session = Session::start(protocol).await;
        let program = Program::new();
        // A new folder says what to write first, and the machine, on every system.
        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_idea", "{protocol:?}: {report}");
        assert!(text(&report["machine"]).contains("core"), "{report}");
        program.write("idea.md", IDEA);
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{protocol:?}: {report}");
        let machine = text(&report["machine"]);
        assert!(
            machine.contains("Wasmtime") && machine.contains("Cranelift"),
            "{report}"
        );
        assert!(
            !machine.contains("LLVM") && !machine.contains("main.s "),
            "{report}"
        );
        assert!(text(&report["next_step"]).contains("main.wat"), "{report}");
        program.write("main.wat", CODE);
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{protocol:?}: {done}");
        assert_eq!(
            done["tests"][0], "main.wat follows every rule and passed all 202 test runs",
            "{done}"
        );
        // Tested, not proven: the proofs of main.wat come with the WebAssembly prover.
        assert!(any_line(&done["tests"], "tested, not proven"), "{done}");
        assert!(
            any_line(&done["proofs"], "generated B1: tested, not proven"),
            "{done}"
        );
        assert!(
            text(&done["memory"]).contains("tested, not proven"),
            "{done}"
        );
        assert!(
            text(&done["next_step"]).contains("tested, not proven"),
            "{done}"
        );
        assert_eq!(
            done["example"],
            "example B1: given arguments [\"2\", \"3\"], the program printed \"5\\n\", exited \
             with code 0",
            "{done}"
        );

        // The lock stamps main.wat as it is, its module and the engine, in format 6, and no
        // proof.
        let lock = program.read("lengua.lock").expect("the lock");
        let module = lengua::wasm::text::build(CODE)
            .expect("main.wat builds")
            .wasm;
        assert!(
            lock.contains("version = 6\ndialect = \"webassembly 1\"\n"),
            "{lock}"
        );
        assert!(
            lock.contains(&format!(
                "\n[code]\nsha256 = \"{}\"\nmodule_sha256 = \"{}\"\nengine = \"wasmtime {}\"\n",
                Sha256Hex::of(CODE.as_bytes()),
                Sha256Hex::of(&module),
                lengua::wasm::engine::ENGINE_VERSION
            )),
            "{lock}"
        );
        assert!(lock.contains("unsafe_blocks = 0\n"), "{lock}");
        assert!(
            lock.contains("target = \"webassembly\"\nfeatures = [")
                && lock.contains("\"bulk-memory\""),
            "{lock}"
        );
        assert!(!lock.contains("[proofs]"), "{lock}");

        // lengua's entry in `verified` names the idea and the code it checked, and changes
        // neither hash, so the lock still holds.
        let idea = program.read("idea.md").expect("idea.md");
        let entries = trust::lengua_entries(&idea);
        assert_eq!(entries.len(), 1, "{idea}");
        assert_eq!(entries[0].by, trust::ACTOR);
        assert_eq!(entries[0].idea, trust::idea_hash(IDEA.as_bytes()));
        assert_eq!(entries[0].code, Sha256Hex::of(CODE.as_bytes()));
        assert_eq!(
            trust::idea_hash(idea.as_bytes()),
            trust::idea_hash(IDEA.as_bytes())
        );
        // Checked again, it stays done, and the entry stays as it was.
        let again = session.settle(&program).await;
        assert_eq!(again["state"], "done", "{again}");
        assert_eq!(program.read("idea.md").as_deref(), Some(idea.as_str()));

        // run runs it on the user's inputs, in a worker of its own.
        let sum = session
            .call_ok("run", with_args(&program, &["2", "3"]))
            .await;
        assert_eq!(sum["stdout"], "5\n", "{sum}");
        assert_eq!(sum["exit_code"], 0, "{sum}");
        assert_eq!(sum["ended"], "exit code 0", "{sum}");
        let negative = session
            .call_ok("run", with_args(&program, &["-7", "-1000000000"]))
            .await;
        assert_eq!(negative["stdout"], "-1000000007\n", "{negative}");
        let usage = session.call_ok("run", with_args(&program, &[])).await;
        assert_eq!(usage["stderr"], "usage: add <a> <b>\n", "{usage}");
        assert_eq!(usage["exit_code"], 1, "{usage}");
        session.service.cancel().await.expect("close the session");
    }
}

/// From a shell, `lengua-mcp run` runs the checked program with the shell's own streams and its
/// exit code, the arguments after the folder with `--` or without; `lengua-mcp check` attests
/// the folder with no lock or key, and not once main.wat is edited, which run refuses too.
#[tokio::test]
async fn a_shell_runs_and_attests_the_checked_program() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    assert!(text(&done["shell"]).ends_with(" --"), "{done}");
    for args in [&["2", "3"][..], &["--", "2", "3"][..]] {
        let sum = run_from_shell(&session, program.path(), args);
        assert_eq!(sum.stdout, b"5\n", "{args:?}: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{args:?}: {sum:?}");
    }
    let usage = run_from_shell(&session, program.path(), &[]);
    assert_eq!(usage.stderr, b"usage: add <a> <b>\n", "{usage:?}");
    assert_eq!(usage.status.code(), Some(1), "{usage:?}");

    // The attester needs no data folder: it writes nothing and reads no lock.
    let lock = program.read("lengua.lock");
    let (code, said) = attest(program.path());
    assert_eq!(code, Some(0), "{said}");
    assert_eq!(said.lines().last(), Some("attested"), "{said}");
    assert!(said.contains("tests: passed all 202 test runs"), "{said}");
    assert!(said.contains("main.wat: follows every rule"), "{said}");
    assert!(
        said.contains(&format!("verified: {}", trust::ACTOR)),
        "{said}"
    );
    assert_eq!(
        program.read("lengua.lock"),
        lock,
        "the attester wrote nothing"
    );

    // Edited, main.wat isn't the code lengua's entry names, and run refuses it.
    program.write("main.wat", &format!("{CODE};; edited\n"));
    let (code, said) = attest(program.path());
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains("names the code"), "{said}");
    assert_eq!(said.lines().last(), Some("not attested"), "{said}");
    let refused = run_from_shell(&session, program.path(), &["2", "3"]);
    assert_eq!(refused.status.code(), Some(125), "{refused:?}");
    let reason = String::from_utf8_lossy(&refused.stderr);
    assert!(
        reason.contains("lengua runs a program only when its idea and code match lengua.lock"),
        "{reason}"
    );
    assert!(refused.stdout.is_empty(), "{refused:?}");

    // Checked again to done, lengua's entry names the edited code, and it attests.
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let idea = program.read("idea.md").expect("idea.md");
    let entries = trust::lengua_entries(&idea);
    assert_eq!(entries.len(), 1, "{idea}");
    assert_eq!(
        entries[0].code,
        Sha256Hex::of(format!("{CODE};; edited\n").as_bytes())
    );
    let (code, said) = attest(program.path());
    assert_eq!(code, Some(0), "{said}");
    session.service.cancel().await.expect("close the session");
}

/// Once the idea changes, lengua's entry in `verified` goes: the new idea is unverified until
/// check takes it to done, as its failing test here keeps it from.
#[tokio::test]
async fn lengua_s_entry_goes_when_the_idea_changes() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let idea = program.read("idea.md").expect("idea.md");
    assert_eq!(trust::lengua_entries(&idea).len(), 1, "{idea}");
    // The usage line the idea now asks for isn't the one main.wat prints.
    program.write(
        "idea.md",
        &changed(&idea, "`usage: add <a> <b>`", "`usage: sum <a> <b>`"),
    );
    let failed = session.settle(&program).await;
    assert_eq!(failed["state"], "tests_failed", "{failed}");
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    assert!(!idea.contains("verified:"), "{idea}");
    assert!(idea.contains("`usage: sum <a> <b>`"), "{idea}");
    session.service.cancel().await.expect("close the session");
}

/// lengua's entry goes wherever check stops, on either path: from an idea made a native one, and
/// from an Attested Computation that has a problem.
#[tokio::test]
async fn lengua_s_entry_goes_wherever_check_stops() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    for (from, to) in [
        ("type: Attested Computation", "type: lengua/program-idea"),
        ("description: Adds", "summary: Adds"),
    ] {
        program.write("idea.md", IDEA);
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{to}: {done}");
        let idea = program.read("idea.md").expect("idea.md");
        assert_eq!(trust::lengua_entries(&idea).len(), 1, "{to}: {idea}");
        program.write("idea.md", &changed(&idea, from, to));
        let stopped = session.check(&program).await;
        assert_ne!(stopped["state"], "done", "{to}: {stopped}");
        let idea = program.read("idea.md").expect("idea.md");
        assert!(trust::lengua_entries(&idea).is_empty(), "{to}: {idea}");
    }
    session.service.cancel().await.expect("close the session");
}

/// A folder's code file is the one its idea's type asks for: both, or main.s for an Attested
/// Computation, is a problem that says what to do.
#[tokio::test]
async fn an_attested_computation_s_code_is_main_wat_alone() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    program.write("main.s", ".intel_syntax noprefix\n");
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(&report["problems"], "holds both main.s and main.wat"),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).starts_with("Next: delete "),
        "{report}"
    );
    std::fs::remove_file(program.file("main.wat")).expect("remove main.wat");
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(&report["problems"], "compile idea.md into main.wat"),
        "{report}"
    );
    // There's no main.wat to keep: the next step compiles it first.
    assert!(
        text(&report["next_step"]).contains("main.wat with the compile skill, delete "),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// What's wrong with main.wat is on its line: a build's problem, and a broken rule in the
/// rule's own words.
#[tokio::test]
async fn a_broken_rule_is_reported_on_its_line() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let unbuilt = changed(CODE, "(i64.add (local.get $a)", "(i64.plus (local.get $a)");
    program.write("main.wat", &unbuilt);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let line = line_of(&unbuilt, "(i64.plus");
    assert!(
        any_line(&report["problems"], &format!("main.wat line {line}: ")),
        "{report}"
    );
    let bare = changed(
        CODE,
        "(call $proc_exit (global.get $EXIT_OK))",
        "(call $proc_exit (i32.const 0))",
    );
    program.write("main.wat", &bare);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let line = line_of(&bare, "(call $proc_exit (i32.const 0))");
    assert!(
        any_line(
            &report["problems"],
            &format!(
                "rule 8 (named constants) main.wat:{line}: the exit status is the bare number 0"
            )
        ),
        "{report}"
    );
    assert!(
        text(&report["next_step"]).contains("power-of-10-webassembly.md"),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

/// A failing test of main.wat is reported as a native program's is: the example and its line
/// in idea.md, what it printed, and a failed assertion on its line of main.wat.
#[tokio::test]
async fn a_failing_test_is_reported_as_native_ones_are() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let wrong = changed(CODE, "(i64.add (local.get $a)", "(i64.sub (local.get $a)");
    program.write("main.wat", &wrong);
    let failed = session.settle(&program).await;
    assert_eq!(failed["state"], "tests_failed", "{failed}");
    assert!(
        any_line(
            &failed["problems"],
            "example B1 (idea.md line 20): stdout should be \"5\\n\", but was \"-1\\n\""
        ),
        "{failed}"
    );
    // Code whose tests fail gets no entry of lengua's in `verified`.
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    assert!(text(&failed["next_step"]).contains("main.wat"), "{failed}");
    // An assertion that fails when the first number is past 1.
    let asserted = changed(
        CODE,
        "        (local.set $a (i64.load (global.get $number)))\n",
        "        (local.set $a (i64.load (global.get $number)))\n        (if (i64.gt_s (local.get \
         $a) (i64.const 1)) (then (call $assert_fail (i32.const 9)) unreachable))\n",
    );
    program.write("main.wat", &asserted);
    let failed = session.settle(&program).await;
    assert_eq!(failed["state"], "tests_failed", "{failed}");
    let line = line_of(&asserted, "(i32.const 9)");
    assert!(
        any_line(
            &failed["problems"],
            &format!("assertion 9 failed at main.wat:{line}")
        ),
        "{failed}"
    );
    session.service.cancel().await.expect("close the session");
}
