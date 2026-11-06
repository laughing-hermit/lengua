//! Proofs about real programs: the template, examples/add, and planted bugs the samples miss.
//!
//! Each case is written once for both Linux CPUs: a table with a [`PerCpu`] cell where the two
//! differ (the folder of a program's twin, an assembly source, the edit that plants a bug), run
//! for every target of [`Profile::ALL`] on either runner, since a proof is mathematics. A cell
//! that cannot be written says why. [`every_prover_test_runs_on_every_target`] fails a test that
//! runs for one CPU only.

use std::time::{Duration, Instant};

use super::*;
use crate::platform::Profile;
use crate::targets::PerCpu;
use crate::toolchain::Toolchain;

/// The share of the computer a proof's budget assumes. A proof spreads its work over every core,
/// and its budget is time on the clock, so a long proof runs alone, as on the computer its budget
/// was set on, while short ones run side by side. Run all at once, as `--include-ignored` runs
/// every slow one, the long ones shared the cores and ran out of their time: each passed alone and
/// failed among the rest.
static TURNS: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// The budget, in seconds on the clock of the 16-core computer the tests were tuned on, past which
/// a proof runs alone.
const ALONE_PAST: u64 = 150;

/// What [`speed_probe`] reads on the computer the budgets are set by: a GitHub runner of Linux on
/// x86-64 with 4 vCPUs (an AMD EPYC 9V45 read 67 ms on 2026-11-05, and ARM64 Linux's runner 66),
/// where a scale of [`REFERENCE_SCALE`] has been enough for every proof.
const REFERENCE_PROBE_MS: f64 = 67.0;

/// The scale that has been enough on the reference computer.
const REFERENCE_SCALE: f64 = 4.0;

/// How many times longer than the budgets are written for a proof's time is here, so a proof that
/// finishes on the reference computer finishes on this one too: in proportion to how long the
/// speed probe takes here against there. It is measured once, not worked out from the number of
/// cores, because a runner with as many cores can be much slower or faster than another, and a
/// slow one reached other verdicts than a fast one in the same time.
fn scale() -> u64 {
    static SCALE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        let here = speed_probe().as_secs_f64() * 1000.0;
        let ratio = REFERENCE_SCALE * here / REFERENCE_PROBE_MS;
        // A tenth of slack under, so a reading a little under the reference doesn't cost a whole
        // step of the budget.
        (ratio - 0.1).ceil().max(1.0) as u64
    })
}

/// How long this computer takes to do a fixed amount of integer work spread over every core, the
/// best of three runs: a measure of its speed that cores alone don't give, since a runner with as
/// many cores can be slower or faster than another.
fn speed_probe() -> Duration {
    const ROUNDS: u64 = 300_000_000;
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    (0..3)
        .map(|_| {
            let started = Instant::now();
            std::thread::scope(|scope| {
                for _ in 0..cores {
                    scope.spawn(|| {
                        let mut mixed = 0x9e37_79b9_7f4a_7c15_u64;
                        for _ in 0..ROUNDS / cores as u64 {
                            mixed =
                                mixed.rotate_left(13) ^ mixed.wrapping_mul(0xff51_afd7_ed55_8ccd);
                        }
                        std::hint::black_box(mixed);
                    });
                }
            });
            started.elapsed()
        })
        .min()
        .unwrap_or_default()
}

/// How many times slower this computer is than the reference one: 1 there. A solver's time for
/// one question is stretched by it, as a proof's whole time is by [`scale`]: a question that
/// takes 6 s on the reference computer took longer than a fixed 10 s on a slow one, and z3 gave
/// up ("a path couldn't be settled: z3 ran out of time") in a proof the budget had time for.
fn slowness() -> f64 {
    scale() as f64 / REFERENCE_SCALE
}

/// Prints what [`speed_probe`] reads here and the scale it makes, to check [`REFERENCE_PROBE_MS`]
/// against the runners' own numbers (dispatch the ignored tests with this filter).
#[test]
#[ignore = "prints this computer's speed probe, to calibrate the budgets' scale by"]
fn the_speed_probe_reads() {
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let probe = speed_probe();
    eprintln!(
        "speed probe: {} ms on {cores} cores (the scale is {})",
        probe.as_millis(),
        scale()
    );
    assert!(probe > Duration::ZERO);
}

/// [`super::prove`], in turn with the other proofs, its budget's time counted from its turn.
fn prove(machine: &Machine, block: &Generated, budget: &Budget) -> Verdict {
    let asked = Instant::now();
    let scale = scale();
    let long =
        budget.deadline.saturating_duration_since(asked) > Duration::from_secs(ALONE_PAST * scale);
    // The whole computer, or a share of it, held until the proof ends.
    let (_alone, _shared) = if long {
        let alone = TURNS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (Some(alone), None)
    } else {
        let shared = TURNS
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (None, Some(shared))
    };
    let mut budget = budget.clone();
    budget.deadline += asked.elapsed();
    super::prove(machine, block, &budget)
}

/// [`prove`] with the whole computer, whatever its budget: for a test that asserts how far a proof
/// gets in its time, which it reaches only with every core.
fn prove_alone(machine: &Machine, block: &Generated, budget: &Budget) -> Verdict {
    let asked = Instant::now();
    let _alone = TURNS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut budget = budget.clone();
    budget.deadline += asked.elapsed();
    super::prove(machine, block, &budget)
}

async fn machine(source: &str) -> Machine {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let folder = tempfile::tempdir().expect("temporary folder");
    let (assembly, object, executable) = (
        folder.path().join("main.s"),
        folder.path().join("main.o"),
        folder.path().join("main"),
    );
    std::fs::write(&assembly, source).expect("write");
    // main.s's first line says which CPU's code it is.
    let profile = if source.starts_with(".arch armv8-a") {
        Profile::LinuxAarch64
    } else {
        Profile::LinuxX86_64
    };
    toolchain
        .assemble(profile, &assembly, &object)
        .await
        .expect("assemble");
    toolchain
        .link(profile, &object, &executable)
        .await
        .expect("link");
    let mut machine =
        crate::checker::decode::read(&std::fs::read(&executable).expect("read")).expect("decode");
    // Which symbol each [rip + name] names, and each checked jump table's cases, as the server
    // passes them on.
    let (parsed, _) = crate::checker::check_layout(source);
    let lengua = crate::checker::lengua_source(profile, source);
    // The CPU that runs it: this one, or under QEMU the emulated one, with its SVE length.
    let host = crate::platform::Machine::detect();
    let features = if profile == Profile::LinuxAarch64 && !cfg!(target_arch = "aarch64") {
        crate::platform::Target {
            profile,
            emulator: Some(crate::platform::Emulator {
                name: "qemu-aarch64".into(),
            }),
        }
        .features(&host)
    } else {
        host.features
    };
    machine.sve_bytes = features.sve;
    // An x86-64 program is mapped for every extension, as the prover golden maps it: a proof is
    // math, and doesn't depend on which CPU the computer has (GitHub's x86-64 runners are a
    // mixed pool, about half without AVX-512).
    let features = if profile == Profile::LinuxX86_64 {
        crate::platform::CpuFeatures::all()
    } else {
        features
    };
    if let Some(program) = crate::checker::decode::map(
        &parsed,
        lengua.as_ref(),
        &machine,
        features,
        &mut Vec::new(),
    ) {
        machine.named = program.named_symbols();
        let jumps = crate::checker::tables::every(&program, &mut Vec::new());
        machine.tables = crate::checker::tables::addresses(&program, &jumps);
    }
    machine
}

fn block(idea: &str, index: usize) -> Generated {
    let idea = crate::idea::parse(idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    let suite = crate::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
    suite.generated[index].clone()
}

/// A proof's budget of `seconds` on the computer these tests are set for, stretched by [`scale`]
/// on a slower one, so a proof that finishes there finishes here too.
fn budget(seconds: u64) -> Budget {
    Budget {
        max_paths: 64,
        max_shapes: 2000,
        question_ms: (10_000.0 * slowness()).round() as u64,
        ..Budget::until(
            Instant::now() + Duration::from_secs(seconds * scale()),
            false,
        )
    }
}

/// The text of a file in the compile skill's assets.
fn asset(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets")
            .join(name),
    )
    .expect("an asset")
}

/// A program the repository has for both targets, compiled from the very same idea: the folder
/// that holds its `idea.md` and `main.s` on each. A case that proves it is written once, with a
/// cell for each target where the two differ (an edit that plants a bug, an assembly source).
type Folders = PerCpu<&'static str>;

/// A change that plants a bug in a program: the first `from` in its main.s becomes `to`.
type Edit = (&'static str, &'static str);

/// The text of `file` in `target`'s folder of a program.
fn twin(folders: &Folders, target: Profile, file: &str) -> String {
    repository_file(&format!("{}/{file}", folders.get(target)))
}

/// `source` with the edits made in order, each `from` being in it as the edits before left it.
fn edited(source: &str, edits: &[(&str, &str)]) -> String {
    let mut text = source.to_owned();
    for &(from, to) in edits {
        assert!(text.contains(from), "the program has no {from:?}");
        text = text.replacen(from, to, 1);
    }
    assert_ne!(text, source, "the edits changed nothing");
    text
}

/// The template of each target, in the compile skill's assets.
const TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: "main-template.s",
    aarch64: "linux-aarch64/main-template.s",
};

/// The template is proven for every input from its idea, every generated block of it, on each
/// target. On AArch64 the digits, which `udiv` and `msub` take off one division at a time, are
/// the formula's remainders, and the overflow checks, a product's high half against its low
/// half's sign, settle in the values' ranges, so the proof needs no solver question.
#[tokio::test]
async fn proves_the_template_for_every_input() {
    if !solvers_installed() {
        return;
    }
    let idea = asset("template-idea.md");
    let suite = {
        let idea = crate::idea::parse(&idea).unwrap_or_else(|problems| panic!("{problems:?}"));
        crate::testfile::check(&idea).unwrap_or_else(|problems| panic!("{problems:?}"))
    };
    for target in Profile::ALL {
        let machine = machine(&asset(TEMPLATES.get(target))).await;
        for (number, _) in suite.generated.iter().enumerate() {
            let block = block(&idea, number);
            let started = Instant::now();
            let verdict = prove(&machine, &block, &budget(120));
            eprintln!(
                "block {number} on {}: {verdict:?} in {:?}",
                target.name(),
                started.elapsed()
            );
            assert!(
                matches!(verdict, Verdict::Proven { .. }),
                "block {number} on {}: {verdict:?}",
                target.name()
            );
        }
    }
}

/// The folders of the streaming `cat`.
const CAT_STREAM: Folders = PerCpu {
    x86_64: "tests/fixtures/cat-stream",
    aarch64: "tests/fixtures/linux-aarch64/cat-stream",
};

/// A streaming filter through `fill`, `cat`, is proven for every length by induction over its
/// read loop, on each target.
#[tokio::test]
async fn proves_a_streaming_filter_for_every_length() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&CAT_STREAM, target, "idea.md");
        let source = twin(&CAT_STREAM, target, "main.s");
        let machine = machine(&source).await;
        let block = block(&idea, 0);
        let started = Instant::now();
        let verdict = prove(&machine, &block, &long_budget());
        eprintln!(
            "the cat on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(_),
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The prover finds lengua's own `fill` in a program that calls it, on each target.
#[tokio::test]
async fn finds_the_library_fill_on_every_cpu() {
    for target in Profile::ALL {
        let machine = machine(&twin(&CAT_STREAM, target, "main.s")).await;
        let image = super::exec::Image::new(&machine).expect("an image of it");
        assert!(image.has_fill(), "{}", target.name());
        assert!(image.fill_head().is_some(), "{}", target.name());
    }
}

/// The folders of the hypotenuse.
const HYPOTENUSE: Folders = PerCpu {
    x86_64: "examples/hypotenuse",
    aarch64: "tests/fixtures/linux-aarch64/hypotenuse",
};

/// The hypotenuse's scale, 1000.0, read from a `.double` in .rodata in place of made from a
/// whole number, on each target.
const SCALE_FROM_RODATA: PerCpu<&[Edit]> = PerCpu {
    x86_64: &[
        (
            ".type usage_msg, @object\n",
            ".type thousand, @object\nthousand:\n    .double 1000.0\n.size thousand, . - thousand\n\n\
             .type usage_msg, @object\n",
        ),
        (
            "    mov eax, UNITS\n    cvtsi2sd xmm2, rax\n",
            "    movsd xmm2, qword ptr [rip + thousand]\n",
        ),
    ],
    aarch64: &[
        (
            ".type usage_msg, @object\n",
            ".type thousand, @object\nthousand:\n    .double 1000.0\n.size thousand, . - thousand\n\n\
             .type usage_msg, @object\n",
        ),
        (
            "    mov x2, #UNITS\n    scvtf d2, x2\n",
            "    adrp x2, thousand\n    add x2, x2, :lo12:thousand\n    ldr d2, [x2]\n",
        ),
    ],
};

/// A pointer moved one double past the scale, which is read after it: on AArch64 the load moves
/// it as it goes (`ldr d2, [x2], #8`), on x86-64 an `add`.
const ONE_DOUBLE_PAST_THE_SCALE: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    movsd xmm2, qword ptr [rip + thousand]\n",
        "    lea rcx, [rip + thousand]\n    movsd xmm2, qword ptr [rcx]\n    add rcx, 8\n    \
         movsd xmm3, qword ptr [rcx]\n",
    ),
    aarch64: (
        "    ldr d2, [x2]\n",
        "    ldr d2, [x2], #8\n    ldr d3, [x2]\n",
    ),
};

/// The result rounded toward zero rather than to the nearest, on each target.
const ROUNDED_TOWARD_ZERO: PerCpu<Edit> = PerCpu {
    x86_64: ("    cvtsd2si rdi, xmm0\n", "    cvttsd2si rdi, xmm0\n"),
    aarch64: ("    frintn d0, d0\n", "    frintz d0, d0\n"),
};

/// The hypotenuse, from the example's idea: its doubles are the formula's step by step, for
/// every pair of whole sides the table draws and for decimal sides over their whole range, where
/// the program divides each side's thousandths by 1000, squares, adds, takes the square root and
/// scales back, the formula's own steps, so the two outputs are one float computed alike. The
/// scale read from a `.double` is the same double; a pointer moved one double past it is caught,
/// rather than taken for usage_msg's bytes, which come next; and rounding toward zero rather
/// than to the nearest is caught.
#[tokio::test]
async fn proves_the_hypotenuse() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let name = target.name();
        let idea = twin(&HYPOTENUSE, target, "idea.md");
        let source = twin(&HYPOTENUSE, target, "main.s");
        let right = machine(&source).await;
        for (number, seconds) in [(0, 120), (1, 240)] {
            let block = block(&idea, number);
            let started = Instant::now();
            let verdict = prove(&right, &block, &budget(seconds));
            eprintln!(
                "hypotenuse block {number} on {name}: {verdict:?} in {:?}",
                started.elapsed()
            );
            assert!(
                matches!(verdict, Verdict::Proven { .. }),
                "block {number} on {name}: {verdict:?}"
            );
        }
        let constant = edited(&source, SCALE_FROM_RODATA.get(target));
        let verdict = prove(&machine(&constant).await, &block(&idea, 1), &budget(240));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{name}: {verdict:?}"
        );
        let past = edited(&constant, &[*ONE_DOUBLE_PAST_THE_SCALE.get(target)]);
        let verdict = prove(&machine(&past).await, &block(&idea, 0), &budget(120));
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{name}: {verdict:?}");
        };
        let Some(Value::Str(problem)) = found.get(UNSAFE) else {
            panic!("{name}: {found:?}");
        };
        assert!(problem.contains("thousand"), "{name}: {problem}");
        let toward_zero = edited(&source, &[*ROUNDED_TOWARD_ZERO.get(target)]);
        let verdict = prove(&machine(&toward_zero).await, &block(&idea, 0), &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
    }
}

/// The hypotenuse with both sides in one vector of two doubles, on each target: the sides
/// converted and divided by the scale, in both lanes, and squared in one step, then the two
/// squares added.
const HYPOTENUSE_ON_VECTORS: PerCpu<fn(&str) -> String> = PerCpu {
    x86_64: hypotenuse_on_vectors_x86_64,
    aarch64: crate::checker::tests::on_vectors,
};

/// x86-64's hypotenuse on packed doubles: `unpcklpd` puts the sides in one register and the
/// scale in both lanes of another, `divpd` and `mulpd` work on both, and `unpckhpd` brings the
/// second square down to be added by `addsd`.
fn hypotenuse_on_vectors_x86_64(text: &str) -> String {
    edited(
        text,
        &[(
            "    cvtsi2sd xmm2, rax\n    cvtsi2sd xmm0, rdi\n    divsd xmm0, xmm2\n    \
             cvtsi2sd xmm1, rsi\n    divsd xmm1, xmm2\n    mulsd xmm0, xmm0\n    \
             mulsd xmm1, xmm1\n    addsd xmm0, xmm1\n",
            "    cvtsi2sd xmm2, rax\n    unpcklpd xmm2, xmm2\n    cvtsi2sd xmm0, rdi\n    \
             cvtsi2sd xmm1, rsi\n    unpcklpd xmm0, xmm1\n    divpd xmm0, xmm2\n    \
             mulpd xmm0, xmm0\n    movapd xmm1, xmm0\n    unpckhpd xmm1, xmm1\n    \
             addsd xmm0, xmm1\n",
        )],
    )
}

/// The scale taken from the lane above it, which holds 0, on each target.
const SCALE_FROM_THE_OTHER_LANE: PerCpu<Edit> = PerCpu {
    x86_64: ("    unpcklpd xmm2, xmm2\n", "    unpckhpd xmm2, xmm2\n"),
    aarch64: ("    dup v1.2d, v2.d[0]\n", "    dup v1.2d, v2.d[1]\n"),
};

/// On vectors, both sides in one of two doubles, each step rounds as the scalar one does: the
/// same double, so the same proofs. The scale taken from the lane above it, which holds 0, is
/// caught.
#[tokio::test]
async fn proves_the_hypotenuse_on_vectors() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let name = target.name();
        let idea = twin(&HYPOTENUSE, target, "idea.md");
        let on_vectors = *HYPOTENUSE_ON_VECTORS.get(target);
        let vectors = on_vectors(&twin(&HYPOTENUSE, target, "main.s"));
        let right = machine(&vectors).await;
        for (number, seconds) in [(0, 120), (1, 240)] {
            let verdict = prove(&right, &block(&idea, number), &budget(seconds));
            assert!(
                matches!(verdict, Verdict::Proven { .. }),
                "on vectors, block {number} on {name}: {verdict:?}"
            );
        }
        let other_lane = edited(&vectors, &[*SCALE_FROM_THE_OTHER_LANE.get(target)]);
        let verdict = prove(&machine(&other_lane).await, &block(&idea, 0), &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
    }
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

/// The folders of the sum of two whole numbers.
const ADD: Folders = PerCpu {
    x86_64: "examples/add",
    aarch64: "tests/fixtures/linux-aarch64/add",
};

/// About a minute on 16 cores in release mode: `cargo test --release -- --ignored`.
#[tokio::test]
#[ignore = "slow: proves 4×10^18 pairs of inputs"]
async fn proves_add_for_every_pair_of_inputs() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let machine = machine(&twin(&ADD, target, "main.s")).await;
        let block = block(&twin(&ADD, target, "idea.md"), 0);
        let started = Instant::now();
        let verdict = prove(&machine, &block, &budget(300));
        eprintln!(
            "add on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The bug that comes out one input in 18 quintillion, planted on each target: the negative of
/// 1234567 comes out one too big. AArch64 builds the number from a `mov` and a `movk`.
const UNLUCKY_NUMBER: PerCpu<&[Edit]> = PerCpu {
    x86_64: &[
        (
            ".equ OUT_BUF_LEN, 21\n",
            ".equ OUT_BUF_LEN, 21\n.equ UNLUCKY, -1234567\n",
        ),
        (
            "    neg rdx\n    jo .Lmain_assert_negate\n",
            "    neg rdx\n    jo .Lmain_assert_negate\n    cmp rdx, UNLUCKY\n    \
             jne .Lmain_lucky\n    inc rdx\n.Lmain_lucky:\n",
        ),
    ],
    aarch64: &[(
        "    negs x0, x1\n    b.vs .Lmain_assert_negate\n",
        "    negs x0, x1\n    b.vs .Lmain_assert_negate\n    mov x2, #0xd687\n    \
         movk x2, #0x12, lsl #16\n    neg x2, x2\n    cmp x0, x2\n    b.ne .Lmain_lucky\n    \
         add x0, x0, #1\n.Lmain_lucky:\n",
    )],
};

#[tokio::test]
async fn finds_a_bug_the_samples_miss() {
    if !solvers_installed() {
        return;
    }
    let block = block(&asset("template-idea.md"), 0);
    for target in Profile::ALL {
        let buggy = edited(&asset(TEMPLATES.get(target)), UNLUCKY_NUMBER.get(target));
        let machine = machine(&buggy).await;
        // Seconds alone; minutes when every test runs at once, with each question slower too.
        let verdict = prove(
            &machine,
            &block,
            &Budget {
                question_ms: (60_000.0 * slowness()).round() as u64,
                ..budget(600)
            },
        );
        assert_eq!(
            verdict,
            Verdict::Counterexample(BTreeMap::from([("n".to_owned(), Value::Int(1_234_567))])),
            "{}",
            target.name()
        );
    }
}

/// A bug planted in a target's template that the checker's rules can't see: the edits, and what
/// the proof reports of it.
#[derive(Clone, Copy)]
struct MemoryBug {
    edits: &'static [Edit],
    needle: &'static str,
}

/// An object right after out_buf, so a byte past out_buf's end is its first.
const SPARE: Edit = (
    ".size out_buf, . - out_buf\n",
    ".size out_buf, . - out_buf\n\n.type spare, @object\nspare:\n    .zero 8\n.size spare, . \
     - spare\n",
);

/// The memory bugs the checker's rules can't see, planted in the template: each has a cell for
/// every target, and the proof reports what broke in that CPU's words. x86-64's frame is below
/// the return address its `call` pushed, so a pointer just past it reaches that address;
/// AArch64's frame record holds the return address inside the frame, so the same bug is a
/// pointer into the record, and a pointer past the frame reaches the caller's.
const MEMORY_BUGS: &[(&str, PerCpu<MemoryBug>)] = &[
    (
        "digits written from 8 bytes into out_buf run past its end",
        PerCpu {
            x86_64: MemoryBug {
                edits: &[(
                    "    lea rsi, [rip + out_buf]\n    mov ecx, OUT_BUF_LEN - 1\n",
                    "    lea rsi, [rip + out_buf + 8]\n    mov ecx, OUT_BUF_LEN - 1\n",
                )],
                needle: "writes 1 byte at byte 27 of out_buf, which has 21 bytes",
            },
            aarch64: MemoryBug {
                edits: &[(
                    "    strb w10, [x1, #OUT_BUF_LEN - 1]\n    mov x2, #OUT_BUF_LEN - 1\n",
                    "    strb w10, [x1, #OUT_BUF_LEN - 1]\n    add x1, x1, #8\n    \
                     mov x2, #OUT_BUF_LEN - 1\n",
                )],
                needle: "writes 1 byte at byte 27 of out_buf, which has 21 bytes",
            },
        },
    ),
    (
        "write_all is lent write_i64's frame, which nothing wrote",
        PerCpu {
            x86_64: MemoryBug {
                edits: &[(
                    "    mov edi, STDOUT\n    call write_all\n",
                    "    mov edi, STDOUT\n    mov rsi, rsp\n    mov edx, 8\n    call write_all\n",
                )],
                needle: "reads byte 0 of write_i64's frame, which has 8 bytes, which nothing wrote",
            },
            // The frame record's 16 bytes are written, so the frame lent is 16 bytes below it.
            aarch64: MemoryBug {
                edits: &[(
                    "    mov x0, #STDOUT\n    bl write_all\n",
                    "    mov x0, #STDOUT\n    sub sp, sp, #16\n    mov x1, sp\n    mov x2, #8\n    \
                     bl write_all\n",
                )],
                needle: "reads byte 0 of write_i64's frame, which has 32 bytes, which nothing wrote",
            },
        },
    ),
    (
        "a pointer writes the return address of write_i64",
        PerCpu {
            // Just past write_i64's frame of 8 bytes.
            x86_64: MemoryBug {
                edits: &[(
                    ".Lwrite_i64_return:\n    add rsp, 8\n",
                    ".Lwrite_i64_return:\n    lea rcx, [rsp + 8]\n    mov byte ptr [rcx], 0\n    \
                     add rsp, 8\n",
                )],
                needle: "so it reaches the return address of write_i64",
            },
            // Into the frame record, where x30 was saved.
            aarch64: MemoryBug {
                edits: &[(
                    ".Lwrite_i64_return:\n    ldp x29, x30, [sp], #16\n",
                    ".Lwrite_i64_return:\n    add x1, sp, #8\n    strb wzr, [x1]\n    \
                     ldp x29, x30, [sp], #16\n",
                )],
                needle: "writes over the return address of write_i64",
            },
        },
    ),
    (
        "a loop that clears one byte too many: its pointer, one step past out_buf, writes \
         spare's first byte, which nothing prints",
        PerCpu {
            x86_64: MemoryBug {
                edits: &[
                    SPARE,
                    (
                        ".Lwrite_i64_digits:\n",
                        ".Lwrite_i64_digits:\n    lea rsi, [rip + out_buf]\n    \
                         mov ecx, OUT_BUF_LEN + 1\n.Lwrite_i64_clear:  # bound: 22\n    \
                         mov byte ptr [rsi], 0\n    inc rsi\n    dec ecx\n    \
                         jnz .Lwrite_i64_clear\n",
                    ),
                ],
                needle: "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches \
                         byte 0 of spare, which has 8 bytes",
            },
            aarch64: MemoryBug {
                edits: &[
                    SPARE,
                    (
                        ".Lwrite_i64_digits:\n",
                        ".Lwrite_i64_digits:\n    adrp x4, out_buf\n    add x4, x4, :lo12:out_buf\n    \
                         mov x5, #OUT_BUF_LEN + 1\n.Lwrite_i64_clear:  // bound: 22\n    \
                         strb wzr, [x4]\n    add x4, x4, #1\n    subs x5, x5, #1\n    \
                         b.ne .Lwrite_i64_clear\n",
                    ),
                ],
                needle: "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches \
                         byte 0 of spare, which has 8 bytes",
            },
        },
    ),
    (
        "a pointer to out_buf's end, as main.s names it, is out_buf's, not spare's",
        PerCpu {
            x86_64: MemoryBug {
                edits: &[
                    SPARE,
                    (
                        ".Lwrite_i64_digits:\n",
                        ".Lwrite_i64_digits:\n    lea rsi, [rip + out_buf + OUT_BUF_LEN]\n    \
                         mov byte ptr [rsi], 0\n",
                    ),
                ],
                needle: "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches \
                         byte 0 of spare",
            },
            aarch64: MemoryBug {
                edits: &[
                    SPARE,
                    (
                        ".Lwrite_i64_digits:\n",
                        ".Lwrite_i64_digits:\n    adrp x4, out_buf\n    add x4, x4, :lo12:out_buf\n    \
                         add x4, x4, #OUT_BUF_LEN\n    strb wzr, [x4]\n",
                    ),
                ],
                needle: "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches \
                         byte 0 of spare",
            },
        },
    ),
    (
        "a pointer stepped past the program's path, the first argument string, reads the next",
        PerCpu {
            x86_64: MemoryBug {
                edits: &[(
                    "    mov rdi, qword ptr [rsi + 8]\n",
                    "    mov rdi, qword ptr [rsi]\n    add rdi, 13\n    \
                     movzx ecx, byte ptr [rdi]\n    mov rdi, qword ptr [rsi + 8]\n",
                )],
                needle: "reads 1 byte at byte 13 of the program's argument 0, which has 13 bytes",
            },
            aarch64: MemoryBug {
                edits: &[(
                    "    ldr x0, [x1, #8]\n",
                    "    ldr x2, [x1]\n    add x2, x2, #13\n    ldrb w3, [x2]\n    \
                     ldr x0, [x1, #8]\n",
                )],
                needle: "reads 1 byte at byte 13 of the program's argument 0, which has 13 bytes",
            },
        },
    ),
    (
        "the return address written where the function started",
        PerCpu {
            // write_all's return address is at [rsp] when it starts.
            x86_64: MemoryBug {
                edits: &[("write_all:\n", "write_all:\n    mov qword ptr [rsp], 0\n")],
                needle: "writes over the return address of write_all",
            },
            // The return address a function saved in its frame record, written through memory.
            aarch64: MemoryBug {
                edits: &[(
                    "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
                    "write_i64:\n    stp x29, x30, [sp, #-16]!\n    str xzr, [sp, #8]\n",
                )],
                needle: "writes over the return address of write_i64",
            },
        },
    ),
];

/// Memory bugs the checker's rules can't see, planted in each target's template: each run of the
/// model holds every access to its object, and the proof reports what broke. Every row is tried
/// on every target, and what each got wrong is listed together.
#[tokio::test]
async fn finds_memory_bugs_the_rules_cannot_see() {
    if !solvers_installed() {
        return;
    }
    let block = block(&asset("template-idea.md"), 0);
    let mut problems = Vec::new();
    for (name, cells) in MEMORY_BUGS {
        for (target, cell) in cells.cells() {
            let buggy = edited(&asset(TEMPLATES.get(target)), cell.edits);
            let machine = machine(&buggy).await;
            let verdict = prove(&machine, &block, &budget(120));
            let found = match &verdict {
                Verdict::Counterexample(values) => match values.get(UNSAFE) {
                    Some(Value::Str(problem)) if problem.contains(cell.needle) => continue,
                    other => format!("{other:?}"),
                },
                other => format!("{other:?}"),
            };
            problems.push(format!(
                "{name} on {}: wanted {:?}, found {found}",
                target.name(),
                cell.needle
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The folders of the absolute difference.
const ABSDIFF: Folders = PerCpu {
    x86_64: "examples/absdiff",
    aarch64: "tests/fixtures/linux-aarch64/absdiff",
};

/// The absolute difference, which branches on the sign of a - b where the formula has abs: the
/// path's own condition must settle the formula's if. Both CPUs' proofs are the very same by the
/// terms alone: 196 shapes, 420 paths and no question (x86-64: run 41911020648; AArch64: run
/// 41921354636). Before AArch64's flags were set as x86-64's (the overflow flag false where the
/// operands' ranges keep the result in range, and `tbnz x0, #63` read as `x0 < 0`), its proof asked
/// the solvers whether the path that fails the abs's assertion could be taken, and they ran out of
/// time (runs 41911020648 and 41911025872).
#[tokio::test]
async fn proves_absolute_difference() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let machine = machine(&twin(&ABSDIFF, target, "main.s")).await;
        let block = block(&twin(&ABSDIFF, target, "idea.md"), 0);
        let started = Instant::now();
        let verdict = prove(&machine, &block, &budget(300));
        eprintln!(
            "absdiff on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(verdict, Verdict::Proven { questions: 0, .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The folders of the mean of two numbers.
const MEAN: Folders = PerCpu {
    x86_64: "examples/mean",
    aarch64: "tests/fixtures/linux-aarch64/mean",
};

/// The mean of two prices in cents, rounded half away from zero: decimals in, decimals out. The
/// parse and the input are one sum; the formula halves at 128 bits and the program at 64, both by
/// a shift, and their digits meet as one term, so the solvers are left how many there are.
#[tokio::test]
#[ignore = "slow: a minute or more"]
async fn proves_the_mean_of_two_prices() {
    for target in Profile::ALL {
        let machine = machine(&twin(&MEAN, target, "main.s")).await;
        let block = block(&twin(&MEAN, target, "idea.md"), 0);
        let started = Instant::now();
        let verdict = prove(&machine, &block, &budget(300));
        eprintln!(
            "mean on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The folders of the program that opens a file its argument names.
const ARCHIVE: Folders = PerCpu {
    x86_64: "examples/archive",
    aarch64: "tests/fixtures/linux-aarch64/archive",
};

/// A file name the input chooses: archive opens the file its argument names, which no test file
/// is, so the run where it's none of them is the only one, and the name it prints is the
/// input's own.
#[tokio::test]
async fn proves_a_file_name_the_input_chooses() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let machine = machine(&twin(&ARCHIVE, target, "main.s")).await;
        let block = block(&twin(&ARCHIVE, target, "idea.md"), 2);
        let verdict = prove(&machine, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The mean of two whole numbers: the program halves a sum of cents with one away from zero
/// added, and the formula multiplies the sum by 50. A sum every part of which the divisor
/// divides divides part by part, so the two meet without the solvers dividing. Both CPUs' proofs
/// are the very same by the terms alone: 196 shapes, 448 paths and no question (x86-64: run
/// 41911020648; AArch64: run 41923532882). AArch64's stopped ("a path couldn't be settled")
/// before `msub` of a quotient and its divisor from the dividend gave the remainder: the program's
/// digits, a sum less ten times its half, did not meet the formula's `bvurem` of it, and the
/// solvers ran out of time on them (runs 41911020648 and 41911025872).
#[tokio::test]
async fn proves_the_mean_of_whole_numbers() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let machine = machine(&twin(&MEAN, target, "main.s")).await;
        let block = block(&twin(&MEAN, target, "idea.md"), 1);
        let started = Instant::now();
        let verdict = prove(&machine, &block, &budget(240));
        eprintln!(
            "mean B2 on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(verdict, Verdict::Proven { questions: 0, .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// Decimal inputs: mean's usage (B4), a bad second price drawn by choice (B5) and a price out of
/// range (B6), each with a decimal drawn over its whole range.
#[tokio::test]
async fn proves_decimal_inputs_where_the_output_is_fixed() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let machine = machine(&twin(&MEAN, target, "main.s")).await;
        let idea = twin(&MEAN, target, "idea.md");
        for index in 3..6 {
            let block = block(&idea, index);
            let verdict = prove(&machine, &block, &budget(120));
            assert!(
                matches!(verdict, Verdict::Proven { .. }),
                "{} on {}: {verdict:?}",
                block.id,
                target.name()
            );
        }
    }
}

/// The idea of the die.
const ROLL_IDEA: &str = r#"---
type: lengua/program-idea
title: roll
description: Rolls a die.
---
# Purpose
The program rolls a die for a player numbered from `1` to `9`.

# Behaviors
## B1: Rolls a die
- Given the argument `1`
- When the program runs
- Then it prints text that matches `rolled {any(1, 6)}` and a newline
- And it exits with code `0`

### For any `player`
- Given the argument `{player}`
- When the program runs
- Then it prints `rolled {any(1, 6)}` and a newline
- And it exits with code `0`

| player |
|---|
| `int(1, 9)` |

# Constraints
- Uses: random bytes
"#;

/// The die on x86-64: `getrandom` (318) for one byte, which `div` takes modulo `faces`.
fn roll_x86_64(faces: u8) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    mov eax, 318
    lea rdi, [rip + random]
    mov esi, 1
    xor edx, edx
    syscall
    movzx eax, byte ptr [rip + random]
    xor edx, edx
    mov ecx, {faces}
    div ecx
    add edx, 49
    mov byte ptr [rip + line + 7], dl
    mov eax, 1
    mov edi, 1
    lea rsi, [rip + line]
    mov edx, 9
    syscall
    mov eax, 60
    xor edi, edi
    syscall
    .size _start, . - _start
    .data
    .type line, @object
line:
    .ascii \"rolled ?\\n\"
    .size line, 9
    .type random, @object
random:
    .byte 0
    .size random, 1
"
    )
}

/// The die on AArch64: `getrandom` (278) for one byte, which `udiv` and `msub` take modulo
/// `faces`.
fn roll_aarch64(faces: u8) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    adrp x0, random
    add x0, x0, :lo12:random
    mov x1, #1
    mov x2, #0
    mov x8, #278
    svc #0
    adrp x1, random
    add x1, x1, :lo12:random
    ldrb w2, [x1]
    mov w3, #{faces}
    udiv w4, w2, w3
    msub w5, w4, w3, w2
    add w5, w5, #49
    adrp x6, line
    add x6, x6, :lo12:line
    strb w5, [x6, #7]
    mov x0, #1
    mov x1, x6
    mov x2, #9
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.data
.type line, @object
line:
    .ascii \"rolled ?\\n\"
.size line, 9
.type random, @object
random:
    .byte 0
.size random, 1
"
    )
}

/// The die on each target.
const ROLL: PerCpu<fn(u8) -> String> = PerCpu {
    x86_64: roll_x86_64,
    aarch64: roll_aarch64,
};

/// A die: one random byte from the kernel, printed as 1 to 6. The kernel's byte is any value, so
/// the proof covers all 256; the rule `{any(1, 6)}` holds for each, and `% 7` breaks it.
#[tokio::test]
async fn proves_a_roll_stays_in_its_range() {
    if !solvers_installed() {
        return;
    }
    let block = block(ROLL_IDEA, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    for target in Profile::ALL {
        let program = *ROLL.get(target);
        let right = machine(&program(6)).await;
        let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
        assert!(found.is_none(), "{}: {found:?}", target.name());
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
        let wrong = machine(&program(7)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The idea of the weekday modulo 7.
const MODULO_IDEA: &str = r#"---
type: lengua/program-idea
title: weekday
description: Prints the day of the week.
---
# Purpose
The program exits with the clock's seconds since 1970 modulo 7, from `0` to `6`.

# Behaviors
## B1: Exits with the seconds modulo 7
- Given the clock at `1735689600`
- When the program runs
- Then it exits with code `1`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints nothing
- And it exits with code `{t % 7}`

| t |
|---|
| `int(0, 4102444800)` |

# Constraints
- Uses: the clock
"#;

/// The clock's seconds modulo `days` on x86-64: `clock_gettime` is 228 there.
fn modulo_x86_64(days: u32) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    mov eax, 228
    xor edi, edi
    lea rsi, [rip + time]
    syscall
    mov rax, qword ptr [rip + time]
    xor edx, edx
    mov ecx, {days}
    div rcx
    mov edi, edx
    mov eax, 60
    syscall
    .size _start, . - _start
    .data
    .type time, @object
time:
    .zero 16
    .size time, 16
"
    )
}

/// The clock's seconds modulo `days` on AArch64: `clock_gettime` is 113 there, and the
/// remainder `msub`'s.
fn modulo_aarch64(days: u32) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, time
    add x1, x1, :lo12:time
    mov x8, #113
    svc #0
    adrp x1, time
    add x1, x1, :lo12:time
    ldr x2, [x1]
    mov x3, #{days}
    udiv x4, x2, x3
    msub x0, x4, x3, x2
    mov x8, #93
    svc #0
.size _start, . - _start
.data
.type time, @object
time:
    .zero 16
.size time, 16
"
    )
}

/// The seconds modulo `days` on each target.
const MODULO: PerCpu<fn(u32) -> String> = PerCpu {
    x86_64: modulo_x86_64,
    aarch64: modulo_aarch64,
};

/// A clock reading the test supplies, taken modulo 7: proven for every second from 1970 to 2100,
/// one shape, since the program never reads the time as digits. (Days of the week need a second
/// 64-bit division, which cvc5 doesn't confirm in time.)
#[tokio::test]
async fn proves_what_a_program_does_with_the_supplied_clock() {
    if !solvers_installed() {
        return;
    }
    let block = block(MODULO_IDEA, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    for target in Profile::ALL {
        let program = *MODULO.get(target);
        let right = machine(&program(7)).await;
        let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
        assert!(found.is_none(), "{}: {found:?}", target.name());
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
        let wrong = machine(&program(6)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

const ECHO_IDEA: &str = r#"---
type: lengua/program-idea
title: echo
description: Prints its argument.
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
"#;

/// Writes its argument: every branch is settled by the bytes' ranges, and the output is the
/// argument's own terms. One program for each target.
const ECHO: PerCpu<&str> = PerCpu {
    x86_64: ".intel_syntax noprefix
.globl _start
.text
.type _start, @function
_start:
    mov rsi, qword ptr [rsp + 16]
    xor edx, edx
.Llength:
    cmp byte ptr [rsi + rdx], 0
    je .Lwrite
    inc rdx
    jmp .Llength
.Lwrite:
    mov edi, 1
    mov eax, 1
    syscall
    mov edi, 0
    mov eax, 60
    syscall
.size _start, . - _start
",
    aarch64: ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    ldr x1, [sp, #16]
    mov x2, #0
.Llength:
    ldrb w3, [x1, x2]
    cbz w3, .Lwrite
    add x2, x2, #1
    b .Llength
.Lwrite:
    mov x0, #1
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
",
};

#[tokio::test]
async fn a_shape_its_terms_settle_starts_no_solver() {
    for target in Profile::ALL {
        let machine = machine(ECHO.get(target)).await;
        let block = block(ECHO_IDEA, 0);
        let image = Image::new(&machine).expect("an entry");
        let each = vec![(
            "s".to_owned(),
            parts("s", &block.vars[0].1, 0).expect("parts"),
        )];
        let mut solvers = Solvers::new(10_000);
        for combination in 0..3 {
            solvers.next();
            let result = prove_shape(
                &image,
                &block,
                &each,
                combination,
                &budget(60),
                &mut solvers,
                None,
            );
            assert!(
                matches!(result, Ok((1, 0, None, false))),
                "combination {combination} on {}",
                target.name()
            );
        }
        assert!(
            solvers.solver.is_none(),
            "no question, so no solver started on {}",
            target.name()
        );
        assert_eq!(
            prove(&machine, &block, &budget(60)),
            Verdict::Proven {
                shapes: 3,
                paths: 3,
                questions: 0,
                alone: None,
                bound: None,
                cut_any_way: false,
                unreached: Vec::new(),
                induction: None,
                lookback: 0,
            },
            "{}",
            target.name()
        );
    }
}

fn fixture(name: &str, file: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
            .join(file),
    )
    .expect("a fixture")
}

/// The folders of the CSV total.
const TOTAL: Folders = PerCpu {
    x86_64: "tests/fixtures/total",
    aarch64: "tests/fixtures/linux-aarch64/total",
};

/// The CSV total Claude Code wrote: its first block has 24,696 input combinations, more than
/// the old cap of 20,000, which each started their own solvers; the terms now settle every one.
#[tokio::test]
async fn proves_the_csv_totals_24696_combinations() {
    for target in Profile::ALL {
        let machine = machine(&twin(&TOTAL, target, "main.s")).await;
        let block = block(&twin(&TOTAL, target, "idea.md"), 0);
        let began = Instant::now();
        let verdict = prove(
            &machine,
            &block,
            &Budget {
                max_shapes: 2_000_000,
                max_paths: 256,
                ..budget(600)
            },
        );
        eprintln!("{} {verdict:?} in {:?}", target.name(), began.elapsed());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    shapes: 24_696,
                    questions: 0,
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The model's runs of the tests' samples are part of the proof's time: with it up, they stop and
/// the proof is "ran out of time", where it used to run each sample to the end.
#[tokio::test]
async fn the_models_runs_of_the_samples_stop_when_the_proofs_time_is_up() {
    for target in Profile::ALL {
        let machine = machine(&twin(&TOTAL, target, "main.s")).await;
        let block = block(&twin(&TOTAL, target, "idea.md"), 0);
        let samples: Vec<crate::testfile::Case> =
            block.cases(1).take(4).filter_map(Result::ok).collect();
        let up = Instant::now();
        let result = agrees_until(&machine, &samples, false, up);
        assert_eq!(
            result.err().as_deref(),
            Some(OUT_OF_TIME),
            "{}",
            target.name()
        );
        // With the time there, the same samples agree, as before.
        let in_time = Instant::now() + Duration::from_secs(600);
        let found = agrees_until(&machine, &samples, false, in_time).expect("the model agrees");
        assert!(found.is_none(), "{}: {found:?}", target.name());
    }
}

/// Where the CSV total's proof spends its time: a few combinations, one at a time, with
/// `LENGUA_PROVE_TRACE=1` for each one's questions.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn traces_a_few_csv_total_combinations() {
    for target in Profile::ALL {
        trace_a_few_csv_total_combinations(target).await;
    }
}

/// [`traces_a_few_csv_total_combinations`] on one target's CSV total.
async fn trace_a_few_csv_total_combinations(target: Profile) {
    let machine = machine(&twin(&TOTAL, target, "main.s")).await;
    let block = block(&twin(&TOTAL, target, "idea.md"), 0);
    let image = Image::new(&machine).expect("an entry");
    let random = block.random_only();
    let each: Vec<(String, Vec<Part>)> = block
        .vars
        .iter()
        .map(|(name, generator)| {
            assert!(!random.contains_key(name));
            (name.clone(), parts(name, generator, 0).expect("parts"))
        })
        .collect();
    let total: usize = each.iter().map(|(_, parts)| parts.len()).product();
    let mut solvers = Solvers::new(10_000);
    let stride = stride(total);
    let mut times = Vec::new();
    let all = Instant::now();
    let turns: Vec<usize> = match std::env::var("LENGUA_COMBINATION") {
        Ok(one) => vec![one.parse().expect("a combination")],
        Err(_) => (0..100).map(|turn| turn * stride % total).collect(),
    };
    for combination in turns {
        solvers.next();
        let began = Instant::now();
        let before = solvers.solver.as_ref().map_or(0, |solver| solver.questions);
        let result = prove_shape(
            &image,
            &block,
            &each,
            combination,
            &budget(120),
            &mut solvers,
            None,
        );
        let questions = solvers.solver.as_ref().map_or(0, |solver| solver.questions) - before;
        let outcome = match result {
            Ok((paths, ..)) => format!("{paths} paths"),
            Err(Stop::Unproven(reason) | Stop::Beyond(reason)) => reason,
            Err(Stop::Counterexample(values)) => format!("counterexample {values:?}"),
        };
        times.push((began.elapsed(), combination, questions, outcome));
    }
    times.sort_by_key(|(time, ..)| std::cmp::Reverse(*time));
    eprintln!(
        "100 combinations on {} in {:?}; the slowest:",
        target.name(),
        all.elapsed()
    );
    for (time, combination, questions, outcome) in times.iter().take(8) {
        eprintln!("  {combination}: {time:?}, {questions} questions, {outcome}");
    }
}

/// calc with its operator any of the three, one symbolic byte, the numbers fixed, so the run
/// forks at the jump table into every case. (A `choice` would run each value on its own.)
const CHOSEN_OPERATOR: &str = r#"---
type: lengua/program-idea
title: calc
description: Combines 6 and 3 with the operator that it gets.
---
# Purpose
The program combines 6 and 3 with the operator that it gets, `1` character. It prints their sum,
difference or product.

# Behaviors
## B1: Applies the operator
- Given the arguments `6`, `+` and `3`
- When the program runs
- Then it prints `9` and a newline
- And it exits with code `0`

### For any `op`
- Given the arguments `6`, `{op}` and `3`
- When the program runs
- Then it prints `{if(op == "+", 9, if(op == "-", 3, 18))}` and a newline
- And it exits with code `0`

| op | runs |
|---|---|
| `string(1, 1, "+-*")` | 10 |

# Constraints
- Uses: nothing
"#;

/// The folders of the calculator with a jump table.
const CALC: Folders = PerCpu {
    x86_64: "tests/fixtures/calc",
    aarch64: "tests/fixtures/linux-aarch64/calc",
};

/// A checked jump table (dialect 4): the proof follows every case the input can reach through
/// it, and only those, and finds a table whose entries send an operator to the wrong case. On
/// AArch64 the table is read with `adrp`, `ldrsw` and `br`.
#[tokio::test]
async fn proves_each_case_a_jump_table_can_take() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let name = target.name();
        let calc = twin(&CALC, target, "main.s");
        let right = machine(&calc).await;
        assert_eq!(right.tables.len(), 1, "{name}: {:?}", right.tables);
        // Each of the three operators, a path through each case its byte can pick.
        let verdict = prove(&right, &block(CHOSEN_OPERATOR, 0), &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { paths, .. } if paths >= 3),
            "{name}: {verdict:?}"
        );
        // B6: an operator that isn't one, where of the characters only `,` passes the bounds
        // check, to the table's entry for it.
        let idea = twin(&CALC, target, "idea.md");
        let verdict = prove(&right, &block(&idea, 4), &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{name}: {verdict:?}"
        );
        // `+` and `-` swapped in the table: 6 + 3 prints 3.
        let table = "    .long .Lapply_add - operations\n    .long .Lapply_unknown - operations\n    \
                     .long .Lapply_subtract - operations\n";
        let swapped = calc.replacen(
            table,
            &table
                .replace("apply_add", "apply_TEMPORARY")
                .replace("apply_subtract", "apply_add")
                .replace("apply_TEMPORARY", "apply_subtract"),
            1,
        );
        assert_ne!(swapped, calc, "{name}: the entries were swapped");
        let wrong = machine(&swapped).await;
        let verdict = prove(&wrong, &block(CHOSEN_OPERATOR, 0), &budget(120));
        let Verdict::Counterexample(values) = &verdict else {
            panic!("{name}: {verdict:?}");
        };
        assert!(
            matches!(values.get("op"), Some(Value::Str(op)) if op == "+" || op == "-"),
            "{name}: {values:?}"
        );
    }
}

/// The idea of the letters turned into capitals through a jump table.
const LETTERS_IDEA: &str = r#"---
type: lengua/program-idea
title: letters
description: Turns letters into capitals.
---
# Purpose
The program turns `1` character of standard input into a capital: `h` to `k` become `W` to `Z`,
and `l` prints nothing.

# Behaviors
## B1: Turns a letter into a capital
- Given standard input that contains `i`
- When the program runs
- Then it prints `X`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{if(text == "l", "", tr(text, "hijk", "WXYZ"))}`
- And it exits with code `0`

| text |
|---|
| `string(1, 1, "hijkl")` |

# Constraints
- Uses: standard input
"#;

/// The letters through a jump table on x86-64: `index` turns the byte into the table's index,
/// and `limit` is the check on it.
fn letters_x86_64(index: &str, limit: u32) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    xor edi, edi
    lea rsi, [rip + buffer]
    mov edx, 1
    xor eax, eax
    syscall
    movzx eax, byte ptr [rip + buffer]
    {index}
    cmp eax, {limit}
    jae .Ldone
    lea rcx, [rip + cases]
    movsxd rax, dword ptr [rcx + rax*4]
    add rax, rcx
    jmp rax
.Lw:
    mov byte ptr [rip + buffer], 87
    jmp .Lprint
.Lx:
    mov byte ptr [rip + buffer], 88
    jmp .Lprint
.Ly:
    mov byte ptr [rip + buffer], 89
    jmp .Lprint
.Lz:
    mov byte ptr [rip + buffer], 90
.Lprint:
    mov edi, 1
    lea rsi, [rip + buffer]
    mov edx, 1
    mov eax, 1
    syscall
.Ldone:
    xor edi, edi
    mov eax, 60
    syscall
    .size _start, . - _start
    .section .rodata
    .type cases, @object
cases:
    .long .Lw - cases
    .long .Lx - cases
    .long .Ly - cases
    .long .Lz - cases
    .size cases, . - cases
    .data
    .type buffer, @object
buffer:
    .zero 1
    .size buffer, 1
"
    )
}

/// The letters through a jump table on AArch64, in the shape the dialect's checked tables have:
/// the index in a whole `x` register, `adrp` and `add :lo12:` for the table, `ldrsw` of its entry
/// and `br`.
fn letters_aarch64(index: &str, limit: u32) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #1
    mov x8, #63
    svc #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    ldrb w9, [x1]
    {index}
    cmp x9, #{limit}
    b.hs .Ldone
    adrp x10, cases
    add x10, x10, :lo12:cases
    ldrsw x11, [x10, x9, lsl #2]
    add x11, x11, x10
    br x11
.Lw:
    mov w3, #87
    b .Lprint
.Lx:
    mov w3, #88
    b .Lprint
.Ly:
    mov w3, #89
    b .Lprint
.Lz:
    mov w3, #90
.Lprint:
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    strb w3, [x1]
    mov x0, #1
    mov x2, #1
    mov x8, #64
    svc #0
.Ldone:
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.section .rodata
.type cases, @object
.balign 4
cases:
    .long .Lw - cases
    .long .Lx - cases
    .long .Ly - cases
    .long .Lz - cases
.size cases, . - cases
.data
.type buffer, @object
buffer:
    .zero 1
.size buffer, 1
"
    )
}

/// The letters through a jump table on one target: the program, and the two ways its index is
/// made from the byte.
type LettersProgram = (fn(&str, u32) -> String, [&'static str; 2]);

/// The letters through a jump table on each target: the program, and the two ways its index is
/// made from the byte (`h` to `k` become 0 to 3, and `l` becomes 4, one past the table).
const LETTERS: PerCpu<LettersProgram> = PerCpu {
    x86_64: (letters_x86_64, ["xor eax, 104", "and eax, 7"]),
    aarch64: (
        letters_aarch64,
        ["mov w3, #104\n    eor w9, w9, w3", "and w9, w9, #7"],
    ),
};

/// A jump table indexed by a byte of the input that only its `cmp` and `jae` bound, so the index
/// can reach past the table and out of the program's memory: the proof reads only the places
/// inside the table and asks the solvers whether any input reaches further. By `xor` the index
/// can be anything, which the table's log follows; by `and`, a few places past it.
#[tokio::test]
async fn proves_a_table_whose_index_only_its_check_bounds() {
    if !solvers_installed() {
        return;
    }
    let block = block(LETTERS_IDEA, 0);
    for target in Profile::ALL {
        let (program, indexes) = *LETTERS.get(target);
        for index in indexes {
            let right = machine(&program(index, 4)).await;
            assert_eq!(
                right.tables.len(),
                1,
                "{}: {:?}",
                target.name(),
                right.tables
            );
            let verdict = prove(&right, &block, &budget(120));
            assert!(
                matches!(verdict, Verdict::Proven { .. }),
                "{index} on {}: {verdict:?}",
                target.name()
            );
            // A check one too loose lets `l` read past the table.
            let wrong = machine(&program(index, 5)).await;
            let verdict = prove(&wrong, &block, &budget(120));
            let Verdict::Counterexample(values) = &verdict else {
                panic!("{index} on {}: {verdict:?}", target.name());
            };
            assert!(
                matches!(values.get(UNSAFE), Some(Value::Str(problem)) if problem.contains("outside cases")),
                "{index} on {}: {values:?}",
                target.name()
            );
        }
    }
}

/// A budget for a fixture's block, which may have many input combinations and paths.
fn fixture_budget(seconds: u64) -> Budget {
    Budget {
        max_shapes: 2_000_000,
        max_paths: 256,
        ..budget(seconds)
    }
}

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

/// The folders of the day count.
const DAYS: Folders = PerCpu {
    x86_64: "tests/fixtures/days",
    aarch64: "tests/fixtures/linux-aarch64/days",
};

/// The clock Claude Code wrote in interactive round 2, with the days from today to a date: the
/// program and the tests compute Hinnant's days_from_civil in different orders, and their sums
/// come out as one term, so every block is proven, the seven day counts' with no solver, on both
/// CPUs. What each block gets, on each target, is listed together. AArch64's second block, B2,
/// stopped ("a path couldn't be settled") before its flags were set as x86-64's (runs
/// 41914526161 and 41914520600), and is proven with no question since (run 41921354636).
#[tokio::test]
async fn proves_every_block_of_the_day_count() {
    if !solvers_installed() {
        return;
    }
    let mut problems = Vec::new();
    for target in Profile::ALL {
        let idea = twin(&DAYS, target, "idea.md");
        let machine = machine(&twin(&DAYS, target, "main.s")).await;
        for index in 0..11 {
            let block = block(&idea, index);
            let verdict = prove(&machine, &block, &fixture_budget(120));
            let said = format!("{} ({index}) on {}", block.id, target.name());
            if !matches!(verdict, Verdict::Proven { bound: None, .. }) {
                problems.push(format!("{said}: {verdict:?}"));
            } else if (1..8).contains(&index)
                && !matches!(verdict, Verdict::Proven { questions: 0, .. })
            {
                problems.push(format!("{said}: proven only with questions: {verdict:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// `idea` with one more generated test right after its `index`th, so at `index + 1`: a copy of
/// that row of its table with the generators `cells` gives by column.
fn with_row(idea: &str, index: usize, cells: &[(&str, &str)]) -> String {
    let lines: Vec<&str> = idea.lines().collect();
    let cells_of = |line: &str| -> Vec<String> {
        line.trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect()
    };
    let mut rows = 0;
    let mut header: Vec<String> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !line.starts_with('|') {
            continue;
        }
        if !lines[at - 1].starts_with('|') {
            header = cells_of(line);
            continue;
        }
        if line.starts_with("|---") {
            continue;
        }
        if rows == index {
            let copied: Vec<String> = header
                .iter()
                .zip(cells_of(line))
                .map(|(column, cell)| {
                    cells
                        .iter()
                        .find(|(name, _)| name == column)
                        .map_or(cell, |(_, generator)| format!("`{generator}`"))
                })
                .collect();
            let mut out: Vec<String> = lines[..=at].iter().map(|line| (*line).to_owned()).collect();
            out.push(format!("| {} |", copied.join(" | ")));
            out.extend(lines[at + 1..].iter().map(|line| (*line).to_owned()));
            return out.join("\n") + "\n";
        }
        rows += 1;
    }
    panic!("the idea has no generated test {index}");
}

/// The folders of the clock of interactive round 3.
const DATES: Folders = PerCpu {
    x86_64: "tests/fixtures/dates",
    aarch64: "tests/fixtures/linux-aarch64/dates",
};

/// The clock of interactive round 3, whose day count's block covers valid and invalid dates at
/// once: it prints `if(valid, days, "")`, a number or nothing, and the error the other way, so
/// one output piece is a printed number or a text as its condition says. Proven for every input
/// on both CPUs. AArch64's stopped ("a path couldn't be settled") before its flags were set as
/// x86-64's (runs 41914526161 and 41914520600); since, it is proven with 1600 shapes, 12098 paths
/// and 173 questions for the solvers (run 41921354636).
#[tokio::test]
async fn proves_a_number_printed_only_where_the_input_is_valid() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&DATES, target, "idea.md");
        let machine = machine(&twin(&DATES, target, "main.s")).await;
        let verdict = prove(&machine, &block(&idea, 1), &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// February 29 of the years 2000 to 9600 that are multiples of 400, a day the clock's own tests
/// never draw: leap only by the 400-year rule.
const CENTURY_LEAP_DAYS: &[(&str, &str)] = &[
    (
        "y",
        "choice(2000, 2400, 2800, 3200, 3600, 4000, 4400, 4800, 5200, 5600, 6000, 6400, 6800, 7200, \
         7600, 8000, 8400, 8800, 9200, 9600)",
    ),
    ("m1", "int(0, 0)"),
    ("m2", "int(2, 2)"),
    ("d1", "int(2, 2)"),
    ("d2", "int(9, 9)"),
    ("t", "int(0, 253402300799)"),
];

/// The bug that leaves the 400-year rule out of the day count, on each target: a year that is a
/// multiple of 100 goes straight to the month's days, so it is never a leap year.
const NO_400_YEAR_RULE: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov rax, rdi\n    xor edx, edx\n    mov ecx, YEARS_PER_ERA\n    div rcx\n    \
         test rdx, rdx\n    jnz .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
        "    jmp .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
    ),
    aarch64: (
        "    mov x9, #YEARS_PER_ERA\n    udiv x14, x0, x9\n    msub x15, x14, x9, x0\n    \
         cbnz x15, .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
        "    b .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
    ),
};

/// Bugs planted in the day count: an epoch a day off, and leap years without the 400-year rule.
#[tokio::test]
async fn finds_bugs_planted_in_the_day_count() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = with_row(&twin(&DAYS, target, "idea.md"), 1, CENTURY_LEAP_DAYS);
        let source = twin(&DAYS, target, "main.s");
        let leap_days = block(&idea, 2);
        let verdict = prove(&machine(&source).await, &leap_days, &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );

        let shifted = edited(
            &source,
            &[(".equ EPOCH_SHIFT, 719468\n", ".equ EPOCH_SHIFT, 719469\n")],
        );
        let verdict = prove(
            &machine(&shifted).await,
            &block(&idea, 1),
            &fixture_budget(120),
        );
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );

        let no_400 = edited(&source, &[*NO_400_YEAR_RULE.get(target)]);
        let verdict = prove(&machine(&no_400).await, &leap_days, &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The CSV total's first generated test with its text fields fixed: three signed amounts summed
/// in cents and printed with their sign, for every amount in range.
const FIXED_FIELDS: &[(&str, &str)] = &[
    ("h1", "choice(\"date\")"),
    ("h2", "choice(\"note\")"),
    ("p1", "choice(\"2026-01-05\")"),
    ("q1", "choice(\"lunch\")"),
    ("p2", "choice(\"2026-01-06\")"),
    ("q2", "choice(\"refund\")"),
    ("p3", "choice(\"2026-01-07\")"),
    ("q3", "choice(\"rent\")"),
    ("a1", "decimal(-999999.99, 999999.99)"),
    ("a2", "decimal(-999999.99, 999999.99)"),
    ("a3", "decimal(-999999.99, 999999.99)"),
];

/// The folders of the CSV total with amounts.
const AMOUNTS: Folders = PerCpu {
    x86_64: "tests/fixtures/amounts",
    aarch64: "tests/fixtures/linux-aarch64/amounts",
};

/// The bug that drops a negative amount's cents, on each target: where the parse negates the
/// amount in cents, it takes the cents off first, as whole hundreds.
const DROPPED_CENTS: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    neg rdx\n.Lparse_return:\n",
        "    mov rax, rdx\n    xor edx, edx\n    mov r8d, 100\n    div r8\n    \
         imul rdx, rax, -100\n    xor eax, eax\n.Lparse_return:\n",
    ),
    aarch64: (
        "    neg x1, x1\n.Lparse_return:\n",
        "    mov x3, #100\n    udiv x4, x1, x3\n    mov x5, #-100\n    mul x1, x4, x5\n\
         .Lparse_return:\n",
    ),
};

/// The CSV total Claude Code wrote in interactive round 2: every amount proven where the text
/// fields are fixed, with 4096 shapes and no question, and a negative amount's cents dropped
/// caught; B2 and B3 proven. What each gets, on each target, is listed together. AArch64's
/// first block stopped ("a path couldn't be settled") before its flags were set as x86-64's
/// (runs 41914526161 and 41914520600); since, it is proven the same way (4096 shapes, 15002
/// paths, no question: run 41921354636).
#[tokio::test]
async fn proves_the_csv_total_for_every_amount() {
    if !solvers_installed() {
        return;
    }
    let mut problems = Vec::new();
    for target in Profile::ALL {
        let name = target.name();
        let idea = with_row(&twin(&AMOUNTS, target, "idea.md"), 0, FIXED_FIELDS);
        let source = twin(&AMOUNTS, target, "main.s");
        let machine_of = machine(&source).await;
        for index in [2, 3] {
            let verdict = prove(&machine_of, &block(&idea, index), &fixture_budget(120));
            if !matches!(verdict, Verdict::Proven { bound: None, .. }) {
                problems.push(format!("block {index} on {name}: {verdict:?}"));
            }
        }
        let fixed = block(&idea, 1);
        let verdict = prove(&machine_of, &fixed, &fixture_budget(300));
        if !matches!(
            verdict,
            Verdict::Proven {
                shapes: 4096,
                questions: 0,
                bound: None,
                ..
            }
        ) {
            problems.push(format!("the first block on {name}: {verdict:?}"));
        }
        let dropped = edited(&source, &[*DROPPED_CENTS.get(target)]);
        let verdict = prove(&machine(&dropped).await, &fixed, &fixture_budget(120));
        if !matches!(verdict, Verdict::Counterexample(_)) {
            problems.push(format!(
                "the cents dropped, on {name}: not found, got {verdict:?}"
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The CSV total's blocks with free text in several fields: B5 proven for every length its
/// texts have, B4 past the 6 bytes it reached before sums had one form, and B1, which proved
/// nothing then, up to a bound.
#[tokio::test]
#[ignore = "takes two minutes"]
async fn proves_the_csv_totals_free_text_blocks() {
    for target in Profile::ALL {
        let idea = twin(&AMOUNTS, target, "idea.md");
        let machine = machine(&twin(&AMOUNTS, target, "main.s")).await;
        let verdict = prove_alone(&machine, &block(&idea, 4), &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "B5 on {}: {verdict:?}",
            target.name()
        );
        let verdict = prove_alone(&machine, &block(&idea, 3), &fixture_budget(60));
        assert!(
            matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 7),
            "B4 on {}: {verdict:?}",
            target.name()
        );
        let verdict = prove_alone(&machine, &block(&idea, 0), &fixture_budget(30));
        assert!(
            matches!(verdict, Verdict::Proven { bound: Some(_), .. }),
            "B1 on {}: {verdict:?}",
            target.name()
        );
    }
}

/// The idea of the day of the week.
const WEEKDAY_IDEA: &str = r#"---
type: lengua/program-idea
title: weekday
description: Prints the day of the week.
---
# Purpose
The program exits with the day of the week of the clock's seconds since 1970, from `0` (Sunday)
to `6`.

# Behaviors
## B1: Exits with the day of the week
- Given the clock at `1735689600`
- When the program runs
- Then it exits with code `3`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints nothing
- And it exits with code `{(t / 86400 + 4) % 7}`

| t |
|---|
| `int(0, 4102444800)` |

# Constraints
- Uses: the clock
"#;

/// The day of the week on x86-64, the epoch's day shifted by `shift`: two `div`s.
fn weekday_x86_64(shift: u32) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    mov eax, 228
    xor edi, edi
    lea rsi, [rip + time]
    syscall
    mov rax, qword ptr [rip + time]
    xor edx, edx
    mov ecx, 86400
    div rcx
    add rax, {shift}
    xor edx, edx
    mov ecx, 7
    div rcx
    mov edi, edx
    mov eax, 60
    syscall
    .size _start, . - _start
    .data
    .type time, @object
time:
    .zero 16
    .size time, 16
"
    )
}

/// The day of the week on AArch64, the epoch's day shifted by `shift`: two `udiv`s, the second
/// with `msub` for the remainder, and 86400 built from a `mov` and a `movk`.
fn weekday_aarch64(shift: u32) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, time
    add x1, x1, :lo12:time
    mov x8, #113
    svc #0
    adrp x1, time
    add x1, x1, :lo12:time
    ldr x2, [x1]
    mov x3, #0x5180
    movk x3, #1, lsl #16
    udiv x2, x2, x3
    add x2, x2, #{shift}
    mov x3, #7
    udiv x4, x2, x3
    msub x0, x4, x3, x2
    mov x8, #93
    svc #0
.size _start, . - _start
.data
.type time, @object
time:
    .zero 16
.size time, 16
"
    )
}

/// The day of the week on each target.
const WEEKDAY: PerCpu<fn(u32) -> String> = PerCpu {
    x86_64: weekday_x86_64,
    aarch64: weekday_aarch64,
};

/// The day of the week from the clock: a second 64-bit division of the first's quotient, which
/// the term rules make the formula's own division, so the proof needs no solver to compare them.
#[tokio::test]
async fn proves_the_day_of_the_week_with_two_divisions() {
    let block = block(WEEKDAY_IDEA, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    for target in Profile::ALL {
        let program = *WEEKDAY.get(target);
        let right = machine(&program(4)).await;
        let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
        assert!(found.is_none(), "{}: {found:?}", target.name());
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
        let wrong = machine(&program(3)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The folders of the hh:mm:ss clock.
const CLOCK: Folders = PerCpu {
    x86_64: "tests/fixtures/clock",
    aarch64: "tests/fixtures/linux-aarch64/clock",
};

/// The hh:mm:ss clock Claude Code wrote: six divisions by constants, and output padded by `if`.
/// It timed out in the solvers before the division rules. On AArch64 each division is `udiv`
/// and the remainder `msub`, which the pool reads as the remainder.
#[tokio::test]
async fn proves_the_hh_mm_ss_clock() {
    for target in Profile::ALL {
        let block = block(&twin(&CLOCK, target, "idea.md"), 0);
        let source = twin(&CLOCK, target, "main.s");
        let right = machine(&source).await;
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
        // An hour one second short.
        let short = edited(
            &source,
            &[(".equ SECONDS_PER_HOUR, 3600", ".equ SECONDS_PER_HOUR, 3599")],
        );
        let verdict = prove(&machine(&short).await, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The idea of the capitals made without branches.
const SHOUT_IDEA: &str = r#"---
type: lengua/program-idea
title: shout
description: Turns text into capitals.
---
# Purpose
The program capitalizes standard input of `0` to `16` characters.

# Behaviors
## B1: Capitalizes its input
- Given standard input that contains `ab`
- When the program runs
- Then it prints `AB`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `` string(0, 16, "aAzZ09 b{`") `` |

# Constraints
- Uses: standard input
"#;

/// The capitals on x86-64: a byte from `a` below `limit` bytes up has 32 taken off, with `setb`.
fn shout_x86_64(limit: u32) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    xor edi, edi
    lea rsi, [rip + buffer]
    mov edx, 64
    xor eax, eax
    syscall
    mov rcx, rax
    xor edx, edx
.Lloop:
    cmp rdx, rcx
    jae .Ldone
    movzx eax, byte ptr [rsi + rdx]
    lea r8d, [rax - 97]
    cmp r8d, {limit}
    setb r9b
    movzx r9d, r9b
    shl r9d, 5
    sub eax, r9d
    mov byte ptr [rsi + rdx], al
    inc rdx
    jmp .Lloop
.Ldone:
    mov edi, 1
    mov rdx, rcx
    mov eax, 1
    syscall
    xor edi, edi
    mov eax, 60
    syscall
    .size _start, . - _start
    .data
    .type buffer, @object
buffer:
    .zero 64
    .size buffer, 64
"
    )
}

/// The capitals on AArch64: a byte from `a` below `limit` bytes up has 32 taken off, with `cset`.
fn shout_aarch64(limit: u32) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #64
    mov x8, #63
    svc #0
    mov x3, x0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x4, #0
.Lloop:
    cmp x4, x3
    b.hs .Ldone
    ldrb w5, [x1, x4]
    sub w6, w5, #97
    cmp w6, #{limit}
    cset w7, lo
    lsl w7, w7, #5
    sub w5, w5, w7
    strb w5, [x1, x4]
    add x4, x4, #1
    b .Lloop
.Ldone:
    mov x0, #1
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, x3
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.data
.type buffer, @object
buffer:
    .zero 64
.size buffer, 64
"
    )
}

/// The capitals on each target.
const SHOUT: PerCpu<fn(u32) -> String> = PerCpu {
    x86_64: shout_x86_64,
    aarch64: shout_aarch64,
};

/// A text formula proven against a program: standard input capitalized byte by byte without
/// branches, against `upper(text)`, for every text up to 16 characters.
#[tokio::test]
async fn proves_a_text_formula() {
    let block = block(SHOUT_IDEA, 0);
    for target in Profile::ALL {
        let program = *SHOUT.get(target);
        let right = machine(&program(26)).await;
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { shapes: 17, .. }),
            "{}: {verdict:?}",
            target.name()
        );
        // Leaves `z` small.
        let wrong = machine(&program(25)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The idea of the count made of bit instructions.
const COUNT_A_IDEA: &str = r#"---
type: lengua/program-idea
title: count a
description: Counts the letter a.
---
# Purpose
The program counts the `a` bytes in standard input of `0` to `16` characters.

# Behaviors
## B1: Counts them
- Given standard input that contains `aba`
- When the program runs
- Then it prints `2` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "a")}` and a newline
- And it exits with code `0`

| text |
|---|
| `string(0, 16, "ab")` |

# Constraints
- Uses: standard input
"#;

/// The count on x86-64: a mask with a bit for each byte that is `byte`, built with `shlx` and
/// counted by `popcnt`.
fn count_with_bits_x86_64(byte: u32) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    xor edi, edi
    lea rsi, [rip + buffer]
    mov edx, 64
    xor eax, eax
    syscall
    mov rcx, rax
    xor edx, edx
    xor r10d, r10d
.Lloop:
    cmp rdx, rcx
    jae .Ldone
    movzx eax, byte ptr [rsi + rdx]
    cmp eax, {byte}
    sete al
    movzx eax, al
    shlx rax, rax, rdx
    or r10, rax
    inc rdx
    jmp .Lloop
.Ldone:
    popcnt r8, r10
    lea rdi, [rip + out]
    xor r9d, r9d
    cmp r8, 10
    jb .Lone
    mov byte ptr [rdi], 49
    sub r8, 10
    mov r9d, 1
.Lone:
    add r8d, 48
    mov byte ptr [rdi + r9], r8b
    inc r9
    mov byte ptr [rdi + r9], 10
    inc r9
    mov edi, 1
    lea rsi, [rip + out]
    mov rdx, r9
    mov eax, 1
    syscall
    xor edi, edi
    mov eax, 60
    syscall
    .size _start, . - _start
    .data
    .type buffer, @object
buffer:
    .zero 64
    .size buffer, 64
    .type out, @object
out:
    .zero 8
    .size out, 8
"
    )
}

/// The count on AArch64: the same mask, built with a shift by a register (`lslv`), counted by
/// the CPU's only population count, `cnt` on bytes of a vector and `addv` across them, since
/// ARMv8.0 has no scalar `popcnt`.
fn count_with_bits_aarch64(byte: u32) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #64
    mov x8, #63
    svc #0
    mov x3, x0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x4, #0
    mov x10, #0
.Lloop:
    cmp x4, x3
    b.hs .Ldone
    ldrb w5, [x1, x4]
    cmp w5, #{byte}
    cset x6, eq
    lsl x6, x6, x4
    orr x10, x10, x6
    add x4, x4, #1
    b .Lloop
.Ldone:
    fmov d0, x10
    cnt v0.8b, v0.8b
    addv b0, v0.8b
    umov w8, v0.b[0]
    adrp x0, out
    add x0, x0, :lo12:out
    mov x9, #0
    cmp w8, #10
    b.lo .Lone
    mov w11, #49
    strb w11, [x0]
    sub w8, w8, #10
    mov x9, #1
.Lone:
    add w8, w8, #48
    strb w8, [x0, x9]
    add x9, x9, #1
    mov w11, #10
    strb w11, [x0, x9]
    add x9, x9, #1
    mov x2, x9
    mov x0, #1
    adrp x1, out
    add x1, x1, :lo12:out
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.data
.type buffer, @object
buffer:
    .zero 64
.size buffer, 64
.type out, @object
out:
    .zero 8
.size out, 8
"
    )
}

/// The count made of bit instructions on each target.
const COUNT_WITH_BITS: PerCpu<fn(u32) -> String> = PerCpu {
    x86_64: count_with_bits_x86_64,
    aarch64: count_with_bits_aarch64,
};

/// Bit instructions against a text formula: a mask of the bytes that are `a`, built without
/// branches by a shift by a register (`shlx`, `lslv`), counted by the CPU's population count
/// (`popcnt`; on AArch64 `cnt` and `addv`), against `count(text, "a")`.
#[tokio::test]
async fn proves_a_count_made_of_bit_instructions() {
    let block = block(COUNT_A_IDEA, 0);
    for target in Profile::ALL {
        let program = *COUNT_WITH_BITS.get(target);
        let right = machine(&program(97)).await;
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { shapes: 17, .. }),
            "{}: {verdict:?}",
            target.name()
        );
        // Counts the `b` bytes instead.
        let wrong = machine(&program(98)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// An echo of standard input on x86-64, with `extra` run first once the input is at least 10
/// bytes.
fn echo_x86_64(extra: &str) -> String {
    format!(
        "    .intel_syntax noprefix
    .globl _start
    .text
    .type _start, @function
_start:
    xor edi, edi
    lea rsi, [rip + buffer]
    mov edx, 8192
    xor eax, eax
    syscall
    mov r12, rax
    cmp rax, 10
    jb .Lprint
{extra}
.Lprint:
    mov edi, 1
    lea rsi, [rip + buffer]
    mov rdx, r12
    mov eax, 1
    syscall
    xor edi, edi
    mov eax, 60
    syscall
    .size _start, . - _start
    .bss
    .type buffer, @object
buffer:
    .zero 8192
    .size buffer, 8192
"
    )
}

/// An echo of standard input on AArch64, with `extra` run first once the input is at least 10
/// bytes.
fn echo_aarch64(extra: &str) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #8192
    mov x8, #63
    svc #0
    mov x12, x0
    cmp x0, #10
    b.lo .Lprint
{extra}
.Lprint:
    mov x0, #1
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, x12
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.bss
.type buffer, @object
buffer:
    .zero 8192
.size buffer, 8192
"
    )
}

/// The echo of one target, and the line of it that gives the read its size.
type EchoProgram = (fn(&str) -> String, &'static str);

/// The echo of each target, and the line of it that gives the read its size.
const ECHO_OF_STDIN: PerCpu<EchoProgram> = PerCpu {
    x86_64: (echo_x86_64, "    mov edx, 8192\n"),
    aarch64: (echo_aarch64, "    mov x2, #8192\n"),
};

fn echo_block(stdout: &str, example: &str) -> Generated {
    let idea = format!(
        r#"---
type: lengua/program-idea
title: echo
description: Prints standard input back.
---
# Purpose
The program prints standard input of `0` to `5000` characters back.

# Behaviors
## B1: Prints it back
- Given standard input that contains `{example}`
- When the program runs
- Then it prints `{example}`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{stdout}`
- When the program runs
- Then it prints `{stdout}`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "ab")` |

# Constraints
- Uses: standard input
"#
    );
    block(&idea, 0)
}

fn long_budget() -> Budget {
    Budget {
        max_shapes: 100_000,
        ..budget(240)
    }
}

/// Text longer than one pipe write, read with `read`: every length a pipe write holds is
/// proven, and the verdict says where it stops and why. Where the proof starts later, it goes
/// on from there.
#[tokio::test]
async fn proves_long_text_up_to_the_bound() {
    for target in Profile::ALL {
        let (echo, _) = *ECHO_OF_STDIN.get(target);
        let machine = machine(&echo("")).await;
        let name = target.name();
        let verdict = prove(&machine, &echo_block("{text}", "ab"), &long_budget());
        let Verdict::Proven { shapes, bound, .. } = verdict else {
            panic!("{name}: {verdict:?}");
        };
        assert_eq!(shapes, PIPE_WRITE + 1, "{name}");
        let bound = bound.expect("a bound");
        assert_eq!((bound.bytes, bound.of), (PIPE_WRITE, 5000), "{name}");
        assert!(
            bound.why.contains("one pipe write"),
            "{name}: {}",
            bound.why
        );
        // With a newline after the text, the longest text no longer fits one pipe write.
        let verdict = prove(&machine, &echo_block("{text}\\n", "ab\\n"), &long_budget());
        let Verdict::Proven { bound, .. } = verdict else {
            panic!("{name}: {verdict:?}");
        };
        let bound = bound.expect("a bound");
        assert_eq!(bound.bytes, PIPE_WRITE - 1, "{name}");
        assert!(
            bound.why.contains("one pipe write"),
            "{name}: {}",
            bound.why
        );
        // A proof that starts where an earlier one stopped proves only from there.
        let resumed = Budget {
            from_length: 4000,
            ..long_budget()
        };
        let verdict = prove(&machine, &echo_block("{text}", "ab"), &resumed);
        let Verdict::Proven { shapes, bound, .. } = verdict else {
            panic!("{name}: {verdict:?}");
        };
        assert_eq!(shapes, PIPE_WRITE + 1 - 4000, "{name}");
        assert_eq!(bound.map(|bound| bound.bytes), Some(PIPE_WRITE), "{name}");
    }
}

/// What each target's echo runs once its input is long enough, which its model doesn't follow,
/// and what the proof says it stopped at: `cpuid` on x86-64, and on AArch64 a `svc` with a
/// number other than 0, which a system call never has.
const UNMODELLED: PerCpu<(&str, &str)> = PerCpu {
    x86_64: ("    xor eax, eax\n    cpuid", "cpuid"),
    aarch64: ("    svc #1", "svc other than"),
};

/// Where the model stops following the program at some length, the proof holds below it.
#[tokio::test]
async fn proves_text_up_to_where_the_model_stops() {
    for target in Profile::ALL {
        let (echo, _) = *ECHO_OF_STDIN.get(target);
        let (extra, stops_at) = *UNMODELLED.get(target);
        let machine = machine(&echo(extra)).await;
        let name = target.name();
        let verdict = prove(&machine, &echo_block("{text}", "ab"), &long_budget());
        let Verdict::Proven { shapes, bound, .. } = verdict else {
            panic!("{name}: {verdict:?}");
        };
        assert_eq!(shapes, 10, "{name}");
        let bound = bound.expect("a bound");
        assert_eq!(bound.bytes, 9, "{name}");
        assert!(bound.why.contains(stops_at), "{name}: {}", bound.why);
    }
}

/// Prints each generated block's verdict for a program's two files, for exploring.
#[tokio::test]
#[ignore = "explores a program's proofs"]
async fn explores_a_program() {
    let folder = std::env::var("LENGUA_EXPLORE").expect("LENGUA_EXPLORE names a folder");
    let read = |name: &str| {
        std::fs::read_to_string(std::path::Path::new(&folder).join(name)).expect("a file")
    };
    let machine = machine(&read("main.s")).await;
    let idea_parsed = crate::idea::parse(&read("idea.md")).expect("valid idea");
    let suite = crate::testfile::check(&idea_parsed).expect("valid tests");
    for block in &suite.generated {
        let began = Instant::now();
        let verdict = prove(
            &machine,
            block,
            &Budget::until(
                Instant::now() + Duration::from_secs(120),
                idea_parsed.network.listens.is_some(),
            ),
        );
        eprintln!("{}: {:?} in {:?}", block.id, verdict, began.elapsed());
    }
}

/// A file of a program in this repository, by its path from the top.
fn repository_file(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn text_budget() -> Budget {
    Budget {
        max_shapes: 100_000,
        ..budget(120)
    }
}

/// A target's vector template in the skill's assets, and the edit that stops its capitals at `y`.
type VectorTemplate = (&'static str, Edit);

/// The vector template of each target, and the edit that stops its capitals at `y`: the bound
/// above `z`, four bytes of it at a time on x86-64's SSE2, a byte in AArch64's NEON.
const VECTOR_TEMPLATES: PerCpu<VectorTemplate> = PerCpu {
    x86_64: (
        "vector-template.s",
        (".equ ABOVE_Z_X4, 0x7b7b7b7b", ".equ ABOVE_Z_X4, 0x7a7a7a7a"),
    ),
    aarch64: (
        "linux-aarch64/vector-template.s",
        (".equ ABOVE_Z, 0x7b", ".equ ABOVE_Z, 0x7a"),
    ),
};

/// Vector code, SSE2 on x86-64 and NEON on AArch64: the vector template capitalizes every line
/// it's given, proven with no solver question; its too-long lines are proven up to one pipe
/// write; and capitals that stop at `y` are found.
#[tokio::test]
async fn proves_the_vector_template() {
    let idea = asset("vector-template-idea.md");
    for target in Profile::ALL {
        let (template, stops_at_y) = *VECTOR_TEMPLATES.get(target);
        let source = asset(template);
        let right = machine(&source).await;
        let shout = block(&idea, 0);
        let started = Instant::now();
        let verdict = prove(&right, &shout, &budget(60));
        eprintln!(
            "vector template B1 on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    questions: 0,
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let too_long = block(&idea, 1);
        let verdict = prove(&right, &too_long, &text_budget());
        let Verdict::Proven {
            bound: Some(bound), ..
        } = &verdict
        else {
            panic!("{}: {verdict:?}", target.name());
        };
        assert_eq!(bound.bytes, PIPE_WRITE - 1, "{}", target.name());
        let wrong = machine(&edited(&source, &[stops_at_y])).await;
        let verdict = prove(&wrong, &shout, &budget(60));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The streaming template of each target, in the skill's assets.
const STREAM_TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: "stream-template.s",
    aarch64: "linux-aarch64/stream-template.s",
};

/// The streaming template that capitalizes only the first buffer-full, on each target: a register
/// that says whether a buffer-full was capitalized before.
const CAPITALIZES_THE_FIRST_FULL_ONLY: PerCpu<[Edit; 2]> = PerCpu {
    x86_64: [
        (
            "    mov rbx, MAX_FULLS\n",
            "    xor r13d, r13d\n    mov rbx, MAX_FULLS\n",
        ),
        (
            "    call shout\n",
            "    test r13, r13\n    jnz .Lstart_shouted\n    call shout\n.Lstart_shouted:\n    \
             mov r13d, 1\n",
        ),
    ],
    aarch64: [
        (
            "    mov x19, #MAX_FULLS\n",
            "    mov x21, #0\n    mov x19, #MAX_FULLS\n",
        ),
        (
            "    bl shout\n",
            "    cbnz x21, .Lstart_shouted\n    bl shout\n.Lstart_shouted:\n    mov x21, #1\n",
        ),
    ],
};

/// Induction: the streaming template, which capitalizes its input a buffer-full at a time
/// through fill, is proven for every length its loop's bound allows, far past what any proof
/// by length reaches; capitals that stop at `y` are still found, and so is a loop that drops
/// the bytes of every buffer-full after the first.
#[tokio::test]
async fn proves_a_stream_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let idea = asset("stream-template-idea.md");
    for target in Profile::ALL {
        let name = target.name();
        let source = asset(STREAM_TEMPLATES.get(target));
        let right = machine(&source).await;
        let shout = block(&idea, 0);
        let verdict = prove(&right, &shout, &text_budget());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(4096),
                    ..
                }
            ),
            "{name}: {verdict:?}"
        );
        let stops_at_y = VECTOR_TEMPLATES.get(target).1;
        let wrong = machine(&edited(&source, &[stops_at_y])).await;
        let verdict = prove(&wrong, &shout, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
        // Only the first buffer-full capitalized: from a later time round's state, the bytes
        // go out as they came in.
        let first_only = machine(&edited(
            &source,
            CAPITALIZES_THE_FIRST_FULL_ONLY.get(target),
        ))
        .await;
        let verdict = prove(&first_only, &shout, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
        // A loop bound of four buffer-fulls: proven for three, as far as the induction
        // reaches, and no further.
        let four = edited(
            &source,
            &[(".equ MAX_FULLS, 1099511627776", ".equ MAX_FULLS, 4")],
        );
        let verdict = prove(&machine(&four).await, &shout, &text_budget());
        assert!(
            matches!(
                &verdict,
                Verdict::Proven {
                    bound: Some(Bound {
                        bytes: 12288,
                        more: true,
                        ..
                    }),
                    induction: Some(4096),
                    ..
                }
            ),
            "{name}: {verdict:?}"
        );
    }
}

/// The folders of the line counter.
const LINES: Folders = PerCpu {
    x86_64: "tests/fixtures/lines",
    aarch64: "tests/fixtures/linux-aarch64/lines",
};

/// The line counter that stops a chunk early, on each target: its loop leaves when the next
/// chunk would pass the end, so the last partial chunk of each buffer-full is never counted.
const MISSES_THE_LAST_CHUNK: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    cmp rcx, rdi\n    jae .Lcount_lines_return\n",
        "    lea rdx, [rcx + 16]\n    cmp rdx, rdi\n    jae .Lcount_lines_return\n",
    ),
    aarch64: (
        "    cmp x10, x1\n    b.hs .Lcount_lines_return\n",
        "    add x4, x10, #CHUNK\n    cmp x4, x1\n    b.hs .Lcount_lines_return\n",
    ),
};

/// The line counter that keeps only the last buffer-full's count, on each target: the register
/// that adds up the counts is set to each in turn.
const COUNTS_THE_LAST_FULL_ALONE: PerCpu<Edit> = PerCpu {
    x86_64: ("    add r13, rax\n", "    mov r13, rax\n"),
    aarch64: ("    add x21, x21, x0\n", "    mov x21, x0\n"),
};

/// Induction over counts: a line count kept in a register, grown by each buffer-full's
/// newlines, 16 bytes at a time, is proven for every length the idea allows; a count that
/// misses the last chunk of each buffer-full is found.
#[tokio::test]
async fn proves_a_count_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&LINES, target, "idea.md");
        let source = twin(&LINES, target, "main.s");
        let right = machine(&source).await;
        let lines = block(&idea, 0);
        // Four minutes, so a machine busy with the other tests still finishes the base.
        let verdict = prove(&right, &lines, &long_budget());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(4096),
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let short = edited(&source, &[*MISSES_THE_LAST_CHUNK.get(target)]);
        let verdict = prove(&machine(&short).await, &lines, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
        // A count of the last buffer-full alone: the first time round it looks like the count
        // so far, and from any later state it isn't, so no induction applies.
        let last_only = edited(&source, &[*COUNTS_THE_LAST_FULL_ALONE.get(target)]);
        let last_only = machine(&last_only).await;
        let image = Image::new(&last_only).expect("an entry");
        let step = induction::step(&image, &lines, &text_budget());
        assert!(step.is_err(), "{}: {step:?}", target.name());
    }
}

/// The folders of the word counter.
const WORDS: Folders = PerCpu {
    x86_64: "tests/fixtures/words",
    aarch64: "tests/fixtures/linux-aarch64/words",
};

/// The two bugs in what crosses from one buffer-full to the next, on each target: each
/// buffer-full taken as if whitespace came before it, and the word-ending flag lost after each
/// one. x86-64 keeps the flag in r14, AArch64 in x23 (0xff for whitespace).
const FORGETS_THE_WORD_ACROSS_FULLS: PerCpu<[Edit; 2]> = PerCpu {
    x86_64: [
        ("    call fill\n", "    mov r14d, 1\n    call fill\n"),
        ("    mov r14, rcx\n", "    xor r14d, r14d\n"),
    ],
    aarch64: [
        (
            "    mov w3, #READ_TO_END\n    bl fill\n",
            "    mov w3, #READ_TO_END\n    mov x23, #0xff\n    bl fill\n",
        ),
        ("    mov x23, x2\n", "    mov x23, #0\n"),
    ],
};

/// The word counter that keeps only the last buffer-full's words, on each target: no register
/// keeps the words as they go.
const COUNTS_THE_LAST_FULLS_WORDS_ALONE: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    add r13, rdx\n    mov r14, rcx\n",
        "    mov r13, rdx\n    mov r14, rcx\n",
    ),
    aarch64: (
        "    add x22, x22, x1\n    mov x23, x2\n",
        "    mov x22, x1\n    mov x23, x2\n",
    ),
};

/// Words, whose count for a buffer-full depends on whether the one before ended inside a word:
/// the step reads two buffer-fulls and checks the second's counts after the first's last byte,
/// whatever the program keeps of it, so the fixture's wc -lw is proven for every length; a
/// program that forgets at each read that the last one ended inside a word, which only an input
/// longer than a buffer-full shows, gets the two buffer-fulls that show it.
#[tokio::test]
async fn proves_words_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&WORDS, target, "idea.md");
        // A smaller buffer than the fixture's, so the base's lengths go by quickly.
        let source = edited(
            &twin(&WORDS, target, "main.s"),
            &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n")],
        );
        let right = machine(&source).await;
        let words = block(&idea, 0);
        let verdict = prove(&right, &words, &long_budget());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(256),
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        // Each buffer-full taken as if whitespace came before it, and the word-ending flag
        // lost after each one: only an input longer than a buffer-full shows either, and the
        // step finds the two buffer-fulls that do.
        for edit in FORGETS_THE_WORD_ACROSS_FULLS.get(target) {
            let why = edit.0;
            let wrong = machine(&edited(&source, &[*edit])).await;
            let verdict = prove(&wrong, &words, &text_budget());
            let Verdict::Counterexample(values) = &verdict else {
                panic!("{} on {why}: {verdict:?}", target.name());
            };
            let Some(Value::Str(text)) = values.get("text") else {
                panic!("{} on {why}: {values:?}", target.name());
            };
            assert!(text.len() > 256, "{} on {why}: {text:?}", target.name());
        }
        // A count of the last buffer-full's words alone: no register keeps the words as they
        // go.
        let last_only = edited(&source, &[*COUNTS_THE_LAST_FULLS_WORDS_ALONE.get(target)]);
        let last_only = machine(&last_only).await;
        let image = Image::new(&last_only).expect("an entry");
        let step = induction::step(&image, &words, &text_budget());
        assert!(step.is_err(), "{}: {step:?}", target.name());
    }
}

/// The folders of the sort example.
const SORT: Folders = PerCpu {
    x86_64: "examples/sort",
    aarch64: "tests/fixtures/linux-aarch64/sort",
};

/// The comparison that puts the longer of two lines with the same start first, on each target:
/// the difference of the two lengths is taken the other way round.
const LONGER_LINE_FIRST: PerCpu<Edit> = PerCpu {
    x86_64: (
        ".Lcompare_lengths:\n    mov rax, r8\n    sub rax, r9\n",
        ".Lcompare_lengths:\n    mov rax, r9\n    sub rax, r8\n",
    ),
    aarch64: (
        ".Lcompare_lengths:\n    sub x0, x9, x10\n",
        ".Lcompare_lengths:\n    sub x0, x10, x9\n",
    ),
};

/// Sorting: sort and uniq over short texts are a choice of texts, one for each way the newlines
/// can fall and the lines compare, so the sort example is proven for every input up to the
/// length where its comparisons branch past the proof's paths, with and without `-u`; a
/// comparison that puts the longer of two lines with the same start first is found.
#[tokio::test]
async fn proves_sorting_for_short_texts() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&SORT, target, "idea.md");
        let source = twin(&SORT, target, "main.s");
        let right = machine(&source).await;
        // Two minutes each, so a machine busy with other tests' proofs still reaches the
        // lengths.
        let budget = || Budget {
            max_shapes: 100_000,
            max_paths: 256,
            ..budget(120)
        };
        // A mebibyte sample is too long for the model to follow, which says nothing of it: the
        // check before the proof goes on with the shorter ones.
        let samples: Vec<crate::testfile::Case> = block(&idea, 0)
            .cases(1)
            .take(24)
            .filter_map(Result::ok)
            .collect();
        assert!(samples.iter().any(|sample| sample.stdin.len() > 1 << 19));
        let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
        assert!(found.is_none(), "{}: {found:?}", target.name());
        for index in [0, 2] {
            let verdict = prove(&right, &block(&idea, index), &budget());
            assert!(
                matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 5),
                "{} on block {index}: {verdict:?}",
                target.name()
            );
        }
        let longer_first = edited(&source, &[*LONGER_LINE_FIRST.get(target)]);
        let verdict = prove(&machine(&longer_first).await, &block(&idea, 0), &budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The word tally template of each target, in the skill's assets, and its three bugs: every word
/// new, since its slot never takes its entry's number; ties in reverse byte order; and the hash
/// used whole as a slot, which the probe's bounds check then stops with its assertion.
type TallyTemplate = (&'static str, [Edit; 3]);

/// The tally template of each target, and its bugs.
const TALLY_TEMPLATES: PerCpu<TallyTemplate> = PerCpu {
    x86_64: (
        "tally-template.s",
        [
            (
                "    mov dword ptr [r15 + rbp*4], eax\n",
                "    mov dword ptr [r15 + rbp*4], 0\n",
            ),
            (
                "    lea rdi, [rdx + r8]\n    lea rdx, [rdx + r9]\n",
                "    lea rdi, [rdx + r9]\n    lea rdx, [rdx + r8]\n",
            ),
            (
                "    and rax, SLOT_MASK\n    mov rdi, r12\n",
                "    mov rdi, r12\n",
            ),
        ],
    ),
    aarch64: (
        "linux-aarch64/tally-template.s",
        [
            (
                "    str w10, [x22, x23, lsl #2]\n",
                "    str wzr, [x22, x23, lsl #2]\n",
            ),
            (
                "    add x0, x2, x12\n    add x2, x2, x13\n",
                "    add x0, x2, x13\n    add x2, x2, x12\n",
            ),
            ("    and x4, x0, #SLOT_MASK\n", "    mov x4, x0\n"),
        ],
    ),
};

/// A word tally through a hash table of a million slots, which the words' hashes choose among:
/// the table's log of stores follows them, so the building block is proven for short texts; a
/// count that forgets to note a word in the table, ties in the wrong order and a slot left
/// unmasked, which its own bounds check then stops, are found. Its questions about where two
/// words' hashes meet keep the solvers busy for minutes, so it runs with the slow tests
/// (`--ignored`).
#[tokio::test]
#[ignore = "minutes of solver work on hash collisions"]
async fn proves_a_tally_for_short_texts() {
    if !solvers_installed() {
        return;
    }
    let idea = asset("tally-template-idea.md");
    for target in Profile::ALL {
        let name = target.name();
        let (template, mutants) = *TALLY_TEMPLATES.get(target);
        let source = asset(template);
        let right = machine(&source).await;
        let tally = block(&idea, 0);
        let verdict = prove_alone(&right, &tally, &budget(90));
        assert!(
            matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 3),
            "{name}: {verdict:?}"
        );
        for mutant in mutants {
            let wrong = edited(&source, &[mutant]);
            let verdict = prove(&machine(&wrong).await, &tally, &budget(60));
            assert!(
                matches!(verdict, Verdict::Counterexample(_)),
                "{name}: {}: {verdict:?}",
                mutant.0
            );
        }
    }
}

/// The parallel template of each target, in the skill's assets.
const PARALLEL_TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: "parallel-template.s",
    aarch64: "linux-aarch64/parallel-template.s",
};

/// The terminal template of each target, in the skill's assets.
const TERMINAL_TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: "terminal-template.s",
    aarch64: "linux-aarch64/terminal-template.s",
};

/// Threads: each of parallel_for's workers runs to its end as it starts, which M7 makes the same
/// as any order, so a program on several threads proves like any other. Each thread here takes
/// 128 bytes, so a text of 300 runs on three.
#[tokio::test]
async fn proves_a_program_on_several_threads() {
    let idea = asset("parallel-template-idea.md")
        .replacen(
            "# Behaviors",
            "Tests here give it up to `300` bytes.\n\n# Behaviors",
            1,
        )
        .replace("string(0, 1048576,", "string(0, 300,");
    for target in Profile::ALL {
        // Sixteen chunks of 128 bytes hold at most 2048.
        let source = asset(PARALLEL_TEMPLATES.get(target))
            .replace(".equ CHUNK, 65536", ".equ CHUNK, 128")
            .replace(".equ INPUT_MAX, 1048576", ".equ INPUT_MAX, 2048");
        assert!(source.contains(".equ CHUNK, 128") && source.contains(".equ INPUT_MAX, 2048"));
        let machine = machine(&source).await;
        let verdict = prove(&machine, &block(&idea, 0), &text_budget());
        assert!(
            matches!(
                &verdict,
                Verdict::Proven {
                    bound: None,
                    unreached,
                    ..
                } if unreached.is_empty()
            ),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// A program for the terminal proves as from a pipe, as its tests run: the terminal's mode
/// can't be read or set, and a wait for the next key always finds one, or the input's end: on
/// AArch64 a `ppoll` with a tenth of a second the kernel reads.
#[tokio::test]
async fn proves_a_program_for_the_terminal() {
    let idea = asset("terminal-template-idea.md");
    for target in Profile::ALL {
        let machine = machine(&asset(TERMINAL_TEMPLATES.get(target))).await;
        for index in 0..2 {
            let verdict = prove(&machine, &block(&idea, index), &text_budget());
            assert!(
                matches!(
                    verdict,
                    Verdict::Proven {
                        bound: None,
                        questions: 0,
                        ..
                    }
                ),
                "block {index} on {}: {verdict:?}",
                target.name()
            );
        }
    }
}

/// The CRC-32C fast tier of each target, in the skill's assets: x86-64's `crc32` over 8 bytes at
/// a time and three chains joined by `pclmulqdq`, AArch64's `crc32cx` chain beside carry-less
/// multiplies that fold the other half of each 32 KiB block. Both meet the very same idea,
/// `fast/crc32c/idea.md`.
const CRC32C_TIERS: PerCpu<&str> = PerCpu {
    x86_64: "fast/crc32c-pclmul/main.s",
    aarch64: "linux-aarch64/fast/crc32c-crc/main.s",
};

/// The functions of the CRC-32C tier that a proof of short texts never runs, on each target, as
/// the proof says (run 41919475930 for both): x86-64's three chains at once start at 12288 bytes,
/// past every text proven, and its copy before them; AArch64's block, which folds 32768 bytes
/// at a time, and the join of its two parts.
const CRC32C_UNREACHED: PerCpu<&[&str]> = PerCpu {
    x86_64: &["move_front", "crc_block"],
    aarch64: &["crc_block", "crc_join"],
};

/// The CRC-32C fast tier meets the formula's chain of byte steps (`crc32` over 8 bytes at a
/// time, then over single bytes; on AArch64 `crc32cx` then `crc32cb`), so each text proves
/// without a question to the solvers, and one bit of the start wrong puts every checksum off.
#[tokio::test]
async fn proves_the_crc32c_checksum() {
    let idea = asset("fast/crc32c/idea.md")
        .replace("tests give it up to `1048576`", "tests give it up to `100`")
        .replace("string(0, 1048576,", "string(0, 100,");
    assert!(idea.contains("string(0, 100,"), "{idea}");
    let block = block(&idea, 0);
    for target in Profile::ALL {
        let name = target.name();
        let source = asset(CRC32C_TIERS.get(target));
        let right = machine(&source).await;
        let verdict = prove(&right, &block, &text_budget());
        eprintln!("crc32c on {name}: {verdict:?}");
        assert!(
            matches!(
                &verdict,
                Verdict::Proven {
                    bound: None,
                    questions: 0,
                    unreached,
                    ..
                } if unreached == CRC32C_UNREACHED.get(target)
            ),
            "{name}: {verdict:?}"
        );
        // One bit of the start wrong puts every checksum off.
        let wrong = edited(
            &source,
            &[(".equ CRC_START, 0xFFFFFFFF", ".equ CRC_START, 0xFFFFFFFE")],
        );
        let verdict = prove(&machine(&wrong).await, &block, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
    }
}

/// The CRC-32C fast tier's own idea, every text up to the 4096 bytes a proof covers.
#[tokio::test]
#[ignore = "slow: 4097 lengths of text"]
async fn proves_the_crc32c_checksum_up_to_the_bound() {
    let block = block(&asset("fast/crc32c/idea.md"), 0);
    for target in Profile::ALL {
        let machine = machine(&asset(CRC32C_TIERS.get(target))).await;
        let started = Instant::now();
        let budget = Budget {
            max_shapes: 100_000,
            ..budget(300)
        };
        let verdict = prove(&machine, &block, &budget);
        eprintln!(
            "crc32c on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// A form of the fast word count: what it is, and its main.s. All the forms of all the targets
/// have one idea, `fast/wc/idea.md`, the same bytes in the tier folders beside it.
type WordCountForm = (&'static str, &'static str);

/// The fast word count's forms on each target: the baseline form every machine of the target
/// runs (SSE2 with `popcnt` on x86-64, NEON on AArch64), and the optional extension's tier beside
/// it (AVX-512, SVE).
const WORD_COUNT_FORMS: PerCpu<[WordCountForm; 2]> = PerCpu {
    x86_64: [
        ("SSE2", "plugin/skills/compile/assets/fast/wc/main.s"),
        (
            "AVX-512",
            "plugin/skills/compile/assets/fast/wc-avx512/main.s",
        ),
    ],
    aarch64: [
        (
            "NEON",
            "plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s",
        ),
        (
            "SVE",
            "plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s",
        ),
    ],
};

/// The fast word count's idea, the one every form of it meets.
const WORD_COUNT_IDEA: &str = "plugin/skills/compile/assets/fast/wc/idea.md";

/// The fast word count, each form on each target, the baseline and the optional extension's tier
/// beside it: texts up to 200 bytes, and a carriage return that's one byte too short is found.
/// The proof doesn't need the extension's hardware: it runs the model, which maps every
/// extension of x86-64; SVE's vectors have the length of the CPU the proof runs for, this CPU's
/// or QEMU's, so a computer with neither skips them.
#[tokio::test]
async fn proves_the_word_count_of_short_texts() {
    if !solvers_installed() {
        return;
    }
    let idea = repository_file(WORD_COUNT_IDEA)
        .replace(
            "The tests give it up to `1048576` bytes.",
            "The tests give it up to `200` bytes.",
        )
        .replace("string(0, 1048576,", "string(0, 200,");
    assert!(idea.contains("string(0, 200,"), "{idea}");
    let block = block(&idea, 0);
    for target in Profile::ALL {
        for (form, path) in WORD_COUNT_FORMS.get(target) {
            let source = repository_file(path);
            let right = machine(&source).await;
            if *form == "SVE" && right.sve_bytes == 0 {
                crate::skiplog::note("neither this CPU nor QEMU's here has SVE");
                continue;
            }
            let verdict = prove(&right, &block, &text_budget());
            assert!(
                matches!(
                    verdict,
                    Verdict::Proven {
                        bound: None,
                        questions: 0,
                        ..
                    }
                ),
                "{form} on {}: {verdict:?}",
                target.name()
            );
            let wrong = edited(&source, &[(".equ TAB_TO_CR, 4", ".equ TAB_TO_CR, 3")]);
            let verdict = prove(&machine(&wrong).await, &block, &text_budget());
            assert!(
                matches!(verdict, Verdict::Counterexample(_)),
                "{form} on {}: {verdict:?}",
                target.name()
            );
        }
    }
}

/// The folders of the upcase example.
const UPCASE: Folders = PerCpu {
    x86_64: "examples/upcase",
    aarch64: "tests/fixtures/linux-aarch64/upcase",
};

/// The bound that lets `z` stay small, on each target: x86-64 compares against a signed
/// constant, AArch64 against the number of letters.
const Z_STAYS_SMALL: PerCpu<Edit> = PerCpu {
    x86_64: ("mov eax, 0x9a9a9a9a", "mov eax, 0x99999999"),
    aarch64: ("    cmp w5, #LETTERS\n", "    cmp w5, #LETTERS - 1\n"),
};

/// Vector code on files: upcase, which Claude Code wrote through lengua with SSE2's signed
/// compare trick on x86-64 and, on AArch64, each letter's capital with no branch on the byte,
/// capitalizes any file of text, its files cut to 300 bytes so every length goes by: every block
/// is proven, and a bound that leaves `z` small is found.
#[tokio::test]
async fn proves_upcase_on_files() {
    for target in Profile::ALL {
        let idea = twin(&UPCASE, target, "idea.md")
            .replacen(
                "# Behaviors",
                "Tests also give it files of `0` to `300` bytes.\n\n# Behaviors",
                1,
            )
            .replacen("string(0, 1048576,", "string(0, 300,", 1);
        let source = twin(&UPCASE, target, "main.s");
        let right = machine(&source).await;
        for number in 0..4 {
            let block = block(&idea, number);
            let started = Instant::now();
            let verdict = prove(&right, &block, &text_budget());
            eprintln!(
                "upcase block {number} on {}: {verdict:?} in {:?}",
                target.name(),
                started.elapsed()
            );
            // The first block's proof needs no solver on either CPU.
            let questions_wanted: Option<usize> = (number == 0).then_some(0);
            assert!(
                matches!(
                    &verdict,
                    Verdict::Proven { bound: None, questions, .. }
                        if questions_wanted.is_none_or(|wanted| *questions == wanted)
                ),
                "block {number} on {}: {verdict:?}",
                target.name()
            );
        }
        let wrong = machine(&edited(&source, &[*Z_STAYS_SMALL.get(target)])).await;
        let verdict = prove(&wrong, &block(&idea, 0), &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
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

fn network_budget(listens: bool) -> Budget {
    Budget {
        listens,
        ..text_budget()
    }
}

/// The event-loop server template of each target, in the skill's assets, and its two bugs:
/// serving the first slot alone, and a connection never closed.
type ServerTemplate = (&'static str, [Edit; 2]);

/// The server template of each target, and its bugs.
const SERVER_TEMPLATES: PerCpu<ServerTemplate> = PerCpu {
    x86_64: (
        "server-template.s",
        [
            (
                "    mov rbx, CLIENTS\n.Lserve_clients_slot:",
                "    mov rbx, 1\n.Lserve_clients_slot:",
            ),
            (
                "    mov edi, ebx\n    call os_close\n",
                "    xor eax, eax\n",
            ),
        ],
    ),
    aarch64: (
        "linux-aarch64/server-template.s",
        [
            (
                "    mov x20, #CLIENTS\n.Lserve_clients_slot:",
                "    mov x20, #1\n.Lserve_clients_slot:",
            ),
            ("    mov w0, w11\n    bl os_close\n", "    mov x0, #0\n"),
        ],
    ),
};

/// A server for clients at once: the event-loop template waits on its listener and every
/// connection with os_poll. One client is proven for every line in range, in
/// every order its connecting and its pieces can come in between polls; two at once, in every
/// such order, and with the first holding its last byte until the second has its answer, as the
/// test's do, as far as the time goes. Serving the first slot alone, so the second client waits
/// forever, and never closing a connection, so its client waits forever, are found.
#[tokio::test]
async fn proves_a_server_for_clients_at_once() {
    if !solvers_installed() {
        return;
    }
    let idea = asset("server-template-idea.md");
    for target in Profile::ALL {
        let name = target.name();
        let (template, [first_slot, open]) = *SERVER_TEMPLATES.get(target);
        let source = asset(template);
        let right = machine(&source).await;
        let (one, two) = (block(&idea, 0), block(&idea, 1));
        let budget = |seconds| Budget {
            max_shapes: 100_000,
            listens: true,
            ..budget(seconds)
        };
        let verdict = prove(&right, &one, &budget(60));
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{name}: {verdict:?}"
        );
        // The model agrees with the real program on the tests' samples, each client but the
        // last holding its last byte, as a check asks before it proves.
        for block in [&one, &two] {
            let samples: Vec<crate::testfile::Case> =
                block.cases(1).take(24).filter_map(Result::ok).collect();
            let found = agrees(&right, &samples, true).expect("the model agrees with the samples");
            assert!(found.is_none(), "{name}: {found:?}");
        }
        let verdict = prove(&right, &two, &budget(20));
        assert!(
            matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 8),
            "{name}: {verdict:?}"
        );
        let verdict = prove(
            &machine(&edited(&source, &[first_slot])).await,
            &two,
            &budget(60),
        );
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
        let verdict = prove(&machine(&edited(&source, &[open])).await, &one, &budget(60));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
    }
}

/// The folders of the program that prints the second of two lines.
const SECOND: Folders = PerCpu {
    x86_64: "tests/fixtures/second",
    aarch64: "tests/fixtures/linux-aarch64/second",
};

/// The edit that reads with one read of the operation in place of fill, on each target. The call
/// of fill stays, past a jump nothing takes, so that the program still has fill, as one that reads
/// with fill elsewhere and with a read of its own here does: a proof takes every count a read can
/// return only for a program that has fill.
const RAW_READ: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov ecx, NEWLINE\n    call fill\n",
        "    call os_read\n    jmp .Lraw_read_done\n    mov ecx, NEWLINE\n    call fill\n.Lraw_read_done:\n",
    ),
    aarch64: (
        "    mov w3, #NEWLINE\n    bl fill\n",
        "    bl os_read\n    b .Lraw_read_done\n    mov w3, #NEWLINE\n    bl fill\n.Lraw_read_done:\n",
    ),
};

/// One read of its own in place of fill is right only when both lines arrive in one read: with
/// at most 8 bytes left, the proof takes every count a read can return and finds a short one.
#[tokio::test]
async fn a_raw_read_of_short_input_is_proven_for_every_count() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let idea = twin(&SECOND, target, "idea.md");
        let raw = edited(&twin(&SECOND, target, "main.s"), &[*RAW_READ.get(target)]);
        let verdict = prove(&machine(&raw).await, &block(&idea, 0), &budget(60));
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{}: {verdict:?}", target.name());
        };
        let Some(Value::Str(delivery)) = found.get(crate::prove::DELIVERY) else {
            panic!("{}: {found:?}", target.name());
        };
        assert!(
            delivery.starts_with("in pieces, cut after byte"),
            "{}: {delivery}",
            target.name()
        );
    }
}

/// The folders of the line-echo server.
const ECHO_SERVER: Folders = PerCpu {
    x86_64: "examples/echo",
    aarch64: "tests/fixtures/linux-aarch64/echo",
};

/// The edit that lends fill a buffer 8 bytes too small for its size, on each target.
const FILL_LENT_TOO_LITTLE: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov edx, LINE_MAX\n    mov ecx, NEWLINE\n    call fill\n",
        "    mov edx, LINE_MAX + 8\n    mov ecx, NEWLINE\n    call fill\n",
    ),
    aarch64: (
        "    mov x2, #LINE_MAX\n    mov w3, #NEWLINE\n    bl fill\n",
        "    mov x2, #LINE_MAX + 8\n    mov w3, #NEWLINE\n    bl fill\n",
    ),
};

/// A server on the network: the line-echo server reads its line with fill, so it answers every
/// client's line however the network cuts it; one that reads once in place of fill, or never
/// closes a connection and so waits on a client that waits for it, is found.
/// A buffer lent to the kernel must hold every byte the call may touch, however little arrives:
/// a raw read, and fill's reads.
#[tokio::test]
async fn buffers_lent_to_the_kernel_hold_every_byte_it_may_touch() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let (echo, size) = *ECHO_OF_STDIN.get(target);
        let name = target.name();
        let bigger = size.replace("8192", "8200");
        let read = edited(&echo(""), &[(size, &bigger)]);
        let verdict = prove(
            &machine(&read).await,
            &echo_block("{text}", "ab"),
            &budget(60),
        );
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{name}: {verdict:?}");
        };
        let Some(Value::Str(problem)) = found.get(UNSAFE) else {
            panic!("{name}: {found:?}");
        };
        assert!(
            problem.starts_with(
                "in _start, the read system call writes 8200 bytes at byte 0 of buffer, which \
                 has 8192 bytes, so it reaches"
            ),
            "{name}: {problem}"
        );
        let idea = twin(&ECHO_SERVER, target, "idea.md");
        let source = twin(&ECHO_SERVER, target, "main.s");
        let fill = edited(&source, &[*FILL_LENT_TOO_LITTLE.get(target)]);
        let verdict = prove(
            &machine(&fill).await,
            &block(&idea, 0),
            &network_budget(true),
        );
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{name}: {verdict:?}");
        };
        let Some(Value::Str(problem)) = found.get(UNSAFE) else {
            panic!("{name}: {found:?}");
        };
        assert!(
            problem.starts_with("in fill, the read system call writes")
                && problem.contains("of line"),
            "{name}: {problem}"
        );
    }
}

/// The three bugs of the line-echo server, on each target: one read in place of fill, reading to
/// the end of the request, and a connection never closed.
type ServerBugs = [Edit; 3];

/// The line-echo server's bugs, on each target. fill's call is `call fill` after `mov ecx,
/// NEWLINE` on x86-64, and `bl fill` after `mov w3, #NEWLINE` on AArch64.
const ECHO_SERVER_BUGS: PerCpu<ServerBugs> = PerCpu {
    x86_64: [
        (
            "    mov ecx, NEWLINE\n    call fill\n",
            "    call os_read\n",
        ),
        (
            "    mov ecx, NEWLINE\n    call fill\n",
            "    mov ecx, -1\n    call fill\n",
        ),
        (
            "    mov edi, ebx\n    call os_close\n",
            "    xor eax, eax\n",
        ),
    ],
    aarch64: [
        ("    mov w3, #NEWLINE\n    bl fill\n", "    bl os_read\n"),
        (
            "    mov w3, #NEWLINE\n    bl fill\n",
            "    mov w3, #-1\n    bl fill\n",
        ),
        ("    mov x0, x19\n    bl os_close\n", "    mov x0, #0\n"),
    ],
};

/// The line-echo server reads its line with fill and is proven however the network cuts the
/// line; one read in place of fill, reading to the end of the request, and never closing a
/// connection are found.
#[tokio::test]
async fn proves_the_echo_server() {
    for target in Profile::ALL {
        let name = target.name();
        let idea = twin(&ECHO_SERVER, target, "idea.md");
        let source = twin(&ECHO_SERVER, target, "main.s");
        let block = block(&idea, 0);
        let right = machine(&source).await;
        let verdict = prove(&right, &block, &network_budget(true));
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    questions: 0,
                    cut_any_way: true,
                    ..
                }
            ),
            "{name}: {verdict:?}"
        );
        let [once, to_the_end, open] = *ECHO_SERVER_BUGS.get(target);
        // One read in place of fill: a line cut short by the network is found.
        let verdict = prove(
            &machine(&edited(&source, &[once])).await,
            &block,
            &network_budget(true),
        );
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{name}: {verdict:?}");
        };
        assert_ne!(
            found.get(DELIVERY),
            Some(&Value::Str(Delivery::Whole.describe())),
            "{name}: {found:?}"
        );
        // Reading to the end of the request: the client, as curl does, keeps its side open
        // while it waits for the answer, so the server waits forever.
        let verdict = prove(
            &machine(&edited(&source, &[to_the_end])).await,
            &block,
            &network_budget(true),
        );
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
        let verdict = prove(
            &machine(&edited(&source, &[open])).await,
            &block,
            &network_budget(true),
        );
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{name}: {verdict:?}"
        );
    }
}

/// The folders of the fetch client.
const FETCH: Folders = PerCpu {
    x86_64: "examples/fetch",
    aarch64: "tests/fixtures/linux-aarch64/fetch",
};

/// The edit that reads the reply once in place of fill, on each target: a reply cut by the
/// network comes up short.
const FETCH_READS_ONCE: PerCpu<Edit> = PerCpu {
    x86_64: ("    mov ecx, -1\n    call fill\n", "    call os_read\n"),
    aarch64: ("    mov w3, #-1\n    bl fill\n", "    bl os_read\n"),
};

/// A client on the network, through the connection lengua makes for it: fetch is proven for
/// every path and body and every delivery, and so is a server it can't reach, for every path; a
/// request with the wrong version, or one cut short before reading the reply, is found.
#[tokio::test]
async fn proves_the_fetch_client() {
    for target in Profile::ALL {
        let name = target.name();
        let idea = twin(&FETCH, target, "idea.md")
            .replacen(
                "path has `1` to `100` characters",
                "path has `1` to `100` characters (tests: `6` and `11`)",
                1,
            )
            .replacen("`string(1, 100, ", "`string(1, 6, ", 1)
            .replacen("`string(0, 65000, ", "`string(0, 11, ", 1);
        let source = twin(&FETCH, target, "main.s");
        let right = machine(&source).await;
        for index in 0..2 {
            let verdict = prove(&right, &block(&idea, index), &network_budget(false));
            // The reply is read with fill: proven however the network cuts it.
            assert!(
                matches!(
                    verdict,
                    Verdict::Proven {
                        bound: None,
                        questions: 0,
                        cut_any_way: true,
                        ..
                    }
                ),
                "block {index} on {name}: {verdict:?}"
            );
        }
        let fetch = block(&idea, 0);
        let wrongs = [
            (
                ".ascii \" HTTP/1.0\\r\\n\\r\\n\"",
                ".ascii \" HTTP/1.1\\r\\n\\r\\n\"",
            ),
            (".equ VERSION_LEN, 13", ".equ VERSION_LEN, 11"),
            // One read in place of fill: a reply cut by the network comes up short.
            *FETCH_READS_ONCE.get(target),
        ];
        for wrong in wrongs {
            let verdict = prove(
                &machine(&edited(&source, &[wrong])).await,
                &fetch,
                &network_budget(false),
            );
            assert!(
                matches!(verdict, Verdict::Counterexample(_)),
                "{name}: {verdict:?}"
            );
        }
    }
}

/// The folders of the word count that pads its counts to a column.
const PADDED: Folders = PerCpu {
    x86_64: "tests/fixtures/padded",
    aarch64: "tests/fixtures/linux-aarch64/padded",
};

/// Output padded to a column by a chain of if(…)s choosing among texts of different lengths, as
/// the word count Claude Code wrote in interactive round 2 prints its counts.
#[tokio::test]
async fn proves_counts_padded_to_a_column() {
    for target in Profile::ALL {
        // Texts up to 120 bytes, so counts of one, two and three digits each get their padding.
        let idea = twin(&PADDED, target, "idea.md")
            .replacen(
                "of up to `1048576` bytes",
                "of up to `1048576` bytes (tests: `120`)",
                1,
            )
            .replacen("string(0, 1048576,", "string(0, 120,", 1);
        let machine = machine(&twin(&PADDED, target, "main.s")).await;
        let verdict = prove(&machine, &block(&idea, 0), &text_budget());
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// One block of a program in tests/fixtures, for exploring: `LENGUA_FIXTURE` names it and
/// `LENGUA_BLOCK` the block's place among the generated ones.
#[tokio::test]
#[ignore = "explores one block of a fixture"]
async fn explores_a_fixture_block() {
    let name = std::env::var("LENGUA_FIXTURE").expect("LENGUA_FIXTURE names a fixture");
    let index: usize = std::env::var("LENGUA_BLOCK")
        .ok()
        .and_then(|block| block.parse().ok())
        .unwrap_or(0);
    let seconds: u64 = std::env::var("LENGUA_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(60);
    let machine = machine(&fixture(&name, "main.s")).await;
    let block = block(&fixture(&name, "idea.md"), index);
    let began = Instant::now();
    let verdict = prove(
        &machine,
        &block,
        &Budget {
            max_shapes: 2_000_000,
            max_paths: 256,
            ..budget(seconds)
        },
    );
    eprintln!("{name} block {index}: {verdict:?} in {:?}", began.elapsed());
}

/// Runs blocks of fixtures that one target's proof can't settle in a process of its own each,
/// with the prover saying which condition it asked the solvers about (`LENGUA_PROVE_EXPLAIN`), and
/// prints the first of what it said: for seeing why a proof ends "a path couldn't be settled".
/// The blocks come from `LENGUA_EXPLAIN`, `fixture:block` pairs separated by spaces, with
/// `LENGUA_EXPLAIN_SECONDS` for each and `LENGUA_EXPLAIN_LINES` lines of what it said.
#[tokio::test]
#[ignore = "explains why a proof is not settled"]
async fn explains_what_a_proof_asks() {
    let blocks = std::env::var("LENGUA_EXPLAIN")
        .unwrap_or_else(|_| "amounts:0 linux-aarch64/amounts:0 linux-aarch64/mean:1".to_owned());
    let seconds = std::env::var("LENGUA_EXPLAIN_SECONDS").unwrap_or_else(|_| "60".to_owned());
    let most: usize = std::env::var("LENGUA_EXPLAIN_LINES")
        .ok()
        .and_then(|lines| lines.parse().ok())
        .unwrap_or(60);
    // "path" has the prover say each path's own conditions too.
    let mode = std::env::var("LENGUA_EXPLAIN_MODE").unwrap_or_else(|_| "path".to_owned());
    let binary = std::env::current_exe().expect("this test's binary");
    for pair in blocks.split_whitespace() {
        let (fixture_name, number) = pair.split_once(':').expect("a fixture and a block");
        let output = std::process::Command::new(&binary)
            .args([
                "--ignored",
                "--exact",
                "prove::tests::explores_a_fixture_block",
                "--nocapture",
            ])
            .env("LENGUA_FIXTURE", fixture_name)
            .env("LENGUA_BLOCK", number)
            .env("LENGUA_SECONDS", &seconds)
            .env("LENGUA_PROVE_EXPLAIN", &mode)
            .output()
            .expect("run the exploring test");
        let said = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = said.lines().collect();
        eprintln!(
            "== {fixture_name} block {number}: {} lines said",
            lines.len()
        );
        for line in lines.iter().take(most) {
            eprintln!("{}", line.chars().take(900).collect::<String>());
        }
        eprintln!("== the last of it");
        for line in lines.iter().rev().take(3).rev() {
            eprintln!("{}", line.chars().take(900).collect::<String>());
        }
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

/// A program that reads standard input whole and prints it back with each byte from `a` to
/// `last` moved down 32: capitals, when `last` is `z`. On AArch64.
fn capitals_aarch64(last: u8) -> String {
    format!(
        ".arch armv8-a
.globl _start
.text
.type _start, @function
_start:
    mov x0, #0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x2, #64
    mov x8, #63
    svc #0
    mov x12, x0
    adrp x1, buffer
    add x1, x1, :lo12:buffer
    mov x3, #0
.Lnext:
    cmp x3, x12
    b.hs .Lprint
    ldrb w5, [x1, x3]
    cmp w5, #97
    b.lo .Lkeep
    cmp w5, #{last}
    b.hi .Lkeep
    sub w5, w5, #32
    strb w5, [x1, x3]
.Lkeep:
    add x3, x3, #1
    b .Lnext
.Lprint:
    mov x0, #1
    mov x2, x12
    mov x8, #64
    svc #0
    mov x0, #0
    mov x8, #93
    svc #0
.size _start, . - _start
.bss
.type buffer, @object
buffer:
    .zero 64
.size buffer, 64
"
    )
}

/// A program that reads standard input whole and prints it back with each byte from `a` to
/// `last` moved down 32: capitals, when `last` is `z`. On x86-64.
fn capitals_x86_64(last: u8) -> String {
    format!(
        ".intel_syntax noprefix
.globl _start
.text
.type _start, @function
_start:
    xor edi, edi
    lea rsi, [rip + buffer]
    mov edx, 64
    xor eax, eax
    syscall
    mov r12, rax
    xor ecx, ecx
.Lnext:
    cmp rcx, r12
    jae .Lprint
    movzx eax, byte ptr [rsi + rcx]
    cmp eax, 97
    jb .Lkeep
    cmp eax, {last}
    ja .Lkeep
    sub eax, 32
    mov byte ptr [rsi + rcx], al
.Lkeep:
    inc rcx
    jmp .Lnext
.Lprint:
    mov edi, 1
    mov rdx, r12
    mov eax, 1
    syscall
    xor edi, edi
    mov eax, 60
    syscall
.size _start, . - _start
.bss
.type buffer, @object
buffer:
    .zero 64
.size buffer, 64
"
    )
}

/// The capitals of each target.
const CAPITALS: PerCpu<fn(u8) -> String> = PerCpu {
    x86_64: capitals_x86_64,
    aarch64: capitals_aarch64,
};

/// The idea of the capitals over text beyond ASCII.
const CAPITALS_IDEA: &str = r#"---
type: lengua/program-idea
title: capitals
description: Turns text into capitals.
---
# Purpose
The program prints standard input of `0` to `8` characters back with each ASCII letter in capitals.

# Behaviors
## B1: Prints it in capitals
- Given standard input that contains `ab`
- When the program runs
- Then it prints `AB`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `string(0, 8, "abz é€")` |

# Constraints
- Uses: standard input
"#;

/// Text beyond ASCII: each character is one of those of its width, é and € among them, and
/// every order of widths is a shape of its own, so capitals of ASCII letters are proven over
/// text with accents and symbols, up to 8 characters; capitals that take 0xc3, é's first byte,
/// for a letter, and so take bytes up to it, are found.
#[tokio::test]
async fn proves_capitals_over_text_beyond_ascii() {
    let block = block(CAPITALS_IDEA, 0);
    // A path for each choice of letter or not at each ASCII place.
    let budget = || Budget {
        max_paths: 512,
        ..text_budget()
    };
    for target in Profile::ALL {
        let program = *CAPITALS.get(target);
        let verdict = prove(&machine(&program(b'z')).await, &block, &budget());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    shapes: 9841,
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let verdict = prove(&machine(&program(0xc3)).await, &block, &budget());
        let Verdict::Counterexample(found) = &verdict else {
            panic!("{}: {verdict:?}", target.name());
        };
        // é's first byte, or €'s later ones, which fall in the same range.
        assert!(
            matches!(found.get("text"), Some(Value::Str(text)) if !text.is_ascii()),
            "{}: {found:?}",
            target.name()
        );
    }
}

/// The folders of the program that counts lines and words.
const COUNT: Folders = PerCpu {
    x86_64: "examples/count",
    aarch64: "tests/fixtures/linux-aarch64/count",
};

/// examples/count's lines and words over text with accented letters and symbols: every byte of
/// a character beyond ASCII belongs to a word.
#[tokio::test]
async fn proves_a_word_count_over_text_beyond_ascii() {
    for target in Profile::ALL {
        let purpose = twin(&COUNT, target, "idea.md").replacen(
            "The tests give it standard input of up to `1048576` bytes",
            "The tests give it standard input of up to `1048576` bytes (tests: `16`)",
            1,
        );
        let idea = purpose.replacen(
            "string(0, 1048576, \"abcdefghijklmnopqrstuvwxyz0123456789 \\n\")",
            "string(0, 16, \"abcdefghijklmnopqrstuvwxyz0123456789 \\né€\")",
            1,
        );
        assert_ne!(idea, purpose);
        let machine = machine(&twin(&COUNT, target, "main.s")).await;
        let verdict = prove(&machine, &block(&idea, 0), &text_budget());
        // Characters of three widths in every order, up to 8 of them.
        let Verdict::Proven {
            bound: Some(bound), ..
        } = &verdict
        else {
            panic!("{}: {verdict:?}", target.name());
        };
        assert_eq!(bound.bytes, 8, "{}: {verdict:?}", target.name());
        assert!(bound.characters, "{}: {verdict:?}", target.name());
        assert!(
            bound.why.contains("8 characters"),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// The folders of the count of pairs of a letter.
const PAIRS: Folders = PerCpu {
    x86_64: "tests/fixtures/pairs",
    aarch64: "tests/fixtures/linux-aarch64/pairs",
};

/// The three bugs in the letter pending across buffer-fulls, on each target: dropped at a
/// buffer-full's end, ignored coming into the count, and one pending from the start. x86-64
/// keeps it in r14, AArch64 in x22.
const FORGETS_THE_PENDING_LETTER: PerCpu<[Edit; 3]> = PerCpu {
    x86_64: [
        ("    mov r14, rdx\n", "    xor r14d, r14d\n"),
        ("    mov rdx, rsi\n", "    xor edx, edx\n"),
        (
            "    xor r14d, r14d\n    mov rbx, MAX_FULLS\n",
            "    mov r14d, 1\n    mov rbx, MAX_FULLS\n",
        ),
    ],
    aarch64: [
        ("    mov x22, x1\n", "    mov x22, #0\n"),
        ("    mov x1, x22\n", "    mov x1, #0\n"),
        (
            "    mov x22, #0\n    mov x19, #MAX_FULLS\n",
            "    mov x22, #1\n    mov x19, #MAX_FULLS\n",
        ),
    ],
};

/// The bug that counts the whole buffer-full, past what fill read, on each target.
const COUNTS_PAST_WHAT_FILL_READ: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov rdi, rax\n    mov rsi, r14\n",
        "    mov edi, BUF_LEN\n    mov rsi, r14\n",
    ),
    aarch64: (
        "    mov x1, x22\n    bl count_pairs\n",
        "    mov x1, x22\n    mov x0, #BUF_LEN\n    bl count_pairs\n",
    ),
};

/// A count of a part that overlaps itself, `aa`: how much of it the text ends with carries
/// across each cut, which no last few bytes settle (on `aaa…` it's the parity of everything
/// read), so a register the first time round shows keeping it, and every later time round
/// proven to keep it, carries the count from one buffer-full to the next: the fixture is proven
/// for every length. Programs that drop it at a buffer-full's end, ignore it coming into the
/// count, or start out with an `a` pending each get an input that shows it.
#[tokio::test]
async fn proves_a_count_of_a_part_that_overlaps_itself_for_every_length() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let source = twin(&PAIRS, target, "main.s");
        let block = block(&twin(&PAIRS, target, "idea.md"), 0);
        let right = machine(&source).await;
        let verdict = prove(&right, &block, &text_budget());
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(256),
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        for edit in FORGETS_THE_PENDING_LETTER.get(target) {
            let wrong = machine(&edited(&source, &[*edit])).await;
            let verdict = prove(&wrong, &block, &text_budget());
            assert!(
                matches!(verdict, Verdict::Counterexample(_)),
                "{} on {:?}: {verdict:?}",
                target.name(),
                edit.1
            );
        }
    }
}

/// The induction's base takes every final piece at once, its length the input's choice: the
/// count loop's exits on that length merge into one run, each part counted where it's inside,
/// so `pairs` with a 4 KiB buffer is proven for every length in seconds with one shape for its
/// base. A program that counts the whole buffer-full, past what fill read, is caught: its base
/// doesn't hold, and from the program's start a text just over a buffer-full shows it.
#[tokio::test]
async fn proves_every_final_piece_at_once_where_a_loop_leaves_on_the_length() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let block = block(&twin(&PAIRS, target, "idea.md"), 0);
        let source = twin(&PAIRS, target, "main.s");
        let larger = edited(&source, &[(".equ BUF_LEN, 256\n", ".equ BUF_LEN, 4096\n")]);
        let large = machine(&larger).await;
        let started = Instant::now();
        let verdict = prove(&large, &block, &text_budget());
        eprintln!(
            "pairs at 4 KiB on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(4096),
                    shapes: 2,
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let past = edited(&source, &[*COUNTS_PAST_WHAT_FILL_READ.get(target)]);
        let verdict = prove(&machine(&past).await, &block, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// A form of the fast word count that is proven for every length at 64 KiB, and the bugs it is
/// held to.
#[derive(Clone, Copy)]
struct LongWordCount {
    /// Its main.s, from the repository's root.
    main: &'static str,
    /// The edits that give the bugs a smaller buffer, whose counterexamples come sooner: the
    /// buffer, and the loop's bound that counts its steps.
    smaller: [Edit; 2],
    /// Four bugs: a piece that forgets the whitespace before it, a step that forgets the one
    /// before it, a lane or word start past the end counted, and the larger of the bytes left and
    /// a step taken for the smaller (or, for SVE, a step that skips a vector).
    bugs: [Edit; 4],
    /// Whether its proof needs no solver question.
    no_question: bool,
    /// Whether it runs on SVE vectors, whose length the proof is given by the CPU it runs for.
    needs_sve: bool,
}

/// The fast word count's two forms on each target, each proven for every length with its own
/// 64 KiB buffer: the baseline every machine of the target runs, then the optional extension's
/// tier beside it. x86-64's mutants run with a 256-byte buffer for the baseline (with 4 KiB the
/// prover took over a minute on a word start past the end, a bug a 3-byte text shows) and 4 KiB
/// for the tier.
const LONG_WORD_COUNTS: PerCpu<[LongWordCount; 2]> = PerCpu {
    x86_64: [
        LongWordCount {
            main: "plugin/skills/compile/assets/fast/wc/main.s",
            smaller: [
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 256\n"),
                ("  # bound: 4096\n", "  # bound: 16\n"),
            ],
            bugs: [
                (
                    "    mov r10d, r14d\n    call count_piece\n",
                    "    mov r10d, 1\n    call count_piece\n",
                ),
                ("    mov r10d, r8d\n    shr r10d, 15\n", ""),
                ("    pandn xmm3, xmm6\n", "    pcmpeqb xmm3, xmm10\n"),
                ("    cmova r8, r9\n", "    cmovb r8, r9\n"),
            ],
            no_question: true,
            needs_sve: false,
        },
        LongWordCount {
            main: "plugin/skills/compile/assets/fast/wc-avx512/main.s",
            smaller: [
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 4096\n"),
                ("  # bound: 1024\n", "  # bound: 64\n"),
            ],
            bugs: [
                (
                    "    mov edx, r14d\n    call count_piece\n",
                    "    mov edx, 1\n    call count_piece\n",
                ),
                (
                    "    mov r10, rax\n    add rcx, VECTOR\n",
                    "    add rcx, VECTOR\n",
                ),
                ("    kmovq k1, rdx\n    dec eax\n", "    kmovq k1, rdx\n"),
                ("    cmova rax, rdx\n", "    cmovb rax, rdx\n"),
            ],
            no_question: true,
            needs_sve: false,
        },
    ],
    aarch64: [
        LongWordCount {
            main: "plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s",
            smaller: [
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 4096\n"),
                ("  // bound: 4096\n", "  // bound: 256\n"),
            ],
            bugs: [
                (
                    "    mov x2, x23\n    bl count_piece\n",
                    "    mov x2, #0xff\n    bl count_piece\n",
                ),
                ("    mov v21.16b, v2.16b\n", ""),
                (
                    "    cmhi v22.16b, v22.16b, v16.16b\n",
                    "    cmhs v22.16b, v22.16b, v16.16b\n",
                ),
                ("    csel x4, x4, x5, lo\n", "    csel x4, x5, x4, lo\n"),
            ],
            no_question: true,
            needs_sve: false,
        },
        LongWordCount {
            main: "plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s",
            smaller: [
                (".equ BUF_LEN, 65536\n", ".equ BUF_LEN, 4096\n"),
                ("  // bound: 4096\n", "  // bound: 256\n"),
            ],
            bugs: [
                (
                    "    mov x2, x23\n    bl count_piece\n",
                    "    mov x2, #0xff\n    bl count_piece\n",
                ),
                ("    mov w12, w13\n", ""),
                (
                    "    ld1b { z0.b }, p0/z, [x0, x10]\n",
                    "    ptrue p0.b\n    ld1b { z0.b }, p0/z, [x0, x10]\n",
                ),
                ("    incb x10\n", "    incb x10, all, mul #2\n"),
            ],
            no_question: false,
            needs_sve: true,
        },
    ],
};

/// The fast word count's form number `form` (0 the baseline, 1 the extension's tier) on each
/// target: fill into 64 KiB, and one loop that masks each step's bytes past the end off, with
/// lanes compared against their places, is proven for every length by induction, whose base
/// reads a buffer-full before every final piece and takes every final piece at once after it,
/// its length the input's choice. A word count that forgets the byte before each buffer-full or
/// before each step, or counts the bytes past the end, is caught.
async fn proves_a_fast_word_count_for_every_length(form: usize) {
    let block = block(&repository_file(WORD_COUNT_IDEA), 0);
    for target in Profile::ALL {
        let name = target.name();
        let count = LONG_WORD_COUNTS.get(target)[form];
        let asset = repository_file(count.main);
        let mut right = machine(&asset).await;
        if count.needs_sve {
            if let Some(bytes) = std::env::var("LENGUA_TRY_SVE_BYTES")
                .ok()
                .and_then(|bytes| bytes.parse().ok())
            {
                right.sve_bytes = bytes;
            }
            if right.sve_bytes == 0 {
                crate::skiplog::note("neither this CPU nor QEMU's here has SVE");
                continue;
            }
        }
        let started = Instant::now();
        let verdict = prove(&right, &block, &long_budget());
        eprintln!(
            "the fast wc {} at 64 KiB on {name}: {verdict:?} in {:?}",
            count.main,
            started.elapsed()
        );
        assert!(
            matches!(
                &verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(65536),
                    questions,
                    ..
                } if !count.no_question || *questions == 0
            ),
            "{name}: {verdict:?}"
        );
        let small = edited(&asset, &count.smaller);
        for bug in count.bugs {
            let wrong = machine(&edited(&small, &[bug])).await;
            let verdict = prove(&wrong, &block, &text_budget());
            assert!(
                matches!(verdict, Verdict::Counterexample(_)),
                "{name}: {:?}: {verdict:?}",
                bug.1
            );
        }
    }
}

/// The baseline fast word count of each target (SSE2 and `popcnt`, NEON), with no solver
/// question, whatever CPU runs the proof.
#[tokio::test]
async fn proves_the_fast_word_count_for_every_length() {
    if !solvers_installed() {
        return;
    }
    proves_a_fast_word_count_for_every_length(0).await;
}

/// The fast tier beside the baseline word count of each target (AVX-512 masks, SVE with
/// `whilelo` at the CPU's vector length), from the very same idea.
#[tokio::test]
async fn proves_the_fast_word_count_tier_for_every_length() {
    if !solvers_installed() {
        return;
    }
    proves_a_fast_word_count_for_every_length(1).await;
}

/// A proof keeps its time even inside one long shape, as the fast word count's base that takes
/// every final piece at once is: given seconds where it needs minutes, it stops within its
/// budget and a shape's least time, where once it ran six minutes past.
#[tokio::test]
async fn a_proof_keeps_its_time_within_one_long_shape() {
    if !solvers_installed() {
        return;
    }
    let block = block(&repository_file(WORD_COUNT_IDEA), 0);
    for target in Profile::ALL {
        let (_, path) = WORD_COUNT_FORMS.get(target)[0];
        let asset = machine(&repository_file(path)).await;
        let short = budget(10);
        let given = short.deadline.saturating_duration_since(Instant::now());
        let started = Instant::now();
        let verdict = prove(&asset, &block, &short);
        let took = started.elapsed();
        assert!(
            took <= given + MIN_SHAPE_TIME * 2 + Duration::from_secs(20),
            "{}: {took:?} for {given:?}: {verdict:?}",
            target.name()
        );
        assert!(
            !matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// Lines of assembly, each indented and ended as the programs write them.
fn assembly(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("    {line}\n")).collect()
}

/// The line counter as it was before it ran on the baseline: its last chunk masked by the bytes
/// left with `bzhi` (BMI2), in place of the SSE2 mask the fixture has now. The prover reads `bzhi`
/// too, so the tests keep proving that form.
fn lines_with_bzhi(source: &str) -> String {
    let steps = [
        (
            assembly(&["movdqu xmm3, xmmword ptr [rip + places]", "pxor xmm4, xmm4"]),
            String::new(),
        ),
        (
            assembly(&[
                "movd xmm2, r9d",
                "pshufb xmm2, xmm4",
                "pminub xmm2, xmm3",
                "pcmpeqb xmm2, xmm3",
                "pand xmm0, xmm2",
            ]),
            String::new(),
        ),
        (
            assembly(&["pmovmskb edx, xmm0", "popcnt edx, edx"]),
            assembly(&[
                "pmovmskb edx, xmm0",
                "bzhi edx, edx, r9d",
                "popcnt edx, edx",
            ]),
        ),
    ];
    let mut text = source.to_owned();
    for (from, to) in steps {
        assert!(text.contains(&from), "the fixture changed: {from:?}");
        text = text.replace(&from, &to);
    }
    text
}

/// `target`'s line counter source with a 64 KiB buffer, C's size, and its idea's block for it.
fn lines_at_64_kib(target: Profile) -> (String, Generated) {
    let idea = twin(&LINES, target, "idea.md").replace("65536", "1048576");
    let source = edited(
        &twin(&LINES, target, "main.s"),
        &[(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 65536\n")],
    );
    (source, block(&idea, 0))
}

/// The line counter with a 64 KiB buffer, C's size, as the fixture is written, on the baseline:
/// its count loop masks its last chunk by the bytes left with SSE2 (`pminub` and `pcmpeqb` against
/// the lanes' places plus one, unsigned compares the prover reads), and with NEON on AArch64
/// (`cmhi` of the bytes left against the lanes' places), and what each byte's mask says of the
/// length comes out as the formula's `position < length`, so the base takes every final piece at
/// once and the counter is proven for every length with no solver question. A counter that skips
/// its last chunk is still caught.
#[tokio::test]
async fn proves_a_count_with_a_64_kib_buffer_for_every_length() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let (source, block) = lines_at_64_kib(target);
        let right = machine(&source).await;
        let started = Instant::now();
        let verdict = prove(&right, &block, &long_budget());
        eprintln!(
            "lines at 64 KiB on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(65536),
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let short = edited(&source, &[*MISSES_THE_LAST_CHUNK.get(target)]);
        let verdict = prove(&machine(&short).await, &block, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// How a target makes the line counter mask its last chunk with one instruction: a change of the
/// fixture, or why the target has no such form.
type OneInstructionMask = Result<fn(&str) -> String, &'static str>;

/// The line counter whose last chunk is masked by the bytes left with one instruction of the
/// target's, in place of the fixture's mask of several, or why the target has no such form.
const MASKED_WITH_ONE_INSTRUCTION: PerCpu<OneInstructionMask> = PerCpu {
    x86_64: Ok(lines_with_bzhi),
    aarch64: Err(
        "AArch64 has no instruction that clears a register's bits from a place in another \
         register up, as BMI2's `bzhi` does: the bitfield instructions (`ubfx`, `bfxil`) take a \
         constant place, so the fixture's own mask, `cmhi` of the bytes left against the lanes' \
         places, is the one form its counter has",
    ),
};

/// The same counter with its last chunk masked by `bzhi` (BMI2), which the prover reads as the
/// formula's `position < length` too: proven for every length with no solver question, two
/// shapes, as before the fixture moved to the baseline. A counter that skips its last chunk, or
/// keeps only the last buffer-full's count, is still caught.
#[tokio::test]
async fn proves_a_count_masked_with_bzhi_for_every_length() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let make = match MASKED_WITH_ONE_INSTRUCTION.get(target) {
            Ok(make) => *make,
            Err(why) => {
                assert!(why.len() > 40, "say why {} can't", target.name());
                continue;
            }
        };
        let (source, block) = lines_at_64_kib(target);
        let source = make(&source);
        let right = machine(&source).await;
        let started = Instant::now();
        let verdict = prove(&right, &block, &long_budget());
        eprintln!(
            "lines with one instruction's mask at 64 KiB on {}: {verdict:?} in {:?}",
            target.name(),
            started.elapsed()
        );
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    induction: Some(65536),
                    shapes: 2,
                    ..
                }
            ),
            "{}: {verdict:?}",
            target.name()
        );
        let short = edited(&source, &[*MISSES_THE_LAST_CHUNK.get(target)]);
        let verdict = prove(&machine(&short).await, &block, &text_budget());
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{}: {verdict:?}",
            target.name()
        );
    }
}

/// A function of a file of tests: its name, whether it is a test, and its body.
struct Function {
    name: String,
    is_test: bool,
    body: String,
}

/// The functions of `text` that begin `indent` in, ended by a closing brace at that indent.
fn functions_of(text: &str, indent: &str) -> Vec<Function> {
    let close = format!("{indent}}}");
    let mut found = Vec::new();
    let mut current: Option<Function> = None;
    let mut is_test = false;
    for line in text.lines() {
        if let Some(function) = current.as_mut() {
            if line == close {
                found.extend(current.take());
            } else {
                function.body.push_str(line);
                function.body.push('\n');
            }
            continue;
        }
        let Some(code) = line.strip_prefix(indent) else {
            continue;
        };
        if code.starts_with("#[tokio::test") || code == "#[test]" {
            is_test = true;
        } else if let Some(rest) = code
            .strip_prefix("async fn ")
            .or_else(|| code.strip_prefix("fn "))
        {
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            current = Some(Function {
                name,
                is_test,
                body: String::new(),
            });
            is_test = false;
        }
    }
    found
}

/// The prover tests that are about no CPU's code: arithmetic on ranges and terms, and tools that
/// prove nothing.
const NOT_ABOUT_A_CPU: &[&str] = &[
    "splits_ranges_into_digit_pieces",
    "goes_through_every_combination_once_by_its_longest_text",
    "reads_conditions_as_intervals_and_settles_only_what_holds",
    "hunt_for_a_seed_the_interval_test_stalls_on",
    "explores_a_program",
    "explores_a_fixture_block",
    "explains_what_a_proof_asks",
    "every_prover_test_runs_on_every_target",
    "the_scan_of_the_tests_finds_each_test",
    "the_short_name_shape_the_proof_tests_is_the_runs",
    "the_speed_probe_reads",
];

/// The suite stays one suite for every target: each test of the prover and of its executor runs
/// on every target (its body, or the function it calls, goes through `Profile::ALL` or a table's
/// cells), or is listed as about no CPU's code, and none is named for one CPU. A test written for
/// one CPU only, which the other would lack, fails here until it has its cell.
#[test]
fn every_prover_test_runs_on_every_target() {
    let executor = repository_file("src/prove/exec.rs");
    let from = executor
        .find("\nmod tests {")
        .expect("the executor's tests");
    let mut problems = Vec::new();
    for functions in [
        functions_of(&repository_file("src/prove/tests.rs"), ""),
        functions_of(&executor[from..], "    "),
    ] {
        let covers = |body: &str| body.contains("Profile::ALL") || body.contains(".cells()");
        for test in functions.iter().filter(|function| function.is_test) {
            if NOT_ABOUT_A_CPU.contains(&test.name.as_str()) {
                continue;
            }
            let through_a_helper = functions.iter().any(|other| {
                other.name != test.name
                    && test.body.contains(&format!("{}(", other.name))
                    && covers(&other.body)
            });
            if !covers(&test.body) && !through_a_helper {
                problems.push(format!("{} runs on no table of targets", test.name));
            }
            if test.name.contains("aarch64") || test.name.contains("x86") {
                problems.push(format!("{} is named for one CPU", test.name));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The scan sees the tests of a file of tests, and not what is not one.
#[test]
fn the_scan_of_the_tests_finds_each_test() {
    // Each line of the sample is a string of its own, so that the scan of this file does not
    // find the sample's functions at the start of a line.
    let text = [
        "#[tokio::test]",
        "#[ignore = \"slow\"]",
        "async fn one() {",
        "    for target in Profile::ALL {}",
        "}",
        "",
        "fn helper() {",
        "    let x = 1;",
        "}",
        "",
        "#[test]",
        "fn two() {",
        "    helper();",
        "}",
    ]
    .join("\n");
    let functions = functions_of(&text, "");
    let names: Vec<(&str, bool)> = functions
        .iter()
        .map(|function| (function.name.as_str(), function.is_test))
        .collect();
    assert_eq!(names, [("one", true), ("helper", false), ("two", true)]);
    assert!(functions[0].body.contains("Profile::ALL"));
    assert!(functions[2].body.contains("helper();"));
}
