//! An outside module: main.wasm, which a compiler made from Rust, C or another language, run as
//! the program of an idea with a `- Module:` line, under the idea's grants and tests, and marked
//! "trusted, not proven" (docs/reviews/outside-2026-11-12.md lists the threat cases and names
//! the test of each).
//!
//! - Through `check`, `run`, `lengua-mcp run`, lengua-run and `lengua-mcp check`: a module that
//!   passes its tests is trusted, not proven, in every reply and before every run, and is never
//!   done, proven, attested or verified, whatever it prints; what lengua's host can't hold by
//!   itself is refused before anything of the module runs; and a module changed since check
//!   doesn't run.
//! - Through the pool of workers, as a test and the `run` tool run a program: hostile modules,
//!   written here as WebAssembly text, which stands in for a compiler's output. None passed a
//!   rule of main.wat's, so what holds them is the host and the engine alone: the deadline for a
//!   loop that never calls the host, in `_start` and in a start function; the stack; the memory
//!   the module declares; every buffer a host call borrows; a table's calls; bulk memory; and
//!   the idea's grants at every call of the host, for a module that skipped `check`'s reading
//!   too.
//! - Real compiler output: a Rust program and a C program built for wasm32-wasip1 on a runner
//!   (`cargo xtask outside-wasm`, which CI's outside-wasm job runs once for every runner, as
//!   the bench's modules are built), each checked and run under an idea: one reads a file its
//!   idea names, and one is refused the network its idea doesn't grant.

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use common::{ADD_CODE, ADD_IDEA, Program, Protocol, REQUEST, Session};
use lengua::runner::{End, Limits, Run, Trap};
use lengua::wasm::engine::Setup;
use lengua::wasm::outside::{self, STATE, TRUST};
use lengua::wasm::{Policy, Pool, Program as Module, Spec};
use serde_json::{Value, json};

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The module `text` builds: WebAssembly text stands in for a compiler's output.
fn wasm(text: &str) -> Vec<u8> {
    lengua::wasm::text::build(text)
        .unwrap_or_else(|problem| panic!("{problem}"))
        .wasm
}

/// The outside module `text` builds, as `check` takes it for an idea that allows `policy`.
fn taken(text: &str, policy: &Policy) -> Module {
    let module =
        outside::read(wasm(text), policy).unwrap_or_else(|problems| panic!("{problems:#?}"));
    Module::new(module.built).expect("the module is kept")
}

/// Runs `module` given `spec`, within `limits`, in a pool of its own.
async fn run(module: &Module, spec: Spec<'_>, limits: Limits) -> Run {
    Pool::new(launcher(), None)
        .run(module, spec, limits)
        .await
        .expect("the program runs")
}

/// Runs the outside module `text` builds with nothing granted, as a test runs it.
async fn run_text(text: &str) -> Run {
    let none = Policy::default();
    run(
        &taken(text, &none),
        Spec::new(&[], &[], none),
        Limits::default(),
    )
    .await
}

/// The code a run exited with.
fn exited(run: &Run) -> i32 {
    match run.end {
        End::Exited(code) => code,
        other => panic!(
            "it didn't exit: {} ({:?})",
            other.describe(Limits::default()),
            run.supplied
        ),
    }
}

const EXIT: &str = r#"(import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))"#;
const WRITE: &str = r#"(import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))"#;
const MEMORY: &str = r#"(memory (export "memory") 1 1)"#;

/// The strings of a reply's list.
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The add idea as an outside module's: its frontmatter names main.wasm, and its `- Module:`
/// line says what the module was built from.
fn outside_add_idea() -> String {
    let idea = ADD_IDEA.replace("computation: main.wat", "computation: main.wasm");
    assert_ne!(idea, ADD_IDEA);
    let line = "- Uses: nothing\n- Module: main.wasm, built from WebAssembly text, in a test";
    let with_line = idea.replace("- Uses: nothing", line);
    assert_ne!(with_line, idea);
    with_line
}

/// An idea of an outside module that prints `prints` and a newline when it gets no arguments,
/// with `constraints` as its `# Constraints` lines.
fn printing_idea(title: &str, prints: &str, constraints: &str) -> String {
    format!(
        "---\ntype: Attested Computation\ntitle: {title}\ndescription: Prints one line of \
         text.\nruntime: webassembly\ncomputation: main.wasm\nexecutor:\n  resource: \
         https://github.com/laughing-hermit/lengua/blob/main/docs/run.md\n  receipt: [stdout, \
         stderr, exit]\nattester:\n  resource: \
         https://github.com/laughing-hermit/lengua/blob/main/docs/check.md\n---\n# Purpose\nThe \
         program prints one line of text. A test uses it.\n\n# Behaviors\n## B1: Prints its \
         line\n- Given no arguments\n- When the program runs\n- Then it prints `{prints}` and a \
         newline\n- And it exits with code `0`\n\n# Constraints\n{constraints}\n- Module: \
         main.wasm, built from WebAssembly text, in a test\n"
    )
}

/// A module that prints `line` and a newline, then exits with code 0; with an argument, it
/// loops without end and calls nothing.
fn printing_module(line: &str) -> String {
    let length = line.len() + 1;
    format!(
        r#"(module {EXIT} {WRITE}
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $sizes (param i32 i32) (result i32)))
  {MEMORY}
  (data (i32.const 64) "{line}\0a")
  (func (export "_start")
    (drop (call $sizes (i32.const 0) (i32.const 4)))
    (if (i32.gt_u (i32.load (i32.const 0)) (i32.const 1))
      (then (loop $forever (br $forever))))
    (i32.store (i32.const 16) (i32.const 64))
    (i32.store (i32.const 20) (i32.const {length}))
    (drop (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 32)))
    (call $exit (i32.const 0))))"#
    )
}

/// A program folder with `idea` and, as main.wasm, the module `text` builds.
fn folder_with(idea: &str, text: &str) -> Program {
    let program = Program::new();
    program.write("idea.md", idea);
    std::fs::write(program.file("main.wasm"), wasm(text)).expect("write main.wasm");
    program
}

/// `run`'s arguments for `program` with `args` for the program.
fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// `lengua-mcp` or lengua-run, `program`, run from a shell with the session's data folder, then
/// `words`.
fn from_shell(program: &Path, session: &Session, words: &[&OsStr]) -> Output {
    Command::new(program)
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .args(words)
        .env_remove("LENGUA_LOG")
        .output()
        .expect("it runs")
}

/// lengua-run beside lengua-mcp, where a run of the whole suite builds it.
fn lengua_run() -> Option<PathBuf> {
    let exe = std::env::consts::EXE_SUFFIX;
    let alone = launcher().with_file_name(format!("lengua-run{exe}"));
    if alone.is_file() {
        Some(alone)
    } else {
        lengua::skiplog::note(
            "lengua-run isn't built beside lengua-mcp, which a run of the whole suite does, so \
             an outside module ran through lengua-mcp alone",
        );
        None
    }
}

/// What a reply must never say of an outside module, anywhere in it.
fn claims_no_more(reply: &Value) {
    assert_ne!(reply["state"], "done", "{reply}");
    assert_ne!(reply["state"], "proving", "{reply}");
    assert!(reply.get("memory").is_none(), "a memory line: {reply}");
    let said = strings(&reply["outside"]);
    assert!(
        said.first().is_some_and(|line| line.starts_with(TRUST)),
        "the reply doesn't start its outside lines with the trust: {reply}"
    );
    for line in strings(&reply["tests"]) {
        assert!(!line.contains("follows every rule"), "{reply}");
    }
    for line in strings(&reply["proofs"]) {
        assert!(line.contains(": not proven"), "{reply}");
    }
    let next = reply["next_step"].as_str().unwrap_or_default();
    assert!(!next.starts_with("Done"), "{reply}");
    assert!(!next.contains("verified list now holds"), "{reply}");
}

/// An outside module that passes its idea's tests is trusted, not proven: `check` says so with
/// every guarantee that doesn't hold, in a state that isn't done, with no proof, no memory line
/// and no entry in idea.md's `verified`; the lock's own text says it; the bundle's log says it;
/// `run`, `lengua-mcp run` and lengua-run run it and say it before it starts; and the attester
/// doesn't attest it.
#[tokio::test]
async fn an_outside_module_is_trusted_not_proven_wherever_it_is_named() {
    let session = Session::start(Protocol::Discover).await;
    let idea = outside_add_idea();
    let program = folder_with(&idea, ADD_CODE);
    let recorded = session.call_ok("check", program.request(REQUEST)).await;
    assert!(
        matches!(recorded["state"].as_str(), Some("testing" | STATE)),
        "{recorded}"
    );
    claims_no_more(&recorded);
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    claims_no_more(&reply);
    let said = strings(&reply["outside"]).join("\n");
    for words in [
        "built from WebAssembly text, in a test",
        "not held: main.wat's rules",
        "not held: bounded loops",
        "not held: the secret rule",
        "not proven: nothing",
        "not attested",
        "held, by lengua's host",
    ] {
        assert!(said.contains(words), "{words}: {reply}");
    }
    let tests = strings(&reply["tests"]).join("\n");
    assert!(tests.contains("main.wasm passed all"), "{reply}");
    assert_eq!(strings(&reply["proofs"]).len(), 1, "{reply}");
    assert!(
        reply["next_step"]
            .as_str()
            .is_some_and(|next| next.starts_with("Trusted, not proven")),
        "{reply}"
    );
    // idea.md is as it was: lengua wrote no entry in its `verified`.
    assert_eq!(program.read("idea.md"), Some(idea));
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("an outside module: trusted, not proven"),
        "{lock}"
    );
    let bundle = program.path().parent().expect("the program's bundle");
    let log = std::fs::read_to_string(bundle.join("log.md")).unwrap_or_default();
    assert!(log.contains("checked: trusted, not proven"), "{log}");
    assert!(!log.contains("checked: done"), "{log}");
    // A second check says the same, from the job that ended.
    let again = session.check(&program).await;
    assert_eq!(again["state"], STATE, "{again}");
    claims_no_more(&again);

    let ran = session
        .call_ok("run", with_args(&program, &["2", "3"]))
        .await;
    assert_eq!(ran["stdout"], "5\n", "{ran}");
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert!(
        ran["outside"]
            .as_str()
            .is_some_and(|line| line.starts_with(TRUST)),
        "{ran}"
    );
    let folder = program.path().as_os_str();
    let words = [OsStr::new("run"), folder, OsStr::new("2"), OsStr::new("3")];
    let shell = from_shell(&launcher(), &session, &words);
    assert_eq!(shell.stdout, b"5\n", "{shell:?}");
    assert_eq!(shell.status.code(), Some(0), "{shell:?}");
    let before = String::from_utf8_lossy(&shell.stderr).into_owned();
    assert!(
        before.starts_with("lengua-mcp: trusted, not proven: main.wasm"),
        "{before}"
    );
    if let Some(alone) = lengua_run() {
        let words = [folder, OsStr::new("2"), OsStr::new("3")];
        let ran = from_shell(&alone, &session, &words);
        assert_eq!(ran.stdout, b"5\n", "{ran:?}");
        assert_eq!(ran.status.code(), Some(0), "{ran:?}");
        let before = String::from_utf8_lossy(&ran.stderr).into_owned();
        assert!(
            before.starts_with("lengua-run: trusted, not proven: main.wasm"),
            "{before}"
        );
    }
    let attested = Command::new(launcher())
        .arg("check")
        .arg(program.path())
        .output()
        .expect("lengua-mcp runs");
    let printed = String::from_utf8_lossy(&attested.stdout).into_owned();
    assert_ne!(attested.status.code(), Some(0), "{printed}");
    assert!(printed.contains("an outside module"), "{printed}");
    assert!(printed.trim_end().ends_with("not attested"), "{printed}");
    session.service.cancel().await.expect("close the session");
}

/// A module can't talk its way to more: one that prints that it is done, proven and attested,
/// and whose tests expect just that, is trusted, not proven, as any other.
#[tokio::test]
async fn a_module_that_says_it_is_proven_is_trusted_not_proven() {
    let session = Session::start(Protocol::Discover).await;
    let claim = "done: every rule holds and every input is proven";
    let idea = printing_idea("claim", claim, "- Uses: nothing");
    let program = folder_with(&idea, &printing_module(claim));
    session.call_ok("check", program.request(REQUEST)).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    claims_no_more(&reply);
    assert_eq!(program.read("idea.md"), Some(idea));
    session.service.cancel().await.expect("close the session");
}

/// What lengua's host can't hold by itself is refused by `check` before anything of the module
/// is compiled or run, and every such reply still says what the module is: no main.wasm yet; a
/// memory with no maximum; a function of lengua's own that the idea doesn't grant; a function
/// of another host; main.wat beside it; and an idea that names a secret.
#[tokio::test]
async fn check_refuses_what_the_host_can_t_hold() {
    let session = Session::start(Protocol::Discover).await;
    let idea = printing_idea("refused", "ok", "- Uses: nothing");
    let program = Program::new();
    program.write("idea.md", &idea);
    let missing = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(missing["state"], "write_code", "{missing}");
    assert!(
        strings(&missing["outside"])
            .first()
            .is_some_and(|line| line.starts_with(TRUST)),
        "{missing}"
    );
    let refused = [
        (
            r#"(module (memory (export "memory") 1) (func (export "_start")))"#.to_owned(),
            "declares no maximum",
        ),
        (
            format!(
                r#"(module (import "lengua_net_1" "tcp_open"
                     (func $open (param i32 i32 i32 i32 i32) (result i32)))
                   {MEMORY} (func (export "_start")))"#
            ),
            "imports lengua_net_1.tcp_open, which its idea doesn't grant",
        ),
        (
            format!(
                r#"(module (import "env" "system" (func $system (param i32) (result i32)))
                   {MEMORY} (func (export "_start")))"#
            ),
            "which lengua's host doesn't have",
        ),
        (
            format!(r#"(module {MEMORY} (func (export "main")))"#),
            "exports no function `_start`",
        ),
    ];
    for (text, why) in &refused {
        std::fs::write(program.file("main.wasm"), wasm(text)).expect("write main.wasm");
        let reply = session.check(&program).await;
        assert_eq!(reply["state"], "code_problems", "{why}: {reply}");
        let problems = strings(&reply["problems"]).join("\n");
        assert!(problems.contains(why), "{why}: {reply}");
        claims_no_more(&reply);
        let run = session.call_error("run", program.arguments()).await;
        assert!(
            run.contains("lengua runs a program only when"),
            "{why}: {run}"
        );
    }
    // Bytes that are no module at all.
    std::fs::write(program.file("main.wasm"), b"#!/bin/sh\nrm -rf ~\n").expect("write main.wasm");
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    // main.wat beside a good main.wasm is a code file too many.
    std::fs::write(program.file("main.wasm"), wasm(&printing_module("ok"))).expect("main.wasm");
    program.write("main.wat", ADD_CODE);
    let both = session.check(&program).await;
    assert_eq!(both["state"], "code_problems", "{both}");
    assert!(
        strings(&both["problems"])
            .join("\n")
            .contains("one code file too many"),
        "{both}"
    );
    std::fs::remove_file(program.file("main.wat")).expect("remove main.wat");
    // An outside module keeps no secret that lengua can check.
    let secret = printing_idea("refused", "ok", "- Uses: nothing\n- Secret: argument `1`");
    program.write("idea.md", &secret);
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "idea_problems", "{reply}");
    assert!(
        strings(&reply["problems"])
            .join("\n")
            .contains("keeps no secret"),
        "{reply}"
    );
    session.service.cancel().await.expect("close the session");
}

/// Only the module `check` tested runs: another in its place, valid or not, doesn't, from the
/// `run` tool or from a shell; and a module that was checked as an outside module doesn't run
/// as a program of main.wat's, nor the other way round, since the idea that says which it is
/// is the idea the lock records.
#[tokio::test]
async fn only_the_module_check_tested_runs() {
    let session = Session::start(Protocol::Discover).await;
    let idea = printing_idea("swap", "ok", "- Uses: nothing");
    let program = folder_with(&idea, &printing_module("ok"));
    session.call_ok("check", program.request(REQUEST)).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    let folder = program.path().as_os_str();
    let shell = || from_shell(&launcher(), &session, &[OsStr::new("run"), folder]);
    assert_eq!(shell().stdout, b"ok\n");
    let tested = std::fs::read(program.file("main.wasm")).expect("main.wasm");
    for other in [wasm(&printing_module("no")), b"not a module".to_vec()] {
        std::fs::write(program.file("main.wasm"), &other).expect("write main.wasm");
        let refused = session.call_error("run", program.arguments()).await;
        assert!(
            refused.contains("lengua runs a program only when"),
            "{refused}"
        );
        let ran = shell();
        assert_eq!(ran.status.code(), Some(125), "{ran:?}");
        assert!(ran.stdout.is_empty(), "{ran:?}");
    }
    std::fs::write(program.file("main.wasm"), &tested).expect("put main.wasm back");
    assert_eq!(shell().stdout, b"ok\n");
    // The same folder as an idea of main.wat's: the idea changed, so nothing of it runs until
    // check has held main.wat to the rules.
    let plain = idea
        .replace("computation: main.wasm", "computation: main.wat")
        .replace(
            "\n- Module: main.wasm, built from WebAssembly text, in a test",
            "",
        );
    assert!(!plain.contains("Module"), "{plain}");
    program.write("idea.md", &plain);
    program.write("main.wat", &printing_module("ok"));
    let refused = session.call_error("run", program.arguments()).await;
    assert!(
        refused.contains("lengua runs a program only when"),
        "{refused}"
    );
    let ran = shell();
    assert_eq!(ran.status.code(), Some(125), "{ran:?}");
    session.service.cancel().await.expect("close the session");
}

/// The `run` tool stops an outside module that never ends at its deadline, with the server
/// still there for the next call; a run from a shell has no deadline, for any program.
#[tokio::test]
async fn the_run_tool_stops_a_module_that_never_ends() {
    let session = Session::start(Protocol::Discover).await;
    let idea = printing_idea("spin", "ok", "- Uses: nothing");
    let program = folder_with(&idea, &printing_module("ok"));
    session.call_ok("check", program.request(REQUEST)).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    let mut arguments = with_args(&program, &["spin"]);
    arguments["timeout_seconds"] = json!(1);
    let started = std::time::Instant::now();
    let ran = session.call_ok("run", arguments).await;
    // Well within the 20 seconds more after which the pool gives up on a worker: the engine
    // stopped the program, not the pool.
    assert!(started.elapsed() < Duration::from_secs(15), "{ran}");
    assert!(ran["exit_code"].is_null(), "{ran}");
    assert!(
        ran["ended"]
            .as_str()
            .is_some_and(|ended| ended.contains("still running after")),
        "{ran}"
    );
    assert!(
        ran["outside"]
            .as_str()
            .is_some_and(|line| line.starts_with(TRUST)),
        "{ran}"
    );
    let ran = session.call_ok("run", program.arguments()).await;
    assert_eq!(ran["stdout"], "ok\n", "{ran}");
    session.service.cancel().await.expect("close the session");
}

/// A loop that never calls the host is stopped at the run's deadline by the engine, in a test's
/// configuration and in the `run` tool's: in `_start`, and in a start function, which runs as
/// the module is instantiated, before `_start`. No rule bounds an outside module's loops.
#[tokio::test(flavor = "multi_thread")]
async fn a_loop_that_never_calls_the_host_ends_at_the_deadline() {
    let in_start =
        format!(r#"(module {MEMORY} (func (export "_start") (loop $forever (br $forever))))"#);
    let at_instantiation = format!(
        r#"(module {MEMORY}
  (func $spin (loop $forever (br $forever)))
  (start $spin)
  (func (export "_start")))"#
    );
    let none = Policy::default();
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    for text in [&in_start, &at_instantiation] {
        let module = taken(text, &none);
        for setup in [Setup::Test, Setup::Run] {
            let spec = Spec {
                setup,
                ..Spec::new(&[], &[], none)
            };
            let started = std::time::Instant::now();
            let ran = run(&module, spec, limits).await;
            assert_eq!(ran.end, End::TimedOut, "{setup:?}: {text}");
            assert!(started.elapsed() < Duration::from_secs(20), "{setup:?}");
        }
    }
}

/// Calls nested without end trap at the engine's stack limit, and a trap ends the program and
/// nothing else: the same pool runs the next program. No rule forbids an outside module's
/// recursion.
#[tokio::test(flavor = "multi_thread")]
async fn calls_without_end_trap_and_the_worker_goes_on() {
    let deep = format!(
        r#"(module {MEMORY}
  (func $deep (result i32) (i32.add (call $deep) (i32.const 1)))
  (func (export "_start") (drop (call $deep))))"#
    );
    let none = Policy::default();
    let pool = Pool::new(launcher(), None);
    let ran = pool
        .run(
            &taken(&deep, &none),
            Spec::new(&[], &[], none),
            Limits::default(),
        )
        .await
        .expect("it runs");
    assert_eq!(ran.end, End::Trapped(Trap::StackOverflow, None));
    for (text, trap) in [
        (r#"(func (export "_start") unreachable)"#, Trap::Unreachable),
        (
            r#"(func (export "_start") (drop (i32.div_u (i32.const 1) (i32.const 0))))"#,
            Trap::DivisionByZero,
        ),
        (
            r#"(func (export "_start") (drop (i32.load (i32.const 65533))))"#,
            Trap::OutOfBounds,
        ),
    ] {
        let module = taken(&format!("(module {MEMORY} {text})"), &none);
        let ran = pool
            .run(&module, Spec::new(&[], &[], none), Limits::default())
            .await
            .expect("it runs");
        assert_eq!(ran.end, End::Trapped(trap, None), "{text}");
    }
    let fine = taken(&printing_module("ok"), &none);
    let ran = pool
        .run(&fine, Spec::new(&[], &[], none), Limits::default())
        .await
        .expect("it runs");
    assert_eq!(
        (exited(&ran), ran.stdout.as_slice()),
        (0, b"ok\n".as_slice())
    );
}

/// An outside module's memory grows to the maximum it declares, which `check` holds to 1 GiB,
/// and no further: past it `memory.grow` gives -1, as the engine's store holds it.
#[tokio::test(flavor = "multi_thread")]
async fn memory_grows_to_what_the_module_declares_and_no_further() {
    let text = format!(
        r#"(module {EXIT} (memory (export "memory") 1 3)
  (func (export "_start")
    (local $grown i32)
    (loop $more
      (if (i32.ne (memory.grow (i32.const 1)) (i32.const -1))
        (then
          (local.set $grown (i32.add (local.get $grown) (i32.const 1)))
          (br_if $more (i32.lt_u (local.get $grown) (i32.const 100))))))
    (call $exit (i32.add (i32.mul (local.get $grown) (i32.const 10)) (memory.size)))))"#
    );
    // Two pages more, to its three, and then none.
    assert_eq!(exited(&run_text(&text).await), 23);
}

/// Every buffer a host call borrows is held to the program's memory by the host itself: an
/// iovec table past the end, a buffer an iovec names past the end, a length that wraps, the
/// arguments written where they don't fit (main.wat's rules size that buffer; nothing sizes an
/// outside module's), and random bytes past the end each stop the program as a load outside
/// its memory does, and nothing of the worker's own memory is read or written.
#[tokio::test(flavor = "multi_thread")]
async fn a_buffer_outside_the_memory_stops_the_program() {
    let args_get = r#"(import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))"#;
    let random = r#"(import "wasi_snapshot_preview1" "random_get"
    (func $random (param i32 i32) (result i32)))"#;
    let fdstat = r#"(import "wasi_snapshot_preview1" "fd_fdstat_get"
    (func $fdstat (param i32 i32) (result i32)))"#;
    let cases = [
        (
            "an iovec table past the memory's end",
            WRITE,
            "(drop (call $fd_write (i32.const 1) (i32.const 65532) (i32.const 1) (i32.const 0)))",
        ),
        (
            "an iovec's buffer past the memory's end",
            WRITE,
            "(i32.store (i32.const 0) (i32.const 65530))
             (i32.store (i32.const 4) (i32.const 64))
             (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16)))",
        ),
        (
            "an iovec's buffer whose end wraps",
            WRITE,
            "(i32.store (i32.const 0) (i32.const -16))
             (i32.store (i32.const 4) (i32.const 64))
             (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16)))",
        ),
        (
            "where the written count goes, past the end",
            WRITE,
            "(i32.store (i32.const 0) (i32.const 64))
             (i32.store (i32.const 4) (i32.const 1))
             (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 65534)))",
        ),
        (
            "the arguments' text where it doesn't fit",
            args_get,
            "(drop (call $args_get (i32.const 0) (i32.const 65532)))",
        ),
        (
            "the arguments' addresses where they don't fit",
            args_get,
            "(drop (call $args_get (i32.const 65534) (i32.const 0)))",
        ),
        (
            "random bytes past the end",
            random,
            "(drop (call $random (i32.const 65000) (i32.const 100000)))",
        ),
        (
            "a descriptor's kind past the end",
            fdstat,
            "(drop (call $fdstat (i32.const 1) (i32.const 65530)))",
        ),
    ];
    let none = Policy::default();
    let args = [
        "a long first argument".to_owned(),
        "and a second".to_owned(),
    ];
    for (what, import, body) in cases {
        let text = format!(r#"(module {import} {MEMORY} (func (export "_start") {body}))"#);
        let module = taken(&text, &none);
        let ran = run(&module, Spec::new(&args, &[], none), Limits::default()).await;
        assert_eq!(ran.end, End::Trapped(Trap::OutOfBounds, None), "{what}");
    }
}

/// What the idea doesn't grant, the host refuses at each call, whatever was read of the module
/// before: this module skips `check`'s reading, which would refuse its imports of lengua's own
/// modules, and every call of the network, the files, the programs, the terminal and the clock
/// answers that it isn't allowed, and does nothing. For main.wat the checker refuses such an
/// import; for an outside module the host's own check at each call is the wall.
#[tokio::test(flavor = "multi_thread")]
async fn the_host_refuses_at_each_call_what_the_idea_doesn_t_grant() {
    // Each function with as many zero arguments as it takes: `(name, module, i32s, i64 at)`.
    let calls: [(&str, &str, &str, &str); 14] = [
        (
            "tcp_open",
            "lengua_net_1",
            "i32 i32 i32 i32 i32",
            "(i32.const 64) (i32.const 11) (i32.const 443) (i32.const 1) (i32.const 0)",
        ),
        (
            "dns_lookup",
            "lengua_net_1",
            "i32 i32 i32 i32 i32",
            "(i32.const 64) (i32.const 11) (i32.const 128) (i32.const 64) (i32.const 0)",
        ),
        (
            "udp_open",
            "lengua_net_1",
            "i32 i32 i32 i32",
            "(i32.const 64) (i32.const 11) (i32.const 53) (i32.const 0)",
        ),
        (
            "udp_recv_from",
            "lengua_net_1",
            "i32 i32 i32 i32 i32",
            "(i32.const 4) (i32.const 128) (i32.const 64) (i32.const 0) (i32.const 8)",
        ),
        (
            "file_open",
            "lengua_fs_1",
            "i32 i32 i32",
            "(i32.const 1) (i32.const 0) (i32.const 0)",
        ),
        (
            "folder_open",
            "lengua_fs_1",
            "i32 i32",
            "(i32.const 1) (i32.const 0)",
        ),
        (
            "program_run",
            "lengua_run_1",
            "i32 i32 i32 i32 i32 i32 i32 i32",
            "(i32.const 64) (i32.const 11) (i32.const 0) (i32.const 0) (i32.const 128) (i32.const 64) (i32.const 0) (i32.const 4)",
        ),
        (
            "term_is",
            "lengua_term_1",
            "i32 i32",
            "(i32.const 0) (i32.const 0)",
        ),
        (
            "term_size",
            "lengua_term_1",
            "i32 i32",
            "(i32.const 0) (i32.const 4)",
        ),
        ("term_raw", "lengua_term_1", "i32", "(i32.const 1)"),
        (
            "path_open",
            "wasi_snapshot_preview1",
            "i32 i32 i32 i32 i32 i64 i64 i32 i32",
            "(i32.const 3) (i32.const 0) (i32.const 64) (i32.const 11) (i32.const 0) (i64.const 0) (i64.const 0) (i32.const 0) (i32.const 0)",
        ),
        (
            "sock_accept",
            "wasi_snapshot_preview1",
            "i32 i32 i32",
            "(i32.const 3) (i32.const 0) (i32.const 0)",
        ),
        (
            "sock_shutdown",
            "wasi_snapshot_preview1",
            "i32 i32",
            "(i32.const 4) (i32.const 2)",
        ),
        (
            "clock_time_get",
            "wasi_snapshot_preview1",
            "i32 i64 i32",
            "(i32.const 0) (i64.const 0) (i32.const 0)",
        ),
    ];
    let imports: String = calls
        .iter()
        .map(|(name, module, params, _)| {
            format!(r#"(import "{module}" "{name}" (func ${name} (param {params}) (result i32)))"#)
        })
        .collect();
    // The exit code counts the calls that answered anything but NOTCAPABLE, 76.
    let body: String = calls
        .iter()
        .map(|(name, _, _, arguments)| {
            format!(
                "(local.set $other (i32.add (local.get $other)
                   (i32.ne (call ${name} {arguments}) (i32.const 76))))"
            )
        })
        .collect();
    let text = format!(
        r#"(module {EXIT} {imports} {MEMORY}
  (data (i32.const 64) "example.com")
  (func (export "_start") (local $other i32) {body} (call $exit (local.get $other))))"#
    );
    let none = Policy::default();
    // `check`'s reading refuses the module for an idea that grants nothing.
    let refused = outside::read(wasm(&text), &none).expect_err("an import the idea doesn't grant");
    assert!(
        refused
            .iter()
            .any(|problem| problem.contains("which its idea doesn't grant")),
        "{refused:#?}"
    );
    // And the host refuses each call of a module that was never read so.
    let unread = Module::of_text(&text).unwrap_or_else(|problem| panic!("{problem}"));
    let args = ["/etc/passwd".to_owned()];
    let ran = run(&unread, Spec::new(&args, &[], none), Limits::default()).await;
    assert_eq!(
        exited(&ran),
        0,
        "{:?}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(
        ran.files.as_ref().is_ok_and(BTreeMap::is_empty),
        "{:?}",
        ran.files
    );
}

/// A toolchain's module calls through a table, which main.wat has none of: the engine checks
/// each such call, and one through an entry that is missing, empty or of another type traps.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_through_a_table_is_checked_by_the_engine() {
    let module = |index: u32| {
        format!(
            r#"(module {EXIT} {MEMORY}
  (type $none (func))
  (type $one (func (param i32)))
  (table 3 3 funcref)
  (elem (i32.const 1) $takes_one)
  (func $takes_one (param i32))
  (func (export "_start") (call_indirect (type $none) (i32.const {index})) (call $exit (i32.const 0))))"#
        )
    };
    // An empty entry, an entry of another type, and one past the table's end.
    for index in [0, 1, 7] {
        let ran = run_text(&module(index)).await;
        assert_eq!(ran.end, End::Trapped(Trap::IndirectCall, None), "{index}");
    }
}

/// Bulk memory, which a toolchain's memcpy and memset compile to, stays inside the memory: a
/// fill or a copy that would cross its end traps before it writes a byte.
#[tokio::test(flavor = "multi_thread")]
async fn bulk_memory_stays_inside_the_memory() {
    for body in [
        "(memory.fill (i32.const 65000) (i32.const 255) (i32.const 100000))",
        "(memory.copy (i32.const 65000) (i32.const 0) (i32.const 1000))",
        "(memory.copy (i32.const 0) (i32.const -8) (i32.const 64))",
    ] {
        let text = format!(r#"(module {MEMORY} (func (export "_start") {body}))"#);
        let ran = run_text(&text).await;
        assert_eq!(ran.end, End::Trapped(Trap::OutOfBounds, None), "{body}");
    }
}

/// Where CI's outside-wasm job put the modules it built from tests/outside's Rust and C, when
/// this run has them (`LENGUA_OUTSIDE_WASM`): `<name>.wasm` for each program. A run on GitHub's
/// runners without them fails, so no such run passes having skipped the compilers' output.
fn compiled(name: &str) -> Option<Vec<u8>> {
    let Some(folder) = std::env::var_os("LENGUA_OUTSIDE_WASM").filter(|folder| !folder.is_empty())
    else {
        assert!(
            std::env::var_os("GITHUB_ACTIONS").is_none(),
            "LENGUA_OUTSIDE_WASM names no folder of the modules built from tests/outside"
        );
        lengua::skiplog::note(
            "LENGUA_OUTSIDE_WASM names no folder, so no module a compiler made from Rust or C \
             was checked or run (cargo xtask outside-wasm builds them)",
        );
        return None;
    };
    let file = Path::new(&folder).join(format!("{name}.wasm"));
    Some(std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display())))
}

/// The idea of tests/outside/`name`, as an idea beside its source.
fn idea_of(name: &str) -> String {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("outside")
        .join(name)
        .join("idea.md");
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// A Rust program, compiled by rustc for wasm32-wasip1 on a runner, under an idea that names
/// files: `check` takes the compiler's module as it is, runs the idea's tests against it, which
/// give it a file to read by its name, and says trusted, not proven; `run` and a shell run it
/// in a folder of the user's. The same module with no files granted is refused the file by
/// the host.
#[tokio::test]
async fn a_rust_program_reads_the_file_its_idea_names() {
    let Some(module) = compiled("count") else {
        return;
    };
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    program.write("idea.md", &idea_of("count"));
    std::fs::write(program.file("main.wasm"), &module).expect("write main.wasm");
    session.call_ok("check", program.request(REQUEST)).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    claims_no_more(&reply);
    let said = strings(&reply["outside"]).join("\n");
    assert!(said.contains("built from Rust"), "{reply}");
    let work = tempfile::tempdir().expect("a working folder");
    std::fs::write(work.path().join("notes.txt"), "one\ntwo\n").expect("a file");
    let mut arguments = program.arguments();
    arguments["folder"] = json!(work.path().to_str().expect("UTF-8"));
    let ran = session.call_ok("run", arguments).await;
    assert_eq!(ran["stdout"], "8\n", "{ran}");
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert!(
        ran["outside"]
            .as_str()
            .is_some_and(|line| line.starts_with(TRUST)),
        "{ran}"
    );
    let words = [
        OsStr::new("run"),
        program.path().as_os_str(),
        OsStr::new("--in"),
        work.path().as_os_str(),
    ];
    let shell = from_shell(&launcher(), &session, &words);
    assert_eq!(shell.stdout, b"8\n", "{shell:?}");
    assert!(
        String::from_utf8_lossy(&shell.stderr).contains("trusted, not proven"),
        "{shell:?}"
    );
    session.service.cancel().await.expect("close the session");
    // With no files granted, the host gives the very same module no folder: it finds no file.
    let files = BTreeMap::from([("notes.txt".to_owned(), "one\ntwo\n".to_owned())]);
    let none = Policy::default();
    let read = outside::read(module, &none).unwrap_or_else(|problems| panic!("{problems:#?}"));
    assert!(
        read.ungranted.contains(&"path_open"),
        "{:?}",
        read.ungranted
    );
    let ungranted = Module::new(read.built).expect("the module is kept");
    let spec = Spec {
        files: &files,
        ..Spec::new(&[], &[], none)
    };
    let ran = run(&ungranted, spec, Limits::default()).await;
    assert_eq!(
        exited(&ran),
        3,
        "{:?}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(
        ran.stdout.is_empty(),
        "{:?}",
        String::from_utf8_lossy(&ran.stdout)
    );
}

/// A C program, compiled by clang for wasm32-wasip1 on a runner, that opens a TCP connection
/// by itself. Under an idea that grants no network, `check` refuses the module for the
/// function it imports; under an idea that grants one computer, the program asks for another,
/// and lengua's host refuses the connection, which the program prints.
#[tokio::test]
async fn a_c_program_is_refused_the_network_its_idea_doesn_t_grant() {
    let Some(module) = compiled("reach") else {
        return;
    };
    let session = Session::start(Protocol::Discover).await;
    let granted = idea_of("reach");
    let line = "- Network: opens TCP connections to port 443 on example.com over TLS\n";
    let ungranted = granted.replace(line, "");
    assert_ne!(ungranted, granted, "the idea's network line changed");
    let program = Program::new();
    program.write("idea.md", &ungranted);
    std::fs::write(program.file("main.wasm"), &module).expect("write main.wasm");
    let refused = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(refused["state"], "code_problems", "{refused}");
    assert!(
        strings(&refused["problems"])
            .join("\n")
            .contains("imports lengua_net_1.tcp_open, which its idea doesn't grant"),
        "{refused}"
    );
    program.write("idea.md", &granted);
    session.check(&program).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], STATE, "{reply}");
    claims_no_more(&reply);
    assert!(
        strings(&reply["outside"])
            .join("\n")
            .contains("built from C"),
        "{reply}"
    );
    let ran = session.call_ok("run", program.arguments()).await;
    assert_eq!(ran["stdout"], "refused\n", "{ran}");
    let connections = ran["connections"].to_string();
    assert!(connections.contains("example.org"), "{ran}");
    session.service.cancel().await.expect("close the session");
}
