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
    toolchain
        .link(Profile::LinuxX86_64, &object, &executable)
        .await
        .expect("link");
    let mut machine =
        crate::checker::decode::read(&std::fs::read(&executable).expect("read")).expect("decode");
    // Which symbol each [rip + name] names, and each checked jump table's cases, as the server
    // passes them on.
    let (parsed, _) = crate::checker::check_layout(source);
    let features = crate::platform::Machine::detect().features;
    if let Some(program) = crate::checker::decode::map(&parsed, &machine, features, &mut Vec::new())
    {
        machine.named = program.named_symbols();
        let jumps = crate::checker::tables::every(&program, &mut Vec::new());
        machine.tables = crate::checker::tables::addresses(&program, &jumps);
    }
    machine
}

fn block(spec: &str, index: usize) -> Generated {
    let spec = crate::spec::parse(spec).unwrap_or_else(|problems| panic!("{problems:?}"));
    let suite = crate::testfile::check(&spec).unwrap_or_else(|problems| panic!("{problems:?}"));
    suite.generated[index].clone()
}

fn budget(seconds: u64) -> Budget {
    Budget {
        max_paths: 64,
        max_shapes: 2000,
        question_ms: 10_000,
        ..Budget::until(Instant::now() + Duration::from_secs(seconds), false)
    }
}

fn asset(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets")
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
    let block = block(&asset("template-spec.md"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(120));
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
    let block = block(&example("add", "spec.md"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(300));
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
    let block = block(&asset("template-spec.md"), 0);
    // Seconds alone; minutes when every test runs at once, with each question slower too.
    let verdict = prove(
        &machine,
        &block,
        &Budget {
            question_ms: 60_000,
            ..budget(600)
        },
    );
    assert_eq!(
        verdict,
        Verdict::Counterexample(BTreeMap::from([("n".to_owned(), Value::Int(1_234_567))]))
    );
}

/// Memory bugs the checker's rules can't see, planted in the template: each run of the model
/// holds every access to its object, and the proof reports what broke.
#[tokio::test]
async fn finds_memory_bugs_the_rules_cannot_see() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    // spare sits right after out_buf, so a byte past out_buf's end is spare's first.
    let spare = (
        ".size out_buf, . - out_buf\n",
        ".size out_buf, . - out_buf\n\n.type spare, @object\nspare:\n    .zero 8\n.size spare, . \
         - spare\n",
    );
    let cases = [
        // Digits written from 8 bytes into out_buf run past its end.
        (
            vec![(
                "    lea rsi, [rip + out_buf]\n    mov ecx, OUT_BUF_LEN - 1\n",
                "    lea rsi, [rip + out_buf + 8]\n    mov ecx, OUT_BUF_LEN - 1\n",
            )],
            "writes 1 byte at byte 27 of out_buf, which has 21 bytes",
        ),
        // write_all is lent write_i64's frame, which nothing wrote.
        (
            vec![(
                "    mov edi, STDOUT\n    call write_all\n",
                "    mov edi, STDOUT\n    mov rsi, rsp\n    mov edx, 8\n    call write_all\n",
            )],
            "reads byte 0 of write_i64's frame, which has 8 bytes, which nothing wrote",
        ),
        // A pointer just past write_i64's frame writes its return address.
        (
            vec![(
                ".Lwrite_i64_return:\n    add rsp, 8\n",
                ".Lwrite_i64_return:\n    lea rcx, [rsp + 8]\n    mov byte ptr [rcx], 0\n    \
                 add rsp, 8\n",
            )],
            "so it reaches the return address of write_i64",
        ),
        // A loop that clears one byte too many: its pointer, one step past out_buf, writes
        // spare's first byte, which nothing prints.
        (
            vec![
                spare,
                (
                    ".Lwrite_i64_digits:\n",
                    ".Lwrite_i64_digits:\n    lea rsi, [rip + out_buf]\n    mov ecx, OUT_BUF_LEN + \
                     1\n.Lwrite_i64_clear:  # bound: 22\n    mov byte ptr [rsi], 0\n    inc rsi\n    \
                     dec ecx\n    jnz .Lwrite_i64_clear\n",
                ),
            ],
            "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches byte 0 of \
             spare, which has 8 bytes",
        ),
        // A pointer to out_buf's end, as main.s names it, is out_buf's, not spare's.
        (
            vec![
                spare,
                (
                    ".Lwrite_i64_digits:\n",
                    ".Lwrite_i64_digits:\n    lea rsi, [rip + out_buf + OUT_BUF_LEN]\n    mov byte \
                     ptr [rsi], 0\n",
                ),
            ],
            "writes 1 byte at byte 21 of out_buf, which has 21 bytes, so it reaches byte 0 of \
             spare",
        ),
        // A pointer stepped past the program's path, the first argument string, reads the
        // next.
        (
            vec![(
                "    mov rdi, qword ptr [rsp + 16]\n",
                "    mov rdi, qword ptr [rsp + 8]\n    add rdi, 13\n    movzx ecx, byte ptr \
                 [rdi]\n    mov rdi, qword ptr [rsp + 16]\n",
            )],
            "reads 1 byte at byte 13 of the program's argument 0, which has 13 bytes",
        ),
    ];
    for (changes, needle) in cases {
        let mut buggy = crate::checker::TEMPLATE.to_owned();
        for (from, to) in changes {
            assert!(buggy.contains(from), "{from:?}");
            buggy = buggy.replacen(from, to, 1);
        }
        let machine = machine(&buggy).await;
        let block = block(&asset("template-spec.md"), 0);
        let verdict = prove(&machine, &block, &budget(120));
        let Verdict::Counterexample(values) = &verdict else {
            panic!("{needle:?}: {verdict:?}");
        };
        let Some(Value::Str(problem)) = values.get(UNSAFE) else {
            panic!("{needle:?}: {values:?}");
        };
        eprintln!("{problem}");
        assert!(problem.contains(needle), "{problem}");
    }
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
    let block = block(&example("absdiff", "spec.md"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(300));
    eprintln!("absdiff: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// The mean of two prices in cents, rounded half away from zero: decimals in, decimals out. The
/// parse and the input are one sum, but the formula halves at 128 bits and the program at 64, so
/// the solvers still compare the digits.
#[tokio::test]
#[ignore = "slow: a minute or more"]
async fn proves_the_mean_of_two_prices() {
    let machine = machine(&example("mean", "main.s")).await;
    let block = block(&example("mean", "spec.md"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(300));
    eprintln!("mean: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// A file name the input chooses: archive opens the file its argument names, which no test file
/// is, so the run where it's none of them is the only one, and the name it prints is the
/// input's own.
#[tokio::test]
async fn proves_a_file_name_the_input_chooses() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("archive", "main.s")).await;
    let block = block(&example("archive", "spec.md"), 2);
    let verdict = prove(&machine, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// The mean of two whole numbers: the program halves a sum of cents with one away from zero
/// added, and the formula multiplies the sum by 50. A sum every part of which the divisor
/// divides divides part by part, so the two meet without the solvers dividing.
#[tokio::test]
async fn proves_the_mean_of_whole_numbers() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("mean", "main.s")).await;
    let block = block(&example("mean", "spec.md"), 1);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(240));
    eprintln!("mean B2: {verdict:?} in {:?}", started.elapsed());
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
        let block = block(&example("mean", "spec.md"), index);
        let verdict = prove(&machine, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{}: {verdict:?}",
            block.id
        );
    }
}

/// The hypotenuse of two whole sides, every pair the table draws: the program and the formula
/// compute the same doubles step by step, so each pair runs to its one output.
#[tokio::test]
async fn proves_the_hypotenuse_of_whole_sides() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("hypotenuse", "main.s")).await;
    let block = block(&example("hypotenuse", "spec.md"), 0);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(120));
    eprintln!("hypotenuse B1: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// The hypotenuse of decimal sides over their whole range: the program divides each side's
/// thousandths by 1000, squares, adds, takes the square root and scales back, the formula's
/// own steps, so the two outputs are one float computed alike.
#[tokio::test]
async fn proves_the_hypotenuse_of_decimal_sides() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let machine = machine(&example("hypotenuse", "main.s")).await;
    let block = block(&example("hypotenuse", "spec.md"), 1);
    let started = Instant::now();
    let verdict = prove(&machine, &block, &budget(240));
    eprintln!("hypotenuse B2: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
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
    const SPEC: &str = r#"---
type: lengua/program-spec
title: roll
---
# Purpose
Rolls a die for a player numbered from `1` to `9`.

# Behaviors
## B1: Rolls a die
- Given the argument `1`
- When the program runs
- Then it prints text matching `rolled {any(1, 6)}` followed by a newline
- And it exits with code `0`

### For any `player`
- Given the argument `{player}`
- When the program runs
- Then it prints `rolled {any(1, 6)}` followed by a newline
- And it exits with code `0`

| player |
|---|
| `int(1, 9)` |

# Constraints
- Uses: random bytes
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
    let block = block(SPEC, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(6)).await;
    let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
    assert!(found.is_none(), "{found:?}");
    let verdict = prove(&right, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(7)).await;
    let verdict = prove(&wrong, &block, &budget(120));
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
    const SPEC: &str = r#"---
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
    let block = block(SPEC, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(7)).await;
    let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
    assert!(found.is_none(), "{found:?}");
    let verdict = prove(&right, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(6)).await;
    let verdict = prove(&wrong, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

const ECHO_SPEC: &str = r#"---
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
    let block = block(ECHO_SPEC, 0);
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
            "combination {combination}"
        );
    }
    assert!(
        solvers.solver.is_none(),
        "no question, so no solver started"
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
/// the old cap of 20,000, which each started their own solvers; the terms now settle every one.
#[tokio::test]
async fn proves_the_csv_totals_24696_combinations() {
    let machine = machine(&fixture("total", "main.s")).await;
    let block = block(&fixture("total", "spec.md"), 0);
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
    eprintln!("{verdict:?} in {:?}", began.elapsed());
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                shapes: 24_696,
                questions: 0,
                ..
            }
        ),
        "{verdict:?}"
    );
}

/// Where the CSV total's proof spends its time: a few combinations, one at a time, with
/// `LENGUA_PROVE_TRACE=1` for each one's questions.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn traces_a_few_csv_total_combinations() {
    let machine = machine(&fixture("total", "main.s")).await;
    let block = block(&fixture("total", "spec.md"), 0);
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
    eprintln!("100 combinations in {:?}; the slowest:", all.elapsed());
    for (time, combination, questions, outcome) in times.iter().take(8) {
        eprintln!("  {combination}: {time:?}, {questions} questions, {outcome}");
    }
}

/// calc with its operator any of the three, one symbolic byte, the numbers fixed, so the run
/// forks at the jump table into every case. (A `choice` would run each value on its own.)
const CHOSEN_OPERATOR: &str = r#"---
type: lengua/program-spec
title: calc
description: Combines 6 and 3 with the operator it's given.
---
# Purpose
Prints 6 and 3 combined by the operator it's given, `1` character: their sum, difference or
product.

# Behaviors
## B1: Applies the operator
- Given the arguments `6`, `+` and `3`
- When the program runs
- Then it prints `9` followed by a newline
- And it exits with code `0`

### For any `op`
- Given the arguments `6`, `{op}` and `3`
- When the program runs
- Then it prints `{if(op == "+", 9, if(op == "-", 3, 18))}` followed by a newline
- And it exits with code `0`

| op | runs |
|---|---|
| `string(1, 1, "+-*")` | 10 |

# Constraints
- Uses: nothing
"#;

/// A checked jump table (dialect 4): the proof follows every case the input can reach through
/// it, and only those, and finds a table whose entries send an operator to the wrong case.
#[tokio::test]
async fn proves_each_case_a_jump_table_can_take() {
    if !solvers_installed() {
        return;
    }
    let calc = fixture("calc", "main.s");
    let right = machine(&calc).await;
    assert_eq!(right.tables.len(), 1, "{:?}", right.tables);
    // Each of the three operators, a path through each case its byte can pick.
    let verdict = prove(&right, &block(CHOSEN_OPERATOR, 0), &budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { paths, .. } if paths >= 3),
        "{verdict:?}"
    );
    // B6: an operator that isn't one, where of the characters only `,` passes the bounds check,
    // to the table's entry for it.
    let verdict = prove(&right, &block(&fixture("calc", "spec.md"), 4), &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
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
    assert_ne!(swapped, calc, "the entries were swapped");
    let wrong = machine(&swapped).await;
    let verdict = prove(&wrong, &block(CHOSEN_OPERATOR, 0), &budget(120));
    let Verdict::Counterexample(values) = &verdict else {
        panic!("{verdict:?}");
    };
    assert!(
        matches!(values.get("op"), Some(Value::Str(op)) if op == "+" || op == "-"),
        "{values:?}"
    );
}

/// A jump table indexed by a byte of the input that only its `cmp` and `jae` bound, so the index
/// can reach past the table and out of the program's memory: the proof reads only the places
/// inside the table and asks the solvers whether any input reaches further. By `xor` the index
/// can be anything, which the table's log follows; by `and`, a few places past it.
#[tokio::test]
async fn proves_a_table_whose_index_only_its_check_bounds() {
    const SPEC: &str = r#"---
type: lengua/program-spec
title: letters
---
# Purpose
Turns `1` character of standard input into a capital: `h` to `k` become `W` to `Z`, and `l`
prints nothing.

# Behaviors
## B1: Turns a letter into a capital
- Given standard input holding `i`
- When the program runs
- Then it prints `X`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{if(text == "l", "", tr(text, "hijk", "WXYZ"))}`
- And it exits with code `0`

| text |
|---|
| `string(1, 1, "hijkl")` |

# Constraints
- Uses: standard input
"#;
    let program = |index: &str, limit: u32| {
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
    };
    if !solvers_installed() {
        return;
    }
    let block = block(SPEC, 0);
    for index in ["xor eax, 104", "and eax, 7"] {
        let right = machine(&program(index, 4)).await;
        assert_eq!(right.tables.len(), 1, "{:?}", right.tables);
        let verdict = prove(&right, &block, &budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { .. }),
            "{index}: {verdict:?}"
        );
        // A check one too loose lets `l` read past the table.
        let wrong = machine(&program(index, 5)).await;
        let verdict = prove(&wrong, &block, &budget(120));
        let Verdict::Counterexample(values) = &verdict else {
            panic!("{index}: {verdict:?}");
        };
        assert!(
            matches!(values.get(UNSAFE), Some(Value::Str(problem)) if problem.contains("outside cases")),
            "{index}: {values:?}"
        );
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
    std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok()
}

/// The clock Claude Code wrote in interactive round 2, with the days from today to a date: the
/// program and the tests compute Hinnant's days_from_civil in different orders, and their sums
/// come out as one term, so every block is proven, the seven day counts' with no solver.
#[tokio::test]
async fn proves_every_block_of_the_day_count() {
    if !solvers_installed() {
        return;
    }
    let spec = fixture("days", "spec.md");
    let machine = machine(&fixture("days", "main.s")).await;
    for index in 0..11 {
        let block = block(&spec, index);
        let verdict = prove(&machine, &block, &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{} ({index}): {verdict:?}",
            block.id
        );
        if (1..8).contains(&index) {
            assert!(
                matches!(verdict, Verdict::Proven { questions: 0, .. }),
                "{verdict:?}"
            );
        }
    }
}

/// `spec` with one more generated test right after its `index`th, so at `index + 1`: a copy of
/// that row of its table with the generators `cells` gives by column.
fn with_row(spec: &str, index: usize, cells: &[(&str, &str)]) -> String {
    let lines: Vec<&str> = spec.lines().collect();
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
    panic!("the spec has no generated test {index}");
}

/// The clock of interactive round 3, whose day count's block covers valid and invalid dates at
/// once: it prints `if(valid, days, "")`, a number or nothing, and the error the other way, so
/// one output piece is a printed number or a text as its condition says.
#[tokio::test]
async fn proves_a_number_printed_only_where_the_input_is_valid() {
    if !solvers_installed() {
        return;
    }
    let spec = fixture("dates", "spec.md");
    let machine = machine(&fixture("dates", "main.s")).await;
    let verdict = prove(&machine, &block(&spec, 1), &fixture_budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { bound: None, .. }),
        "{verdict:?}"
    );
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

/// Bugs planted in the day count: an epoch a day off, and leap years without the 400-year rule.
#[tokio::test]
async fn finds_bugs_planted_in_the_day_count() {
    if !solvers_installed() {
        return;
    }
    let spec = with_row(&fixture("days", "spec.md"), 1, CENTURY_LEAP_DAYS);
    let source = fixture("days", "main.s");
    let leap_days = block(&spec, 2);
    let verdict = prove(&machine(&source).await, &leap_days, &fixture_budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");

    let shifted = source.replacen(
        ".equ EPOCH_SHIFT, 719468\n",
        ".equ EPOCH_SHIFT, 719469\n",
        1,
    );
    assert_ne!(shifted, source);
    let verdict = prove(
        &machine(&shifted).await,
        &block(&spec, 1),
        &fixture_budget(120),
    );
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");

    let no_400 = source.replacen(
        "    mov rax, rdi\n    xor edx, edx\n    mov ecx, YEARS_PER_ERA\n    div rcx\n    \
         test rdx, rdx\n    jnz .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
        "    jmp .Ldays_from_date_check_day\n.Ldays_from_date_leap:\n",
        1,
    );
    assert_ne!(no_400, source);
    let verdict = prove(&machine(&no_400).await, &leap_days, &fixture_budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
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

/// The CSV total Claude Code wrote in interactive round 2: every amount proven where the text
/// fields are fixed, and a negative amount's cents dropped caught; B2 and B3 proven.
#[tokio::test]
async fn proves_the_csv_total_for_every_amount() {
    if !solvers_installed() {
        return;
    }
    let spec = with_row(&fixture("amounts", "spec.md"), 0, FIXED_FIELDS);
    let source = fixture("amounts", "main.s");
    let machine_of = machine(&source).await;
    for index in [2, 3] {
        let verdict = prove(&machine_of, &block(&spec, index), &fixture_budget(120));
        assert!(
            matches!(verdict, Verdict::Proven { bound: None, .. }),
            "{index}: {verdict:?}"
        );
    }
    let fixed = block(&spec, 1);
    let verdict = prove(&machine_of, &fixed, &fixture_budget(300));
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                shapes: 4096,
                questions: 0,
                bound: None,
                ..
            }
        ),
        "{verdict:?}"
    );

    let dropped = source.replacen(
        "    neg rdx\n.Lparse_return:\n",
        "    mov rax, rdx\n    xor edx, edx\n    mov r8d, 100\n    div r8\n    \
         imul rdx, rax, -100\n    xor eax, eax\n.Lparse_return:\n",
        1,
    );
    assert_ne!(dropped, source);
    let verdict = prove(&machine(&dropped).await, &fixed, &fixture_budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// The CSV total's blocks with free text in several fields: B5 proven for every length its
/// texts have, B4 past the 6 bytes it reached before sums had one form, and B1, which proved
/// nothing then, up to a bound.
#[tokio::test]
#[ignore = "takes two minutes"]
async fn proves_the_csv_totals_free_text_blocks() {
    let spec = fixture("amounts", "spec.md");
    let machine = machine(&fixture("amounts", "main.s")).await;
    let verdict = prove(&machine, &block(&spec, 4), &fixture_budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { bound: None, .. }),
        "B5: {verdict:?}"
    );
    let verdict = prove(&machine, &block(&spec, 3), &fixture_budget(60));
    assert!(
        matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 7),
        "B4: {verdict:?}"
    );
    let verdict = prove(&machine, &block(&spec, 0), &fixture_budget(30));
    assert!(
        matches!(verdict, Verdict::Proven { bound: Some(_), .. }),
        "B1: {verdict:?}"
    );
}

/// The day of the week from the clock: a second 64-bit division of the first's quotient, which
/// the term rules make the formula's own division, so the proof needs no solver to compare them.
#[tokio::test]
async fn proves_the_day_of_the_week_with_two_divisions() {
    const SPEC: &str = r#"---
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
    let block = block(SPEC, 0);
    let samples: Vec<crate::testfile::Case> =
        block.cases(1).take(4).filter_map(Result::ok).collect();
    let right = machine(&program(4)).await;
    let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
    assert!(found.is_none(), "{found:?}");
    let verdict = prove(&right, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    let wrong = machine(&program(3)).await;
    let verdict = prove(&wrong, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// The hh:mm:ss clock Claude Code wrote: six divisions by constants, and output padded by `if`.
/// It timed out in the solvers before the division rules.
#[tokio::test]
async fn proves_the_hh_mm_ss_clock() {
    let block = block(&fixture("clock", "spec.md"), 0);
    let source = fixture("clock", "main.s");
    let right = machine(&source).await;
    let verdict = prove(&right, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
    // An hour one second short.
    let wrong =
        machine(&source.replace(".equ SECONDS_PER_HOUR, 3600", ".equ SECONDS_PER_HOUR, 3599"))
            .await;
    let verdict = prove(&wrong, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// A text formula proven against a program: standard input capitalized byte by byte without
/// branches, against `upper(text)`, for every text up to 16 characters.
#[tokio::test]
async fn proves_a_text_formula() {
    const SPEC: &str = r#"---
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

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `` string(0, 16, "aAzZ09 b{`") `` |

# Constraints
- Uses: standard input
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
    let block = block(SPEC, 0);
    let right = machine(&program(26)).await;
    let verdict = prove(&right, &block, &budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { shapes: 17, .. }),
        "{verdict:?}"
    );
    // Leaves `z` small.
    let wrong = machine(&program(25)).await;
    let verdict = prove(&wrong, &block, &budget(120));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Bit instructions against a text formula: a mask of the bytes that are `a`, built without
/// branches by `shlx`, counted by `popcnt`, against `count(text, "a")`.
#[tokio::test]
async fn proves_a_count_made_of_bit_instructions() {
    const SPEC: &str = r#"---
type: lengua/program-spec
title: count a
---
# Purpose
Counts the `a` bytes in standard input of `0` to `16` characters.

# Behaviors
## B1: Counts them
- Given standard input holding `aba`
- When the program runs
- Then it prints `2` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, "a")}` followed by a newline
- And it exits with code `0`

| text |
|---|
| `string(0, 16, "ab")` |

# Constraints
- Uses: standard input
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
    let block = block(SPEC, 0);
    let right = machine(&program(97)).await;
    let verdict = prove(&right, &block, &budget(120));
    assert!(
        matches!(verdict, Verdict::Proven { shapes: 17, .. }),
        "{verdict:?}"
    );
    // Counts the `b` bytes instead.
    let wrong = machine(&program(98)).await;
    let verdict = prove(&wrong, &block, &budget(120));
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
    let spec = format!(
        r#"---
type: lengua/program-spec
title: echo
---
# Purpose
Prints standard input of `0` to `5000` characters back.

# Behaviors
## B1: Prints it back
- Given standard input holding `{example}`
- When the program runs
- Then it prints `{example}`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{stdout}`
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
    block(&spec, 0)
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
    let machine = machine(&echo("")).await;
    let verdict = prove(&machine, &echo_block("{text}", "ab"), &long_budget());
    let Verdict::Proven { shapes, bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert_eq!(shapes, PIPE_WRITE + 1);
    let bound = bound.expect("a bound");
    assert_eq!((bound.bytes, bound.of), (PIPE_WRITE, 5000));
    assert!(bound.why.contains("one pipe write"), "{}", bound.why);
    // With a newline after the text, the longest text no longer fits one pipe write.
    let verdict = prove(&machine, &echo_block("{text}\\n", "ab\\n"), &long_budget());
    let Verdict::Proven { bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    let bound = bound.expect("a bound");
    assert_eq!(bound.bytes, PIPE_WRITE - 1);
    assert!(bound.why.contains("one pipe write"), "{}", bound.why);
    // A proof that starts where an earlier one stopped proves only from there.
    let resumed = Budget {
        from_length: 4000,
        ..long_budget()
    };
    let verdict = prove(&machine, &echo_block("{text}", "ab"), &resumed);
    let Verdict::Proven { shapes, bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert_eq!(shapes, PIPE_WRITE + 1 - 4000);
    assert_eq!(bound.map(|bound| bound.bytes), Some(PIPE_WRITE));
}

/// Where the model stops following the program at some length, the proof holds below it.
#[tokio::test]
async fn proves_text_up_to_where_the_model_stops() {
    let machine = machine(&echo("    xor eax, eax\n    cpuid")).await;
    let verdict = prove(&machine, &echo_block("{text}", "ab"), &long_budget());
    let Verdict::Proven { shapes, bound, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert_eq!(shapes, 10);
    let bound = bound.expect("a bound");
    assert_eq!(bound.bytes, 9);
    assert!(bound.why.contains("cpuid"), "{}", bound.why);
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
    let spec_parsed = crate::spec::parse(&read("spec.md")).expect("valid spec");
    let suite = crate::testfile::check(&spec_parsed).expect("valid tests");
    for block in &suite.generated {
        let began = Instant::now();
        let verdict = prove(
            &machine,
            block,
            &Budget::until(
                Instant::now() + Duration::from_secs(120),
                spec_parsed.network.listens.is_some(),
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

/// Vector code, SSE2: the vector template capitalizes every line it's given, proven with no
/// solver question; its too-long lines are proven up to one pipe write; and capitals that stop
/// at `y` are found.
#[tokio::test]
async fn proves_the_vector_template() {
    let spec = asset("vector-template-spec.md");
    let source = asset("vector-template.s");
    let right = machine(&source).await;
    let shout = block(&spec, 0);
    let verdict = prove(&right, &shout, &budget(60));
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
    let too_long = block(&spec, 1);
    let verdict = prove(&right, &too_long, &text_budget());
    let Verdict::Proven {
        bound: Some(bound), ..
    } = &verdict
    else {
        panic!("{verdict:?}");
    };
    assert_eq!(bound.bytes, PIPE_WRITE - 1);
    let wrong =
        machine(&source.replace(".equ ABOVE_Z_X4, 0x7b7b7b7b", ".equ ABOVE_Z_X4, 0x7a7a7a7a"))
            .await;
    let verdict = prove(&wrong, &shout, &budget(60));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Induction: the streaming template, which capitalizes its input a buffer-full at a time
/// through fill, is proven for every length its loop's bound allows, far past what any proof
/// by length reaches; capitals that stop at `y` are still found, and so is a loop that drops
/// the bytes of every buffer-full after the first.
#[tokio::test]
async fn proves_a_stream_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let spec = asset("stream-template-spec.md");
    let source = asset("stream-template.s");
    let right = machine(&source).await;
    let shout = block(&spec, 0);
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
        "{verdict:?}"
    );
    let wrong =
        machine(&source.replace(".equ ABOVE_Z_X4, 0x7b7b7b7b", ".equ ABOVE_Z_X4, 0x7a7a7a7a"))
            .await;
    let verdict = prove(&wrong, &shout, &text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    // Only the first buffer-full capitalized: from a later time round's state, the bytes go
    // out as they came in.
    let first_only = machine(
        &source
            .replace(
                "    mov rbx, MAX_FULLS\n",
                "    xor r13d, r13d\n    mov rbx, MAX_FULLS\n",
            )
            .replace(
                "    call shout\n",
                "    test r13, r13\n    jnz .Lstart_shouted\n    call shout\n.Lstart_shouted:\n    \
                 mov r13d, 1\n",
            ),
    )
    .await;
    let verdict = prove(&first_only, &shout, &text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    // A loop bound of four buffer-fulls: proven for three, as far as the induction reaches,
    // and no further.
    let four = machine(&source.replace(".equ MAX_FULLS, 1099511627776", ".equ MAX_FULLS, 4")).await;
    let verdict = prove(&four, &shout, &text_budget());
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
        "{verdict:?}"
    );
}

/// Induction over counts: a line count kept in a register, grown by each buffer-full's
/// newlines, 16 bytes at a time, is proven for every length the spec allows; a count that
/// misses the last chunk of each buffer-full is found.
#[tokio::test]
async fn proves_a_count_for_every_length_by_induction() {
    if !solvers_installed() {
        return;
    }
    let spec = fixture("lines", "spec.md");
    let source = fixture("lines", "main.s");
    let right = machine(&source).await;
    let lines = block(&spec, 0);
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
        "{verdict:?}"
    );
    let short = machine(&source.replace(
        "    cmp rcx, rdi\n    jae .Lcount_lines_return\n",
        "    lea rdx, [rcx + 16]\n    cmp rdx, rdi\n    jae .Lcount_lines_return\n",
    ))
    .await;
    let verdict = prove(&short, &lines, &text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    // A count of the last buffer-full alone: the first time round it looks like the count so
    // far, and from any later state it isn't, so no induction applies.
    let last_only = machine(&source.replace("    add r13, rax\n", "    mov r13, rax\n")).await;
    let image = Image::new(&last_only).expect("an entry");
    let step = induction::step(&image, &lines, &text_budget());
    assert!(step.is_err(), "{step:?}");
}

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
    let spec = fixture("words", "spec.md");
    // A smaller buffer than the fixture's, so the base's lengths go by quickly.
    let source = fixture("words", "main.s").replace(".equ BUF_LEN, 4096\n", ".equ BUF_LEN, 256\n");
    let right = machine(&source).await;
    let words = block(&spec, 0);
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
        "{verdict:?}"
    );
    // Each buffer-full taken as if whitespace came before it, and the word-ending flag lost
    // after each one: only an input longer than a buffer-full shows either, and the step finds
    // the two buffer-fulls that do.
    for (wrong, why) in [
        ("    call fill\n", "    mov r14d, 1\n    call fill\n"),
        (".Lstart_next:\n", ".Lstart_next:\n    xor r14d, r14d\n"),
    ]
    .map(|(from, to)| (source.replacen(from, to, 1), from))
    {
        let wrong = machine(&wrong).await;
        let verdict = prove(&wrong, &words, &text_budget());
        let Verdict::Counterexample(values) = &verdict else {
            panic!("{why}: {verdict:?}");
        };
        let Some(Value::Str(text)) = values.get("text") else {
            panic!("{why}: {values:?}");
        };
        assert!(text.len() > 256, "{why}: {text:?}");
    }
    // A count of the last buffer-full's words alone: no register keeps the words as they go.
    let last_only = machine(&source.replace(
        "    add r13, rdx\n    mov r14, rcx\n    mov rsi, r15\n",
        "    mov r13, rdx\n    mov r14, rcx\n    mov rsi, r15\n",
    ))
    .await;
    let image = Image::new(&last_only).expect("an entry");
    let step = induction::step(&image, &words, &text_budget());
    assert!(step.is_err(), "{step:?}");
}

/// Sorting: sort and uniq over short texts are a choice of texts, one for each way the newlines
/// can fall and the lines compare, so the sort example is proven for every input up to the
/// length where its comparisons branch past the proof's paths, with and without `-u`; a
/// comparison that puts the longer of two lines with the same start first is found.
#[tokio::test]
async fn proves_sorting_for_short_texts() {
    if !solvers_installed() {
        return;
    }
    let spec = repository_file("examples/sort/spec.md");
    let source = repository_file("examples/sort/main.s");
    let right = machine(&source).await;
    // Two minutes each, so a machine busy with other tests' proofs still reaches the lengths.
    let budget = || Budget {
        max_shapes: 100_000,
        max_paths: 256,
        ..budget(120)
    };
    // A mebibyte sample is too long for the model to follow, which says nothing of it: the
    // check before the proof goes on with the shorter ones.
    let samples: Vec<crate::testfile::Case> = block(&spec, 0)
        .cases(1)
        .take(24)
        .filter_map(Result::ok)
        .collect();
    assert!(samples.iter().any(|sample| sample.stdin.len() > 1 << 19));
    let found = agrees(&right, &samples, false).expect("the model agrees with the samples");
    assert!(found.is_none(), "{found:?}");
    for index in [0, 2] {
        let verdict = prove(&right, &block(&spec, index), &budget());
        assert!(
            matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 5),
            "{verdict:?}"
        );
    }
    let longer_first = source.replace(
        ".Lcompare_lengths:\n    mov rax, r8\n    sub rax, r9\n",
        ".Lcompare_lengths:\n    mov rax, r9\n    sub rax, r8\n",
    );
    assert_ne!(longer_first, source);
    let verdict = prove(&machine(&longer_first).await, &block(&spec, 0), &budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

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
    let spec = asset("tally-template-spec.md");
    let source = asset("tally-template.s");
    let right = machine(&source).await;
    let tally = block(&spec, 0);
    let verdict = prove(&right, &tally, &budget(90));
    assert!(
        matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 3),
        "{verdict:?}"
    );
    let mutants = [
        // Every word new: its slot never takes its entry's number.
        (
            "    mov dword ptr [r15 + rbp*4], eax\n",
            "    mov dword ptr [r15 + rbp*4], 0\n",
        ),
        // Ties in reverse byte order.
        (
            "    lea rdi, [rdx + r8]\n    lea rdx, [rdx + r9]\n",
            "    lea rdi, [rdx + r9]\n    lea rdx, [rdx + r8]\n",
        ),
    ];
    // The hash used whole as a slot, which the probe's bounds check stops with its assertion.
    let unmasked = (
        "    and rax, SLOT_MASK\n    mov rdi, r12\n",
        "    mov rdi, r12\n",
    );
    for (from, to) in mutants.into_iter().chain([unmasked]) {
        let wrong = source.replacen(from, to, 1);
        assert_ne!(wrong, source, "{from}");
        let verdict = prove(&machine(&wrong).await, &tally, &budget(60));
        assert!(
            matches!(verdict, Verdict::Counterexample(_)),
            "{from}: {verdict:?}"
        );
    }
}

/// Vector code, AVX-512 with masks: the fast word count's lines and words, its popcounts of
/// masks meeting the formulas' counts as one term; a count that misses carriage returns is
/// found. Texts up to 200 bytes, past three whole vectors and a tail, keep the test quick even
/// beside the others; up to 4096 prove the same way.
/// Threads: each of parallel_for's workers runs to its end as it starts, which M7 makes the same
/// as any order, so a program on several threads proves like any other. Each thread here takes
/// 128 bytes, so a text of 300 runs on three.
#[tokio::test]
async fn proves_a_program_on_several_threads() {
    let folder = "plugin/skills/compile/assets/";
    let spec = repository_file(&format!("{folder}parallel-template-spec.md"))
        .replacen(
            "# Behaviors",
            "Tests here give it up to `300` bytes.\n\n# Behaviors",
            1,
        )
        .replace("string(0, 1048576,", "string(0, 300,");
    // Sixteen chunks of 128 bytes hold at most 2048.
    let source = repository_file(&format!("{folder}parallel-template.s"))
        .replace(".equ CHUNK, 65536", ".equ CHUNK, 128")
        .replace(".equ INPUT_MAX, 1048576", ".equ INPUT_MAX, 2048");
    assert!(source.contains(".equ CHUNK, 128") && source.contains(".equ INPUT_MAX, 2048"));
    let machine = machine(&source).await;
    let verdict = prove(&machine, &block(&spec, 0), &text_budget());
    assert!(
        matches!(
            &verdict,
            Verdict::Proven {
                bound: None,
                unreached,
                ..
            } if unreached.is_empty()
        ),
        "{verdict:?}"
    );
}

/// A program for the terminal proves as from a pipe, as its tests run: the terminal's mode
/// can't be read or set, and a wait for the next key always finds one, or the input's end.
#[tokio::test]
async fn proves_a_program_for_the_terminal() {
    let folder = "plugin/skills/compile/assets/";
    let spec = repository_file(&format!("{folder}terminal-template-spec.md"));
    let machine = machine(&repository_file(&format!("{folder}terminal-template.s"))).await;
    for index in 0..2 {
        let verdict = prove(&machine, &block(&spec, index), &text_budget());
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
}

/// The CRC-32C asset's own spec, every text up to the 4096 bytes a proof covers.
#[tokio::test]
#[ignore = "slow: 4097 lengths of text"]
async fn proves_the_crc32c_checksum_up_to_the_bound() {
    let folder = "plugin/skills/compile/assets/fast/crc32c/";
    let block = block(&repository_file(&format!("{folder}spec.md")), 0);
    let machine = machine(&repository_file(&format!("{folder}main.s"))).await;
    let started = Instant::now();
    let budget = Budget {
        max_shapes: 100_000,
        ..budget(300)
    };
    let verdict = prove(&machine, &block, &budget);
    eprintln!("crc32c: {verdict:?} in {:?}", started.elapsed());
    assert!(matches!(verdict, Verdict::Proven { .. }), "{verdict:?}");
}

/// The CRC-32C asset: `crc32` over 8 bytes at a time, then over single bytes, meets the
/// formula's chain of byte steps, so each text proves without a question to the solvers.
#[tokio::test]
async fn proves_the_crc32c_checksum() {
    let folder = "plugin/skills/compile/assets/fast/crc32c/";
    let spec = repository_file(&format!("{folder}spec.md"))
        .replace("Tests give it up to `1048576`", "Tests give it up to `100`")
        .replace("string(0, 1048576,", "string(0, 100,");
    assert!(spec.contains("string(0, 100,"), "{spec}");
    let source = repository_file(&format!("{folder}main.s"));
    let block = block(&spec, 0);
    let right = machine(&source).await;
    let verdict = prove(&right, &block, &text_budget());
    // Three chains at once start at 12288 bytes, past every text proven: the proof says it
    // never ran them.
    assert!(
        matches!(
            &verdict,
            Verdict::Proven {
                bound: None,
                questions: 0,
                unreached,
                ..
            } if unreached == &["move_front", "crc_block"]
        ),
        "{verdict:?}"
    );
    // One bit of the start wrong puts every checksum off.
    let wrong =
        machine(&source.replace(".equ CRC_START, 0xFFFFFFFF", ".equ CRC_START, 0xFFFFFFFE")).await;
    let verdict = prove(&wrong, &block, &text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

#[tokio::test]
async fn proves_the_avx512_word_count() {
    if !crate::platform::Machine::detect().features.avx512bw {
        eprintln!("this CPU has no AVX-512BW");
        return;
    }
    let folder = "plugin/skills/compile/assets/fast/wc/";
    let spec = repository_file(&format!("{folder}spec.md"))
        .replace(
            "Tests give it up to `1048576` bytes.",
            "Tests give it up to `200` bytes.",
        )
        .replace("string(0, 1048576,", "string(0, 200,");
    assert!(spec.contains("string(0, 200,"), "{spec}");
    let source = repository_file(&format!("{folder}main.s"));
    let block = block(&spec, 0);
    let right = machine(&source).await;
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
        "{verdict:?}"
    );
    let wrong = machine(&source.replace(".equ TAB_TO_CR, 4", ".equ TAB_TO_CR, 3")).await;
    let verdict = prove(&wrong, &block, &text_budget());
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// Vector code on files: upcase, which Claude Code wrote through lengua with SSE2's signed
/// compare trick, capitalizes any file of text; a bound that leaves `z` small is found.
#[tokio::test]
async fn proves_upcase_on_files() {
    let spec = repository_file("examples/upcase/spec.md")
        .replacen(
            "# Behaviors",
            "Tests also give it files of `0` to `300` bytes.\n\n# Behaviors",
            1,
        )
        .replacen("string(0, 1048576,", "string(0, 300,", 1);
    let source = repository_file("examples/upcase/main.s");
    let block = block(&spec, 0);
    let right = machine(&source).await;
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
        "{verdict:?}"
    );
    let wrong = machine(&source.replace("mov eax, 0x9a9a9a9a", "mov eax, 0x99999999")).await;
    let verdict = prove(&wrong, &block, &text_budget());
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

/// A server for clients at once: the event-loop template waits on its listener and every
/// connection with poll. One client is proven for every line in range, in every order its
/// connecting and its pieces can come in between polls; two at once, in every such order, and
/// with the first holding its last byte until the second has its answer, as the test's do, as
/// far as the time goes. Serving the first slot alone, so the second client waits forever, and
/// never closing a connection, so its client waits forever, are found.
#[tokio::test]
async fn proves_a_server_for_clients_at_once() {
    if !solvers_installed() {
        return;
    }
    let spec = asset("server-template-spec.md");
    let source = asset("server-template.s");
    let right = machine(&source).await;
    let (one, two) = (block(&spec, 0), block(&spec, 1));
    let budget = |seconds| Budget {
        max_shapes: 100_000,
        listens: true,
        ..budget(seconds)
    };
    let verdict = prove(&right, &one, &budget(60));
    assert!(
        matches!(verdict, Verdict::Proven { bound: None, .. }),
        "{verdict:?}"
    );
    // The model agrees with the real program on the tests' samples, each client but the last
    // holding its last byte, as a check asks before it proves.
    for block in [&one, &two] {
        let samples: Vec<crate::testfile::Case> =
            block.cases(1).take(24).filter_map(Result::ok).collect();
        let found = agrees(&right, &samples, true).expect("the model agrees with the samples");
        assert!(found.is_none(), "{found:?}");
    }
    let verdict = prove(&right, &two, &budget(20));
    assert!(
        matches!(&verdict, Verdict::Proven { bound: Some(bound), .. } if bound.bytes >= 8),
        "{verdict:?}"
    );
    let first_slot = source.replace(
        "    mov rbx, CLIENTS\n.Lserve_clients_slot:",
        "    mov rbx, 1\n.Lserve_clients_slot:",
    );
    assert_ne!(first_slot, source);
    let verdict = prove(&machine(&first_slot).await, &two, &budget(60));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    let open = source.replace(
        "    mov eax, SYS_CLOSE\n    mov edi, ebx\n    syscall\n",
        "    xor eax, eax\n",
    );
    assert_ne!(open, source);
    let verdict = prove(&machine(&open).await, &one, &budget(60));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// A server on the network: the line-echo server reads its line with fill, so it answers every
/// client's line however the network cuts it; one that reads once in place of fill, or never
/// closes a connection and so waits on a client that waits for it, is found.
/// A buffer lent to the kernel must hold every byte the call may touch, however little arrives:
/// a raw read, and fill's reads.
#[tokio::test]
async fn buffers_lent_to_the_kernel_hold_every_byte_it_may_touch() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let read = echo("").replacen("    mov edx, 8192\n", "    mov edx, 8200\n", 1);
    let verdict = prove(
        &machine(&read).await,
        &echo_block("{text}", "ab"),
        &budget(60),
    );
    let Verdict::Counterexample(found) = &verdict else {
        panic!("{verdict:?}");
    };
    let Some(Value::Str(problem)) = found.get(UNSAFE) else {
        panic!("{found:?}");
    };
    assert!(
        problem.starts_with(
            "in _start, the read system call writes 8200 bytes at byte 0 of buffer, which has \
             8192 bytes, so it reaches"
        ),
        "{problem}"
    );
    let spec = repository_file("examples/echo/spec.md");
    let source = repository_file("examples/echo/main.s");
    let fill = source.replacen(
        "    mov edx, LINE_MAX\n    mov ecx, NEWLINE\n    call fill\n",
        "    mov edx, LINE_MAX + 8\n    mov ecx, NEWLINE\n    call fill\n",
        1,
    );
    assert_ne!(fill, source);
    let verdict = prove(
        &machine(&fill).await,
        &block(&spec, 0),
        &network_budget(true),
    );
    let Verdict::Counterexample(found) = &verdict else {
        panic!("{verdict:?}");
    };
    let Some(Value::Str(problem)) = found.get(UNSAFE) else {
        panic!("{found:?}");
    };
    assert!(
        problem.starts_with("in fill, the read system call writes") && problem.contains("of line"),
        "{problem}"
    );
}

#[tokio::test]
async fn proves_the_echo_server() {
    let spec = repository_file("examples/echo/spec.md");
    let source = repository_file("examples/echo/main.s");
    let block = block(&spec, 0);
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
        "{verdict:?}"
    );
    // One read in place of fill: a line cut short by the network is found.
    let once = source.replacen(
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov eax, SYS_READ\n    mov edi, ebx\n    syscall\n",
        1,
    );
    assert_ne!(once, source);
    let verdict = prove(&machine(&once).await, &block, &network_budget(true));
    let Verdict::Counterexample(found) = &verdict else {
        panic!("{verdict:?}");
    };
    assert_ne!(
        found.get(DELIVERY),
        Some(&Value::Str(Delivery::Whole.describe())),
        "{found:?}"
    );
    // Reading to the end of the request: the client, as curl does, keeps its side open while
    // it waits for the answer, so the server waits forever.
    let to_the_end = source.replacen(
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov ecx, -1\n    call fill\n",
        1,
    );
    assert_ne!(to_the_end, source);
    let verdict = prove(&machine(&to_the_end).await, &block, &network_budget(true));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
    let open = source.replacen(
        "    mov eax, SYS_CLOSE\n    mov edi, ebx\n    syscall\n",
        "    xor eax, eax\n",
        1,
    );
    assert_ne!(open, source);
    let verdict = prove(&machine(&open).await, &block, &network_budget(true));
    assert!(matches!(verdict, Verdict::Counterexample(_)), "{verdict:?}");
}

/// A client on the network: fetch's argument errors are proven for every port, and its fetch for
/// every path and body, port and delivery; a request with the wrong version, or one cut short
/// before reading the reply, is found.
#[tokio::test]
async fn proves_the_fetch_client() {
    let spec = repository_file("examples/fetch/spec.md")
        .replacen(
            "path holds `1` to `100` characters",
            "path holds `1` to `100` characters (tests: `6` and `11`)",
            1,
        )
        .replacen("`string(1, 100, ", "`string(1, 6, ", 1)
        .replacen("`string(0, 65000, ", "`string(0, 11, ", 1);
    let source = repository_file("examples/fetch/main.s");
    let right = machine(&source).await;
    for index in 0..3 {
        let verdict = prove(&right, &block(&spec, index), &network_budget(false));
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
            "block {index}: {verdict:?}"
        );
    }
    let fetch = block(&spec, 0);
    for wrong in [
        source.replacen(
            ".ascii \" HTTP/1.0\\r\\n\\r\\n\"",
            ".ascii \" HTTP/1.1\\r\\n\\r\\n\"",
            1,
        ),
        source.replacen(".equ VERSION_LEN, 13", ".equ VERSION_LEN, 11", 1),
        // One read in place of fill: a reply cut by the network comes up short.
        source.replacen(
            "    mov ecx, -1\n    call fill\n",
            "    mov eax, SYS_READ\n    syscall\n",
            1,
        ),
    ] {
        assert_ne!(wrong, source);
        let verdict = prove(&machine(&wrong).await, &fetch, &network_budget(false));
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
    let spec = fixture("padded", "spec.md")
        .replacen(
            "of up to `1048576` bytes",
            "of up to `1048576` bytes (tests: `120`)",
            1,
        )
        .replacen("string(0, 1048576,", "string(0, 120,", 1);
    let machine = machine(&fixture("padded", "main.s")).await;
    let verdict = prove(&machine, &block(&spec, 0), &text_budget());
    assert!(
        matches!(verdict, Verdict::Proven { bound: None, .. }),
        "{verdict:?}"
    );
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
    let block = block(&fixture(&name, "spec.md"), index);
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
/// `last` moved down 32: capitals, when `last` is `z`.
fn capitals(last: u8) -> String {
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

/// Text beyond ASCII: each character is one of those of its width, é and € among them, and
/// every order of widths is a shape of its own, so capitals of ASCII letters are proven over
/// text with accents and symbols, up to 8 characters; capitals that take 0xc3, é's first byte,
/// for a letter, and so take bytes up to it, are found.
#[tokio::test]
async fn proves_capitals_over_text_beyond_ascii() {
    const SPEC: &str = r#"---
type: lengua/program-spec
title: capitals
---
# Purpose
Prints standard input of `0` to `8` characters back with each ASCII letter in capitals.

# Behaviors
## B1: Prints it in capitals
- Given standard input holding `ab`
- When the program runs
- Then it prints `AB`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `string(0, 8, "abz é€")` |

# Constraints
- Uses: standard input
"#;
    let block = block(SPEC, 0);
    // A path for each choice of letter or not at each ASCII place.
    let budget = || Budget {
        max_paths: 512,
        ..text_budget()
    };
    let verdict = prove(&machine(&capitals(b'z')).await, &block, &budget());
    assert!(
        matches!(
            verdict,
            Verdict::Proven {
                bound: None,
                shapes: 9841,
                ..
            }
        ),
        "{verdict:?}"
    );
    let verdict = prove(&machine(&capitals(0xc3)).await, &block, &budget());
    let Verdict::Counterexample(found) = &verdict else {
        panic!("{verdict:?}");
    };
    // é's first byte, or €'s later ones, which fall in the same range.
    assert!(
        matches!(found.get("text"), Some(Value::Str(text)) if !text.is_ascii()),
        "{found:?}"
    );
}

/// examples/count's lines and words over text with accented letters and symbols: every byte of
/// a character beyond ASCII belongs to a word.
#[tokio::test]
async fn proves_a_word_count_over_text_beyond_ascii() {
    let purpose = repository_file("examples/count/spec.md").replacen(
        "The tests give it standard\ninput of up to `1048576` bytes",
        "The tests give it standard\ninput of up to `1048576` bytes (tests: `16`)",
        1,
    );
    let spec = purpose.replacen(
        "string(0, 1048576, \"abcdefghijklmnopqrstuvwxyz0123456789 \\n\")",
        "string(0, 16, \"abcdefghijklmnopqrstuvwxyz0123456789 \\né€\")",
        1,
    );
    assert_ne!(spec, purpose);
    let machine = machine(&repository_file("examples/count/main.s")).await;
    let verdict = prove(&machine, &block(&spec, 0), &text_budget());
    // Characters of three widths in every order, up to 8 of them.
    let Verdict::Proven {
        bound: Some(bound), ..
    } = &verdict
    else {
        panic!("{verdict:?}");
    };
    assert_eq!(bound.bytes, 8, "{verdict:?}");
    assert!(bound.characters, "{verdict:?}");
    assert!(bound.why.contains("8 characters"), "{verdict:?}");
}
