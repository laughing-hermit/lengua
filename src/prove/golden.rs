//! Round 9's guard G3, for the WebAssembly lengua: the prover on every program in the repository,
//! recorded at one commit and compared with the recording made at another, so that a change to
//! the prover that moves any run or proof shows. For each generated block it records its samples'
//! runs through the concrete model (how each ended, its steps, and hashes of what it wrote) and
//! its proofs. The programs are the checker golden's ([`crate::checker::golden::programs`]). The
//! native lengua's guard, on the x86-64 and ARM64 models, is at the tag `native-final`.
//!
//! The server's proofs go as far as their time allows, which differs from one run to the next, so
//! here each block is proven in ways that always end the same: by induction, its step over the
//! read loop as the idea gives the block and its base over the shortest final pieces (with a
//! smaller buffer for the programs in [`SMALLER`]); and by length over part of its ranges, the
//! widest of [`CUTS`] with at most [`COMBINATIONS`] input combinations (from a narrower one for
//! the blocks in [`NARROW`]), so that every combination is proven. Each proof has a model of its
//! own, so that the functions one proof's runs reached say nothing of another's. The samples keep
//! the idea's ranges, edges included, but for texts longer than [`SAMPLE_TEXT`]. A proof that
//! still runs out of time is marked `~`, since how far it gets depends on the machine.
//!
//! `LENGUA_GOLDEN=write cargo test --release --lib prove::golden -- --ignored` records them under
//! `target/golden/`, in prover-wasm.txt; without the variable the test compares. The recording
//! isn't committed: [`crate::checker::golden`] says how a baseline is made.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use super::exec::{self, End, State};
use super::term::{Pool, Term};
use super::wasm::{Module, Wasm};
use super::{AT_ONCE_PATHS, Budget, Verdict, by_length, sample_start, size};
use crate::checker::golden::{Program, compare_or_record, programs};
use crate::expr::Generator;
use crate::hash::Sha256Hex;
use crate::testfile::Generated;
use crate::wasm::Policy;
use crate::wasm::text::{Built, build};

/// The recording's name, under `target/golden/`.
const RECORDING: &str = "prover-wasm.txt";

/// Each proof's time: far more than any proof here takes, so only one that never ends runs out.
const PROOF_SECONDS: u64 = 300;

/// The samples a block's model runs, as the server takes them.
const SAMPLES: usize = 24;

/// The longest text a sample runs with: two buffer-fulls of the 4096 bytes that most of the read
/// loops take, so loops go round many times, without a megabyte's run through the model. Longer
/// texts take the same operators round again; induction proves the idea's lengths.
const SAMPLE_TEXT: usize = 1 << 13;

/// The parts of a block's ranges a proof by length here may go through, widest first: each text
/// at most so many bytes past its shortest, and each whole number, or decimal's digits, at most
/// so big.
const CUTS: [(usize, i128); 4] = [(3, 999), (1, 99), (0, 9), (0, 0)];

/// The most input combinations a proof by length here goes through, as the prover counts them.
const COMBINATIONS: usize = 1024;

/// How many bytes past the shortest the base of a proof by induction goes here.
const BASE_LENGTHS: usize = 3;

/// The blocks, by program folder and number, whose proofs by length take minutes over the wider
/// parts of their ranges, with where in [`CUTS`] each starts: calc's 64-bit products, and
/// histogram's table of counts, as the native golden narrowed them.
const NARROW: [(&str, usize, usize); 2] = [("calc", 3, 2), ("histogram", 1, 1)];

/// A program folder, and the changes to its main.wat that give the smaller buffer its proof by
/// induction is proven with.
type Smaller = (&'static str, &'static [(&'static str, &'static str)]);

/// The programs proven by induction here with a smaller buffer than their own, by folder, as the
/// prover's own tests prove lines and count: a step over a buffer-full of thousands of symbolic
/// bytes takes the solvers minutes. Each change is a line of main.wat and what it becomes: the
/// buffer's size, and where the program counts the steps a buffer-full takes, their count and
/// where the last one starts, so its loops still go over the whole of the smaller buffer.
const SMALLER: [Smaller; 6] = [
    (
        "vowels",
        &[
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
        ],
    ),
    (
        "count",
        &[
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
        ],
    ),
    (
        "lines",
        &[
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
        ],
    ),
    (
        "padded",
        &[
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
        ],
    ),
    (
        "upcase-stdin",
        &[
            (
                "(global $buffer.size i32 (i32.const 65536))",
                "(global $buffer.size i32 (i32.const 256))",
            ),
            (
                "(global $BLOCKS i32 (i32.const 1024))",
                "(global $BLOCKS i32 (i32.const 4))",
            ),
            (
                "(global $LAST_BLOCK i32 (i32.const 65472))",
                "(global $LAST_BLOCK i32 (i32.const 192))",
            ),
            (
                "(global $LAST_CHUNK i32 (i32.const 65520))",
                "(global $LAST_CHUNK i32 (i32.const 240))",
            ),
        ],
    ),
    (
        "words",
        &[(
            "(global $buffer.size i32 (i32.const 4096))",
            "(global $buffer.size i32 (i32.const 256))",
        )],
    ),
];

/// What a run of the model left, for the recording: how it ended, its steps, and hashes of what
/// it wrote to standard output and error, to files and on its connections, with the folders it
/// made and removed. A value that isn't a constant prints as `?`.
fn fingerprint(run: &State<Wasm>, pool: &Pool) -> String {
    let value = |term: Term| {
        pool.constant(term)
            .map_or_else(|| "?".to_owned(), |value| format!("{value:x}"))
    };
    let bytes = |terms: &[Term]| -> String {
        let known: Option<Vec<u8>> = terms
            .iter()
            .map(|term| {
                pool.constant(*term)
                    .and_then(|byte| u8::try_from(byte).ok())
            })
            .collect();
        match known {
            Some(known) => Sha256Hex::of(&known).as_str()[..16].to_owned(),
            None => format!("?{}", terms.len()),
        }
    };
    let folders = |names: &BTreeSet<String>| {
        names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
    };
    let world = &run.world;
    let end = match &world.end {
        Some(End::Exit(status)) => format!("exit {}", value(*status)),
        Some(other) => format!("{other:?}"),
        None => "running".to_owned(),
    };
    let written: Vec<String> = world
        .written
        .iter()
        .map(|(name, contents)| format!("{name}={}", bytes(contents)))
        .collect();
    let sent: Vec<String> = world
        .connections
        .iter()
        .map(|connection| {
            let how = if connection.accepted {
                "accepted"
            } else {
                "made"
            };
            format!("{how} {}={}", connection.exchange, bytes(&connection.sent))
        })
        .collect();
    format!(
        "{end}; {} steps; out {}; err {}; written [{}]; made [{}]; removed [{}]; sent [{}]",
        world.steps,
        bytes(&world.stdout),
        bytes(&world.stderr),
        written.join(","),
        folders(&world.made),
        folders(&world.removed),
        sent.join(",")
    )
}

/// `why` a proof stopped, or `~` where it's only that the time ran out.
fn reason(why: &str) -> String {
    let timed = ["out of time", "time ran out", "would take about"];
    if timed.iter().any(|timed| why.contains(timed)) {
        "~ out of time".to_owned()
    } else {
        why.to_owned()
    }
}

/// A proof's result, without what depends on how the solvers' time went: which solver answered
/// alone, and a counterexample's values (one of many).
fn verdict(found: &Verdict) -> String {
    match found {
        Verdict::Proven {
            bound: Some(bound), ..
        } if bound.more => "~ proven as far as the time allowed".to_owned(),
        Verdict::Proven {
            shapes,
            paths,
            questions,
            bound,
            cut_any_way,
            unreached,
            induction,
            lookback,
            ..
        } => format!(
            "proven: {shapes} shapes, {paths} paths, {questions} questions, bound {bound:?}, \
             cut {cut_any_way}, unreached {unreached:?}, induction {induction:?}, lookback \
             {lookback}"
        ),
        Verdict::Counterexample(values) => {
            format!("counterexample of {:?}", values.keys().collect::<Vec<_>>())
        }
        Verdict::NotProven(why) => format!("not proven: {}", reason(why)),
    }
}

/// `low..=high` cut to the numbers of at most `magnitude`, or, for a range with none, to its
/// first ones.
fn cut(low: i128, high: i128, magnitude: i128) -> (i128, i128) {
    let (from, to) = (low.max(-magnitude), high.min(magnitude));
    if from <= to {
        (from, to)
    } else {
        (low, high.min(low.saturating_add(magnitude)))
    }
}

/// `block` with each text at most `lengths` bytes past its shortest and each number at most
/// `magnitude`: a part of every range, so what holds for the idea's holds for it.
fn cut_to(block: &Generated, lengths: usize, magnitude: i128) -> Generated {
    let mut block = block.clone();
    for (_, generator) in &mut block.vars {
        match generator {
            Generator::Str { min, max, .. } => {
                *max = (*max).min(min.saturating_add(lengths));
            }
            Generator::Int { lo, hi } | Generator::Dec { lo, hi, .. } => {
                (*lo, *hi) = cut(*lo, *hi, magnitude);
            }
            Generator::Choice(_) => {}
        }
    }
    block
}

/// `block` cut to the widest of [`CUTS`] from the `from`th with at most [`COMBINATIONS`] input
/// combinations, or the narrowest, and which it is.
fn shortened(block: &Generated, from: usize) -> (Generated, (usize, i128)) {
    let fits = CUTS[from..]
        .iter()
        .find(|(lengths, magnitude)| size(&cut_to(block, *lengths, *magnitude)) <= COMBINATIONS)
        .unwrap_or(&CUTS[CUTS.len() - 1]);
    (cut_to(block, fits.0, fits.1), *fits)
}

/// `block` with each text at most [`SAMPLE_TEXT`] long, for its samples.
fn sampled(block: &Generated) -> Generated {
    let mut block = block.clone();
    for (_, generator) in &mut block.vars {
        if let Generator::Str { min, max, .. } = generator {
            (*min, *max) = ((*min).min(SAMPLE_TEXT), (*max).min(SAMPLE_TEXT));
        }
    }
    block
}

/// A budget of `seconds`: with [`PROOF_SECONDS`], one that ends only a proof that never would.
fn budget(seconds: u64, listens: bool) -> Budget {
    Budget::until(Instant::now() + Duration::from_secs(seconds), listens)
}

/// The proof by induction of `block` on `module` as the idea gives it: its step over the read
/// loop, and, where that holds, its base from the loop's head over every final piece at once,
/// or else over the shortest, as far as [`BASE_LENGTHS`] bytes past the shortest. The base the
/// server proves goes on to a whole buffer-full, as far as its time allows.
fn induction(module: &Module, block: &Generated, listens: bool, seconds: u64) -> String {
    let found = match super::induction::step(module, block, &budget(seconds, listens)) {
        Ok(found) => found,
        Err(refused) => return format!("none: {}", reason(&refused.why)),
    };
    // As the prover does: every final piece up to a buffer-full at once where it can, else the
    // shortest few one length at a time.
    let mut whole = block.clone();
    if let [(_, Generator::Str { min, max, .. })] = whole.vars.as_mut_slice() {
        *min = (*min).min(1);
        *max = (*max).min(found.chunk);
    }
    let within = Budget {
        within: true,
        max_paths: budget(seconds, listens)
            .max_paths
            .saturating_mul(AT_ONCE_PATHS),
        ..budget(seconds, listens)
    };
    if let Ok(proven @ Verdict::Proven { bound: None, .. }) =
        by_length(module, &whole, &within, Some(&found.hook))
    {
        return format!(
            "step holds: {} bytes a time round, reach {}, {} paths, {} questions, lookback {}; \
             every final piece at once {}",
            found.chunk,
            found.reach,
            found.paths,
            found.questions,
            found.lookback,
            verdict(&proven)
        );
    }
    let mut base = block.clone();
    let mut longest = 0;
    if let [(_, Generator::Str { min, max, .. })] = base.vars.as_mut_slice() {
        *min = (*min).min(1);
        *max = (*max).min(*min + BASE_LENGTHS).min(found.chunk);
        longest = *max;
    }
    let proven = by_length(module, &base, &budget(seconds, listens), Some(&found.hook))
        .unwrap_or_else(Verdict::NotProven);
    format!(
        "step holds: {} bytes a time round, reach {}, {} paths, {} questions, lookback {}; base \
         to {longest} bytes {}",
        found.chunk,
        found.reach,
        found.paths,
        found.questions,
        found.lookback,
        verdict(&proven)
    )
}

/// `block`'s proofs, each on a model of its own whose idea lets it reach what `policy` allows: by
/// induction as the idea gives it, of `induced`, the program's build or its [`SMALLER`]
/// buffer's; then by length over part of its ranges, of `built`, from the `from`th of [`CUTS`];
/// with how long each took, for whoever watches the recording.
fn proofs(
    (built, induced): (&Built, &Built),
    policy: Policy,
    block: &Generated,
    listens: bool,
    from: usize,
    seconds: u64,
) -> (String, [Duration; 2]) {
    if block.uses_now() {
        return ("uses now".to_owned(), [Duration::ZERO; 2]);
    }
    let (module, smaller) = match (Module::new(built, policy), Module::new(induced, policy)) {
        (Ok(module), Ok(smaller)) => (module, smaller),
        (Err(why), _) | (_, Err(why)) => {
            return (format!("the model refuses it: {why}"), [Duration::ZERO; 2]);
        }
    };
    let began = Instant::now();
    let induction = induction(&smaller, block, listens, seconds);
    let inducted = began.elapsed();
    let (cut, (lengths, magnitude)) = shortened(block, from);
    let length = by_length(&module, &cut, &budget(seconds, listens), None)
        .unwrap_or_else(Verdict::NotProven);
    (
        format!(
            "by induction {induction}; by length, texts +{lengths} and numbers to {magnitude}, {}",
            verdict(&length)
        ),
        [inducted, began.elapsed() - inducted],
    )
}

/// The build of `program` that its proofs by induction take, with a smaller buffer where
/// [`SMALLER`] gives it one, which `recording` notes; none where it has none, or where the
/// changes don't apply, which `recording` says.
fn smaller(program: &Program, recording: &mut String) -> Option<Built> {
    let (_, changes) = SMALLER
        .iter()
        .find(|(folder, _)| *folder == program.folder)?;
    let mut text = program.text.clone();
    for (line, smaller) in *changes {
        if !text.contains(line) {
            let _ = writeln!(recording, "  no smaller buffer: main.wat has no {line:?}");
            return None;
        }
        text = text.replacen(line, smaller, 1);
    }
    match build(&text) {
        Ok(built) => {
            let _ = writeln!(recording, "  by induction with {}", changes[0].1);
            Some(built)
        }
        Err(problem) => {
            let _ = writeln!(recording, "  no smaller buffer: {problem}");
            None
        }
    }
}

/// The recording: per program and generated block, each sample's run and the proofs.
fn record() -> String {
    let mut recording = String::new();
    for program in programs() {
        record_program(&program, PROOF_SECONDS, &mut recording);
    }
    recording
}

/// What the recording holds of `program`: each generated block's samples and its proofs, each
/// proof with `seconds` of time.
fn record_program(program: &Program, seconds: u64, recording: &mut String) {
    let _ = writeln!(recording, "== {}", program.name);
    let idea = match &program.idea {
        Ok(idea) => idea,
        Err(why) => {
            let _ = writeln!(recording, "  the idea doesn't parse: {why}");
            return;
        }
    };
    let built = match build(&program.text) {
        Ok(built) => built,
        Err(problem) => {
            let _ = writeln!(recording, "  doesn't build: {problem}");
            return;
        }
    };
    let policy = Policy::of(&idea.uses, &idea.network);
    // The samples' model; each proof takes one of its own.
    let module = match Module::new(&built, policy) {
        Ok(module) => module,
        Err(why) => {
            let _ = writeln!(recording, "  the model refuses it: {why}");
            return;
        }
    };
    // The build its proofs by induction take, with a smaller buffer where it has one.
    let smaller = smaller(program, recording);
    let induced = smaller.as_ref().unwrap_or(&built);
    let suite = match crate::testfile::check(idea) {
        Ok(suite) => suite,
        Err(problems) => {
            let _ = writeln!(recording, "  the idea's tests don't check: {problems:?}");
            return;
        }
    };
    let listens = idea.network.listens.is_some();
    for (index, block) in suite.generated.iter().enumerate() {
        let number = index + 1;
        let began = Instant::now();
        let sampled = sampled(block);
        let samples: Vec<_> = sampled
            .cases(1)
            .take(SAMPLES)
            .filter_map(Result::ok)
            .collect();
        for (sample, case) in samples.iter().enumerate() {
            let mut pool = Pool::default();
            let start = sample_start(&module, &mut pool, case, listens);
            let runs = exec::explore(&module, start, &mut pool, &mut exec::Concrete, 2);
            let found = match runs {
                Ok(runs) => runs
                    .iter()
                    .map(|run| fingerprint(run, &pool))
                    .collect::<Vec<_>>()
                    .join(" | "),
                Err(why) => format!("the model stopped: {why}"),
            };
            let _ = writeln!(recording, "  B{number} sample {sample}: {found}");
        }
        let ran = began.elapsed();
        let from = NARROW
            .iter()
            .find(|(folder, at, _)| *folder == program.folder && *at == number)
            .map_or(0, |(.., from)| *from);
        let (found, [inducted, proven]) =
            proofs((&built, induced), policy, block, listens, from, seconds);
        eprintln!(
            "{} B{number}: samples {:.1} s, induction {:.1} s, length {:.1} s: {found}",
            program.name,
            ran.as_secs_f64(),
            inducted.as_secs_f64(),
            proven.as_secs_f64()
        );
        let _ = writeln!(recording, "  B{number} proofs: {found}");
    }
}

/// The guard: every program's samples run and its blocks are proven as recorded (or recorded,
/// where there is nothing to compare with: see [`compare_or_record`]).
#[test]
#[ignore = "the prover's golden: run with --ignored around a change to the prover"]
fn every_run_and_proof_is_as_recorded() {
    compare_or_record(RECORDING, &record());
}

/// The programs whose proofs the test below holds to a small budget, by folder: the two whose
/// main.wat sorts records in place, where the recording once never ended.
const HELD_TO_TIME: [&str; 2] = ["sort-large", "tally"];

/// The time each proof of [`HELD_TO_TIME`] gets in the test below, in seconds.
const SHORT_SECONDS: u64 = 5;

/// The longest a program of [`HELD_TO_TIME`] may take in the test below, in seconds: GitHub's Linux
/// runner on x86-64 took 40 for the two (run 42846675507).
const LONGEST_SECONDS: u64 = 300;

/// A proof ends with its budget: every block of the programs in [`HELD_TO_TIME`], its samples
/// run and its proofs made with [`SHORT_SECONDS`] each, is done in minutes, where one of them
/// once ran for hours past a budget of 300 seconds.
#[test]
fn a_proof_ends_within_its_budget() {
    let held: Vec<Program> = programs()
        .into_iter()
        .filter(|program| HELD_TO_TIME.contains(&program.folder.as_str()))
        .collect();
    assert_eq!(held.len(), HELD_TO_TIME.len());
    for program in held {
        let began = Instant::now();
        let mut recording = String::new();
        record_program(&program, SHORT_SECONDS, &mut recording);
        eprintln!("{}: {:.1} s", program.name, began.elapsed().as_secs_f64());
        assert!(recording.contains(" proofs: "), "{recording}");
        assert!(
            began.elapsed() < Duration::from_secs(LONGEST_SECONDS),
            "{} took {:?}",
            program.name,
            began.elapsed()
        );
    }
}

/// A run is recorded by how it ended, its steps and what it wrote: here an exit with code 3 and
/// nothing written, whose outputs' hashes are the empty text's.
#[test]
fn a_run_is_recorded_by_its_end_its_steps_and_what_it_wrote() {
    let text = "(module
  (import \"wasi_snapshot_preview1\" \"proc_exit\" (func $exit (param i32)))
  (memory (export \"memory\") 1 1)
  (func (export \"_start\")
    (call $exit (i32.const 3))))
";
    let built = build(text).expect("it builds");
    let module = Module::new(&built, Policy::default()).expect("the model takes it");
    let mut pool = Pool::default();
    let start = super::wasm::begin(&module, &mut pool, &[], &[], Vec::new());
    let runs =
        exec::explore(&module, start, &mut pool, &mut exec::Concrete, 2).expect("the model runs");
    let [run] = runs.as_slice() else {
        panic!("{} runs", runs.len());
    };
    let found = fingerprint(run, &pool);
    assert!(found.starts_with("exit 3; "), "{found}");
    assert!(
        found.contains(" steps; out e3b0c44298fc1c14; err e3b0c44298fc1c14; written []; made []"),
        "{found}"
    );
}
