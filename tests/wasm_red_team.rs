//! The red team for the WebAssembly dialect: programs written to get past lengua's own checks,
//! which the checker must refuse up front, before anything runs. Each attempt is the WebAssembly
//! main template with a change that tries it, or a module of its own: an import the idea doesn't
//! allow, an indirect call through a table, `memory.grow`, a start function, an extra export, a
//! store into a read-only region, an access past its region, an index nothing checks, lengua's
//! addresses below 1024, recursion and mutual recursion, a loop without a bound, a trapping
//! conversion, an unguarded division, relaxed SIMD and atomics, a name lengua keeps, an exit
//! status lengua keeps, and an ignored result.
//!
//! An attempt is refused either by the build, whose validator holds a module to the engine's
//! features, or by a rule, which names what it found on main.wat's line. Any attempt that isn't
//! refused is a bug in lengua to fix before anything else. Nothing here needs more than this
//! crate, so every attempt runs on every computer.

use lengua::checker::Rule;
use lengua::checker::wasm::{Context, check};
use lengua::idea::Network;
use lengua::wasm::text::build;

/// The WebAssembly main template, which most attempts change.
const TEMPLATE: &str = include_str!("../plugin/skills/compile/assets/main-template.wat");

/// A change to the template: the first `from` in it becomes `to`.
type Edit = (&'static str, &'static str);

/// Who must refuse an attempt.
#[derive(Debug, Clone, Copy)]
enum Refusal {
    /// The build, with a problem whose message has this in it.
    Build(&'static str),
    /// The checker, with a violation of this rule whose report line has this in it.
    Rule(Rule, &'static str),
}

/// One attempt.
struct Attempt {
    /// What it tries.
    name: &'static str,
    /// Its changes to the template; none when `module` is the whole text.
    edits: &'static [Edit],
    /// A module of its own, instead of the template.
    module: Option<&'static str>,
    /// Who refuses it, and with what.
    refusal: Refusal,
}

/// main.wat's text for `attempt`.
fn text_of(attempt: &Attempt) -> String {
    if let Some(module) = attempt.module {
        return module.to_owned();
    }
    let mut text = TEMPLATE.to_owned();
    for (from, to) in attempt.edits {
        assert!(
            text.contains(from),
            "{}: {from:?} isn't in the template",
            attempt.name
        );
        text = text.replacen(from, to, 1);
    }
    text
}

/// Asserts that `attempt` is refused as it says.
fn assert_refused(attempt: &Attempt) {
    let text = text_of(attempt);
    let built = build(&text);
    match (attempt.refusal, built) {
        (Refusal::Build(needle), Err(problem)) => assert!(
            problem.message.contains(needle),
            "{}: the build refused it, but with {problem}",
            attempt.name
        ),
        (Refusal::Build(needle), Ok(_)) => panic!(
            "{}: the build took it, where it must refuse it with {needle:?}",
            attempt.name
        ),
        (Refusal::Rule(_, _), Err(problem)) => panic!(
            "{}: the build refused it ({problem}), where a rule must",
            attempt.name
        ),
        (Refusal::Rule(rule, needle), Ok(built)) => {
            let network = Network::default();
            let report = check(
                &text,
                &built,
                &Context {
                    uses: &[],
                    network: &network,
                },
            );
            assert!(!report.passed(), "{}: not refused", attempt.name);
            let found = report.violations.iter().any(|violation| {
                violation.rule == rule && violation.in_file("main.wat").contains(needle)
            });
            assert!(
                found,
                "{}: no {rule:?} violation with {needle:?} in {:#?}",
                attempt.name,
                report.lines()
            );
        }
    }
}

/// The attempts, each with what refuses it.
const ATTEMPTS: &[Attempt] = &[
    Attempt {
        name: "an import the idea doesn't allow",
        edits: &[(
            "  (memory (export \"memory\") 1 1)",
            "  (import \"wasi_snapshot_preview1\" \"path_open\" (func $open (param i32 i32 i32 i32 \
             i32 i64 i64 i32 i32) (result i32)))\n  (memory (export \"memory\") 1 1)",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Resources, "`path_open` isn't one the idea allows"),
    },
    Attempt {
        name: "an import from another module",
        edits: &[(
            "  (memory (export \"memory\") 1 1)",
            "  (import \"env\" \"system\" (func $system (param i32) (result i32)))\n  (memory \
             (export \"memory\") 1 1)",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Resources, "imported from `env`"),
    },
    Attempt {
        name: "an indirect call through a table",
        edits: &[
            (
                "  (memory (export \"memory\") 1 1)",
                "  (memory (export \"memory\") 1 1)\n  (table 1 funcref)\n  (elem (i32.const 0) \
                 $write_i64)",
            ),
            (
                "(call $write_i64 (i64.sub (i64.const 0) (local.get $n)))",
                "(call_indirect (param i64) (result i32) (i64.sub (i64.const 0) (local.get $n)) \
                 (global.get $NO_ERROR))",
            ),
        ],
        module: None,
        refusal: Refusal::Rule(Rule::Indirection, "a table"),
    },
    Attempt {
        name: "memory.grow",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (drop (memory.grow (global.get \
             $NO_ERROR)))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Resources, "`memory.grow`"),
    },
    Attempt {
        name: "a start function",
        edits: &[(
            "  (memory (export \"memory\") 1 1)",
            "  (memory (export \"memory\") 1 1)\n  (start $main)",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Indirection, "a start function"),
    },
    Attempt {
        name: "an extra export",
        edits: &[(
            "  (memory (export \"memory\") 1 1)",
            "  (memory (export \"memory\") 1 1)\n  (export \"parse\" (func $parse_i64))",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Scope, "exports `parse`"),
    },
    Attempt {
        name: "a store into a read-only region",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (i32.store8 (global.get $nan) \
             (global.get $MINUS))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Immutability, "writes to $nan, a read-only region"),
    },
    Attempt {
        name: "an access past its region",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (i64.store offset=16 (global.get \
             $out) (local.get $value))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Bounds, "reaches byte 23 of $out, which has 21 bytes"),
    },
    Attempt {
        name: "an index out of its region",
        edits: &[(
            "          (if (i32.ge_u (local.get $i) (global.get $args.size))\n            (then \
             (call $assert_fail (i32.const 4)) unreachable))\n",
            "",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Indirection, "isn't bounds-checked first"),
    },
    Attempt {
        name: "an index a parameter gives, never checked",
        edits: &[(
            "    (if (i32.ge_u (local.get $i) (global.get $args.size))\n      (then (call \
             $assert_fail (i32.const 3)) unreachable))\n",
            "",
        )],
        module: None,
        refusal: Refusal::Rule(
            Rule::Indirection,
            "indexes $args with $i, which isn't bounds-checked first",
        ),
    },
    Attempt {
        name: "a store into a read-only region at an index a parameter gives",
        edits: &[(
            "    (local.set $status (global.get $ERROR_INVALID))\n",
            "    (local.set $status (global.get $ERROR_INVALID))\n    (i32.store8 (i32.add \
             (global.get $nan) (local.get $at)) (global.get $MINUS))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Immutability, "writes to $nan, a read-only region"),
    },
    Attempt {
        name: "an address made of two regions' addresses added",
        edits: &[(
            "    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))\n",
            "    (i32.store8 (i32.add (global.get $out) (global.get $out)) (global.get \
             $NEWLINE))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Owners, "made from a plain number"),
    },
    Attempt {
        name: "a store through one of two regions, one of them read-only",
        edits: &[(
            "    (i32.store8 (i32.add (global.get $out) (local.get $at)) (global.get $NEWLINE))\n",
            "    (i32.store8 (select (global.get $out) (global.get $nan) (local.get $at)) \
             (global.get $NEWLINE))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Immutability, "writes to $nan, a read-only region"),
    },
    Attempt {
        name: "a check of an index kept, then the index changed before the check is used",
        edits: &[(
            "    (local.set $i (local.get $at))\n    (if (i32.ge_u (local.get $i) (global.get \
             $args.size))\n      (then (call $assert_fail (i32.const 3)) unreachable))\n",
            "    (local.set $c (i32.ge_u (local.get $i) (global.get $args.size)))\n    \
             (local.set $i (local.get $at))\n    (if (local.get $c) (then (call $assert_fail \
             (i32.const 3)) unreachable))\n",
        )],
        module: None,
        refusal: Refusal::Rule(
            Rule::Indirection,
            "indexes $args with $i, which isn't bounds-checked first",
        ),
    },
    Attempt {
        name: "a divisor checked not zero, then changed before the division",
        edits: &[(
            "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n",
            "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n          \
             (local.set $err (i32.eqz (local.get $size)))\n          (local.set $size (i32.load \
             (global.get $sizes)))\n          (br_if $not_a_number (local.get $err))\n          \
             (local.set $err (i32.div_u (global.get $args.size) (local.get $size)))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Dialect, "nothing checks isn't zero"),
    },
    Attempt {
        name: "a callee's comparison taken as a check of the caller's own local",
        edits: &[
            (
                "  ;; Writes $value in decimal",
                "  (func $small (param $k i32) (result i32)\n    (i32.lt_u (local.get $k) \
                 (global.get $args.size)))\n\n  ;; Writes $value in decimal",
            ),
            (
                "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n",
                "          (local.set $size (i32.load offset=4 (global.get $sizes)))\n          \
                 (local.set $err (i32.load (global.get $sizes)))\n          (if (call $small \
                 (local.get $size))\n            (then (drop (i32.load8_u (i32.add (global.get \
                 $args) (local.get $err))))))\n",
            ),
        ],
        module: None,
        refusal: Refusal::Rule(
            Rule::Indirection,
            "indexes $args with $err, which isn't bounds-checked first",
        ),
    },
    Attempt {
        name: "an address a loop passes back to itself as its parameter",
        edits: &[
            (
                "    (local $n i64)\n    (block $write_failed",
                "    (local $n i64)\n    (local $left i32)\n    (local $p i32)\n    (block \
                 $write_failed",
            ),
            (
                "          (call $proc_exit (global.get $EXIT_OK))\n",
                "          (global.get $number)\n          (local.set $left (global.get \
                 $ARGC_EXPECTED))\n          (loop $poke (param i32) (result i32)  ;; \
                 bound: 2\n            (local.set $p)\n            (i32.store8 (local.get $p) \
                 (global.get $MINUS))\n            (global.get $usage)\n            \
                 (local.set $left (i32.sub (local.get $left) (i32.const 1)))\n            \
                 (br_if $poke (local.get $left)))\n          (drop)\n          (call $proc_exit \
                 (global.get $EXIT_OK))\n",
            ),
        ],
        module: None,
        refusal: Refusal::Rule(Rule::Immutability, "writes to $usage, a read-only region"),
    },
    Attempt {
        name: "lengua's addresses below 1024, through a plain number",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (i32.store (global.get $NO_ERROR) \
             (global.get $NO_ERROR))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Owners, "a plain number"),
    },
    Attempt {
        name: "lengua's addresses below 1024, through a region placed there",
        edits: &[(
            "  (global $sizes i32 (i32.const 2048))",
            "  (global $sizes i32 (i32.const 0))",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Owners, "inside lengua's addresses 0 to 1023"),
    },
    Attempt {
        name: "recursion",
        edits: &[(
            "    (local.set $status (global.get $ERROR_INVALID))\n",
            "    (local.set $status (global.get $ERROR_INVALID))\n    (drop (call $parse_i64 \
             (local.get $at) (local.get $into)))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::ControlFlow, "recursion"),
    },
    Attempt {
        name: "mutual recursion",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (call $main)\n",
        )],
        module: None,
        refusal: Refusal::Rule(
            Rule::ControlFlow,
            "which leads back to $write_i64: recursion",
        ),
    },
    Attempt {
        name: "a loop without a bound",
        edits: &[("(loop $digit  ;; bound: 20", "(loop $digit")],
        module: None,
        refusal: Refusal::Rule(Rule::Loops, "no `;; bound: N`"),
    },
    Attempt {
        name: "a loop without a counter",
        edits: &[(
            "          (local.set $left (i32.sub (local.get $left) (i32.const 1)))\n          \
             (br_if $digit (local.get $left)))\n        ;; More characters",
            "          (br $digit))\n        ;; More characters",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Loops, "doesn't end by taking one from its counter"),
    },
    Attempt {
        name: "a trapping conversion",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (i64.trunc_f64_u (f64.convert_i64_s (local.get \
             $value))))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Dialect, "`i64.trunc_f64_u` traps"),
    },
    Attempt {
        name: "an unguarded division",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (i64.div_u (global.get $TEN) (local.get $value)))\n",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Dialect, "nothing checks isn't zero"),
    },
    Attempt {
        name: "relaxed SIMD",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (drop \
             (i32x4.relaxed_trunc_f32x4_s (v128.const f32x4 0 0 0 0)))\n",
        )],
        module: None,
        refusal: Refusal::Build("relaxed SIMD support is not enabled"),
    },
    Attempt {
        name: "atomics",
        edits: &[(
            "    (local.set $magnitude (local.get $value))\n",
            "    (local.set $magnitude (local.get $value))\n    (drop (i32.atomic.load \
             (global.get $out)))\n",
        )],
        module: None,
        refusal: Refusal::Build("threads support is not enabled"),
    },
    Attempt {
        name: "a name lengua keeps for its library, defined",
        edits: &[(
            "  ;; Writes $value in decimal",
            "  (func $assert_fail (param $number i32)\n    (call $proc_exit (global.get \
             $EXIT_OK))\n    unreachable)\n\n  ;; Writes $value in decimal",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Dialect, "a name lengua keeps"),
    },
    Attempt {
        name: "a name lengua keeps for its own imports",
        edits: &[(
            "  ;; Writes $value in decimal",
            "  (func $lengua.fd_write)\n\n  ;; Writes $value in decimal",
        )],
        module: None,
        refusal: Refusal::Build("lengua keeps"),
    },
    Attempt {
        name: "the exit status of a failed assertion",
        edits: &[(
            "(global $EXIT_USAGE i32 (i32.const 1))",
            "(global $EXIT_USAGE i32 (i32.const 134))",
        )],
        module: None,
        refusal: Refusal::Rule(
            Rule::Constants,
            "134 is the exit status of a failed assertion",
        ),
    },
    Attempt {
        name: "an ignored result",
        edits: &[(
            "(local.set $err (call $args_get (global.get $argv) (global.get $args)))",
            "(drop (call $args_get (global.get $argv) (global.get $args)))",
        )],
        module: None,
        refusal: Refusal::Rule(Rule::Results, "isn't tested: it's dropped"),
    },
    Attempt {
        name: "a memory that grows on its own",
        edits: &[],
        module: Some(
            "(module\n  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $proc_exit (param \
             i32)))\n  (memory (export \"memory\") 1)\n  (global $EXIT_OK i32 (i32.const 0))\n  \
             (func $main (export \"_start\")\n    (call $proc_exit (global.get $EXIT_OK))\n    \
             unreachable))\n",
        ),
        refusal: Refusal::Rule(Rule::Resources, "must be equal"),
    },
];

#[test]
fn every_attempt_is_refused() {
    for attempt in ATTEMPTS {
        assert_refused(attempt);
    }
}

/// The template the attempts change passes, so each refusal is the attempt's.
#[test]
fn the_template_itself_passes() {
    let built = build(TEMPLATE).unwrap_or_else(|problem| panic!("the template builds: {problem}"));
    let network = Network::default();
    let report = check(
        TEMPLATE,
        &built,
        &Context {
            uses: &[],
            network: &network,
        },
    );
    assert!(report.passed(), "{:#?}", report.lines());
}
