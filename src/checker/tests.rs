//! The template passes; every rule and every known hazard has a failing fixture, each checked
//! for its rule and line.

use super::*;
use crate::toolchain::Toolchain;

/// What the template's spec allows.
const SYSCALLS: [Syscall; 2] = [Syscall::Write, Syscall::Exit];

async fn check_with(text: &str, features: CpuFeatures) -> Report {
    check_full(text, features, &SYSCALLS, None).await
}

async fn check_full(
    text: &str,
    features: CpuFeatures,
    syscalls: &[Syscall],
    threads: Option<u32>,
) -> Report {
    let (source, layout) = check_layout(text);
    if !layout.passed() {
        return layout;
    }
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    let folder = tempfile::tempdir().expect("temporary folder");
    let main = folder.path().join("main.s");
    let object = folder.path().join("main.o");
    let executable = folder.path().join("main");
    std::fs::write(&main, text).expect("write main.s");
    toolchain
        .assemble(Profile::LinuxX86_64, &main, &object)
        .await
        .unwrap_or_else(|error| panic!("assembling failed: {error:?}"));
    toolchain
        .link(&object, &executable)
        .await
        .unwrap_or_else(|error| panic!("linking failed: {error:?}"));
    let bytes = std::fs::read(&executable).expect("read the executable");
    check_program(
        &source,
        &bytes,
        Context {
            syscalls,
            threads,
            features,
            profile: Profile::LinuxX86_64,
        },
    )
}

async fn check(text: &str) -> Report {
    check_with(text, CpuFeatures::all()).await
}

/// The line number, counting from 1, where `needle` (which may span lines) first starts.
fn line_of(text: &str, needle: &str) -> usize {
    let position = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} isn't in the text"));
    text[..position].matches('\n').count() + 1
}

/// Replaces the first `old` in the template with `new`.
fn with(old: &str, new: &str) -> String {
    assert!(TEMPLATE.contains(old), "{old:?} isn't in the template");
    TEMPLATE.replacen(old, new, 1)
}

/// Asserts a violation of `rule` on `line` whose text contains `needle`.
fn assert_violation(report: &Report, rule: Rule, line: usize, needle: &str) {
    let found = report.violations.iter().any(|violation| {
        violation.rule == rule
            && violation.line == Some(line)
            && violation.to_string().contains(needle)
    });
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(
        found,
        "no {rule:?} violation on line {line} containing {needle:?} in {all:#?}"
    );
}

#[tokio::test]
async fn the_template_passes_every_rule() {
    let report = check(TEMPLATE).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    let facts = report.facts.expect("the rules ran");
    assert_eq!(facts.functions, 4);
    assert_eq!(facts.assertions, 11);
    assert!(facts.max_stack_bytes >= 48, "{facts:?}");
    assert!(report.unsafe_blocks.is_empty());
}

#[tokio::test]
async fn layout_hazards_are_dialect_violations() {
    let line = line_of(TEMPLATE, "    xor r9d, r9d");
    for (new, needle) in [
        (
            "    xor r9d, r9d; xor eax, eax\n",
            "`;` puts several statements",
        ),
        ("    xor r9d, r9d /* clear */\n", "`/* */` comments"),
        ("    rep\n", "is a prefix"),
        ("    fld1\n", "`fld1` isn't in dialect 2"),
        ("    mov rax, offset usage_msg\n", "offset"),
        ("    mov rax, [usage_msg]\n", "address data relative to rip"),
    ] {
        let report = check(&with("    xor r9d, r9d\n", new)).await;
        assert_violation(&report, Rule::Dialect, line, needle);
    }
    let label = line_of(TEMPLATE, ".Lparse_digits:");
    let report = check(&with(
        ".Lparse_digits:\n",
        ".Lparse_digits: cmp byte ptr [rdi], 0\n",
    ))
    .await;
    assert_violation(
        &report,
        Rule::Dialect,
        label,
        "a label and an instruction share a line",
    );
}

#[tokio::test]
async fn constants_must_come_first_and_stay_numbers() {
    let late = TEMPLATE.replacen(".equ MAX_WRITES, 64\n", "", 1).replacen(
        ".section .bss\n",
        ".section .bss\n.equ MAX_WRITES, 64\n",
        1,
    );
    let report = check(&late).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&late, ".equ MAX_WRITES, 64"),
        "below the header block",
    );
    let report = check(&with(
        ".equ USAGE_MSG_LEN, 18",
        ".equ USAGE_MSG_LEN, . - usage_msg",
    ))
    .await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(TEMPLATE, ".equ USAGE_MSG_LEN"),
        "uses an address",
    );
    let report = check(&with(
        ".equ NEWLINE, 10\n",
        ".equ NEWLINE, 10\n.equ NEWLINE, 13\n",
    ))
    .await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(TEMPLATE, ".equ NEWLINE, 10") + 1,
        "defined twice",
    );
}

#[tokio::test]
async fn x87_breaks_the_line_to_instruction_match() {
    let line = line_of(TEMPLATE, "    xor r9d, r9d");
    let report = check(&with("    xor r9d, r9d\n", "    fstsw ax\n")).await;
    assert_violation(&report, Rule::Dialect, line, "don't match line for line");
}

#[tokio::test]
async fn instructions_outside_dialect_1_are_rejected() {
    let line = line_of(TEMPLATE, "    xor r9d, r9d");
    let report = check(&with("    xor r9d, r9d\n", "    leave\n")).await;
    assert_violation(&report, Rule::Dialect, line, "`leave` isn't in dialect 2");
}

#[tokio::test]
async fn bmi2_follows_the_cpu() {
    let line = line_of(TEMPLATE, "    xor r9d, r9d");
    let text = with("    xor r9d, r9d\n", "    shlx r9, r9, rcx\n");
    let report = check(&text).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "shlx is fine on a CPU with BMI2: {all:#?}");
    let without_bmi2 = CpuFeatures {
        bmi2: false,
        ..CpuFeatures::all()
    };
    let report = check_with(&text, without_bmi2).await;
    assert_violation(&report, Rule::Dialect, line, "needs BMI2");
}

#[tokio::test]
async fn operands_must_mean_what_the_machine_does() {
    let (mut source, _) = check_layout(TEMPLATE);
    let line = line_of(TEMPLATE, "    mov r8, MAX_WRITES");
    for function in &mut source.functions {
        for item in &mut function.items {
            if let source::Item::Instruction(instruction) = item
                && instruction.line == line
            {
                instruction.operands[1] = source::Operand::Immediate {
                    value: 63,
                    name: None,
                };
            }
        }
    }
    let folder = tempfile::tempdir().expect("temporary folder");
    let main = folder.path().join("main.s");
    std::fs::write(&main, TEMPLATE).expect("write main.s");
    let toolchain = Toolchain::discover().await.expect("toolchain");
    toolchain
        .assemble(Profile::LinuxX86_64, &main, &folder.path().join("main.o"))
        .await
        .expect("assemble");
    toolchain
        .link(&folder.path().join("main.o"), &folder.path().join("main"))
        .await
        .expect("link");
    let bytes = std::fs::read(folder.path().join("main")).expect("read");
    let report = check_program(
        &source,
        &bytes,
        Context {
            syscalls: &SYSCALLS,
            threads: None,
            features: CpuFeatures::all(),
            profile: Profile::LinuxX86_64,
        },
    );
    assert_violation(
        &report,
        Rule::Dialect,
        line,
        "is 63 as written but 64 in the machine code",
    );
}

#[tokio::test]
async fn the_stack_is_tracked_exactly() {
    let line = line_of(TEMPLATE, "    test rsi, rsi");
    let report = check(&with(
        "    test rsi, rsi\n",
        "    mov qword ptr [rsp - 8], rax\n    test rsi, rsi\n",
    ))
    .await;
    assert_violation(&report, Rule::Resources, line, "uses memory below rsp");

    let start = line_of(TEMPLATE, "    mov rax, qword ptr [rsp]");
    let report = check(&with(
        "    mov rax, qword ptr [rsp]\n",
        "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
    ))
    .await;
    assert_violation(&report, Rule::Resources, start, "changes rsp");

    // A call in _start is aligned with nothing pushed; one push misaligns it.
    let call = line_of(TEMPLATE, "    call parse_i64");
    let report = check(&with(
        "    call parse_i64\n",
        "    push rax\n    call parse_i64\n    pop rcx\n",
    ))
    .await;
    assert_violation(&report, Rule::Functions, call + 1, "isn't 16-byte aligned");

    // A normal function enters 8 bytes deep, so write_i64 needs its sub rsp, 8.
    let text = with("    sub rsp, 8\n", "").replacen("    add rsp, 8\n", "", 1);
    let report = check(&text).await;
    let call = line_of(&text, "    mov edi, STDOUT\n    call write_all") + 1;
    assert_violation(&report, Rule::Functions, call, "isn't 16-byte aligned");
}

#[tokio::test]
async fn loops_must_be_counted() {
    // A counter in rcx doesn't survive the syscall in the loop.
    let text = with("    mov r8, MAX_WRITES\n", "    mov rcx, MAX_WRITES\n").replacen(
        "    dec r8\n    jnz .Lwrite_all_loop\n",
        "    dec rcx\n    jnz .Lwrite_all_loop\n",
        1,
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Loops,
        line_of(
            &text,
            "    syscall\n    test rax, rax\n    js .Lwrite_all_return",
        ),
        "changes the loop counter rcx",
    );

    // Jumping into the middle of a loop.
    let text = with(
        "    mov r8, MAX_WRITES\n",
        "    jz .Lwrite_all_retry\n    mov r8, MAX_WRITES\n",
    )
    .replacen(
        "    cmp rax, rdx\n",
        ".Lwrite_all_retry:\n    cmp rax, rdx\n",
        1,
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&text, "    jz .Lwrite_all_retry"),
        "jumps into the middle of a loop",
    );

    // A bound smaller than the counter.
    let text = with(
        ".Lwrite_all_loop:  # bound: 64",
        ".Lwrite_all_loop:  # bound: 8",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&text, "    mov eax, SYS_WRITE"),
        "outside 1 to the bound 8",
    );
}

#[tokio::test]
async fn assertions_have_one_place_each() {
    // Two jumps share assertion 9.
    let text = with(
        "    jz .Lwrite_all_assert_empty\n",
        "    jz .Lwrite_all_assert_null\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(&text, "    mov edi, 9"),
        "reached from 2 places",
    );
    // Assertion 10 now has no way in, so it can never run.
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&text, "    mov edi, 10"),
        "can never run",
    );

    let text = with("    mov edi, 11\n", "    mov edi, 9\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(
            &text,
            "    mov edi, 9\n    call assert_fail\n.size write_all",
        ),
        "used twice",
    );

    let text = with("    add dl, 48\n", "    add dl, 49\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(&text, "assert_fail:"),
        "assert_fail differs from the template",
    );
}

#[tokio::test]
async fn comparison_chains_need_a_default() {
    let text = with(
        "    mov rdi, rdx\n",
        "    cmp rdx, 1\n    je .Lstart_one\n    cmp rdx, 2\n    je .Lstart_one\n.Lstart_one:\n    mov rdi, rdx\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&text, "    cmp rdx, 1"),
        "has no default",
    );
}

#[tokio::test]
async fn results_are_checked_right_away() {
    let text = with(
        "    call parse_i64\n    test rax, rax\n",
        "    call parse_i64\n.Lstart_parsed:\n    test rax, rax\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Results,
        line_of(&text, "    call parse_i64"),
        "a label comes before the check",
    );

    let text = with(
        "    syscall\n    test rax, rax\n    js .Lwrite_all_return\n",
        "    syscall\n    mov r9, rax\n    test rax, rax\n    js .Lwrite_all_return\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Results,
        line_of(&text, "    syscall\n    mov r9, rax"),
        "rax is used before it's checked",
    );
}

#[tokio::test]
async fn system_calls_and_exit_codes_are_named_and_allowed() {
    let text = with("    mov edi, EXIT_OK\n", "    mov edi, 0\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&text, "    mov edi, 0\n"),
        "without a named constant",
    );

    let text = with("    mov eax, SYS_EXIT\n", "    mov eax, 60\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&text, "    mov eax, 60\n"),
        "isn't named",
    );

    let text = with(
        ".equ SYS_EXIT, 60\n",
        ".equ SYS_EXIT, 60\n.equ SYS_READ, 0\n",
    )
    .replacen("    mov eax, SYS_WRITE\n", "    mov eax, SYS_READ\n", 1);
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&text, "    mov eax, SYS_READ") + 1,
        "isn't one the spec allows",
    );
}

#[tokio::test]
async fn data_stays_in_scope() {
    let text = with(".equ USAGE_MSG_LEN, 18", ".equ USAGE_MSG_LEN, 17");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&text, ".equ USAGE_MSG_LEN"),
        "usage_msg is 18 bytes",
    );

    let text = with(
        "    test rsi, rsi\n",
        "    mov byte ptr [rip + usage_msg], 0\n    test rsi, rsi\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&text, "    mov byte ptr [rip + usage_msg], 0"),
        "writes to read-only usage_msg",
    );

    let text = with(
        "    test rsi, rsi\n",
        "    lea r9, [rip + out_buf]\n    test rsi, rsi\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&text, "out_buf:"),
        "out_buf is used by write_all, write_i64",
    );
}

#[tokio::test]
async fn indexed_accesses_are_bounds_checked() {
    let text = with("    cmp rcx, MAX_CHARS\n", "    cmp rdx, MAX_CHARS\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&text, "    movzx edx, byte ptr [rdi + rcx]"),
        "isn't bounds-checked",
    );

    let marked = with(
        "    movzx edx, byte ptr [rdi + rcx]\n",
        "    # unsafe: the loop counter keeps rcx below MAX_CHARS\n    movzx edx, byte ptr [rdi + rcx]\n    # end unsafe\n",
    )
    .replacen("    cmp rcx, MAX_CHARS\n", "    cmp rdx, MAX_CHARS\n", 1);
    let report = check(&marked).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    assert_eq!(report.unsafe_blocks.len(), 1);
}

#[tokio::test]
async fn bounds_checks_compare_without_sign_and_fit_the_buffer() {
    let text = with(
        "    jae .Lparse_assert_index\n",
        "    jge .Lparse_assert_index\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&text, "    jge .Lparse_assert_index"),
        "jumps with jge",
    );

    let text = with(
        "    cmp ecx, OUT_BUF_LEN\n    jae .Lwrite_i64_assert_digit_index\n",
        "    cmp ecx, OUT_BUF_LEN + 1\n    jae .Lwrite_i64_assert_digit_index\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Indirection,
        line_of(&text, "    cmp ecx, OUT_BUF_LEN + 1"),
        "reach byte 21 of out_buf, which has 21 bytes",
    );

    // `lea` computes an address without touching memory, so its index needs no check.
    let text = with(
        "    cmp rcx, MAX_CHARS\n",
        "    lea r11, [rdi + rcx]\n    cmp rcx, MAX_CHARS\n",
    );
    let report = check(&text).await;
    assert!(report.violations.is_empty(), "{:#?}", report.violations);
}

#[tokio::test]
async fn functions_stay_small_and_save_what_they_change() {
    let text = with(
        "    test rsi, rsi\n",
        "    xor ebx, ebx\n    test rsi, rsi\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&text, "write_all:"),
        "changes rbx without saving it",
    );

    let text = with("    call parse_i64\n", "    call _start\n");
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::ControlFlow,
        line_of(&text, "    call _start"),
        "recursion",
    );
}

#[tokio::test]
async fn a_long_function_gets_a_place_to_split() {
    let text = with(
        "    jo .Lparse_invalid\n    inc rcx\n    dec r8\n",
        &format!(
            "    jo .Lparse_invalid\n    inc rcx\n{}    dec r8\n",
            "    nop\n".repeat(25)
        ),
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&text, "parse_i64:"),
        "the loop `.Lparse_loop` (40 instructions), can be a function of its own",
    );
}

#[test]
fn reports_have_a_fixed_format() {
    let violation = Violation {
        rule: Rule::Resources,
        line: Some(42),
        problem: "`and rsp, -16` changes rsp".into(),
        fix: "change rsp only with push, pop, call, ret, and add or sub rsp with a constant".into(),
    };
    assert_eq!(
        violation.to_string(),
        "rule 3 (fixed resources) main.s:42: `and rsp, -16` changes rsp. Fix: change rsp only \
         with push, pop, call, ret, and add or sub rsp with a constant."
    );
    let layout = Violation {
        rule: Rule::Dialect,
        line: Some(7),
        problem: "`;` puts several statements on one line".into(),
        fix: "write one instruction per line".into(),
    };
    assert_eq!(
        layout.to_string(),
        "dialect main.s:7: `;` puts several statements on one line. Fix: write one instruction \
         per line."
    );
}

/// The template with `lines` added right after `label`, the first line of a function.
fn with_after(label: &str, lines: &str) -> String {
    with(&format!("{label}\n"), &format!("{label}\n{lines}"))
}

#[tokio::test]
async fn integer_vector_instructions_are_in_dialect_2() {
    let vectors = "    pxor xmm0, xmm0\n    vpaddb xmm1, xmm1, xmmword ptr [rdi]\n    \
                   vpaddb ymm1, ymm2, ymm3\n    vpcmpeqb k1, zmm0, zmm1\n    kortestq k1, k1\n    \
                   vpternlogd zmm0, zmm1, zmm2, 0x96\n    vpaddd zmm0 {k1}, zmm1, zmm2\n    \
                   vpaddd zmm0 {k1} {z}, zmm1, dword ptr [rdi]{1to16}\n    \
                   vpcmpeqb k1 {k2}, zmm0, zmm1\n    vmovdqu8 zmm0 {k1} {z}, zmmword ptr [rdi]\n    \
                   vpcompressb zmm1 {k1} {z}, zmm0\n    vzeroupper\n";
    let text = with_after("parse_i64:", vectors);
    let report = check(&text).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    // Each extension needs the CPU to have it; SSE2 is in every x86-64 CPU.
    let without_avx2 = CpuFeatures {
        avx2: false,
        ..CpuFeatures::all()
    };
    let report = check_with(&text, without_avx2).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    vpaddb ymm1"),
        "`vpaddb` needs AVX2, which this CPU doesn't have",
    );
    assert!(
        !report
            .violations
            .iter()
            .any(|violation| violation.to_string().contains("`pxor`")),
        "SSE2 is baseline"
    );
}

#[tokio::test]
async fn vectors_leave_out_alignment_and_rounding_overrides() {
    for (line, needle) in [
        ("    rcpps xmm0, xmm1\n", "`rcpps` isn't in dialect 2"),
        (
            "    movdqa xmm0, xmm1\n",
            "`movdqa` faults unless its memory is aligned",
        ),
        (
            "    paddb xmm0, xmmword ptr [rdi]\n",
            "`paddb` reads 16 bytes of memory in its SSE form",
        ),
        (
            "    vpaddd zmm0, zmm1, zmm2, {rn-sae}\n",
            "rounding overrides ({rn-sae}) aren't in the dialect",
        ),
        (
            "    vpaddd zmm0, zmm1 {k1}, zmm2\n",
            "go on the first operand",
        ),
        (
            "    paddb mm0, mm1\n",
            "MMX registers aren't in the dialect",
        ),
    ] {
        let text = with_after("parse_i64:", line);
        let report = check(&text).await;
        assert_violation(&report, Rule::Dialect, line_of(&text, line), needle);
    }
}

#[tokio::test]
async fn vector_accesses_stay_inside_their_data() {
    // An indexed 16-byte store must fit the buffer for every index the check lets through.
    let text = with(
        "    mov byte ptr [rsi + rcx], dl\n",
        "    movdqu xmmword ptr [rsi + rcx], xmm0\n",
    );
    let report = check(&text).await;
    let compare = text
        .lines()
        .take(line_of(&text, "    movdqu xmmword ptr [rsi + rcx], xmm0"))
        .collect::<Vec<_>>()
        .iter()
        .rposition(|line| line.trim() == "cmp ecx, OUT_BUF_LEN")
        .expect("the bounds check")
        + 1;
    assert_violation(
        &report,
        Rule::Indirection,
        compare,
        "reach byte 35 of out_buf, which has 21 bytes",
    );

    // A masked access counts its full width too: the mask may leave lanes out, but the check
    // doesn't trust it to.
    let text = with(
        "    mov byte ptr [rsi + rcx], dl\n",
        "    vmovdqu8 zmmword ptr [rsi + rcx] {k1}, zmm0\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Indirection,
        compare,
        "reach byte 83 of out_buf, which has 21 bytes",
    );

    // A [rip + name] access must fit the symbol over its whole width.
    let text = with_after(
        "_start:",
        "    movdqu xmm0, xmmword ptr [rip + usage_msg + 8]\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Scope,
        line_of(&text, "    movdqu xmm0, xmmword ptr [rip + usage_msg + 8]"),
        "reaches 6 bytes past the end of usage_msg, which has 18 bytes",
    );
}

#[tokio::test]
async fn double_precision_is_in_dialect_2() {
    let doubles = "    cvtsi2sd xmm0, rdi\n    addsd xmm0, xmm1\n    divsd xmm0, xmm1\n    \
                   sqrtsd xmm0, xmm0\n    roundsd xmm0, xmm0, 4\n    vmulpd ymm0, ymm1, ymm2\n    \
                   vaddpd zmm0 {k1}, zmm1, qword ptr [rdi]{1to8}\n    ucomisd xmm0, xmm1\n    \
                   cvttsd2si rax, xmm0\n";
    let text = with(
        ".section .rodata\n",
        ".section .rodata\n\n.type hundred, @object\nhundred:\n    .double 100.0, -0.5\n.size \
         hundred, . - hundred\n",
    );
    let text = text.replacen("parse_i64:\n", &format!("parse_i64:\n{doubles}"), 1);
    let report = check(&text).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    for (line, needle) in [
        ("    rcpss xmm0, xmm1\n", "`rcpss` isn't in dialect 2"),
        (
            "    vrsqrt14ps zmm0, zmm1\n",
            "`vrsqrt14ps` isn't in dialect 2",
        ),
        (
            "    vcvtph2ps xmm0, xmm1\n",
            "`vcvtph2ps` isn't in dialect 2",
        ),
        ("    dpps xmm0, xmm1, 0xff\n", "`dpps` isn't in dialect 2"),
        (
            "    ldmxcsr dword ptr [rsp]\n",
            "`ldmxcsr` isn't in dialect 2",
        ),
        ("    movsd\n", "string instruction"),
    ] {
        let text = with_after("parse_i64:", line);
        let report = check(&text).await;
        assert_violation(&report, Rule::Dialect, line_of(&text, line), needle);
    }
    let text = with(
        ".section .rodata\n",
        ".section .rodata\n\n.type hundred, @object\nhundred:\n    .double HUNDRED\n.size hundred, \
         . - hundred\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&text, "    .double HUNDRED"),
        "isn't a plain decimal number",
    );
}

#[tokio::test]
async fn single_precision_and_fma_are_in_dialect_2() {
    let singles = "    cvtsi2ss xmm0, rdi\n    divss xmm0, dword ptr [rip + tenth]\n    \
                   vfmadd231ss xmm0, xmm1, xmm2\n    vfmadd213pd ymm0, ymm1, ymm2\n    \
                   vfnmadd231ps zmm0 {k1}, zmm1, dword ptr [rdi]{1to16}\n    \
                   cvtss2sd xmm1, xmm0\n    cvtsd2ss xmm0, xmm1\n    sqrtss xmm0, xmm0\n    \
                   vmulps ymm0, ymm1, ymm2\n    ucomiss xmm0, xmm1\n    cvttss2si eax, xmm0\n";
    let text = with(
        ".section .rodata\n",
        ".section .rodata\n\n.type tenth, @object\ntenth:\n    .float 10.0\n.size tenth, . - \
         tenth\n",
    );
    let text = text.replacen("parse_i64:\n", &format!("parse_i64:\n{singles}"), 1);
    let report = check(&text).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    // Fused multiply-add needs the CPU to have it.
    let without_fma = CpuFeatures {
        fma: false,
        ..CpuFeatures::all()
    };
    let report = check_with(&text, without_fma).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    vfmadd231ss"),
        "`vfmadd231ss` needs FMA, which this CPU doesn't have",
    );

    let text = with(
        ".section .rodata\n",
        ".section .rodata\n\n.type tenth, @object\ntenth:\n    .float 1e1\n.size tenth, . - \
         tenth\n",
    );
    let report = check(&text).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&text, "    .float 1e1"),
        "`.float 1e1` isn't a plain decimal number",
    );
}

/// What the parallel template's spec allows.
const PARALLEL_SYSCALLS: [Syscall; 3] = [Syscall::Read, Syscall::Write, Syscall::Exit];

async fn check_parallel(text: &str) -> Report {
    check_full(text, CpuFeatures::all(), &PARALLEL_SYSCALLS, Some(16)).await
}

#[tokio::test]
async fn the_parallel_template_passes_every_rule() {
    let report = check_parallel(PARALLEL_TEMPLATE).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
}

#[tokio::test]
async fn threads_need_the_spec_and_the_canonical_code() {
    // Threads the spec doesn't allow.
    let report = check_full(
        PARALLEL_TEMPLATE,
        CpuFeatures::all(),
        &PARALLEL_SYSCALLS,
        None,
    )
    .await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(PARALLEL_TEMPLATE, "parallel_for:"),
        "spec.md doesn't allow threads",
    );
    // More threads than it allows.
    let report = check_full(
        PARALLEL_TEMPLATE,
        CpuFeatures::all(),
        &PARALLEL_SYSCALLS,
        Some(8),
    )
    .await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(PARALLEL_TEMPLATE, ".equ THREADS, 16"),
        "THREADS is 16, but the spec allows from 1 to 8 threads",
    );
    for (old, new, rule, needle, message) in [
        (
            "    mov ebx, 1\n    mov r13, THREADS\n.Lparallel_for_start:",
            "    mov ebx, 2\n    mov r13, THREADS\n.Lparallel_for_start:",
            Rule::Resources,
            "parallel_for:",
            "parallel_for differs from the template",
        ),
        (
            ".equ CLONE_FLAGS, 0x350F00",
            ".equ CLONE_FLAGS, 0x50F00",
            Rule::Resources,
            ".equ CLONE_FLAGS",
            "CLONE_FLAGS must be 3477248",
        ),
        (
            ".equ SLOT_SIZE, CHUNK",
            ".equ SLOT_SIZE, CHUNK + 8",
            Rule::Resources,
            ".equ SLOT_SIZE",
            "isn't a positive multiple of 64",
        ),
    ] {
        let text = PARALLEL_TEMPLATE.replacen(old, new, 1);
        assert_ne!(text, PARALLEL_TEMPLATE, "{old}");
        let report = check_parallel(&text).await;
        assert_violation(&report, rule, line_of(&text, needle), message);
    }
    // Each thread's stack must hold the worker's deepest path: here 128 bytes, in 64.
    let text = PARALLEL_TEMPLATE
        .replacen(".equ STACK_SIZE, 4096", ".equ STACK_SIZE, 64", 1)
        .replacen(
            "worker:\n    push rbx\n",
            "worker:\n    push rbx\n    sub rsp, 64\n",
            1,
        )
        .replacen(
            ".Lworker_done:\n    mov rax, rdx\n    pop rbx\n",
            ".Lworker_done:\n    mov rax, rdx\n    add rsp, 64\n    pop rbx\n",
            1,
        );
    let report = check_parallel(&text).await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&text, ".equ STACK_SIZE"),
        "a worker's stack can reach 128 bytes, but STACK_SIZE is 64",
    );
}

#[tokio::test]
async fn workers_write_only_their_stack_and_their_slot() {
    let loop_end = "    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n";
    for (old, new, rule, needle, message) in [
        (
            loop_end,
            "    mov qword ptr [rip + input_size], rcx\n    add r9, VECTOR\n    dec r8\n    \
             jnz .Lworker_loop\n",
            Rule::Scope,
            "    mov qword ptr [rip + input_size], rcx",
            "writes input_size on a worker thread",
        ),
        (
            loop_end,
            "    mov byte ptr [rsi], al\n    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
            Rule::Scope,
            "    mov byte ptr [rsi], al",
            "writes through rsi on a worker thread",
        ),
        (
            "    cmp r9, SLOT_SIZE - VECTOR + 1\n",
            "    cmp r9, SLOT_SIZE\n",
            Rule::Scope,
            "    movdqu xmmword ptr [rbx + r9], xmm0",
            "outside the worker's slot",
        ),
        (
            loop_end,
            "    add rbx, VECTOR\n    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
            Rule::Scope,
            "    add rbx, VECTOR",
            "changes rbx, which holds the worker's slot",
        ),
        (
            ".Lworker_done:\n    mov rax, rdx\n",
            ".Lworker_done:\n    mov eax, SYS_WRITE\n    syscall\n    test rax, rax\n    \
             js .Lworker_done_assert\n    mov rax, rdx\n",
            Rule::Resources,
            "    mov eax, SYS_WRITE\n    syscall\n    test rax, rax\n    js .Lworker_done_assert",
            "makes a system call on a worker thread",
        ),
    ] {
        let mut text = PARALLEL_TEMPLATE.replacen(old, new, 1);
        assert_ne!(text, PARALLEL_TEMPLATE, "{old}");
        if new.contains(".Lworker_done_assert") {
            text = text.replacen(
                ".Lworker_assert_index:\n",
                ".Lworker_done_assert:\n    mov edi, 16\n    call assert_fail\n.Lworker_assert_index:\n",
                1,
            );
        }
        let report = check_parallel(&text).await;
        let line = line_of(&text, needle.lines().next().unwrap_or(needle))
            + usize::from(needle.contains("syscall"));
        assert_violation(&report, rule, line, message);
    }
}

#[tokio::test]
async fn only_the_thread_code_starts_threads() {
    // clone anywhere else is a system call the spec doesn't list.
    let text = PARALLEL_TEMPLATE.replacen(
        "_start:\n    call read_input\n",
        "_start:\n    mov edi, CLONE_FLAGS\n    mov eax, SYS_CLONE\n    syscall\n    test rax, rax\n    \
         js .Lstart_assert_length\n    call read_input\n",
        1,
    );
    let report = check_parallel(&text).await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(
            &text,
            "    mov eax, SYS_CLONE\n    syscall\n    test rax, rax\n    js .Lstart",
        ) + 1,
        "system call 56 isn't one the spec allows",
    );
    // The main thread can't call a function that runs on worker threads.
    let text = PARALLEL_TEMPLATE.replacen(
        "    mov rdi, r12\n    call total_results\n",
        "    mov rdi, r12\n    call total_results\n    xor edi, edi\n    lea rsi, [rip + slots]\n    \
         call worker\n",
        1,
    );
    let report = check_parallel(&text).await;
    assert_violation(
        &report,
        Rule::Resources,
        line_of(&text, "    call worker\n    cmp rax, rbx"),
        "_start calls worker, which only the thread code may call",
    );
}

#[tokio::test]
async fn only_a_server_takes_connections_forever() {
    let echo = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/echo/main.s"),
    )
    .expect("the echo example");
    let server = [
        Syscall::Read,
        Syscall::Close,
        Syscall::Accept4,
        Syscall::Sendto,
        Syscall::Exit,
    ];
    let report = check_full(&echo, CpuFeatures::all(), &server, None).await;
    assert!(report.passed(), "{report:#?}");
    // A program that doesn't serve has no loop without a bound.
    let not_a_server = [Syscall::Read, Syscall::Close, Syscall::Exit];
    let report = check_full(&echo, CpuFeatures::all(), &not_a_server, None).await;
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&echo, ".Lstart_serve:  # bound: forever") + 1,
        "only a server's loop that takes connections may run forever",
    );
    // A loop that could end by itself isn't one that runs forever.
    let ending = echo.replacen(
        "    jmp .Lstart_serve\n",
        "    test rbx, rbx\n    jnz .Lstart_serve\n    jmp .Lstart_serve\n",
        1,
    );
    let report = check_full(&ending, CpuFeatures::all(), &server, None).await;
    assert_violation(
        &report,
        Rule::Loops,
        line_of(&ending, "    jnz .Lstart_serve"),
        "closes with an unconditional `jmp`",
    );
}

/// A program that reads with fill, in tests/fixtures.
fn with_fill() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/second/main.s"),
    )
    .expect("the fixture")
}

#[tokio::test]
async fn fill_is_copied_word_for_word_with_its_constants() {
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let text = with_fill();
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    assert!(report.passed(), "{:?}", report.violations);

    // fill read as a whole for every cut is only as right as its code is the template's.
    let changed = text.replacen(
        "    sub rdx, rax\n    jz .Lfill_done\n",
        "    sub rdx, rax\n    cmp rdx, 1\n    jbe .Lfill_done\n",
        1,
    );
    assert_ne!(changed, text);
    let report = check_full(&changed, CpuFeatures::all(), &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Assertions,
        line_of(&changed, "fill:"),
        "fill differs from the template",
    );

    let fewer = text.replacen(
        ".equ MAX_FILL_READS, 1099511627776\n",
        ".equ MAX_FILL_READS, 64\n",
        1,
    );
    assert_ne!(fewer, text);
    let report = check_full(&fewer, CpuFeatures::all(), &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Constants,
        line_of(&fewer, ".equ MAX_FILL_READS"),
        "MAX_FILL_READS must be 1099511627776",
    );
}
