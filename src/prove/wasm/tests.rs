//! The WebAssembly model on its own: WebAssembly's integer, float and SIMD128 semantics, each trap
//! at its line of main.wat, lengua's library, the host's calls, what the model refuses to run, a
//! branch, a division, a load and counts of bits the input decides, a run with nothing to decide
//! that still ends with the proof's time and the computer's memory, `$fill` read as a whole,
//! where a read loop is, and what an induction's step can't take there: a read before the loop's
//! head, and a time round that changes the working folder. Then, with the solvers, the echo and
//! greet fixtures proven for every input, add for numbers of up to three digits (for every input
//! in a slow ignored test), and a planted bug in each found, the traps naming their lines;
//! cat-stream, the stream template, and lines in SIMD128 proven for every length by induction, a
//! planted bug in each found, and the bytes the two programs the step can't take drop found;
//! count in SIMD128 proven by length, as the flag it asserts is any value at the step's head; cat
//! for every length its idea allows; the hypotenuse in doubles and the vector template.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::expr::{Generator, Value};
use crate::prove::exec::Oracle;
use crate::prove::exec::induction::Held;
use crate::prove::{Budget, UNSAFE, Verdict, prove_wasm};
use crate::testfile::Generated;
use crate::wasm::Policy;
use crate::wasm::text::build;

/// The module of `text`, whose idea lets it reach what `policy` allows.
fn module(text: &str, policy: Policy) -> Module {
    let built = build(text).unwrap_or_else(|problem| panic!("{problem}: {text}"));
    Module::new(&built, policy).unwrap_or_else(|why| panic!("{why}: {text}"))
}

/// What `text` does in the model on `given`.
fn ran(text: &str, policy: Policy, given: &Given) -> Ran {
    run(&module(text, policy), given).unwrap_or_else(|why| panic!("{why}: {text}"))
}

/// A program that exits with the i32 `body` leaves, after `fields` of its own.
fn exits_with(fields: &str, body: &str) -> String {
    format!(
        "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit (param i32)))\n  \
         (memory (export \"memory\") 1 1)\n  {fields}\n  (func (export \"_start\") (local $i i32) \
         (local $s i32)\n    (call $exit {body})))\n"
    )
}

/// Checks of WebAssembly's semantics, each the fields it needs and an i32 that is 1 when the
/// model gets it right: division toward zero, shift counts modulo the width, rotations, counts of
/// bits, extensions and wraps, little-endian memory of every width, the data segments, bulk
/// memory, globals, calls, multiple values, and every kind of branch.
const CHECKS: &[(&str, &str)] = &[
    (
        "",
        "(i32.eq (i32.div_s (i32.const -7) (i32.const 2)) (i32.const -3))",
    ),
    (
        "",
        "(i32.eq (i32.rem_s (i32.const -7) (i32.const 2)) (i32.const -1))",
    ),
    (
        "",
        "(i32.eq (i32.div_u (i32.const -1) (i32.const 2)) (i32.const 2147483647))",
    ),
    (
        "",
        "(i32.eq (i32.rem_u (i32.const -1) (i32.const 10)) (i32.const 5))",
    ),
    (
        "",
        "(i32.eq (i32.shl (i32.const 1) (i32.const 33)) (i32.const 2))",
    ),
    (
        "",
        "(i32.eq (i32.shr_s (i32.const -8) (i32.const 1)) (i32.const -4))",
    ),
    (
        "",
        "(i32.eq (i32.shr_u (i32.const -8) (i32.const 1)) (i32.const 2147483644))",
    ),
    (
        "",
        "(i32.eq (i32.rotl (i32.const 0x80000001) (i32.const 1)) (i32.const 3))",
    ),
    (
        "",
        "(i32.eq (i32.rotr (i32.const 3) (i32.const 33)) (i32.const 0x80000001))",
    ),
    ("", "(i32.eq (i32.clz (i32.const 1)) (i32.const 31))"),
    ("", "(i32.eq (i32.clz (i32.const 0)) (i32.const 32))"),
    ("", "(i32.eq (i32.ctz (i32.const 8)) (i32.const 3))"),
    ("", "(i32.eq (i32.ctz (i32.const 0)) (i32.const 32))"),
    (
        "",
        "(i32.eq (i32.popcnt (i32.const 0xff00ff)) (i32.const 16))",
    ),
    (
        "",
        "(i32.eq (i32.extend8_s (i32.const 0x80)) (i32.const -128))",
    ),
    (
        "",
        "(i32.eq (i32.extend16_s (i32.const 0x8000)) (i32.const -32768))",
    ),
    (
        "",
        "(i64.eq (i64.extend_i32_s (i32.const -1)) (i64.const -1))",
    ),
    (
        "",
        "(i64.eq (i64.extend_i32_u (i32.const -1)) (i64.const 4294967295))",
    ),
    (
        "",
        "(i64.eq (i64.extend32_s (i64.const 0x80000000)) (i64.const -2147483648))",
    ),
    (
        "",
        "(i32.eq (i32.wrap_i64 (i64.const 0x100000005)) (i32.const 5))",
    ),
    (
        "",
        "(i64.eq (i64.rem_s (i64.const -9223372036854775808) (i64.const -1)) (i64.const 0))",
    ),
    (
        "",
        "(i64.eq (i64.div_s (i64.const -9) (i64.const 4)) (i64.const -2))",
    ),
    ("", "(i64.eq (i64.clz (i64.const 1)) (i64.const 63))"),
    (
        "",
        "(i64.eq (i64.ctz (i64.const 0x100000000)) (i64.const 32))",
    ),
    ("", "(i64.eq (i64.popcnt (i64.const -1)) (i64.const 64))"),
    (
        "",
        "(i64.eq (i64.rotl (i64.const 1) (i64.const 65)) (i64.const 2))",
    ),
    (
        "",
        "(i64.eq (i64.shr_u (i64.const -1) (i64.const 127)) (i64.const 1))",
    ),
    ("", "(i32.lt_s (i32.const -1) (i32.const 0))"),
    ("", "(i32.gt_u (i32.const -1) (i32.const 0))"),
    ("", "(i32.le_u (i32.const 5) (i32.const 5))"),
    ("", "(i32.ge_s (i32.const -5) (i32.const -6))"),
    ("", "(i64.ne (i64.const 1) (i64.const 2))"),
    ("", "(i32.eqz (i32.const 0))"),
    (
        "",
        "(i32.eq (select (i32.const 5) (i32.const 6) (i32.const 0)) (i32.const 6))",
    ),
    (
        "",
        "(i32.eq (i32.mul (i32.const 65536) (i32.const 65537)) (i32.const 65536))",
    ),
    (
        "",
        "(i32.store (i32.const 1024) (i32.const 0x01020304)) (i32.eq (i32.load8_u (i32.const \
         1024)) (i32.const 4))",
    ),
    (
        "",
        "(i32.store (i32.const 1024) (i32.const 0x01020304)) (i32.eq (i32.load16_s offset=2 \
         (i32.const 1024)) (i32.const 0x0102))",
    ),
    (
        "",
        "(i64.store (i32.const 1024) (i64.const -2)) (i64.eq (i64.load32_u (i32.const 1024)) \
         (i64.const 4294967294))",
    ),
    (
        "",
        "(i32.store8 (i32.const 1024) (i32.const 0x1ff)) (i32.eq (i32.load8_s (i32.const 1024)) \
         (i32.const -1))",
    ),
    (
        "(data (i32.const 2048) \"\\2a\")",
        "(i32.eq (i32.load8_u (i32.const 2048)) (i32.const 42))",
    ),
    (
        "",
        "(memory.fill (i32.const 3000) (i32.const 7) (i32.const 4)) (i32.eq (i32.load (i32.const \
         3000)) (i32.const 0x07070707))",
    ),
    (
        "",
        "(i32.store (i32.const 3000) (i32.const 0x04030201)) (memory.copy (i32.const 3001) \
         (i32.const 3000) (i32.const 3)) (i32.eq (i32.load (i32.const 3000)) (i32.const \
         0x03020101))",
    ),
    ("", "(i32.eq (memory.size) (i32.const 1))"),
    ("", "(i32.eq (memory.grow (i32.const 1)) (i32.const -1))"),
    (
        "(global $g (mut i32) (i32.const 5))",
        "(global.set $g (i32.add (global.get $g) (i32.const 1))) (i32.eq (global.get $g) \
         (i32.const 6))",
    ),
    (
        "(func $double (param i32) (result i32) (i32.mul (local.get 0) (i32.const 2)))",
        "(i32.eq (call $double (i32.const 21)) (i32.const 42))",
    ),
    (
        "",
        "(i32.eq (i32.add (block (result i32 i32) (i32.const 40) (i32.const 2))) (i32.const 42))",
    ),
    (
        "",
        "(i32.eq (block (result i32) (br_if 0 (i32.const 7) (i32.const 1)) (drop) (i32.const 2)) \
         (i32.const 7))",
    ),
    (
        "",
        "(i32.eq (block $out (result i32) (block $one (block $zero (br_table $zero $one \
         (i32.const 1))) (br $out (i32.const 10))) (i32.const 1)) (i32.const 1))",
    ),
    (
        "",
        "(loop $l (local.set $s (i32.add (local.get $s) (local.get $i))) (local.set $i (i32.add \
         (local.get $i) (i32.const 1))) (br_if $l (i32.lt_u (local.get $i) (i32.const 10)))) \
         (i32.eq (local.get $s) (i32.const 45))",
    ),
    (
        "",
        "(i32.eq (if (result i32) (i32.const 0) (then (i32.const 1)) (else (i32.const 2))) \
         (i32.const 2))",
    ),
];

#[test]
fn integers_and_control_follow_webassemblys_semantics() {
    let mut wrong = Vec::new();
    for (fields, body) in CHECKS {
        let text = exits_with(fields, body);
        let done = ran(&text, Policy::default(), &Given::default());
        if done.end != Ended::Exit(1) {
            wrong.push(format!("{body}: {:?}", done.end));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A module with the memory MEMORY that exits with what growing it a page gives, plus one: 2
/// where it grows, from its one page, and 0 where it doesn't.
const GROWS: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  MEMORY
  (func (export "_start")
    (call $exit (i32.add (memory.grow (i32.const 1)) (i32.const 1)))))
"#;

/// The memory grows as far as the engine lets it: one the module keeps to itself to its maximum,
/// or without one to the most a 32-bit memory has; one exported as `memory` to its maximum, or
/// without one not at all.
#[test]
fn memory_grows_as_far_as_the_engine_lets_it() {
    let memories = [
        ("(memory 1)", 2),
        ("(memory 1 3)", 2),
        ("(memory 1 1)", 0),
        ("(memory (export \"memory\") 1)", 0),
        ("(memory (export \"memory\") 1 3)", 2),
    ];
    for (memory, exit) in memories {
        let text = GROWS.replace("MEMORY", memory);
        let done = ran(&text, Policy::default(), &Given::default());
        assert_eq!(done.end, Ended::Exit(exit), "{memory}");
    }
}

/// A module whose `_start` runs `body`, which is on line 4.
fn starts(body: &str) -> String {
    format!(
        "(module\n  (memory (export \"memory\") 1 1)\n  (func (export \"_start\")\n    {body}))\n"
    )
}

/// Each trap the engine stops at, with the line of main.wat it's on and what the engine says.
#[test]
fn each_trap_ends_the_run_at_its_line() {
    let traps = [
        (
            starts("(drop (i32.div_u (i32.const 1) (i32.const 0)))"),
            4,
            "an integer division by zero",
        ),
        (
            starts("(drop (i64.rem_s (i64.const 1) (i64.const 0)))"),
            4,
            "an integer division by zero",
        ),
        (
            starts("(drop (i32.div_s (i32.const -2147483648) (i32.const -1)))"),
            4,
            "a signed division whose result doesn't fit",
        ),
        (starts("unreachable"), 4, "it reached `unreachable`"),
        (
            starts("(drop (i32.load (i32.const 65533)))"),
            4,
            "it reached memory outside its own",
        ),
        (
            starts("(i32.store8 (i32.const 65536) (i32.const 1))"),
            4,
            "it reached memory outside its own",
        ),
        (
            starts("(memory.fill (i32.const 65530) (i32.const 0) (i32.const 7))"),
            4,
            "it reached memory outside its own",
        ),
        (
            "(module\n  (import \"wasi_snapshot_preview1\" \"fd_write\"\n    (func $fd_write \
             (param i32 i32 i32 i32) (result i32)))\n  (memory (export \"memory\") 1 1)\n  (func \
             (export \"_start\")\n    (drop (call $fd_write (i32.const 1) (i32.const 65532) \
             (i32.const 1) (i32.const 0)))))\n"
                .to_owned(),
            6,
            "it reached memory outside its own",
        ),
    ];
    for (text, line, what) in traps {
        let done = ran(&text, Policy::default(), &Given::default());
        assert_eq!(
            done.end,
            Ended::Trap(format!("main.wat line {line}: {what}")),
            "{text}"
        );
    }
}

/// Why the model refuses the module of `text`.
fn refusal(text: &str) -> String {
    let built = build(text).unwrap_or_else(|problem| panic!("{problem}: {text}"));
    Module::new(&built, Policy::default())
        .err()
        .unwrap_or_else(|| panic!("the model took {text}"))
}

/// Imports that count WASI's values but aren't of its types, which the engine doesn't link:
/// clock_time_get's precision and fd_seek's offset are 64 bits, and proc_exit's code 32.
const MISTYPED: [&str; 3] = [
    r#"(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $time (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (call $time (i32.const 0) (i32.const 0) (i32.const 1024)))))
"#,
    r#"(module
  (import "wasi_snapshot_preview1" "fd_seek"
    (func $seek (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (call $seek (i32.const 1) (i32.const -1) (i32.const 0) (i32.const 1024)))))
"#,
    r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i64)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (call $exit (i64.const 0))))
"#,
];

/// A table, and an element segment past its end, which the engine traps on as it starts the
/// module.
const TABLED: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (table 1 funcref)
  (func $f)
  (elem (i32.const 1) $f)
  (func (export "_start")
    (call $exit (i32.const 0))))
"#;

/// What the engine doesn't start, or traps on as it starts it, the model refuses, so no proof of
/// it holds: an import of another type than WASI's, and a table or an element segment.
#[test]
fn what_the_engine_doesnt_start_the_model_refuses() {
    for text in MISTYPED {
        let refused = refusal(text);
        assert!(refused.contains("other than WASI's"), "{refused}");
    }
    let refused = refusal(TABLED);
    assert!(refused.contains("a table"), "{refused}");
}

/// A module whose `_start` calls a chain of `length` functions, each the next.
fn chain(length: usize) -> String {
    let mut text = String::from("(module\n  (memory (export \"memory\") 1 1)\n");
    for link in 0..length {
        let next = link + 1;
        text.push_str(&format!("  (func $f{link} (call $f{next}))\n"));
    }
    text.push_str(&format!(
        "  (func $f{length})\n  (func (export \"_start\") (call $f0)))\n"
    ));
    text
}

/// A function that calls itself.
const ITSELF: &str = r#"(module
  (memory (export "memory") 1 1)
  (func $down
    (call $down))
  (func (export "_start")
    (call $down)))
"#;

/// Two functions that call each other.
const THROUGH: &str = r#"(module
  (memory (export "memory") 1 1)
  (func $ping (call $pong))
  (func $pong (call $ping))
  (func (export "_start") (call $ping)))
"#;

/// The start of a module whose `_start` calls `$deep` with DEPTH, its locals still to come.
const DEEP: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (drop (call $deep (i32.const DEPTH)))
    (call $exit (i32.const 0)))
  (func $deep (param $n i32) (result i64)
    (local $sum i64)"#;

/// A module whose `$deep` calls itself `depth` times, each call keeping `values` values of 64
/// bits across the next: the engine's stack runs out once the calls keep more than its 512 KiB.
fn recursing(depth: u32, values: u32) -> String {
    let mut locals = String::new();
    let mut loads = String::new();
    let mut sums = String::new();
    for value in 0..values {
        let offset = value * 8;
        locals.push_str(&format!("\n    (local $v{value} i64)"));
        loads.push_str(&format!(
            "\n    (local.set $v{value} (i64.load offset={offset} (i32.const 1024)))"
        ));
        sums.push_str(&format!(
            "\n    (local.set $sum (i64.add (local.get $sum) (local.get $v{value})))"
        ));
    }
    let call = "\n    (if (local.get $n)\n      (then (local.set $sum\n        (call $deep \
                (i32.sub (local.get $n) (i32.const 1))))))";
    let start = DEEP.replace("DEPTH", &depth.to_string());
    format!("{start}{locals}{loads}{call}{sums}\n    (local.get $sum)))\n")
}

/// What the model refuses because the engine's stack could run out: a function that calls
/// itself, directly or through others, and calls that may go deeper than the model is sure the
/// engine's stack holds. The model once took a recursion of fewer than 1024 calls, which the
/// engine traps on where its frames are large; a chain of calls well within the stack runs.
#[test]
fn recursion_and_calls_past_the_engines_stack_are_refused() {
    let refused = refusal(ITSELF);
    assert!(refused.starts_with("$down calls itself"), "{refused}");
    let refused = refusal(THROUGH);
    assert!(refused.starts_with("$ping calls itself"), "{refused}");
    let refused = refusal(&recursing(1000, 80));
    assert!(refused.starts_with("$deep calls itself"), "{refused}");
    let refused = refusal(&chain(300));
    assert!(refused.contains("deepest calls"), "{refused}");
    let done = ran(&chain(100), Policy::default(), &Given::default());
    assert_eq!(done.end, Ended::Exit(0));
}

/// Reads a line with lengua's library and writes it back, as tests/host.rs's own case does.
const LINE: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $got i32)
    (local.set $got (call $fill (i32.const 0) (i32.const 64) (i32.const 100) (i32.const 10)))
    (if (i32.lt_s (local.get $got) (i32.const 0))
      (then (call $exit (i32.sub (i32.const 0) (local.get $got)))))
    (call $exit (i32.sub (i32.const 0)
      (call $write_all (i32.const 1) (i32.const 64) (local.get $got))))))
"#;

#[test]
fn lengua_s_library_runs_as_the_code_it_is() {
    let assertion = "(module\n  (memory (export \"memory\") 1)\n  (func (export \"_start\") \
                     (call $assert_fail (i32.const 7))))\n";
    let done = ran(assertion, Policy::default(), &Given::default());
    assert_eq!(done.end, Ended::Exit(134));
    assert_eq!(done.stderr, b"lengua: assertion 007 failed\n");
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let given = Given {
        stdin: b"a line\n".to_vec(),
        ..Given::default()
    };
    let done = ran(LINE, reads, &given);
    assert_eq!(done.end, Ended::Exit(0));
    assert_eq!(done.stdout, b"a line\n");
}

/// Reads standard input once and writes what it read; exits with the read's error, if any.
const ECHO: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (local $errno i32)
    (i32.store (i32.const 0) (i32.const 64))
    (i32.store (i32.const 4) (i32.const 64))
    (local.set $errno (call $fd_read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 4) (i32.load (i32.const 8)))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
    (call $exit (i32.const 0))))
"#;

/// Copies in.txt to out.txt in its working folder.
const COPY: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 100) "in.txt")
  (data (i32.const 120) "out.txt")
  (func (export "_start")
    (local $errno i32)
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 100) (i32.const 6)
      (i32.const 0) (i64.const 2) (i64.const 0) (i32.const 0) (i32.const 0)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 16) (i32.const 200))
    (i32.store (i32.const 20) (i32.const 64))
    (local.set $errno
      (call $fd_read (i32.load (i32.const 0)) (i32.const 16) (i32.const 1) (i32.const 24)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (local.set $errno (call $open (i32.const 3) (i32.const 0) (i32.const 120) (i32.const 7)
      (i32.const 9) (i64.const 64) (i64.const 0) (i32.const 0) (i32.const 4)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (i32.store (i32.const 20) (i32.load (i32.const 24)))
    (local.set $errno
      (call $fd_write (i32.load (i32.const 4)) (i32.const 16) (i32.const 1) (i32.const 28)))
    (if (local.get $errno) (then (call $exit (local.get $errno))))
    (drop (call $close (i32.load (i32.const 0))))
    (call $exit (call $close (i32.load (i32.const 4))))))
"#;

#[test]
fn host_calls_answer_as_lengua_s_host_does() {
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let given = Given {
        stdin: b"abc".to_vec(),
        ..Given::default()
    };
    let done = ran(ECHO, reads, &given);
    assert_eq!((done.end, done.stdout), (Ended::Exit(0), b"abc".to_vec()));
    // An idea that doesn't name standard input is refused it: NOTCAPABLE, 76.
    let done = ran(ECHO, Policy::default(), &given);
    assert_eq!((done.end, done.stdout), (Ended::Exit(76), Vec::new()));
    // Files: in.txt copied to out.txt, both there after the run.
    let files = Policy {
        files: true,
        ..Policy::default()
    };
    let given = Given {
        files: BTreeMap::from([("in.txt".to_owned(), b"hello".to_vec())]),
        ..Given::default()
    };
    let done = ran(COPY, files, &given);
    assert_eq!(done.end, Ended::Exit(0));
    assert_eq!(
        done.files,
        BTreeMap::from([
            ("in.txt".to_owned(), b"hello".to_vec()),
            ("out.txt".to_owned(), b"hello".to_vec()),
        ])
    );
    // Without files named, path_open is refused: NOTCAPABLE.
    let done = ran(COPY, Policy::default(), &given);
    assert_eq!(done.end, Ended::Exit(76));
}

/// Reads the connection at descriptor 5 five times, each into a buffer of its own room, and exits
/// with the number of the first read that doesn't give the count it should, or with 0.
const CHUNKS: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 2 2)
  ;; One read of the connection into $room bytes at 4096: its count.
  (func $read (param $room i32) (result i32)
    (i32.store (i32.const 1024) (i32.const 4096))
    (i32.store (i32.const 1028) (local.get $room))
    (drop (call $fd_read (i32.const 5) (i32.const 1024) (i32.const 1) (i32.const 1032)))
    (i32.load (i32.const 1032)))
  ;; Exits with $number where a read into $room bytes doesn't give $count.
  (func $expect (param $number i32) (param $room i32) (param $count i32)
    (if (i32.ne (call $read (local.get $room)) (local.get $count))
      (then (call $exit (local.get $number)))))
  (func (export "_start")
    (call $expect (i32.const 1) (i32.const 32768) (i32.const 16384))
    (call $expect (i32.const 2) (i32.const 10000) (i32.const 10000))
    (call $expect (i32.const 3) (i32.const 32768) (i32.const 6384))
    (call $expect (i32.const 4) (i32.const 32768) (i32.const 7232))
    (call $expect (i32.const 5) (i32.const 32768) (i32.const 0))
    (call $exit (i32.const 0))))
"#;

/// A connection's reads give what lengua's host gives: it takes what arrives in at most 16 KiB
/// at a time, and a read gets only what one take brought in. The test's server replies 40,000
/// bytes at once, which the host takes in as 16 KiB, 16 KiB and the rest, so the reads give
/// 16,384 bytes, then 10,000 and 6,384 from the second take, then 7,232, and then the end.
#[test]
fn a_connection_reads_what_the_host_took_in_at_once() {
    let one = Policy {
        connections: 1,
        ..Policy::default()
    };
    let module = module(CHUNKS, one);
    let mut pool = Pool::default();
    let reply = exec::bytes(&mut pool, &[7; 40_000]);
    let network = net::Network {
        server: vec![(Vec::new(), vec![reply])],
        made: vec![Some(0)],
        client: Vec::new(),
        listening: false,
        at_once: false,
        eager: true,
    };
    let start = begin(&module, &mut pool, &[], &[], Vec::new()).with_network(network);
    let runs = exec::explore(&module, start, &mut pool, &mut exec::Concrete, 2).expect("the run");
    let [run] = runs.as_slice() else {
        panic!("{} runs", runs.len());
    };
    let Some(End::Exit(code)) = &run.world.end else {
        panic!("{:?}", run.world.end);
    };
    assert_eq!(pool.constant(*code), Some(0));
}

/// Waits with poll_oneoff on the listener, which no client calls, and on the time of day's
/// clock, for TIMEOUT nanoseconds, or till that reading where FLAGS is 1; then exits with the
/// first event's own word: 2, the clock's.
const WAITS: &str = r#"(module
  (import "wasi_snapshot_preview1" "poll_oneoff"
    (func $poll (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    ;; A read on descriptor 4, the listener, with the word 1.
    (i64.store (i32.const 1280) (i64.const 1))
    (i32.store8 (i32.const 1288) (i32.const 1))
    (i32.store (i32.const 1296) (i32.const 4))
    ;; The time of day's clock, with the word 2.
    (i64.store (i32.const 1328) (i64.const 2))
    (i32.store8 (i32.const 1336) (i32.const 0))
    (i32.store (i32.const 1344) (i32.const 0))
    (i64.store (i32.const 1352) (i64.const TIMEOUT))
    (i32.store16 (i32.const 1368) (i32.const FLAGS))
    (drop (call $poll (i32.const 1280) (i32.const 1216) (i32.const 2) (i32.const 1064)))
    (call $exit (i32.wrap_i64 (i64.load (i32.const 1216))))))
"#;

/// A wait beside a descriptor takes its time, as lengua's host waits in real time then: beside
/// the listener, which no client calls, a wait of a second comes, as does a time the test's
/// clock, at a second, has passed; one of 5 seconds, past a test's time limit, never does, nor
/// does a time on the test's clock that is still to come, as its readings don't move.
#[test]
fn a_wait_beside_a_descriptor_comes_only_within_a_tests_time() {
    let serves = Policy {
        clock: true,
        listens: true,
        ..Policy::default()
    };
    let given = Given {
        clock: vec![(1, 0)],
        listens: true,
        ..Given::default()
    };
    let waits = [
        ("1000000000", "0", Some(2)),
        ("500000000", "1", Some(2)),
        ("5000000000", "0", None),
        ("2000000000", "1", None),
    ];
    for (timeout, flags, exit) in waits {
        let text = WAITS.replace("TIMEOUT", timeout).replace("FLAGS", flags);
        let done = ran(&text, serves, &given);
        let never = Ended::Other(host::NEVER_COMES.to_owned());
        let wanted = exit.map_or(never, Ended::Exit);
        assert_eq!(done.end, wanted, "{timeout} {flags}");
    }
}

/// An oracle for which every branch can go both ways.
struct Anything;

impl Oracle for Anything {
    fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
        Ok(true)
    }
}

/// Reads a byte of standard input into 2048, then runs `rest`, which starts on line 11.
fn reads_a_byte(rest: &str) -> String {
    format!(
        "(module\n  (import \"wasi_snapshot_preview1\" \"fd_read\"\n    (func $fd_read (param i32 \
         i32 i32 i32) (result i32)))\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func \
         $exit (param i32)))\n  (memory (export \"memory\") 1 1)\n  (data (i32.const 3000) \
         \"abcd\")\n  (func (export \"_start\")\n    (i32.store (i32.const 1024) (i32.const \
         2048))\n    (i32.store (i32.const 1028) (i32.const 1))\n    (drop (call $fd_read \
         (i32.const 0) (i32.const 1024) (i32.const 1) (i32.const 1032)))\n    {rest}))\n"
    )
}

/// The runs of `text` on a byte of standard input the input chooses, `byte`, every branch
/// taken both ways.
fn runs_on_a_byte(text: &str) -> (Pool, Vec<State<Wasm>>) {
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let module = module(text, reads);
    let mut pool = Pool::default();
    let byte = pool.var(8, "byte");
    let start = begin(&module, &mut pool, &[], &[], vec![byte]);
    let runs = exec::explore(&module, start, &mut pool, &mut Anything, 16).expect("the runs");
    (pool, runs)
}

/// What the run's exit code is for `byte`.
fn exit_for(pool: &Pool, run: &State<Wasm>, byte: u128) -> Option<u128> {
    let Some(End::Exit(code)) = &run.world.end else {
        return None;
    };
    pool.evaluate(*code, &HashMap::from([("byte".to_owned(), byte)]))
}

#[test]
fn what_the_input_decides_splits_runs_and_holds_them_to_their_traps() {
    // A branch the byte decides: a run each way, each keeping its condition.
    let text = reads_a_byte(
        "(if (i32.gt_u (i32.load8_u (i32.const 2048)) (i32.const 100))\n      (then (call $exit \
         (i32.const 1))))\n    (call $exit (i32.div_u (i32.const 100) (i32.load8_u (i32.const \
         2048))))",
    );
    let (pool, runs) = runs_on_a_byte(&text);
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(runs.iter().all(|run| run.world.path.len() == 1));
    // The division by the byte is the proof's to rule out, on its line.
    let divides = runs
        .iter()
        .find(|run| !run.world.obligations.is_empty())
        .expect("the run that divides");
    assert_eq!(
        divides.world.obligations[0].1,
        "main.wat line 13: an integer division by zero"
    );
    assert_eq!(exit_for(&pool, divides, 4), Some(25));
    // A load whose place the byte chooses, inside a table: every place it can be, no trap.
    let text = reads_a_byte(
        "(call $exit (i32.load8_u (i32.add (i32.const 3000) (i32.and (i32.load8_u (i32.const \
         2048)) (i32.const 3)))))",
    );
    let (pool, runs) = runs_on_a_byte(&text);
    assert_eq!(runs.len(), 1);
    assert!(runs[0].world.obligations.is_empty());
    assert_eq!(exit_for(&pool, &runs[0], 2), Some(u128::from(b'c')));
    assert_eq!(exit_for(&pool, &runs[0], 7), Some(u128::from(b'd')));
    // One that may leave the memory: the trap is an obligation, on its line.
    let text = reads_a_byte(
        "(call $exit (i32.load8_u (i32.add (i32.const 65530) (i32.load8_u (i32.const 2048)))))",
    );
    let (_, runs) = runs_on_a_byte(&text);
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].world.obligations[0].1,
        "main.wat line 11: it reached memory outside its own"
    );
}

/// What `operator`, a count of bits, gives on `value`, `bits` wide, as WebAssembly has it.
fn bits_counted(operator: &str, value: u64, bits: u32) -> u32 {
    match operator {
        "clz" => value.leading_zeros() - (64 - bits),
        "ctz" => value.trailing_zeros().min(bits),
        _ => value.count_ones(),
    }
}

/// The counts of bits on a value the input decides, whose terms a proof takes, rather than on a
/// known value, which the model counts at once: each gives what its operator does, at both
/// widths, on bytes at the bottom of the value and at its top.
#[test]
fn counts_of_bits_follow_a_value_the_input_decides() {
    let byte = "(i32.load8_u (i32.const 2048))";
    for operator in ["clz", "ctz", "popcnt"] {
        for (bits, shift) in [(32, 0), (32, 24), (64, 0), (64, 56)] {
            let expression = if bits == 32 {
                format!("(i32.{operator} (i32.shl {byte} (i32.const {shift})))")
            } else {
                format!(
                    "(i32.wrap_i64 (i64.{operator} (i64.shl (i64.extend_i32_u {byte}) \
                     (i64.const {shift}))))"
                )
            };
            let text = reads_a_byte(&format!("(call $exit {expression})"));
            let (pool, runs) = runs_on_a_byte(&text);
            assert_eq!(runs.len(), 1, "{expression}");
            for value in [0_u8, 1, 2, 0x10, 0x5a, 0x80, 0xff] {
                let wanted = bits_counted(operator, u64::from(value) << shift, bits);
                assert_eq!(
                    exit_for(&pool, &runs[0], u128::from(value)),
                    Some(u128::from(wanted)),
                    "{expression} of {value:#x}"
                );
            }
        }
    }
}

/// A folder's listing in each of a test's two orders, as a proof takes both.
#[test]
fn a_listing_is_taken_in_both_orders() {
    let text = "(module\n  (import \"wasi_snapshot_preview1\" \"fd_readdir\"\n    (func \
                $fd_readdir (param i32 i32 i32 i64 i32) (result i32)))\n  (import \
                \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit (param i32)))\n  (memory \
                (export \"memory\") 1 1)\n  (func (export \"_start\")\n    (drop (call \
                $fd_readdir (i32.const 3) (i32.const 4096) (i32.const 4096) (i64.const 0) \
                (i32.const 1024)))\n    (call $exit (i32.load8_u (i32.const 4120)))))\n";
    let files = Policy {
        files: true,
        ..Policy::default()
    };
    let module = module(text, files);
    let mut pool = Pool::default();
    let given = BTreeMap::from([
        ("a".to_owned(), exec::bytes(&mut pool, b"1")),
        ("b".to_owned(), exec::bytes(&mut pool, b"2")),
    ]);
    let start = begin(&module, &mut pool, &[], &[], Vec::new()).with_files(given);
    let runs = exec::explore(&module, start, &mut pool, &mut exec::Concrete, 4).expect("the runs");
    // The first entry's name, after its 24-byte head: `a` sorted, `b` turned round.
    let mut first: Vec<Option<u128>> = runs
        .iter()
        .map(|run| match &run.world.end {
            Some(End::Exit(code)) => pool.constant(*code),
            _ => None,
        })
        .collect();
    first.sort_unstable();
    assert_eq!(first, [Some(u128::from(b'a')), Some(u128::from(b'b'))]);
    // A run given its order lists in it alone.
    for (reversed, name) in [(false, b'a'), (true, b'b')] {
        let given = Given {
            files: BTreeMap::from([
                ("a".to_owned(), b"1".to_vec()),
                ("b".to_owned(), b"2".to_vec()),
            ]),
            reversed,
            ..Given::default()
        };
        assert_eq!(ran(text, files, &given).end, Ended::Exit(name));
    }
}

/// An oracle with no branch to decide, which says the proof's time is up the fourth time it's
/// asked.
struct TimeUpAtTheFourthLook(std::cell::Cell<u32>);

impl Oracle for TimeUpAtTheFourthLook {
    fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
        Err("a concrete run reached a branch that depends on something unknown".into())
    }

    fn in_time(&self) -> Result<(), String> {
        self.0.set(self.0.get() + 1);
        if self.0.get() > 3 {
            Err("time is up".into())
        } else {
            Ok(())
        }
    }
}

/// An oracle with no branch to decide, which says the computer has no memory left the first
/// time it's asked.
struct NoRoomAtTheFirstLook(std::cell::Cell<u32>);

impl Oracle for NoRoomAtTheFirstLook {
    fn possible(&mut self, _: &Pool, _: &[Term], _: Term) -> Result<bool, String> {
        Err("a concrete run reached a branch that depends on something unknown".into())
    }

    fn has_room(&self) -> Result<(), String> {
        self.0.set(self.0.get() + 1);
        Err("no room".into())
    }
}

/// A loop `times` times round with nothing to decide, each round a new number, then an exit
/// with 0.
fn counted_loop(times: u32) -> String {
    format!(
        "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit (param i32)))\n  \
         (memory (export \"memory\") 1 1)\n  (func (export \"_start\") (local $left i32)\n    \
         (local.set $left (i32.const {times}))\n    (loop $round\n      (local.set $left \
         (i32.sub (local.get $left) (i32.const 1)))\n      (br_if $round (local.get $left)))\n    \
         (call $exit (i32.const 0))))\n"
    )
}

/// A run of 100,000 times round a loop ends with the proof's time, though it never asks the
/// oracle about a branch: this is how a hash of the input, all straight-line code over unknown
/// values, stays within the proof's time. `explore` asks before every step.
#[test]
fn a_run_with_no_branch_on_its_inputs_keeps_the_proofs_time() {
    let module = module(&counted_loop(100_000), Policy::default());
    let mut pool = Pool::default();
    let start = begin(&module, &mut pool, &[], &[], Vec::new());
    let mut oracle = TimeUpAtTheFourthLook(std::cell::Cell::new(0));
    let result = exec::explore(&module, start, &mut pool, &mut oracle, 2);
    assert_eq!(result.err().as_deref(), Some("time is up"));
    assert_eq!(oracle.0.get(), 4);
}

/// A run that makes a new number every round ends with the computer's memory, though it never
/// asks the oracle about a branch: a hash of the input, all straight-line code over unknown
/// values, makes terms by the million, and the steps alone don't bound them. `explore` asks each
/// time its pool has grown by tens of thousands of terms.
#[test]
fn a_run_that_fills_the_pool_keeps_the_memory() {
    let module = module(&counted_loop(1_000_000), Policy::default());
    let mut pool = Pool::default();
    let start = begin(&module, &mut pool, &[], &[], Vec::new());
    let mut oracle = NoRoomAtTheFirstLook(std::cell::Cell::new(0));
    let result = exec::explore(&module, start, &mut pool, &mut oracle, 2);
    assert_eq!(result.err().as_deref(), Some("no room"));
    assert_eq!(oracle.0.get(), 1);
}

/// A run on a pool that found the memory short ends at its next step, though the oracle never
/// has to ask: one step can make the terms by the million in a single extraction, far between
/// two looks of the loop's, and what the pool made after that is nothing to go on.
#[test]
fn a_run_on_a_pool_that_found_the_memory_short_ends_at_its_next_step() {
    let module = module(&exits_with("", "(i32.const 0)"), Policy::default());
    let _no_memory = crate::prove::watch::NoMemoryForTest::pretend();
    let mut pool = Pool::default();
    let start = begin(&module, &mut pool, &[], &[], Vec::new());
    let one = pool.bv(32, 1);
    while !pool.out_of_room() {
        let next = pool.var(32, &format!("b{}", pool.len()));
        let _ = pool.add(next, one);
    }
    let result = exec::explore(&module, start, &mut pool, &mut exec::Concrete, 2);
    assert_eq!(
        result.err().as_deref(),
        Some(crate::prove::watch::OUT_OF_MEMORY)
    );
}

/// Whether z3 is installed, which every proof needs: a test without it says so and passes.
fn solvers_installed() -> bool {
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        crate::skiplog::note("z3 isn't installed");
    }
    installed
}

/// Whether cvc5 is installed too, which confirms every answer of z3's that rules a case out.
fn both_solvers() -> bool {
    std::process::Command::new("cvc5")
        .arg("--version")
        .output()
        .is_ok()
}

/// A file of the program `name`: the fixture of that name in tests/fixtures, or else the example
/// in examples/. add, echo and greet are in both, and these tests take the fixtures, the
/// prover's small ones; count and hypotenuse are examples.
fn fixture(name: &str, file: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let in_fixtures = root.join("tests/fixtures").join(name);
    let folder = if in_fixtures.is_dir() {
        in_fixtures
    } else {
        root.join("examples").join(name)
    };
    let path = folder.join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A file of the compile skill's assets, its templates among them.
fn asset(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The generated blocks of the idea `text`, and what the idea lets its program reach.
fn blocks_in(text: &str) -> (Vec<Generated>, Policy) {
    let idea = crate::idea::parse(text).unwrap_or_else(|problems| panic!("{problems:?}"));
    let suite = crate::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    (suite.generated, Policy::of(&idea.uses, &idea.network))
}

/// The generated blocks of the fixture `name`'s idea, and what its idea lets it reach.
fn blocks_of(name: &str) -> (Vec<Generated>, Policy) {
    blocks_in(&fixture(name, "idea.md"))
}

/// A proof's budget: at most `seconds`, its listener at descriptor 4 when it `listens`.
fn budget(seconds: u64, listens: bool) -> Budget {
    Budget {
        max_paths: 64,
        max_shapes: 2000,
        ..Budget::until(Instant::now() + Duration::from_secs(seconds), listens)
    }
}

/// Whether `verdict` proves every input in range, z3 and cvc5 agreeing where both are installed.
fn proven(verdict: &Verdict) -> bool {
    matches!(
        verdict,
        Verdict::Proven { bound: None, alone, .. } if alone.is_none() || !both_solvers()
    )
}

/// add's first block over the shapes of every sign and of up to three digits: the whole range,
/// to 10^9, is a longer proof, which `proves_add_for_every_pair_of_inputs` makes.
fn sums() -> Generated {
    let (blocks, _) = blocks_of("add");
    let mut sums = blocks[0].clone();
    for (_, generator) in &mut sums.vars {
        *generator = Generator::Int { lo: -999, hi: 999 };
    }
    sums
}

/// `source` with the first `from` in it made `to`.
fn edited(source: &str, (from, to): (&str, &str)) -> String {
    assert!(source.contains(from), "the program has no {from:?}");
    source.replacen(from, to, 1)
}

/// The line of `text`, from 1, of the first that holds `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle:?}"))
        + 1
}

/// The trap a counterexample found, with what it says.
fn trap(verdict: &Verdict) -> String {
    let Verdict::Counterexample(found) = verdict else {
        panic!("not a counterexample: {verdict:?}");
    };
    match found.get(UNSAFE) {
        Some(Value::Str(problem)) => problem.clone(),
        other => panic!("not a trap: {other:?} in {found:?}"),
    }
}

/// Every block of greet and the echo server, for every input; add for every sign and for numbers
/// of up to three digits, which takes its code's every path but the longer numbers' rounds of
/// its loops.
#[test]
fn proves_greet_and_echo_and_add_to_three_digits() {
    if !solvers_installed() {
        return;
    }
    let (_, policy) = blocks_of("add");
    let built = build(&fixture("add", "main.wat")).expect("add builds");
    let verdict = prove_wasm(&built, policy, &sums(), &budget(900, false));
    assert!(proven(&verdict), "add: {verdict:?}");
    let (blocks, policy) = blocks_of("greet");
    let built = build(&fixture("greet", "main.wat")).expect("greet builds");
    for (number, block) in blocks.iter().enumerate() {
        let verdict = prove_wasm(&built, policy, block, &budget(900, false));
        assert!(proven(&verdict), "greet's block {number}: {verdict:?}");
    }
    let (blocks, policy) = blocks_of("echo");
    let built = build(&fixture("echo", "main.wat")).expect("echo builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &budget(900, true));
    assert!(proven(&verdict), "echo: {verdict:?}");
}

/// add's first block as its idea has it: every pair of numbers from -10^9 to 10^9, all of
/// `$parse_i64`'s digits and `$write_i64`'s scales. Slow, as the native add's was, so the slow
/// ignored tests run it: `cargo test --release -- --ignored`.
#[test]
#[ignore = "slow: proves 4×10^18 pairs of inputs"]
fn proves_add_for_every_pair_of_inputs() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("add");
    let built = build(&fixture("add", "main.wat")).expect("add builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &budget(3600, false));
    assert!(proven(&verdict), "add: {verdict:?}");
}

/// add's sign, which a wrong constant makes zero's too.
const SIGN: &str = ";; The sign, for a value below zero.\n    (if (i64.lt_s (local.get $value) \
                    (i64.const 0))";

/// add's check that ends its digits, without which its last division is by zero.
const GUARD: &str = "        (br_if $written (i64.eqz (local.get $power)))\n";

/// echo's call of `$fill`, which an access one past its region lends one byte too many.
const FILL: &str = "(global.get $line) (global.get $line.size) (global.get $NEWLINE)";

/// echo's answer, which an off-by-one cuts short.
const ANSWER: &str = "(call $write_all (local.get $fd) (global.get $line) (local.get $got))";

/// greet's check of a name's length, which an off-by-one makes refuse the longest.
const LONGEST: &str = "(br_if $usage (i32.gt_u (local.get $length) (global.get $MAX_NAME)))";

#[test]
fn finds_a_planted_bug_in_each_fixture() {
    if !solvers_installed() {
        return;
    }
    let (_, adds) = blocks_of("add");
    let add = fixture("add", "main.wat");
    // A wrong constant: a sum of zero comes out as "-0".
    let below_one = SIGN.replace("(i64.const 0)", "(i64.const 1)");
    let wrong = edited(&add, (SIGN, below_one.as_str()));
    let built = build(&wrong).expect("it builds");
    let verdict = prove_wasm(&built, adds, &sums(), &budget(900, false));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("a wrong constant: {verdict:?}");
    };
    let (Some(Value::Int(a)), Some(Value::Int(b))) = (found.get("a"), found.get("b")) else {
        panic!("a wrong constant: {found:?}");
    };
    assert_eq!(a + b, 0, "a wrong constant: {found:?}");
    // An unguarded division: the digits go on past the last, and divide by a power of zero.
    let unguarded = edited(&add, (GUARD, ""));
    let line = line_of(
        &unguarded,
        "(i64.div_u (local.get $magnitude) (local.get $power))",
    );
    let built = build(&unguarded).expect("it builds");
    let verdict = prove_wasm(&built, adds, &sums(), &budget(900, false));
    assert_eq!(
        trap(&verdict),
        format!("main.wat line {line}: an integer division by zero")
    );
    // An access one past a region: $fill is lent a byte past the line, past the memory's end.
    let (echoes, policy) = blocks_of("echo");
    let echo = fixture("echo", "main.wat");
    let past = edited(
        &echo,
        (
            FILL,
            "(global.get $line) (i32.add (global.get $line.size) (i32.const 1)) (global.get \
             $NEWLINE)",
        ),
    );
    let line = line_of(&past, "(call $fill");
    let built = build(&past).expect("it builds");
    let verdict = prove_wasm(&built, policy, &echoes[0], &budget(900, true));
    assert_eq!(
        trap(&verdict),
        format!("main.wat line {line}: it reached memory outside its own")
    );
    // An off-by-one: the answer loses its last byte.
    let short = edited(
        &echo,
        (
            ANSWER,
            "(call $write_all (local.get $fd) (global.get $line) (i32.sub (local.get $got) \
             (i32.const 1)))",
        ),
    );
    let built = build(&short).expect("it builds");
    let verdict = prove_wasm(&built, policy, &echoes[0], &budget(900, true));
    assert!(
        matches!(verdict, Verdict::Counterexample(_)),
        "an off-by-one: {verdict:?}"
    );
    // An off-by-one in greet: the longest name gets the usage line.
    let (greets, policy) = blocks_of("greet");
    let greet = fixture("greet", "main.wat");
    let at_most = LONGEST.replace("i32.gt_u", "i32.ge_u");
    let refused = edited(&greet, (LONGEST, at_most.as_str()));
    let built = build(&refused).expect("it builds");
    let verdict = prove_wasm(&built, policy, &greets[0], &budget(900, false));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("greet's off-by-one: {verdict:?}");
    };
    assert!(
        matches!(found.get("name"), Some(Value::Str(name)) if name.chars().count() == 8),
        "greet's off-by-one: {found:?}"
    );
}

/// A loop that reads standard input with its own fd_read, 16 bytes at a time, four times.
const RAW_LOOP: &str = r#"(module
  (import "wasi_snapshot_preview1" "fd_read"
    (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (local $left i32)
    (local.set $left (i32.const 4))
    (loop $more
      (i32.store (i32.const 1024) (i32.const 2048))
      (i32.store (i32.const 1028) (i32.const 16))
      (drop (call $fd_read (i32.const 0) (i32.const 1024) (i32.const 1) (i32.const 1032)))
      (local.set $left (i32.sub (local.get $left) (i32.const 1)))
      (br_if $more (local.get $left)))
    (call $exit (i32.const 0))))
"#;

/// A proof's budget for texts of many lengths: at most `seconds`, and as many input
/// combinations as a proof over every length up to a buffer-full or two takes.
fn stream_budget(seconds: u64) -> Budget {
    Budget {
        max_paths: 64,
        max_shapes: 100_000,
        ..Budget::until(Instant::now() + Duration::from_secs(seconds), false)
    }
}

/// `source` with each of `edits` made, the first of each `from` in it.
fn with_edits(source: &str, edits: &[(&str, &str)]) -> String {
    edits
        .iter()
        .fold(source.to_owned(), |text, edit| edited(&text, *edit))
}

/// What a proof reads whole, for every way its input can be cut, and where the read loop is: a
/// program that reads standard input only through `$fill` does, its stop byte known, and its
/// loop's call of `$fill` is the head; one with no loop has none; a program on the network, and a
/// loop of the program's own fd_read, don't.
#[test]
fn a_read_loop_is_found_where_a_program_reads_its_input_whole() {
    let streams = module(
        &fixture("cat-stream", "main.wat"),
        blocks_of("cat-stream").1,
    );
    assert!(streams.reads_whole());
    let head = streams.read_loop_head();
    assert!(head.is_ok(), "{head:?}");
    // cat reads its input with one `$fill`: whole, but in no loop.
    let cat = module(&fixture("cat", "main.wat"), blocks_of("cat").1);
    assert!(cat.reads_whole());
    let why = cat.read_loop_head().expect_err("cat has no read loop");
    assert!(why.contains("no loop"), "{why}");
    // The vector template's `$fill` stops at a newline: every place it can stop is a run.
    let (_, shouts) = blocks_in(&asset("vector-template-wat-idea.md"));
    let shout = module(&asset("vector-template.wat"), shouts);
    assert!(shout.reads_whole());
    assert!(shout.read_loop_head().is_err());
    // The echo server is on the network.
    let echo = module(&fixture("echo", "main.wat"), blocks_of("echo").1);
    assert!(!echo.reads_whole());
    // A loop of the program's own fd_read sees how its input is cut.
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let raw = module(RAW_LOOP, reads);
    assert!(!raw.reads_whole());
    let why = raw.read_loop_head().expect_err("its reads are its own");
    assert!(why.contains("fd_read itself"), "{why}");
}

/// In a proof, `$fill` to the end of the input reads it whole, however long: cat-stream copies
/// 9000 bytes, more than two pipe writes, with no read of standard input through the world.
#[test]
fn a_fill_to_the_end_reads_input_of_any_length_whole_in_a_proof() {
    let (_, policy) = blocks_of("cat-stream");
    let module = module(&fixture("cat-stream", "main.wat"), policy);
    let text: Vec<u8> = (0..9000_usize).map(|at| b"abehlo \n"[at % 8]).collect();
    let mut pool = Pool::default();
    let stdin = exec::bytes(&mut pool, &text);
    let start = begin(&module, &mut pool, &[], &[], stdin).for_every_cut();
    let runs = exec::explore(&module, start, &mut pool, &mut exec::Concrete, 2).expect("the run");
    let [run] = runs.as_slice() else {
        panic!("{} runs", runs.len());
    };
    let Some(End::Exit(code)) = &run.world.end else {
        panic!("{:?}", run.world.end);
    };
    assert_eq!(pool.constant(*code), Some(0));
    let out: Option<Vec<u8>> = run
        .world
        .stdout
        .iter()
        .map(|byte| {
            pool.constant(*byte)
                .and_then(|value| u8::try_from(value).ok())
        })
        .collect();
    assert_eq!(out.as_deref(), Some(text.as_slice()));
    assert!(!run.world.raw_reads);
}

/// A program that reads standard input once before its read loop and again in the loop, and
/// copies the loop's pieces only where that first read got nothing: of every input longer than
/// its buffer of 64 bytes it prints the first 64 alone.
const READS_BEFORE_ITS_LOOP: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (local $first i32)
    (local $got i32)
    (local.set $first (call $fill (i32.const 0) (i32.const 1024) (i32.const 64) (i32.const -1)))
    (drop (call $write_all (i32.const 1) (i32.const 1024) (local.get $first)))
    (block $done
      (loop $piece
        (local.set $got
          (call $fill (i32.const 0) (i32.const 1024) (i32.const 64) (i32.const -1)))
        (br_if $done (i32.eqz (local.get $got)))
        (if (i32.eqz (local.get $first))
          (then (drop (call $write_all (i32.const 1) (i32.const 1024) (local.get $got)))))
        (br $piece)))
    (call $exit (i32.const 0))))
"#;

/// The runs of the first round of an induction's step on `text`: from the program's start with
/// no input, every branch taken both ways, each fed `a` and `b` at the read loop's head and
/// stopping when it comes back there.
fn step_runs(text: &str, policy: Policy) -> (Pool, Vec<State<Wasm>>) {
    let module = module(text, policy);
    let hook = Hook {
        head: module.read_loop_head().expect("a read loop"),
        stop: true,
        feed: Some(b"ab".to_vec()),
        ..Hook::default()
    };
    let mut pool = Pool::default();
    let start = begin(&module, &mut pool, &[], &[], Vec::new()).for_every_cut();
    let start = hooked(start, Rc::new(hook));
    let runs = exec::explore(&module, start, &mut pool, &mut Anything, 8).expect("the runs");
    (pool, runs)
}

/// A read loop's head must be a run's first read, as fill's own loop's is in machine code: the
/// step comes to the head with no input read, so a read before it got nothing, where a real
/// run's got the input's first bytes. The step's run of a program that reads before its loop
/// stops at the head, unfollowed.
#[test]
fn a_read_before_the_read_loops_head_stops_the_step() {
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let (_, runs) = step_runs(READS_BEFORE_ITS_LOOP, reads);
    let [run] = runs.as_slice() else {
        panic!("{} runs", runs.len());
    };
    let Some(End::Unsupported(why)) = &run.world.end else {
        panic!("{:?}", run.world.end);
    };
    assert!(why.contains("before the read loop's head"), "{why}");
}

/// A program that makes the folder `d` before its read loop and copies each piece it reads only
/// where removing `d` works: the first time round alone, so of every input longer than its
/// buffer of 64 bytes it prints the first 64 alone.
const REMOVES_A_FOLDER_IN_ITS_LOOP: &str = r#"(module
  (import "wasi_snapshot_preview1" "path_create_directory"
    (func $mkdir (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_remove_directory"
    (func $rmdir (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (data (i32.const 512) "d")
  (func (export "_start")
    (local $got i32)
    (drop (call $mkdir (i32.const 3) (i32.const 512) (i32.const 1)))
    (block $done
      (loop $piece
        (local.set $got
          (call $fill (i32.const 0) (i32.const 1024) (i32.const 64) (i32.const -1)))
        (br_if $done (i32.eqz (local.get $got)))
        (if (i32.eqz (call $rmdir (i32.const 3) (i32.const 512) (i32.const 1)))
          (then (drop (call $write_all (i32.const 1) (i32.const 1024) (local.get $got)))))
        (br $piece)))
    (call $exit (i32.const 0))))
"#;

/// What a time round the read loop does to the working folder, the next one finds there: a time
/// round that removes a folder has changed the state the loop's head stands for, which a proof
/// by induction can't take for the next time round's.
#[test]
fn a_time_round_that_changes_the_working_folder_is_a_change() {
    let files = Policy {
        standard_input: true,
        files: true,
        ..Policy::default()
    };
    let (_, runs) = step_runs(REMOVES_A_FOLDER_IN_ITS_LOOP, files);
    let [run] = runs.as_slice() else {
        panic!("{} runs", runs.len());
    };
    assert_eq!(run.world.end, Some(End::Looped));
    let changes = run.changes();
    assert_eq!(
        changes.other.first().map(String::as_str),
        Some("a time round the read loop changes the working folder"),
        "{changes:?}"
    );
}

/// cat-stream's copy, written a buffer-full at a time.
const WRITES: &str = "(call $write_all (global.get $STDOUT) (global.get $buffer) (local.get $got))";

/// The copy that drops the last byte of a full buffer-full: 4096 shifted down 12 places is 1.
const DROPS_A_FULL_BUFFERS_LAST_BYTE: &str = "(call $write_all (global.get $STDOUT) (global.get \
                                              $buffer) (i32.sub (local.get $got) (i32.shr_u \
                                              (local.get $got) (i32.const 12))))";

/// cat-stream is proven for every length by induction over its read loop, its input arriving
/// cut any way: from any state the loop can be in at its head, one more buffer-full of 4096
/// bytes is copied out, and every input of up to a buffer-full is proven from there. A copy that
/// drops a byte only of a full buffer-full, which no input shorter than one shows, is found.
#[test]
fn proves_a_streaming_copy_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("cat-stream");
    let source = fixture("cat-stream", "main.wat");
    let built = build(&source).expect("cat-stream builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                induction: Some(4096),
                cut_any_way: true,
                ..
            }
        ),
        "{verdict:?}"
    );
    let short = edited(&source, (WRITES, DROPS_A_FULL_BUFFERS_LAST_BYTE));
    let built = build(&short).expect("it builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("a dropped byte: {verdict:?}");
    };
    let Some(Value::Str(text)) = found.get("text") else {
        panic!("a dropped byte: {found:?}");
    };
    assert!(text.len() >= 4096, "a dropped byte: {} bytes", text.len());
}

/// The program that reads before its loop, which an induction whose step began with that read
/// getting nothing would prove for every length: proven by length, the bytes past its first
/// buffer-full it drops are found.
#[test]
fn finds_the_bytes_a_read_before_the_read_loop_drops() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("cat-stream");
    let built = build(READS_BEFORE_ITS_LOOP).expect("it builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("a read before the loop: {verdict:?}");
    };
    let Some(Value::Str(text)) = found.get("text") else {
        panic!("a read before the loop: {found:?}");
    };
    assert!(text.len() > 64, "{} bytes", text.len());
}

/// The program that removes a folder in its read loop, which an induction blind to the working
/// folder would prove for every length: proven by length, the bytes past its first buffer-full
/// it drops are found. Its texts are of a byte or more, so the folder is always removed: after
/// an empty one it stays, which the block doesn't allow.
#[test]
fn finds_the_bytes_a_folder_removed_in_the_read_loop_drops() {
    if !solvers_installed() {
        return;
    }
    let (blocks, _) = blocks_of("cat-stream");
    let mut block = blocks[0].clone();
    for (_, generator) in &mut block.vars {
        if let Generator::Str { min, max, .. } = generator {
            (*min, *max) = (1, 200);
        }
    }
    let files = Policy {
        standard_input: true,
        files: true,
        ..Policy::default()
    };
    let built = build(REMOVES_A_FOLDER_IN_ITS_LOOP).expect("it builds");
    let verdict = prove_wasm(&built, files, &block, &stream_budget(900));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("a folder removed in the loop: {verdict:?}");
    };
    let Some(Value::Str(text)) = found.get("text") else {
        panic!("a folder removed in the loop: {found:?}");
    };
    assert!(text.len() > 64, "{} bytes", text.len());
}

/// The stream template with a buffer of 256 bytes, so the base's lengths go by quickly, as the
/// native prover's own tests shrank theirs.
const STREAM_AT_256: [(&str, &str); 1] = [(
    "(global $buffer.size i32 (i32.const 65536))",
    "(global $buffer.size i32 (i32.const 256))",
)];

/// The stream template's letters, which a wrong bound stops at `y`.
const LETTERS: (&str, &str) = (
    "(global $LETTERS i32 (i32.const 26))",
    "(global $LETTERS i32 (i32.const 25))",
);

/// The stream template, which capitalizes its input a buffer-full at a time through `$fill`, is
/// proven for every length its loop's bound allows, far past what a proof by length reaches, as
/// the native template was; capitals that stop at `y` are still found.
#[test]
fn proves_the_stream_template_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_in(&asset("stream-template-wat-idea.md"));
    let source = with_edits(&asset("stream-template.wat"), &STREAM_AT_256);
    let built = build(&source).expect("the template builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                induction: Some(256),
                ..
            }
        ),
        "{verdict:?}"
    );
    let wrong = edited(&source, LETTERS);
    let built = build(&wrong).expect("it builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    assert!(
        matches!(verdict, Verdict::Counterexample(_)),
        "capitals that stop at y: {verdict:?}"
    );
}

/// cat reads its whole input with one `$fill`, so no read loop: its proof goes by length, and
/// now every length its idea allows, 5000 bytes, past what one pipe write holds, however the
/// input arrives.
#[test]
fn proves_one_whole_read_for_every_length_its_idea_allows() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("cat");
    let built = build(&fixture("cat", "main.wat")).expect("cat builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                induction: None,
                cut_any_way: true,
                ..
            }
        ),
        "{verdict:?}"
    );
}

/// Checks of WebAssembly's floats in the model, each an i32 that is 1 when it gets them right:
/// rounding to nearest, ties to even; every NaN an operation gives canonical, the positive quiet
/// one with no payload, as lengua's engine makes it; `abs`, `neg` and `copysign` on the sign bit
/// alone, a NaN's too; `min` and `max` with a NaN and with zeros of both signs; the roundings;
/// the comparisons, a NaN unordered; the conversions both ways, `trunc_sat` saturating and 0 for
/// a NaN; and a float's bits through memory.
const FLOAT_CHECKS: &[&str] = &[
    "(f64.eq (f64.add (f64.const 0.1) (f64.const 0.2)) (f64.const 0x1.3333333333334p-2))",
    "(f32.eq (f32.mul (f32.const 3) (f32.const 0.5)) (f32.const 1.5))",
    "(i64.eq (i64.reinterpret_f64 (f64.add (f64.const nan:0x4) (f64.const 1))) (i64.const \
     0x7ff8000000000000))",
    "(i64.eq (i64.reinterpret_f64 (f64.div (f64.const 0) (f64.const 0))) (i64.const \
     0x7ff8000000000000))",
    "(i32.eq (i32.reinterpret_f32 (f32.sqrt (f32.const -1))) (i32.const 0x7fc00000))",
    "(i64.eq (i64.reinterpret_f64 (f64.neg (f64.const nan:0x4))) (i64.const \
     0xfff0000000000004))",
    "(i64.eq (i64.reinterpret_f64 (f64.abs (f64.const -nan))) (i64.const 0x7ff8000000000000))",
    "(f64.eq (f64.copysign (f64.const 3) (f64.const -0)) (f64.const -3))",
    "(i64.eq (i64.reinterpret_f64 (f64.min (f64.const 0) (f64.const -0))) (i64.const \
     0x8000000000000000))",
    "(i64.eq (i64.reinterpret_f64 (f64.max (f64.const -0) (f64.const 0))) (i64.const 0))",
    "(i64.eq (i64.reinterpret_f64 (f64.min (f64.const 1) (f64.const nan))) (i64.const \
     0x7ff8000000000000))",
    "(f64.eq (f64.max (f64.const 1) (f64.const 2)) (f64.const 2))",
    "(f32.eq (f32.min (f32.const -1) (f32.const 2)) (f32.const -1))",
    "(f64.eq (f64.nearest (f64.const 2.5)) (f64.const 2))",
    "(f64.eq (f64.nearest (f64.const 3.5)) (f64.const 4))",
    "(i64.eq (i64.reinterpret_f64 (f64.nearest (f64.const -0.5))) (i64.const \
     0x8000000000000000))",
    "(f64.eq (f64.ceil (f64.const 1.2)) (f64.const 2))",
    "(f64.eq (f64.floor (f64.const -1.2)) (f64.const -2))",
    "(f64.eq (f64.trunc (f64.const -1.7)) (f64.const -1))",
    "(f64.ne (f64.const nan) (f64.const nan))",
    "(i32.eqz (f64.eq (f64.const nan) (f64.const nan)))",
    "(f64.eq (f64.const 0) (f64.const -0))",
    "(f64.lt (f64.const -1) (f64.const 0))",
    "(i32.eqz (f64.lt (f64.const nan) (f64.const 0)))",
    "(i32.eqz (f64.ge (f64.const nan) (f64.const 0)))",
    "(f64.le (f64.const 1) (f64.const 1))",
    "(f64.ge (f64.const 2) (f64.const 1))",
    "(f64.gt (f64.const 2) (f64.const 1))",
    "(f64.eq (f64.convert_i32_s (i32.const -1)) (f64.const -1))",
    "(f64.eq (f64.convert_i32_u (i32.const -1)) (f64.const 4294967295))",
    "(f64.eq (f64.convert_i64_u (i64.const -1)) (f64.const 18446744073709551616))",
    "(f64.eq (f64.convert_i64_s (i64.const -9)) (f64.const -9))",
    "(f32.eq (f32.convert_i64_s (i64.const 16777217)) (f32.const 16777216))",
    "(f32.eq (f32.convert_i32_u (i32.const -1)) (f32.const 4294967296))",
    "(f32.eq (f32.demote_f64 (f64.const 0.1)) (f32.const 0.1))",
    "(i32.eq (i32.reinterpret_f32 (f32.demote_f64 (f64.const nan:0x4))) (i32.const \
     0x7fc00000))",
    "(f64.eq (f64.promote_f32 (f32.const 0.5)) (f64.const 0.5))",
    "(i32.eq (i32.trunc_sat_f64_s (f64.const nan)) (i32.const 0))",
    "(i32.eq (i32.trunc_sat_f64_s (f64.const 1e10)) (i32.const 2147483647))",
    "(i32.eq (i32.trunc_sat_f64_s (f64.const -1e10)) (i32.const -2147483648))",
    "(i32.eq (i32.trunc_sat_f64_s (f64.const -2.9)) (i32.const -2))",
    "(i32.eq (i32.trunc_sat_f64_u (f64.const -2.9)) (i32.const 0))",
    "(i32.eq (i32.trunc_sat_f64_u (f64.const 4294967296)) (i32.const -1))",
    "(i32.eq (i32.trunc_sat_f64_u (f64.const 3e9)) (i32.const 3000000000))",
    "(i64.eq (i64.trunc_sat_f64_u (f64.const 18446744073709551616)) (i64.const -1))",
    "(i64.eq (i64.trunc_sat_f64_u (f64.const 9.3e18)) (i64.const 9300000000000000000))",
    "(i64.eq (i64.trunc_sat_f32_s (f32.const -inf)) (i64.const -9223372036854775808))",
    "(i64.eq (i64.trunc_sat_f64_s (f64.nearest (f64.const 1234.5))) (i64.const 1234))",
    "(i32.eq (i32.reinterpret_f32 (f32.const 1)) (i32.const 0x3f800000))",
    "(f64.store (i32.const 1024) (f64.const 1.5)) (f64.eq (f64.load (i32.const 1024)) \
     (f64.const 1.5))",
];

/// Checks of SIMD128's operators in the model, each an i32 that is 1 when it gets them right:
/// splats and lanes, a vector's bits through memory, the bitwise operators and tests, bitmasks,
/// lane arithmetic, wrapping and saturating, the comparisons, shifts modulo the lane's width,
/// and the shuffles.
const VECTOR_CHECKS: &[&str] = &[
    "(i32.eq (i8x16.extract_lane_u 3 (i8x16.splat (i32.const 0x1ff))) (i32.const 0xff))",
    "(i32.eq (i8x16.extract_lane_s 3 (i8x16.splat (i32.const 0x1ff))) (i32.const -1))",
    "(i32.eq (i16x8.extract_lane_u 7 (i16x8.splat (i32.const 0x18000))) (i32.const 0x8000))",
    "(i64.eq (i64x2.extract_lane 1 (i64x2.splat (i64.const -3))) (i64.const -3))",
    "(f64.eq (f64x2.extract_lane 1 (f64x2.splat (f64.const 1.5))) (f64.const 1.5))",
    "(i32.eq (i32x4.extract_lane 2 (i32x4.replace_lane 2 (i32x4.splat (i32.const 0)) (i32.const \
     7))) (i32.const 7))",
    "(v128.store (i32.const 1024) (v128.const i32x4 1 2 3 4)) (i32.eq (i32.load offset=8 \
     (i32.const 1024)) (i32.const 3))",
    "(i64.store (i32.const 1024) (i64.const 0x0807060504030201)) (i32.eq (i8x16.extract_lane_u \
     5 (v128.load (i32.const 1024))) (i32.const 6))",
    "(i32.eq (i8x16.bitmask (i8x16.eq (v128.const i8x16 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 \
     16) (i8x16.splat (i32.const 3)))) (i32.const 4))",
    "(i32.eq (i16x8.bitmask (v128.const i16x8 -1 0 -1 0 0 0 0 -1)) (i32.const 0x85))",
    "(i32.eq (i32x4.bitmask (v128.const i32x4 0 -1 0 -1)) (i32.const 10))",
    "(i32.eq (i32x4.extract_lane 1 (i32x4.add (v128.const i32x4 1 2 3 4) (v128.const i32x4 10 \
     20 30 40))) (i32.const 22))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.sub (i8x16.splat (i32.const 1)) (i8x16.splat \
     (i32.const 2)))) (i32.const 255))",
    "(i32.eq (i16x8.extract_lane_u 0 (i16x8.mul (i16x8.splat (i32.const 300)) (i16x8.splat \
     (i32.const 300)))) (i32.const 24464))",
    "(i32.eq (i8x16.extract_lane_s 0 (i8x16.add_sat_s (i8x16.splat (i32.const 100)) \
     (i8x16.splat (i32.const 100)))) (i32.const 127))",
    "(i32.eq (i8x16.extract_lane_s 0 (i8x16.sub_sat_s (i8x16.splat (i32.const -100)) \
     (i8x16.splat (i32.const 100)))) (i32.const -128))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.add_sat_u (i8x16.splat (i32.const 200)) \
     (i8x16.splat (i32.const 100)))) (i32.const 255))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.sub_sat_u (i8x16.splat (i32.const 1)) \
     (i8x16.splat (i32.const 2)))) (i32.const 0))",
    "(i32.eq (i8x16.extract_lane_s 0 (i8x16.min_s (i8x16.splat (i32.const -1)) (i8x16.splat \
     (i32.const 1)))) (i32.const -1))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.min_u (i8x16.splat (i32.const -1)) (i8x16.splat \
     (i32.const 1)))) (i32.const 1))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.avgr_u (i8x16.splat (i32.const 255)) \
     (i8x16.splat (i32.const 254)))) (i32.const 255))",
    "(i32.eq (i8x16.extract_lane_s 0 (i8x16.abs (i8x16.splat (i32.const -128)))) (i32.const \
     -128))",
    "(i32.eq (i32x4.extract_lane 0 (i32x4.neg (i32x4.splat (i32.const 5)))) (i32.const -5))",
    "(i32.eq (i8x16.bitmask (i8x16.lt_u (v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 \
     15) (i8x16.splat (i32.const 4)))) (i32.const 15))",
    "(i32.eq (i8x16.bitmask (i8x16.le_u (v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 \
     15) (i8x16.splat (i32.const 4)))) (i32.const 31))",
    "(i32.eq (i8x16.bitmask (i8x16.gt_s (v128.const i8x16 0 -1 2 -3 4 -5 6 -7 8 -9 10 -11 12 \
     -13 14 -15) (i8x16.splat (i32.const 0)))) (i32.const 0x5554))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.shl (i8x16.splat (i32.const 1)) (i32.const 9))) \
     (i32.const 2))",
    "(i32.eq (i8x16.extract_lane_s 0 (i8x16.shr_s (i8x16.splat (i32.const 0x80)) (i32.const \
     7))) (i32.const -1))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.shr_u (i8x16.splat (i32.const 0x80)) (i32.const \
     7))) (i32.const 1))",
    "(i32.eq (i32x4.extract_lane 3 (i32x4.shl (i32x4.splat (i32.const 1)) (i32.const 33))) \
     (i32.const 2))",
    "(v128.any_true (v128.const i64x2 0 1))",
    "(i32.eqz (v128.any_true (v128.const i64x2 0 0)))",
    "(i32.eqz (i32x4.all_true (v128.const i32x4 1 2 0 4)))",
    "(i8x16.all_true (i8x16.splat (i32.const 1)))",
    "(i32.eq (i32x4.extract_lane 0 (v128.bitselect (i32x4.splat (i32.const 0xff00ff00)) \
     (i32x4.splat (i32.const 0x00ff00ff)) (i32x4.splat (i32.const 0xffff0000)))) (i32.const \
     0xff0000ff))",
    "(i32.eq (i32x4.extract_lane 0 (v128.andnot (i32x4.splat (i32.const 0xff)) (i32x4.splat \
     (i32.const 0x0f)))) (i32.const 0xf0))",
    "(i32.eq (i32x4.extract_lane 0 (v128.xor (v128.not (i32x4.splat (i32.const 0))) \
     (i32x4.splat (i32.const 1)))) (i32.const -2))",
    "(i32.eq (i8x16.extract_lane_u 5 (i8x16.shuffle 0 1 2 3 4 21 6 7 8 9 10 11 12 13 14 15 \
     (i8x16.splat (i32.const 1)) (i8x16.splat (i32.const 2)))) (i32.const 2))",
    "(i32.eq (i8x16.extract_lane_u 0 (i8x16.swizzle (v128.const i8x16 10 11 12 13 14 15 16 17 \
     18 19 20 21 22 23 24 25) (v128.const i8x16 3 200 0 0 0 0 0 0 0 0 0 0 0 0 0 0))) \
     (i32.const 13))",
    "(i32.eq (i8x16.extract_lane_u 1 (i8x16.swizzle (v128.const i8x16 10 11 12 13 14 15 16 17 \
     18 19 20 21 22 23 24 25) (v128.const i8x16 3 200 0 0 0 0 0 0 0 0 0 0 0 0 0 0))) \
     (i32.const 0))",
];

#[test]
fn floats_and_vectors_follow_webassemblys_semantics() {
    let mut wrong = Vec::new();
    for body in FLOAT_CHECKS.iter().chain(VECTOR_CHECKS) {
        let text = exits_with("", body);
        let done = ran(&text, Policy::default(), &Given::default());
        if done.end != Ended::Exit(1) {
            wrong.push(format!("{body}: {:?}", done.end));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// In a proof, `$fill` to a newline stops after any newline that has arrived, as a read can end
/// there, or reads on to the end: a run each, the one that stops short noting where.
#[test]
fn a_fill_to_a_stop_byte_stops_wherever_a_read_can_end() {
    let reads = Policy {
        standard_input: true,
        ..Policy::default()
    };
    let module = module(LINE, reads);
    let mut pool = Pool::default();
    let stdin = exec::bytes(&mut pool, b"ab\ncd\n");
    let start = begin(&module, &mut pool, &[], &[], stdin).for_every_cut();
    let runs = exec::explore(&module, start, &mut pool, &mut Anything, 8).expect("the runs");
    let mut outputs: Vec<(Vec<u8>, Option<usize>)> = runs
        .iter()
        .map(|run| {
            let out = run
                .world
                .stdout
                .iter()
                .filter_map(|byte| {
                    pool.constant(*byte)
                        .and_then(|value| u8::try_from(value).ok())
                })
                .collect();
            (out, run.world.fill_cut)
        })
        .collect();
    outputs.sort();
    assert_eq!(
        outputs,
        [(b"ab\n".to_vec(), Some(3)), (b"ab\ncd\n".to_vec(), None)]
    );
}

/// A file of a fixture with `edits` made.
fn edited_fixture(name: &str, edits: &[(&str, &str)]) -> String {
    with_edits(&fixture(name, "main.wat"), edits)
}

/// The line counter with a buffer of 64 bytes, four vectors of 16, so the base's every final
/// piece goes by quickly: the model merges no loop's exits, so a piece's length is a path.
const LINES_AT_64: [(&str, &str); 3] = [
    (
        "(global $buffer.size i32 (i32.const 4096))",
        "(global $buffer.size i32 (i32.const 64))",
    ),
    (
        "(global $CHUNKS i32 (i32.const 256))",
        "(global $CHUNKS i32 (i32.const 4))",
    ),
    (
        "(global $LAST_CHUNK i32 (i32.const 4080))",
        "(global $LAST_CHUNK i32 (i32.const 48))",
    ),
];

/// The line counter that keeps the last buffer-full's count alone.
const COUNTS_THE_LAST_FULL_ALONE: (&str, &str) = (
    "(i64.add (local.get $lines)\n                (i64.extend_i32_u (call $count_lines \
     (local.get $got)))))",
    "(i64.extend_i32_u (call $count_lines (local.get $got))))",
);

/// Induction over counts in WebAssembly: a line count kept in an i64 local, grown by each
/// buffer-full's newlines, counted 16 bytes at a time with SIMD128's compare and bitmask, is
/// proven for every length the idea allows; a count of the last buffer-full alone, which looks
/// like the count so far only the first time round, has no induction.
#[test]
fn proves_a_vector_line_count_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("lines");
    let source = edited_fixture("lines", &LINES_AT_64);
    let built = build(&source).expect("lines builds");
    let verdict = prove_wasm(&built, policy, &blocks[0], &stream_budget(900));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                induction: Some(64),
                ..
            }
        ),
        "{verdict:?}"
    );
    let last_only = edited(&source, COUNTS_THE_LAST_FULL_ALONE);
    let built = build(&last_only).expect("it builds");
    let module = Module::new(&built, policy).expect("the model");
    let step = crate::prove::induction::step(&module, &blocks[0], &stream_budget(300));
    assert!(step.is_err(), "{step:?}");
}

/// The word and line counter with a buffer of 64 bytes, as [`LINES_AT_64`].
const COUNT_AT_64: [(&str, &str); 3] = [
    (
        "(global $buffer.size i32 (i32.const 65536))",
        "(global $buffer.size i32 (i32.const 64))",
    ),
    (
        "(global $CHUNKS i32 (i32.const 4096))",
        "(global $CHUNKS i32 (i32.const 4))",
    ),
    (
        "(global $LAST_CHUNK i32 (i32.const 65520))",
        "(global $LAST_CHUNK i32 (i32.const 48))",
    ),
];

/// count, whose words in a buffer-full depend on whether the one before ended inside a word,
/// carries the last byte of one buffer-full to the next, which its `$count_piece` asserts is a
/// byte. The induction's step makes it any value at the read loop's head, and from there the
/// assertion fails, so the step doesn't hold and `wc -lw` in SIMD128 is proven by length, as far
/// as a proof's time goes: here every text of up to a buffer-full and a byte, the byte carried
/// from the first piece to the second. The native count keeps a flag in a register it never
/// asserts.
#[test]
fn proves_a_vector_word_count_by_length_as_its_flag_is_asserted() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("count");
    let built = build(&edited_fixture("count", &COUNT_AT_64)).expect("count builds");
    let mut block = blocks[0].clone();
    for (_, generator) in &mut block.vars {
        if let Generator::Str { max, .. } = generator {
            *max = 65;
        }
    }
    let verdict = prove_wasm(&built, policy, &block, &stream_budget(1200));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                induction: None,
                ..
            }
        ),
        "{verdict:?}"
    );
}

/// The hypotenuse computes in doubles, which the model takes as the engine does, and every block
/// of it is proven for every input in range: the whole sides and the decimal ones, the usage
/// line, a side that isn't a number and one out of range.
#[test]
fn proves_the_hypotenuse_in_doubles() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_of("hypotenuse");
    let built = build(&fixture("hypotenuse", "main.wat")).expect("hypotenuse builds");
    for (number, block) in blocks.iter().enumerate() {
        let verdict = prove_wasm(&built, policy, block, &stream_budget(900));
        assert!(proven(&verdict), "hypotenuse's block {number}: {verdict:?}");
    }
}

/// The vector template finds its line's newline 16 bytes at a time and capitalizes it 16 bytes
/// at a time: every generated block of it is proven for every input in range, however its input
/// arrives, its `$fill` stopping wherever a read can end after a newline.
#[test]
fn proves_the_vector_templates_behaviors() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_in(&asset("vector-template-wat-idea.md"));
    let built = build(&asset("vector-template.wat")).expect("the template builds");
    for (number, block) in blocks.iter().enumerate() {
        let verdict = prove_wasm(&built, policy, block, &stream_budget(900));
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    cut_any_way: true,
                    ..
                }
            ),
            "the vector template's block {number}: {verdict:?}"
        );
    }
}
