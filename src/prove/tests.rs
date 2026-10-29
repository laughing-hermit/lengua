//! Proofs about real programs: the template, examples/add, and planted bugs the samples miss.

use std::time::{Duration, Instant};

use super::*;
use crate::platform::Profile;
use crate::toolchain::Toolchain;

async fn machine(source: &str) -> Machine {
    let toolchain = Toolchain::discover().await.expect("llvm-mc and ld.lld");
    let folder = tempfile::tempdir().expect("temporary folder");
    let (assembly, object, executable) = (
        folder.path().join("main.s"),
        folder.path().join("main.o"),
        folder.path().join("main"),
    );
    std::fs::write(&assembly, source).expect("write");
    toolchain
        .assemble(Profile::LinuxX86_64, &assembly, &object)
        .await
        .expect("assemble");
    toolchain.link(&object, &executable).await.expect("link");
    crate::checker::decode::read(&std::fs::read(&executable).expect("read")).expect("decode")
}

fn block(spec: &str, tests: &str, index: usize) -> Generated {
    let spec = crate::spec::parse(spec).expect("valid spec");
    let label = crate::hash::Sha256Hex::of(b"").label();
    let tests = tests.replacen(
        &tests
            .lines()
            .find(|line| line.starts_with("spec = "))
            .expect("a label")
            .to_owned(),
        &format!("spec = \"{label}\""),
        1,
    );
    let suite = crate::testfile::check(&tests, &spec, &label).expect("valid tests");
    suite.generated[index].clone()
}

fn budget(seconds: u64) -> Budget {
    Budget {
        deadline: Instant::now() + Duration::from_secs(seconds),
        max_paths: 64,
        max_shapes: 2000,
        question_ms: 10_000,
    }
}

fn asset(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/write-assembly/assets")
            .join(name),
    )
    .expect("an asset")
}

#[tokio::test]
async fn proves_the_template_for_every_input() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("z3 isn't installed");
        return;
    }
    let machine = machine(crate::checker::TEMPLATE).await;
    let block = block(&asset("template-spec.md"), &asset("template-tests.toml"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(120));
    eprintln!("{verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
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

fn example(program: &str, name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(program)
            .join(name),
    )
    .expect("an example file")
}

/// About a minute on 16 cores in release mode: `cargo test --release -- --ignored`.
#[tokio::test]
#[ignore = "slow: proves 4×10^18 pairs of inputs"]
async fn proves_add_for_every_pair_of_inputs() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("add", "main.s")).await;
    let block = block(&example("add", "spec.md"), &example("add", "tests.toml"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(300));
    eprintln!("add: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

#[tokio::test]
async fn finds_a_bug_the_samples_miss() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    // The negative of 1234567 comes out one too big: one input in 18 quintillion.
    let buggy = crate::checker::TEMPLATE
        .replacen(".equ OUT_BUF_LEN, 21\n", ".equ OUT_BUF_LEN, 21\n.equ UNLUCKY, -1234567\n", 1)
        .replacen(
            "    neg rdx\n    jo .Lstart_assert_negate\n",
            "    neg rdx\n    jo .Lstart_assert_negate\n    cmp rdx, UNLUCKY\n    jne .Lstart_lucky\n    \
             inc rdx\n.Lstart_lucky:\n",
            1,
        );
    assert_ne!(buggy, crate::checker::TEMPLATE);
    let machine = machine(&buggy).await;
    let block = block(&asset("template-spec.md"), &asset("template-tests.toml"), 0);
    let verdict = prove(&machine, &block, budget(120));
    assert_eq!(
        verdict,
        Verdict::Counterexample(BTreeMap::from([("n".to_owned(), Value::Int(1_234_567))]))
    );
}

/// The absolute difference, which branches on the sign of a - b where the formula has abs: the
/// path's own condition must settle the formula's if.
#[tokio::test]
async fn proves_absolute_difference() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("absdiff", "main.s")).await;
    let block = block(
        &example("absdiff", "spec.md"),
        &example("absdiff", "tests.toml"),
        0,
    );
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(300));
    eprintln!("absdiff: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// The mean of two prices in cents, rounded half away from zero: decimals in, decimals out. Not
/// proven yet: mean adds whole × 100 + cents, the input is one chain of digits, and the solver
/// can't settle that the two are equal in time. A linear normal form for sums would make them
/// one term.
#[tokio::test]
#[ignore = "not proven yet: the parse and the input differ in shape"]
async fn proves_the_mean_of_two_prices() {
    let machine = machine(&example("mean", "main.s")).await;
    let block = block(
        &example("mean", "spec.md"),
        &example("mean", "tests.toml"),
        0,
    );
    let started = Instant::now();
    let verdict = prove(&machine, &block, budget(300));
    eprintln!("mean: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// Decimal inputs: mean's usage (B4), a bad second price drawn by choice (B5) and a price out of
/// range (B6), each with a decimal drawn over its whole range.
#[tokio::test]
async fn proves_decimal_inputs_where_the_output_is_fixed() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("mean", "main.s")).await;
    for index in 3..6 {
        let block = block(
            &example("mean", "spec.md"),
            &example("mean", "tests.toml"),
            index,
        );
        let verdict = prove(&machine, &block, budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            block.id
        );
    }
}

/// A die: one random byte from the kernel, printed as 1 to 6. The kernel's byte is any value, so
/// the proof covers all 256; the rule `{any(1, 6)}` holds for each, and `% 7` breaks it.
#[tokio::test]
async fn proves_a_roll_stays_in_its_range() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    const SPEC: &str = "---
type: lengua/program-spec
title: roll
---
# Purpose
Rolls a die for a player numbered from `1` to `9`.

# Behaviors
## B1: Rolls a die
- Given the argument `1`
- When the program runs
- Then it prints `rolled ` and a number from `1` to `6`, followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, getrandom, exit
";
    const TESTS: &str = r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
args = ["1"]
stdout = { matches = "rolled {any(1, 6)}\n" }
exit_code = 0

[[generated]]
id = "B1"
vars = { player = "int(1, 9)" }
args = ["{player}"]
stdout = "rolled {any(1, 6)}\n"
exit_code = 0
"#;
    let program = |faces: u8| {
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
    };
    let block = block(SPEC, TESTS, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(6)).await;
    agrees(&right, &samples).expect("the model agrees with the samples");
    let verdict = prove(&right, &block, budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(7)).await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// A clock reading the test supplies, taken modulo 7: proven for every second from 1970 to 2100,
/// one shape, since the program never reads the time as digits. (Days of the week need a second
/// 64-bit division, which cvc5 doesn't confirm in time.)
#[tokio::test]
async fn proves_what_a_program_does_with_the_supplied_clock() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    const SPEC: &str = "---
type: lengua/program-spec
title: weekday
---
# Purpose
Exits with the clock's seconds since 1970 modulo 7, from `0` to `6`.

# Behaviors
## B1: Exits with the seconds modulo 7
- Given the clock at `1735689600`
- When the program runs
- Then it exits with code `1`

# Constraints
- Allowed system calls: clock_gettime, exit
";
    const TESTS: &str = r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
clock = ["1735689600"]
exit_code = 1

[[generated]]
id = "B1"
vars = { t = "int(0, 4102444800)" }
clock = ["{t}"]
exit_code = "{t % 7}"
"#;
    let program = |days: u32| {
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
    };
    let block = block(SPEC, TESTS, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(7)).await;
    agrees(&right, &samples).expect("the model agrees with the samples");
    let verdict = prove(&right, &block, budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(6)).await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

const ECHO_SPEC: &str = "---
type: lengua/program-spec
title: echo
description: Prints its argument.
---
# Purpose
Prints its argument, `1` to `3` letters.

# Behaviors
## B1: Prints the argument
- Given the argument `ab`
- When the program runs
- Then it prints `ab`
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
";

const ECHO_TESTS: &str = r#"[meta]
spec = "sha256:x"

[[example]]
id = "B1"
args = ["ab"]
stdout = "ab"
exit_code = 0

[[generated]]
id = "B1"
vars = { s = 'string(1, 3, "abc")' }
args = ["{s}"]
stdout = "{s}"
exit_code = 0
"#;

/// Writes its argument: every branch is settled by the bytes' ranges, and the output is the
/// argument's own terms.
const ECHO: &str = ".intel_syntax noprefix
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
";

#[tokio::test]
async fn a_shape_its_terms_settle_starts_no_solver() {
    let machine = machine(ECHO).await;
    let block = block(ECHO_SPEC, ECHO_TESTS, 0);
    let image = Image::new(&machine).expect("an entry");
    let each = vec![("s".to_owned(), parts("s", &block.vars[0].1).expect("parts"))];
    let mut solvers = Solvers::new(10_000);
    for combination in 0..3 {
        solvers.next();
        let result = prove_shape(&image, &block, &each, combination, budget(60), &mut solvers);
        assert!(
            matches!(result, Ok((1, 0, None))),
            "combination {combination}"
        );
    }
    assert!(
        solvers.solver.is_none(),
        "no question, so no solver started"
    );
    assert_eq!(
        prove(&machine, &block, budget(60)),
        Verdict::Proven {
            shapes: 3,
            paths: 3,
            questions: 0,
            alone: None
        }
    );
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

/// The CSV total Claude Code wrote: its first block has 24,696 input combinations, more than
/// the old cap of 20,000, which each started their own solvers.
#[tokio::test]
#[ignore = "takes a minute or more"]
async fn proves_the_csv_totals_24696_combinations() {
    let machine = machine(&fixture("total", "main.s")).await;
    let block = block(
        &fixture("total", "spec.md"),
        &fixture("total", "tests.toml"),
        0,
    );
    let began = Instant::now();
    let verdict = prove(
        &machine,
        &block,
        Budget {
            max_shapes: 2_000_000,
            max_paths: 256,
            ..budget(600)
        },
    );
    eprintln!("{verdict:?} in {:?}", began.elapsed());
    assert!(
        matches!(verdict, Verdict::Proven { shapes: 24_696, .. }),
        "{verdict:?}"
    );
}

/// Where the CSV total's proof spends its time: a few combinations, one at a time, with
/// `LENGUA_PROVE_TRACE=1` for each one's questions.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn traces_a_few_csv_total_combinations() {
    let machine = machine(&fixture("total", "main.s")).await;
    let block = block(
        &fixture("total", "spec.md"),
        &fixture("total", "tests.toml"),
        0,
    );
    let image = Image::new(&machine).expect("an entry");
    let random = block.random_only();
    let each: Vec<(String, Vec<Part>)> = block
        .vars
        .iter()
        .map(|(name, generator)| {
            assert!(!random.contains_key(name));
            (name.clone(), parts(name, generator).expect("parts"))
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
            budget(120),
            &mut solvers,
        );
        let questions = solvers.solver.as_ref().map_or(0, |solver| solver.questions) - before;
        let outcome = match result {
            Ok((paths, _, _)) => format!("{paths} paths"),
            Err(Stop::Unproven(reason)) => reason,
            Err(Stop::Counterexample(values)) => format!("counterexample {values:?}"),
        };
        times.push((began.elapsed(), combination, questions, outcome));
    }
    times.sort_by_key(|(time, ..)| std::cmp::Reverse(*time));
    eprintln!("100 combinations in {:?}; the slowest:", all.elapsed());
    for (time, combination, questions, outcome) in times.iter().take(8) {
        eprintln!("  {combination}: {time:?}, {questions} questions, {outcome}");
    }
}

/// The day of the week from the clock: a second 64-bit division of the first's quotient, which
/// the term rules make the formula's own division, so the proof needs no solver to compare them.
#[tokio::test]
async fn proves_the_day_of_the_week_with_two_divisions() {
    const SPEC: &str = "---
type: lengua/program-spec
title: weekday
---
# Purpose
Exits with the day of the week of the clock's seconds since 1970, from `0` (Sunday) to `6`.

# Behaviors
## B1: Exits with the day of the week
- Given the clock at `1735689600`
- When the program runs
- Then it exits with code `3`

# Constraints
- Allowed system calls: clock_gettime, exit
";
    const TESTS: &str = r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
clock = ["1735689600"]
exit_code = 3

[[generated]]
id = "B1"
vars = { t = "int(0, 4102444800)" }
clock = ["{t}"]
exit_code = "{(t / 86400 + 4) % 7}"
"#;
    let program = |shift: u32| {
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
    };
    let block = block(SPEC, TESTS, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(4)).await;
    agrees(&right, &samples).expect("the model agrees with the samples");
    let verdict = prove(&right, &block, budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(3)).await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// The hh:mm:ss clock Claude Code wrote: six divisions by constants, and output padded by `if`.
/// It timed out in the solvers before the division rules.
#[tokio::test]
async fn proves_the_hh_mm_ss_clock() {
    let block = block(
        &fixture("clock", "spec.md"),
        &fixture("clock", "tests.toml"),
        0,
    );
    let source = fixture("clock", "main.s");
    let right = machine(&source).await;
    let verdict = prove(&right, &block, budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    // An hour one second short.
    let wrong =
        machine(&source.replace(".equ SECONDS_PER_HOUR, 3600", ".equ SECONDS_PER_HOUR, 3599"))
            .await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}
