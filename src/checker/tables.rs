//! Checked jump tables, the one jump through a register the dialect allows (dialect 4). A
//! bounds-checked index picks an entry of a read-only table of the function's own labels, then
//! exactly
//!
//! ```text
//!     lea rcx, [rip + kinds]
//!     movsxd rax, dword ptr [rcx + rax*4]
//!     add rax, rcx
//!     jmp rax
//! ```
//!
//! with each entry `.long .Lcase - kinds`; on AArch64, exactly
//!
//! ```text
//!     adrp x1, kinds
//!     add x1, x1, :lo12:kinds
//!     ldrsw x2, [x1, x0, lsl #2]
//!     add x2, x2, x1
//!     br x2
//! ```
//!
//! Rule 9 holds the index to the table as it holds any index to its buffer, so every target is
//! one of the table's labels: control flow stays fixed and known, and the control flow graph and
//! the proof follow every case.

use std::collections::BTreeMap;

use super::decode::{MappedFunction, Program};
use super::{Rule, Violation};
use crate::isa::{Control, Insn, Isa, Op, Operand, Value};

/// A jump through a checked table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableJump {
    /// The step of the `jmp`.
    pub jump: usize,
    /// The steps of the cases, in the table's order.
    pub cases: Vec<usize>,
}

/// Whether `insn` jumps through a register.
#[must_use]
pub fn is_register_jump(insn: &Insn) -> bool {
    insn.control == Control::IndirectJump && matches!(insn.operands.as_slice(), [Operand::Reg(_)])
}

/// Every function's jumps through checked tables, by the function's name. Any other jump
/// through a register is a violation of rule 9.
pub fn every(
    program: &Program<'_>,
    violations: &mut Vec<Violation>,
) -> BTreeMap<String, Vec<TableJump>> {
    program
        .functions
        .iter()
        .map(|function| {
            (
                function.name().to_owned(),
                find(function, program, violations),
            )
        })
        .collect()
}

/// Each checked table jump's address and its cases' addresses, in the table's order, for
/// proofs, which see only addresses.
#[must_use]
pub fn addresses(
    program: &Program<'_>,
    jumps: &BTreeMap<String, Vec<TableJump>>,
) -> BTreeMap<u64, Vec<u64>> {
    program
        .functions
        .iter()
        .flat_map(|function| {
            jumps
                .get(function.name())
                .into_iter()
                .flatten()
                .map(move |jump| {
                    let ip = |step: usize| function.steps[step].insn.address;
                    (
                        ip(jump.jump),
                        jump.cases.iter().map(|case| ip(*case)).collect(),
                    )
                })
        })
        .collect()
}

/// The function's jumps through checked tables. Any other jump through a register is a
/// violation of rule 9.
pub fn find(
    function: &MappedFunction<'_>,
    program: &Program<'_>,
    violations: &mut Vec<Violation>,
) -> Vec<TableJump> {
    let mut found = Vec::new();
    for (index, step) in function.steps.iter().enumerate() {
        if !is_register_jump(&step.insn) {
            continue;
        }
        match cases(function, program, index) {
            Ok(cases) => found.push(TableJump { jump: index, cases }),
            Err(problem) => violations.push(Violation {
                rule: Rule::Indirection,
                line: Some(step.source.line),
                problem,
                fix: match program.isa {
                    Isa::X86_64 => {
                        "jump through a register only through a checked jump table: `cmp` the \
                         index with the table's length and `jae` away, then `lea rcx, [rip + \
                         table]`, `movsxd rax, dword ptr [rcx + rax*4]`, `add rax, rcx` and `jmp \
                         rax`, with the table in .rodata holding `.long .Lcase - table` for \
                         labels after the jump in the same function"
                    }
                    Isa::Aarch64 => {
                        "jump through a register only through a checked jump table: `cmp` the \
                         index with the table's length and `b.hs` away, then `adrp x1, table`, \
                         `add x1, x1, :lo12:table`, `ldrsw x2, [x1, x0, lsl #2]`, `add x2, x2, \
                         x1` and `br x2`, with the table in .rodata holding `.long .Lcase - \
                         table` for labels after the jump in the same function"
                    }
                }
                .into(),
            }),
        }
    }
    found
}

/// The cases of the jump at `index`, when it and the three steps before it are a checked jump
/// table's shape: the table's address (on AArch64, the `add` of its `adrp` pair, which the
/// dialect keeps together), its entry, and the entry added to the address.
fn cases(
    function: &MappedFunction<'_>,
    program: &Program<'_>,
    index: usize,
) -> Result<Vec<usize>, String> {
    let shape = || {
        format!(
            "`{}` jumps through a register, but not through a checked jump table",
            function.steps[index].source.text
        )
    };
    if index < 3 {
        return Err(shape());
    }
    // One straight run: no other path comes in between the table's address and the jump.
    if (index - 2..=index).any(|at| function.label_before(at).is_some()) {
        return Err(shape());
    }
    let (lea, load, add) = (
        &function.steps[index - 3],
        &function.steps[index - 2],
        &function.steps[index - 1],
    );
    let [Operand::Reg(target)] = function.steps[index].insn.operands[..] else {
        return Err(shape());
    };
    // add target, base; on AArch64 add target, target, base, unshifted
    let base = match (program.isa, &add.insn.operands[..], &add.insn.value) {
        (Isa::X86_64, &[Operand::Reg(added), Operand::Reg(base)], _) if added == target => base,
        (Isa::Aarch64, _, &Value::Add(Operand::Reg(added), Operand::Reg(base)))
            if added == target && add.insn.dst == Some(target) =>
        {
            base
        }
        _ => return Err(shape()),
    };
    if add.insn.mnemonic != "add" || base == target {
        return Err(shape());
    }
    // movsxd target, dword ptr [base + index*4]; on AArch64 ldrsw target, [base, index, lsl #2]
    let loads_entry = load.insn.memory.is_some_and(|memory| {
        memory.base == Some(base)
            && memory.index.is_some()
            && memory.scale == 4
            && memory.displacement == 0
    });
    let widens = match program.isa {
        Isa::X86_64 => "movsxd",
        Isa::Aarch64 => "ldrsw",
    };
    if load.insn.mnemonic != widens
        || !matches!(load.insn.operands[..], [Operand::Reg(loaded), Operand::Mem] if loaded == target)
        || !loads_entry
    {
        return Err(shape());
    }
    // lea base, [rip + table]; on AArch64 add base, base, :lo12:table
    if !matches!(lea.insn.op, Op::AddressOf { dst } if dst == base) {
        return Err(shape());
    }
    let Some((name, data, 0)) = program.named(lea) else {
        return Err(shape());
    };
    let symbol = program
        .source
        .data
        .iter()
        .find(|symbol| symbol.name == name)
        .ok_or_else(shape)?;
    if symbol.table.is_empty() {
        return Err(format!(
            "`{}` jumps through {name}, which isn't a jump table",
            function.steps[index].source.text
        ));
    }
    if data.size != symbol.table.len() as u64 * 4 {
        return Err(format!(
            "jump table {name} holds other data besides its {} entries",
            symbol.table.len()
        ));
    }
    // Each entry a label of this function, after the jump: a table jumps forward, so every
    // backward jump stays a loop's end.
    symbol
        .table
        .iter()
        .map(|label| {
            let at = function
                .labels
                .iter()
                .find(|at| at.label.name == *label)
                .ok_or_else(|| {
                    format!(
                        "jump table {name} names {label}, which isn't a label of {}",
                        function.name()
                    )
                })?;
            if at.index <= index {
                return Err(format!(
                    "jump table {name} names {label}, which comes before the jump: a table jumps \
                     forward only"
                ));
            }
            Ok(at.index)
        })
        .collect()
}
