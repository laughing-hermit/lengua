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
//! with each entry `.long .Lcase - kinds`. Rule 9 holds the index to the table as it holds any
//! index to its buffer, so every target is one of the table's labels: control flow stays fixed
//! and known, and the control flow graph and the proof follow every case.

use std::collections::BTreeMap;

use iced_x86::{FlowControl, Instruction, OpKind, Register};

use super::decode::{MappedFunction, Program, mnemonic};
use super::{Rule, Violation};

/// A jump through a checked table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableJump {
    /// The step of the `jmp`.
    pub jump: usize,
    /// The steps of the cases, in the table's order.
    pub cases: Vec<usize>,
}

/// Whether `instruction` jumps through a register.
#[must_use]
pub fn is_register_jump(instruction: &Instruction) -> bool {
    instruction.flow_control() == FlowControl::IndirectBranch
        && instruction.op_kind(0) == OpKind::Register
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
                    let ip = |step: usize| function.steps[step].machine.ip();
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
        if !is_register_jump(&step.machine) {
            continue;
        }
        match cases(function, program, index) {
            Ok(cases) => found.push(TableJump { jump: index, cases }),
            Err(problem) => violations.push(Violation {
                rule: Rule::Indirection,
                line: Some(step.source.line),
                problem,
                fix: "jump through a register only through a checked jump table: `cmp` the index \
                      with the table's length and `jae` away, then `lea rcx, [rip + table]`, \
                      `movsxd rax, dword ptr [rcx + rax*4]`, `add rax, rcx` and `jmp rax`, with \
                      the table in .rodata holding `.long .Lcase - table` for labels after the \
                      jump in the same function"
                    .into(),
            }),
        }
    }
    found
}

/// The cases of the jump at `index`, when it and the three steps before it are a checked jump
/// table's shape.
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
    let target = function.steps[index].machine.op_register(0);
    let (lea_machine, load_machine, add_machine) = (&lea.machine, &load.machine, &add.machine);
    // add target, base
    if mnemonic(add_machine) != "add"
        || add_machine.op_kind(0) != OpKind::Register
        || add_machine.op_kind(1) != OpKind::Register
        || add_machine.op_register(0) != target
    {
        return Err(shape());
    }
    let base = add_machine.op_register(1);
    if base == target {
        return Err(shape());
    }
    // movsxd target, dword ptr [base + index*4]
    if mnemonic(load_machine) != "movsxd"
        || load_machine.op_register(0) != target
        || load_machine.op_kind(1) != OpKind::Memory
        || load_machine.memory_base() != base
        || load_machine.memory_index() == Register::None
        || load_machine.memory_index_scale() != 4
        || load_machine.memory_displacement64() != 0
    {
        return Err(shape());
    }
    // lea base, [rip + table]
    if mnemonic(lea_machine) != "lea" || lea_machine.op_register(0) != base {
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
