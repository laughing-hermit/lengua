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
    let profile = match source.isa {
        Isa::X86_64 => Profile::LinuxX86_64,
        Isa::Aarch64 => Profile::LinuxAarch64,
    };
    let folder = tempfile::tempdir().expect("temporary folder");
    let main = folder.path().join("main.s");
    let object = folder.path().join("main.o");
    let executable = folder.path().join("main");
    std::fs::write(&main, text).expect("write main.s");
    toolchain
        .assemble(profile, &main, &object)
        .await
        .unwrap_or_else(|error| panic!("assembling failed: {error:?}"));
    toolchain
        .link(profile, &object, &executable)
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
            profile,
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
        ("    fld1\n", "`fld1` isn't in dialect 4"),
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
    assert_violation(&report, Rule::Dialect, line, "`leave` isn't in dialect 4");
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
        .link(
            Profile::LinuxX86_64,
            &folder.path().join("main.o"),
            &folder.path().join("main"),
        )
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

/// A number that doesn't fit its instruction's field is refused: the assembler would keep only
/// its low bits, so main.s wouldn't say what runs.
#[tokio::test]
async fn numbers_must_fit_their_field() {
    for (old, new, bits) in [
        (
            "    mov edx, USAGE_MSG_LEN\n",
            "    mov edx, 4294967314\n",
            32,
        ),
        (
            "    cmp byte ptr [rdi], '-'\n",
            "    cmp byte ptr [rdi], 301\n",
            8,
        ),
    ] {
        let text = with(old, new);
        let report = check(&text).await;
        assert_violation(
            &report,
            Rule::Dialect,
            line_of(&text, new),
            &format!("which doesn't fit in its {bits} bits"),
        );
    }
    // A number that fits, read signed or unsigned, is as written.
    for (old, new) in [
        ("    mov edx, USAGE_MSG_LEN\n", "    mov edx, 4294967295\n"),
        ("    mov edx, USAGE_MSG_LEN\n", "    mov edx, -1\n"),
    ] {
        let text = with(old, new);
        let report = check(&text).await;
        assert!(
            !report
                .violations
                .iter()
                .any(|violation| violation.to_string().contains("doesn't fit")),
            "{new:?}: {:#?}",
            report.violations
        );
    }
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

/// The calc fixture, whose `apply` picks its operator's case through a checked jump table.
fn calc() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/calc/main.s"),
    )
    .expect("calc's main.s")
}

/// `calc` with the first `old` replaced by `new`.
fn calc_with(old: &str, new: &str) -> String {
    let calc = calc();
    assert!(calc.contains(old), "{old:?} isn't in calc");
    calc.replacen(old, new, 1)
}

/// Dialect 4's one jump through a register: a checked jump table passes, with the table's cases
/// as the jump's successors, and every other way to jump through a register or memory is refused.
#[tokio::test]
async fn jumps_go_through_registers_only_as_checked_tables() {
    let report = check(&calc()).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    let facts = report.facts.expect("the rules ran");
    assert_eq!(facts.tables.len(), 1, "{facts:?}");
    let (jump, cases) = facts.tables.iter().next().expect("one table");
    assert_eq!(cases.len(), 4, "{cases:?}");
    assert!(cases.iter().all(|case| case > jump), "{cases:?}");

    let jump = "    jmp rax\n";
    let entries = "    .long .Lapply_multiply - operations\n";
    for (text, rule, at, needle) in [
        // The index unchecked, or checked against a limit past the table.
        (
            calc_with("    cmp eax, OPERATORS\n    jae .Lapply_unknown\n", ""),
            Rule::Indirection,
            "    movsxd rax, dword ptr [rcx + rax*4]\n",
            "isn't bounds-checked",
        ),
        (
            calc_with("    cmp eax, OPERATORS\n", "    cmp eax, OPERATORS + 1\n"),
            Rule::Indirection,
            "    cmp eax, OPERATORS + 1\n",
            "reach byte 19 of operations, which has 16 bytes",
        ),
        // Entries naming what isn't a case of this jump.
        (
            calc_with(entries, "    .long .Lstart_exit - operations\n"),
            Rule::Indirection,
            jump,
            "names .Lstart_exit, which isn't a label of apply",
        ),
        (
            calc_with(
                "    lea rcx, [rip + operations]\n",
                ".Lapply_table:\n    lea rcx, [rip + operations]\n",
            )
            .replacen(entries, "    .long .Lapply_table - operations\n", 1),
            Rule::Indirection,
            jump,
            "comes before the jump",
        ),
        // A table that can change, isn't only entries, or isn't the table's own distances.
        (
            calc_with(
                "# apply's jump table",
                ".section .data\n# apply's jump table",
            )
            .replacen(
                ".size operations, . - operations\n",
                ".size operations, . - operations\n.section .rodata\n",
                1,
            ),
            Rule::Immutability,
            entries,
            "jump table operations is in writable data",
        ),
        (
            calc_with(entries, "    .quad .Lapply_multiply - operations\n"),
            Rule::Indirection,
            "    .quad .Lapply_multiply - operations\n",
            "isn't a jump table's entry",
        ),
        (
            calc_with(entries, "    .long .Lapply_multiply - nan_msg\n"),
            Rule::Indirection,
            "    .long .Lapply_multiply - nan_msg\n",
            "isn't a jump table's entry",
        ),
        (
            calc_with(
                "    .long .Lapply_subtract - operations\n",
                "    .long .Lapply_subtract - operations\n    .long 0\n",
            ),
            Rule::Indirection,
            "    .long 0\n",
            "mixes numbers with its entries",
        ),
        // Any other jump through a register, or one through memory.
        (
            calc_with(
                "    add rax, rcx\n    jmp rax\n",
                "    add rax, rcx\n    or rax, 1\n    jmp rax\n",
            ),
            Rule::Indirection,
            jump,
            "not through a checked jump table",
        ),
        (
            calc_with("    add rax, rcx\n", ".Lapply_into:\n    add rax, rcx\n"),
            Rule::Indirection,
            jump,
            "not through a checked jump table",
        ),
        (
            calc_with(
                "    movsxd rax, dword ptr [rcx + rax*4]\n    add rax, rcx\n    jmp rax\n",
                "    jmp qword ptr [rcx + rax*8]\n",
            ),
            Rule::Indirection,
            "    jmp qword ptr [rcx + rax*8]\n",
            "jumps through a register or memory",
        ),
    ] {
        let report = check(&text).await;
        assert_violation(&report, rule, line_of(&text, at), needle);
    }
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

/// Every program in the repository follows every rule, memory rules included, with what its own
/// spec allows: the examples, the fixtures Claude Code wrote, the skill's assets and the
/// benchmark's own programs.
#[tokio::test]
async fn every_program_in_the_repository_follows_every_rule() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = root.join("plugin/skills/compile/assets");
    let mut programs: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for folder in [
        "examples",
        "tests/fixtures",
        "plugin/skills/compile/assets/fast",
    ] {
        for entry in std::fs::read_dir(root.join(folder)).expect("a folder of programs") {
            let path = entry.expect("an entry").path();
            // The CSV total Claude Code wrote in interactive round 2 names a character as a
            // constant, from before constants were held to numbers; its proof tests need only its
            // machine code.
            if path.ends_with("total") {
                continue;
            }
            if path.join("main.s").is_file() {
                programs.push((path.join("spec.md"), path.join("main.s")));
            }
        }
    }
    for (spec, code) in [
        ("template-spec.md", "main-template.s"),
        ("vector-template-spec.md", "vector-template.s"),
        ("parallel-template-spec.md", "parallel-template.s"),
        ("server-template-spec.md", "server-template.s"),
        ("terminal-template-spec.md", "terminal-template.s"),
    ] {
        programs.push((assets.join(spec), assets.join(code)));
    }
    let benches = root.join("benches/programs");
    for (spec, code) in [
        ("examples/upcase/spec.md", "upcase/avx512.s"),
        (
            "plugin/skills/compile/assets/parallel-template-spec.md",
            "shout/avx512.s",
        ),
        (
            "plugin/skills/compile/assets/parallel-template-spec.md",
            "shout/avx512-one.s",
        ),
    ] {
        programs.push((root.join(spec), benches.join(code)));
    }
    // On x86-64 each program is held to this CPU's features, as check holds it; on another CPU
    // the programs can't run, and the checker, which doesn't depend on the computer it runs on,
    // checks them for every feature.
    let features = if cfg!(target_arch = "x86_64") {
        crate::platform::Machine::detect().features
    } else {
        CpuFeatures::all()
    };
    let mut failed = Vec::new();
    for (spec, code) in &programs {
        let spec = crate::spec::parse(&std::fs::read_to_string(spec).expect("a spec"))
            .unwrap_or_else(|problems| panic!("{}: {problems:?}", spec.display()));
        let text = std::fs::read_to_string(code).expect("a program");
        let needs_more = text.contains("vpcmpub") || text.contains("zmm");
        if needs_more && !features.avx512bw {
            continue;
        }
        let report = check_full(&text, features, &spec.syscalls, spec.threads).await;
        if !report.passed() {
            let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
            failed.push(format!("{}: {all:#?}", code.display()));
        }
    }
    assert!(programs.len() >= 25, "{} programs", programs.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// M7: a worker's index bounded only by a register, and a worker reading another thread's part,
/// are found.
#[tokio::test]
async fn workers_never_race() {
    let cases = [
        (
            "    cmp r9, SLOT_SIZE - VECTOR + 1\n    jae .Lworker_assert_slot\n    movdqu xmm0, \
             xmmword ptr [rbx + r9]\n",
            "    movdqu xmm0, xmmword ptr [rbx + r9]\n",
            "    movdqu xmm0, xmmword ptr [rbx + r9]",
            "indexes the worker's slot with no constant limit",
        ),
        (
            "    mov rbx, rsi\n    cmp rdi, THREADS\n",
            "    mov rbx, rsi\n    mov rax, qword ptr [rip + slots]\n    cmp rdi, THREADS\n",
            "    mov rax, qword ptr [rip + slots]",
            "reaches slots on a worker thread, where other threads write their parts of it",
        ),
    ];
    for (from, to, at, needle) in cases {
        assert!(PARALLEL_TEMPLATE.contains(from), "{from:?}");
        let text = PARALLEL_TEMPLATE.replacen(from, to, 1);
        let report = check_parallel(&text).await;
        assert_violation(&report, Rule::Races, line_of(&text, at), needle);
    }
}

/// Memory safety: a bug planted for each memory rule is found, on its line.
#[tokio::test]
async fn memory_rules_find_each_planted_bug() {
    let cases: [(&str, &str, &str, Rule, &str); 17] = [
        // M2: a bounds check of part of the index, and one with a negative limit.
        (
            ".Lparse_loop:  # bound: 20\n    cmp rcx, MAX_CHARS\n",
            ".Lparse_loop:  # bound: 20\n    cmp ecx, MAX_CHARS\n",
            "    cmp ecx, MAX_CHARS",
            Rule::Bounds,
            "compares only the low 32 bits of rcx",
        ),
        (
            ".Lparse_loop:  # bound: 20\n    cmp rcx, MAX_CHARS\n",
            ".Lparse_loop:  # bound: 20\n    cmp rcx, -1\n",
            "    cmp rcx, -1",
            Rule::Bounds,
            "compares with -1, which an unsigned jump reads as a huge number",
        ),
        // M2: a bit test whose offset is a register.
        (
            "    movzx edx, byte ptr [rdi + rcx]\n",
            "    movzx edx, byte ptr [rdi + rcx]\n    bt qword ptr [rdi], rcx\n",
            "    bt qword ptr [rdi], rcx",
            Rule::Bounds,
            "tests a bit in memory at an offset in a register",
        ),
        // M1: an address made from a plain number.
        (
            ".Lstart_usage:\n",
            ".Lstart_usage:\n    mov esi, MAX_WRITES\n    mov byte ptr [rsi], 0\n",
            "    mov byte ptr [rsi], 0",
            Rule::Owners,
            "holds a plain number here, not a pointer",
        ),
        // M2: a constant offset one byte past a symbol.
        (
            ".Lstart_write_failed:\n",
            ".Lstart_write_failed:\n    lea rsi, [rip + out_buf]\n    mov byte ptr [rsi + \
             OUT_BUF_LEN], 0\n",
            "    mov byte ptr [rsi + OUT_BUF_LEN], 0",
            Rule::Bounds,
            "reaches byte 21 of out_buf, which has 21 bytes",
        ),
        // M2: one byte past usage_msg is nan_msg's first, but main.s names usage_msg.
        (
            ".Lstart_write_failed:\n",
            ".Lstart_write_failed:\n    movzx eax, byte ptr [rip + usage_msg + USAGE_MSG_LEN]\n",
            "    movzx eax, byte ptr [rip + usage_msg + USAGE_MSG_LEN]",
            Rule::Scope,
            "reaches 1 byte past the end of usage_msg, which has 18 bytes",
        ),
        // M2: so is a pointer just past usage_msg's end, as main.s makes it.
        (
            ".Lstart_write_failed:\n",
            ".Lstart_write_failed:\n    lea rsi, [rip + usage_msg + USAGE_MSG_LEN]\n    movzx \
             eax, byte ptr [rsi]\n",
            "    movzx eax, byte ptr [rsi]",
            Rule::Bounds,
            "reaches byte 18 of usage_msg, which has 18 bytes",
        ),
        // M3: an address in the frame, returned.
        (
            ".Lparse_return:\n    ret\n",
            ".Lparse_return:\n    lea rdx, [rsp]\n    ret\n",
            "    lea rdx, [rsp]\n    ret",
            Rule::Lifetimes,
            "returns an address in its own frame in rdx",
        ),
        // M3: an address in the frame, stored where it outlives the function.
        (
            ".Lparse_return:\n    ret\n",
            ".Lparse_return:\n    lea rcx, [rsp]\n    mov qword ptr [rip + out_buf], rcx\n    \
             ret\n",
            "    mov qword ptr [rip + out_buf], rcx",
            Rule::Lifetimes,
            "an address in this function's frame, outside the frame",
        ),
        // M3: a read past the frame, into the caller's.
        (
            "parse_i64:\n",
            "parse_i64:\n    mov r10, qword ptr [rsp + 8]\n",
            "    mov r10, qword ptr [rsp + 8]",
            Rule::Lifetimes,
            "into its return address or its caller's frame",
        ),
        // M3: a pointer to a known place in the frame, written past the frame.
        (
            ".Lwrite_i64_return:\n    add rsp, 8\n",
            ".Lwrite_i64_return:\n    lea rcx, [rsp + 8]\n    mov byte ptr [rcx], 0\n    add \
             rsp, 8\n",
            "    mov byte ptr [rcx], 0",
            Rule::Lifetimes,
            "reaches past this function's frame of 8 bytes, into its return address",
        ),
        // M5: a frame slot read before anything writes it.
        (
            "parse_i64:\n",
            "parse_i64:\n    sub rsp, 8\n    mov r10, qword ptr [rsp]\n    add rsp, 8\n",
            "    mov r10, qword ptr [rsp]",
            Rule::Initialization,
            "reads the frame at rsp + 0, which some path reaches without writing",
        ),
        // M1: what a function returns is followed through the call: parse_i64 returns a number.
        (
            "    call parse_i64\n    test rax, rax\n",
            "    call parse_i64\n    mov byte ptr [rax], 0\n    test rax, rax\n",
            "    mov byte ptr [rax], 0",
            Rule::Owners,
            "uses rax as an address, but it holds a plain number here",
        ),
        // M4: the program's arguments are only read.
        (
            "    mov rdi, qword ptr [rsp + 16]\n",
            "    mov rdi, qword ptr [rsp + 16]\n    mov byte ptr [rdi], 0\n",
            "    mov byte ptr [rdi], 0",
            Rule::ExclusiveWrites,
            "writes into the program's arguments, which it only reads",
        ),
        // M5: a frame lent to a function that only reads it is still unwritten.
        (
            ".Lwrite_i64_return:\n    add rsp, 8\n",
            ".Lwrite_i64_return:\n    mov rdi, rsp\n    call parse_i64\n    mov r10, qword ptr \
             [rsp]\n    add rsp, 8\n",
            "    mov r10, qword ptr [rsp]",
            Rule::Initialization,
            "reads the frame at rsp + 0, which some path reaches without writing",
        ),
        // M6: read-only data written through a register.
        (
            ".Lstart_usage:\n",
            ".Lstart_usage:\n    lea rsi, [rip + usage_msg]\n    mov byte ptr [rsi], 0\n",
            "    mov byte ptr [rsi], 0",
            Rule::Immutability,
            "writes to read-only usage_msg",
        ),
        // M8: a frame bigger than the stack's limit.
        (
            "_start:\n",
            "_start:\n    sub rsp, 2097152\n",
            "_start:",
            Rule::StackSize,
            "bytes of stack, more than 1048576",
        ),
    ];
    for (from, to, at, rule, needle) in cases {
        let text = with(from, to);
        let report = check(&text).await;
        // A returned address is reported where it's returned, on the `ret`.
        let line = match rule {
            Rule::StackSize => None,
            _ if at.ends_with("\n    ret") => Some(line_of(&text, at) + 1),
            _ => Some(line_of(&text, at)),
        };
        let found = report.violations.iter().any(|violation| {
            violation.rule == rule
                && (line.is_none() || violation.line == line)
                && violation.to_string().contains(needle)
        });
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(
            found,
            "no {rule:?} violation containing {needle:?} in {all:#?}"
        );
    }
}

/// A pointer just past a symbol's end belongs to the symbol main.s names, whatever starts there:
/// walking back from it stays inside, whether the next symbol is right after or there's none.
#[tokio::test]
async fn a_pointer_past_a_symbol_s_end_is_that_symbol_s() {
    let allowed = [
        // nan_msg starts where usage_msg ends.
        (
            ".Lstart_write_failed:\n",
            "    lea rsi, [rip + usage_msg + USAGE_MSG_LEN]\n    movzx eax, byte ptr [rsi - 1]\n",
        ),
        // Nothing comes after out_buf.
        (
            ".Lwrite_i64_digits:\n",
            "    lea rsi, [rip + out_buf + OUT_BUF_LEN]\n    dec rsi\n    mov byte ptr [rsi], 0\n",
        ),
    ];
    for (label, lines) in allowed {
        let text = with(label, &format!("{label}{lines}"));
        let report = check(&text).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{lines:?}: {all:#?}");
    }
    let text = with(
        ".Lstart_write_failed:\n",
        ".Lstart_write_failed:\n    lea rsi, [rip + usage_msg + USAGE_MSG_LEN + 1]\n",
    );
    assert_violation(
        &check(&text).await,
        Rule::Scope,
        line_of(&text, "    lea rsi, [rip + usage_msg + USAGE_MSG_LEN + 1]"),
        "points 1 byte past the end of usage_msg, which has 18 bytes",
    );
}

/// Two functions that write through rdi and read through rsi: `fill` with a `read` system call,
/// `refill` by calling `fill`.
const FILLERS: &str =
    "# Reads up to rdx bytes of standard input into rdi, and returns the byte at rsi.
# In: rdi = the buffer, rsi = a byte to look at, rdx = how many, at least 1.
# Out: rax = the byte.
# returns: result
.type fill, @function
fill:
    push rbx
    mov rbx, rsi
    mov rsi, rdi
    xor edi, edi
    mov eax, SYS_READ
    syscall
    movzx eax, byte ptr [rbx]
    pop rbx
    ret
.size fill, . - fill

# Reads up to 8 bytes of standard input into rdi, and returns the byte at rsi.
# In: rdi = the buffer, rsi = a byte to look at.
# Out: rax = the byte.
# returns: result
.type refill, @function
refill:
    sub rsp, 8
    mov edx, 8
    call fill
    add rsp, 8
    ret
.size refill, . - refill

";

#[tokio::test]
async fn one_address_is_never_lent_twice_for_writing() {
    let lend = |setup: &str, callee: &str| {
        let text = with(
            ".equ SYS_WRITE, 1\n",
            ".equ SYS_READ, 0\n.equ SYS_WRITE, 1\n",
        )
        .replacen(
            "# Writes rdx bytes from rsi to file descriptor edi",
            &format!("{FILLERS}# Writes rdx bytes from rsi to file descriptor edi"),
            1,
        )
        .replacen(
            ".Lstart_usage:\n",
            &format!(".Lstart_usage:\n{setup}    mov edx, 8\n    call {callee}\n"),
            1,
        );
        assert!(text.contains(&format!("    call {callee}\n")));
        text
    };
    let refused = [
        (
            "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf]\n",
            "fill",
            "lends one address, out_buf, in both rdi and rsi, and fill writes through rdi",
        ),
        // A copy of the pointer is the same address.
        (
            "    lea rdi, [rip + out_buf + 4]\n    mov rsi, rdi\n",
            "fill",
            "lends one address, out_buf + 4, in both rdi and rsi",
        ),
        // refill writes through rdi because what it calls does.
        (
            "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf]\n",
            "refill",
            "and refill writes through rdi",
        ),
    ];
    for (setup, callee, needle) in refused {
        let text = lend(setup, callee);
        let report = check_full(&text, CpuFeatures::all(), &PARALLEL_SYSCALLS, None).await;
        assert_violation(
            &report,
            Rule::ExclusiveWrites,
            line_of(&text, &format!("    call {callee}\n")),
            needle,
        );
    }
    // Separate parts of one buffer may be lent together, as objects in an arena are, and a
    // buffer only read may be lent twice.
    let allowed = [
        "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf + 8]\n",
        "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + usage_msg]\n",
    ];
    for setup in allowed {
        let report = check_full(
            &lend(setup, "fill"),
            CpuFeatures::all(),
            &PARALLEL_SYSCALLS,
            None,
        )
        .await;
        let refused: Vec<String> = report
            .violations
            .iter()
            .filter(|violation| violation.rule == Rule::ExclusiveWrites)
            .map(ToString::to_string)
            .collect();
        assert!(refused.is_empty(), "{setup:?} was refused: {refused:#?}");
    }
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
        ("    rcpps xmm0, xmm1\n", "`rcpps` isn't in dialect 4"),
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
        ("    rcpss xmm0, xmm1\n", "`rcpss` isn't in dialect 4"),
        (
            "    vrsqrt14ps zmm0, zmm1\n",
            "`vrsqrt14ps` isn't in dialect 4",
        ),
        (
            "    vcvtph2ps xmm0, xmm1\n",
            "`vcvtph2ps` isn't in dialect 4",
        ),
        ("    dpps xmm0, xmm1, 0xff\n", "`dpps` isn't in dialect 4"),
        (
            "    ldmxcsr dword ptr [rsp]\n",
            "`ldmxcsr` isn't in dialect 4",
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

/// The AArch64 template passes every rule.
#[tokio::test]
async fn the_aarch64_template_passes() {
    let report = check(AARCH64_TEMPLATE).await;
    assert!(
        report.violations.is_empty(),
        "{}",
        report
            .violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // The rules read all of it, as they do x86-64's template.
    let facts = report.facts.expect("the rules ran");
    assert_eq!(facts.functions, 4);
    assert_eq!(facts.assertions, 11);
    assert!(facts.max_stack_bytes >= 48, "{facts:?}");
    assert!(report.unsafe_blocks.is_empty());
}

/// The AArch64 template with `old` replaced by `new`, once.
fn with_aarch64(old: &str, new: &str) -> String {
    assert!(
        AARCH64_TEMPLATE.contains(old),
        "{old:?} isn't in the AArch64 template"
    );
    AARCH64_TEMPLATE.replacen(old, new, 1)
}

/// Each way of breaking the AArch64 dialect is refused at its line, for its rule: each case
/// replaces `old` with `new`, and the violation is on the line that starts `at`.
#[tokio::test]
async fn the_aarch64_dialect_refuses_what_it_should() {
    let cases: [(&str, &str, &str, Rule, &str); 16] = [
        // The low bits of one symbol's address on another's page.
        (
            "    adrp x1, usage_msg\n    add x1, x1, :lo12:usage_msg\n",
            "    adrp x1, nan_msg\n    add x1, x1, :lo12:usage_msg\n",
            "    add x1, x1, :lo12:usage_msg",
            Rule::Dialect,
            "isn't right after `adrp` of usage_msg into the same register",
        ),
        // A `sub` sets no flags: the `b.ne` after it reads stale ones.
        (
            "    subs x3, x3, #1\n    b.ne .Lparse_loop\n",
            "    sub x3, x3, #1\n    b.ne .Lparse_loop\n",
            "    b.ne .Lparse_loop",
            Rule::Loops,
            "isn't the end of a counted loop",
        ),
        (
            "    stp x29, x30, [sp, #-16]!\n    mov x9, x0\n",
            "    stp x29, x30, [sp, #-16]!\n    sub sp, sp, #8\n",
            "    sub sp, sp, #8",
            Rule::Resources,
            "but sp stays a multiple of 16",
        ),
        (
            "    ldp x29, x30, [sp], #16\n    ret\n.Lwrite_i64_assert_min:",
            "    ldp x29, x30, [sp], #16\n    ret x3\n.Lwrite_i64_assert_min:",
            "    ret x3",
            Rule::Dialect,
            "returns through a register other than x30",
        ),
        (
            "    ldr x0, [sp, #16]\n",
            "    ldr x0, [x1, :lo12:usage_msg]\n",
            "    ldr x0, [x1, :lo12:usage_msg]",
            Rule::Dialect,
            "`:lo12:` goes on the `add` after `adrp`, never inside an address",
        ),
        (
            "    adrp x1, usage_msg\n    add x1, x1, :lo12:usage_msg\n",
            "    adr x1, usage_msg\n    nop\n",
            "    adr x1, usage_msg",
            Rule::Dialect,
            "adr isn't in the dialect",
        ),
        (
            "    ldr x0, [sp]\n",
            "    ldr x0, usage_msg\n",
            "    ldr x0, usage_msg",
            Rule::Dialect,
            "its address isn't in brackets",
        ),
        (
            "    mov x9, #0\n",
            "    mov x18, #0\n",
            "    mov x18, #0",
            Rule::Dialect,
            "uses x18",
        ),
        (
            "    mov x9, #0\n",
            "# a comment\n    mov x9, #0\n",
            "# a comment",
            Rule::Dialect,
            "`#` comments aren't in the AArch64 dialect",
        ),
        (
            "    add x2, x2, #1\n",
            "    add x2, x2, #5000\n",
            "    add x2, x2, #5000",
            Rule::Dialect,
            "isn't a number an add can hold",
        ),
        (
            "    svc #0\n",
            "    svc #1\n",
            "    svc #1",
            Rule::Dialect,
            "Linux takes system calls as `svc #0`",
        ),
        (
            "    bl write_all\n",
            "    blr x1\n",
            "    blr x1",
            Rule::Dialect,
            "calls through a register",
        ),
        (
            ".arch armv8-a\n",
            ".arch armv8.2-a\n",
            ".arch armv8.2-a",
            Rule::Dialect,
            "main.s must start with .arch armv8-a",
        ),
        // A number from a literal pool, an index widened from its low half, and a jump through
        // a register that isn't a checked jump table's.
        (
            "    mov x9, #0\n",
            "    ldr x9, =5000\n",
            "    ldr x9, =5000",
            Rule::Dialect,
            "a number from a literal pool isn't in the dialect",
        ),
        (
            "    ldrb w10, [x0, x2]\n",
            "    ldrb w10, [x0, w2, uxtw]\n",
            "    ldrb w10, [x0, w2, uxtw]",
            Rule::Dialect,
            "widens its index with uxtw",
        ),
        (
            ".Lparse_return:\n    ret\n",
            ".Lparse_return:\n    br x30\n",
            "    br x30",
            Rule::Indirection,
            "not through a checked jump table",
        ),
    ];
    for (old, new, at, rule, needle) in cases {
        let text = with_aarch64(old, new);
        let report = check(&text).await;
        assert_violation(&report, rule, line_of(&text, at), needle);
    }
}

/// Without its frame record, a function that calls can't return.
#[tokio::test]
async fn an_aarch64_function_that_calls_saves_x30() {
    let text = with_aarch64(
        "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
        "write_i64:\n",
    )
    .replacen(
        "    ldp x29, x30, [sp], #16\n    ret\n.Lwrite_i64_assert_min:",
        "    ret\n.Lwrite_i64_assert_min:",
        1,
    );
    let report = check(&text).await;
    let line = line_of(&text, "write_i64:\n");
    assert_violation(&report, Rule::Functions, line, "doesn't save x30");
}

/// A loop may end `sub` and `cbnz` on its counter, which reads no flags.
#[tokio::test]
async fn an_aarch64_loop_may_end_with_cbnz() {
    let text = with_aarch64(
        "    subs x3, x3, #1\n    b.ne .Lparse_loop\n",
        "    sub x3, x3, #1\n    cbnz x3, .Lparse_loop\n",
    );
    let report = check(&text).await;
    assert!(
        report.violations.is_empty(),
        "{:#?}",
        report
            .violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
}

/// What the rules say about AArch64 code is said in AArch64's words: no x86-64 register or
/// instruction in any message, whichever rule speaks.
#[tokio::test]
async fn the_aarch64_messages_say_aarch64() {
    let x86 = [
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rsp", "rbp", "eax", "edi", "[rip", "jae", "jnz",
        "jmp", "`test ", "lea ", "`call ", "# bound", "# unsafe", "# shared",
    ];
    let mutants = [
        // A result used unchecked, a system call and an exit status not named, a loop counter
        // set from a bare number, an index unchecked, memory below sp, an assertion's number in
        // the wrong register, and the canonical assert_fail changed.
        ("    tbnz x0, #63, .Lstart_not_a_number\n", "    nop\n"),
        ("    mov x8, #SYS_EXIT\n", "    mov x8, #93\n"),
        ("    mov w0, #EXIT_OK\n", "    mov w0, #0\n"),
        ("    mov x3, #MAX_CHARS\n", "    mov x3, #20\n"),
        ("    cmp x2, #MAX_CHARS\n", "    cmp x1, #MAX_CHARS\n"),
        ("    mov x9, #0\n", "    str x0, [sp, #-16]\n"),
        ("    mov w0, #6\n", "    mov w1, #6\n"),
        ("    mov w0, #1\n", "    mov w0, #9\n"),
    ];
    for (old, new) in mutants {
        let report = check(&with_aarch64(old, new)).await;
        assert!(!report.violations.is_empty(), "{new:?} breaks no rule");
        for violation in &report.violations {
            let text = violation.to_string();
            for word in x86 {
                assert!(
                    !text.contains(word),
                    "{new:?}: {word:?} in an AArch64 message: {text}"
                );
            }
        }
    }
}

/// An AArch64 program that reads with fill: cat over standard input of any length, in
/// tests/fixtures/linux-aarch64.
fn aarch64_with_fill() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linux-aarch64/cat/main.s"),
    )
    .expect("the fixture")
}

/// An AArch64 file program, the upcase example's spec compiled for AArch64: it reads one file
/// whole, capitalizes its letters with no branch on a byte, and writes another, passing every
/// rule, memory rules included.
#[tokio::test]
async fn an_aarch64_file_program_passes_every_rule() {
    let syscalls = [
        Syscall::Openat,
        Syscall::Close,
        Syscall::Read,
        Syscall::Write,
        Syscall::Exit,
    ];
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linux-aarch64/upcase/main.s"),
    )
    .expect("the fixture");
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
}

/// The AArch64 hypotenuse with its scale, 1000.0, a `.double` in .rodata that `ldr d2` reads,
/// rather than `scvtf` of a whole number.
pub(crate) fn from_rodata(text: &str) -> String {
    let changed = text
        .replacen(
            ".type assert_message, @object\n",
            ".type thousand, @object\nthousand:\n    .double 1000.0\n.size thousand, . - thousand\n\n\
             .type assert_message, @object\n",
            1,
        )
        .replacen(
            "    mov x2, #UNITS\n    scvtf d2, x2\n",
            "    adrp x2, thousand\n    add x2, x2, :lo12:thousand\n    ldr d2, [x2]\n",
            1,
        );
    assert!(
        changed.contains("    .double 1000.0\n") && changed.contains("    ldr d2, [x2]\n"),
        "{changed}"
    );
    changed
}

/// The AArch64 hypotenuse with both sides in one vector of two doubles: converted, divided by the
/// scale and squared lane by lane, then the two squares added by `faddp`. Each step rounds as the
/// scalar one does, so it computes the same double.
pub(crate) fn on_vectors(text: &str) -> String {
    let changed = text.replacen(
        "    scvtf d0, x0
    fdiv d0, d0, d2
    scvtf d1, x1
    fdiv d1, d1, d2
    \
         fmul d0, d0, d0
    fmul d1, d1, d1
    fadd d0, d0, d1
",
        "    fmov d0, x0
    mov v0.d[1], x1
    scvtf v0.2d, v0.2d
    dup v1.2d, v2.d[0]
    \
         fdiv v0.2d, v0.2d, v1.2d
    fmul v0.2d, v0.2d, v0.2d
    faddp d0, v0.2d
",
        1,
    );
    assert!(changed.contains("    faddp d0, v0.2d\n"), "{changed}");
    changed
}

/// An AArch64 floating-point program, the hypotenuse example's spec compiled for AArch64: its
/// doubles (scvtf, fdiv, fmul, fadd, fsqrt, frintn, fcvtzs) pass every rule, and so do they on
/// vectors. d8 to d15 are
/// callee-saved, so a function that changes one saves it, in stp pairs after the frame record and
/// restored in mirror order, as x19 to x28 are; half precision isn't in the dialect.
#[tokio::test]
async fn an_aarch64_float_program_passes_every_rule() {
    let syscalls = [Syscall::Write, Syscall::Exit];
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linux-aarch64/hypotenuse/main.s"),
    )
    .expect("the fixture");
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    let vectors = on_vectors(&text);
    let report = check_full(&vectors, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    // The scale kept in d8, which a caller expects back.
    let unsaved = text.replace(" d2", " d8");
    assert_ne!(unsaved, text);
    let report = check_full(&unsaved, CpuFeatures::all(), &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&unsaved, "print_hypotenuse:"),
        "print_hypotenuse changes d8 without saving it",
    );

    let opening = "print_hypotenuse:\n    stp x29, x30, [sp, #-16]!\n";
    let closing = ".Lprint_hypotenuse_return:\n    ldp x29, x30, [sp], #16\n";
    let saved = unsaved
        .replacen(
            opening,
            &format!("{opening}    stp d8, d9, [sp, #-16]!\n"),
            1,
        )
        .replacen(
            closing,
            ".Lprint_hypotenuse_return:\n    ldp d8, d9, [sp], #16\n    ldp x29, x30, [sp], #16\n",
            1,
        );
    let report = check_full(&saved, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    // Restored before the frame record rather than after it: not the mirror of the saves.
    let unmirrored = unsaved
        .replacen(
            opening,
            &format!("{opening}    stp d8, d9, [sp, #-16]!\n"),
            1,
        )
        .replacen(
            closing,
            ".Lprint_hypotenuse_return:\n    ldp x29, x30, [sp], #16\n    ldp d8, d9, [sp], #16\n",
            1,
        );
    let report = check_full(&unmirrored, CpuFeatures::all(), &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Functions,
        line_of(&unmirrored, "    ret\n.Lprint_hypotenuse_assert_side_a"),
        "don't mirror its pushes",
    );

    // The scale as a constant in .rodata, read through its address.
    let constant = from_rodata(&text);
    let report = check_full(&constant, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    let half = text.replacen("    fadd d0, d0, d1\n", "    fadd h0, h0, h1\n", 1);
    assert_ne!(half, text);
    let report = check_full(&half, CpuFeatures::all(), &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&half, "    fadd h0"),
        "`fadd h0, h0, h1`: operand 1 is a half-precision or byte register, which the dialect \
         doesn't have",
    );
}

/// The AArch64 vector template, x86-64's `shout` spec compiled with NEON, passes every rule. Its
/// 16-byte loads and stores are held to their buffer as x86-64's are, a V register a function
/// changes from v8 to v15 is saved as d8 to d15 are, and x18, widened indexes and half precision
/// on vectors stay out.
#[tokio::test]
async fn the_aarch64_vector_template_passes_every_rule() {
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets/linux-aarch64/vector-template.s"),
    )
    .expect("the template");
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    let cases = [
        (
            "    cmp x2, #LINE_BUF_LEN - CHUNK + 1\n    b.hs .Lshout_assert_index\n",
            "    cmp x2, #LINE_BUF_LEN - CHUNK + 2\n    b.hs .Lshout_assert_index\n",
            Rule::Indirection,
            "    cmp x2, #LINE_BUF_LEN - CHUNK + 2",
            "this check lets `ldr q0, [x1, x2]` reach byte 80 of line_buf, which has 80 bytes",
        ),
        (
            "    cmhi v4.16b, v0.16b, v1.16b\n",
            "    cmhi v8.16b, v0.16b, v1.16b\n",
            Rule::Functions,
            "shout:",
            "shout changes d8 without saving it",
        ),
        (
            "    movi v1.16b, #NEWLINE\n",
            "    dup v1.16b, w18\n",
            Rule::Dialect,
            "    dup v1.16b, w18",
            "uses x18",
        ),
        (
            "    ldr q0, [x1, x2]\n    cmeq",
            "    ldr q0, [x1, w2, uxtw]\n    cmeq",
            Rule::Dialect,
            "    ldr q0, [x1, w2, uxtw]",
            "widens its index with uxtw",
        ),
        (
            "    sub v0.16b, v0.16b, v4.16b\n",
            "    fadd v0.8h, v0.8h, v4.8h\n",
            Rule::Dialect,
            "    fadd v0.8h",
            "aren't one arrangement of floats",
        ),
    ];
    for (from, to, rule, at, needle) in cases {
        let changed = text.replacen(from, to, 1);
        assert_ne!(changed, text, "{to}");
        let report = check_full(&changed, CpuFeatures::all(), &syscalls, None).await;
        assert_violation(&report, rule, line_of(&changed, at), needle);
    }
}

/// The AArch64 fast wc, the skill's fast `wc` compiled with NEON: fill into 64 KiB, and one loop
/// that masks each 16-byte step to the bytes left, passes every rule.
#[tokio::test]
async fn the_aarch64_fast_wc_passes_every_rule() {
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s"),
    )
    .expect("the asset");
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");

    // A pointer moved by an access's writeback points into what it pointed into, as one moved
    // by `sub` does.
    let writeback = text
        .replacen(
            "    sub x1, x1, #1\n    strb w5, [x1]\n",
            "    strb w5, [x1, #-1]!\n",
            1,
        )
        .replacen(
            "    sub x0, x0, #1\n    strb w2, [x0]\n",
            "    strb w2, [x0, #-1]!\n",
            1,
        );
    assert_eq!(writeback.matches("#-1]!").count(), 2);
    let report = check_full(&writeback, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
}

/// lengua's AArch64 versions of the benchmark's programs pass every rule, as its x86-64 ones do:
/// what the benchmark measures is lengua's own code.
#[tokio::test]
async fn the_aarch64_benchmark_programs_pass_every_rule() {
    let files = [
        Syscall::Openat,
        Syscall::Close,
        Syscall::Read,
        Syscall::Write,
        Syscall::Exit,
    ];
    for (path, syscalls) in [
        (
            "tests/fixtures/linux-aarch64/add/main.s",
            &[Syscall::Write, Syscall::Exit][..],
        ),
        ("benches/programs/upcase/neon.s", &files[..]),
        (
            "benches/programs/shout/neon-one.s",
            &[Syscall::Read, Syscall::Write, Syscall::Exit][..],
        ),
    ] {
        let text =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .expect("the program");
        let report = check_full(&text, CpuFeatures::all(), syscalls, None).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{path}: {all:#?}");
    }
}

/// AArch64's fill passes every rule, and is held to its template as x86-64's is: its code and
/// its constants.
#[tokio::test]
async fn aarch64_fill_is_copied_word_for_word_with_its_constants() {
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let text = aarch64_with_fill();
    let report = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    let facts = report.facts.expect("the rules ran");
    assert_eq!((facts.functions, facts.assertions), (3, 7), "{facts:?}");
    assert!(facts.max_stack_bytes >= 32, "{facts:?}");

    let changed = text.replacen(
        "    subs x2, x2, x0\n    b.eq .Lfill_done\n",
        "    subs x2, x2, x0\n    cmp x2, #1\n    b.ls .Lfill_done\n",
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

/// A constant's expression means to lengua what it means to llvm-mc, on either CPU: the same
/// value for each operator and precedence, `>>` shifting in zeros; and what wouldn't fit in 64
/// bits is refused.
#[tokio::test]
async fn expressions_mean_what_they_mean_to_llvm_mc() {
    use object::{Object, ObjectSection};
    let expressions = [
        "1 + 6 & 3",
        "2 * 3 >> 1",
        "1 << 4 + 1",
        "6 | 1 & 3",
        "12 & 10 | 1",
        "1 - 2 & 3",
        "-1 & 3 + 1",
        "9 / 2 & 3",
        "7 & 6 * 1",
        "N >> 1",
        "N >> 60",
        "N << 3",
        "-N / 3",
        "N / 3",
        "(1 + 2) * 3",
        "A & 0xffff",
        "A >> 16",
        "M & 0xffff",
        "M >> 16 & 0xffff",
        "(M >> 32) & 0xffff",
        "M >> 48",
        "1 << 62",
        "N & -N",
        "0x7fffffffffffffff >> 1 | 1",
        "'a' | 0x20",
        "A>>16&0xffff",
    ];
    let constants: BTreeMap<String, source::Constant> =
        [("N", -8), ("A", 1_000_000_000), ("M", -1_000_000_000)]
            .into_iter()
            .map(|(name, value)| (name.to_owned(), source::Constant { value, line: 0 }))
            .collect();
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    for (profile, header) in [
        (Profile::LinuxX86_64, ".intel_syntax noprefix"),
        (Profile::LinuxAarch64, ".arch armv8-a"),
    ] {
        let mut text = format!("{header}\n.equ N, -8\n.equ A, 1000000000\n.equ M, -1000000000\n");
        text.push_str(".data\n");
        for expression in expressions {
            text.push_str(&format!(".quad {expression}\n"));
        }
        let folder = tempfile::tempdir().expect("temporary folder");
        let (source, object) = (folder.path().join("e.s"), folder.path().join("e.o"));
        std::fs::write(&source, text).expect("write the expressions");
        toolchain
            .assemble(profile, &source, &object)
            .await
            .unwrap_or_else(|error| panic!("assembling failed: {error:?}"));
        let bytes = std::fs::read(&object).expect("read the object");
        let file = object::File::parse(&*bytes).expect("an object file");
        let data = file
            .section_by_name(".data")
            .and_then(|section| section.data().ok())
            .expect("its data");
        let (words, _) = data.as_chunks::<8>();
        assert_eq!(words.len(), expressions.len());
        for (expression, word) in expressions.iter().zip(words) {
            assert_eq!(
                source::evaluate(expression, &constants),
                Ok(i64::from_le_bytes(*word)),
                "{expression} on {profile:?}"
            );
        }
    }
    for refused in ["1 << 63", "N << 61 << 3", "1 << 64", "1 >> -1", "A * A * A"] {
        assert!(
            source::evaluate(refused, &constants).is_err(),
            "{refused} is read"
        );
    }
}

/// The AArch64 calc fixture, the x86-64 one's spec compiled for AArch64: its `apply` picks its
/// operator's case through a checked jump table.
fn aarch64_calc() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linux-aarch64/calc/main.s"),
    )
    .expect("calc's AArch64 main.s")
}

/// The AArch64 calc with the first `old` replaced by `new`.
fn aarch64_calc_with(old: &str, new: &str) -> String {
    let calc = aarch64_calc();
    assert!(calc.contains(old), "{old:?} isn't in the AArch64 calc");
    calc.replacen(old, new, 1)
}

/// On AArch64 too, the one jump through a register is a checked jump table's: it passes, with
/// its cases as the jump's successors, and every other `br` is refused.
#[tokio::test]
async fn aarch64_jumps_go_through_registers_only_as_checked_tables() {
    let report = check(&aarch64_calc()).await;
    let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    assert!(report.passed(), "{all:#?}");
    let facts = report.facts.expect("the rules ran");
    assert_eq!(facts.tables.len(), 1, "{facts:?}");
    let (jump, cases) = facts.tables.iter().next().expect("one table");
    assert_eq!(cases.len(), 4, "{cases:?}");
    assert!(cases.iter().all(|case| case > jump), "{cases:?}");

    let jump = "    br x11\n";
    let entries = "    .long .Lapply_multiply - operations\n";
    let shape = "not through a checked jump table";
    for (text, rule, at, needle) in [
        // The index unchecked, or checked against a limit past the table.
        (
            aarch64_calc_with("    cmp x9, #OPERATORS\n    b.hs .Lapply_unknown\n", ""),
            Rule::Indirection,
            "    ldrsw x11, [x10, x9, lsl #2]\n",
            "isn't bounds-checked",
        ),
        (
            aarch64_calc_with("    cmp x9, #OPERATORS\n", "    cmp x9, #OPERATORS + 1\n"),
            Rule::Indirection,
            "    cmp x9, #OPERATORS + 1\n",
            "reach byte 19 of operations, which has 16 bytes",
        ),
        // Entries naming what isn't a case of this jump.
        (
            aarch64_calc_with(entries, "    .long .Lstart_exit - operations\n"),
            Rule::Indirection,
            jump,
            "names .Lstart_exit, which isn't a label of apply",
        ),
        (
            aarch64_calc_with(
                "    adrp x10, operations\n",
                ".Lapply_table:\n    adrp x10, operations\n",
            )
            .replacen(entries, "    .long .Lapply_table - operations\n", 1),
            Rule::Indirection,
            jump,
            "comes before the jump",
        ),
        // A table that can change, isn't only entries, or isn't the table's own distances.
        (
            aarch64_calc_with(
                "// apply's jump table",
                ".section .data\n// apply's jump table",
            )
            .replacen(
                ".size operations, . - operations\n",
                ".size operations, . - operations\n.section .rodata\n",
                1,
            ),
            Rule::Immutability,
            entries,
            "jump table operations is in writable data",
        ),
        (
            aarch64_calc_with(entries, "    .quad .Lapply_multiply - operations\n"),
            Rule::Indirection,
            "    .quad .Lapply_multiply - operations\n",
            "isn't a jump table's entry",
        ),
        (
            aarch64_calc_with(entries, "    .long .Lapply_multiply - nan_msg\n"),
            Rule::Indirection,
            "    .long .Lapply_multiply - nan_msg\n",
            "isn't a jump table's entry",
        ),
        (
            aarch64_calc_with(
                "    .long .Lapply_subtract - operations\n",
                "    .long .Lapply_subtract - operations\n    .long 0\n",
            ),
            Rule::Indirection,
            "    .long 0\n",
            "mixes numbers with its entries",
        ),
        // Any other jump through a register: the target changed after the add, another path
        // coming in, the entry shifted or added into another register, a load that doesn't
        // sign-extend, a widened index, or another symbol's address.
        (
            aarch64_calc_with(
                "    add x11, x11, x10\n    br x11\n",
                "    add x11, x11, x10\n    orr x11, x11, #1\n    br x11\n",
            ),
            Rule::Indirection,
            jump,
            shape,
        ),
        (
            aarch64_calc_with(
                "    add x11, x11, x10\n",
                ".Lapply_into:\n    add x11, x11, x10\n",
            ),
            Rule::Indirection,
            jump,
            shape,
        ),
        (
            aarch64_calc_with("    add x11, x11, x10\n", "    add x11, x11, x10, lsl #1\n"),
            Rule::Indirection,
            jump,
            shape,
        ),
        (
            aarch64_calc_with("    add x11, x11, x10\n", "    add x12, x11, x10\n"),
            Rule::Indirection,
            jump,
            shape,
        ),
        (
            aarch64_calc_with(
                "    ldrsw x11, [x10, x9, lsl #2]\n",
                "    ldr w11, [x10, x9, lsl #2]\n",
            ),
            Rule::Indirection,
            jump,
            shape,
        ),
        (
            aarch64_calc_with(
                "    ldrsw x11, [x10, x9, lsl #2]\n",
                "    ldrsw x11, [x10, w9, sxtw #2]\n",
            ),
            Rule::Dialect,
            "    ldrsw x11, [x10, w9, sxtw #2]\n",
            "widens its index with sxtw",
        ),
        (
            aarch64_calc_with(
                "    adrp x10, operations\n    add x10, x10, :lo12:operations\n",
                "    adrp x10, nan_msg\n    add x10, x10, :lo12:nan_msg\n",
            ),
            Rule::Indirection,
            jump,
            "jumps through nan_msg, which isn't a jump table",
        ),
    ] {
        let report = check(&text).await;
        assert_violation(&report, rule, line_of(&text, at), needle);
        for violation in &report.violations {
            let text = violation.to_string();
            assert!(
                !["jae", "rax", "rcx", "lea ", "movsxd"]
                    .iter()
                    .any(|word| text.contains(word)),
                "an x86-64 word in an AArch64 message: {text}"
            );
        }
    }
}

/// On AArch64 a number too wide for one `mov` is built 16 bits at a time: a `mov` of its low
/// bits and `movk`s of the rest, which together are `mov xR, NAME` when every line names the
/// same constant and they make all of it.
#[tokio::test]
async fn an_aarch64_constant_may_be_built_16_bits_at_a_time() {
    let counter = "    mov x3, #MAX_CHARS\n";
    for built in [
        "    mov x3, #MAX_CHARS & 0xffff\n    movk x3, #MAX_CHARS >> 16, lsl #16\n",
        "    mov w3, #MAX_CHARS & 0xffff\n    movk w3, #(MAX_CHARS >> 16) & 0xffff, lsl #16\n",
        "    mov x3, #MAX_CHARS & 0xffff\n    movk x3, #MAX_CHARS >> 16, lsl #16\n    movk x3, \
         #MAX_CHARS >> 32, lsl #32\n    movk x3, #MAX_CHARS >> 48, lsl #48\n",
    ] {
        let text = with_aarch64(counter, built);
        let report = check(&text).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{built:?}: {all:#?}");
    }
    for (built, why) in [
        (
            "    mov x3, #MAX_CHARS & 0xffff\n    movk x3, #1, lsl #16\n",
            "the pieces make 65556, not MAX_CHARS",
        ),
        (
            "    mov x3, #MAX_CHARS & 0xffff\n    movk x3, #MAX_DIGITS >> 16, lsl #16\n",
            "the pieces name two constants",
        ),
        (
            "    mov x3, #0\n    movk x3, #MAX_CHARS, lsl #0\n",
            "only the movk names the constant",
        ),
        (
            "    mov x3, #MAX_CHARS & 0xffff\n.Lparse_built:\n    movk x3, #MAX_CHARS >> 16, lsl \
             #16\n",
            "another path comes in between the pieces",
        ),
    ] {
        let text = with_aarch64(counter, built);
        let report = check(&text).await;
        // The violation is on the loop's first instruction.
        assert_violation(
            &report,
            Rule::Loops,
            line_of(&text, ".Lparse_loop:") + 1,
            "the loop counter isn't set by `mov xR, #NAME`",
        );
        assert!(!report.passed(), "{why}");
    }
}

/// A bounds check's limit in a register is held to the buffer when the code right before the
/// compare puts a constant there, as an immediate limit is: on AArch64, whose compares hold only
/// 12-bit numbers, that's how a larger buffer is checked. A limit that isn't a constant there is
/// the proof's to hold, as before.
#[tokio::test]
async fn a_bounds_check_against_a_constant_in_a_register_is_held_to_the_buffer() {
    let x86 = |limit: &str| {
        with(
            "    cmp ecx, OUT_BUF_LEN\n",
            &format!("    mov r11d, {limit}\n    cmp ecx, r11d\n"),
        )
    };
    let aarch64 = |set: &str| {
        with_aarch64(
            "    cmp x2, #OUT_BUF_LEN\n",
            &format!("{set}    cmp x2, x14\n"),
        )
    };
    for text in [
        x86("OUT_BUF_LEN"),
        aarch64("    mov x14, #OUT_BUF_LEN\n"),
        aarch64("    mov x14, #OUT_BUF_LEN & 0xffff\n    movk x14, #OUT_BUF_LEN >> 16, lsl #16\n"),
        // Not a constant: the proof holds it.
        aarch64("    mov x14, x9\n"),
    ] {
        let report = check(&text).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{all:#?}");
    }
    for (text, compare, access) in [
        (
            x86("1000"),
            "    cmp ecx, r11d\n",
            "`mov byte ptr [rsi + rcx], dl` reach byte 999 of out_buf, which has 21 bytes",
        ),
        (
            aarch64("    mov x14, #1000\n"),
            "    cmp x2, x14\n",
            "`strb w13, [x1, x2]` reach byte 999 of out_buf, which has 21 bytes",
        ),
        (
            aarch64("    mov x14, #OUT_BUF_LEN & 0xffff\n    movk x14, #1, lsl #16\n"),
            "    cmp x2, x14\n",
            "reach byte 65556 of out_buf, which has 21 bytes",
        ),
        // A negative limit is a huge one to the unsigned jump after the compare.
        (
            aarch64("    mov x14, #-1\n"),
            "    cmp x2, x14\n",
            "reach byte 18446744073709551614 of out_buf",
        ),
    ] {
        let report = check(&text).await;
        assert_violation(&report, Rule::Indirection, line_of(&text, compare), access);
    }
}
