//! Round 9's guard G3: the prover on every program in the repository, x86-64's and ARM64's,
//! recorded before its machine model moves onto per-CPU tables, so each step of the move can be
//! shown to change nothing: each generated block's samples run through the concrete model (how
//! each ended, its steps, registers, flags and output), and each block's proofs. A CPU's
//! programs are the checker golden's list for it ([`crate::checker::golden::PLACES`]); the
//! programs that use SVE are recorded at vectors of 16 bytes, so the recording does not depend
//! on the computer that makes it.
//!
//! The server's proofs go as far as their time allows, which differs from one run to the next, so
//! here each block is proven in ways that always end the same: by induction, its step over the
//! read loop as the spec gives the block and its base over the shortest final pieces (with a
//! smaller buffer for the programs in [`SMALLER`]); and by length over part of its ranges, the
//! widest of [`CUTS`] with at most [`COMBINATIONS`] input combinations (from a narrower one for
//! the blocks in [`NARROW`]), so that every combination is proven. The samples keep the spec's
//! ranges, edges included, but for texts longer than [`SAMPLE_TEXT`]. A proof that still runs
//! out of time is marked `~`, since how far it gets depends on the machine.
//!
//! `LENGUA_GOLDEN=write cargo test --release --lib prove::golden -- --ignored` records them under
//! `target/golden/`; without the variable the test compares (see [`crate::checker::golden`]).

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use super::exec::{self, Image};
use super::term::Pool;
use super::{Budget, Verdict, by_length, sample_start};
use crate::checker::decode::Machine;
use crate::checker::golden::{PLACES, built, compare_or_record, programs};
use crate::expr::Generator;
use crate::platform::Profile;
use crate::targets::PerCpu;
use crate::testfile::Generated;
use crate::toolchain::Toolchain;

/// Each proof's time: far more than any proof here takes, so only one that never ends runs out.
const PROOF_SECONDS: u64 = 300;

/// The samples a block's model runs, as the server takes them.
const SAMPLES: usize = 24;

/// The longest text a sample runs with: two of the smallest buffer the programs read into, and
/// two pipe writes, so loops go round many times, without a megabyte's run through the model.
/// Longer texts take the same instructions round again; induction proves the spec's lengths.
const SAMPLE_TEXT: usize = 1 << 13;

/// The parts of a block's ranges a proof by length here may go through, widest first: each text
/// at most so many bytes past its shortest, and each whole number, or decimal's digits, at most
/// so big.
const CUTS: [(usize, i128); 4] = [(3, 999), (1, 99), (0, 9), (0, 0)];

/// The most input combinations a proof by length here goes through, as the prover counts them.
const COMBINATIONS: usize = 1024;

/// How many bytes past the shortest the base of a proof by induction goes here.
const BASE_LENGTHS: usize = 3;

/// The length of the SVE vectors the programs that use them are recorded at, in bytes.
const SVE_BYTES: u16 = 16;

/// The blocks, by program and number, whose proofs by length take minutes over the wider parts
/// of their ranges, with where in [`CUTS`] each starts: calc's 64-bit products, and histogram's
/// table of counts, which the solvers take a minute and a half over at texts of 3 bytes. Each
/// target's own programs.
const NARROW: PerCpu<&[(&str, usize, usize)]> = PerCpu {
    x86_64: &[
        ("tests/fixtures/calc/main.s", 3, 2),
        ("plugin/skills/compile/assets/fast/histogram/main.s", 1, 1),
    ],
    aarch64: &[
        ("tests/fixtures/linux-aarch64/calc/main.s", 3, 2),
        (
            "plugin/skills/compile/assets/linux-aarch64/fast/histogram/main.s",
            1,
            1,
        ),
    ],
};

/// A program and the changes to its main.s that give the smaller buffer its proof by induction
/// is proven with.
type Smaller = (&'static str, &'static [(&'static str, &'static str)]);

/// The programs proven by induction here with a smaller buffer than their own, as the prover's
/// tests prove them: the step over a buffer-full of 4096 symbolic bytes takes the solvers
/// minutes. Each change is a line of main.s and what it becomes, a loop's bound with its
/// buffer's where it counts the buffer's steps. Each target's own programs.
const SMALLER: PerCpu<&[Smaller]> = PerCpu {
    x86_64: &[
        (
            "tests/fixtures/lines/main.s",
            &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n")],
        ),
        (
            "tests/fixtures/words/main.s",
            &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n")],
        ),
        (
            "plugin/skills/compile/assets/fast/wc/main.s",
            &[
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 256\n"),
                ("  # bound: 4096\n", "  # bound: 16\n"),
            ],
        ),
        (
            "plugin/skills/compile/assets/fast/wc-avx512/main.s",
            &[
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 256\n"),
                ("  # bound: 1024\n", "  # bound: 4\n"),
            ],
        ),
    ],
    aarch64: &[
        (
            "tests/fixtures/linux-aarch64/lines/main.s",
            &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n")],
        ),
        (
            "tests/fixtures/linux-aarch64/words/main.s",
            &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n")],
        ),
        (
            "plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s",
            &[
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 256\n"),
                ("  // bound: 4096\n", "  // bound: 16\n"),
            ],
        ),
        (
            "plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s",
            &[
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 256\n"),
                ("  // bound: 4096\n", "  // bound: 16\n"),
            ],
        ),
    ],
};

/// The machine code of `text` for `profile`'s CPU, with what main.s names and its jump tables,
/// as the server passes them to the prover.
fn machine_of(profile: Profile, text: &str, bytes: &[u8]) -> Machine {
    let mut machine = crate::checker::decode::read(bytes).expect("decode");
    let (parsed, _) = crate::checker::check_layout(text);
    if let Some(program) = crate::checker::decode::map(
        &parsed,
        &machine,
        crate::platform::CpuFeatures::all(),
        &mut Vec::new(),
    ) {
        machine.named = program.named_symbols();
        let jumps = crate::checker::tables::every(&program, &mut Vec::new());
        machine.tables = crate::checker::tables::addresses(&program, &jumps);
    }
    if profile == Profile::LinuxAarch64 {
        machine.sve_bytes = SVE_BYTES;
    }
    machine
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
/// `magnitude`: a part of every range, so what holds for the spec's holds for it.
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
        .find(|(lengths, magnitude)| {
            super::size(&cut_to(block, *lengths, *magnitude)) <= COMBINATIONS
        })
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

/// A budget that ends only a proof that never would.
fn budget(listens: bool) -> Budget {
    Budget::until(Instant::now() + Duration::from_secs(PROOF_SECONDS), listens)
}

/// The proof by induction of `block` as the spec gives it: its step over the read loop, and,
/// where that holds, its base from the loop's head over the shortest final pieces, as far as
/// [`BASE_LENGTHS`] bytes past the shortest. The base the server proves goes on to a whole
/// buffer-full, as far as its time allows.
fn induction(image: &Image, block: &Generated, listens: bool) -> String {
    let found = match super::induction::step(image, block, &budget(listens)) {
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
        max_paths: budget(listens)
            .max_paths
            .saturating_mul(super::AT_ONCE_PATHS),
        ..budget(listens)
    };
    if let Ok(proven @ Verdict::Proven { bound: None, .. }) =
        by_length(image, &whole, &within, Some(&found.hook))
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
    let proven = by_length(image, &base, &budget(listens), Some(&found.hook))
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

/// `block`'s proofs: by induction as the spec gives it, of `induced`, the program or its
/// [`SMALLER`] buffer's build; then by length over part of its ranges, from the `from`th of
/// [`CUTS`]; with how long each took, for whoever watches the recording.
fn proofs(
    machine: &Machine,
    induced: &Machine,
    block: &Generated,
    listens: bool,
    from: usize,
) -> (String, [Duration; 2]) {
    if block.uses_now() {
        return ("uses now".to_owned(), [Duration::ZERO; 2]);
    }
    let (Some(image), Some(smaller)) = (Image::new(machine), Image::new(induced)) else {
        return ("no _start".to_owned(), [Duration::ZERO; 2]);
    };
    let began = Instant::now();
    let induction = induction(&smaller, block, listens);
    let inducted = began.elapsed();
    let (cut, (lengths, magnitude)) = shortened(block, from);
    let length = by_length(&image, &cut, &budget(listens), None).unwrap_or_else(Verdict::NotProven);
    (
        format!(
            "by induction {induction}; by length, texts +{lengths} and numbers to {magnitude}, {}",
            verdict(&length)
        ),
        [inducted, began.elapsed() - inducted],
    )
}

/// The recording: per program and generated block, each sample's run and the proofs.
async fn record(profile: Profile) -> String {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let mut recording = String::new();
    for program in programs(profile) {
        let _ = writeln!(recording, "== {}", program.name);
        let Ok(bytes) = built(profile, &toolchain, &program.text).await else {
            let _ = writeln!(recording, "  doesn't build");
            continue;
        };
        let machine = machine_of(profile, &program.text, &bytes);
        let Some(image) = Image::new(&machine) else {
            let _ = writeln!(recording, "  no _start");
            continue;
        };
        // The build its proofs by induction take, with a smaller buffer where it has one.
        let induced = match SMALLER
            .get(profile)
            .iter()
            .find(|(name, _)| *name == program.name)
        {
            Some((_, changes)) => {
                let mut text = program.text.clone();
                for (line, smaller) in *changes {
                    assert!(text.contains(line), "{} has {line:?}", program.name);
                    text = text.replace(line, smaller);
                }
                let bytes = built(profile, &toolchain, &text)
                    .await
                    .expect("the smaller buffer builds");
                let _ = writeln!(recording, "  by induction with {}", changes[0].1.trim_end());
                machine_of(profile, &text, &bytes)
            }
            None => machine.clone(),
        };
        let suite = match crate::testfile::check(&program.spec) {
            Ok(suite) => suite,
            Err(problems) => {
                let _ = writeln!(recording, "  the spec's tests don't check: {problems:?}");
                continue;
            }
        };
        let listens = program.spec.network.listens.is_some();
        for (index, block) in suite.generated.iter().enumerate() {
            let began = Instant::now();
            let sampled = sampled(block);
            let samples: Vec<_> = sampled
                .cases(1)
                .take(SAMPLES)
                .filter_map(Result::ok)
                .collect();
            for (sample, case) in samples.iter().enumerate() {
                let mut pool = Pool::default();
                let start = sample_start(&image, &mut pool, case, listens);
                let runs = exec::explore(&image, start, &mut pool, &mut exec::Concrete, 2);
                let found = match runs {
                    Ok(runs) => runs
                        .iter()
                        .map(|run| run.fingerprint(&pool))
                        .collect::<Vec<_>>()
                        .join(" | "),
                    Err(why) => format!("the model stopped: {why}"),
                };
                let _ = writeln!(recording, "  B{} sample {sample}: {found}", index + 1);
            }
            let ran = began.elapsed();
            let from = NARROW
                .get(profile)
                .iter()
                .find(|(name, number, _)| *name == program.name && *number == index + 1)
                .map_or(0, |(.., from)| *from);
            let (found, [inducted, proven]) = proofs(&machine, &induced, block, listens, from);
            eprintln!(
                "{} B{}: samples {:.1} s, induction {:.1} s, length {:.1} s: {found}",
                program.name,
                index + 1,
                ran.as_secs_f64(),
                inducted.as_secs_f64(),
                proven.as_secs_f64()
            );
            let _ = writeln!(recording, "  B{} proofs: {found}", index + 1);
        }
    }
    recording
}

/// The guard on `profile`'s programs: their samples' runs and their proofs are as recorded (or
/// recorded, where there is nothing to compare with: see [`compare_or_record`]).
async fn runs_and_proofs_are_as_recorded(profile: Profile) {
    let found = record(profile).await;
    compare_or_record(&format!("prover-{}.txt", PLACES.get(profile).label), &found);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Round 9's guard: run with --ignored around the prover's refactor"]
async fn every_run_and_proof_is_as_recorded() {
    runs_and_proofs_are_as_recorded(Profile::LinuxX86_64).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Round 9's guard: run with --ignored around the prover's refactor"]
async fn every_run_and_proof_is_as_recorded_on_aarch64() {
    runs_and_proofs_are_as_recorded(Profile::LinuxAarch64).await;
}

/// SPIKE (round 2, step 0b.1), thrown away: `text` with the read of its `fill` moved into a
/// function `os_read` that `fill` calls, as the library's `fill` will call the edge's.
fn through_a_call(profile: Profile, text: &str) -> String {
    let (read, call, function) = match profile {
        Profile::LinuxX86_64 => (
            "    mov eax, SYS_READ\n    syscall\n",
            "    call os_read\n",
            "\n.text\n.type os_read, @function\nos_read:\n    mov eax, SYS_READ\n    syscall\n    ret\n.size os_read, . - os_read\n",
        ),
        Profile::LinuxAarch64 => (
            "    mov x8, #SYS_READ\n    svc #0\n",
            "    bl os_read\n",
            "\n.text\n.type os_read, @function\nos_read:\n    mov x8, #SYS_READ\n    svc #0\n    ret\n.size os_read, . - os_read\n",
        ),
    };
    assert_eq!(text.matches(read).count(), 1, "one read, in fill");
    format!("{}{function}", text.replace(read, call))
}

/// SPIKE: a proof's verdict without its digits (counts, sizes), and without the function
/// `os_read`, which no run calls where `fill` stands in for it.
fn kind(proof: &str) -> String {
    proof
        .replace(", \"os_read\"", "")
        .replace("\"os_read\", ", "")
        .replace("\"os_read\"", "")
        .chars()
        .filter(|character| !character.is_ascii_digit())
        .collect()
}

/// SPIKE: cat-stream and lines, proven as shipped and with `fill` calling `os_read`: the same
/// verdicts, in particular the same proof by induction.
async fn proofs_survive_a_call(profile: Profile) {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    for program in programs(profile) {
        if !(program.name.contains("/cat-stream/") || program.name.contains("/lines/")) {
            continue;
        }
        let suite = crate::testfile::check(&program.spec).expect("the spec's tests check");
        let listens = program.spec.network.listens.is_some();
        let mut verdicts: Vec<Vec<String>> = Vec::new();
        for (label, text) in [
            ("shipped", program.text.clone()),
            ("through a call", through_a_call(profile, &program.text)),
        ] {
            let bytes = built(profile, &toolchain, &text)
                .await
                .expect("it builds");
            let machine = machine_of(profile, &text, &bytes);
            let induced = match SMALLER
                .get(profile)
                .iter()
                .find(|(name, _)| *name == program.name)
            {
                Some((_, changes)) => {
                    let mut smaller = text.clone();
                    for (line, replacement) in *changes {
                        assert!(smaller.contains(line), "{} has {line:?}", program.name);
                        smaller = smaller.replace(line, replacement);
                    }
                    let bytes = built(profile, &toolchain, &smaller)
                        .await
                        .expect("the smaller buffer builds");
                    machine_of(profile, &smaller, &bytes)
                }
                None => machine.clone(),
            };
            let mut found = Vec::new();
            for (index, block) in suite.generated.iter().enumerate() {
                let (proof, _) = proofs(&machine, &induced, block, listens, 0);
                eprintln!("SPIKE {} {label} B{}: {proof}", program.name, index + 1);
                found.push(proof);
            }
            verdicts.push(found);
        }
        assert!(
            verdicts[0]
                .iter()
                .all(|proof| !proof.contains("by induction none")),
            "{} is proven by induction as shipped",
            program.name
        );
        let kinds = |found: &[String]| -> Vec<String> {
            found.iter().map(|proof| kind(proof)).collect()
        };
        assert_eq!(
            kinds(&verdicts[0]),
            kinds(&verdicts[1]),
            "{}: the proof changed when fill called os_read",
            program.name
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "round 2, step 0b.1: a spike"]
async fn spike_fill_through_a_call() {
    proofs_survive_a_call(Profile::LinuxX86_64).await;
    proofs_survive_a_call(Profile::LinuxAarch64).await;
}
