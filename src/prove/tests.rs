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
        listens: false,
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
    agrees(&right, &samples, false).expect("the model agrees with the samples");
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
    agrees(&right, &samples, false).expect("the model agrees with the samples");
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
            alone: None,
            bound: None,
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
            Err(Stop::Unproven(reason) | Stop::Beyond(reason)) => reason,
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
    agrees(&right, &samples, false).expect("the model agrees with the samples");
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

/// A text formula proven against a program: standard input capitalized byte by byte without
/// branches, against `upper(text)`, for every text up to 16 characters.
#[tokio::test]
async fn proves_a_text_formula() {
    const SPEC: &str = "---
type: lengua/program-spec
title: shout
---
# Purpose
Capitalizes standard input of `0` to `16` characters.

# Behaviors
## B1: Capitalizes its input
- Given standard input holding `ab`
- When the program runs
- Then it prints `AB`
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
";
    const TESTS: &str = r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
stdin = "ab"
stdout = "AB"
exit_code = 0

[[generated]]
id = "B1"
vars = { text = 'string(0, 16, "aAzZ09 b{`")' }
stdin = "{text}"
stdout = "{upper(text)}"
exit_code = 0
"#;
    let program = |limit: u32| {
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
    };
    let block = block(SPEC, TESTS, 0);
    let right = machine(&program(26)).await;
    let verdict = prove(&right, &block, budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { shapes: 17, .. }),
        "{verdict:?}"
    );
    // Leaves `z` small.
    let wrong = machine(&program(25)).await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Bit instructions against a text formula: a mask of the bytes that are `a`, built without
/// branches by `shlx`, counted by `popcnt`, against `count(text, "a")`.
#[tokio::test]
async fn proves_a_count_made_of_bit_instructions() {
    const SPEC: &str = "---
type: lengua/program-spec
title: count a
---
# Purpose
Counts the `a` bytes in standard input of `0` to `16` characters.

# Behaviors
## B1: Counts them
- Given standard input holding `aba`
- When the program runs
- Then it prints `2` and a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
";
    const TESTS: &str = r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
stdin = "aba"
stdout = "2\n"
exit_code = 0

[[generated]]
id = "B1"
vars = { text = 'string(0, 16, "ab")' }
stdin = "{text}"
stdout = "{count(text, \"a\")}\n"
exit_code = 0
"#;
    let program = |byte: u32| {
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
    };
    let block = block(SPEC, TESTS, 0);
    let right = machine(&program(97)).await;
    let verdict = prove(&right, &block, budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { shapes: 17, .. }),
        "{verdict:?}"
    );
    // Counts the `b` bytes instead.
    let wrong = machine(&program(98)).await;
    let verdict = prove(&wrong, &block, budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// An echo of standard input, with `extra` run first once the input is at least 10 bytes.
fn echo(extra: &str) -> String {
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

fn echo_block(stdout: &str, example: &str) -> Generated {
    const SPEC: &str = "---
type: lengua/program-spec
title: echo
---
# Purpose
Prints standard input of `0` to `5000` characters back.

# Behaviors
## B1: Prints it back
- Given standard input holding `ab`
- When the program runs
- Then it prints `ab`
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
";
    let tests = format!(
        r#"[meta]
spec = "sha256:0"

[[example]]
id = "B1"
stdin = "{example}"
stdout = "{example}"
exit_code = 0

[[generated]]
id = "B1"
vars = {{ text = 'string(0, 5000, "ab")' }}
stdin = "{stdout}"
stdout = "{stdout}"
exit_code = 0
"#
    );
    block(SPEC, &tests, 0)
}

fn long_budget() -> Budget {
    Budget {
        max_shapes: 100_000,
        ..budget(240)
    }
}

/// Text longer than the proof covers: every length up to the bound is proven, and the verdict
/// says where it stops and why.
#[tokio::test]
async fn proves_long_text_up_to_the_bound() {
    let machine = machine(&echo("")).await;
    let verdict = prove(&machine, &echo_block("{text}", "ab"), long_budget());
    let Verdict::Proven { shapes, bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert_eq!(shapes, TEXT_BOUND + 1);
    assert_eq!(
        bound,
        Some(Bound {
            bytes: TEXT_BOUND,
            why: PAST_THE_BOUND.into()
        })
    );
    // With a newline after the text, the longest text no longer fits one pipe write.
    let verdict = prove(&machine, &echo_block("{text}\\n", "ab\\n"), long_budget());
    let Verdict::Proven { bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    let bound = bound.expect("a bound");
    assert_eq!(bound.bytes, TEXT_BOUND - 1);
    assert!(bound.why.contains("one pipe write"), "{}", bound.why);
}

/// Where the model stops following the program at some length, the proof holds below it.
#[tokio::test]
async fn proves_text_up_to_where_the_model_stops() {
    let machine = machine(&echo("    xor eax, eax\n    cpuid")).await;
    let verdict = prove(&machine, &echo_block("{text}", "ab"), long_budget());
    let Verdict::Proven { shapes, bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert_eq!(shapes, 10);
    let bound = bound.expect("a bound");
    assert_eq!(bound.bytes, 9);
    assert!(bound.why.contains("cpuid"), "{}", bound.why);
}

/// Prints each generated block's verdict for a program's three files, for exploring.
#[tokio::test]
#[ignore = "explores a program's proofs"]
async fn explores_a_program() {
    let folder = std::env::var("LENGUA_EXPLORE").expect("LENGUA_EXPLORE names a folder");
    let read = |name: &str| {
        std::fs::read_to_string(std::path::Path::new(&folder).join(name)).expect("a file")
    };
    let machine = machine(&read("main.s")).await;
    let (spec, tests) = (read("spec.md"), read("tests.toml"));
    let spec_parsed = crate::spec::parse(&spec).expect("valid spec");
    let label = crate::hash::Sha256Hex::of(b"").label();
    let line = tests
        .lines()
        .find(|line| line.starts_with("spec = "))
        .expect("a label")
        .to_owned();
    let tests = tests.replacen(&line, &format!("spec = \"{label}\""), 1);
    let suite = crate::testfile::check(&tests, &spec_parsed, &label).expect("valid tests");
    for block in &suite.generated {
        let began = Instant::now();
        let verdict = prove(
            &machine,
            block,
            Budget {
                max_shapes: 2_000_000,
                listens: spec_parsed.network.listens.is_some(),
                ..budget(120)
            },
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

/// Vector code, SSE2: the vector template capitalizes every line it's given, proven with no
/// solver question; its too-long lines are proven up to one pipe write; and capitals that stop
/// at `y` are found.
#[tokio::test]
async fn proves_the_vector_template() {
    let (spec, tests) = (
        asset("vector-template-spec.md"),
        asset("vector-template-tests.toml"),
    );
    let source = asset("vector-template.s");
    let right = machine(&source).await;
    let shout = block(&spec, &tests, 0);
    let verdict = prove(&right, &shout, budget(60));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                questions: 0,
                ..
            }
        ),
        "{verdict:?}"
    );
    let too_long = block(&spec, &tests, 1);
    let verdict = prove(&right, &too_long, text_budget());
    let Verdict::Proven {
        bound: Some(bound), ..
    } = &verdict
    else {
        panic!("{verdict:?}");
    };
    assert_eq!(bound.bytes, TEXT_BOUND - 1);
    let wrong =
        machine(&source.replace(".equ ABOVE_Z_X4, 0x7b7b7b7b", ".equ ABOVE_Z_X4, 0x7a7a7a7a"))
            .await;
    let verdict = prove(&wrong, &shout, budget(60));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Vector code, AVX-512 with masks: the fast word count's lines and words, its popcounts of
/// masks meeting the formulas' counts as one term; a count that misses carriage returns is
/// found. Texts up to 300 bytes keep the test quick; up to 4096 prove the same way.
#[tokio::test]
async fn proves_the_avx512_word_count() {
    if !crate::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    let folder = "plugin/skills/write-assembly/assets/fast/wc/";
    let spec = repository_file(&format!("{folder}spec.md")).replace(
        "Tests give it up to `1048576` bytes.",
        "Tests give it up to `300` bytes.",
    );
    let tests = repository_file(&format!("{folder}tests.toml"))
        .replace("string(0, 1048576,", "string(0, 300,");
    let source = repository_file(&format!("{folder}main.s"));
    let block = block(&spec, &tests, 0);
    let right = machine(&source).await;
    let verdict = prove(&right, &block, text_budget());
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                questions: 0,
                ..
            }
        ),
        "{verdict:?}"
    );
    let wrong = machine(&source.replace(".equ TAB_TO_CR, 4", ".equ TAB_TO_CR, 3")).await;
    let verdict = prove(&wrong, &block, text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Vector code on files: upcase, which Claude Code wrote through lengua with SSE2's signed
/// compare trick, capitalizes any file of text; a bound that leaves `z` small is found.
#[tokio::test]
async fn proves_upcase_on_files() {
    let spec = repository_file("examples/upcase/spec.md").replacen(
        "# Behaviors",
        "Tests also give it files of `0` to `300` bytes.\n\n# Behaviors",
        1,
    );
    let tests = repository_file("examples/upcase/tests.toml").replacen(
        "string(0, 1048576,",
        "string(0, 300,",
        1,
    );
    let source = repository_file("examples/upcase/main.s");
    let block = block(&spec, &tests, 0);
    let right = machine(&source).await;
    let verdict = prove(&right, &block, text_budget());
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                questions: 0,
                ..
            }
        ),
        "{verdict:?}"
    );
    let wrong = machine(&source.replace("mov eax, 0x9a9a9a9a", "mov eax, 0x99999999")).await;
    let verdict = prove(&wrong, &block, text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
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
        let mut levels = Levels::new(&each);
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

/// A server on the network: the line-echo server answers every client's line, delivered whole
/// or cut as the network may cut it; one that answers after its first read, or never closes a
/// connection and so waits on a client that waits for it, is found.
#[tokio::test]
async fn proves_the_echo_server() {
    let spec = repository_file("examples/echo/spec.md");
    let tests = repository_file("examples/echo/tests.toml");
    let source = repository_file("examples/echo/main.s");
    let block = block(&spec, &tests, 0);
    let right = machine(&source).await;
    let verdict = prove(&right, &block, network_budget(true));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                questions: 0,
                ..
            }
        ),
        "{verdict:?}"
    );
    let once = source.replacen(
        "    cmp eax, NEWLINE\n    je .Lecho_line_send",
        "    cmp eax, NEWLINE\n    jmp .Lecho_line_send",
        1,
    );
    assert_ne!(once, source);
    let verdict = prove(&machine(&once).await, &block, network_budget(true));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("{verdict:?}");
    };
    assert_ne!(
        found.get(DELIVERY),
        Some(&Value::Str(Delivery::Whole.describe().into())),
        "{found:?}"
    );
    let open = source.replacen(
        "    mov eax, SYS_CLOSE\n    mov edi, ebx\n    syscall\n",
        "    xor eax, eax\n",
        1,
    );
    assert_ne!(open, source);
    let verdict = prove(&machine(&open).await, &block, network_budget(true));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// A client on the network: fetch's argument errors are proven for every port, and its fetch for
/// every path and body, port and delivery; a request with the wrong version, or one cut short
/// before reading the reply, is found.
#[tokio::test]
async fn proves_the_fetch_client() {
    let spec = repository_file("examples/fetch/spec.md").replacen(
        "path holds `1` to `100` characters",
        "path holds `1` to `100` characters (tests: `6` and `11`)",
        1,
    );
    let tests = repository_file("examples/fetch/tests.toml")
        .replacen("path = 'string(1, 100, ", "path = 'string(1, 6, ", 1)
        .replacen("body = 'string(0, 65000, ", "body = 'string(0, 11, ", 1);
    let source = repository_file("examples/fetch/main.s");
    let right = machine(&source).await;
    for index in 0..3 {
        let verdict = prove(&right, &block(&spec, &tests, index), network_budget(false));
        assert!(
            matches!(
                verdict,
                Verdict::Proven {
                    bound: None,
                    questions: 0,
                    ..
                }
            ),
            "block {index}: {verdict:?}"
        );
    }
    let fetch = block(&spec, &tests, 0);
    for wrong in [
        source.replacen(
            ".ascii \" HTTP/1.0\\r\\n\\r\\n\"",
            ".ascii \" HTTP/1.1\\r\\n\\r\\n\"",
            1,
        ),
        source.replacen(".equ VERSION_LEN, 13", ".equ VERSION_LEN, 11", 1),
    ] {
        assert_ne!(wrong, source);
        let verdict = prove(&machine(&wrong).await, &fetch, network_budget(false));
        assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    }
}

/// Output padded to a column by a chain of if(…)s choosing among texts of different lengths, as
/// the AVX-512 word count Claude Code wrote in interactive round 2 prints its counts.
#[tokio::test]
async fn proves_counts_padded_to_a_column() {
    if !crate::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    // Texts up to 120 bytes, so counts of one, two and three digits each get their padding.
    let spec = fixture("padded", "spec.md").replacen(
        "of up to `1048576` bytes",
        "of up to `1048576` bytes (tests: `120`)",
        1,
    );
    let tests = fixture("padded", "tests.toml").replacen("string(0, 1048576,", "string(0, 120,", 1);
    let machine = machine(&fixture("padded", "main.s")).await;
    let verdict = prove(&machine, &block(&spec, &tests, 0), text_budget());
    assert!(
        matches!(verdict, Verdict::Proven { bound: None, .. }),
        "{verdict:?}"
    );
}
