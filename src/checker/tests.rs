//! The template passes; every rule and every known hazard has a failing fixture, each checked
//! for its rule and line.

use super::*;
use crate::targets::PerCpu;
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
    // A program in the edge form is given every operation: these helpers check a program's own
    // rules, not which operations its spec allows (`check_edge` says them).
    let ops: &[crate::edge::Op] = if crate::edge::form(text) == crate::edge::Form::Edge {
        &crate::edge::Op::ALL
    } else {
        &[]
    };
    check_allowing(text, features, syscalls, ops, threads).await
}

/// A program in the edge form, checked with the operations its spec allows.
async fn check_edge(text: &str, ops: &[crate::edge::Op]) -> Report {
    check_allowing(text, CpuFeatures::all(), &SYSCALLS, ops, None).await
}

async fn check_allowing(
    text: &str,
    features: CpuFeatures,
    syscalls: &[Syscall],
    ops: &[crate::edge::Op],
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
    let lengua = lengua_source(profile, text);
    check_program(
        &source,
        &bytes,
        Context {
            syscalls,
            ops,
            threads,
            lengua: lengua.as_ref(),
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

/// A tampering with the parsed main.s of a target, after the machine code was built from the real
/// one: the first line that starts with `line`, what is changed in its parsed form, and what the
/// check then says of the line.
struct Tamper {
    line: &'static str,
    change: fn(&mut source::Instruction),
    needle: &'static str,
}

/// Each target's tampering with `MAX_WRITES`, the number a loop counter starts from: main.s is
/// made to say 63 where the machine code says 64. x86-64 compares the operands of the parsed line,
/// AArch64 the instruction its text reads as.
const TAMPERS: PerCpu<Tamper> = PerCpu {
    x86_64: Tamper {
        line: "    mov r8, MAX_WRITES",
        change: |instruction| {
            instruction.operands[1] = source::Operand::Immediate {
                value: 63,
                name: None,
            };
        },
        needle: "is 63 as written but 64 in the machine code",
    },
    aarch64: Tamper {
        line: "    mov x10, #MAX_WRITES",
        change: |instruction| instruction.text = String::from("mov x10, #63"),
        needle: "in the machine code: as written it's",
    },
};

#[tokio::test]
async fn operands_must_mean_what_the_machine_does() {
    for (target, tamper) in TAMPERS.cells() {
        let program = *TEMPLATES.get(target);
        let (mut source, _) = check_layout(program);
        let line = line_of(program, tamper.line);
        for function in &mut source.functions {
            for item in &mut function.items {
                if let source::Item::Instruction(instruction) = item
                    && instruction.line == line
                {
                    (tamper.change)(instruction);
                }
            }
        }
        let folder = tempfile::tempdir().expect("temporary folder");
        let main = folder.path().join("main.s");
        std::fs::write(&main, program).expect("write main.s");
        let toolchain = Toolchain::discover().await.expect("toolchain");
        toolchain
            .assemble(target, &main, &folder.path().join("main.o"))
            .await
            .expect("assemble");
        toolchain
            .link(
                target,
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
                ops: &[],
                threads: None,
                lengua: None,
                features: CpuFeatures::all(),
                profile: target,
            },
        );
        assert_violation(&report, Rule::Dialect, line, tamper.needle);
    }
}

/// A number that doesn't fit its instruction's field is refused: the assembler would keep only
/// its low bits, so main.s wouldn't say what runs.
const NUMBERS: Fixtures = &[
    (
        "a number wider than a 32-bit field",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    mov edx, USAGE_MSG_LEN\n", "    mov edx, 4294967314\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    mov edx, 4294967314"),
                    "which doesn't fit in its 32 bits",
                )],
            },
            aarch64: Plan::NotApplicable(
                "an AArch64 instruction word has fixed fields (12-bit add numbers, 16-bit \
                 pieces of a move, bitmask numbers) and LLVM refuses a number that none holds \
                 instead of keeping its low bits, so no program assembles with a number cut short",
            ),
        },
    ),
    (
        "a number wider than an 8-bit field",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    cmp byte ptr [rdi], '-'\n",
                    "    cmp byte ptr [rdi], 301\n",
                )],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    cmp byte ptr [rdi], 301"),
                    "which doesn't fit in its 8 bits",
                )],
            },
            aarch64: Plan::NotApplicable(
                "an AArch64 compare takes a 12-bit number or a register, never a byte-sized \
                 field that the assembler could cut a wider number to: LLVM refuses the number",
            ),
        },
    ),
    (
        "the largest number a field holds, read as unsigned",
        PerCpu {
            x86_64: Plan::Quiet {
                edits: &[("    mov edx, USAGE_MSG_LEN\n", "    mov edx, 4294967295\n")],
                word: "doesn't fit",
            },
            aarch64: Plan::Quiet {
                edits: &[("    mov x2, #USAGE_MSG_LEN\n", "    mov x2, #65535\n")],
                word: "doesn't fit",
            },
        },
    ),
    (
        "minus one, read as signed",
        PerCpu {
            x86_64: Plan::Quiet {
                edits: &[("    mov edx, USAGE_MSG_LEN\n", "    mov edx, -1\n")],
                word: "doesn't fit",
            },
            aarch64: Plan::Quiet {
                edits: &[("    mov x2, #USAGE_MSG_LEN\n", "    mov x2, #-1\n")],
                word: "doesn't fit",
            },
        },
    ),
];

#[tokio::test]
async fn numbers_must_fit_their_field() {
    hold(&ON_THE_TEMPLATE, NUMBERS).await;
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
            calc_with(entries, "    .long .Lmain_exit - operations\n"),
            Rule::Indirection,
            jump,
            "names .Lmain_exit, which isn't a label of apply",
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
        ("examples/count/spec.md", "count/avx512.s"),
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
    // checks them for every feature. So is a program for AVX-512 on an x86-64 CPU without it
    // (GitHub's runners are a mixed pool): it can't run here, but its rules don't depend on the
    // computer.
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
        // The fast tiers of SHA-256, AES and the CRC-32C use instructions an older CPU lacks
        // (SHA, AES-NI, PCLMULQDQ): their rules don't depend on the computer either.
        let crypto = ["sha256rnds2", "aesenc", "pclmulqdq"]
            .iter()
            .any(|name| text.contains(name));
        let features = if (needs_more && !features.avx512bw) || crypto {
            CpuFeatures::all()
        } else {
            features
        };
        let report = check_allowing(&text, features, &spec.syscalls, &spec.ops, spec.threads).await;
        if !report.passed() {
            let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
            failed.push(format!("{}: {all:#?}", code.display()));
        }
    }
    assert!(programs.len() >= 25, "{} programs", programs.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The programs that still define `_start`, in the inline form: they make their own system calls
/// and carry their own `assert_fail`. The list only shrinks: a program that moves to the edge form
/// leaves it in the same commit, and none joins it. Round 2 ends with it empty and the form gone.
const INLINE_FORM: &[&str] = &[
    "benches/programs/count/avx512.s",
    "benches/programs/count/sve.s",
    "benches/programs/shout/avx512-one.s",
    "benches/programs/shout/avx512.s",
    "benches/programs/shout/neon-one.s",
    "benches/programs/upcase/avx512.s",
    "benches/programs/upcase/neon.s",
    "examples/aes/main.s",
    "examples/archive/main.s",
    "examples/count/main.s",
    "examples/dice/main.s",
    "examples/echo/main.s",
    "examples/fetch/main.s",
    "examples/sha256/main.s",
    "examples/sort/main.s",
    "examples/upcase/main.s",
    "plugin/skills/compile/assets/fast/aes-ni/main.s",
    "plugin/skills/compile/assets/fast/crc32c/main.s",
    "plugin/skills/compile/assets/fast/crc32c-pclmul/main.s",
    "plugin/skills/compile/assets/fast/histogram/main.s",
    "plugin/skills/compile/assets/fast/sha256-ni/main.s",
    "plugin/skills/compile/assets/fast/wc-avx512/main.s",
    "plugin/skills/compile/assets/fast/wc/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/aes-aes/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/crc32c/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/histogram/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/sha256-sha2/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s",
    "plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s",
    "plugin/skills/compile/assets/linux-aarch64/parallel-template.s",
    "plugin/skills/compile/assets/linux-aarch64/server-template.s",
    "plugin/skills/compile/assets/linux-aarch64/stream-template.s",
    "plugin/skills/compile/assets/linux-aarch64/tally-template.s",
    "plugin/skills/compile/assets/linux-aarch64/terminal-template.s",
    "plugin/skills/compile/assets/linux-aarch64/vector-template.s",
    "plugin/skills/compile/assets/parallel-template.s",
    "plugin/skills/compile/assets/server-template.s",
    "plugin/skills/compile/assets/stream-template.s",
    "plugin/skills/compile/assets/tally-template.s",
    "plugin/skills/compile/assets/terminal-template.s",
    "plugin/skills/compile/assets/vector-template.s",
    "tests/fixtures/amounts/main.s",
    "tests/fixtures/clock/main.s",
    "tests/fixtures/dates/main.s",
    "tests/fixtures/days/main.s",
    "tests/fixtures/lines/main.s",
    "tests/fixtures/linux-aarch64/aes/main.s",
    "tests/fixtures/linux-aarch64/amounts/main.s",
    "tests/fixtures/linux-aarch64/archive/main.s",
    "tests/fixtures/linux-aarch64/clock/main.s",
    "tests/fixtures/linux-aarch64/count/main.s",
    "tests/fixtures/linux-aarch64/dates/main.s",
    "tests/fixtures/linux-aarch64/days/main.s",
    "tests/fixtures/linux-aarch64/dice/main.s",
    "tests/fixtures/linux-aarch64/echo/main.s",
    "tests/fixtures/linux-aarch64/fetch/main.s",
    "tests/fixtures/linux-aarch64/lines/main.s",
    "tests/fixtures/linux-aarch64/nap/main.s",
    "tests/fixtures/linux-aarch64/padded/main.s",
    "tests/fixtures/linux-aarch64/pairs/main.s",
    "tests/fixtures/linux-aarch64/second/main.s",
    "tests/fixtures/linux-aarch64/sha256/main.s",
    "tests/fixtures/linux-aarch64/sort/main.s",
    "tests/fixtures/linux-aarch64/sweep/main.s",
    "tests/fixtures/linux-aarch64/total/main.s",
    "tests/fixtures/linux-aarch64/upcase/main.s",
    "tests/fixtures/linux-aarch64/words/main.s",
    "tests/fixtures/nap/main.s",
    "tests/fixtures/padded/main.s",
    "tests/fixtures/pairs/main.s",
    "tests/fixtures/second/main.s",
    "tests/fixtures/sweep/main.s",
    "tests/fixtures/total/main.s",
    "tests/fixtures/words/main.s",
];

/// Every `.s` file under `folder` that defines `_start`, by its path from the repository's root.
fn defining_start(root: &std::path::Path, folder: &std::path::Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(folder).expect("a folder of programs") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            defining_start(root, &path, found);
        } else if path.extension().is_some_and(|extension| extension == "s")
            && std::fs::read_to_string(&path)
                .is_ok_and(|text| text.lines().any(|line| line == "_start:"))
        {
            let relative = path.strip_prefix(root).expect("under the root");
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// The inline form is for the programs not yet moved to the edge form, and the list says which.
#[test]
fn the_inline_form_is_only_for_the_programs_not_yet_moved() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for folder in [
        "examples",
        "tests/fixtures",
        "plugin/skills/compile/assets",
        "benches/programs",
    ] {
        defining_start(root, &root.join(folder), &mut found);
    }
    let found: std::collections::BTreeSet<&str> = found.iter().map(String::as_str).collect();
    let listed: std::collections::BTreeSet<&str> = INLINE_FORM.iter().copied().collect();
    let joined: Vec<&&str> = found.difference(&listed).collect();
    let moved: Vec<&&str> = listed.difference(&found).collect();
    assert!(
        joined.is_empty() && moved.is_empty(),
        "in the inline form and not on the list (the list only shrinks, so this one must move to \
         the edge form): {joined:?}; on the list and no longer in the inline form (take it off): \
         {moved:?}"
    );
}

/// Where a planted bug's violation is reported.
#[derive(Debug, Clone, Copy)]
enum At {
    /// On the first line that starts with this text.
    Line(&'static str),
    /// On the `ret` right after the first line that starts with this text: an address that is
    /// returned is reported where it is returned.
    Ret(&'static str),
    /// On the line right after the first that starts with this text: a system call is reported
    /// on the instruction that makes it, after the one that names it.
    After(&'static str),
    /// On no line of its own: the stack's size is the program's.
    Nowhere,
}

/// A bug planted in a target's template: `old`, which the template has, becomes `new`, and the
/// rules must report a violation of `rule` whose text contains `needle`, `at` the line.
#[derive(Debug, Clone, Copy)]
struct Planted {
    old: &'static str,
    new: &'static str,
    at: At,
    rule: Rule,
    needle: &'static str,
}

/// What one target does with a planted bug.
#[derive(Debug, Clone, Copy)]
enum Cell {
    /// The bug is planted in the target's template.
    Plant(Planted),
    /// The bug can't be written on this CPU, for this reason of its hardware.
    NotApplicable(&'static str),
}

/// The bugs planted for the memory rules, each with a cell for every target: a target added to
/// lengua breaks this table until its cells are filled, and a row can't be left out of one
/// target without saying why that CPU can't have it.
const PLANTED: &[(&str, PerCpu<Cell>)] = &[
    (
        "a bounds check of part of the index (M2)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lparse_loop:  # bound: 20\n    cmp rcx, MAX_CHARS\n",
                new: ".Lparse_loop:  # bound: 20\n    cmp ecx, MAX_CHARS\n",
                at: At::Line("    cmp ecx, MAX_CHARS"),
                rule: Rule::Bounds,
                needle: "compares only the low 32 bits of rcx",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lparse_loop:  // bound: 20\n    cmp x2, #MAX_CHARS\n",
                new: ".Lparse_loop:  // bound: 20\n    cmp w2, #MAX_CHARS\n",
                at: At::Line("    cmp w2, #MAX_CHARS"),
                rule: Rule::Bounds,
                needle: "compares only the low 32 bits",
            }),
        },
    ),
    (
        "a bounds check with a negative limit (M2)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lparse_loop:  # bound: 20\n    cmp rcx, MAX_CHARS\n",
                new: ".Lparse_loop:  # bound: 20\n    cmp rcx, -1\n",
                at: At::Line("    cmp rcx, -1"),
                rule: Rule::Bounds,
                needle: "compares with -1, which an unsigned jump reads as a huge number",
            }),
            aarch64: Cell::NotApplicable(
                "AArch64's `cmp` takes an unsigned 12-bit number, so a limit of -1 can't be \
                 written as one: the assembler turns `cmp x2, #-1` into `cmn x2, #1`, an add",
            ),
        },
    ),
    (
        "a bit test whose offset is a register (M2)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "    movzx edx, byte ptr [rdi + rcx]\n",
                new: "    movzx edx, byte ptr [rdi + rcx]\n    bt qword ptr [rdi], rcx\n",
                at: At::Line("    bt qword ptr [rdi], rcx"),
                rule: Rule::Bounds,
                needle: "tests a bit in memory at an offset in a register",
            }),
            aarch64: Cell::NotApplicable(
                "AArch64 has no instruction that tests a bit in memory at a register's offset: its \
                 loads and stores take byte addresses, and `tbz` and `tbnz` test a bit of a \
                 register",
            ),
        },
    ),
    (
        "an address made from a plain number (M1)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lstart_usage:\n",
                new: ".Lstart_usage:\n    mov esi, MAX_WRITES\n    mov byte ptr [rsi], 0\n",
                at: At::Line("    mov byte ptr [rsi], 0"),
                rule: Rule::Owners,
                needle: "holds a plain number here, not a pointer",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lstart_usage:\n",
                new: ".Lstart_usage:\n    mov x1, #MAX_WRITES\n    strb wzr, [x1]\n",
                at: At::Line("    strb wzr, [x1]"),
                rule: Rule::Owners,
                needle: "holds a plain number here, not a pointer",
            }),
        },
    ),
    (
        "a constant offset one byte past a symbol (M2)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lstart_write_failed:\n",
                new: concat!(
                    ".Lstart_write_failed:\n",
                    "    lea rsi, [rip + out_buf]\n",
                    "    mov byte ptr [rsi + OUT_BUF_LEN], 0\n"
                ),
                at: At::Line("    mov byte ptr [rsi + OUT_BUF_LEN], 0"),
                rule: Rule::Bounds,
                needle: "reaches byte 21 of out_buf, which has 21 bytes",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lstart_write_failed:\n",
                new: concat!(
                    ".Lstart_write_failed:\n",
                    "    adrp x1, out_buf\n",
                    "    add x1, x1, :lo12:out_buf\n",
                    "    strb wzr, [x1, #OUT_BUF_LEN]\n"
                ),
                at: At::Line("    strb wzr, [x1, #OUT_BUF_LEN]"),
                rule: Rule::Bounds,
                needle: "reaches byte 21 of out_buf, which has 21 bytes",
            }),
        },
    ),
    (
        "a direct reference one byte past a symbol (M2): the next symbol starts there",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lstart_write_failed:\n",
                new: concat!(
                    ".Lstart_write_failed:\n",
                    "    movzx eax, byte ptr [rip + usage_msg + USAGE_MSG_LEN]\n"
                ),
                at: At::Line("    movzx eax, byte ptr [rip + usage_msg + USAGE_MSG_LEN]"),
                rule: Rule::Scope,
                needle: "reaches 1 byte past the end of usage_msg, which has 18 bytes",
            }),
            aarch64: Cell::NotApplicable(
                "AArch64 has no access relative to the program counter with a displacement: an \
                 address is `adrp` of the bare symbol and `add :lo12:`, and the offset is the \
                 load's, which the next row plants",
            ),
        },
    ),
    (
        "a pointer just past a symbol's end, as main.s makes it (M2)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lstart_write_failed:\n",
                new: concat!(
                    ".Lstart_write_failed:\n",
                    "    lea rsi, [rip + usage_msg + USAGE_MSG_LEN]\n",
                    "    movzx eax, byte ptr [rsi]\n"
                ),
                at: At::Line("    movzx eax, byte ptr [rsi]"),
                rule: Rule::Bounds,
                needle: "reaches byte 18 of usage_msg, which has 18 bytes",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lstart_write_failed:\n",
                new: concat!(
                    ".Lstart_write_failed:\n",
                    "    adrp x1, usage_msg\n",
                    "    add x1, x1, :lo12:usage_msg\n",
                    "    ldrb w2, [x1, #USAGE_MSG_LEN]\n"
                ),
                at: At::Line("    ldrb w2, [x1, #USAGE_MSG_LEN]"),
                rule: Rule::Bounds,
                needle: "reaches byte 18 of usage_msg, which has 18 bytes",
            }),
        },
    ),
    (
        "an address in the frame, returned (M3)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lparse_return:\n    ret\n",
                new: ".Lparse_return:\n    lea rdx, [rsp]\n    ret\n",
                at: At::Ret("    lea rdx, [rsp]"),
                rule: Rule::Lifetimes,
                needle: "returns an address in its own frame in rdx",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    ldp x29, x30, [sp], #16\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    mov x1, sp\n",
                    "    ldp x29, x30, [sp], #16\n"
                ),
                at: At::Ret("    ldp x29, x30, [sp], #16"),
                rule: Rule::Lifetimes,
                needle: "returns an address in its own frame",
            }),
        },
    ),
    (
        "an address in the frame, stored where it outlives the function (M3)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lparse_return:\n    ret\n",
                new: concat!(
                    ".Lparse_return:\n",
                    "    lea rcx, [rsp]\n",
                    "    mov qword ptr [rip + out_buf], rcx\n",
                    "    ret\n"
                ),
                at: At::Line("    mov qword ptr [rip + out_buf], rcx"),
                rule: Rule::Lifetimes,
                needle: "an address in this function's frame, outside the frame",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    ldp x29, x30, [sp], #16\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    mov x1, sp\n",
                    "    adrp x2, out_buf\n",
                    "    add x2, x2, :lo12:out_buf\n",
                    "    str x1, [x2]\n",
                    "    ldp x29, x30, [sp], #16\n"
                ),
                at: At::Line("    str x1, [x2]"),
                rule: Rule::Lifetimes,
                needle: "an address in this function's frame, outside the frame",
            }),
        },
    ),
    (
        "a read past the frame, into the caller's (M3)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "parse_i64:\n",
                new: "parse_i64:\n    mov r10, qword ptr [rsp + 8]\n",
                at: At::Line("    mov r10, qword ptr [rsp + 8]"),
                rule: Rule::Lifetimes,
                needle: "into its return address or its caller's frame",
            }),
            aarch64: Cell::Plant(Planted {
                old: "parse_i64:\n",
                new: "parse_i64:\n    ldr x10, [sp, #8]\n",
                at: At::Line("    ldr x10, [sp, #8]"),
                rule: Rule::Lifetimes,
                needle: "into its return address or its caller's frame",
            }),
        },
    ),
    (
        "a pointer to a known place in the frame, written past the frame (M3)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    add rsp, 8\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    lea rcx, [rsp + 8]\n",
                    "    mov byte ptr [rcx], 0\n",
                    "    add rsp, 8\n"
                ),
                at: At::Line("    mov byte ptr [rcx], 0"),
                rule: Rule::Lifetimes,
                needle: "reaches past this function's frame of 8 bytes, into its return address",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    ldp x29, x30, [sp], #16\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    add x1, sp, #16\n",
                    "    strb wzr, [x1]\n",
                    "    ldp x29, x30, [sp], #16\n"
                ),
                at: At::Line("    strb wzr, [x1]"),
                rule: Rule::Lifetimes,
                needle: "reaches past this function's frame of 16 bytes",
            }),
        },
    ),
    (
        "the return address written, where the function started (M3)",
        PerCpu {
            // write_all's return address is at [rsp] when it starts.
            x86_64: Cell::Plant(Planted {
                old: "write_all:\n",
                new: "write_all:\n    mov qword ptr [rsp], 0\n",
                at: At::Line("    mov qword ptr [rsp], 0"),
                rule: Rule::Lifetimes,
                needle: "into its return address",
            }),
            // The return address a function saved in its frame record, written through memory.
            aarch64: Cell::Plant(Planted {
                old: "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
                new: concat!(
                    "write_i64:\n",
                    "    stp x29, x30, [sp, #-16]!\n",
                    "    str xzr, [sp, #8]\n"
                ),
                at: At::Line("    str xzr, [sp, #8]"),
                rule: Rule::Lifetimes,
                needle: "return address",
            }),
        },
    ),
    (
        "the return address written through a pointer to it (M3)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "write_all:\n",
                new: "write_all:\n    lea rcx, [rsp]\n    mov qword ptr [rcx], 0\n",
                at: At::Line("    mov qword ptr [rcx], 0"),
                rule: Rule::Lifetimes,
                needle: "into its return address",
            }),
            aarch64: Cell::Plant(Planted {
                old: "write_i64:\n    stp x29, x30, [sp, #-16]!\n",
                new: concat!(
                    "write_i64:\n",
                    "    stp x29, x30, [sp, #-16]!\n",
                    "    add x1, sp, #8\n",
                    "    str xzr, [x1]\n"
                ),
                at: At::Line("    str xzr, [x1]"),
                rule: Rule::Lifetimes,
                needle: "return address",
            }),
        },
    ),
    (
        "a frame slot read before anything writes it (M5)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "parse_i64:\n",
                new: concat!(
                    "parse_i64:\n",
                    "    sub rsp, 8\n",
                    "    mov r10, qword ptr [rsp]\n",
                    "    add rsp, 8\n"
                ),
                at: At::Line("    mov r10, qword ptr [rsp]"),
                rule: Rule::Initialization,
                needle: "reads the frame at rsp + 0, which some path reaches without writing",
            }),
            aarch64: Cell::Plant(Planted {
                old: "parse_i64:\n",
                new: concat!(
                    "parse_i64:\n",
                    "    sub sp, sp, #16\n",
                    "    ldr x10, [sp]\n",
                    "    add sp, sp, #16\n"
                ),
                at: At::Line("    ldr x10, [sp]"),
                rule: Rule::Initialization,
                needle: "which some path reaches without writing",
            }),
        },
    ),
    (
        "what a function returns, followed through the call (M1): parse_i64 returns a number",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "    call parse_i64\n    test rax, rax\n",
                new: concat!(
                    "    call parse_i64\n",
                    "    mov byte ptr [rax], 0\n",
                    "    test rax, rax\n"
                ),
                at: At::Line("    mov byte ptr [rax], 0"),
                rule: Rule::Owners,
                needle: "uses rax as an address, but it holds a plain number here",
            }),
            aarch64: Cell::Plant(Planted {
                old: "    bl parse_i64\n    tbnz x0, #63, .Lstart_not_a_number\n",
                new: concat!(
                    "    bl parse_i64\n",
                    "    strb wzr, [x0]\n",
                    "    tbnz x0, #63, .Lstart_not_a_number\n"
                ),
                at: At::Line("    strb wzr, [x0]"),
                rule: Rule::Owners,
                needle: "as an address, but it holds a plain number here",
            }),
        },
    ),
    (
        "the program's arguments written (M4): they are only read",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "    mov rdi, qword ptr [rsp + 16]\n",
                new: "    mov rdi, qword ptr [rsp + 16]\n    mov byte ptr [rdi], 0\n",
                at: At::Line("    mov byte ptr [rdi], 0"),
                rule: Rule::ExclusiveWrites,
                needle: "writes into the program's arguments, which it only reads",
            }),
            aarch64: Cell::Plant(Planted {
                old: "    cbz x0, .Lstart_assert_argv\n",
                new: "    cbz x0, .Lstart_assert_argv\n    strb wzr, [x0]\n",
                at: At::Line("    strb wzr, [x0]"),
                rule: Rule::ExclusiveWrites,
                needle: "writes into the program's arguments, which it only reads",
            }),
        },
    ),
    (
        "a frame lent to a function that only reads it is still unwritten (M5)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    add rsp, 8\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    mov rdi, rsp\n",
                    "    call parse_i64\n",
                    "    mov r10, qword ptr [rsp]\n",
                    "    add rsp, 8\n"
                ),
                at: At::Line("    mov r10, qword ptr [rsp]"),
                rule: Rule::Initialization,
                needle: "reads the frame at rsp + 0, which some path reaches without writing",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lwrite_i64_return:\n    ldp x29, x30, [sp], #16\n",
                new: concat!(
                    ".Lwrite_i64_return:\n",
                    "    sub sp, sp, #16\n",
                    "    mov x0, sp\n",
                    "    bl parse_i64\n",
                    "    ldr x10, [sp]\n",
                    "    add sp, sp, #16\n",
                    "    ldp x29, x30, [sp], #16\n"
                ),
                at: At::Line("    ldr x10, [sp]"),
                rule: Rule::Initialization,
                needle: "which some path reaches without writing",
            }),
        },
    ),
    (
        "read-only data written through a register (M6)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: ".Lstart_usage:\n",
                new: concat!(
                    ".Lstart_usage:\n",
                    "    lea rsi, [rip + usage_msg]\n",
                    "    mov byte ptr [rsi], 0\n"
                ),
                at: At::Line("    mov byte ptr [rsi], 0"),
                rule: Rule::Immutability,
                needle: "writes to read-only usage_msg",
            }),
            aarch64: Cell::Plant(Planted {
                old: ".Lstart_usage:\n",
                new: concat!(
                    ".Lstart_usage:\n",
                    "    adrp x1, usage_msg\n",
                    "    add x1, x1, :lo12:usage_msg\n",
                    "    strb wzr, [x1]\n"
                ),
                at: At::Line("    strb wzr, [x1]"),
                rule: Rule::Immutability,
                needle: "writes to read-only usage_msg",
            }),
        },
    ),
    (
        "a frame bigger than the stack's limit (M8)",
        PerCpu {
            x86_64: Cell::Plant(Planted {
                old: "_start:\n",
                new: "_start:\n    sub rsp, 2097152\n",
                at: At::Nowhere,
                rule: Rule::StackSize,
                needle: "bytes of stack, more than 1048576",
            }),
            aarch64: Cell::Plant(Planted {
                old: "_start:\n",
                new: "_start:\n    sub sp, sp, #1024, lsl #12\n",
                at: At::Nowhere,
                rule: Rule::StackSize,
                needle: "more than 1048576",
            }),
        },
    ),
];

/// Memory safety on every target: a bug planted for each memory rule is found, on its line. Each
/// row of [`PLANTED`] replaces text of a target's template, where its CPU can have the bug, and
/// wants the violation the row names.
#[tokio::test]
async fn memory_rules_find_each_planted_bug() {
    for (name, cells) in PLANTED {
        for (target, cell) in cells.cells() {
            let Cell::Plant(planted) = cell else {
                continue;
            };
            let template = match target {
                Profile::LinuxX86_64 => TEMPLATE,
                Profile::LinuxAarch64 => AARCH64_TEMPLATE,
            };
            assert!(
                template.contains(planted.old),
                "{name} on {}: {:?} isn't in the template",
                target.name(),
                planted.old
            );
            let text = template.replacen(planted.old, planted.new, 1);
            let report = check(&text).await;
            let line = match planted.at {
                At::Nowhere => None,
                At::Line(at) => Some(line_of(&text, at)),
                At::Ret(at) | At::After(at) => Some(line_of(&text, at) + 1),
            };
            let found = report.violations.iter().any(|violation| {
                violation.rule == planted.rule
                    && (line.is_none() || violation.line == line)
                    && violation.to_string().contains(planted.needle)
            });
            let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
            assert!(
                found,
                "{name} on {}: no {:?} violation containing {:?} in {all:#?}",
                target.name(),
                planted.rule,
                planted.needle
            );
        }
    }
}

/// Every planted bug is planted on a target or says why that CPU can't have it, and no target
/// plants fewer than the tests this table replaced did.
#[test]
fn every_planted_bug_is_planted_or_excused_on_every_target() {
    for (name, cells) in PLANTED {
        let mut planted = 0;
        for (target, cell) in cells.cells() {
            match cell {
                Cell::Plant(_) => planted += 1,
                Cell::NotApplicable(why) => {
                    assert!(why.len() > 40, "{name}: say why {} can't", target.name());
                }
            }
        }
        assert!(planted > 0, "{name}: no target plants it");
    }
    let planted = |target: Profile| {
        PLANTED
            .iter()
            .filter(|(_, cells)| matches!(cells.get(target), Cell::Plant(_)))
            .count()
    };
    // The 17 bugs x86-64's test planted, and the 14 AArch64's did with its stack-limit test.
    assert!(planted(Profile::LinuxX86_64) >= 17);
    assert!(planted(Profile::LinuxAarch64) >= 15);
}

/// A change to a program: the first `old` in it becomes `new`.
type Edit = (&'static str, &'static str);

/// A violation a fixture must cause: its rule, where it is reported, and a word of its text.
type Finding = (Rule, At, &'static str);

/// What one target does with a fixture of a rule.
#[derive(Debug, Clone, Copy)]
enum Plan {
    /// The target's program with `edits` made, in order, breaks the rules in each of `finds`.
    Breaks {
        edits: &'static [Edit],
        finds: &'static [Finding],
    },
    /// The target's program with `edits` made still follows every rule, and holds this many
    /// blocks marked unsafe.
    Follows {
        edits: &'static [Edit],
        unsafe_blocks: usize,
    },
    /// The target's program with `edits` made has no violation whose text has this word.
    Quiet {
        edits: &'static [Edit],
        word: &'static str,
    },
    /// The fixture can't be written on this CPU, for this reason of its hardware.
    NotApplicable(&'static str),
}

/// A table of fixtures, each row a way of breaking a rule with a cell for every target: a target
/// added to lengua breaks every table until its cells are written.
type Fixtures = &'static [(&'static str, PerCpu<Plan>)];

/// The programs a table's fixtures are made of: one for each target, the system calls its spec
/// allows, and the threads.
struct Programs {
    texts: PerCpu<&'static str>,
    syscalls: &'static [Syscall],
    threads: Option<u32>,
}

/// Each target's main template.
const TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: TEMPLATE,
    aarch64: AARCH64_TEMPLATE,
};

/// Each target's parallel template.
const PARALLEL_TEMPLATES: PerCpu<&str> = PerCpu {
    x86_64: PARALLEL_TEMPLATE,
    aarch64: PARALLEL_TEMPLATE_AARCH64,
};

/// The main templates, with what their spec allows.
const ON_THE_TEMPLATE: Programs = Programs {
    texts: TEMPLATES,
    syscalls: &SYSCALLS,
    threads: None,
};

/// The parallel templates, with what their spec allows.
const ON_THE_PARALLEL_TEMPLATE: Programs = Programs {
    texts: PARALLEL_TEMPLATES,
    syscalls: &PARALLEL_SYSCALLS,
    threads: Some(16),
};

/// `base` with each edit made in turn, or the first `old` it doesn't have.
fn edited(base: &str, edits: &[Edit]) -> Result<String, String> {
    let mut text = base.to_owned();
    for &(old, new) in edits {
        if !text.contains(old) {
            return Err(format!("{old:?} isn't in the program"));
        }
        text = text.replacen(old, new, 1);
    }
    Ok(text)
}

/// The line a finding is reported on in `text`: none for a finding that is the program's.
fn expected_line(text: &str, at: At) -> Result<Option<usize>, String> {
    let find = |needle: &str| {
        text.find(needle)
            .map(|position| text[..position].matches('\n').count() + 1)
            .ok_or_else(|| format!("{needle:?} isn't in the edited program"))
    };
    match at {
        At::Nowhere => Ok(None),
        At::Line(needle) => find(needle).map(Some),
        At::Ret(needle) | At::After(needle) => find(needle).map(|line| Some(line + 1)),
    }
}

/// Why `report` lacks the violation `finding` asks for in `text`, if it does.
fn unmet(text: &str, report: &Report, finding: &Finding) -> Option<String> {
    let (rule, at, needle) = *finding;
    let line = match expected_line(text, at) {
        Ok(line) => line,
        Err(why) => return Some(why),
    };
    let found = report.violations.iter().any(|violation| {
        violation.rule == rule
            && (line.is_none() || violation.line == line)
            && violation.to_string().contains(needle)
    });
    (!found).then(|| {
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        format!("no {rule:?} violation containing {needle:?} on {line:?} in {all:#?}")
    })
}

/// `target`'s program with `edits` made, and what the rules say of it, or why the edits can't be
/// made.
async fn checked(
    programs: &Programs,
    target: Profile,
    edits: &[Edit],
) -> Result<(String, Report), String> {
    let text = edited(programs.texts.get(target), edits)?;
    let features = CpuFeatures::all();
    let report = check_full(&text, features, programs.syscalls, programs.threads).await;
    Ok((text, report))
}

/// Holds every fixture of `table` to its cell on every target: the program with the cell's edits
/// made is checked, and each violation the cell names must be in the report. Every miss of every
/// row is reported at once.
async fn hold(programs: &Programs, table: Fixtures) {
    let mut missed = Vec::new();
    for (name, cells) in table {
        for (target, plan) in cells.cells() {
            let on = format!("{name} on {}", target.name());
            match plan {
                Plan::NotApplicable(_) => {}
                Plan::Follows {
                    edits,
                    unsafe_blocks,
                } => match checked(programs, target, edits).await {
                    Err(why) => missed.push(format!("{on}: {why}")),
                    Ok((_, report)) => {
                        if !report.passed() || report.unsafe_blocks.len() != *unsafe_blocks {
                            let all: Vec<String> =
                                report.violations.iter().map(ToString::to_string).collect();
                            missed.push(format!(
                                "{on}: broke a rule, or holds {} unsafe blocks, not \
                                 {unsafe_blocks}: {all:#?}",
                                report.unsafe_blocks.len()
                            ));
                        }
                    }
                },
                Plan::Breaks { edits, finds } => match checked(programs, target, edits).await {
                    Err(why) => missed.push(format!("{on}: {why}")),
                    Ok((text, report)) => {
                        for finding in *finds {
                            if let Some(problem) = unmet(&text, &report, finding) {
                                missed.push(format!("{on}: {problem}"));
                            }
                        }
                    }
                },
                Plan::Quiet { edits, word } => match checked(programs, target, edits).await {
                    Err(why) => missed.push(format!("{on}: {why}")),
                    Ok((_, report)) => {
                        let said: Vec<String> = report
                            .violations
                            .iter()
                            .map(ToString::to_string)
                            .filter(|violation| violation.contains(*word))
                            .collect();
                        if !said.is_empty() {
                            missed.push(format!("{on}: a violation says {word:?}: {said:#?}"));
                        }
                    }
                },
            }
        }
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// The stack pointer: nothing below it, moved only by the frame's own instructions, and a call
/// made with the stack where the calling convention keeps it.
const STACK: Fixtures = &[
    (
        "memory below the stack pointer",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    test rsi, rsi\n",
                    "    mov qword ptr [rsp - 8], rax\n    test rsi, rsi\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("    mov qword ptr [rsp - 8], rax"),
                    "uses memory below rsp",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cbz x1, .Lwrite_all_assert_null\n",
                    "    str x0, [sp, #-16]\n    cbz x1, .Lwrite_all_assert_null\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("    str x0, [sp, #-16]"),
                    "uses memory below sp",
                )],
            },
        },
    ),
    (
        "the stack pointer set by something the frame doesn't do",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    mov rax, qword ptr [rsp]\n",
                    "    and rsp, -16\n    mov rax, qword ptr [rsp]\n",
                )],
                finds: &[(Rule::Resources, At::Line("    and rsp, -16"), "changes rsp")],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    ldr x0, [sp]\n    cmp x0, #ARGC_EXPECTED\n",
                    "    mov sp, x9\n    ldr x0, [sp]\n    cmp x0, #ARGC_EXPECTED\n",
                )],
                finds: &[(Rule::Resources, At::Line("    mov sp, x9"), "sets sp")],
            },
        },
    ),
    (
        "a call made with the stack off its multiple of 16",
        PerCpu {
            // A call in _start is aligned with nothing pushed; one push misaligns it.
            x86_64: Plan::Breaks {
                edits: &[(
                    "    call parse_i64\n",
                    "    push rax\n    call parse_i64\n    pop rcx\n",
                )],
                finds: &[(
                    Rule::Functions,
                    At::Line("    call parse_i64"),
                    "isn't 16-byte aligned",
                )],
            },
            // AArch64 faults on an sp that isn't a multiple of 16, so the dialect refuses the
            // move itself.
            aarch64: Plan::Breaks {
                edits: &[(
                    "    bl parse_i64\n",
                    "    sub sp, sp, #8\n    bl parse_i64\n    add sp, sp, #8\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("    sub sp, sp, #8"),
                    "but sp stays a multiple of 16",
                )],
            },
        },
    ),
    (
        "a function entered 8 bytes deep that makes a call without adjusting the stack",
        PerCpu {
            // A normal function enters 8 bytes deep, so write_i64 needs its sub rsp, 8.
            x86_64: Plan::Breaks {
                edits: &[("    sub rsp, 8\n", ""), ("    add rsp, 8\n", "")],
                finds: &[(
                    Rule::Functions,
                    At::After("    mov edi, STDOUT"),
                    "isn't 16-byte aligned",
                )],
            },
            aarch64: Plan::NotApplicable(
                "an AArch64 call leaves its return address in x30, not on the stack, and a \
                 function keeps sp a multiple of 16 for its whole life (the row above refuses \
                 any other move), so no function is entered 8 bytes deep",
            ),
        },
    ),
];

#[tokio::test]
async fn the_stack_is_tracked_exactly() {
    hold(&ON_THE_TEMPLATE, STACK).await;
}

/// Rule 2: every loop is counted, from a counter nothing in it changes, entered at its head,
/// with a bound the counter starts within.
const LOOPS: Fixtures = &[
    (
        "a counter that the loop's system call changes",
        PerCpu {
            // A counter in rcx doesn't survive the syscall in the loop.
            x86_64: Plan::Breaks {
                edits: &[
                    ("    mov r8, MAX_WRITES\n", "    mov rcx, MAX_WRITES\n"),
                    (
                        "    dec r8\n    jnz .Lwrite_all_loop\n",
                        "    dec rcx\n    jnz .Lwrite_all_loop\n",
                    ),
                ],
                finds: &[(
                    Rule::Loops,
                    At::Line("    syscall\n    test rax, rax\n    js .Lwrite_all_return"),
                    "changes the loop counter rcx",
                )],
            },
            // A counter in x0 is the system call's result, and the loop's first instruction sets
            // it too.
            aarch64: Plan::Breaks {
                edits: &[
                    ("    mov x10, #MAX_WRITES\n", "    mov x0, #MAX_WRITES\n"),
                    (
                        "    subs x10, x10, #1\n    b.ne .Lwrite_all_loop\n",
                        "    subs x0, x0, #1\n    b.ne .Lwrite_all_loop\n",
                    ),
                ],
                finds: &[(
                    Rule::Loops,
                    At::Line("    mov x0, x9\n    mov x8, #SYS_WRITE"),
                    "changes the loop counter x0",
                )],
            },
        },
    ),
    (
        "a jump into the middle of a loop",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[
                    (
                        "    mov r8, MAX_WRITES\n",
                        "    jz .Lwrite_all_retry\n    mov r8, MAX_WRITES\n",
                    ),
                    (
                        "    cmp rax, rdx\n",
                        ".Lwrite_all_retry:\n    cmp rax, rdx\n",
                    ),
                ],
                finds: &[(
                    Rule::Loops,
                    At::Line("    jz .Lwrite_all_retry"),
                    "jumps into the middle of a loop",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[
                    (
                        "    mov x10, #MAX_WRITES\n",
                        "    cbz x9, .Lwrite_all_retry\n    mov x10, #MAX_WRITES\n",
                    ),
                    ("    cmp x0, x2\n", ".Lwrite_all_retry:\n    cmp x0, x2\n"),
                ],
                finds: &[(
                    Rule::Loops,
                    At::Line("    cbz x9, .Lwrite_all_retry"),
                    "jumps into the middle of a loop",
                )],
            },
        },
    ),
    (
        "a bound smaller than the counter",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    ".Lwrite_all_loop:  # bound: 64",
                    ".Lwrite_all_loop:  # bound: 8",
                )],
                finds: &[(
                    Rule::Loops,
                    At::Line("    mov eax, SYS_WRITE"),
                    "outside 1 to the bound 8",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    ".Lwrite_all_loop:  // bound: 64",
                    ".Lwrite_all_loop:  // bound: 8",
                )],
                finds: &[(
                    Rule::Loops,
                    At::Line("    mov x0, x9\n    mov x8, #SYS_WRITE"),
                    "outside 1 to the bound 8",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn loops_must_be_counted() {
    hold(&ON_THE_TEMPLATE, LOOPS).await;
}

/// Rule 5: an assertion has one place to be reached from, a number of its own, and the template's
/// handler.
const ASSERTIONS: Fixtures = &[
    (
        "two checks that share one assertion",
        PerCpu {
            // Two jumps share assertion 9, and assertion 10 now has no way in, so it can never run.
            x86_64: Plan::Breaks {
                edits: &[(
                    "    jz .Lwrite_all_assert_empty\n",
                    "    jz .Lwrite_all_assert_null\n",
                )],
                finds: &[
                    (
                        Rule::Assertions,
                        At::Line("    mov edi, 9"),
                        "reached from 2 places",
                    ),
                    (
                        Rule::ControlFlow,
                        At::Line("    mov edi, 10"),
                        "can never run",
                    ),
                ],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cbz x2, .Lwrite_all_assert_empty\n",
                    "    cbz x2, .Lwrite_all_assert_null\n",
                )],
                finds: &[
                    (
                        Rule::Assertions,
                        At::Line("    mov w0, #10"),
                        "reached from 2 places",
                    ),
                    (
                        Rule::ControlFlow,
                        At::Line("    mov w0, #11"),
                        "can never run",
                    ),
                ],
            },
        },
    ),
    (
        "an assertion reached only through a branch on the zero register",
        PerCpu {
            x86_64: Plan::NotApplicable(
                "x86-64 has no zero register: every conditional jump there tests the flags",
            ),
            // `cbz xzr` always jumps, as `b` does, so the check never decides anything: assertion
            // 10 always fails, and the lines after the jump can never run.
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cbz x1, .Lwrite_all_assert_null\n",
                    "    cbz xzr, .Lwrite_all_assert_null\n",
                )],
                finds: &[
                    (
                        Rule::Assertions,
                        At::Line("    mov w0, #10"),
                        "always fails",
                    ),
                    (
                        Rule::ControlFlow,
                        At::Line("    cbz x2, .Lwrite_all_assert_empty"),
                        "can never run",
                    ),
                ],
            },
        },
    ),
    (
        "an assertion number used twice",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    mov edi, 11\n", "    mov edi, 9\n")],
                finds: &[(
                    Rule::Assertions,
                    At::Line("    mov edi, 9\n    call assert_fail\n.size write_all"),
                    "used twice",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov w0, #12\n", "    mov w0, #10\n")],
                finds: &[(
                    Rule::Assertions,
                    At::Line("    mov w0, #10\n    bl assert_fail\n.size write_all"),
                    "used twice",
                )],
            },
        },
    ),
    (
        "an assertion handler that isn't the template's",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    add dl, 48\n", "    add dl, 49\n")],
                finds: &[(
                    Rule::Assertions,
                    At::Line("assert_fail:"),
                    "assert_fail differs from the template",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    add w0, w0, #48\n", "    add w0, w0, #49\n")],
                finds: &[(
                    Rule::Assertions,
                    At::Line("assert_fail:"),
                    "assert_fail differs from the template",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn assertions_have_one_place_each() {
    hold(&ON_THE_TEMPLATE, ASSERTIONS).await;
}

/// Rule 1: a chain of comparisons on one register ends in a default.
const CHAINS: Fixtures = &[(
    "a chain of comparisons with no default",
    PerCpu {
        x86_64: Plan::Breaks {
            edits: &[(
                "    mov rdi, rdx\n",
                "    cmp rdx, 1\n    je .Lstart_one\n    cmp rdx, 2\n    je .Lstart_one\n\
                 .Lstart_one:\n    mov rdi, rdx\n",
            )],
            finds: &[(
                Rule::ControlFlow,
                At::Line("    cmp rdx, 1"),
                "has no default",
            )],
        },
        aarch64: Plan::Breaks {
            edits: &[(
                "    bl write_i64\n",
                "    cmp x1, #1\n    b.eq .Lstart_one\n    cmp x1, #2\n    b.eq .Lstart_one\n\
                 .Lstart_one:\n    bl write_i64\n",
            )],
            finds: &[(
                Rule::ControlFlow,
                At::Line("    cmp x1, #1"),
                "has no default",
            )],
        },
    },
)];

#[tokio::test]
async fn comparison_chains_need_a_default() {
    hold(&ON_THE_TEMPLATE, CHAINS).await;
}

/// Rule 7: a result is checked right away, before anything else touches the register it's in.
const RESULTS: Fixtures = &[
    (
        "a label between a call and the check of its result",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    call parse_i64\n    test rax, rax\n",
                    "    call parse_i64\n.Lstart_parsed:\n    test rax, rax\n",
                )],
                finds: &[(
                    Rule::Results,
                    At::Line("    call parse_i64"),
                    "a label comes before the check",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    bl parse_i64\n    tbnz x0, #63, .Lstart_not_a_number\n",
                    "    bl parse_i64\n.Lstart_parsed:\n    tbnz x0, #63, .Lstart_not_a_number\n",
                )],
                finds: &[(
                    Rule::Results,
                    At::Line("    bl parse_i64"),
                    "a label comes before the check",
                )],
            },
        },
    ),
    (
        "a system call's result used before it's checked",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    syscall\n    test rax, rax\n    js .Lwrite_all_return\n",
                    "    syscall\n    mov r9, rax\n    test rax, rax\n    js .Lwrite_all_return\n",
                )],
                finds: &[(
                    Rule::Results,
                    At::Line("    syscall\n    mov r9, rax"),
                    "rax is used before it's checked",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    svc #0\n    tbnz x0, #63, .Lwrite_all_return\n",
                    "    svc #0\n    mov x11, x0\n    tbnz x0, #63, .Lwrite_all_return\n",
                )],
                finds: &[(
                    Rule::Results,
                    At::Line("    svc #0\n    mov x11, x0"),
                    "x0 is used before it's checked",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn results_are_checked_right_away() {
    hold(&ON_THE_TEMPLATE, RESULTS).await;
}

/// Rules 3 and 8: an exit status and a system call are named constants, and a system call is one
/// the spec allows.
const CALLS: Fixtures = &[
    (
        "an exit status that isn't a named constant",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    mov edi, EXIT_OK\n", "    mov edi, 0\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line("    mov edi, 0\n"),
                    "without a named constant",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov w0, #EXIT_OK\n", "    mov w0, #0\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line("    mov w0, #0\n"),
                    "without a named constant",
                )],
            },
        },
    ),
    (
        "a system call that isn't named",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    mov eax, SYS_EXIT\n", "    mov eax, 60\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line("    mov eax, 60\n"),
                    "isn't named",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov x8, #SYS_EXIT\n", "    mov x8, #93\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line("    mov x8, #93\n"),
                    "isn't named",
                )],
            },
        },
    ),
    (
        "a system call the spec doesn't allow",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[
                    (
                        ".equ SYS_EXIT, 60\n",
                        ".equ SYS_EXIT, 60\n.equ SYS_READ, 0\n",
                    ),
                    ("    mov eax, SYS_WRITE\n", "    mov eax, SYS_READ\n"),
                ],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov eax, SYS_READ"),
                    "isn't one the spec allows",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[
                    (
                        ".equ SYS_EXIT, 93\n",
                        ".equ SYS_EXIT, 93\n.equ SYS_READ, 63\n",
                    ),
                    ("    mov x8, #SYS_WRITE\n", "    mov x8, #SYS_READ\n"),
                ],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov x8, #SYS_READ"),
                    "isn't one the spec allows",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn system_calls_and_exit_codes_are_named_and_allowed() {
    hold(&ON_THE_TEMPLATE, CALLS).await;
}

/// Rule 6: data is named by its size, written only where it's writable, and used by one function.
const SCOPES: Fixtures = &[
    (
        "a length that isn't the size of the data it names",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(".equ USAGE_MSG_LEN, 18", ".equ USAGE_MSG_LEN, 17")],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ USAGE_MSG_LEN"),
                    "usage_msg is 18 bytes",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(".equ USAGE_MSG_LEN, 18", ".equ USAGE_MSG_LEN, 17")],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ USAGE_MSG_LEN"),
                    "usage_msg is 18 bytes",
                )],
            },
        },
    ),
    (
        "read-only data written by naming it",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    test rsi, rsi\n",
                    "    mov byte ptr [rip + usage_msg], 0\n    test rsi, rsi\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    mov byte ptr [rip + usage_msg], 0"),
                    "writes to read-only usage_msg",
                )],
            },
            aarch64: Plan::NotApplicable(
                "AArch64 has no memory operand that names a symbol: an address is adrp and \
                 add :lo12:, and a store goes through the register that holds it, which the \
                 planted row of M6 (read-only data written through a register) refuses",
            ),
        },
    ),
    (
        "data used by two functions",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    test rsi, rsi\n",
                    "    lea r9, [rip + out_buf]\n    test rsi, rsi\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("out_buf:"),
                    "out_buf is used by write_all, write_i64",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cbz x1, .Lwrite_all_assert_null\n",
                    "    adrp x9, out_buf\n    add x9, x9, :lo12:out_buf\n    cbz x1, \
                     .Lwrite_all_assert_null\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("out_buf:"),
                    "out_buf is used by write_all, write_i64",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn data_stays_in_scope() {
    hold(&ON_THE_TEMPLATE, SCOPES).await;
}

/// Rule 9: an indexed access is bounds-checked first, unless a person marks it unsafe.
const INDEXES: Fixtures = &[
    (
        "an index that was compared under another name",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    cmp rcx, MAX_CHARS\n", "    cmp rdx, MAX_CHARS\n")],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    movzx edx, byte ptr [rdi + rcx]"),
                    "isn't bounds-checked",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    cmp x2, #MAX_CHARS\n", "    cmp x1, #MAX_CHARS\n")],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    ldrb w10, [x0, x2]"),
                    "isn't bounds-checked",
                )],
            },
        },
    ),
    (
        "an access marked unsafe",
        PerCpu {
            x86_64: Plan::Follows {
                edits: &[
                    (
                        "    movzx edx, byte ptr [rdi + rcx]\n",
                        "    # unsafe: the loop counter keeps rcx below MAX_CHARS\n    movzx edx, \
                         byte ptr [rdi + rcx]\n    # end unsafe\n",
                    ),
                    ("    cmp rcx, MAX_CHARS\n", "    cmp rdx, MAX_CHARS\n"),
                ],
                unsafe_blocks: 1,
            },
            aarch64: Plan::Follows {
                edits: &[
                    (
                        "    ldrb w10, [x0, x2]\n",
                        "    // unsafe: the loop counter keeps x2 below MAX_CHARS\n    ldrb w10, \
                         [x0, x2]\n    // end unsafe\n",
                    ),
                    ("    cmp x2, #MAX_CHARS\n", "    cmp x1, #MAX_CHARS\n"),
                ],
                unsafe_blocks: 1,
            },
        },
    ),
];

#[tokio::test]
async fn indexed_accesses_are_bounds_checked() {
    hold(&ON_THE_TEMPLATE, INDEXES).await;
}

/// M2 and rule 9: a bounds check compares without sign, whole, and keeps every access inside its
/// buffer.
const BOUNDS: Fixtures = &[
    (
        "a bounds check that jumps with a signed condition",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    jae .Lparse_assert_index\n",
                    "    jge .Lparse_assert_index\n",
                )],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    jge .Lparse_assert_index"),
                    "jumps with jge",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    b.hs .Lparse_assert_index\n",
                    "    b.ge .Lparse_assert_index\n",
                )],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    b.ge .Lparse_assert_index"),
                    "jumps with b.ge",
                )],
            },
        },
    ),
    (
        "a bounds check that lets one byte too many through",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    cmp ecx, OUT_BUF_LEN\n    jae .Lwrite_i64_assert_digit_index\n",
                    "    cmp ecx, OUT_BUF_LEN + 1\n    jae .Lwrite_i64_assert_digit_index\n",
                )],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    cmp ecx, OUT_BUF_LEN + 1"),
                    "reach byte 21 of out_buf, which has 21 bytes",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cmp x2, #OUT_BUF_LEN\n    b.hs .Lwrite_i64_assert_digit_index\n",
                    "    cmp x2, #OUT_BUF_LEN + 1\n    b.hs .Lwrite_i64_assert_digit_index\n",
                )],
                finds: &[(
                    Rule::Indirection,
                    At::Line("    cmp x2, #OUT_BUF_LEN + 1"),
                    "reach byte 21 of out_buf, which has 21 bytes",
                )],
            },
        },
    ),
    (
        "an address computed with an index, which needs no check",
        PerCpu {
            // `lea` computes an address without touching memory, so its index needs no check.
            x86_64: Plan::Follows {
                edits: &[(
                    "    cmp rcx, MAX_CHARS\n",
                    "    lea r11, [rdi + rcx]\n    cmp rcx, MAX_CHARS\n",
                )],
                unsafe_blocks: 0,
            },
            aarch64: Plan::NotApplicable(
                "AArch64 computes an address with add, which has no memory operand and so no \
                 index to check: an index appears only in a load or a store, which the first \
                 rows hold to its check",
            ),
        },
    ),
];

#[tokio::test]
async fn bounds_checks_compare_without_sign_and_fit_the_buffer() {
    hold(&ON_THE_TEMPLATE, BOUNDS).await;
}

/// Twenty-five `nop` lines, to make a function too long.
macro_rules! nops {
    () => {
        concat!(
            "    nop\n    nop\n    nop\n    nop\n    nop\n",
            "    nop\n    nop\n    nop\n    nop\n    nop\n",
            "    nop\n    nop\n    nop\n    nop\n    nop\n",
            "    nop\n    nop\n    nop\n    nop\n    nop\n",
            "    nop\n    nop\n    nop\n    nop\n    nop\n"
        )
    };
}

/// Rule 4: a function saves the registers it changes, stays small, doesn't call itself.
const FUNCTIONS: Fixtures = &[
    (
        "a function that changes a register it must give back",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    test rsi, rsi\n",
                    "    xor ebx, ebx\n    test rsi, rsi\n",
                )],
                finds: &[(
                    Rule::Functions,
                    At::Line("write_all:"),
                    "changes rbx without saving it",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    cbz x1, .Lwrite_all_assert_null\n",
                    "    mov x19, #0\n    cbz x1, .Lwrite_all_assert_null\n",
                )],
                finds: &[(
                    Rule::Functions,
                    At::Line("write_all:"),
                    "changes x19 without saving it",
                )],
            },
        },
    ),
    (
        "recursion",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    call parse_i64\n", "    call _start\n")],
                finds: &[(Rule::ControlFlow, At::Line("    call _start"), "recursion")],
            },
            aarch64: Plan::Breaks {
                edits: &[("    bl parse_i64\n", "    bl _start\n")],
                finds: &[(Rule::ControlFlow, At::Line("    bl _start"), "recursion")],
            },
        },
    ),
    (
        "a function too long, with a place to split it",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    jo .Lparse_invalid\n    inc rcx\n    dec r8\n",
                    concat!(
                        "    jo .Lparse_invalid\n    inc rcx\n",
                        nops!(),
                        "    dec r8\n"
                    ),
                )],
                finds: &[(
                    Rule::Functions,
                    At::Line("parse_i64:"),
                    "the loop `.Lparse_loop` (40 instructions), can be a function of its own",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    add x2, x2, #1\n    subs x3, x3, #1\n",
                    concat!("    add x2, x2, #1\n", nops!(), "    subs x3, x3, #1\n"),
                )],
                finds: &[(
                    Rule::Functions,
                    At::Line("parse_i64:"),
                    "the loop `.Lparse_loop` (41 instructions), can be a function of its own",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn functions_stay_small_and_save_what_they_change() {
    hold(&ON_THE_TEMPLATE, FUNCTIONS).await;
}

/// The dialect's layout: one instruction a line, comments the CPU's own, nothing the dialect
/// leaves out, and every line one machine instruction.
const LAYOUT: Fixtures = &[
    (
        "several statements on one line",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    xor r9d, r9d; xor eax, eax\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    xor r9d, r9d; xor eax, eax"),
                    "`;` puts several statements",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov x9, #0\n", "    mov x9, #0; mov x9, #0\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    mov x9, #0; mov x9, #0"),
                    "`;` puts several statements",
                )],
            },
        },
    ),
    (
        "a /* */ comment",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    xor r9d, r9d /* clear */\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    xor r9d, r9d /* clear */"),
                    "`/* */` comments",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov x9, #0\n", "    mov x9, #0 /* clear */\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    mov x9, #0 /* clear */"),
                    "`/* */` comments",
                )],
            },
        },
    ),
    (
        "an instruction prefix",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    rep\n")],
                finds: &[(Rule::Dialect, At::Line("    rep"), "is a prefix")],
            },
            aarch64: Plan::NotApplicable(
                "AArch64 has no instruction prefixes: rep, lock and the segment overrides are \
                 x86 ways to change what the next instruction does, and every AArch64 \
                 instruction is one word that says all of it",
            ),
        },
    ),
    (
        "an x87 instruction",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    fld1\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    fld1"),
                    "`fld1` isn't in dialect 4",
                )],
            },
            aarch64: Plan::NotApplicable(
                "AArch64 has no x87 stack: its floating point is in the s and d registers it \
                 shares with the vectors, and what the dialect leaves out of it is the next \
                 row but one",
            ),
        },
    ),
    (
        "a symbol's address taken as a number",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    mov rax, offset usage_msg\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    mov rax, offset usage_msg"),
                    "offset",
                )],
            },
            aarch64: Plan::NotApplicable(
                "an AArch64 instruction word is 32 bits, so no immediate can hold an address: \
                 an address is adrp and add :lo12:, and the literal pool that would hold one \
                 is refused by the dialect's own table",
            ),
        },
    ),
    (
        "data addressed without the CPU's own way to name an address",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    mov rax, [usage_msg]\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    mov rax, [usage_msg]"),
                    "address data relative to rip",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[("    mov x9, #0\n", "    ldr x9, usage_msg\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    ldr x9, usage_msg"),
                    "its address isn't in brackets",
                )],
            },
        },
    ),
    (
        "a label and an instruction on one line",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    ".Lparse_digits:\n",
                    ".Lparse_digits: cmp byte ptr [rdi], 0\n",
                )],
                finds: &[(
                    Rule::Dialect,
                    At::Line(".Lparse_digits:"),
                    "a label and an instruction share a line",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(".Lparse_digits:\n", ".Lparse_digits: nop\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line(".Lparse_digits:"),
                    "a label and an instruction share a line",
                )],
            },
        },
    ),
    (
        "an instruction the dialect leaves out",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    leave\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    leave"),
                    "`leave` isn't in dialect 4",
                )],
            },
            // An exclusive load: AArch64's atomics are not in the dialect.
            aarch64: Plan::Breaks {
                edits: &[("    mov x9, #0\n", "    ldxr x10, [x0]\n    mov x9, #0\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    ldxr x10, [x0]"),
                    "isn't an instruction lengua's AArch64 dialect has",
                )],
            },
        },
    ),
    (
        "an instruction that assembles to two",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[("    xor r9d, r9d\n", "    fstsw ax\n")],
                finds: &[(
                    Rule::Dialect,
                    At::Line("    fstsw ax"),
                    "don't match line for line",
                )],
            },
            aarch64: Plan::NotApplicable(
                "an AArch64 instruction is one 32-bit word, so a line can't become two \
                 machine instructions as x87's fstsw does (an fwait and an fnstsw)",
            ),
        },
    ),
];

#[tokio::test]
async fn layout_hazards_are_dialect_violations() {
    hold(&ON_THE_TEMPLATE, LAYOUT).await;
}

/// Rule 8: the constants come first, are numbers, and are named once.
const CONSTANTS: Fixtures = &[
    (
        "a constant below the header block",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[
                    (".equ MAX_WRITES, 64\n", ""),
                    (".section .bss\n", ".section .bss\n.equ MAX_WRITES, 64\n"),
                ],
                finds: &[(
                    Rule::Dialect,
                    At::Line(".equ MAX_WRITES, 64"),
                    "below the header block",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[
                    (".equ MAX_WRITES, 64\n", ""),
                    (".section .bss\n", ".section .bss\n.equ MAX_WRITES, 64\n"),
                ],
                finds: &[(
                    Rule::Dialect,
                    At::Line(".equ MAX_WRITES, 64"),
                    "below the header block",
                )],
            },
        },
    ),
    (
        "a constant that uses an address",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    ".equ USAGE_MSG_LEN, 18",
                    ".equ USAGE_MSG_LEN, . - usage_msg",
                )],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ USAGE_MSG_LEN"),
                    "uses an address",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    ".equ USAGE_MSG_LEN, 18",
                    ".equ USAGE_MSG_LEN, . - usage_msg",
                )],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ USAGE_MSG_LEN"),
                    "uses an address",
                )],
            },
        },
    ),
    (
        "a constant defined twice",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(".equ NEWLINE, 10\n", ".equ NEWLINE, 10\n.equ NEWLINE, 13\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ NEWLINE, 13"),
                    "defined twice",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(".equ NEWLINE, 10\n", ".equ NEWLINE, 10\n.equ NEWLINE, 13\n")],
                finds: &[(
                    Rule::Constants,
                    At::Line(".equ NEWLINE, 13"),
                    "defined twice",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn constants_must_come_first_and_stay_numbers() {
    hold(&ON_THE_TEMPLATE, CONSTANTS).await;
}

/// M7 and the thread rules: threads need the spec's approval and the template's own code, a
/// worker writes only its stack and its slot, reads only inside the slot, makes no system call and
/// leaves the register that holds the slot alone, and only the thread code starts threads.
const THREADS: Fixtures = &[
    (
        "a worker's index bounded only by a register",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    cmp r9, SLOT_SIZE - VECTOR + 1\n    jae .Lworker_assert_slot\n    movdqu xmm0, \
                     xmmword ptr [rbx + r9]\n",
                    "    movdqu xmm0, xmmword ptr [rbx + r9]\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    movdqu xmm0, xmmword ptr [rbx + r9]"),
                    "indexes the worker's slot with no constant limit",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    mov x15, #SLOT_SCAN\n    cmp x14, x15\n    b.hs .Lworker_assert_slot\n    ldr q0, \
                     [x19, x14]\n",
                    "    ldr q0, [x19, x14]\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    ldr q0, [x19, x14]"),
                    "indexes the worker's slot with no constant limit",
                )],
            },
        },
    ),
    (
        "a worker reading what other threads write",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    mov rbx, rsi\n    cmp rdi, THREADS\n",
                    "    mov rbx, rsi\n    mov rax, qword ptr [rip + slots]\n    cmp rdi, THREADS\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    mov rax, qword ptr [rip + slots]"),
                    "reaches slots on a worker thread, where other threads write their parts of it",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    mov x19, x1\n    cmp x0, #THREADS\n",
                    "    mov x19, x1\n    adrp x13, slots\n    add x13, x13, :lo12:slots\n    cmp x0, \
                     #THREADS\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    add x13, x13, :lo12:slots"),
                    "on a worker thread, where other threads write their parts of it",
                )],
            },
        },
    ),
    (
        "a worker writing the shared slots through the symbol's address",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    movdqu xmmword ptr [rbx + r9], xmm0\n",
                    "    lea rax, [rip + slots]\n    movdqu xmmword ptr [rax + r9], xmm0\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    lea rax, [rip + slots]"),
                    "on a worker thread, where other threads write their parts of it",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    str q0, [x19, x14]\n",
                    "    adrp x13, slots\n    add x13, x13, :lo12:slots\n    str q0, [x13, x14]\n",
                )],
                finds: &[(
                    Rule::Races,
                    At::Line("    add x13, x13, :lo12:slots"),
                    "on a worker thread, where other threads write their parts of it",
                )],
            },
        },
    ),
    (
        "parallel_for changed",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    mov ebx, 1\n    mov r13, THREADS\n.Lparallel_for_start:",
                    "    mov ebx, 2\n    mov r13, THREADS\n.Lparallel_for_start:",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("parallel_for:"),
                    "parallel_for differs from the template",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    mov x21, #THREADS\n.Lparallel_for_start:",
                    "    mov x21, #THREADS - 1\n.Lparallel_for_start:",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("parallel_for:"),
                    "parallel_for differs from the template",
                )],
            },
        },
    ),
    (
        "clone's flags changed",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(".equ CLONE_FLAGS, 0x350F00", ".equ CLONE_FLAGS, 0x50F00")],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ CLONE_FLAGS"),
                    "CLONE_FLAGS must be 3477248",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(".equ CLONE_FLAGS, 0x350F00", ".equ CLONE_FLAGS, 0x50F00")],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ CLONE_FLAGS"),
                    "CLONE_FLAGS must be 3477248",
                )],
            },
        },
    ),
    (
        "a slot that isn't a multiple of 64 bytes",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(".equ SLOT_SIZE, CHUNK", ".equ SLOT_SIZE, CHUNK + 8")],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ SLOT_SIZE"),
                    "isn't a positive multiple of 64",
                )],
            },
            // 65528 is 16 bits, which one `mov` holds; CHUNK + 8 is not, and the dialect refuses
            // the `mov` before the rules for threads are asked.
            aarch64: Plan::Breaks {
                edits: &[(".equ SLOT_SIZE, CHUNK", ".equ SLOT_SIZE, CHUNK - 8")],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ SLOT_SIZE"),
                    "isn't a positive multiple of 64",
                )],
            },
        },
    ),
    (
        "a thread's stack too small for the worker's deepest path",
        PerCpu {
            // Each thread's stack must hold the worker's deepest path: here 128 bytes, in 64.
            x86_64: Plan::Breaks {
                edits: &[
                    (".equ STACK_SIZE, 4096", ".equ STACK_SIZE, 64"),
                    (
                        "worker:\n    push rbx\n",
                        "worker:\n    push rbx\n    sub rsp, 64\n",
                    ),
                    (
                        ".Lworker_done:\n    mov rax, rdx\n    pop rbx\n",
                        ".Lworker_done:\n    mov rax, rdx\n    add rsp, 64\n    pop rbx\n",
                    ),
                ],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ STACK_SIZE"),
                    "a worker's stack can reach 128 bytes, but STACK_SIZE is 64",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[
                    (".equ STACK_SIZE, 4096", ".equ STACK_SIZE, 64"),
                    (
                        "worker:\n    stp x19, x20, [sp, #-16]!\n",
                        "worker:\n    stp x19, x20, [sp, #-16]!\n    sub sp, sp, #64\n",
                    ),
                    (
                        ".Lworker_done:\n    mov x0, x12\n    ldp x19, x20, [sp], #16\n",
                        ".Lworker_done:\n    mov x0, x12\n    add sp, sp, #64\n    ldp x19, x20, \
                         [sp], #16\n",
                    ),
                ],
                finds: &[(
                    Rule::Resources,
                    At::Line(".equ STACK_SIZE"),
                    "but STACK_SIZE is 64",
                )],
            },
        },
    ),
    (
        "a worker writing a variable every thread shares",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
                    "    mov qword ptr [rip + input_size], rcx\n    add r9, VECTOR\n    dec r8\n    \
                     jnz .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    mov qword ptr [rip + input_size], rcx"),
                    "writes input_size on a worker thread",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    b.ne .Lworker_loop\n",
                    "    adrp x13, input_size\n    add x13, x13, :lo12:input_size\n    str x12, \
                     [x13]\n    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    b.ne \
                     .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    str x12, [x13]"),
                    "writes through x13 on a worker thread",
                )],
            },
        },
    ),
    (
        "a worker writing through a register that isn't its slot",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
                    "    mov byte ptr [rsi], al\n    add r9, VECTOR\n    dec r8\n    jnz \
                     .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    mov byte ptr [rsi], al"),
                    "writes through rsi on a worker thread",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    b.ne .Lworker_loop\n",
                    "    strb w10, [x1]\n    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    b.ne \
                     .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    strb w10, [x1]"),
                    "writes through x1 on a worker thread",
                )],
            },
        },
    ),
    (
        "a worker's slot scanned past its end",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    cmp r9, SLOT_SIZE - VECTOR + 1\n",
                    "    cmp r9, SLOT_SIZE\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    movdqu xmmword ptr [rbx + r9], xmm0"),
                    "outside the worker's slot",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    ".equ SLOT_SCAN, SLOT_SIZE - VECTOR + 1\n",
                    ".equ SLOT_SCAN, SLOT_SIZE\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    str q0, [x19, x14]"),
                    "outside the worker's slot",
                )],
            },
        },
    ),
    (
        "a worker changing the register that holds its slot",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
                    "    add rbx, VECTOR\n    add r9, VECTOR\n    dec r8\n    jnz .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    add rbx, VECTOR"),
                    "changes rbx, which holds the worker's slot",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    b.ne .Lworker_loop\n",
                    "    add x19, x19, #VECTOR\n    add x14, x14, #VECTOR\n    subs x16, x16, #1\n    \
                     b.ne .Lworker_loop\n",
                )],
                finds: &[(
                    Rule::Scope,
                    At::Line("    add x19, x19, #VECTOR"),
                    "changes x19, which holds the worker's slot",
                )],
            },
        },
    ),
    (
        "a system call on a worker thread",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[
                    (
                        ".Lworker_done:\n    mov rax, rdx\n",
                        ".Lworker_done:\n    mov eax, SYS_WRITE\n    syscall\n    test rax, rax\n    \
                         js .Lworker_done_assert\n    mov rax, rdx\n",
                    ),
                    (
                        ".Lworker_assert_index:\n",
                        ".Lworker_done_assert:\n    mov edi, 16\n    call \
                         assert_fail\n.Lworker_assert_index:\n",
                    ),
                ],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov eax, SYS_WRITE"),
                    "makes a system call on a worker thread",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[
                    (
                        ".Lworker_done:\n    mov x0, x12\n",
                        ".Lworker_done:\n    mov x8, #SYS_WRITE\n    svc #0\n    tbnz x0, #63, \
                         .Lworker_done_assert\n    mov x0, x12\n",
                    ),
                    (
                        ".Lworker_assert_index:\n",
                        ".Lworker_done_assert:\n    mov w0, #16\n    bl \
                         assert_fail\n.Lworker_assert_index:\n",
                    ),
                ],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov x8, #SYS_WRITE"),
                    "makes a system call on a worker thread",
                )],
            },
        },
    ),
    (
        "a thread started outside the thread code",
        PerCpu {
            // clone anywhere else is a system call the spec doesn't list.
            x86_64: Plan::Breaks {
                edits: &[(
                    "_start:\n    call read_input\n",
                    "_start:\n    mov edi, CLONE_FLAGS\n    mov eax, SYS_CLONE\n    syscall\n    test \
                     rax, rax\n    js .Lstart_assert_length\n    call read_input\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov eax, SYS_CLONE"),
                    "system call 56 isn't one the spec allows",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "_start:\n    bl read_input\n",
                    "_start:\n    mov x0, #CLONE_FLAGS & 0xffff\n    movk x0, #CLONE_FLAGS >> 16, \
                     lsl #16\n    mov x8, #SYS_CLONE\n    svc #0\n    tbnz x0, #63, \
                     .Lstart_read_failed\n    bl read_input\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::After("    mov x8, #SYS_CLONE"),
                    "system call 220 isn't one the spec allows",
                )],
            },
        },
    ),
    (
        "the main thread calling a function of the workers",
        PerCpu {
            x86_64: Plan::Breaks {
                edits: &[(
                    "    mov rdi, r12\n    call total_results\n",
                    "    mov rdi, r12\n    call total_results\n    xor edi, edi\n    lea rsi, [rip + \
                     slots]\n    call worker\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("    call worker\n    cmp rax, rbx"),
                    "_start calls worker, which only the thread code may call",
                )],
            },
            aarch64: Plan::Breaks {
                edits: &[(
                    "    mov x0, x20\n    bl total_results\n",
                    "    mov x0, x20\n    bl total_results\n    mov x0, #0\n    adrp x1, slots\n    \
                     add x1, x1, :lo12:slots\n    bl worker\n",
                )],
                finds: &[(
                    Rule::Resources,
                    At::Line("    bl worker\n    cmp x0, x19"),
                    "_start calls worker, which only the thread code may call",
                )],
            },
        },
    ),
];

#[tokio::test]
async fn threads_follow_the_rules_of_the_thread_code() {
    hold(&ON_THE_PARALLEL_TEMPLATE, THREADS).await;
}

/// Threads need the spec's approval: a program that starts them under a spec that allows none is
/// refused, and so is one whose THREADS is more than the spec allows.
#[tokio::test]
async fn threads_need_the_specs_approval() {
    for (_, template) in PARALLEL_TEMPLATES.cells() {
        let report = check_full(template, CpuFeatures::all(), &PARALLEL_SYSCALLS, None).await;
        assert_violation(
            &report,
            Rule::Resources,
            line_of(template, "parallel_for:"),
            "spec.md doesn't allow threads",
        );
        let report = check_full(template, CpuFeatures::all(), &PARALLEL_SYSCALLS, Some(8)).await;
        assert_violation(
            &report,
            Rule::Resources,
            line_of(template, ".equ THREADS, 16"),
            "THREADS is 16, but the spec allows from 1 to 8 threads",
        );
    }
}

/// Every target's parallel template passes every rule.
#[tokio::test]
async fn the_parallel_templates_pass_every_rule() {
    for (target, template) in PARALLEL_TEMPLATES.cells() {
        let report = check_parallel(template).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{}: {all:#?}", target.name());
    }
}

/// Every target's main template passes every rule, and the rules read all of it.
#[tokio::test]
async fn the_template_passes_every_rule() {
    for (target, template) in TEMPLATES.cells() {
        let report = check(template).await;
        let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
        assert!(report.passed(), "{}: {all:#?}", target.name());
        let facts = report.facts.expect("the rules ran");
        assert_eq!(facts.functions, 4, "{}", target.name());
        assert_eq!(facts.assertions, 11, "{}", target.name());
        assert!(facts.max_stack_bytes >= 48, "{}: {facts:?}", target.name());
        assert!(report.unsafe_blocks.is_empty(), "{}", target.name());
    }
}

/// What a target's fillers are made of and where they go: the function that writes through its
/// first argument and reads through its second (`fill`) and the one that calls it (`refill`), put
/// before the template's `write_all`, and the call that lends 8 bytes to either.
struct Kit {
    /// A constant the fillers name, added: this line of the template becomes the other.
    constant: Option<Edit>,
    /// The fillers.
    fillers: &'static str,
    /// The comment of the function the fillers go before.
    before: &'static str,
    /// The label the call goes after.
    label: &'static str,
    /// The line that sets the call's size.
    size: &'static str,
    /// The call's instruction, up to the function it calls.
    call: &'static str,
}

/// AArch64's `fill` and `refill`: `fill` stores a byte through x0 and returns the byte at x1.
const AARCH64_FILLERS: &str = "// Stores a byte through x0, and returns the byte at x1.
// In: x0 = the buffer, x1 = a byte to look at, x2 = how many, at least 1.
// Out: x0 = the byte.
// returns: result
.type fill, @function
fill:
    strb w2, [x0]
    ldrb w0, [x1]
    ret
.size fill, . - fill

// Stores a byte through x0, by fill, and returns the byte at x1.
// In: x0 = the buffer, x1 = a byte to look at.
// Out: x0 = the byte.
// returns: result
.type refill, @function
refill:
    stp x29, x30, [sp, #-16]!
    mov x2, #8
    bl fill
    ldp x29, x30, [sp], #16
    ret
.size refill, . - refill

";

/// Each target's fillers.
const KITS: PerCpu<Kit> = PerCpu {
    x86_64: Kit {
        constant: Some((
            ".equ SYS_WRITE, 1\n",
            ".equ SYS_READ, 0\n.equ SYS_WRITE, 1\n",
        )),
        fillers: FILLERS,
        before: "# Writes rdx bytes from rsi to file descriptor edi",
        label: ".Lstart_usage:\n",
        size: "    mov edx, 8\n",
        call: "    call ",
    },
    aarch64: Kit {
        constant: None,
        fillers: AARCH64_FILLERS,
        before: "// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.",
        label: ".Lstart_usage:\n",
        size: "    mov x2, #8\n",
        call: "    bl ",
    },
};

/// A way of lending an address to a function that writes through one argument and reads through
/// another: the lines that put it in the two registers, the function called, and what the refusal
/// says, none where the lending is allowed.
struct Lending {
    setup: &'static str,
    callee: &'static str,
    refused: Option<&'static str>,
}

/// M4: an address a function writes through is lent to it once.
const LENDINGS: &[(&str, PerCpu<Lending>)] = &[
    (
        "the same address in both registers",
        PerCpu {
            x86_64: Lending {
                setup: "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf]\n",
                callee: "fill",
                refused: Some(
                    "lends one address, out_buf, in both rdi and rsi, and fill writes through rdi",
                ),
            },
            aarch64: Lending {
                setup: "    adrp x0, out_buf\n    add x0, x0, :lo12:out_buf\n    adrp x1, out_buf\n    \
                        add x1, x1, :lo12:out_buf\n",
                callee: "fill",
                refused: Some(
                    "lends one address, out_buf, in both x0 and x1, and fill writes through x0",
                ),
            },
        },
    ),
    (
        "a copy of the pointer, which is the same address",
        PerCpu {
            x86_64: Lending {
                setup: "    lea rdi, [rip + out_buf + 4]\n    mov rsi, rdi\n",
                callee: "fill",
                refused: Some("lends one address, out_buf + 4, in both rdi and rsi"),
            },
            aarch64: Lending {
                setup: "    adrp x0, out_buf\n    add x0, x0, :lo12:out_buf\n    add x0, x0, #4\n    \
                        mov x1, x0\n",
                callee: "fill",
                refused: Some("lends one address, out_buf + 4, in both x0 and x1"),
            },
        },
    ),
    (
        "a function that writes through the address because what it calls does",
        PerCpu {
            x86_64: Lending {
                setup: "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf]\n",
                callee: "refill",
                refused: Some("and refill writes through rdi"),
            },
            aarch64: Lending {
                setup: "    adrp x0, out_buf\n    add x0, x0, :lo12:out_buf\n    adrp x1, out_buf\n    \
                        add x1, x1, :lo12:out_buf\n",
                callee: "refill",
                refused: Some("and refill writes through x0"),
            },
        },
    ),
    (
        "separate parts of one buffer, as objects in an arena are",
        PerCpu {
            x86_64: Lending {
                setup: "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + out_buf + 8]\n",
                callee: "fill",
                refused: None,
            },
            aarch64: Lending {
                setup: "    adrp x0, out_buf\n    add x0, x0, :lo12:out_buf\n    adrp x1, out_buf\n    \
                        add x1, x1, :lo12:out_buf\n    add x1, x1, #8\n",
                callee: "fill",
                refused: None,
            },
        },
    ),
    (
        "a buffer that is only read, lent twice",
        PerCpu {
            x86_64: Lending {
                setup: "    lea rdi, [rip + out_buf]\n    lea rsi, [rip + usage_msg]\n",
                callee: "fill",
                refused: None,
            },
            aarch64: Lending {
                setup: "    adrp x0, out_buf\n    add x0, x0, :lo12:out_buf\n    adrp x1, usage_msg\n    \
                        add x1, x1, :lo12:usage_msg\n",
                callee: "fill",
                refused: None,
            },
        },
    ),
];

/// The template of `target` with its fillers added, and a call of `callee` right after the label
/// of its usage path, with `setup` before it.
fn lending(target: Profile, setup: &str, callee: &str) -> String {
    let kit = KITS.get(target);
    let mut text = (*TEMPLATES.get(target)).to_owned();
    if let Some((old, new)) = kit.constant {
        assert!(text.contains(old), "{old:?} isn't in the template");
        text = text.replacen(old, new, 1);
    }
    assert!(
        text.contains(kit.before),
        "{:?} isn't in the template",
        kit.before
    );
    text = text.replacen(kit.before, &format!("{}{}", kit.fillers, kit.before), 1);
    assert!(
        text.contains(kit.label),
        "{:?} isn't in the template",
        kit.label
    );
    let call = format!("{}{setup}{}{}{callee}\n", kit.label, kit.size, kit.call);
    text.replacen(kit.label, &call, 1)
}

#[tokio::test]
async fn one_address_is_never_lent_twice_for_writing() {
    let mut missed = Vec::new();
    for (name, cells) in LENDINGS {
        for (target, cell) in cells.cells() {
            let text = lending(target, cell.setup, cell.callee);
            let report = check_full(&text, CpuFeatures::all(), &PARALLEL_SYSCALLS, None).await;
            let refusals: Vec<String> = report
                .violations
                .iter()
                .filter(|violation| violation.rule == Rule::ExclusiveWrites)
                .map(ToString::to_string)
                .collect();
            let on = format!("{name} on {}", target.name());
            match cell.refused {
                // Refused on the line of the call, which the usage path starts with.
                Some(needle) => {
                    let call = format!("{}{}\n", KITS.get(target).call, cell.callee);
                    let line = line_of(&text, &call);
                    let found = report.violations.iter().any(|violation| {
                        violation.rule == Rule::ExclusiveWrites
                            && violation.line == Some(line)
                            && violation.to_string().contains(needle)
                    });
                    if !found {
                        missed.push(format!("{on}: not refused for {needle:?}: {refusals:#?}"));
                    }
                }
                None => {
                    if !refusals.is_empty() {
                        missed.push(format!("{on}: was refused: {refusals:#?}"));
                    }
                }
            }
        }
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// Every table of fixtures, with its name.
const TABLES: &[(&str, Fixtures)] = &[
    ("the stack", STACK),
    ("loops", LOOPS),
    ("assertions", ASSERTIONS),
    ("chains", CHAINS),
    ("results", RESULTS),
    ("calls", CALLS),
    ("scope", SCOPES),
    ("indexes", INDEXES),
    ("bounds", BOUNDS),
    ("functions", FUNCTIONS),
    ("layout", LAYOUT),
    ("constants", CONSTANTS),
    ("numbers", NUMBERS),
    ("threads", THREADS),
];

/// Every fixture is written on a target or says why that CPU can't have it, and no target writes
/// fewer than the tests these tables replaced did.
#[test]
fn every_fixture_is_written_or_excused_on_every_target() {
    let mut written = PerCpu {
        x86_64: 0,
        aarch64: 0,
    };
    for (table, rows) in TABLES {
        for (name, cells) in *rows {
            let mut any = false;
            for (target, plan) in cells.cells() {
                if let Plan::NotApplicable(why) = plan {
                    assert!(
                        why.len() > 40,
                        "{table}: {name}: say why {} can't",
                        target.name()
                    );
                    continue;
                }
                any = true;
                match target {
                    Profile::LinuxX86_64 => written.x86_64 += 1,
                    Profile::LinuxAarch64 => written.aarch64 += 1,
                }
            }
            assert!(any, "{table}: {name}: no target writes it");
        }
    }
    for (_, cells) in LENDINGS {
        match (cells.x86_64.refused, cells.aarch64.refused) {
            (Some(_), Some(_)) | (None, None) => {}
            _ => panic!("a lending is refused on one target and allowed on the other"),
        }
    }
    assert!(written.x86_64 >= 57, "{}", written.x86_64);
    assert!(written.aarch64 >= 48, "{}", written.aarch64);
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

/// Every AArch64 asset and fixture passes every rule, held to the calls its own spec names, as
/// x86-64's are.
#[tokio::test]
async fn every_aarch64_asset_follows_every_rule() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = root.join("plugin/skills/compile/assets");
    let mut programs = Vec::new();
    for (spec, code) in [
        ("template-spec.md", "linux-aarch64/main-template.s"),
        ("vector-template-spec.md", "linux-aarch64/vector-template.s"),
        ("stream-template-spec.md", "linux-aarch64/stream-template.s"),
        ("tally-template-spec.md", "linux-aarch64/tally-template.s"),
        ("server-template-spec.md", "linux-aarch64/server-template.s"),
        (
            "terminal-template-spec.md",
            "linux-aarch64/terminal-template.s",
        ),
        ("fast/wc/spec.md", "linux-aarch64/fast/wc/main.s"),
        ("fast/wc/spec.md", "linux-aarch64/fast/wc-sve/main.s"),
        ("fast/crc32c/spec.md", "linux-aarch64/fast/crc32c/main.s"),
        (
            "fast/crc32c/spec.md",
            "linux-aarch64/fast/crc32c-crc/main.s",
        ),
        (
            "linux-aarch64/fast/sha256-sha2/spec.md",
            "linux-aarch64/fast/sha256-sha2/main.s",
        ),
        (
            "linux-aarch64/fast/aes-aes/spec.md",
            "linux-aarch64/fast/aes-aes/main.s",
        ),
        (
            "fast/histogram/spec.md",
            "linux-aarch64/fast/histogram/main.s",
        ),
        (
            "parallel-template-spec.md",
            "linux-aarch64/parallel-template.s",
        ),
    ] {
        programs.push((assets.join(spec), assets.join(code)));
    }
    // Each fixture is a folder of spec.md and main.s.
    for entry in std::fs::read_dir(root.join("tests/fixtures/linux-aarch64")).expect("fixtures") {
        let path = entry.expect("an entry").path();
        if path.join("main.s").is_file() {
            programs.push((path.join("spec.md"), path.join("main.s")));
        }
    }
    let mut failed = Vec::new();
    for (spec, code) in &programs {
        let spec = crate::spec::parse(&std::fs::read_to_string(spec).expect("a spec"))
            .unwrap_or_else(|problems| panic!("{}: {problems:?}", spec.display()));
        let text = std::fs::read_to_string(code).expect("a program");
        let report = check_full(&text, CpuFeatures::all(), &spec.syscalls, spec.threads).await;
        if !report.passed() {
            let all: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
            failed.push(format!("{}: {all:#?}", code.display()));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// AArch64's CRC-32 instructions are held to a CPU that has them, as x86-64's extensions are.
#[tokio::test]
async fn aarch64_crc_needs_a_cpu_with_crc() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc/main.s"),
    )
    .expect("the program");
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let without = CpuFeatures {
        crc: false,
        ..CpuFeatures::all()
    };
    let report = check_full(&text, without, &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    crc32cx w0, w0, x13"),
        "needs CRC, which this CPU doesn't have",
    );
}

/// AArch64's carry-less multiply of doublewords is held to a CPU with the PMULL extension, as
/// x86-64's pclmulqdq is to one with PCLMUL; its bytes' form is in every ARM64 CPU.
#[tokio::test]
async fn aarch64_pmull_needs_a_cpu_with_pmull() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc/main.s"),
    )
    .expect("the program");
    let syscalls = [Syscall::Read, Syscall::Write, Syscall::Exit];
    let without = CpuFeatures {
        pmull: false,
        ..CpuFeatures::all()
    };
    let report = check_full(&text, without, &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    pmull v16.1q, v0.1d, v24.1d"),
        "needs the PMULL extension, which this CPU doesn't have",
    );
    let bytes = text.replacen(
        "    pmull v16.1q, v0.1d, v24.1d",
        "    pmull v16.8h, v0.8b, v24.8b",
        1,
    );
    let report = check_full(&bytes, without, &syscalls, None).await;
    assert!(
        !report
            .violations
            .iter()
            .any(|violation| violation.problem.contains("pmull v16.8h, v0.8b, v24.8b")),
        "{:?}",
        report.violations
    );
}

/// AArch64's AES and SHA instructions are held to a CPU with the AES and SHA2 extensions, as
/// x86-64's are to one with AES-NI and SHA.
#[tokio::test]
async fn aarch64_aes_and_sha_need_a_cpu_with_them() {
    let text = crate::checker::AARCH64_TEMPLATE
        .replacen(
            ".arch armv8-a\n",
            ".arch armv8-a\n.arch_extension aes\n.arch_extension sha2\n",
            1,
        )
        .replacen(
            "_start:\n",
            "_start:\n    aese v0.16b, v1.16b\n    sha256h q0, q1, v2.4s\n",
            1,
        );
    let syscalls = [Syscall::Write, Syscall::Exit];
    let without = CpuFeatures {
        aes: false,
        sha: false,
        ..CpuFeatures::all()
    };
    let report = check_full(&text, without, &syscalls, None).await;
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    aese v0.16b, v1.16b"),
        "needs the AES extension, which this CPU doesn't have",
    );
    assert_violation(
        &report,
        Rule::Dialect,
        line_of(&text, "    sha256h q0, q1, v2.4s"),
        "needs the SHA2 extension, which this CPU doesn't have",
    );
    let with = check_full(&text, CpuFeatures::all(), &syscalls, None).await;
    assert!(
        !with
            .violations
            .iter()
            .any(|violation| violation.problem.contains("extension")),
        "{:?}",
        with.violations
    );
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

/// `b.al`, `b.nv` and a branch on the zero register that must jump (`cbz`, `tbz`) always jump, as
/// `b` does: the code after one can never run.
#[tokio::test]
async fn code_after_an_always_taken_aarch64_branch_can_never_run() {
    for branch in [
        "b.al .Lstart_exit",
        "b.nv .Lstart_exit",
        "cbz xzr, .Lstart_exit",
        "cbz wzr, .Lstart_exit",
        "tbz xzr, #5, .Lstart_exit",
        "tbz wzr, #31, .Lstart_exit",
    ] {
        let text = with_aarch64(
            "    mov w0, #EXIT_OK\n    b .Lstart_exit\n",
            &format!(
                "    mov w0, #EXIT_OK\n    {branch}\n    mov w0, #99\n    b \
                 .Lstart_exit\n"
            ),
        );
        let report = check(&text).await;
        assert_violation(
            &report,
            Rule::ControlFlow,
            line_of(&text, "    mov w0, #99"),
            "can never run",
        );
    }
}

/// A branch on the zero register that is never taken (`cbnz`, `tbnz`) goes nowhere: the code at
/// its target can never run unless something else jumps there.
#[tokio::test]
async fn code_only_a_never_taken_aarch64_branch_reaches_can_never_run() {
    for branch in ["cbnz xzr", "cbnz wzr", "tbnz xzr, #5", "tbnz wzr, #31"] {
        let text = with_aarch64(
            "    mov w0, #EXIT_OK\n    b .Lstart_exit\n",
            &format!(
                "    mov w0, #EXIT_OK\n    {branch}, .Lstart_never\n    b .Lstart_exit\n\
                 .Lstart_never:\n    mov w0, #99\n    b .Lstart_exit\n"
            ),
        );
        let report = check(&text).await;
        assert_violation(
            &report,
            Rule::ControlFlow,
            line_of(&text, "    mov w0, #99"),
            "can never run",
        );
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

/// An AArch64 program of the inline form that reads with fill: cat over standard input of any
/// length, as tests/fixtures/linux-aarch64/cat-stream was before it moved to the edge form.
fn aarch64_with_fill() -> String {
    include_str!("inline_form/linux-aarch64/cat-stream.s").to_owned()
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
            aarch64_calc_with(entries, "    .long .Lmain_exit - operations\n"),
            Rule::Indirection,
            jump,
            "names .Lmain_exit, which isn't a label of apply",
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

/// The frozen texts are the inline form's: each is the constant it is looked up by, none defines
/// `main`, and a name that isn't one of them has none.
#[test]
fn the_frozen_texts_are_the_inline_forms() {
    let frozen = [
        ("main-template.s", TEMPLATE),
        ("parallel-template.s", PARALLEL_TEMPLATE),
        ("fill.s", FILL_TEMPLATE),
        ("linux-aarch64/main-template.s", AARCH64_TEMPLATE),
        (
            "linux-aarch64/parallel-template.s",
            PARALLEL_TEMPLATE_AARCH64,
        ),
        ("linux-aarch64/fill.s", AARCH64_FILL_TEMPLATE),
    ];
    for (name, text) in frozen {
        assert_eq!(super::frozen(name), Some(text), "{name}");
        assert_eq!(crate::edge::form(text), crate::edge::Form::Inline, "{name}");
    }
    assert_eq!(super::frozen("server-template.s"), None);
}

/// lengua.s read as a program, with the first line main.s gives it: its 26 functions are all
/// there, and the dialect's parser finds fault only with how its data is laid out (alignment
/// directives and sizes that are bare numbers), which is not a program's rule to hold lengua to.
#[test]
fn the_dialect_finds_only_the_data_layout_of_lengua_s_at_fault() {
    for profile in Profile::ALL {
        let (first, call) = match profile.isa() {
            Isa::X86_64 => (".intel_syntax noprefix\n", "call"),
            Isa::Aarch64 => (".arch armv8-a\n", "bl"),
        };
        let mut main = String::from("main:\n");
        for op in crate::edge::Op::ALL {
            main.push_str(&format!("    {call} {}\n", op.symbol()));
        }
        for name in ["assert_fail", "fill", "parallel_for"] {
            main.push_str(&format!("    {call} {name}\n"));
        }
        let text = crate::edge::lengua_text(profile, &main).expect("the edge form");
        let (source, violations) = source::parse(&format!("{first}{text}"));
        assert_eq!(source.functions.len(), 26, "{profile:?}");
        let others: Vec<String> = violations
            .iter()
            .map(ToString::to_string)
            .filter(|violation| {
                !violation.contains(".balign emits bytes outside a data symbol")
                    && !violation.contains("uses a bare number")
            })
            .collect();
        assert!(others.is_empty(), "{profile:?}: {others:#?}");
    }
}

/// The smallest program that ends with 0, in the inline form and in the edge form, for each CPU.
const EXITS_INLINE: PerCpu<&str> = PerCpu {
    x86_64: ".intel_syntax noprefix\n.equ SYS_EXIT, 231\n.equ EXIT_OK, 0\n\n.text\n.globl _start\n\
             .type _start, @function\n_start:\n    mov edi, EXIT_OK\n    mov eax, SYS_EXIT\n    syscall\n\
             .size _start, . - _start\n",
    aarch64: ".arch armv8-a\n.equ SYS_EXIT, 94\n.equ EXIT_OK, 0\n\n.text\n.globl _start\n\
              .type _start, @function\n_start:\n    mov w0, #EXIT_OK\n    mov x8, #SYS_EXIT\n    svc #0\n\
              .size _start, . - _start\n",
};

/// The same program against the edge: `main` is entered by a call, and ends with `os_exit`.
const EXITS_EDGE: PerCpu<&str> = PerCpu {
    x86_64: ".intel_syntax noprefix\n.equ EXIT_OK, 0\n\n.text\n.type main, @function\nmain:\n\
             \x20   sub rsp, 8\n    mov edi, EXIT_OK\n    call os_exit\n.size main, . - main\n",
    aarch64: ".arch armv8-a\n.equ EXIT_OK, 0\n\n.text\n.type main, @function\nmain:\n\
              \x20   stp x29, x30, [sp, #-16]!\n    mov w0, #EXIT_OK\n    bl os_exit\n.size main, . - main\n",
};

/// The same program in the two forms has the same facts, apart from the deepest the stack gets,
/// which counts the edge's own frames in the edge form: lengua's code doesn't leak into what the
/// rules learn of the program.
#[tokio::test]
async fn the_two_forms_have_the_same_facts() {
    for profile in Profile::ALL {
        let facts = |text: &'static str| async move {
            check_full(text, CpuFeatures::all(), &SYSCALLS, None)
                .await
                .facts
                .expect("the rules got far enough to learn the facts")
        };
        let inline = facts(EXITS_INLINE.get(profile)).await;
        let edge = facts(EXITS_EDGE.get(profile)).await;
        assert_eq!(edge.features, inline.features, "{profile:?}");
        assert_eq!(edge.named, inline.named, "{profile:?}");
        assert_eq!(edge.tables, inline.tables, "{profile:?}");
        assert_eq!(edge.functions, inline.functions, "{profile:?}");
        assert_eq!(edge.assertions, inline.assertions, "{profile:?}");
        assert_eq!(edge.sve_bytes, inline.sve_bytes, "{profile:?}");
        assert!(
            edge.max_stack_bytes > inline.max_stack_bytes,
            "{profile:?}: {} and {}",
            edge.max_stack_bytes,
            inline.max_stack_bytes
        );
    }
}

/// lengua.s is matched to the executable line for line, and every function of the executable is
/// main.s's or lengua's; a call to an operation changes what the edge's convention changes and
/// no more, where an ordinary call changes everything a function may.
#[tokio::test]
async fn lengua_s_is_matched_and_an_operation_keeps_what_the_edge_keeps() {
    let toolchain = Toolchain::discover()
        .await
        .expect("llvm-mc and ld.lld are installed");
    for profile in Profile::ALL {
        let (first, call, ret) = match profile.isa() {
            Isa::X86_64 => (".intel_syntax noprefix", "call", "ret"),
            Isa::Aarch64 => (".arch armv8-a", "bl", "ret"),
        };
        // A main that calls all of it, and the worker the thread library runs.
        let mut text = format!("{first}\n\n.text\n.type main, @function\nmain:\n");
        for op in crate::edge::Op::ALL {
            text.push_str(&format!("    {call} {}\n", op.symbol()));
        }
        for name in ["assert_fail", "fill", "parallel_for"] {
            text.push_str(&format!("    {call} {name}\n"));
        }
        text.push_str(&format!(
            ".size main, . - main\n.type worker, @function\nworker:\n    {ret}\n.size worker, . - worker\n"
        ));
        let folder = tempfile::tempdir().expect("temporary folder");
        let (main, object, executable) = (
            folder.path().join("main.s"),
            folder.path().join("main.o"),
            folder.path().join("main"),
        );
        std::fs::write(&main, &text).expect("write main.s");
        toolchain
            .assemble(profile, &main, &object)
            .await
            .unwrap_or_else(|error| panic!("{profile:?} assembling: {error:?}"));
        toolchain
            .link(profile, &object, &executable)
            .await
            .unwrap_or_else(|error| panic!("{profile:?} linking: {error:?}"));
        let machine = decode::read(&std::fs::read(&executable).expect("the executable"))
            .expect("the executable is readable");
        let (source, layout) = check_layout(&text);
        assert!(layout.passed(), "{profile:?}: {:?}", layout.violations);
        let lengua = lengua_source(profile, &text).expect("the edge form");
        let mut violations = Vec::new();
        let program = decode::map(
            &source,
            Some(&lengua),
            &machine,
            CpuFeatures::all(),
            &mut violations,
        )
        .unwrap_or_else(|| panic!("{profile:?}: {violations:#?}"));
        assert_eq!(program.lengua.len(), 26, "{profile:?}");
        assert_eq!(program.functions.len(), 2, "{profile:?}");
        // lengua's data is not main.s's.
        assert!(
            program.data.is_empty(),
            "{profile:?}: {:?}",
            program.data.keys()
        );
        assert!(!program.lengua_data.is_empty(), "{profile:?}");

        let abi = program.abi();
        let main = program.function("main").expect("main");
        let call_to = |name: &str| {
            let address = program
                .lengua_at_name(name)
                .expect("lengua's function")
                .address;
            main.steps
                .iter()
                .find(|step| step.insn.control == crate::isa::Control::Call(address))
                .map(|step| &step.insn)
                .unwrap_or_else(|| panic!("{profile:?}: main doesn't call {name}"))
        };
        let operation = call_to("os_exit");
        for register in abi.edge_clobbers {
            assert!(
                operation.changes(*register, abi),
                "{profile:?} {register:?}"
            );
        }
        for register in abi.edge_arguments {
            if !abi.edge_clobbers.contains(register) {
                assert!(
                    !operation.changes(*register, abi),
                    "{profile:?} {register:?}"
                );
            }
        }
        for register in abi.callee_saved {
            assert!(
                !operation.changes(*register, abi),
                "{profile:?} {register:?}"
            );
        }
        let ordinary = call_to("assert_fail");
        for register in abi.caller_saved {
            assert!(ordinary.changes(*register, abi), "{profile:?} {register:?}");
        }
        // lengua's own functions have a flow and a stack without a fault of the kind the first
        // rules find, apart from the guard after an exit that can never run, and the calls to the
        // one helper that needs no aligned stack.
        let exits = [
            profile.syscall_number(Syscall::Exit),
            profile.syscall_number(Syscall::ExitGroup),
        ];
        let mut faults = Vec::new();
        for function in &program.lengua {
            cfg::analyze(function, &program, &exits, &[], &mut faults);
        }
        let faults: Vec<String> = faults
            .iter()
            .map(ToString::to_string)
            .filter(|fault| !fault.contains("can never run") && !fault.contains("16-byte aligned"))
            .collect();
        assert!(faults.is_empty(), "{profile:?}: {faults:#?}");
    }
}

/// A program in the edge form that passes every rule: it checks its arguments, writes a greeting
/// and exits.
const EDGE_BASE: PerCpu<&str> = PerCpu {
    x86_64: ".intel_syntax noprefix\n\n\
            .equ STDOUT, 1\n\
            .equ EXIT_OK, 0\n\
            .equ EXIT_WRITE_FAILED, 1\n\
            .equ ARGC_MIN, 1\n\
            .equ GREETING_LEN, 6\n\n\
            .section .rodata\n\n\
            .type greeting, @object\n\
            greeting:\n    .ascii \"hello\\n\"\n\
            .size greeting, . - greeting\n\n\
            .text\n\n\
            # Writes a greeting and exits: main gets the argument count, argv and envp.\n\
            .type main, @function\n\
            main:\n\
            \x20   sub rsp, 8\n\
            \x20   test rsi, rsi\n\
            \x20   jz .Lmain_assert_argv\n\
            \x20   cmp rdi, ARGC_MIN\n\
            \x20   jb .Lmain_assert_argc\n\
            \x20   mov edi, STDOUT\n\
            \x20   lea rsi, [rip + greeting]\n\
            \x20   mov edx, GREETING_LEN\n\
            \x20   call os_write\n\
            \x20   test rax, rax\n\
            \x20   js .Lmain_write_failed\n\
            \x20   mov edi, EXIT_OK\n\
            \x20   call os_exit\n\
            .Lmain_write_failed:\n\
            \x20   mov edi, EXIT_WRITE_FAILED\n\
            \x20   call os_exit\n\
            .Lmain_assert_argv:\n\
            \x20   mov edi, 1\n\
            \x20   call assert_fail\n\
            .Lmain_assert_argc:\n\
            \x20   mov edi, 2\n\
            \x20   call assert_fail\n\
            .size main, . - main\n",
    aarch64: ".arch armv8-a\n\n\
            .equ STDOUT, 1\n\
            .equ EXIT_OK, 0\n\
            .equ EXIT_WRITE_FAILED, 1\n\
            .equ ARGC_MIN, 1\n\
            .equ GREETING_LEN, 6\n\n\
            .section .rodata\n\n\
            .type greeting, @object\n\
            greeting:\n    .ascii \"hello\\n\"\n\
            .size greeting, . - greeting\n\n\
            .text\n\n\
            // Writes a greeting and exits: main gets the argument count, argv and envp.\n\
            .type main, @function\n\
            main:\n\
            \x20   cbz x1, .Lmain_assert_argv\n\
            \x20   cmp x0, #ARGC_MIN\n\
            \x20   b.lo .Lmain_assert_argc\n\
            \x20   mov x0, #STDOUT\n\
            \x20   adrp x1, greeting\n\
            \x20   add x1, x1, :lo12:greeting\n\
            \x20   mov x2, #GREETING_LEN\n\
            \x20   bl os_write\n\
            \x20   tbnz x0, #63, .Lmain_write_failed\n\
            \x20   mov x0, #EXIT_OK\n\
            \x20   bl os_exit\n\
            .Lmain_write_failed:\n\
            \x20   mov x0, #EXIT_WRITE_FAILED\n\
            \x20   bl os_exit\n\
            .Lmain_assert_argv:\n\
            \x20   mov w0, #1\n\
            \x20   bl assert_fail\n\
            .Lmain_assert_argc:\n\
            \x20   mov w0, #2\n\
            \x20   bl assert_fail\n\
            .size main, . - main\n",
};

/// What the base's spec allows: output and exit, which every program has.
const BASE_OPS: [crate::edge::Op; 2] = [crate::edge::Op::Write, crate::edge::Op::Exit];

/// The base passes every rule, on both CPUs.
#[tokio::test]
async fn an_edge_form_program_can_pass_every_rule() {
    for profile in Profile::ALL {
        let report = check_edge(EDGE_BASE.get(profile), &BASE_OPS).await;
        assert!(report.passed(), "{profile:?}: {:#?}", report.violations);
    }
}

/// One change to the base that a rule of the edge form refuses, for each CPU, and what the checker
/// says of it.
struct EdgeFixture {
    what: &'static str,
    edits: PerCpu<&'static [(&'static str, &'static str)]>,
    /// The operations the spec allows.
    ops: &'static [crate::edge::Op],
    says: &'static str,
}

/// Both CPUs' edit of the same text, where the instructions don't differ.
const fn both(
    edits: &'static [(&'static str, &'static str)],
) -> PerCpu<&'static [(&'static str, &'static str)]> {
    PerCpu {
        x86_64: edits,
        aarch64: edits,
    }
}

const EDGE_FIXTURES: &[EdgeFixture] = &[
    EdgeFixture {
        what: "a system call of its own",
        edits: PerCpu {
            x86_64: &[(
                "    mov edi, EXIT_OK\n    call os_exit\n",
                "    mov edi, EXIT_OK\n    mov eax, 231\n    syscall\n",
            )],
            aarch64: &[(
                "    mov x0, #EXIT_OK\n    bl os_exit\n",
                "    mov x0, #EXIT_OK\n    mov x8, #94\n    svc #0\n",
            )],
        },
        ops: &BASE_OPS,
        says: "makes a system call by itself",
    },
    EdgeFixture {
        what: "a _start beside main",
        edits: both(&[(
            ".text\n",
            ".text\n.globl _start\n.type _start, @function\n_start:\n    nop\n.size _start, . - _start\n",
        )]),
        ops: &BASE_OPS,
        says: "main.s defines main as well as _start",
    },
    EdgeFixture {
        what: "main exported",
        edits: both(&[(".text\n", ".text\n.globl main\n")]),
        ops: &BASE_OPS,
        says: "main is exported",
    },
    EdgeFixture {
        what: "a function named for a library function",
        edits: both(&[(
            ".size main, . - main\n",
            ".size main, . - main\n\n.type fill, @function\nfill:\n    ret\n.size fill, . - fill\n",
        )]),
        ops: &BASE_OPS,
        says: "fill is a name lengua keeps",
    },
    EdgeFixture {
        what: "a constant with lengua's prefix",
        edits: both(&[(".equ STDOUT, 1\n", ".equ STDOUT, 1\n.equ LENGUA_EXTRA, 7\n")]),
        ops: &BASE_OPS,
        says: "LENGUA_EXTRA is a name lengua keeps",
    },
    EdgeFixture {
        what: "an operation the spec doesn't allow",
        edits: PerCpu {
            x86_64: &[("    call os_write\n", "    call os_clock\n")],
            aarch64: &[("    bl os_write\n", "    bl os_clock\n")],
        },
        ops: &BASE_OPS,
        says: "os_clock isn't one the spec allows",
    },
    EdgeFixture {
        what: "a library function whose operations the spec doesn't allow",
        edits: PerCpu {
            x86_64: &[("    call os_write\n", "    call fill\n")],
            aarch64: &[("    bl os_write\n", "    bl fill\n")],
        },
        ops: &BASE_OPS,
        says: "fill isn't one the spec allows (it calls os_read)",
    },
    EdgeFixture {
        what: "a result nobody tests",
        edits: PerCpu {
            x86_64: &[("    test rax, rax\n    js .Lmain_write_failed\n", "")],
            aarch64: &[("    tbnz x0, #63, .Lmain_write_failed\n", "")],
        },
        ops: &BASE_OPS,
        says: "os_write's result",
    },
    EdgeFixture {
        what: "an exit status that isn't a named constant",
        edits: PerCpu {
            x86_64: &[("    mov edi, EXIT_OK\n", "    mov edi, 0\n")],
            aarch64: &[("    mov x0, #EXIT_OK\n", "    mov x0, #0\n")],
        },
        ops: &BASE_OPS,
        says: "without a named constant",
    },
    EdgeFixture {
        what: "a main that returns",
        edits: both(&[(".size main, . - main\n", "    ret\n.size main, . - main\n")]),
        ops: &BASE_OPS,
        says: "main returns, but it has nothing to return to",
    },
    EdgeFixture {
        what: "a call on a stack that isn't aligned",
        edits: PerCpu {
            x86_64: &[("    sub rsp, 8\n", "")],
            aarch64: &[("main:\n", "main:\n    sub sp, sp, #8\n")],
        },
        ops: &BASE_OPS,
        says: // x86-64 says the stack isn't 16-byte aligned; AArch64 never lets sp be anything else.
        "16",
    },
    EdgeFixture {
        what: "a frame as big as a page",
        edits: PerCpu {
            x86_64: &[("    sub rsp, 8\n", "    sub rsp, 4104\n")],
            aarch64: &[(
                "main:\n",
                "main:\n    sub sp, sp, #4080\n    sub sp, sp, #16\n",
            )],
        },
        ops: &BASE_OPS,
        says: "frame is",
    },
    EdgeFixture {
        what: "a call into lengua's own code",
        edits: PerCpu {
            x86_64: &[("    call os_write\n", "    call lengua_error\n")],
            aarch64: &[("    bl os_write\n", "    bl lengua_error\n")],
        },
        ops: &BASE_OPS,
        says: "enters lengua's own code",
    },
    EdgeFixture {
        what: "a register the edge changes, read after the call",
        edits: PerCpu {
            x86_64: &[(
                "    call os_write\n",
                "    call os_write\n    mov r8, rcx\n",
            )],
            aarch64: &[("    bl os_write\n", "    bl os_write\n    mov x9, x16\n")],
        },
        ops: &BASE_OPS,
        says: "is read after a call that reaches the OS",
    },
    EdgeFixture {
        what: "a vector register held across a call",
        edits: PerCpu {
            x86_64: &[(
                "    call os_write\n    test rax, rax\n",
                "    vpxor ymm0, ymm0, ymm0\n    call os_write\n    vmovdqu ymm1, ymm0\n    test rax, rax\n",
            )],
            aarch64: &[("    bl os_write\n", "    bl os_write\n    mov x9, x17\n")],
        },
        ops: &BASE_OPS,
        says: "is read after a call that reaches the OS",
    },
    EdgeFixture {
        what: "a jump into an operation",
        edits: PerCpu {
            x86_64: &[("    js .Lmain_write_failed\n", "    js os_exit\n")],
            aarch64: &[(
                "    tbnz x0, #63, .Lmain_write_failed\n",
                "    tbnz x0, #63, os_exit\n",
            )],
        },
        ops: &BASE_OPS,
        says: "jumps outside main",
    },
];

/// Each change is refused, on both CPUs, for the reason its rule gives.
#[tokio::test]
async fn the_edge_form_rules_refuse_what_they_name() {
    for fixture in EDGE_FIXTURES {
        for profile in Profile::ALL {
            let mut text = EDGE_BASE.get(profile).to_string();
            for (from, to) in *fixture.edits.get(profile) {
                assert!(
                    text.contains(from),
                    "{} on {profile:?}: no {from:?}",
                    fixture.what
                );
                text = text.replacen(from, to, 1);
            }
            let report = check_edge(&text, fixture.ops).await;
            let found: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
            assert!(
                found
                    .iter()
                    .any(|violation| violation.contains(fixture.says)),
                "{} on {profile:?}: wanted {:?} in {found:#?}",
                fixture.what,
                fixture.says
            );
        }
    }
}

/// What the edge keeps is still there after a call: an argument register, which the program gave
/// the operation, is read again with no word from the checker.
#[tokio::test]
async fn a_register_the_edge_keeps_is_readable_after_a_call() {
    for (profile, from, to) in [
        (
            Profile::LinuxX86_64,
            "    call os_write\n",
            "    call os_write\n    mov r8, rsi\n",
        ),
        (
            Profile::LinuxAarch64,
            "    bl os_write\n",
            "    bl os_write\n    mov x9, x1\n",
        ),
    ] {
        let text = EDGE_BASE.get(profile).replacen(from, to, 1);
        let report = check_edge(&text, &BASE_OPS).await;
        assert!(report.passed(), "{profile:?}: {:#?}", report.violations);
    }
}
