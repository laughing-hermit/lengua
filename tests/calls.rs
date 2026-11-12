//! A program that calls a module (`- Calls:`, W23): an outside module, which a compiler made
//! from another language and lengua marks "trusted, not proven", or another lengua program,
//! which lengua checked on this computer and whose idea offers functions with `- Exports:`
//! (docs/reviews/calls-2026-11-12.md lists what a called module can and cannot reach, and names
//! the test of each).
//!
//! - Through the pool of workers, as a test and the `run` tool run a program: modules written
//!   here as WebAssembly text, which stands in for a compiler's output and passed no rule. A
//!   called module is an instance of its own with a memory of its own; numbers pass in a call
//!   and bytes by copy; it gets no function of the host, whatever it imports; and the run's
//!   deadline, the engine's stack and the memory its module declares bound it.
//! - Through `check`, `run`, `lengua-mcp run`, lengua-run and `lengua-mcp check`: `check` reads
//!   a called module and refuses what lengua doesn't take; the reply, the lock and every run
//!   say what is trusted and what is checked; a module changed since check doesn't run; a called
//!   program's lock is verified before its module is loaded; and each tool of a run works.
//! - Real compiler output: the Rust library of examples/checksum-rust and a C library with the
//!   same functions, each built on a runner (`cargo xtask outside-wasm`).

mod common;

use std::ffi::OsStr;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use common::{Program, Protocol, Session};
use lengua::runner::{End, Limits, Run, Trap};
use lengua::wasm::{CalledModule, Policy, Pool, Program as Module, Spec};
use serde_json::{Value, json};

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// lengua-run beside lengua-mcp, where a run of the whole suite builds it.
fn lengua_run() -> Option<PathBuf> {
    let exe = std::env::consts::EXE_SUFFIX;
    let alone = launcher().with_file_name(format!("lengua-run{exe}"));
    if alone.is_file() {
        Some(alone)
    } else {
        lengua::skiplog::note(
            "lengua-run isn't built beside lengua-mcp, which a run of the whole suite does, so a \
             program that calls a module ran through lengua-mcp alone",
        );
        None
    }
}

/// The module `text` builds: WebAssembly text stands in for a compiler's output.
fn wasm(text: &str) -> Vec<u8> {
    lengua::wasm::text::build(text)
        .unwrap_or_else(|problem| panic!("{problem}"))
        .wasm
}

/// What the lines of `# Constraints` in `lines` allow.
fn policy(lines: &[&str]) -> Policy {
    let (uses, network) =
        lengua::idea::constraints_of(lines).unwrap_or_else(|problems| panic!("{problems:?}"));
    Policy::of(&uses, &network)
}

/// The program `main` builds, with each of `called`, a name and a module's text, beside it.
fn calling(main: &str, called: &[(&str, &str)]) -> Module {
    let built = lengua::wasm::text::build(main).unwrap_or_else(|problem| panic!("{problem}"));
    let calls = called
        .iter()
        .map(|(name, text)| CalledModule::new(name, &wasm(text)).expect("the module is kept"))
        .collect();
    Module::new(built)
        .expect("the program is kept")
        .with_calls(calls)
}

/// Runs `module` with what `policy` allows, as a test runs it, for at most `seconds`.
async fn run(module: &Module, policy: Policy, seconds: u64) -> Run {
    let limits = Limits {
        timeout: Duration::from_secs(seconds),
        ..Limits::default()
    };
    Pool::new(launcher(), None)
        .run(module, Spec::new(&[], &[], policy), limits)
        .await
        .expect("the program runs")
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

const EXIT: &str = r#"(import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))"#;
const WRITE: &str = r#"(import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))"#;
const GIVE: &str = r#"(import "lengua_call_1" "give"
    (func $give (param i32 i32 i32 i32) (result i32)))"#;
const TAKE: &str = r#"(import "lengua_call_1" "take"
    (func $take (param i32 i32 i32 i32) (result i32)))"#;
const MEMORY: &str = r#"(memory (export "memory") 1 1)"#;

/// The line of an idea that calls the module `lib` for `functions`.
fn calls_line(functions: &str) -> String {
    format!("- Calls: lib.wasm, built from WebAssembly text, in a test; its functions: {functions}")
}

/// A library that keeps a buffer at 1024 of its own memory, adds two numbers, and puts the
/// ASCII letters of its buffer's first bytes in capitals.
const LIBRARY: &str = r#"(module
  (memory (export "memory") 1 1)
  (func (export "buffer") (result i32) (i32.const 1024))
  (func (export "add") (param i32 i32) (result i32)
    (i32.add (local.get 0) (local.get 1)))
  (func (export "upper") (param $length i32)
    (local $i i32)
    (local $at i32)
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (local.set $at (i32.add (i32.const 1024) (local.get $i)))
        (if (i32.and
              (i32.ge_u (i32.load8_u (local.get $at)) (i32.const 97))
              (i32.le_u (i32.load8_u (local.get $at)) (i32.const 122)))
          (then
            (i32.store8 (local.get $at)
              (i32.sub (i32.load8_u (local.get $at)) (i32.const 32)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))))"#;

/// Numbers pass in a call, bytes pass by copy: the program gives the module four bytes, has it
/// change them in its own memory, takes them back and prints them, and exits with a sum the
/// module computed.
#[tokio::test]
async fn a_program_calls_a_module_with_numbers_and_copies_bytes() {
    let main = format!(
        r#"(module {EXIT} {WRITE} {GIVE} {TAKE}
  (import "lib" "buffer" (func $buffer (result i32)))
  (import "lib" "add" (func $add (param i32 i32) (result i32)))
  (import "lib" "upper" (func $upper (param i32)))
  {MEMORY}
  (data (i32.const 64) "abc\0a")
  (func (export "_start")
    (if (call $give (i32.const 0) (call $buffer) (i32.const 64) (i32.const 4))
      (then (call $exit (i32.const 90))))
    (call $upper (i32.const 4))
    (if (call $take (i32.const 0) (call $buffer) (i32.const 128) (i32.const 4))
      (then (call $exit (i32.const 91))))
    (i32.store (i32.const 16) (i32.const 128))
    (i32.store (i32.const 20) (i32.const 4))
    (drop (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 32)))
    (call $exit (call $add (i32.const 2) (i32.const 3)))))"#
    );
    let line = calls_line("buffer() -> i32, add(i32, i32) -> i32, upper(i32)");
    let allowed = policy(&["- Uses: nothing", &line]);
    let ran = run(&calling(&main, &[("lib", LIBRARY)]), allowed, 20).await;
    assert_eq!(ran.stdout, b"ABC\n", "{:?}", ran.supplied);
    assert_eq!(exited(&ran), 5);
    // What the program gave is still its own: the module changed its copy.
    assert!(ran.stderr.is_empty(), "{:?}", ran.stderr);
}

/// Every type of number crosses, and the bits of a float are the ones that were passed.
#[tokio::test]
async fn every_type_of_number_crosses_a_call() {
    let library = r#"(module
  (func (export "mix") (param i32 i64 f32 f64) (result f64)
    (f64.add
      (f64.add (f64.convert_i32_s (local.get 0)) (f64.convert_i64_s (local.get 1)))
      (f64.add (f64.promote_f32 (local.get 2)) (local.get 3))))
  (func (export "wide") (param i64) (result i64)
    (i64.mul (local.get 0) (i64.const 3)))
  (func (export "single") (param f32) (result f32)
    (f32.neg (local.get 0))))"#;
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "mix" (func $mix (param i32 i64 f32 f64) (result f64)))
  (import "lib" "wide" (func $wide (param i64) (result i64)))
  (import "lib" "single" (func $single (param f32) (result f32)))
  {MEMORY}
  (func (export "_start")
    (if (f64.ne (call $mix (i32.const 1) (i64.const 2) (f32.const 0.5) (f64.const 0.25))
          (f64.const 3.75))
      (then (call $exit (i32.const 1))))
    (if (i64.ne (call $wide (i64.const 0x100000001)) (i64.const 0x300000003))
      (then (call $exit (i32.const 2))))
    (if (f32.ne (call $single (f32.const 1.5)) (f32.const -1.5))
      (then (call $exit (i32.const 3))))
    (call $exit (i32.const 0))))"#
    );
    let line =
        calls_line("mix(i32, i64, f32, f64) -> f64, wide(i64) -> i64 and single(f32) -> f32");
    let allowed = policy(&["- Uses: nothing", &line]);
    let ran = run(&calling(&main, &[("lib", library)]), allowed, 20).await;
    assert_eq!(exited(&ran), 0, "{:?}", ran.supplied);
}

/// A called module's memory is its own, and the program's is the program's: a byte each writes
/// at the same address is two bytes, and neither reads the other's.
#[tokio::test]
async fn a_called_module_has_a_memory_of_its_own() {
    // What the module's byte at 2048 was before it wrote 0xaa there.
    let library = r#"(module
  (memory (export "memory") 1 1)
  (func (export "scribble") (result i32)
    (local $before i32)
    (local.set $before (i32.load8_u (i32.const 2048)))
    (i32.store8 (i32.const 2048) (i32.const 0xaa))
    (local.get $before)))"#;
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "scribble" (func $scribble (result i32)))
  {MEMORY}
  (func (export "_start")
    (i32.store8 (i32.const 2048) (i32.const 0x11))
    ;; The module doesn't see the program's byte.
    (if (call $scribble) (then (call $exit (i32.const 1))))
    ;; The module keeps its own between calls.
    (if (i32.ne (call $scribble) (i32.const 0xaa)) (then (call $exit (i32.const 2))))
    ;; The program's byte is as it was.
    (call $exit (i32.load8_u (i32.const 2048)))))"#
    );
    let allowed = policy(&["- Uses: nothing", &calls_line("scribble() -> i32")]);
    let ran = run(&calling(&main, &[("lib", library)]), allowed, 20).await;
    assert_eq!(exited(&ran), 0x11, "{:?}", ran.supplied);
}

/// A called module gets no function of lengua's host, whatever it imports and whatever the
/// program's idea grants the program: a module that calls `fd_write`, `path_open` or
/// `proc_exit` stops the run there, and nothing it asked for happens. Here the module skipped
/// `check`'s reading, which refuses any import, so what holds it is the host alone.
#[tokio::test]
async fn a_called_module_reaches_no_function_of_the_host() {
    let shout = r#"(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 64) "leak\0a")
  (func (export "reach") (result i32)
    (i32.store (i32.const 16) (i32.const 64))
    (i32.store (i32.const 20) (i32.const 5))
    (call $fd_write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 32))))"#;
    let open = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 64) "made.txt")
  (func (export "reach") (result i32)
    (call $path_open (i32.const 3) (i32.const 0) (i32.const 64) (i32.const 8) (i32.const 1)
      (i64.const -1) (i64.const -1) (i32.const 0) (i32.const 128))))"#;
    let exit = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (func (export "reach") (result i32)
    (call $exit (i32.const 0))
    (i32.const 0)))"#;
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "reach" (func $reach (result i32)))
  {MEMORY}
  (func (export "_start")
    (call $exit (i32.add (i32.const 40) (call $reach)))))"#
    );
    let line = calls_line("reach() -> i32");
    // The program itself may use files: the called module still may not.
    for (name, library, lines) in [
        ("fd_write", shout, vec!["- Uses: nothing", &line]),
        ("path_open", open, vec!["- Uses: files", &line]),
        ("proc_exit", exit, vec!["- Uses: nothing", &line]),
    ] {
        let ran = run(&calling(&main, &[("lib", library)]), policy(&lines), 20).await;
        assert!(
            matches!(ran.end, End::Trapped(Trap::Other, _)),
            "{name}: {} ({:?})",
            ran.end.describe(Limits::default()),
            ran.supplied
        );
        assert!(
            ran.supplied
                .as_deref()
                .is_some_and(|said| said.contains("a called module reaches nothing")),
            "{name}: {:?}",
            ran.supplied
        );
        assert!(ran.stdout.is_empty(), "{name}: {:?}", ran.stdout);
        assert!(
            ran.files.as_ref().is_ok_and(|files| files.is_empty()),
            "{name}: {:?}",
            ran.files
        );
    }
}

/// A called module that never ends is stopped at the run's deadline, in a function and in a
/// start function, which runs before the program starts; and one whose calls never return
/// stops at the engine's stack, placed at the call main.wat made.
#[tokio::test]
async fn the_deadline_and_the_stack_bound_a_called_module() {
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "work" (func $work (result i32)))
  {MEMORY}
  (func (export "_start")
    (call $exit (call $work))))"#
    );
    let allowed = policy(&["- Uses: nothing", &calls_line("work() -> i32")]);
    let spins = r#"(module
  (func (export "work") (result i32)
    (loop $forever (br $forever))
    (i32.const 0)))"#;
    let ran = run(&calling(&main, &[("lib", spins)]), allowed, 2).await;
    assert_eq!(ran.end, End::TimedOut, "{:?}", ran.supplied);
    let starts = r#"(module
  (func $start (loop $forever (br $forever)))
  (start $start)
  (func (export "work") (result i32) (i32.const 0)))"#;
    let ran = run(&calling(&main, &[("lib", starts)]), allowed, 2).await;
    assert_eq!(ran.end, End::TimedOut, "{:?}", ran.supplied);
    let recurses = r#"(module
  (func $down (export "work") (result i32)
    (i32.add (call $down) (i32.const 1))))"#;
    let ran = run(&calling(&main, &[("lib", recurses)]), allowed, 20).await;
    assert!(
        matches!(ran.end, End::Trapped(Trap::StackOverflow, _)),
        "{} ({:?})",
        ran.end.describe(Limits::default()),
        ran.supplied
    );
    assert!(
        ran.supplied
            .as_deref()
            .is_some_and(|said| said.contains("inside a module it calls")),
        "{:?}",
        ran.supplied
    );
}

/// A called module's memory grows to the maximum its module declares and no further, and a
/// load outside it traps the run.
#[tokio::test]
async fn a_called_module_keeps_to_the_memory_it_declares() {
    let library = r#"(module
  (memory (export "memory") 1 2)
  (func (export "grow") (param i32) (result i32)
    (memory.grow (local.get 0)))
  (func (export "peek") (param i32) (result i32)
    (i32.load8_u (local.get 0))))"#;
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "grow" (func $grow (param i32) (result i32)))
  (import "lib" "peek" (func $peek (param i32) (result i32)))
  {MEMORY}
  (func (export "_start")
    ;; One page more is inside its maximum, and gives the size before: 1.
    (if (i32.ne (call $grow (i32.const 1)) (i32.const 1)) (then (call $exit (i32.const 1))))
    ;; A page past its maximum is refused.
    (if (i32.ne (call $grow (i32.const 1)) (i32.const -1)) (then (call $exit (i32.const 2))))
    (if (i32.ne (call $grow (i32.const 16000)) (i32.const -1)) (then (call $exit (i32.const 3))))
    ;; The last byte of its two pages is its own; the next one is outside.
    (drop (call $peek (i32.const 131071)))
    (call $exit (call $peek (i32.const 131072)))))"#
    );
    let line = calls_line("grow(i32) -> i32, peek(i32) -> i32");
    let allowed = policy(&["- Uses: nothing", &line]);
    let ran = run(&calling(&main, &[("lib", library)]), allowed, 20).await;
    assert!(
        matches!(ran.end, End::Trapped(Trap::OutOfBounds, _)),
        "{} ({:?})",
        ran.end.describe(Limits::default()),
        ran.supplied
    );
}

/// A copy is checked against both memories before a byte moves: bytes outside the called
/// module's memory, a module the idea doesn't call and a module with no memory give `INVAL`;
/// a buffer outside the program's own memory stops the run, as for every call of the host.
#[tokio::test]
async fn a_copy_stays_inside_both_memories() {
    const INVAL: i32 = 28;
    let bare = r#"(module (func (export "nothing")))"#;
    let line = "- Calls: lib.wasm, built from WebAssembly text, in a test; its functions: \
                buffer() -> i32, add(i32, i32) -> i32, upper(i32)";
    let other = "- Calls: bare.wasm, built from WebAssembly text, in a test; its functions: \
                 nothing()";
    let allowed = policy(&["- Uses: nothing", line, other]);
    // Each call, and what it must give: (way, module, at, own, len).
    let calls = [
        ("give", 0, 1024, 64, 4, 0),
        ("take", 0, 1024, 64, 4, 0),
        // The last four bytes of the module's one page, and one byte past them.
        ("give", 0, 65532, 64, 4, 0),
        ("give", 0, 65533, 64, 4, INVAL),
        ("take", 0, 65533, 64, 4, INVAL),
        ("give", 0, -1, 64, 4, INVAL),
        ("take", 0, 65536, 64, 1, INVAL),
        // Nothing at all, at the end of each memory.
        ("give", 0, 65536, 65536, 0, 0),
        // A module with no memory, and modules the idea doesn't call.
        ("give", 1, 0, 64, 0, INVAL),
        ("take", 1, 0, 64, 4, INVAL),
        ("give", 2, 1024, 64, 4, INVAL),
        ("take", -1, 1024, 64, 4, INVAL),
    ];
    for (way, module, at, own, len, wanted) in calls {
        let main = format!(
            r#"(module {EXIT} {GIVE} {TAKE}
  (import "bare" "nothing" (func $nothing))
  {MEMORY}
  (func (export "_start")
    (call $nothing)
    (call $exit
      (call ${way} (i32.const {module}) (i32.const {at}) (i32.const {own}) (i32.const {len})))))"#
        );
        let program = calling(&main, &[("lib", LIBRARY), ("bare", bare)]);
        let ran = run(&program, allowed, 20).await;
        assert_eq!(
            exited(&ran),
            wanted,
            "{way}({module}, {at}, {own}, {len}): {:?}",
            ran.supplied
        );
    }
    // The program's own side: outside its memory, the run stops.
    for (way, own, len) in [("give", 65533, 4), ("take", 65533, 4), ("give", -1, 1)] {
        let main = format!(
            r#"(module {EXIT} {GIVE} {TAKE}
  (import "bare" "nothing" (func $nothing))
  {MEMORY}
  (func (export "_start")
    (call $nothing)
    (call $exit
      (call ${way} (i32.const 0) (i32.const 1024) (i32.const {own}) (i32.const {len})))))"#
        );
        let program = calling(&main, &[("lib", LIBRARY), ("bare", bare)]);
        let ran = run(&program, allowed, 20).await;
        assert!(
            matches!(ran.end, End::Trapped(Trap::OutOfBounds, _)),
            "{way} from {own}: {} ({:?})",
            ran.end.describe(Limits::default()),
            ran.supplied
        );
    }
}

/// A program whose idea calls no module gets no copy: `give` and `take` answer `NOTCAPABLE`.
#[tokio::test]
async fn a_program_whose_idea_calls_nothing_copies_nothing() {
    const NOTCAPABLE: i32 = 76;
    for way in ["give", "take"] {
        let main = format!(
            r#"(module {EXIT} {GIVE} {TAKE}
  {MEMORY}
  (func (export "_start")
    (call $exit
      (call ${way} (i32.const 0) (i32.const 1024) (i32.const 64) (i32.const 4)))))"#
        );
        let ran = run(&calling(&main, &[]), policy(&["- Uses: nothing"]), 20).await;
        assert_eq!(exited(&ran), NOTCAPABLE, "{way}: {:?}", ran.supplied);
    }
}

/// A run starts only beside every module its idea calls, each with the functions the idea
/// names, of the types the idea gives them: without the module, without a function, or with a
/// function of another type, the program never starts, and the run says why.
#[tokio::test]
async fn a_run_starts_only_beside_the_modules_its_idea_names() {
    let main = format!(
        r#"(module {EXIT}
  (import "lib" "add" (func $add (param i32 i32) (result i32)))
  {MEMORY}
  (func (export "_start")
    (call $exit (call $add (i32.const 2) (i32.const 3)))))"#
    );
    let allowed = policy(&["- Uses: nothing", &calls_line("add(i32, i32) -> i32")]);
    let ran = run(&calling(&main, &[("lib", LIBRARY)]), allowed, 20).await;
    assert_eq!(exited(&ran), 5, "{:?}", ran.supplied);
    let wide = r#"(module
  (func (export "add") (param i64 i64) (result i32) (i32.const 5)))"#;
    let none = r#"(module (func (export "sum") (param i32 i32) (result i32) (i32.const 5)))"#;
    for (called, why) in [
        (vec![], "the run wasn't given it"),
        (vec![("other", LIBRARY)], "the run wasn't given it"),
        (
            vec![("lib", wide)],
            "doesn't have the type the idea gives it",
        ),
        (vec![("lib", none)], "it exports no function `add`"),
    ] {
        let ran = run(&calling(&main, &called), allowed, 20).await;
        assert_eq!(ran.end, End::Exited(127), "{why}: {:?}", ran.supplied);
        assert!(
            ran.supplied
                .as_deref()
                .is_some_and(|said| said.contains(why)),
            "{why}: {:?}",
            ran.supplied
        );
        assert!(ran.stdout.is_empty());
    }
}

/// The idea, the code and the Rust source of examples/checksum-rust, which calls crc32.wasm.
const CALLER_IDEA: &str = include_str!("../examples/checksum-rust/idea.md");
const CALLER_CODE: &str = include_str!("../examples/checksum-rust/main.wat");

/// examples/crc32-library, a lengua program that offers two functions, and
/// examples/checksum-library, which calls it.
const LIBRARY_IDEA: &str = include_str!("../examples/crc32-library/idea.md");
const LIBRARY_CODE: &str = include_str!("../examples/crc32-library/main.wat");
const LIBRARY_CALLER_IDEA: &str = include_str!("../examples/checksum-library/idea.md");
const LIBRARY_CALLER_CODE: &str = include_str!("../examples/checksum-library/main.wat");

/// The part all three build on.
const DECIMAL_IDEA: &str = include_str!("../examples/decimal/idea.md");
const DECIMAL_CODE: &str = include_str!("../examples/decimal/main.wat");

/// The words of the user's that the programs here are recorded with.
const REQUEST: &str = "Make me a program that prints the CRC-32 of its input";

/// A module with the two functions crc32.wasm has, in WebAssembly text, which stands in for the
/// compiler's output: its buffer at 2048, and the CRC-32 of the buffer's first bytes, xored
/// with `{flip}`, so that a text with another number is another module that computes another
/// CRC.
fn crc32_text(flip: u32) -> String {
    format!(
        r#"(module
  (memory (export "memory") 1 1)
  (func (export "buffer") (result i32) (i32.const 2048))
  (func (export "crc32") (param $length i32) (result i32)
    (local $i i32)
    (local $c i32)
    (local $bit i32)
    (local.set $c (i32.const -1))
    (block $done
      (loop $byte
        (br_if $done (i32.ge_u (local.get $i) (local.get $length)))
        (local.set $c
          (i32.xor (local.get $c) (i32.load8_u (i32.add (i32.const 2048) (local.get $i)))))
        (local.set $bit (i32.const 8))
        (loop $shift
          (local.set $c
            (i32.xor (i32.shr_u (local.get $c) (i32.const 1))
              (i32.and (i32.const 0xedb88320)
                (i32.sub (i32.const 0) (i32.and (local.get $c) (i32.const 1))))))
          (local.set $bit (i32.sub (local.get $bit) (i32.const 1)))
          (br_if $shift (local.get $bit)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $byte)))
    (i32.xor (i32.xor (local.get $c) (i32.const -1)) (i32.const {flip}))))"#
    )
}

/// The part decimal beside `program`, checked.
async fn with_decimal(session: &Session, program: &Program) {
    let decimal = program.beside("decimal");
    decimal.write("idea.md", DECIMAL_IDEA);
    session.record(&decimal).await;
    decimal.write("main.wat", DECIMAL_CODE);
    let reply = session.settle(&decimal).await;
    assert_eq!(reply["state"], "done", "{reply}");
}

/// A program folder with examples/checksum-rust's idea and main.wat, its part beside it,
/// checked, and `module` as crc32.wasm: the idea is recorded, and the code is yet to be
/// checked.
async fn caller(session: &Session, module: &[u8]) -> Program {
    let program = Program::new();
    with_decimal(session, &program).await;
    program.write("idea.md", CALLER_IDEA);
    std::fs::write(program.file("crc32.wasm"), module).expect("write crc32.wasm");
    let recorded = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    program.write("main.wat", CALLER_CODE);
    program
}

/// `run`'s arguments for `program` with `stdin` as its standard input and `more` beside it.
fn with_stdin(program: &Program, stdin: &str, more: &Value) -> Value {
    let mut arguments = program.arguments();
    arguments["stdin"] = json!(stdin);
    if let Some(more) = more.as_object() {
        for (name, value) in more {
            arguments[name.as_str()] = value.clone();
        }
    }
    arguments
}

/// `lengua-mcp` or lengua-run, `program`, run from a shell with the session's data folder, then
/// `words`, with `stdin` as its standard input.
fn from_shell(program: &Path, session: &Session, words: &[&OsStr], stdin: &[u8]) -> Output {
    let mut child = Command::new(program)
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .args(words)
        .env_remove("LENGUA_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("it starts");
    // A run that lengua refuses ends without reading its input: a write to it may then fail.
    let mut input = child.stdin.take().expect("its input");
    let _ = input.write_all(stdin);
    drop(input);
    child.wait_with_output().expect("it runs")
}

/// The CRC-32 of `123456789`, as every implementation gives it.
const CHECK_INPUT: &str = "123456789";
const CHECK_CRC: &str = "3421780262\n";

/// A program that calls an outside module passes its tests with the module beside it, and
/// every place that names it says what is trusted: `check`'s reply has a `calls` line that
/// starts with the module and "trusted, not proven", its next step says to tell the user, and
/// lengua writes no entry in idea.md's `verified`; the lock records the module's hash; `run`,
/// `lengua-mcp run` and lengua-run run the two together and say the trust before the program
/// starts; and the attester doesn't attest the folder.
#[tokio::test]
async fn a_program_that_calls_an_outside_module_says_what_is_trusted() {
    let session = Session::start(Protocol::Discover).await;
    let module = wasm(&crc32_text(0));
    let program = caller(&session, &module).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let calls = strings(&reply["calls"]);
    assert_eq!(calls.len(), 1, "{reply}");
    assert!(
        calls[0].starts_with("crc32.wasm: trusted, not proven: an outside module"),
        "{reply}"
    );
    for words in [
        "built from Rust (crc32.rs) with rustc for wasm32-wasip1",
        "buffer() -> i32, crc32(i32) -> i32",
        "it imports nothing and gets no function of the host",
        "Not held: what its functions give back",
    ] {
        assert!(calls[0].contains(words), "{words}: {reply}");
    }
    let next = reply["next_step"].as_str().unwrap_or_default();
    assert!(
        next.contains(
            "The program calls crc32.wasm, an outside module that is trusted, not proven"
        ),
        "{reply}"
    );
    assert!(!next.contains("verified list now holds"), "{reply}");
    assert!(reply.get("outside").is_none(), "{reply}");
    // idea.md is as it was: lengua wrote no entry in its `verified`.
    assert_eq!(program.read("idea.md").as_deref(), Some(CALLER_IDEA));
    let lock = program.read("lengua.lock").expect("the lock");
    let hash = lengua::hash::Sha256Hex::of(&module);
    assert!(
        lock.contains(&format!(
            "calls = [{{ call = \"crc32.wasm\", module = \"{hash}\", checked = false }}]"
        )),
        "{lock}"
    );
    // check kept the called module compiled for both kinds of run, under the user's seal.
    for setup in [
        lengua::wasm::engine::Setup::Run,
        lengua::wasm::engine::Setup::Shell,
    ] {
        let kept = lengua::state::compiled_for(
            session.data_dir.path(),
            lengua::wasm::text::BUILDER,
            &hash,
            setup,
        );
        assert!(kept.is_file(), "check kept no {}", kept.display());
    }

    let ran = session
        .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert_eq!(ran["stdout"], CHECK_CRC, "{ran}");
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert!(
        strings(&ran["may_reach"])
            .iter()
            .any(|line| line.contains("crc32.wasm, an outside module that is trusted, not proven")),
        "{ran}"
    );
    // The `run` tool's worker loads the called module's compiled form only under the user's
    // seal: with other bytes in its place, the run compiles the module again, gives the same
    // CRC and keeps what it compiled.
    let kept = lengua::state::compiled_for(
        session.data_dir.path(),
        lengua::wasm::text::BUILDER,
        &hash,
        lengua::wasm::engine::Setup::Run,
    );
    let planted = b"not a compiled module".to_vec();
    std::fs::write(&kept, &planted).expect("plant it");
    let again = session
        .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert_eq!(again["stdout"], CHECK_CRC, "{again}");
    let now = std::fs::read(&kept).unwrap_or_default();
    assert!(
        now != planted && now.len() > planted.len(),
        "the run didn't keep the called module it compiled"
    );
    let folder = program.path().as_os_str();
    let shell = from_shell(
        &launcher(),
        &session,
        &[OsStr::new("run"), folder],
        CHECK_INPUT.as_bytes(),
    );
    assert_eq!(shell.stdout, CHECK_CRC.as_bytes(), "{shell:?}");
    assert_eq!(shell.status.code(), Some(0), "{shell:?}");
    let before = String::from_utf8_lossy(&shell.stderr).into_owned();
    assert!(
        before.contains(
            "lengua-mcp: it calls crc32.wasm, an outside module that is trusted, not proven"
        ),
        "{before}"
    );
    if let Some(alone) = lengua_run() {
        let ran = from_shell(&alone, &session, &[folder], CHECK_INPUT.as_bytes());
        assert_eq!(ran.stdout, CHECK_CRC.as_bytes(), "{ran:?}");
        assert_eq!(ran.status.code(), Some(0), "{ran:?}");
        let before = String::from_utf8_lossy(&ran.stderr).into_owned();
        assert!(
            before.contains("lengua-run: it calls crc32.wasm, an outside module that is trusted"),
            "{before}"
        );
        // lengua-run loaded both modules in their compiled forms: it handed nothing over.
        assert!(!before.contains("lengua-mcp compiles it"), "{before}");
    }
    let attested = Command::new(launcher())
        .arg("check")
        .arg(program.path())
        .env_remove("LENGUA_LOG")
        .output()
        .expect("the attester runs");
    let said = String::from_utf8_lossy(&attested.stdout).into_owned()
        + &String::from_utf8_lossy(&attested.stderr);
    assert!(!attested.status.success(), "{said}");
    assert!(said.contains("main.wat calls a module"), "{said}");
    assert!(said.contains("not attested"), "{said}");
    session.service.cancel().await.expect("close the session");
}

/// What depends on a call is tested, not proven, and the rest is left to its own proof: of the
/// program's generated tests, each one that reaches the call says it is tested, not proven, and
/// names the call; the one that ends before any call, on input that is too long, doesn't.
#[tokio::test]
async fn a_proof_stops_at_a_call_and_holds_before_it() {
    let session = Session::proving_for(Protocol::Discover, 240).await;
    let program = caller(&session, &wasm(&crc32_text(0))).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let proofs = strings(&reply["proofs"]);
    let of = |id: &str| {
        proofs
            .iter()
            .find(|line| line.starts_with(&format!("generated {id}:")))
            .unwrap_or_else(|| panic!("no proof line for {id}: {reply}"))
            .clone()
    };
    for id in ["B1", "B4"] {
        let line = of(id);
        assert!(line.contains("not proven"), "{line}");
        assert!(line.contains("a module it calls"), "{line}");
    }
    // Its runs end before any call, so no call stands in its proof's way: it is proven, or
    // it says another reason, such as its time (the library's own test of the model pins that
    // a run before a call is followed to its end).
    let before = of("B3");
    assert!(!before.contains("a module it calls"), "{before}");
    session.service.cancel().await.expect("close the session");
}

/// A run holds each called module to the hash the lock records: with another module in its
/// place, or none, `run` and a run from a shell refuse the program, and nothing of either
/// module runs; `check` then tests the program with the module that is there.
#[tokio::test]
async fn a_called_module_changed_since_check_does_not_run() {
    let session = Session::start(Protocol::Discover).await;
    let program = caller(&session, &wasm(&crc32_text(0))).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let ran = session
        .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert_eq!(ran["stdout"], CHECK_CRC, "{ran}");
    // Another module with the same functions, which computes another number.
    std::fs::write(program.file("crc32.wasm"), wasm(&crc32_text(1))).expect("swap crc32.wasm");
    let refused = session
        .call_error("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert!(
        refused.contains("crc32.wasm, which isn't the module its tests ran with"),
        "{refused}"
    );
    let folder = program.path().as_os_str();
    let mut shells = vec![(launcher(), vec![OsStr::new("run"), folder])];
    if let Some(alone) = lengua_run() {
        shells.push((alone, vec![folder]));
    }
    for (shell, words) in &shells {
        let ran = from_shell(shell, &session, words, CHECK_INPUT.as_bytes());
        assert_eq!(ran.status.code(), Some(125), "{ran:?}");
        assert!(ran.stdout.is_empty(), "{ran:?}");
        let said = String::from_utf8_lossy(&ran.stderr).into_owned();
        assert!(
            said.contains("isn't the module its tests ran with"),
            "{said}"
        );
    }
    // check tests the program with the module that is there now, which gives another CRC.
    let failed = session.settle(&program).await;
    assert_eq!(failed["state"], "tests_failed", "{failed}");
    // With no module at all, a run refuses too, and check says what to build.
    std::fs::remove_file(program.file("crc32.wasm")).expect("remove crc32.wasm");
    let refused = session
        .call_error("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert!(refused.contains("won't run"), "{refused}");
    let missing = session.check(&program).await;
    assert_eq!(missing["state"], "code_problems", "{missing}");
    let problems = strings(&missing["problems"]).join("\n");
    assert!(problems.contains("holds no crc32.wasm"), "{missing}");
    session.service.cancel().await.expect("close the session");
}

/// `check` reads a called module before anything of it is compiled or run, and refuses what
/// lengua doesn't take: any import, a memory with no maximum or one that isn't exported, a
/// function the idea names that it doesn't export, or exports with another type, and what
/// isn't a module of the engine's features.
#[tokio::test]
async fn check_refuses_a_called_module_lengua_does_not_take() {
    let session = Session::start(Protocol::Discover).await;
    let functions = r#"(func (export "buffer") (result i32) (i32.const 2048))
  (func (export "crc32") (param i32) (result i32) (i32.const 0))"#;
    let cases = [
        (
            format!(
                r#"(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1 1) {functions})"#
            ),
            "a module that a program calls imports nothing",
        ),
        (
            format!(
                r#"(module
  (import "env" "now" (func (result i64)))
  (memory (export "memory") 1 1) {functions})"#
            ),
            "crc32.wasm imports env.now",
        ),
        (
            format!(r#"(module (memory (export "memory") 1) {functions})"#),
            "memory declares no maximum",
        ),
        (
            format!(r#"(module (memory 1 1) {functions})"#),
            "doesn't export its memory as `memory`",
        ),
        (
            format!(r#"(module (memory (export "memory") 1 65536) {functions})"#),
            "may grow to 65536 pages",
        ),
        (
            r#"(module (memory (export "memory") 1 1)
  (func (export "buffer") (result i32) (i32.const 2048)))"#
                .to_owned(),
            "exports no function `crc32`",
        ),
        (
            r#"(module (memory (export "memory") 1 1)
  (func (export "buffer") (result i32) (i32.const 2048))
  (func (export "crc32") (param i64) (result i32) (i32.const 0)))"#
                .to_owned(),
            "exports `crc32` as (i64) -> (i32)",
        ),
    ];
    let program = caller(&session, &wasm(&crc32_text(0))).await;
    for (text, why) in &cases {
        std::fs::write(program.file("crc32.wasm"), wasm(text)).expect("write crc32.wasm");
        let reply = session.check(&program).await;
        assert_eq!(reply["state"], "code_problems", "{why}: {reply}");
        let problems = strings(&reply["problems"]).join("\n");
        assert!(problems.contains(why), "{why}: {reply}");
        assert!(
            !program.file("lengua.lock").is_file() || {
                let lock = program.read("lengua.lock").unwrap_or_default();
                !lock.contains("\n[code]")
            }
        );
    }
    // Bytes that are no module, and a module with a feature the engine has off.
    std::fs::write(program.file("crc32.wasm"), b"not a module").expect("write crc32.wasm");
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    assert!(
        strings(&reply["problems"])
            .join("\n")
            .contains("isn't a WebAssembly module lengua's engine takes"),
        "{reply}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The rules hold main.wat's imports to the idea's `- Calls:` line: a function the line doesn't
/// name, or one imported with another type, is a problem of the code.
#[tokio::test]
async fn main_wat_calls_only_what_its_idea_names() {
    let session = Session::start(Protocol::Discover).await;
    let program = caller(&session, &wasm(&crc32_text(0))).await;
    let unnamed = CALLER_CODE.replace(
        r#"(import "crc32" "buffer" (func $buffer (result i32)))"#,
        r#"(import "crc32" "text" (func $buffer (result i32)))"#,
    );
    assert_ne!(unnamed, CALLER_CODE);
    program.write("main.wat", &unnamed);
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    let problems = strings(&reply["problems"]).join("\n");
    assert!(
        problems.contains("the idea's `- Calls:` line names no function `text` of it"),
        "{reply}"
    );
    let retyped = CALLER_CODE
        .replace(
            r#"(import "crc32" "buffer" (func $buffer (result i32)))"#,
            r#"(import "crc32" "buffer" (func $buffer (result i64)))"#,
        )
        .replace("(call $buffer)", "(i32.wrap_i64 (call $buffer))");
    program.write("main.wat", &retyped);
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    let problems = strings(&reply["problems"]).join("\n");
    assert!(
        problems.contains("the idea's `- Calls:` line says `buffer() -> i32`"),
        "{reply}"
    );
    // A module the idea doesn't call at all is no module a program imports from.
    let elsewhere = CALLER_CODE.replace(
        r#"(import "crc32" "buffer" (func $buffer (result i32)))"#,
        r#"(import "other" "buffer" (func $buffer (result i32)))"#,
    );
    program.write("main.wat", &elsewhere);
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    let problems = strings(&reply["problems"]).join("\n");
    assert!(problems.contains("is imported from `other`"), "{reply}");
    session.service.cancel().await.expect("close the session");
}

/// What an idea can't say beside `- Calls:` or `- Exports:`: a secret, which a call would carry
/// out of what the secret rule follows; work split into pieces, for a program that calls; both
/// lines at once; and either for an outside module. And a line that doesn't read is refused
/// with how to write it.
#[test]
fn an_idea_that_calls_names_no_secret_and_does_not_split() {
    let problems = |idea: &str| -> String {
        match lengua::idea::parse(idea) {
            Ok(_) => String::new(),
            Err(problems) => problems
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    };
    assert_eq!(problems(CALLER_IDEA), "");
    assert_eq!(problems(LIBRARY_IDEA), "");
    assert_eq!(problems(LIBRARY_CALLER_IDEA), "");
    let uses = "- Uses: standard input";
    let with = |idea: &str, line: &str| {
        let changed = idea.replace(uses, &format!("{uses}\n{line}"));
        assert_ne!(changed, idea);
        changed
    };
    let secret = "- Secret: standard input";
    let splits = "- Splits: its work into up to 8 pieces, each of up to 1024 bytes, each gives \
                  back 32 bytes";
    assert!(
        problems(&with(CALLER_IDEA, secret))
            .contains("a program that calls a module names no secret"),
        "{}",
        problems(&with(CALLER_IDEA, secret))
    );
    assert!(
        problems(&with(CALLER_IDEA, splits))
            .contains("a program that calls a module doesn't split its work"),
        "{}",
        problems(&with(CALLER_IDEA, splits))
    );
    assert!(
        problems(&with(LIBRARY_IDEA, secret))
            .contains("a program that offers functions to other programs names no secret"),
        "{}",
        problems(&with(LIBRARY_IDEA, secret))
    );
    let both = with(
        LIBRARY_IDEA,
        "- Calls: other.wasm, built from C; its functions: f()",
    );
    assert!(
        problems(&both)
            .contains("a program that offers functions to other programs calls no module"),
        "{}",
        problems(&both)
    );
    let twice = with(
        CALLER_IDEA,
        "- Calls: crc32.wasm, built from C; its functions: other()",
    );
    assert!(
        problems(&twice).contains("the module `crc32` is called on two lines"),
        "{}",
        problems(&twice)
    );
    let unread = with(LIBRARY_IDEA, "- Calls: other.wasm; its functions: f()");
    assert!(
        problems(&unread).contains("built from"),
        "{}",
        problems(&unread)
    );
    // A run reads the same lines: the grants of the idea hold its calls and its exports.
    let (_, network) = lengua::idea::grants(CALLER_IDEA).expect("its grants read");
    assert_eq!(network.reach.calls.len(), 1);
    assert_eq!(network.reach.calls[0].name, "crc32");
    assert!(!network.reach.calls[0].checked());
    let (_, network) = lengua::idea::grants(LIBRARY_IDEA).expect("its grants read");
    assert_eq!(network.reach.exports.len(), 2);
}

/// A program folder beside `program` with examples/crc32-library's idea and `code`, checked
/// to done.
async fn library_beside(session: &Session, program: &Program, code: &str) -> Program {
    let library = program.beside("crc32-library");
    library.write("idea.md", LIBRARY_IDEA);
    session.call_ok("check", library.request(REQUEST)).await;
    library.write("main.wat", code);
    let reply = session.settle(&library).await;
    assert_eq!(reply["state"], "done", "{reply}");
    library
}

/// A program folder with examples/checksum-library's idea and main.wat, its part and the
/// library it calls beside it, each checked: the program itself is yet to be checked.
async fn library_caller(session: &Session) -> (Program, Program) {
    let program = Program::new();
    with_decimal(session, &program).await;
    let library = library_beside(session, &program, LIBRARY_CODE).await;
    program.write("idea.md", LIBRARY_CALLER_IDEA);
    let recorded = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    program.write("main.wat", LIBRARY_CALLER_CODE);
    (program, library)
}

/// A lengua program is a library: its idea offers functions with `- Exports:`, another program
/// calls them, and every place says "checked", not "trusted": `check`'s `calls` line, with what
/// the library's own lock records of its proofs; the lock; and each run, by the `run` tool,
/// `lengua-mcp run` and lengua-run, which starts the library's module beside the program.
#[tokio::test]
async fn a_program_calls_a_lengua_program_that_lengua_checked() {
    let session = Session::start(Protocol::Discover).await;
    let (program, library) = library_caller(&session).await;
    // The library alone is a program like any other: done, with lengua's entry.
    let alone = session
        .call_ok("run", with_stdin(&library, CHECK_INPUT, &json!({})))
        .await;
    assert_eq!(alone["stdout"], CHECK_CRC, "{alone}");
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let calls = strings(&reply["calls"]);
    assert_eq!(calls.len(), 1, "{reply}");
    assert!(
        calls[0]
            .starts_with("[crc32-library](../crc32-library/idea.md): checked: a lengua program"),
        "{reply}"
    );
    assert!(calls[0].contains("its lock verified"), "{reply}");
    assert!(!calls[0].contains("trusted, not proven"), "{reply}");
    let next = reply["next_step"].as_str().unwrap_or_default();
    assert!(!next.contains("trusted, not proven"), "{reply}");
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("calls = [{ call = \"../crc32-library\", module = \""),
        "{lock}"
    );
    assert!(lock.contains(", checked = true }]"), "{lock}");
    let ran = session
        .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
        .await;
    assert_eq!(ran["stdout"], CHECK_CRC, "{ran}");
    assert!(
        strings(&ran["may_reach"])
            .iter()
            .any(|line| line.contains("it calls the lengua program ../crc32-library")),
        "{ran}"
    );
    let folder = program.path().as_os_str();
    let mut shells = vec![(launcher(), vec![OsStr::new("run"), folder])];
    if let Some(alone) = lengua_run() {
        shells.push((alone, vec![folder]));
    }
    for (shell, words) in &shells {
        let ran = from_shell(shell, &session, words, CHECK_INPUT.as_bytes());
        assert_eq!(ran.stdout, CHECK_CRC.as_bytes(), "{ran:?}");
        assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    }
    session.service.cancel().await.expect("close the session");
}

/// A called program's lock is verified before its module is loaded, at every run: once its
/// main.wat is no longer what its lock records, its lock is damaged, or its lock is gone, the
/// program that calls it doesn't run, by the `run` tool or from a shell; and a library that
/// was checked again with other code is another module than the caller's tests ran with.
#[tokio::test]
async fn a_called_program_is_verified_before_it_is_loaded() {
    let session = Session::start(Protocol::Discover).await;
    let (program, library) = library_caller(&session).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let folder = program.path().as_os_str();
    let refused = |why: &'static str| {
        let session = &session;
        let program = &program;
        async move {
            let said = session
                .call_error("run", with_stdin(program, CHECK_INPUT, &json!({})))
                .await;
            assert!(said.contains("won't run"), "{why}: {said}");
            let ran = from_shell(
                &launcher(),
                session,
                &[OsStr::new("run"), folder],
                CHECK_INPUT.as_bytes(),
            );
            assert_eq!(ran.status.code(), Some(125), "{why}: {ran:?}");
            assert!(ran.stdout.is_empty(), "{why}: {ran:?}");
            said
        }
    };
    let runs = || async {
        let ran = session
            .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
            .await;
        assert_eq!(ran["stdout"], CHECK_CRC, "{ran}");
    };
    runs().await;
    let lock = library.read("lengua.lock").expect("the library's lock");
    // Its code changed since its check.
    library.write("main.wat", &format!("{LIBRARY_CODE}\n;; changed\n"));
    let said = refused("its main.wat changed").await;
    assert!(said.contains("isn't checked on this computer"), "{said}");
    library.write("main.wat", LIBRARY_CODE);
    runs().await;
    // Its lock is damaged: one hex digit of a hash is another.
    let damaged = if lock.contains("sha256 = \"0") {
        lock.replacen("sha256 = \"0", "sha256 = \"1", 1)
    } else {
        let at = lock.find("sha256 = \"").expect("a hash") + "sha256 = \"".len();
        let mut bytes = lock.clone().into_bytes();
        bytes[at] = b'0';
        String::from_utf8(bytes).expect("a lock is text")
    };
    assert_ne!(damaged, lock);
    library.write("lengua.lock", &damaged);
    refused("its lock is damaged").await;
    // Its lock is gone.
    std::fs::remove_file(library.file("lengua.lock")).expect("remove the lock");
    refused("its lock is gone").await;
    library.write("lengua.lock", &lock);
    runs().await;
    // The library checked again with other code: its tests pass, and it is another module.
    let other = LIBRARY_CODE.replace(
        "(local.set $c (global.get $CRC32_ALL_ONES))",
        "(local.set $c (i32.xor (global.get $CRC32_ALL_ONES) (i32.const 0)))",
    );
    assert_ne!(other, LIBRARY_CODE);
    library.write("main.wat", &other);
    let again = session.settle(&library).await;
    assert_eq!(again["state"], "done", "{again}");
    let said = refused("the library is another module").await;
    assert!(
        said.contains("isn't the module its tests ran with"),
        "{said}"
    );
    // check of the caller tests it with the library as it is now.
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    runs().await;
    session.service.cancel().await.expect("close the session");
}

/// A program calls only a lengua program that is checked here and offers the functions: a
/// library that isn't checked, one whose idea offers no such function, and an outside module's
/// idea named as a program are problems of the caller's code, each saying what to do.
#[tokio::test]
async fn a_called_program_is_checked_and_offers_the_functions() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    with_decimal(&session, &program).await;
    program.write("idea.md", LIBRARY_CALLER_IDEA);
    let recorded = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    program.write("main.wat", LIBRARY_CALLER_CODE);
    // No library beside it yet.
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    assert!(
        strings(&reply["problems"])
            .join("\n")
            .contains("isn't checked on this computer"),
        "{reply}"
    );
    // The library there, with its idea recorded and its code not yet checked.
    let library = program.beside("crc32-library");
    library.write("idea.md", LIBRARY_IDEA);
    session.call_ok("check", library.request(REQUEST)).await;
    let reply = session.check(&program).await;
    assert_eq!(reply["state"], "code_problems", "{reply}");
    assert!(
        strings(&reply["problems"])
            .join("\n")
            .contains("isn't checked on this computer"),
        "{reply}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The rules hold main.wat to its idea's `- Exports:` line: a function the line lists that
/// main.wat doesn't export, an export the line doesn't list, an export of another type, and an
/// exported function that takes an address, which a caller's number can't be, are problems of
/// the code.
#[tokio::test]
async fn main_wat_exports_what_its_idea_offers_and_takes_no_address() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::new();
    with_decimal(&session, &program).await;
    program.write("idea.md", LIBRARY_IDEA);
    let recorded = session.call_ok("check", program.request(REQUEST)).await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    let problems_of = |code: String| {
        let session = &session;
        let program = &program;
        async move {
            assert_ne!(code, LIBRARY_CODE);
            program.write("main.wat", &code);
            let reply = session.check(program).await;
            assert_eq!(reply["state"], "code_problems", "{reply}");
            strings(&reply["problems"]).join("\n")
        }
    };
    let unexported = LIBRARY_CODE.replace(
        r#"(func $buffer (export "buffer") (result i32)"#,
        "(func $buffer (result i32)",
    );
    let problems = problems_of(unexported).await;
    assert!(
        problems.contains("main.wat exports no function `buffer`"),
        "{problems}"
    );
    let unlisted = LIBRARY_CODE.replace(
        r#"(func $buffer (export "buffer") (result i32)"#,
        r#"(func $buffer (export "buffer") (export "text") (result i32)"#,
    );
    let problems = problems_of(unlisted).await;
    assert!(problems.contains("main.wat exports `text`"), "{problems}");
    // An exported function that reads through a number its caller passes. Its own `_start`
    // doesn't call it here, so no caller of this module lends it a region: the only callers
    // are other programs.
    let lent = LIBRARY_CODE
        .replace(
            "(i32.load8_u (i32.add (global.get $text) (local.get $i)))",
            "(i32.load8_u (i32.add (local.get $length) (local.get $i)))",
        )
        .replace(
            "(local.set $crc (call $crc32 (local.get $got)))",
            "(local.set $crc (local.get $got))",
        );
    let problems = problems_of(lent).await;
    assert!(
        problems.contains("$crc32 uses its parameter $length as an address"),
        "{problems}"
    );
    // The library as it is goes to done, and offers its functions.
    program.write("main.wat", LIBRARY_CODE);
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    session.service.cancel().await.expect("close the session");
}

/// Each tool of a run works with a program that calls a module: a trace has the copy the
/// program made to the module, with the module's number first, and says that a call of the
/// module's function is no call of the host; a profile samples the run; and a stop reads
/// main.wat's values as for any program, with the called module beside the copy.
#[tokio::test]
async fn the_tools_of_a_run_work_with_a_program_that_calls() {
    let session = Session::start(Protocol::Handshake).await;
    let program = caller(&session, &wasm(&crc32_text(0))).await;
    let reply = session.settle(&program).await;
    assert_eq!(reply["state"], "done", "{reply}");
    let traced = session
        .call_ok(
            "run",
            with_stdin(&program, CHECK_INPUT, &json!({ "trace": true })),
        )
        .await;
    assert_eq!(traced["stdout"], CHECK_CRC, "{traced}");
    let calls = strings(&traced["host_calls"]["calls"]);
    assert!(
        calls
            .iter()
            .any(|call| call == "lengua_call_1.give(0, 2048, 2048, 9) = 0"),
        "{traced}"
    );
    assert!(
        calls.iter().all(|call| !call.starts_with("crc32.")),
        "{traced}"
    );
    let note = traced["host_calls"]["note"].as_str().unwrap_or_default();
    assert!(
        note.contains("A call of a function of a module the program calls is no call of the host"),
        "{traced}"
    );
    let profiled = session
        .call_ok(
            "run",
            with_stdin(&program, CHECK_INPUT, &json!({ "profile": true })),
        )
        .await;
    assert_eq!(profiled["stdout"], CHECK_CRC, "{profiled}");
    let note = profiled["profile"]["note"].as_str().unwrap_or_default();
    assert!(
        note.contains("A sample inside a module the program calls counts for"),
        "{profiled}"
    );
    let stopped = session
        .call_ok(
            "run",
            with_stdin(
                &program,
                CHECK_INPUT,
                &json!({ "stop_at": "$decimal_format" }),
            ),
        )
        .await;
    assert_eq!(stopped["stopped"]["reached"], true, "{stopped}");
    // The stop is after the call: the value it shows is the CRC the module gave.
    let locals = strings(&stopped["stopped"]["locals"]).join("\n");
    assert!(locals.contains("3421780262"), "{stopped}");
    // From a shell: a trace, and a stop, whose copy runs beside the called module too.
    let folder = program.path().as_os_str();
    let ran = from_shell(
        &launcher(),
        &session,
        &[OsStr::new("run"), folder, OsStr::new("--trace")],
        CHECK_INPUT.as_bytes(),
    );
    assert_eq!(ran.stdout, CHECK_CRC.as_bytes(), "{ran:?}");
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    assert!(
        said.contains("lengua-mcp: trace: lengua_call_1.give(0, 2048, 2048, 9) = 0"),
        "{said}"
    );
    let ran = from_shell(
        &launcher(),
        &session,
        &[
            OsStr::new("run"),
            folder,
            OsStr::new("--stop-at"),
            OsStr::new("$decimal_format"),
        ],
        CHECK_INPUT.as_bytes(),
    );
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    assert!(said.contains("3421780262"), "{ran:?}");
    let ran = from_shell(
        &launcher(),
        &session,
        &[OsStr::new("run"), folder, OsStr::new("--profile")],
        CHECK_INPUT.as_bytes(),
    );
    assert_eq!(ran.stdout, CHECK_CRC.as_bytes(), "{ran:?}");
    session.service.cancel().await.expect("close the session");
}

/// Real compiler output: the Rust library of examples/checksum-rust, built by rustc as a
/// `cdylib` with no standard library, and a C library with the same two functions, built by
/// wasi-sdk's clang with no standard library and no entry. `check` takes each as it is, with no
/// import, runs the program's tests with it, and says trusted, not proven; `run` and a run from
/// a shell give the CRC-32 that every implementation gives.
#[tokio::test]
async fn a_program_calls_a_library_that_a_compiler_made() {
    for (name, built_from) in [
        ("checksum-rust.crc32", None),
        ("crc32", Some("C (tests/outside/crc32/lib.c) with clang")),
    ] {
        let Some(module) = common::compiled(name) else {
            return;
        };
        let session = Session::start(Protocol::Discover).await;
        let program = Program::new();
        with_decimal(&session, &program).await;
        let idea = match built_from {
            None => CALLER_IDEA.to_owned(),
            Some(words) => {
                let idea = CALLER_IDEA.replace(
                    "built from Rust (crc32.rs) with rustc for wasm32-wasip1",
                    &format!("built from {words}"),
                );
                assert_ne!(idea, CALLER_IDEA);
                idea
            }
        };
        program.write("idea.md", &idea);
        std::fs::write(program.file("crc32.wasm"), &module).expect("write crc32.wasm");
        let recorded = session.call_ok("check", program.request(REQUEST)).await;
        assert_eq!(recorded["state"], "write_code", "{name}: {recorded}");
        program.write("main.wat", CALLER_CODE);
        let reply = session.settle(&program).await;
        assert_eq!(reply["state"], "done", "{name}: {reply}");
        let calls = strings(&reply["calls"]);
        assert!(
            calls
                .first()
                .is_some_and(|line| line.starts_with("crc32.wasm: trusted, not proven")),
            "{name}: {reply}"
        );
        let ran = session
            .call_ok("run", with_stdin(&program, CHECK_INPUT, &json!({})))
            .await;
        assert_eq!(ran["stdout"], CHECK_CRC, "{name}: {ran}");
        let shell = from_shell(
            &launcher(),
            &session,
            &[OsStr::new("run"), program.path().as_os_str()],
            b"The quick brown fox jumps over the lazy dog",
        );
        assert_eq!(shell.stdout, b"1095738169\n", "{name}: {shell:?}");
        session.service.cancel().await.expect("close the session");
    }
}
