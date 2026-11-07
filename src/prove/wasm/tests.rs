//! The WebAssembly model on its own: WebAssembly's integer semantics, each trap at its line of
//! main.wat, lengua's library, the host's calls, and a branch, a division and a load the input
//! decides.

use std::collections::HashMap;

use super::*;
use crate::prove::exec::Oracle;
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
    ("", "(i32.eq (i32.div_s (i32.const -7) (i32.const 2)) (i32.const -3))"),
    ("", "(i32.eq (i32.rem_s (i32.const -7) (i32.const 2)) (i32.const -1))"),
    (
        "",
        "(i32.eq (i32.div_u (i32.const -1) (i32.const 2)) (i32.const 2147483647))",
    ),
    ("", "(i32.eq (i32.rem_u (i32.const -1) (i32.const 10)) (i32.const 5))"),
    ("", "(i32.eq (i32.shl (i32.const 1) (i32.const 33)) (i32.const 2))"),
    ("", "(i32.eq (i32.shr_s (i32.const -8) (i32.const 1)) (i32.const -4))"),
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
    ("", "(i32.eq (i32.popcnt (i32.const 0xff00ff)) (i32.const 16))"),
    ("", "(i32.eq (i32.extend8_s (i32.const 0x80)) (i32.const -128))"),
    ("", "(i32.eq (i32.extend16_s (i32.const 0x8000)) (i32.const -32768))"),
    ("", "(i64.eq (i64.extend_i32_s (i32.const -1)) (i64.const -1))"),
    ("", "(i64.eq (i64.extend_i32_u (i32.const -1)) (i64.const 4294967295))"),
    (
        "",
        "(i64.eq (i64.extend32_s (i64.const 0x80000000)) (i64.const -2147483648))",
    ),
    ("", "(i32.eq (i32.wrap_i64 (i64.const 0x100000005)) (i32.const 5))"),
    (
        "",
        "(i64.eq (i64.rem_s (i64.const -9223372036854775808) (i64.const -1)) (i64.const 0))",
    ),
    ("", "(i64.eq (i64.div_s (i64.const -9) (i64.const 4)) (i64.const -2))"),
    ("", "(i64.eq (i64.clz (i64.const 1)) (i64.const 63))"),
    ("", "(i64.eq (i64.ctz (i64.const 0x100000000)) (i64.const 32))"),
    ("", "(i64.eq (i64.popcnt (i64.const -1)) (i64.const 64))"),
    ("", "(i64.eq (i64.rotl (i64.const 1) (i64.const 65)) (i64.const 2))"),
    ("", "(i64.eq (i64.shr_u (i64.const -1) (i64.const 127)) (i64.const 1))"),
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
            "(module\n  (memory (export \"memory\") 1 1)\n  (func $down\n    (call $down))\n  \
             (func (export \"_start\")\n    (call $down)))\n"
                .to_owned(),
            4,
            "its calls went deeper than the stack holds",
        ),
        (
            "(module\n  (import \"wasi_snapshot_preview1\" \"fd_write\"\n    (func $fd_write (param \
             i32 i32 i32 i32) (result i32)))\n  (memory (export \"memory\") 1 1)\n  (func (export \
             \"_start\")\n    (drop (call $fd_write (i32.const 1) (i32.const 65532) (i32.const 1) \
             (i32.const 0)))))\n"
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
    let runs =
        exec::explore(&module, start, &mut pool, &mut exec::Concrete, 4).expect("the runs");
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
