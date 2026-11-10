//! The rule over made-up code, for both CPUs on every computer; then over the code the engine
//! compiles here: secret-equal's passes, a shift of 8-bit lanes by a secret amount is refused on
//! x86-64, whose code looks a mask up in a table, and passes on ARM64, whose code is straight,
//! and a branch is refused when it is made for an operator that takes a secret, and only then.

use wasmparser::Operator;

use super::*;
use crate::checker::wasm::model;
use crate::checker::wasm::{Context, Report};
use crate::idea::Source;
use crate::wasm::text::{Built, build};

/// An operator at `offset` that takes a secret, held as `takes`.
fn taken(offset: u64, takes: Takes) -> Taken {
    Taken {
        offset,
        line: Some(7),
        name: "i32.xor".to_owned(),
        takes,
    }
}

/// What the rule refuses in `code` for `cpu` when the operators `taken` take a secret: each
/// operator's offset and what its code has.
fn refused(cpu: Cpu, code: &Code, taken: &[Taken]) -> Vec<(u64, Made)> {
    judge(cpu, code, taken)
        .findings
        .iter()
        .map(|finding| (finding.offset, finding.made))
        .collect()
}

/// x86-64 code of one function: `xor eax, ecx` made for no operator, `jb` made for the operator
/// at 10, `jb` made for the one at 20, and `ret`.
fn x86_64_code() -> Code {
    Code {
        text: vec![0x31, 0xC8, 0x72, 0x00, 0x72, 0x00, 0xC3],
        functions: vec![(0, 7)],
        map: vec![(0, None), (2, Some(10)), (4, Some(20)), (6, None)],
    }
}

/// ARM64 code of one function: `eor` made for no operator, `b.eq` made for the operator at 10,
/// `cbz` made for the one at 20, and `ret`.
fn aarch64_code() -> Code {
    let words: [u32; 4] = [0x4A02_0020, 0x5400_0040, 0xB400_0040, 0xD65F_03C0];
    Code {
        text: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
        functions: vec![(0, 16)],
        map: vec![(0, None), (4, Some(10)), (8, Some(20)), (12, None)],
    }
}

/// A branch is refused when the address map gives it an operator that takes a secret, on that
/// operator, and only then: the same code passes when no operator takes one, or another does.
#[test]
fn a_branch_is_refused_for_the_operator_it_was_made_for() {
    for (cpu, code) in [
        (Cpu::X86_64, x86_64_code()),
        (Cpu::Aarch64, aarch64_code()),
    ] {
        assert!(refused(cpu, &code, &[]).is_empty(), "{cpu:?}");
        assert_eq!(
            refused(cpu, &code, &[taken(10, Takes::Plain)]),
            [(10, Made::Branch)],
            "{cpu:?}"
        );
        assert_eq!(
            refused(cpu, &code, &[taken(20, Takes::Plain)]),
            [(20, Made::Branch)],
            "{cpu:?}"
        );
        // An operator with no code of its own has nothing to refuse.
        let other = judge(cpu, &code, &[taken(30, Takes::Plain)]);
        assert_eq!((other.operators, other.instructions), (0, 0), "{cpu:?}");
        assert!(other.findings.is_empty(), "{cpu:?}");
        // One instruction is read for each operator that has code.
        let both = judge(
            cpu,
            &code,
            &[taken(10, Takes::Plain), taken(20, Takes::Plain)],
        );
        assert_eq!((both.operators, both.instructions), (2, 2), "{cpu:?}");
    }
}

/// What the secret rule holds of an operator, its code may do: a `select`'s may branch, since
/// its condition isn't secret, and so may a load's or a store's, which checks an address; the
/// code of a call of main.wat's own function may not, and nor may code that computes.
#[test]
fn an_operators_code_may_do_what_the_secret_rule_holds_of_it() {
    for (cpu, code) in [
        (Cpu::X86_64, x86_64_code()),
        (Cpu::Aarch64, aarch64_code()),
    ] {
        for takes in [Takes::Choice, Takes::Memory] {
            assert!(
                refused(cpu, &code, &[taken(10, takes)]).is_empty(),
                "{cpu:?}"
            );
        }
        for takes in [Takes::Plain, Takes::Call] {
            assert_eq!(
                refused(cpu, &code, &[taken(10, takes)]),
                [(10, Made::Branch)],
                "{cpu:?}"
            );
        }
    }
    let kinds = [
        Made::Branch,
        Made::IndirectJump,
        Made::Call,
        Made::IndirectCall,
        Made::Division,
        Made::Table,
        Made::Unread,
    ];
    let may = |takes: Takes| -> Vec<Made> {
        kinds
            .into_iter()
            .filter(|made| allowed(takes, *made))
            .collect()
    };
    assert!(may(Takes::Plain).is_empty());
    assert_eq!(may(Takes::Choice), [Made::Branch]);
    assert_eq!(may(Takes::Call), [Made::Call]);
    assert_eq!(
        may(Takes::Memory),
        [Made::Branch, Made::Call, Made::IndirectCall, Made::Table]
    );
}

/// Bytes that are no instruction are refused, not passed, whatever the secret rule holds of the
/// operator they were made for.
#[test]
fn code_that_cant_be_read_is_refused() {
    // x86-64: `push es`, which 64-bit code doesn't have. ARM64: a word of a group with no
    // instructions.
    let codes = [
        (Cpu::X86_64, vec![0x06_u8]),
        (Cpu::Aarch64, 0x0200_0000_u32.to_le_bytes().to_vec()),
    ];
    for (cpu, text) in codes {
        let code = Code {
            functions: vec![(0, text.len())],
            text,
            map: vec![(0, Some(10))],
        };
        for takes in [Takes::Plain, Takes::Choice, Takes::Memory, Takes::Call] {
            assert_eq!(
                refused(cpu, &code, &[taken(10, takes)]),
                [(10, Made::Unread)],
                "{cpu:?} {takes:?}"
            );
        }
    }
}

/// Only the module's functions are read, each stretch up to its function's end: a branch past
/// the end, though the address map's last stretch has no end of its own, isn't the operator's.
#[test]
fn a_stretch_ends_with_its_function() {
    // `xor eax, ecx` ends the function; `jb` after it is another's code.
    let code = Code {
        text: vec![0x31, 0xC8, 0x72, 0x00],
        functions: vec![(0, 2)],
        map: vec![(0, Some(10))],
    };
    let judged = judge(Cpu::X86_64, &code, &[taken(10, Takes::Plain)]);
    assert_eq!((judged.operators, judged.instructions), (1, 1));
    assert!(judged.findings.is_empty(), "{:?}", judged.findings);
    // With the function two bytes longer, the branch is its code.
    let longer = Code {
        functions: vec![(0, 4)],
        ..code
    };
    assert_eq!(
        refused(Cpu::X86_64, &longer, &[taken(10, Takes::Plain)]),
        [(10, Made::Branch)]
    );
}

/// A finding is a violation of the secret rule on its operator's line, which names the CPU,
/// the operator, what its code has, and what to write instead.
#[test]
fn a_finding_is_a_violation_on_its_operators_line() {
    let operators = [taken(10, Takes::Plain)];
    let finding = Finding {
        offset: 10,
        made: Made::Table,
        name: "lea".to_owned(),
    };
    let found = violations(Cpu::X86_64, &operators, &[finding]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, Rule::Secrets);
    assert_eq!(found[0].line, Some(7));
    let line = found[0].in_file("main.wat");
    assert!(
        line.starts_with(
            "secrets main.wat:7: on x86-64, the engine compiles this `i32.xor`, which takes a \
             secret, to code with the address of a table in the code"
        ),
        "{line}"
    );
    assert!(line.contains("(`lea`)"), "{line}");
    assert!(
        line.contains("Fix: shift by an amount that isn't secret"),
        "{line}"
    );
}

/// A program that reads a secret from standard input into `$text`, with `;; plant` where each
/// test puts its code. Only what the secret rule gives of it is the tests'.
const PROGRAM: &str = r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1 1)
  (global $STDIN i32 (i32.const 0))
  (global $EXIT_OK i32 (i32.const 0))
  (global $READ_TO_END i32 (i32.const -1))
  (global $text i32 (i32.const 1024))
  (global $text.size i32 (i32.const 64))
  (func $main (export "_start")
    (local $got i32)
    (local.set $got
      (call $fill (global.get $STDIN) (global.get $text) (global.get $text.size)
        (global.get $READ_TO_END)))
    ;; plant
    (call $proc_exit (global.get $EXIT_OK))
    unreachable))
"#;

/// The line number, counting from 1, where `needle` first starts in `text`.
fn line_of(text: &str, needle: &str) -> usize {
    let position = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} isn't in the text"));
    text[..position].matches('\n').count() + 1
}

/// `text` built, and what the rules say of it for an idea that uses standard input and names
/// `secrets` as secret.
fn checked(text: &str, secrets: &[Source]) -> (Built, Report) {
    let built = build(text).unwrap_or_else(|problem| panic!("it builds: {problem}"));
    let (uses, network) =
        crate::idea::constraints(&["standard input"], "").expect("valid constraints");
    let report = crate::checker::wasm::check(
        text,
        &built,
        &Context {
            uses: &uses,
            network: &network,
            secrets,
        },
    );
    (built, report)
}

/// [`PROGRAM`] with `code` on its `;; plant` line, standard input secret: its text, and what
/// the check of its machine code says, as a run and a run from a shell compile it here.
fn planted(code: &str) -> (String, Result<Passed, Vec<Violation>>) {
    let text = PROGRAM.replacen("    ;; plant\n", &format!("    {code}\n"), 1);
    let (built, report) = checked(&text, &[Source::StandardInput]);
    assert!(
        report
            .secret_operators
            .iter()
            .any(|taken| taken.name == "i8x16.shl"),
        "{:?}",
        report.secret_operators
    );
    let result = check(
        &built.wasm,
        &report.secret_operators,
        &[Setup::Run, Setup::Shell],
        None,
    );
    (text, result)
}

/// The lines of `violations`, as a report shows them.
fn shown(violations: &[Violation]) -> Vec<String> {
    violations
        .iter()
        .map(|violation| violation.in_file("main.wat"))
        .collect()
}

/// A shift of 8-bit lanes by a secret amount passes the secret rule, which sees no branch and
/// no address in it. On x86-64 the engine's code looks its mask up in a table at an index made
/// from the amount, and the check refuses it on the shift's line; on ARM64 the engine's code is
/// straight, and it passes.
#[test]
fn a_lane_shift_by_a_secret_amount_is_refused_where_its_code_reads_a_table() {
    let (text, result) = planted(
        "(v128.store (global.get $text) (i8x16.shl (v128.load (global.get $text)) \
         (i32.load8_u offset=16 (global.get $text))))",
    );
    match Cpu::here() {
        Some(Cpu::X86_64) => {
            let refused = result.expect_err("x86-64's code reads a table");
            let lines = shown(&refused);
            let line = line_of(&text, "(v128.store");
            let expected = format!(
                "secrets main.wat:{line}: on x86-64, the engine compiles this `i8x16.shl`, \
                 which takes a secret, to code with the address of a table in the code"
            );
            assert!(
                lines.iter().any(|shown| shown.starts_with(&expected)),
                "{lines:#?}"
            );
            assert_eq!(refused.len(), 1, "{lines:#?}");
        }
        Some(Cpu::Aarch64) => {
            let passed = result.unwrap_or_else(|refused| panic!("{:#?}", shown(&refused)));
            assert_eq!(passed.cpu, Cpu::Aarch64);
            assert!(passed.with_code > 0 && passed.instructions > 0, "{passed:?}");
        }
        None => panic!("lengua is built for x86-64 and ARM64"),
    }
}

/// The same shift by a constant passes on both CPUs: the engine loads one mask from the code,
/// with no index. By an amount that varies and isn't secret, x86-64's code reads the table all
/// the same, and the check, which can't tell which operand the index came from, refuses it.
#[test]
fn a_lane_shift_by_a_constant_passes() {
    let (_, constant) = planted(
        "(v128.store (global.get $text) (i8x16.shl (v128.load (global.get $text)) (i32.const \
         3)))",
    );
    let passed = constant.unwrap_or_else(|refused| panic!("{:#?}", shown(&refused)));
    assert!(passed.with_code > 0 && passed.instructions > 0, "{passed:?}");
    let (_, varying) = planted(
        "(v128.store (global.get $text) (i8x16.shl (v128.load (global.get $text)) (local.get \
         $got)))",
    );
    assert_eq!(
        varying.is_err(),
        Cpu::here() == Some(Cpu::X86_64),
        "{varying:?}"
    );
}

/// The offset in the module of the first operator of `text`'s own functions that `wanted`
/// takes.
fn offset_of(text: &str, built: &Built, wanted: impl Fn(&Operator<'_>) -> bool) -> u64 {
    let module = model::Module::read(text, built).expect("the module reads back");
    module
        .own()
        .flat_map(|function| function.body.iter())
        .find(|op| wanted(&op.operator))
        .map(|op| op.offset)
        .expect("the operator is in main.wat")
}

/// The engine's own code, on this CPU: a `br_if` on a byte of memory compiles to a conditional
/// branch, which the address map gives the `br_if`. Named as an operator that takes a secret,
/// it is refused; with the `i32.xor` beside it named instead, whose code is straight, the same
/// module passes, and so it does with nothing named.
#[test]
fn the_engines_branch_is_refused_only_for_the_operator_it_was_made_for() {
    let text = r#"(module
  (memory (export "memory") 1)
  (func (export "_start")
    (block $out
      (br_if $out (i32.load8_u (i32.const 16)))
      (i32.store8 (i32.const 17)
        (i32.xor (i32.load8_u (i32.const 18)) (i32.load8_u (i32.const 19)))))))
"#;
    let built = build(text).unwrap_or_else(|problem| panic!("it builds: {problem}"));
    let named = |name: &str, offset: u64| Taken {
        offset,
        line: Some(line_of(text, name)),
        name: name.to_owned(),
        takes: Takes::Plain,
    };
    let branch = named(
        "br_if",
        offset_of(text, &built, |op| matches!(op, Operator::BrIf { .. })),
    );
    let xor = named(
        "i32.xor",
        offset_of(text, &built, |op| matches!(op, Operator::I32Xor)),
    );
    let cpu = Cpu::here().expect("lengua is built for x86-64 and ARM64");
    for setup in [Setup::Run, Setup::Shell] {
        let refused = check(&built.wasm, std::slice::from_ref(&branch), &[setup], None)
            .expect_err("the branch is the br_if's");
        let lines = shown(&refused);
        let expected = format!(
            "secrets main.wat:5: on {}, the engine compiles this `br_if`, which takes a secret, \
             to code with a conditional branch",
            cpu.name()
        );
        assert!(
            lines.iter().any(|line| line.starts_with(&expected)),
            "{setup:?}: {lines:#?}"
        );
        let passed = check(&built.wasm, std::slice::from_ref(&xor), &[setup], None)
            .unwrap_or_else(|refused| panic!("{setup:?}: {:#?}", shown(&refused)));
        assert_eq!(passed.with_code, 1, "{setup:?}: {passed:?}");
        assert!(passed.instructions > 0, "{setup:?}: {passed:?}");
        let nothing = check(&built.wasm, &[], &[setup], None)
            .unwrap_or_else(|refused| panic!("{setup:?}: {:#?}", shown(&refused)));
        assert_eq!((nothing.with_code, nothing.instructions), (0, 0));
    }
}

/// secret-equal, the shipped program whose idea names standard input as secret, passes the
/// rules and then the check of its machine code on this CPU, which reads code for its operators
/// that take a secret. With no secret named, the rules give no operator to check.
#[test]
fn secret_equal_s_machine_code_passes() {
    let text = include_str!("../../../../examples/secret-equal/main.wat");
    let (built, report) = checked(text, &[Source::StandardInput]);
    assert!(report.passed(), "{:#?}", report.lines());
    let names: Vec<&str> = report
        .secret_operators
        .iter()
        .map(|taken| taken.name.as_str())
        .collect();
    for name in ["i32.xor", "i32.or", "i32.eqz", "i32.sub", "i32.and", "i32.store"] {
        assert!(names.contains(&name), "{name} isn't in {names:?}");
    }
    assert!(
        report
            .secret_operators
            .iter()
            .all(|taken| taken.line.is_some()),
        "{:?}",
        report.secret_operators
    );
    let passed = check(
        &built.wasm,
        &report.secret_operators,
        &[Setup::Run, Setup::Shell],
        None,
    )
    .unwrap_or_else(|refused| panic!("{:#?}", shown(&refused)));
    assert_eq!(Some(passed.cpu), Cpu::here());
    assert_eq!(passed.operators, report.secret_operators.len());
    assert!(passed.with_code > 0 && passed.instructions > 0, "{passed:?}");
    assert!(passed.line().contains(passed.cpu.name()), "{}", passed.line());
    let (_, public) = checked(text, &[]);
    assert!(public.secret_operators.is_empty());
}
