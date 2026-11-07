//! main.wat's path end to end, the same on every system: the add program, an Attested
//! Computation, from the user's request to done through the MCP session, then run with its
//! arguments, run from a shell with `lengua-mcp run`, and attested with `lengua-mcp check`; its
//! lock's stamp, format 6; lengua's entry in the idea's `verified`, written at done and taken away
//! once the idea changes; and how check answers a folder with both code files, a rule broken on
//! its line, and failing tests. Then, with the solvers, main.wat's proofs through check as main.s's
//! go: add's proof named in the reply and kept in the lock, a planted wrong constant found as a
//! counterexample, and a program the model refuses left tested, not proven, with the reason.
//! Nothing here needs LLVM, a Linux kernel or a CPU target.

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

/// add's generated test as its idea has it: every pair of numbers up to 10^9, a longer proof
/// than a test waits for.
const ADD_TABLE: &str = "| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | \
                         200 |";

/// add's generated test over numbers of up to three digits, which its proof covers well within a
/// test's time, as the prover's own test of add does.
const SMALL_TABLE: &str = "| `int(-999, 999)` | `int(-999, 999)` | 200 |";

/// add's generated test with `b` from 2 to 500, running only its ten boundary values, of which
/// none sums to zero: no test tries a sum of zero.
const NO_ZERO_TABLE: &str = "| `int(-999, 999)` | `int(2, 500)` | 10 |";

/// The numbers add's Purpose gives, where its generators take their ranges from.
const ADD_PURPOSE: &str = "between `-1000000000` and `1000000000`.";

/// [`ADD_PURPOSE`] for [`SMALL_TABLE`].
const SMALL_PURPOSE: &str = "between `-999` and `999`.";

/// [`ADD_PURPOSE`] for [`NO_ZERO_TABLE`].
const NO_ZERO_PURPOSE: &str = "between `-999` and `999`, the second from `2` to `500`.";

/// add's sign for a value below zero, which a wrong constant makes zero's too.
const SIGN: &str = ";; The sign, for a value below zero.\n    (if (i64.lt_s (local.get $value) \
                    (i64.const 0))";

/// add's sum.
const SUM: &str = "(i64.add (local.get $a) (i64.load (global.get $number)))";

/// The sum through a lane of 64-bit floats and back, on one line: exact for every sum add's idea
/// allows, and an operator the WebAssembly model doesn't follow, a float lane's arithmetic.
const FLOAT_SUM: &str = "(i64.trunc_sat_f64_s (f64x2.extract_lane 0 (f64x2.add (f64x2.splat \
                         (f64.convert_i64_s (local.get $a))) (f64x2.splat (f64.convert_i64_s \
                         (i64.load (global.get $number)))))))";

/// How long the sessions and the attester that prove give the proofs: far more than add's numbers
/// of up to three digits need on any runner.
const PROOF_SECONDS: u64 = 600;

/// What `run` says of a build a proof found wrong.
const WRONG_BUILD: &str = "The proof found inputs where main.wat is wrong";

/// Whether z3 is installed, which the proofs' questions need; a note in the run's summary says so
/// when it isn't.
fn solvers_installed() -> bool {
    let installed = Command::new("z3").arg("--version").output().is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
    }
    installed
}

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

/// `lengua-mcp check` of the program in `folder`, giving its proofs `seconds` in all: its exit
/// code, and what it printed.
fn attest_for(folder: &Path, seconds: u64) -> (Option<i32>, String) {
    let attested = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("check")
        .arg(folder)
        .env("LENGUA_PROOF_SECONDS", seconds.to_string())
        .output()
        .expect("lengua-mcp runs");
    (
        attested.status.code(),
        String::from_utf8_lossy(&attested.stdout).into_owned(),
    )
}

/// `lengua-mcp check` of the program in `folder`, giving its proofs no time, as the sessions of
/// tests that aren't about proofs do.
fn attest(folder: &Path) -> (Option<i32>, String) {
    attest_for(folder, 0)
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
/// the engine and no LLVM, the tests pass, the proof, which this session gives no time, says
/// tested, not proven, and only it does, the lock stamps main.wat in format 6, the idea's
/// `verified` holds lengua's entry, and `run` runs it.
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
        // The proof had no time, so its line says tested, not proven, and nothing else says
        // main.wat's proofs are still to come.
        assert!(!any_line(&done["tests"], "tested, not proven"), "{done}");
        assert!(
            any_line(&done["proofs"], "generated B1: tested, not proven: "),
            "{done}"
        );
        assert!(
            text(&done["memory"]).starts_with("the rules (M1 to M8) hold"),
            "{done}"
        );
        assert!(
            !text(&done["memory"]).contains("WebAssembly prover"),
            "{done}"
        );
        assert!(
            text(&done["next_step"]).contains("and what's proven, memory included."),
            "{done}"
        );
        assert_eq!(
            done["example"],
            "example B1: given arguments [\"2\", \"3\"], the program printed \"5\\n\", exited \
             with code 0",
            "{done}"
        );

        // The lock stamps main.wat as it is, its module and the engine, in format 6, and no
        // proof, which had no time.
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
    // It proves too, here with no time for it, and says so.
    assert!(
        said.contains("\nproofs: generated B1: tested, not proven: "),
        "{said}"
    );
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
    // The planted assertion's own line: `(i32.const 9)` alone is first `$NINE`'s.
    let line = line_of(&asserted, "(call $assert_fail (i32.const 9))");
    assert!(
        any_line(
            &failed["problems"],
            &format!("assertion 9 failed at main.wat:{line}")
        ),
        "{failed}"
    );
    session.service.cancel().await.expect("close the session");
}

/// add's proof, over numbers of up to three digits: the reply names it, proven for every input in
/// range through the WebAssembly, with a memory line that says what it settles of the rules'
/// obligations; the lock keeps it, and a restarted server replays it from there; and
/// `lengua-mcp check` proves it too, within its own time, and says so.
#[tokio::test]
async fn add_s_proof_is_named_and_the_lock_keeps_it() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_for(Protocol::Discover, PROOF_SECONDS).await;
    let program = Program::new();
    let small = changed(IDEA, ADD_TABLE, SMALL_TABLE);
    program.write("idea.md", &changed(&small, ADD_PURPOSE, SMALL_PURPOSE));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let proof = text(&done["proofs"][0]);
    assert!(
        proof.starts_with("generated B1: proven for every input in range (")
            && proof.contains(" through the WebAssembly), "),
        "{done}"
    );
    assert!(!any_line(&done["tests"], "tested, not proven"), "{done}");
    let memory = text(&done["memory"]);
    assert!(
        memory.contains("; on every run the proofs of B1 cover, ")
            || memory.ends_with("so they leave nothing to the proofs"),
        "{done}"
    );
    // The lock keeps the proof, of the module check tested.
    let module = lengua::wasm::text::build(CODE)
        .expect("main.wat builds")
        .wasm;
    let kept = format!(
        "\n[proofs]\nexecutable_sha256 = \"{}\"\nproven = [\"B1\"]\n",
        Sha256Hex::of(&module)
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(lock.contains(&kept), "{lock}");
    // A restarted server has no job in memory: it runs the tests again and takes the proof the
    // lock records of this very module.
    let session = session.restart().await;
    let again = session.settle(&program).await;
    assert_eq!(again["state"], "done", "{again}");
    let replayed = "generated B1: proven for every input in range, as lengua.lock records";
    assert_eq!(again["proofs"][0], replayed, "{again}");
    session.service.cancel().await.expect("close the session");
    // The attester proves it too, in its own time, and says so.
    let (code, said) = attest_for(program.path(), PROOF_SECONDS);
    assert_eq!(code, Some(0), "{said}");
    assert!(
        said.contains("\nproofs: generated B1: proven for every input in range ("),
        "{said}"
    );
    assert!(
        said.contains("\nmemory: the rules (M1 to M8) hold every address"),
        "{said}"
    );
    assert_eq!(said.lines().last(), Some("attested"), "{said}");
}

/// A wrong constant planted in add's main.wat, so that a sum of zero prints as "-0", is a
/// counterexample through check, the same state as a native program's: B1 runs only its ten
/// boundary values, none of which sums to zero, so the tests pass, and the proof finds the sum,
/// which a real run confirms. The lock records it, so run refuses the build, after a restart
/// too. lengua's entry, which check writes only if it says done while the proof still runs, is
/// gone once the proof finds the sum, and the check after the restart, which replays the lock's
/// record, writes none; the attester, given an entry written by hand, doesn't take its word but
/// finds the sum too.
#[tokio::test]
async fn a_planted_wrong_constant_is_a_counterexample_through_check() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_for(Protocol::Discover, PROOF_SECONDS).await;
    let program = Program::new();
    let no_zero = changed(IDEA, ADD_TABLE, NO_ZERO_TABLE);
    program.write("idea.md", &changed(&no_zero, ADD_PURPOSE, NO_ZERO_PURPOSE));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let below_one = SIGN.replace("(i64.const 0)", "(i64.const 1)");
    let wrong = changed(CODE, SIGN, &below_one);
    program.write("main.wat", &wrong);
    let failed = session.settle(&program).await;
    assert_eq!(failed["state"], "tests_failed", "{failed}");
    assert_eq!(
        failed["tests"][0], "main.wat follows every rule and passed all 12 test runs",
        "{failed}"
    );
    let found = "generated B1: the proof found inputs the tests didn't try where main.wat is \
                 wrong: with a = ";
    assert!(any_line(&failed["problems"], found), "{failed}");
    assert!(
        any_line(
            &failed["problems"],
            "stdout should be \"0\\n\", but was \"-0\\n\""
        ),
        "{failed}"
    );
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    // run refuses the build, after a restart too: the lock records what the proof found.
    let lock = program.read("lengua.lock").expect("the lock");
    let recorded = "wrong = [{ id = \"B1\", found = \"the proof found inputs the tests didn't try";
    assert!(lock.contains(recorded), "{lock}");
    let refused = session
        .call_error("run", with_args(&program, &["-2", "2"]))
        .await;
    assert!(refused.contains(WRONG_BUILD), "{refused}");
    let session = session.restart().await;
    let refused = session
        .call_error("run", with_args(&program, &["-2", "2"]))
        .await;
    assert!(refused.contains(WRONG_BUILD), "{refused}");
    let again = session.settle(&program).await;
    assert_eq!(again["state"], "tests_failed", "{again}");
    assert!(any_line(&again["problems"], found), "{again}");
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    session.service.cancel().await.expect("close the session");
    // The attester doesn't take an entry's word for it: given one by hand, it proves main.wat
    // and finds the sum too.
    let entry = trust::Verified::new(
        "2026-11-07T00:00:00Z",
        trust::idea_hash(idea.as_bytes()),
        Sha256Hex::of(wrong.as_bytes()),
    );
    program.write("idea.md", &trust::with_lengua_verified(&idea, &entry));
    let (code, said) = attest_for(program.path(), PROOF_SECONDS);
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains(&format!("\nproofs: {found}")), "{said}");
    assert_eq!(said.lines().last(), Some("not attested"), "{said}");
}

/// A program the WebAssembly model refuses, here for the float lanes add's sum goes through,
/// passes its tests, and its proof stays tested, not proven, with the model's reason on its line
/// of main.wat; the lock records no proof.
#[tokio::test]
async fn a_program_the_model_refuses_stays_tested_not_proven() {
    let session = Session::proving_for(Protocol::Discover, 60).await;
    let program = add(&session).await;
    let floats = changed(CODE, SUM, FLOAT_SUM);
    program.write("main.wat", &floats);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let line = line_of(&floats, "(f64x2.add");
    let refused = format!(
        "generated B1: tested, not proven: main.wat line {line}: `F64x2Add`, which the \
         WebAssembly model doesn't follow yet"
    );
    assert_eq!(done["proofs"][0], refused, "{done}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(!lock.contains("[proofs]"), "{lock}");
    session.service.cancel().await.expect("close the session");
}
