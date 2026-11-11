//! main.wat's path end to end, the same on every system: the add program, an Attested
//! Computation, from the user's request to done through the MCP session, then run with its
//! arguments, run from a shell with `lengua-mcp run`, and attested with `lengua-mcp check`; its
//! lock's stamp, format 7; lengua's entry in the idea's `verified`, written at done and taken away
//! once the idea changes; how check answers a folder with the native lengua's main.s, a rule
//! broken on its line, and failing tests; small ranges tried in full; a run's environment, the
//! only one its program gets; and a program built on another, its part. Then, with the solvers,
//! main.wat's proofs through check: add's proof named in the reply and kept in the lock, a proof
//! by length that stops short kept as far as it went and proven on with prove_further, a planted
//! wrong constant found as a counterexample, a wrong verdict that outlasts a change of lengua's
//! own build, one recorded after done, which the next check replays, one found once another
//! build stamped the same files, which goes on to that build's module, and a program the model
//! refuses left tested, not proven, with the reason.

mod common;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use common::{Program, Protocol, Session};
use lengua::hash::Sha256Hex;
use lengua::trust;
use serde_json::{Value, json};

/// The add program's idea, an Attested Computation.
const IDEA: &str = include_str!("fixtures/add/idea.md");

/// Its code, in WebAssembly text.
const CODE: &str = include_str!("fixtures/add/main.wat");

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

/// The cat program's idea, whose texts run to 5000 bytes, and its code: it reads its whole input
/// with one `$fill`, so its proof goes one length after another.
const CAT_IDEA: &str = include_str!("fixtures/cat/idea.md");

/// cat's main.wat.
const CAT_CODE: &str = include_str!("fixtures/cat/main.wat");

/// How long the session that proves cat gives its proofs: a few seconds, far less than every
/// length of its 5000 bytes takes (src/prove/wasm/tests.rs gives the whole proof 900), so B1's
/// proof stops short of them.
const SHORT_PROOF_SECONDS: u64 = 4;

/// What a proof line that stopped short of a text's longest length offers.
const FURTHER: &str = "; check with prove_further to prove on from there";

/// The compile skill's main template's idea, negate, and its code.
const TEMPLATE_IDEA: &str = include_str!("../plugin/skills/compile/assets/main-template-idea.md");

/// The main template, main-template.wat.
const TEMPLATE_CODE: &str = include_str!("../plugin/skills/compile/assets/main-template.wat");

/// The greet program's idea, which reads the `NAME` environment variable, and its code.
const GREET_IDEA: &str = include_str!("fixtures/greet/idea.md");

/// greet's main.wat.
const GREET_CODE: &str = include_str!("fixtures/greet/main.wat");

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
/// tested, not proven, and only it does, the lock stamps main.wat in format 7, the idea's
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

        // The lock stamps main.wat as it is, its module and the engine, in format 7, and no
        // proof, which had no time.
        let lock = program.read("lengua.lock").expect("the lock");
        let module = lengua::wasm::text::build(CODE)
            .expect("main.wat builds")
            .wasm;
        assert!(
            lock.contains("version = 7\ndialect = \"webassembly 1\"\n"),
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
    // A shell's run logs nothing of lengua's own unless LENGUA_LOG asks, and with it the
    // program's output is the same.
    let logged = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(program.path())
        .args(["2", "3"])
        .env("LENGUA_LOG", "warn")
        .output()
        .expect("lengua-mcp runs");
    assert_eq!(logged.stdout, b"5\n", "{logged:?}");
    assert_eq!(logged.status.code(), Some(0), "{logged:?}");
    let usage = run_from_shell(&session, program.path(), &[]);
    assert_eq!(usage.stderr, b"usage: add <a> <b>\n", "{usage:?}");
    assert_eq!(usage.status.code(), Some(1), "{usage:?}");
    // add's idea uses no environment, so a run that gives it some is refused, as its grants
    // say.
    let given = run_from_shell(&session, program.path(), &["--env", "NAME=you", "2", "3"]);
    assert_eq!(given.status.code(), Some(125), "{given:?}");
    let reason = String::from_utf8_lossy(&given.stderr);
    assert!(
        reason.contains(
            "idea.md doesn't use the environment, so the program gets no environment variables"
        ),
        "{reason}"
    );
    assert!(given.stdout.is_empty(), "{given:?}");

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

/// A module of its own, valid, that prints 6 and a newline: another module than add's.
const OTHER: &str = r#"(module
  (memory (export "memory") 1)
  (data (i32.const 64) "6\n")
  (func (export "_start")
    (drop (call $write_all (i32.const 1) (i32.const 64) (i32.const 2)))))
"#;

/// Where check keeps `module`, under this build's folder of `session`'s data folder.
fn kept(session: &Session, module: &[u8]) -> PathBuf {
    let builder = lengua::wasm::text::BUILDER;
    lengua::state::tested_module(session.data_dir.path(), builder, &Sha256Hex::of(module))
}

/// Where check keeps `module` compiled for this computer, under this build's folder of
/// `session`'s data folder.
fn compiled(session: &Session, module: &[u8]) -> PathBuf {
    let builder = lengua::wasm::text::BUILDER;
    lengua::state::compiled_module(session.data_dir.path(), builder, &Sha256Hex::of(module))
}

/// check keeps the module whose tests passed, and `lengua-mcp run` compiles it where it has no
/// compiled form to load: once add is done, the kept file holds main.wat's build, whose SHA-256
/// is the lock's module hash. With no compiled form, a kept file of other bytes, no file, and
/// the right module only under another builder's folder each leave the run to build main.wat,
/// held to the lock, print add's sum, and keep the build again, with its compiled form; an
/// edited main.wat is refused as before; and once check stamps an edit that changes the module,
/// the old module's files are gone and the new one's are there.
#[tokio::test]
async fn a_shell_runs_the_module_check_kept() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let module = lengua::wasm::text::build(CODE)
        .expect("main.wat builds")
        .wasm;
    let file = kept(&session, &module);
    let held = std::fs::read(&file).ok();
    assert_eq!(held.as_ref(), Some(&module), "{}", file.display());
    let lock = program.read("lengua.lock").expect("the lock");
    let hash = format!("module_sha256 = \"{}\"\n", Sha256Hex::of(&module));
    assert!(lock.contains(&hash), "{lock}");
    let ahead = compiled(&session, &module);
    assert!(ahead.is_file(), "check compiled the module ahead of its runs");
    let sums = |why: &str| {
        // With no compiled form to load, the run takes the kept module, or builds main.wat.
        let _ = std::fs::remove_file(&ahead);
        let sum = run_from_shell(&session, program.path(), &["2", "3"]);
        assert_eq!(sum.stdout, b"5\n", "{why}: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{why}: {sum:?}");
        assert_eq!(std::fs::read(&file).ok().as_ref(), Some(&module), "{why}");
        assert!(ahead.is_file(), "{why}: the run kept no compiled module");
    };
    sums("the kept module");
    let other = lengua::wasm::text::build(OTHER).expect("it builds").wasm;
    std::fs::write(&file, &other).expect("write another module");
    sums("another module in its place");
    std::fs::remove_file(&file).expect("remove the kept module");
    sums("no kept module");
    let builder = lengua::wasm::text::BUILDER ^ 1;
    let elsewhere =
        lengua::state::tested_module(session.data_dir.path(), builder, &Sha256Hex::of(&module));
    let folder = elsewhere.parent().expect("its folder");
    std::fs::create_dir_all(folder).expect("make its folder");
    std::fs::write(&elsewhere, &module).expect("keep the module under another builder");
    std::fs::remove_file(&file).expect("remove the kept module");
    sums("the module under another builder's folder alone");
    // Edited, main.wat is refused, kept module or not.
    program.write("main.wat", &format!("{CODE};; edited\n"));
    let refused = run_from_shell(&session, program.path(), &["2", "3"]);
    assert_eq!(refused.status.code(), Some(125), "{refused:?}");
    assert!(refused.stdout.is_empty(), "{refused:?}");
    // An edit that changes the module, checked to done: the old module's file goes.
    let local = "    (local $a i64)\n";
    let nop = changed(CODE, local, &format!("{local}    nop\n"));
    program.write("main.wat", &nop);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let rebuilt = lengua::wasm::text::build(&nop)
        .expect("main.wat builds")
        .wasm;
    assert_ne!(rebuilt, module);
    assert!(!file.exists(), "{}", file.display());
    assert!(!ahead.exists(), "{}", ahead.display());
    let new = kept(&session, &rebuilt);
    let held = std::fs::read(&new).ok();
    assert_eq!(held.as_ref(), Some(&rebuilt), "{}", new.display());
    let new = compiled(&session, &rebuilt);
    assert!(new.is_file(), "{}", new.display());
    session.service.cancel().await.expect("close the session");
}

/// A run from a shell loads a compiled module only under the user's key, as the module its lock
/// records. check compiles add ahead of its runs and seals what the engine made; in its place,
/// a program that prints 6, compiled by the same engine and sealed by the same key as its own
/// module, add's own with one byte changed, add's own compiled by another key, bytes no engine
/// made, and nothing are each left unloaded: `lengua-mcp run` prints add's sum, from the module
/// check tested, compiled again, and puts add's own compiled form back. lengua-run, which has no
/// compiler, loads none of them either: it says so and leaves the run to lengua-mcp. With the
/// compiled form check made, it runs add itself, and says nothing.
#[tokio::test]
async fn a_shell_loads_a_compiled_module_only_under_the_user_s_key() {
    use lengua::wasm::engine::{self, Setup};
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let module = lengua::wasm::text::build(CODE)
        .expect("main.wat builds")
        .wasm;
    let hash = Sha256Hex::of(&module);
    let file = compiled(&session, &module);
    let sealed = std::fs::read(&file).expect("check compiled the module ahead of its runs");
    let key = signing_key(&session);
    let runner = wasmtime::Engine::new(&engine::runner_config(Setup::Shell)).expect("an engine");
    let loads = |bytes: &[u8]| key.load_sealed(&runner, &hash, bytes).is_ok();
    assert!(loads(&sealed), "check's compiled module doesn't load");

    let compiler = engine::engine(Setup::Shell, None).expect("the engine that compiles");
    let other = lengua::wasm::text::build(OTHER).expect("it builds").wasm;
    let six = key
        .compile_sealed(&compiler, &Sha256Hex::of(&other), &other)
        .expect("it compiles");
    let mut changed = sealed.clone();
    let middle = changed.len() / 2;
    changed[middle] ^= 1;
    let stranger = lengua::lock::Key::from_bytes([9; 32]);
    let theirs = stranger
        .compile_sealed(&compiler, &hash, &module)
        .expect("it compiles");
    let planted: [(&str, Vec<u8>); 5] = [
        ("another module, under the user's key as itself", six),
        ("add's own compiled module with a byte changed", changed),
        ("add's module under another key", theirs),
        ("bytes no engine made", b"not a compiled module".to_vec()),
        ("nothing", Vec::new()),
    ];
    let exe = std::env::consts::EXE_SUFFIX;
    let alone = Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .with_file_name(format!("lengua-run{exe}"));
    let run_alone = || {
        Command::new(&alone)
            .arg("--data-dir")
            .arg(session.data_dir.path())
            .arg(program.path())
            .args(["2", "3"])
            .output()
            .expect("lengua-run runs")
    };
    if !alone.is_file() {
        lengua::skiplog::note(
            "lengua-run isn't built beside lengua-mcp, which a run of the whole suite does, so \
             only lengua-mcp's run was held to the user's key",
        );
    }
    for (what, bytes) in &planted {
        assert!(!loads(bytes), "{what} loads as add's module");
        std::fs::write(&file, bytes).expect("plant it");
        if alone.is_file() {
            let sum = run_alone();
            assert_eq!(sum.stdout, b"5\n", "{what}: {sum:?}");
            assert_eq!(sum.status.code(), Some(0), "{what}: {sum:?}");
            let said = String::from_utf8_lossy(&sum.stderr);
            assert!(
                said.contains("has no compiled form to load here"),
                "{what}: lengua-run didn't say it left the run to lengua-mcp: {said}"
            );
            std::fs::write(&file, bytes).expect("plant it again");
        }
        let sum = run_from_shell(&session, program.path(), &["2", "3"]);
        assert_eq!(sum.stdout, b"5\n", "{what}: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{what}: {sum:?}");
        let now = std::fs::read(&file).expect("a compiled module");
        assert!(loads(&now), "{what}: the run didn't put add's compiled module back");
    }
    if alone.is_file() {
        let sum = run_alone();
        assert_eq!(sum.stdout, b"5\n", "{sum:?}");
        assert!(sum.stderr.is_empty(), "lengua-run left the run to lengua-mcp: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{sum:?}");
    }
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

/// lengua's entry goes wherever check stops: from an idea made one of the native lengua's type,
/// and from an Attested Computation that has a problem.
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

/// A folder's code file is main.wat alone: the native lengua's main.s beside it is a problem that
/// says what to do, and so is main.s with no main.wat, where check and the attester both say where
/// the native lengua went. An Attested Computation beside main.s alone is recorded with the
/// user's words first, so once main.wat is there and main.s gone, check needs no words again.
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
        any_line(
            &report["problems"],
            "lengua compiles an idea to main.wat since 0.2.0"
        ) && any_line(
            &report["problems"],
            "the tag native-final keeps the native lengua"
        ),
        "{report}"
    );
    // There's no main.wat to keep: the next step compiles it first.
    assert!(
        text(&report["next_step"]).contains("main.wat with the compile skill, delete "),
        "{report}"
    );
    let (code, said) = attest(program.path());
    assert_eq!(code, Some(1), "{said}");
    assert!(
        said.contains("the tag native-final keeps the native lengua"),
        "{said}"
    );
    assert_eq!(said.lines().last(), Some("not attested"), "{said}");
    let fresh = Program::new();
    fresh.write("idea.md", IDEA);
    fresh.write("main.s", ".intel_syntax noprefix\n");
    let report = session.record(&fresh).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        any_line(
            &report["problems"],
            "the tag native-final keeps the native lengua"
        ),
        "{report}"
    );
    std::fs::remove_file(fresh.file("main.s")).expect("remove main.s");
    fresh.write("main.wat", CODE);
    let done = session.settle(&fresh).await;
    assert_eq!(done["state"], "done", "{done}");
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

/// A failing test of main.wat is reported with its lines: the example and its line in idea.md,
/// what it printed, and a failed assertion on its line of main.wat.
#[tokio::test]
async fn a_failing_test_is_reported_with_its_lines() {
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

/// A row whose ranges hold at most 5000 combinations runs every one of them, and the reply says
/// so: the main template's idea with its number from -100 to 100 tries all 201 in B1 and in B4.
#[tokio::test]
async fn small_ranges_run_every_value() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    let full = "`int(-9223372036854775807, 9223372036854775807)`";
    assert_eq!(TEMPLATE_IDEA.matches(full).count(), 2, "{full}");
    let narrowed = changed(
        &TEMPLATE_IDEA.replace(full, "`int(-100, 100)`"),
        "between `-9223372036854775807`\nand `9223372036854775807`",
        "between `-100`\nand `100`",
    );
    program.write("idea.md", &narrowed);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", TEMPLATE_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    for id in ["B1", "B4"] {
        let every = format!("generated {id} tried every value of n in its range (all 201)");
        assert!(any_line(&done["tests"], &every), "{done}");
    }
    session.service.cancel().await.expect("close the session");
}

/// A program gets only the environment its test or its run gives it: greet, checked to done,
/// greets the name a run's `env` gives, and shows its usage line to a run that gives none.
#[tokio::test]
async fn a_run_gives_a_program_only_the_environment_it_names() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", GREET_IDEA);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", GREET_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let mut arguments = program.arguments();
    arguments["env"] = json!({"NAME": "Grace"});
    let greeted = session.call_ok("run", arguments).await;
    assert_eq!(greeted["stdout"], "Hello, Grace!\n", "{greeted}");
    assert_eq!(greeted["exit_code"], 0, "{greeted}");
    // Nothing else reaches it: without NAME, it shows how to use it.
    let usage = session.call_ok("run", program.arguments()).await;
    assert_eq!(usage["stderr"], "usage: NAME=<who> greet\n", "{usage}");
    assert_eq!(usage["exit_code"], 1, "{usage}");
    session.service.cancel().await.expect("close the session");
}

/// A program builds on another, its part, by taking the part's functions word for word, each
/// with a `;; from:` line above it. sum, add's idea and code under another title that builds on
/// add, takes nothing from it at first; then its `$write_i64` says it comes from add, which isn't
/// checked yet; once add is done, so is sum; and a copy that drifts from add's own is told to
/// copy it again.
#[tokio::test]
async fn a_program_builds_on_another() {
    let session = Session::start(Protocol::Discover).await;
    let parent = tempfile::tempdir().expect("a folder for both");
    let part = Program::new_in(parent.path(), "add");
    let node = Program::new_in(parent.path(), "sum");
    part.write("idea.md", IDEA);
    part.write("main.wat", CODE);
    let link = format!("../{}", part.name());
    let building = changed(
        &changed(IDEA, "title: add\n", "title: sum\n"),
        "- Uses: nothing",
        &format!("- Uses: nothing\n- Builds on: [add]({link}/idea.md)"),
    );
    node.write("idea.md", &building);
    let report = session.record(&node).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let problems = |report: &Value| report["problems"].to_string();
    // A part it builds on but takes nothing from.
    node.write("main.wat", CODE);
    let report = session.check(&node).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        problems(&report).contains("takes nothing from it"),
        "{report}"
    );
    // A part that isn't checked yet.
    let header = "  (func $write_i64 (param $value i64) (result i32)  ;; returns: result\n";
    let taken = changed(CODE, header, &format!("  ;; from: {link}\n{header}"));
    node.write("main.wat", &taken);
    let report = session.check(&node).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        problems(&report).contains("isn't checked on this computer"),
        "{report}"
    );
    // Once the part is done, so is the program built on it.
    session.record(&part).await;
    let done = session.settle(&part).await;
    assert_eq!(done["state"], "done", "{done}");
    let done = session.settle(&node).await;
    assert_eq!(done["state"], "done", "{done}");
    // A copy that drifts from the part's own.
    let drifted = changed(
        &taken,
        "    (local $magnitude i64)\n",
        "    (local $magnitude i64)\n    (local $spare i32)\n",
    );
    node.write("main.wat", &drifted);
    let report = session.check(&node).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    assert!(
        problems(&report).contains("differs from") && problems(&report).contains("copy it again"),
        "{report}"
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
        "\n[proofs]\nmodule_sha256 = \"{}\"\nproven = [\"B1\"]\n",
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

/// How far B1's proof line says its proof by length went, in bytes: the N of "proven for every
/// input with text up to N of …", or none when the line names no such length.
fn reached(line: &str) -> Option<usize> {
    line.strip_prefix("generated B1: proven for every input with text up to ")?
        .split(' ')
        .next()?
        .parse()
        .ok()
}

/// A proof by length that stops short of the longest text its idea allows: cat's texts run to
/// 5000 bytes, and a few seconds prove only the shorter ones. The reply says how far B1's proof
/// went and offers to prove on; the lock keeps that bound apart from the full proofs; and a check
/// with prove_further goes on from there, further or to the end. A runner that proves every
/// length in those seconds, or none, leaves nothing to prove on from, and the run's summary says
/// so.
#[tokio::test]
async fn a_proof_that_stops_short_is_kept_and_proven_on() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_for(Protocol::Discover, SHORT_PROOF_SECONDS).await;
    let program = Program::new();
    program.write("idea.md", CAT_IDEA);
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", CAT_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let first = text(&done["proofs"][0]);
    let Some(before) = reached(first) else {
        lengua::skiplog::note("cat's proof didn't stop short of its 5000 bytes in its seconds");
        session.service.cancel().await.expect("close the session");
        return;
    };
    assert!(
        first.contains(" of 5000 bytes (") && first.contains(FURTHER),
        "{done}"
    );
    // The lock keeps the bound apart from the full proofs.
    let lock = program.read("lengua.lock").expect("the lock");
    let bound = format!("{{ id = \"B1\", bytes = {before}, of = 5000, more = true }}");
    assert!(lock.contains(&bound), "{lock}");
    assert!(!lock.contains("proven = [\"B1\""), "{lock}");
    // Proving on goes on from there: further, or to the last length.
    let mut arguments = program.arguments();
    arguments["prove_further"] = json!(true);
    let further = session.settle_with(&program, arguments).await;
    assert_eq!(further["state"], "done", "{further}");
    let again = text(&further["proofs"][0]);
    match reached(again) {
        Some(after) => assert!(after > before, "{before} bytes, then {after}: {further}"),
        None => assert!(
            again.starts_with("generated B1: proven for every input in range ("),
            "{further}"
        ),
    }
    session.service.cancel().await.expect("close the session");
}

/// A wrong constant planted in add's main.wat, so that a sum of zero prints as "-0", is a
/// counterexample through check, the tests_failed state: B1 runs only its ten
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

/// What the lock below records a proof found wrong on add's B1.
const FOUND: &str = "with a = 1 and b = -1: it printed \"-0\"";

/// Whether `ran`, a run from a shell, was refused as a build a proof found wrong.
fn refused_as_wrong(ran: &Output) -> bool {
    ran.status.code() == Some(125)
        && ran.stdout.is_empty()
        && String::from_utf8_lossy(&ran.stderr).contains(WRONG_BUILD)
}

/// The key `session`'s server signs locks with, from its data folder, to sign a lock as lengua
/// does.
fn signing_key(session: &Session) -> lengua::lock::Key {
    let key_bytes: [u8; 32] = std::fs::read(session.data_dir.path().join("key"))
        .expect("the signing key")
        .try_into()
        .expect("32 bytes");
    lengua::lock::Key::from_bytes(key_bytes)
}

/// Whether `program`'s lengua.lock holds `text`.
fn lock_holds(program: &Program, text: &str) -> bool {
    program
        .read("lengua.lock")
        .is_some_and(|lock| lock.contains(text))
}

/// Waits until `program`'s lengua.lock holds `text`, for as long as a proving session gives its
/// proofs; `never` says what didn't come when it never does.
async fn until_the_lock_holds(program: &Program, text: &str, never: &str) {
    let (started, patience) = (Instant::now(), Duration::from_secs(PROOF_SECONDS));
    while !lock_holds(program, text) {
        assert!(started.elapsed() < patience, "{never}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// What a proof found wrong outlasts a change of lengua's own build, after which check builds
/// another module of the same main.wat. add, checked to done, gets a lock that names another
/// module and records a wrong verdict on B1 about it, signed with the session's key, as a lock
/// from before such a change does: `lengua-mcp run` refuses it. Checked again, in a restarted
/// session, B1's verdict is replayed about the new module, which the lock now names in its
/// proofs, and idea.md holds no entry of lengua's; and `lengua-mcp run` still refuses it, where
/// a check that dropped the verdict would let it print the sum.
#[tokio::test]
async fn a_wrong_verdict_outlasts_a_change_of_lengua_s_build() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let key = signing_key(&session);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    let earlier = Sha256Hex::of(b"the module an earlier build of lengua made");
    let stamp = lock.code.as_mut().expect("the code's stamp");
    stamp.module_sha256 = earlier.clone();
    lock.proofs = Some(Box::new(lengua::lock::ProofStamp {
        module_sha256: earlier,
        proven: Vec::new(),
        bounded: Vec::new(),
        solvers: "z3 and cvc5".to_owned(),
        wrong: vec![("B1".to_owned(), FOUND.to_owned())],
        unproven: Vec::new(),
    }));
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let ran = run_from_shell(&session, program.path(), &["2", "3"]);
    assert!(refused_as_wrong(&ran), "{ran:?}");
    let session = session.restart().await;
    let again = session.settle(&program).await;
    assert_eq!(again["state"], "tests_failed", "{again}");
    let replayed = format!("generated B1: {FOUND}");
    assert!(any_line(&again["problems"], &replayed), "{again}");
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    let module = lengua::wasm::text::build(CODE)
        .expect("main.wat builds")
        .wasm;
    let lock = program.read("lengua.lock").expect("the lock");
    let hash = Sha256Hex::of(&module);
    let proofs = format!("\n[proofs]\nmodule_sha256 = \"{hash}\"\n");
    assert!(lock.contains(&proofs), "{lock}");
    let ran = run_from_shell(&session, program.path(), &["2", "3"]);
    assert!(refused_as_wrong(&ran), "{ran:?}");
    session.service.cancel().await.expect("close the session");
}

/// A wrong verdict lengua.lock comes to record about the very module a check took to done, as
/// another server's proof of the same folder, or another build's, records one, is news to that
/// check: the next check, in the same session, starts over and replays the lock's record, says
/// tests_failed with B1's line and takes lengua's entry out of idea.md, where the ended job alone
/// would say done while run refuses.
#[tokio::test]
async fn a_wrong_verdict_recorded_after_done_is_the_next_check_s_news() {
    let session = Session::start(Protocol::Discover).await;
    let program = add(&session).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let idea = program.read("idea.md").expect("idea.md");
    assert_eq!(trust::lengua_entries(&idea).len(), 1, "{idea}");
    let key = signing_key(&session);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    let stamp = lock.code.as_ref().expect("the code's stamp");
    let module = stamp.module_sha256.clone();
    lock.proofs = Some(Box::new(lengua::lock::ProofStamp {
        module_sha256: module,
        proven: Vec::new(),
        bounded: Vec::new(),
        solvers: "z3 and cvc5".to_owned(),
        wrong: vec![("B1".to_owned(), FOUND.to_owned())],
        unproven: Vec::new(),
    }));
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let again = session.settle(&program).await;
    assert_eq!(again["state"], "tests_failed", "{again}");
    let replayed = format!("generated B1: {FOUND}");
    assert!(any_line(&again["problems"], &replayed), "{again}");
    let idea = program.read("idea.md").expect("idea.md");
    assert!(trust::lengua_entries(&idea).is_empty(), "{idea}");
    let ran = run_from_shell(&session, program.path(), &["2", "3"]);
    assert!(refused_as_wrong(&ran), "{ran:?}");
    session.service.cancel().await.expect("close the session");
}

/// How long each write of the proofs to lengua.lock waits in the session below: time to stamp
/// the lock as another build of lengua would, after check's stamp and before the proof's record.
const STAMP_DELAY_MS: u64 = 3000;

/// What a proof finds wrong once another build of lengua stamped the same files goes on to that
/// build's module. Once check stamps the planted wrong constant's module, and before the proof
/// records what it found, the lock is signed again with the session's key so that its stamp names
/// main.wat's module with its names, as a build of lengua from before the name section was cut
/// made it, kept where a run from a shell finds it: what another build's check of the same folder
/// leaves. The proof's record goes to that module, and `lengua-mcp run` refuses it, where a
/// record dropped for naming another module would let it run.
#[tokio::test]
async fn a_wrong_verdict_found_after_another_build_stamped_the_files_goes_on() {
    if !solvers_installed() {
        return;
    }
    let session = Session::proving_slow_to_record(Protocol::Discover, 50, STAMP_DELAY_MS).await;
    let program = Program::new();
    let no_zero = changed(IDEA, ADD_TABLE, NO_ZERO_TABLE);
    program.write("idea.md", &changed(&no_zero, ADD_PURPOSE, NO_ZERO_PURPOSE));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let below_one = SIGN.replace("(i64.const 0)", "(i64.const 1)");
    let wrong = changed(CODE, SIGN, &below_one);
    program.write("main.wat", &wrong);
    let built = lengua::wasm::text::build(&wrong).expect("main.wat builds");
    let report = session.check(&program).await;
    assert!(
        matches!(report["state"].as_str(), Some("testing" | "proving")),
        "{report}"
    );
    let ours = format!("module_sha256 = \"{}\"\n", Sha256Hex::of(&built.wasm));
    until_the_lock_holds(&program, &ours, "check never stamped main.wat").await;
    // Another build stamps the same files as its own module, and keeps it.
    let key = signing_key(&session);
    let mut lock = lengua::lock::read(program.path(), Some(&key))
        .expect("a valid lock")
        .expect("a lock");
    // The proof's record hasn't come yet: its write waits.
    assert!(lock.proofs.is_none(), "{lock:?}");
    let theirs = Sha256Hex::of(&built.named);
    let stamp = lock.code.as_mut().expect("the code's stamp");
    stamp.module_sha256 = theirs.clone();
    lengua::lock::write(program.path(), &lock, &key).expect("write the lock");
    let file = kept(&session, &built.named);
    std::fs::create_dir_all(file.parent().expect("its folder")).expect("make its folder");
    std::fs::write(&file, &built.named).expect("keep their module");
    // The proof's record goes to their module, which run then refuses.
    let found = "wrong = [{ id = \"B1\", found = \"the proof found inputs the tests didn't try";
    until_the_lock_holds(&program, found, "the proof's record never came").await;
    let lock = program.read("lengua.lock").expect("the lock");
    let proofs = format!("\n[proofs]\nmodule_sha256 = \"{theirs}\"\n");
    assert!(lock.contains(&proofs), "{lock}");
    let ran = run_from_shell(&session, program.path(), &["2", "3"]);
    assert!(refused_as_wrong(&ran), "{ran:?}");
    session.service.cancel().await.expect("close the session");
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

/// secret-equal's idea, whose `- Secret:` line names standard input.
const SECRET_IDEA: &str = include_str!("../examples/secret-equal/idea.md");

/// Its code, which compares the two lines with no branch on their bytes.
const SECRET_CODE: &str = include_str!("../examples/secret-equal/main.wat");

/// How secret-equal chooses its answer: a mask made from the differences, with no branch.
const MASKED: &str = "          (local.set $mask\n            (i32.sub (i32.const 0) (i32.eqz \
                      (call $differ (local.get $half)))))\n";

/// The same answer chosen by a branch on the differences, which the secret rule refuses.
const BRANCHED: &str = "          (local.set $mask (i32.const 0))\n          (if (i32.eqz (call \
                        $differ (local.get $half)))\n            (then (local.set $mask \
                        (global.get $ALL_ONES))))\n";

/// secret-equal from the user's request to done: its idea names standard input as secret, its
/// main.wat follows every rule, the secret rule's too, and passes its tests, and `run` answers
/// `same` and `diff`. With a branch on the differences in place of its mask, the secret rule
/// refuses main.wat on that line, with what to write instead.
#[tokio::test]
async fn secret_equal_goes_from_its_request_to_done_and_a_branch_on_a_secret_is_refused() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", SECRET_IDEA);
    let report = session
        .call_ok(
            "check",
            program.request("Make me a program that says whether two secret lines are the same"),
        )
        .await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", SECRET_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    assert!(
        text(&done["tests"][0]).starts_with("main.wat follows every rule and passed all "),
        "{done}"
    );
    for (stdin, answer) in [("aba\naba\n", "same\n"), ("aba\nabb\n", "diff\n")] {
        let mut arguments = program.arguments();
        arguments["stdin"] = json!(stdin);
        let ran = session.call_ok("run", arguments).await;
        assert_eq!(ran["stdout"], answer, "{ran}");
        assert_eq!(ran["exit_code"], 0, "{ran}");
    }
    let branched = changed(SECRET_CODE, MASKED, BRANCHED);
    program.write("main.wat", &branched);
    let report = session.check(&program).await;
    assert_eq!(report["state"], "code_problems", "{report}");
    let line = line_of(&branched, "(if (i32.eqz (call $differ");
    assert!(
        any_line(
            &report["problems"],
            &format!(
                "secrets main.wat:{line}: the condition of this `if` is computed from a secret"
            )
        ),
        "{report}"
    );
    assert!(any_line(&report["problems"], "with a mask"), "{report}");
    session.service.cancel().await.expect("close the session");
}

/// Where secret-equal has written its answer's newline: the shift below goes after it.
const ANSWERED: &str = "          (i32.store8 offset=4 (global.get $out) (global.get $NEWLINE))\n";

/// The answer written, then the first 16 bytes of the input shifted as 8-bit lanes, by one bit
/// or by none as the answer's mask says: an amount made from a secret. The secret rule passes
/// it, since nothing in it branches on a secret or makes an address from one.
const SHIFTED: &str = "          (i32.store8 offset=4 (global.get $out) (global.get $NEWLINE))\n          \
                       (v128.store (global.get $text)\n            (i8x16.shl (v128.load \
                       (global.get $text))\n              (i32.and (local.get $mask) (global.get \
                       $ODD))))\n";

/// The name `check` gives the CPU this test runs on.
fn cpu() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86-64"
    } else {
        "ARM64"
    }
}

/// secret-equal's idea names standard input as secret, so `check` also reads the machine code
/// the engine compiles its main.wat to on this computer, and says which CPU's it read; `run`,
/// a run from a shell and the attester check that code again before they run it. With a shift
/// of 8-bit lanes by an amount made from the secret, which the secret rule passes, x86-64's
/// code looks a mask up in a table at an index made from the amount, and `check` refuses
/// main.wat on the shift's line with what to write instead; ARM64's code is straight, and the
/// same main.wat goes to done there.
#[tokio::test]
async fn the_machine_code_of_a_secret_idea_is_checked_on_this_cpu() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", SECRET_IDEA);
    let report = session
        .call_ok(
            "check",
            program.request("Make me a program that says whether two secret lines are the same"),
        )
        .await;
    assert_eq!(report["state"], "write_code", "{report}");
    program.write("main.wat", SECRET_CODE);
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    let kept = format!(
        "the {} code the engine compiles main.wat to on this computer keeps the secret rule",
        cpu()
    );
    assert!(any_line(&done["tests"], &kept), "{done}");
    // `run` checks the code it is about to run, and runs it.
    let mut arguments = program.arguments();
    arguments["stdin"] = json!("aba\naba\n");
    let ran = session.call_ok("run", arguments).await;
    assert_eq!(ran["stdout"], "same\n", "{ran}");
    // So does a run from a shell, here with no input, which the program answers with its usage.
    let usage = run_from_shell(&session, program.path(), &[]);
    assert_eq!(
        usage.stderr, b"usage: two lines of the same length\n",
        "{usage:?}"
    );
    assert_eq!(usage.status.code(), Some(1), "{usage:?}");
    // And the attester, which says what it read.
    let (code, said) = attest(program.path());
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains(&format!("main.wat: {kept}")), "{said}");

    let shifted = changed(SECRET_CODE, ANSWERED, SHIFTED);
    program.write("main.wat", &shifted);
    let report = session.settle(&program).await;
    if cfg!(target_arch = "x86_64") {
        assert_eq!(report["state"], "code_problems", "{report}");
        // On one of the three lines planted: the shift's own, or the line of the operator the
        // table's code is beside, when the engine's optimizer made the shift anew.
        let first = line_of(&shifted, "(v128.store (global.get $text)");
        let table = "with the address of a table in the code";
        let on_its_line = (first..first + 3).any(|line| {
            report["problems"].as_array().is_some_and(|problems| {
                problems.iter().any(|problem| {
                    let problem = text(problem);
                    problem.starts_with(&format!("secrets main.wat:{line}: on x86-64, "))
                        && problem.contains(table)
                })
            })
        });
        assert!(on_its_line, "{report}");
        assert!(
            any_line(
                &report["problems"],
                "Fix: shift by an amount that isn't secret"
            ),
            "{report}"
        );
        // The attester refuses it the same way.
        let (code, said) = attest(program.path());
        assert_ne!(code, Some(0), "{said}");
        assert!(
            said.contains("on x86-64, ") && said.contains(table),
            "{said}"
        );
    } else {
        assert_eq!(report["state"], "done", "{report}");
        assert!(any_line(&report["tests"], &kept), "{report}");
    }
    session.service.cancel().await.expect("close the session");
}
