//! AArch64's half of reading main.s back: each line of main.s matched to its machine word field
//! by field, through the one canonical form lengua reads both into, and what the dialect allows
//! of each instruction.

use super::decode::{MappedFunction, Program, Step, Word};
use super::source::{Function as SourceFunction, Instruction as SourceInstruction, Operand};
use super::{Rule, Violation};
use crate::isa::aarch64::sve::Sve;
use crate::isa::aarch64::{
    A64Reg, Address, Extend, Gpr, Indirect, Inst, Source, Writeback, facts, text,
};
use crate::isa::{Insn, MemRef, Op, Reg, Value};
use crate::platform::CpuFeatures;

/// The word's canonical form, or the word itself when it isn't in the dialect, for messages.
#[must_use]
pub fn display(word: &Word) -> String {
    word.inst
        .as_ref()
        .map_or_else(|_| format!("{:#010x}", word.bits), ToString::to_string)
}

/// The steps of an AArch64 function: each line of main.s with its word's facts, when there's
/// one word a line and every word is in the dialect.
pub(super) fn steps<'a>(
    function: &'a SourceFunction,
    sources: Vec<&'a SourceInstruction>,
    words: &[Word],
    violations: &mut Vec<Violation>,
) -> Option<Vec<Step<'a>>> {
    if sources.len() != words.len() {
        let line = sources
            .get(words.len())
            .map_or(function.line, |source| source.line);
        violations.push(Violation {
            rule: Rule::Dialect,
            line: Some(line),
            problem: format!(
                "main.s and the machine code of {} don't match line for line: it has {} lines \
                 of code and {} instructions",
                function.name,
                sources.len(),
                words.len()
            ),
            fix: "write exactly one instruction per line, with no directives between \
                  instructions"
                .into(),
        });
        return None;
    }
    let mut steps = Vec::new();
    let before = violations.len();
    for (source, word) in sources.into_iter().zip(words) {
        match &word.inst {
            Ok(inst) => steps.push(Step {
                source,
                insn: insn(source, inst, word.address),
            }),
            Err(_) => violations.push(Violation {
                rule: Rule::Dialect,
                line: Some(source.line),
                problem: format!(
                    "`{}` became {:#010x} in the machine code, which isn't an instruction \
                     lengua's AArch64 dialect has",
                    source.text, word.bits
                ),
                fix: "use the instructions the dialect reference lists: whole numbers, floating \
                      point in s and d registers, and vectors of whole numbers; no floating point \
                      on vectors, half precision, atomics or system registers"
                    .into(),
            }),
        }
    }
    (violations.len() == before).then_some(steps)
}

/// The facts of `inst`, the word the line `source` became. An `add` of `:lo12:name` completes
/// the address `adrp` began, as `lea` makes one on x86-64: its result is the address of the data
/// symbol it names.
fn insn(source: &SourceInstruction, inst: &Inst, address: u64) -> Insn {
    let mut insn = facts::facts(inst, address);
    let names_data = source.operands.iter().any(|operand| {
        matches!(operand, Operand::Memory(memory) if memory.symbol.is_some() && memory.base.is_none())
    });
    if let Inst::AddImm {
        sub: false,
        flags: false,
        rd,
        ..
    } = inst
        && names_data
    {
        insn.op = Op::AddressOf { dst: Reg::A64(*rd) };
        insn.value = Value::Address;
        insn.memory = Some(MemRef {
            base_moves: None,
            base: None,
            index: None,
            scale: 1,
            displacement: 0,
            pc_relative: true,
            width: 0,
            reads: false,
            writes: false,
        });
    }
    insn
}

/// What each name in a line of `function` stands for: a constant's value (or an expression of
/// constants), or the address of one of the function's labels, a function or a data symbol.
fn named(function: &MappedFunction<'_>, program: &Program<'_>, name: &str) -> Option<i64> {
    let constants = &program.source.constants;
    constants
        .get(name)
        .map(|constant| constant.value)
        .or_else(|| {
            function
                .labels
                .iter()
                .find(|at| at.label.name == name)
                .map(|at| at.address.cast_signed())
        })
        .or_else(|| {
            program
                .function(name)
                .map(|found| found.address.cast_signed())
        })
        // A call into lengua's unit lands on its function: where the rules then say what's
        // allowed to enter it.
        .or_else(|| {
            program
                .lengua_at_name(name)
                .map(|found| found.address.cast_signed())
        })
        .or_else(|| {
            program
                .data
                .get(name)
                .map(|data| data.address.cast_signed())
        })
        .or_else(|| super::source::evaluate(name, constants).ok())
}

/// Compares the line with the word it became, field by field: both read into their canonical
/// form, the line with the addresses its names have in the executable.
pub(super) fn same_instruction(
    step: &Step<'_>,
    word: &Word,
    function: &MappedFunction<'_>,
    program: &Program<'_>,
) -> Result<(), String> {
    let decoded = word.inst.as_ref().map_err(Clone::clone)?;
    let line = step.source.text.as_str();
    let (mnemonic, operands) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let names = |name: &str| named(function, program, name);
    let written = text::parse(mnemonic, operands.trim(), word.address, &names)?;
    if written == *decoded {
        Ok(())
    } else {
        Err(format!("as written it's `{written}`"))
    }
}

/// Every register the instruction names.
fn registers(inst: &Inst) -> Vec<A64Reg> {
    match *inst {
        Inst::Crypto { .. } => Vec::new(),
        Inst::Sve(sve) => match sve {
            Sve::Whilelo { rn, rm, .. } => vec![rn, rm],
            Sve::Access { base, index, .. } => vec![base, index],
            Sve::Cntp { rd, .. } | Sve::Count { rd, .. } | Sve::Lastb { rd, .. } => vec![rd],
            Sve::DupReg { rn, .. } => vec![rn],
            Sve::Insr { rm, .. } => vec![rm],
            _ => Vec::new(),
        },
        Inst::AddImm { rd, rn, .. }
        | Inst::LogicalImm { rd, rn, .. }
        | Inst::Bitfield { rd, rn, .. }
        | Inst::OneSource { rd, rn, .. } => vec![rd, rn],
        Inst::AddShifted { rd, rn, rm, .. }
        | Inst::LogicalShifted { rd, rn, rm, .. }
        | Inst::Extract { rd, rn, rm, .. }
        | Inst::CondSelect { rd, rn, rm, .. }
        | Inst::TwoSource { rd, rn, rm, .. }
        | Inst::Crc { rd, rn, rm, .. } => vec![rd, rn, rm],
        Inst::ThreeSource { rd, rn, rm, ra, .. } => vec![rd, rn, rm, ra],
        Inst::MoveWide { rd, .. } | Inst::Adrp { rd, .. } | Inst::Adr { rd, .. } => vec![rd],
        Inst::CompareBranch { rt, .. } | Inst::TestBranch { rt, .. } => vec![rt],
        Inst::BranchReg { rn, .. } => vec![rn],
        Inst::LoadStore { rt, address, .. } => [vec![rt], address_registers(address)].concat(),
        Inst::Pair {
            rt, rt2, address, ..
        } => [vec![rt, rt2], address_registers(address)].concat(),
        Inst::Branch { .. } | Inst::BranchCond { .. } | Inst::Svc { .. } | Inst::Nop => Vec::new(),
        // A floating-point instruction's general registers: what it moves to or from one, and
        // its address's.
        Inst::FpConvert { gpr, .. } => vec![gpr],
        Inst::FpLoadStore { address, .. } | Inst::FpPair { address, .. } => {
            address_registers(address)
        }
        Inst::FpOneSource { .. }
        | Inst::FpTwoSource { .. }
        | Inst::FpThreeSource { .. }
        | Inst::FpCompare { .. }
        | Inst::FpCondSelect { .. }
        | Inst::FpMoveImm { .. } => Vec::new(),
        // A vector instruction's general registers: what it moves to or from one, and its
        // address's.
        Inst::VectorDup {
            from: Source::General(gpr),
            ..
        }
        | Inst::VectorInsert {
            from: Source::General(gpr),
            ..
        }
        | Inst::VectorToGeneral { rd: gpr, .. } => vec![gpr],
        Inst::VectorLoadStore { address, .. } | Inst::VectorPair { address, .. } => {
            address_registers(address)
        }
        Inst::VectorStructures { base, step, .. } => match step {
            Some(Writeback::Register(by)) => vec![base, by],
            _ => vec![base],
        },
        Inst::VectorThree { .. }
        | Inst::Pmull { .. }
        | Inst::VectorTwo { .. }
        | Inst::VectorAcross { .. }
        | Inst::VectorShift { .. }
        | Inst::VectorPermute { .. }
        | Inst::VectorExtract { .. }
        | Inst::VectorTable { .. }
        | Inst::VectorDup { .. }
        | Inst::VectorInsert { .. }
        | Inst::VectorImmediate { .. } => Vec::new(),
    }
}

/// The registers an address names.
fn address_registers(address: Address) -> Vec<A64Reg> {
    match address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. } => vec![base],
        Address::Register { base, index, .. } => vec![base, index],
    }
}

/// What the dialect allows of the instruction the line became.
pub(super) fn check_instruction(
    step: &Step<'_>,
    word: &Word,
    features: CpuFeatures,
    violations: &mut Vec<Violation>,
) {
    let Ok(inst) = &word.inst else {
        return;
    };
    let line = Some(step.source.line);
    let text = &step.source.text;
    let mut refuse = |problem: String, fix: &str| {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem,
            fix: fix.into(),
        });
    };
    if registers(inst)
        .iter()
        .any(|register| register.gpr == Gpr::R(18))
    {
        refuse(
            format!("`{text}` uses x18, the platform register on Apple's and Windows' ARM64"),
            "use another register: x0 to x15 for values that live within a function, x19 to x28 \
             for ones that live across calls",
        );
    }
    if matches!(inst, Inst::Crc { .. }) && !features.crc {
        refuse(
            format!("`{text}` needs CRC, which this CPU doesn't have"),
            "compute the checksum in software, as from a table, on a CPU without CRC",
        );
    }
    if let Inst::Crypto { op, .. } = inst
        && !(if op.aes() { features.aes } else { features.sha })
    {
        refuse(
            format!(
                "`{text}` needs the {} extension, which this CPU doesn't have",
                if op.aes() { "AES" } else { "SHA2" }
            ),
            "do without the CPU's AES or SHA instructions on this CPU",
        );
    }
    if matches!(inst, Inst::Sve(_)) && features.sve == 0 {
        refuse(
            format!(
                "`{text}` is SVE, which this CPU doesn't have, or with vectors past the {} bytes \
                 lengua's model holds",
                crate::platform::SVE_MAX_BYTES
            ),
            "use NEON's 16-byte vectors, which every ARM64 CPU has",
        );
    }
    if let Inst::Pmull { rn, .. } = inst
        && crate::isa::aarch64::pmull_wide(*rn)
        && !features.pmull
    {
        refuse(
            format!("`{text}` needs the PMULL extension, which this CPU doesn't have"),
            "multiply bytes with `pmull v0.8h, v1.8b, v2.8b`, which every ARM64 CPU has, or do \
             without carry-less multiplication",
        );
    }
    match *inst {
        Inst::Adr { .. } => refuse(
            format!("`{text}`: adr isn't in the dialect"),
            "make an address with `adrp xN, name` followed by `add xN, xN, :lo12:name`",
        ),
        Inst::BranchReg {
            kind: Indirect::Blr,
            ..
        } => refuse(
            format!("`{text}` calls through a register"),
            "call functions by name, with bl",
        ),
        Inst::BranchReg {
            kind: Indirect::Ret,
            rn,
        } if rn.gpr != Gpr::R(30) => refuse(
            format!("`{text}` returns through a register other than x30"),
            "return with plain `ret`, through x30, which the frame record keeps",
        ),
        Inst::Svc { imm } if imm != 0 => refuse(
            format!("`{text}`: Linux takes system calls as `svc #0`"),
            "write `svc #0`, with the call's number in x8",
        ),
        // A bounds check compares the whole index, so the access uses the whole of it; a
        // sign-extended low half could be negative however the whole compares.
        Inst::LoadStore {
            address:
                Address::Register {
                    extend: extend @ (Extend::Uxtw | Extend::Sxtw | Extend::Sxtx),
                    ..
                },
            ..
        }
        | Inst::FpLoadStore {
            address:
                Address::Register {
                    extend: extend @ (Extend::Uxtw | Extend::Sxtw | Extend::Sxtx),
                    ..
                },
            ..
        }
        | Inst::VectorLoadStore {
            address:
                Address::Register {
                    extend: extend @ (Extend::Uxtw | Extend::Sxtw | Extend::Sxtx),
                    ..
                },
            ..
        } => refuse(
            format!(
                "`{text}` widens its index with {}",
                match extend {
                    Extend::Uxtw => "uxtw",
                    Extend::Sxtw => "sxtw",
                    _ => "sxtx",
                }
            ),
            "index with a whole x register, as `[xB, xI]` or `[xB, xI, lsl #3]`, after \
             `cmp xI, LEN` and `b.hs` away",
        ),
        _ => {}
    }
}

/// The data symbol a `:lo12:` operand of `step` names, if it has one.
fn low_bits_of<'a>(step: &'a Step<'_>) -> Option<&'a str> {
    step.source
        .operands
        .iter()
        .find_map(|operand| match operand {
            Operand::Memory(memory) if memory.base.is_none() => memory.symbol.as_deref(),
            _ => None,
        })
}

/// Each `add xD, xD, :lo12:name` comes right after `adrp xD, name`: only then does it make
/// name's address, as its facts say.
pub(super) fn check_pairs(function: &MappedFunction<'_>, violations: &mut Vec<Violation>) {
    for (index, step) in function.steps.iter().enumerate() {
        let Some(symbol) = low_bits_of(step) else {
            continue;
        };
        let register = match step.insn.op {
            Op::AddressOf { dst } => Some(dst),
            _ => None,
        };
        let pairs = index.checked_sub(1).is_some_and(|before| {
            let adrp = &function.steps[before];
            function.label_before(index).is_none()
                && adrp.source.mnemonic == "adrp"
                && matches!(
                    adrp.source.operands.as_slice(),
                    [Operand::Register(page), Operand::Target(name)]
                        if Some(*page) == register && name == symbol
                )
                && matches!(
                    step.source.operands.as_slice(),
                    [Operand::Register(rd), Operand::Register(rn), _]
                        if Some(*rd) == register && rd == rn
                )
        });
        if !pairs {
            violations.push(Violation {
                rule: Rule::Dialect,
                line: Some(step.source.line),
                problem: format!(
                    "`{}` isn't right after `adrp` of {symbol} into the same register, so it \
                     doesn't make {symbol}'s address",
                    step.source.text
                ),
                fix: format!(
                    "make an address with `adrp xN, {symbol}` and, on the next line, \
                     `add xN, xN, :lo12:{symbol}`"
                ),
            });
        }
    }
}

/// The rules' messages say x86-64's instructions and registers; on AArch64 these phrases say
/// AArch64's instead, longest first so a long phrase is replaced before a shorter one inside it.
const WORDS: [(&str, &str); 43] = [
    (
        "it gets its index in rdi and its slot in rsi, and returns a result in rax",
        "it gets its index in x0 and its slot in x1, and returns a result in x0",
    ),
    (
        "keep rbx as worker sets it, `mov rbx, rsi` before any call; use other registers for \
         everything else",
        "keep x19 as worker sets it, `mov x19, x1` before any call; use other registers for \
         everything else",
    ),
    (
        "a worker writes only its own stack, [rsp + …], and its slot, [rbx + …]",
        "a worker writes only its own stack, [sp, …], and its slot, [x19, …]",
    ),
    (
        "write results into the slot, [rbx + …], or return them in rax",
        "write results into the slot, [x19, …], or return them in x0",
    ),
    (
        "start worker with `push rbx`, then `mov rbx, rsi`",
        "start worker with `stp x19, x20, [sp, #-16]!`, then `mov x19, x1`",
    ),
    (
        "a worker reaches only its own slot, through rbx, as worker sets it",
        "a worker reaches only its own slot, through x19, as worker sets it",
    ),
    (
        "changes rbx, which holds the worker's slot",
        "changes x19, which holds the worker's slot",
    ),
    (
        "uses rbx before it holds the slot",
        "uses x19 before it holds the slot",
    ),
    (
        "push callee-saved registers (rbx, rbp, r12-r15) at the start and pop them in reverse order \
         right before ret",
        "save callee-saved registers (x19 to x29, d8 to d15) in stp pairs at the start and restore \
         them with ldp in reverse order right before ret",
    ),
    (
        "calls change rax, rcx, rdx, rsi, rdi and r8-r11, and syscall changes rax, rcx and r11, so \
         loops with calls count in rbx or r12-r15",
        "calls change x0 to x18 and x30, and svc changes x0, so loops with calls count in x19 to \
         x28",
    ),
    (
        "change rsp only with push, pop, call, ret, and add or sub rsp with a constant",
        "change sp only with the frame record's stp and ldp, bl, ret, and add or sub sp with a \
         multiple of 16",
    ),
    (
        "adjust rsp by 8 (sub rsp, 8 … add rsp, 8, or an extra push) so calls happen at a multiple \
         of 16",
        "keep sp a multiple of 16",
    ),
    (
        "`test rax, rax` and a conditional jump (js) to the error path",
        "`tbnz x0, #63, …` to the error path",
    ),
    (
        "pop the same registers in reverse order, immediately before ret",
        "restore the same registers with ldp in reverse order, immediately before ret",
    ),
    (
        "`mov edi, <number 1-999>` and `call assert_fail`",
        "`mov w0, #<number 1-999>` and `bl assert_fail`",
    ),
    (
        "then dec R and jnz .Lname",
        "then subs xR, xR, #1 and b.ne .Lname (or sub xR, xR, #1 and cbnz xR, .Lname)",
    ),
    (
        "the frame the function made with push and sub rsp",
        "the frame the function made with stp and sub sp",
    ),
    (
        "undo every push and sub rsp before ret",
        "undo every stp and sub sp before ret",
    ),
    (
        "end every path with ret, jmp,",
        "end every path with ret, b,",
    ),
    (
        "jump away with jae (or ja)",
        "branch away with b.hs (or b.hi)",
    ),
    ("lea [rip + name]", "adrp and add :lo12:"),
    ("as [rip + name]", "with adrp and add :lo12:"),
    ("[rsp + d]", "[sp, #d]"),
    ("`mov edi, EXIT_NAME`", "`mov w0, #EXIT_NAME`"),
    ("set edi with", "set w0 with"),
    ("`mov R, NAME`", "`mov xR, #NAME`"),
    ("mov R, NAME", "mov xR, #NAME"),
    ("never change R inside", "never change xR inside"),
    ("rax is used before", "x0 is used before"),
    // The annotations, in AArch64's comments.
    ("# bound:", "// bound:"),
    ("# unsafe:", "// unsafe:"),
    ("# end unsafe", "// end unsafe"),
    ("# shared:", "// shared:"),
    (", LEN)", ", #LEN)"),
    (", LEN`", ", #LEN`"),
    ("(jae)", "(b.hs)"),
    ("unconditional `jmp`", "unconditional `b`"),
    ("with `jmp` to", "with `b` to"),
    ("comparison `jne`", "comparison `b.ne`"),
    (
        "assets/main-template.s",
        "assets/linux-aarch64/main-template.s",
    ),
    ("below rsp", "below sp"),
    ("sub rsp", "sub sp"),
    ("at rsp + ", "at sp + "),
];

/// `violation` in AArch64's words.
pub(super) fn in_words(violation: &mut Violation) {
    for (x86, arm) in WORDS {
        violation.problem = violation.problem.replace(x86, arm);
        violation.fix = violation.fix.replace(x86, arm);
    }
}

/// SVE's names in a program's needs, by its vector length: what it was proven at, 16, 32, 48 or
/// 64 bytes.
const SVE_LENGTHS: [&str; 4] = ["sve16", "sve32", "sve48", "sve64"];

/// The CPU extensions an AArch64 program's instructions need beyond the base, each once: `crc`,
/// `pmull`, and SVE as `sve` with the vector length its proof ran at, since a proof holds for
/// that length alone.
#[must_use]
pub fn features_needed(
    machine: &crate::checker::decode::Machine,
    features: CpuFeatures,
) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for function in machine.functions.values() {
        for word in function.words() {
            let Ok(inst) = &word.inst else {
                continue;
            };
            let name = match inst {
                Inst::Crc { .. } => Some("crc"),
                Inst::Pmull { rn, .. } if crate::isa::aarch64::pmull_wide(*rn) => Some("pmull"),
                Inst::Crypto { op, .. } if op.aes() => Some("aes"),
                Inst::Crypto { .. } => Some("sha"),
                Inst::Sve(_) => SVE_LENGTHS
                    .get(usize::from(features.sve / 16).wrapping_sub(1))
                    .copied(),
                _ => None,
            };
            if let Some(name) = name
                && !found.contains(&name)
            {
                found.push(name);
            }
        }
    }
    found
}

/// Whether `cpu` has the AArch64 extension `name` names, as [`features_needed`] names them, or
/// none for a name that isn't one.
#[must_use]
pub fn cpu_has(name: &str, cpu: &CpuFeatures) -> Option<bool> {
    match name {
        "crc" => Some(cpu.crc),
        "pmull" => Some(cpu.pmull),
        "aes" => Some(cpu.aes),
        "sha" => Some(cpu.sha),
        _ => {
            let place = SVE_LENGTHS.iter().position(|known| *known == name)?;
            Some(usize::from(cpu.sve) == 16 * (place + 1))
        }
    }
}
