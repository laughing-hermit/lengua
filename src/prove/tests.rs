//! The proof driver on its own, and how a proof keeps its time and starts its solvers on
//! WebAssembly programs: a range of numbers in digit pieces, every combination of the inputs'
//! parts once, by its longest text, and conditions read as intervals that settle only what holds;
//! then a shape whose terms settle it, which starts no solver, the model's runs of the tests'
//! samples, which stop once the proof's time is up, and a proof that keeps its time within one
//! long shape.

use std::time::{Duration, Instant};

use super::*;
use crate::wasm::Policy;
use crate::wasm::text::build;

/// A file of the repository, by its path from the repository's root.
fn repository_file(path: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The generated blocks of the idea `text`, and what the idea lets its program reach.
fn blocks_in(text: &str) -> (Vec<Generated>, Policy) {
    let idea = crate::idea::parse(text).unwrap_or_else(|problems| panic!("{problems:?}"));
    let suite = crate::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    (suite.generated, Policy::of(&idea.uses, &idea.network))
}

/// A proof's budget: at most `seconds`.
fn budget(seconds: u64) -> Budget {
    Budget {
        max_paths: 64,
        max_shapes: 2000,
        ..Budget::until(Instant::now() + Duration::from_secs(seconds), false)
    }
}

/// Whether z3 is installed, which a proof that asks the solvers needs: a test without it says so
/// and passes.
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

#[test]
fn splits_ranges_into_digit_pieces() {
    let pieces = cover(&digits_of(1000, 4), &digits_of(1234, 4));
    assert_eq!(
        pieces,
        vec![
            vec![(1, 1), (0, 1), (0, 9), (0, 9)],
            vec![(1, 1), (2, 2), (0, 2), (0, 9)],
            vec![(1, 1), (2, 2), (3, 3), (0, 4)],
        ]
    );
    // Every value from -1000 to 1234, and nothing else, in some shape.
    let all = shapes(-1000, 1234);
    let mut count = 0_u64;
    for shape in &all {
        assert!(shape.low >= -1000 && shape.high <= 1234, "{shape:?}");
        count += shape
            .digits
            .iter()
            .map(|(low, high)| u64::from(high - low + 1))
            .product::<u64>();
    }
    assert_eq!(count, 2235);
    assert!(shapes(i128::from(i64::MIN) + 1, i128::from(i64::MAX)).len() < 200);
}

#[test]
fn goes_through_every_combination_once_by_its_longest_text() {
    let text = |lengths: &[usize]| -> Vec<Part> {
        lengths
            .iter()
            .map(|length| Part::Text(*length, b"ab".to_vec()))
            .collect()
    };
    let cases: Vec<Vec<(String, Vec<Part>)>> = vec![
        vec![
            ("a".into(), text(&[0, 1, 2])),
            ("b".into(), text(&[1, 2])),
            (
                "c".into(),
                vec![
                    Part::Delivery(Delivery::Whole),
                    Part::Delivery(Delivery::Pieces),
                ],
            ),
        ],
        vec![("a".into(), text(&[3, 1, 2]))],
        vec![
            ("c".into(), vec![Part::Delivery(Delivery::Whole)]),
            ("a".into(), text(&[5, 0])),
            ("b".into(), text(&[2, 5, 7])),
        ],
    ];
    for each in cases {
        let total: usize = each.iter().map(|(_, parts)| parts.len()).product();
        let mut levels = Levels::new(&each, 0);
        let mut seen = Vec::new();
        let mut last = 0;
        while let Some((combination, length)) = levels.next() {
            assert!(length >= last, "longer first: {length} after {last}");
            last = length;
            // The length given is the combination's longest text.
            let mut rest = combination;
            let mut longest = 0;
            for (_, parts) in &each {
                longest = longest.max(part_length(&parts[rest % parts.len()]));
                rest /= parts.len();
            }
            assert_eq!(length, longest, "combination {combination}");
            seen.push(combination);
        }
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..total).collect::<Vec<_>>(),
            "every combination once"
        );
    }
}

fn seed() -> u64 {
    std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).expect("a seed");
            u64::from_le_bytes(bytes)
        })
}

/// A random condition on `x`: comparisons with constants of `x`, `x` plus a constant, `-x`, `x`
/// widened or a choice between two of them, joined by and, or and not.
fn random_condition(pool: &mut Pool, rng: &mut ChaCha8Rng, x: Term, depth: u32) -> Term {
    if depth == 0 || rng.random_range(0..3) == 0 {
        let width = pool.width(x);
        let subject = |pool: &mut Pool, rng: &mut ChaCha8Rng| match rng.random_range(0..4) {
            0 => x,
            1 => {
                let constant = pool.bv(width, rng.random_range(0..1_u128 << width));
                pool.add(x, constant)
            }
            2 => pool.neg(x),
            _ => pool.zero_extend(x, 6),
        };
        let mut compared = subject(pool, rng);
        if rng.random_range(0..4) == 0 {
            let other = subject(pool, rng);
            if pool.width(other) == pool.width(compared) {
                let choice = random_condition(pool, rng, x, 0);
                compared = pool.ite(choice, compared, other);
            }
        }
        let width = pool.width(compared);
        let constant = pool.bv(width, rng.random_range(0..1_u128 << width));
        return match rng.random_range(0..5) {
            0 => pool.ult(compared, constant),
            1 => pool.ult(constant, compared),
            2 => pool.slt(compared, constant),
            3 => pool.slt(constant, compared),
            _ => pool.eq(compared, constant),
        };
    }
    let a = random_condition(pool, rng, x, depth - 1);
    match rng.random_range(0..3) {
        0 => {
            let b = random_condition(pool, rng, x, depth - 1);
            pool.bool_and(a, b)
        }
        1 => {
            let b = random_condition(pool, rng, x, depth - 1);
            pool.bool_or(a, b)
        }
        _ => pool.bool_not(a),
    }
}

/// `term`'s value where `x` is `value`, read signed at its width.
fn signed_value(pool: &Pool, term: Term, x: Term, value: u128) -> i128 {
    let width = pool.width(term);
    let bits = pool
        .evaluate_with(term, &mut |_, variable| (variable == x).then_some(value))
        .expect("a value");
    let bits = i128::try_from(bits).expect("fits");
    if width < 128 && bits >= 1 << (width - 1) {
        bits - (1 << width)
    } else {
        bits
    }
}

/// The values a condition allows, read as intervals, are exactly where it holds; the path's
/// comparisons rule a condition out only where no value meets them all; and an output check
/// settled under a path, split on what the path leaves open, agrees with the check wherever the
/// path holds.
#[test]
fn reads_conditions_as_intervals_and_settles_only_what_holds() {
    let seed = seed();
    // Said first, so that a run that stalls in it (as one did on ARM64 once, for over an hour)
    // names the seed to run it again with: LENGUA_RANDOM_SEED.
    eprintln!("reads_conditions_as_intervals_and_settles_only_what_holds: seed {seed}");
    intervals_with(seed);
}

/// Looks for a seed the test above stalls on, as one did once: 400 seeds from a random one, each
/// given a minute, the first to stall named in the failure.
#[test]
#[ignore = "hunts for a seed the interval test stalls on: ci.yml mode ignored, filter hunt"]
fn hunt_for_a_seed_the_interval_test_stalls_on() {
    let first = seed();
    for seed in first..first.wrapping_add(400) {
        let (done, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            intervals_with(seed);
            let _ = done.send(());
        });
        assert!(
            wait.recv_timeout(Duration::from_secs(60)).is_ok(),
            "seed {seed} stalls the interval test for over a minute: run it with \
             LENGUA_RANDOM_SEED={seed}"
        );
    }
}

/// The interval test's rounds, from `seed`.
fn intervals_with(seed: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let (mut read, mut ruled_out, mut settled_some) = (0, 0, 0);
    for round in 0..1500 {
        let mut pool = Pool::default();
        let width = if round % 2 == 0 { 8 } else { 10 };
        let x = pool.var(width, "x");
        let every = 0..1_u128 << width;
        let holds = |pool: &Pool, condition: Term, value: u128| {
            pool.evaluate_with(condition, &mut |_, variable| {
                (variable == x).then_some(value)
            }) == Some(1)
        };
        let condition = random_condition(&mut pool, &mut rng, x, 3);
        if let Some((term, values)) = values_of(&pool, condition) {
            read += 1;
            for value in every.clone() {
                let at = signed_value(&pool, term, x, value);
                let inside = values.iter().any(|(low, high)| *low <= at && at <= *high);
                assert_eq!(
                    holds(&pool, condition, value),
                    inside,
                    "seed {seed}, round {round}: {} at x = {value}: {values:?}",
                    pool.sketch(condition, 8)
                );
            }
        }
        let path: Vec<Term> = (0..rng.random_range(1..4))
            .map(|_| random_condition(&mut pool, &mut rng, x, 2))
            .collect();
        let on_path =
            |pool: &Pool, value: u128| path.iter().all(|known| holds(pool, *known, value));
        if refuted(&pool, &path, condition) {
            ruled_out += 1;
            for value in every.clone() {
                assert!(
                    !(on_path(&pool, value) && holds(&pool, condition, value)),
                    "seed {seed}, round {round}: x = {value} meets the path and {}",
                    pool.sketch(condition, 8)
                );
            }
        }
        let check = random_condition(&mut pool, &mut rng, x, 3);
        let simpler = settled(&mut pool, &path, check, MAX_CHOICES);
        if simpler != check {
            settled_some += 1;
        }
        for value in every.clone() {
            if on_path(&pool, value) {
                assert_eq!(
                    holds(&pool, simpler, value),
                    holds(&pool, check, value),
                    "seed {seed}, round {round}: at x = {value}, {} settled as {}",
                    pool.sketch(check, 8),
                    pool.sketch(simpler, 8)
                );
            }
        }
    }
    assert!(
        read > 300 && ruled_out > 50 && settled_some > 50,
        "seed {seed}: {read} read, {ruled_out} ruled out, {settled_some} settled"
    );
}

/// An Attested Computation whose program prints its argument, of 1 to 3 letters.
const ECHO_IDEA: &str = r#"---
type: Attested Computation
title: echo
description: Prints its argument.
runtime: webassembly
computation: main.wat
parameters:
  - { name: s, type: text, required: true, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program prints its argument, `1` to `3` letters.

# Behaviors
## B1: Prints the argument
- Given the argument `ab`
- When the program runs
- Then it prints `ab`
- And it exits with code `0`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `{s}`
- And it exits with code `0`

| s |
|---|
| `string(1, 3, "abc")` |

# Constraints
- Uses: nothing
"#;

/// Writes its argument, whose end it finds a byte at a time: every branch is settled by the
/// bytes' ranges, none of which holds 0, and the output is the argument's own terms.
const ECHO: &str = r#"(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1 1)
  (func (export "_start")
    (local $at i32)
    (local $length i32)
    (drop (call $args_sizes_get (i32.const 0) (i32.const 4)))
    (drop (call $args_get (i32.const 16) (i32.const 1024)))
    (local.set $at (i32.load (i32.const 20)))
    (block $end
      (loop $next
        (br_if $end (i32.eqz (i32.load8_u (i32.add (local.get $at) (local.get $length)))))
        (local.set $length (i32.add (local.get $length) (i32.const 1)))
        (br $next)))
    (i32.store (i32.const 8) (local.get $at))
    (i32.store (i32.const 12) (local.get $length))
    (drop (call $fd_write (i32.const 1) (i32.const 8) (i32.const 1) (i32.const 24)))
    (call $exit (i32.const 0))))
"#;

/// The solvers start at the first question the terms don't settle: a shape whose every branch
/// and output its terms settle starts none, and the proof of every shape asks nothing.
#[test]
fn a_shape_its_terms_settle_starts_no_solver() {
    let (blocks, policy) = blocks_in(ECHO_IDEA);
    let block = &blocks[0];
    let built = build(ECHO).expect("echo builds");
    let module = super::wasm::Module::new(&built, policy).expect("the model");
    let each = vec![(
        "s".to_owned(),
        parts("s", &block.vars[0].1, 0).expect("parts"),
    )];
    let mut solvers = Solvers::new(10_000);
    for combination in 0..3 {
        solvers.next();
        let result = prove_shape(
            &module,
            block,
            &each,
            combination,
            &budget(60),
            &mut solvers,
            None,
        );
        assert!(
            matches!(result, Ok((1, 0, None, false))),
            "combination {combination}"
        );
    }
    assert!(
        solvers.solver.is_none(),
        "no question, so no solver started"
    );
    let verdict = prove_wasm(&built, policy, block, &budget(60));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                shapes: 3,
                paths: 3,
                questions: 0,
                alone: None,
                bound: None,
                ..
            }
        ),
        "{verdict:?}"
    );
}

/// The model's runs of the tests' samples are part of the proof's time: with it up, they stop and
/// the proof is "ran out of time", where it used to run each sample to the end. With the time
/// there, the same samples of the CSV total agree, as before.
#[test]
fn the_models_runs_of_the_samples_stop_when_the_proofs_time_is_up() {
    let (blocks, policy) = blocks_in(&repository_file("tests/fixtures/total/idea.md"));
    let built = build(&repository_file("tests/fixtures/total/main.wat")).expect("total builds");
    let samples: Vec<crate::testfile::Case> =
        blocks[0].cases(1).take(4).filter_map(Result::ok).collect();
    assert!(!samples.is_empty(), "the CSV total's samples");
    let result = agrees_wasm(&built, policy, &samples, false, Instant::now());
    assert_eq!(result.err().as_deref(), Some(OUT_OF_TIME));
    let in_time = Instant::now() + Duration::from_secs(600);
    let found = agrees_wasm(&built, policy, &samples, false, in_time).expect("the model agrees");
    assert!(found.is_none(), "{found:?}");
}

/// A proof keeps its time even where one shape of it is long, as the shapes of count's 64 KiB
/// buffer-fulls are: given seconds where it needs minutes, it stops within its budget and a
/// shape's least time, where a proof once ran six minutes past. A computer fast enough to prove
/// it whole within the budget never meets the deadline, so the proof is made again with a tenth
/// of the budget; one proven whole even then kept its time.
#[test]
fn a_proof_keeps_its_time_within_one_long_shape() {
    if !solvers_installed() {
        return;
    }
    let (blocks, policy) = blocks_in(&repository_file("examples/count/idea.md"));
    let built = build(&repository_file("examples/count/main.wat")).expect("count builds");
    for seconds in [10, 1] {
        let short = budget(seconds);
        let given = short.deadline.saturating_duration_since(Instant::now());
        let started = Instant::now();
        let verdict = prove_wasm(&built, policy, &blocks[0], &short);
        let took = started.elapsed();
        assert!(
            took <= given + MIN_SHAPE_TIME * 2 + Duration::from_secs(20),
            "{took:?} for {given:?}: {verdict:?}"
        );
        if !matches!(verdict, Verdict::Proven { bound: None, .. }) {
            break;
        }
    }
}
