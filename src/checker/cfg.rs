//! Control flow and stack depth: which instruction can follow which, what can run at all, and
//! exactly how deep the stack is before every instruction.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::decode::{MappedFunction, Program};
use super::{Rule, Violation};
use crate::isa::{Abi, Control, Op, Reg, Stack};

/// The function every assertion calls.
pub const ASSERT_FAIL: &str = "assert_fail";

/// How control leaves an instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// Falls through to the next instruction.
    Next,
    /// Jumps to the instruction at this index.
    Jump(usize),
    /// Jumps to this index or falls through.
    Branch(usize),
    /// Returns to the caller.
    Return,
    /// Calls a function, which returns unless it's `assert_fail`.
    Call {
        /// The function's name.
        callee: String,
    },
    /// A system call; exits end the program.
    Syscall {
        /// Whether it's `exit` or `exit_group`.
        exit: bool,
    },
    /// A call into lengua's edge: one of its operations, which keeps what the edge's convention
    /// keeps. `os_exit` ends the program.
    Edge {
        /// The operation.
        op: crate::edge::Op,
    },
    /// Jumps through a checked table to the instruction at one of these indexes.
    Table(Vec<usize>),
    /// Something the dialect doesn't allow; already reported.
    Stop,
}

/// The control flow and stack depths of one function.
#[derive(Debug, Clone)]
pub struct Flow {
    /// How control leaves each instruction.
    pub exits: Vec<Exit>,
    /// The instructions that can run next, for each instruction.
    pub successors: Vec<Vec<usize>>,
    /// The instructions that can run just before, for each instruction.
    pub predecessors: Vec<Vec<usize>>,
    /// Whether each instruction can run at all.
    pub reachable: Vec<bool>,
    /// Bytes on the stack before each instruction, counting the return address.
    pub depth: Vec<Option<i64>>,
    /// The deepest the stack gets inside the function itself.
    pub max_depth: i64,
}

/// The index of the first instruction of the straight-line run holding `index`: runs start at
/// the function's start and at every label.
#[must_use]
pub fn run_start(function: &MappedFunction<'_>, index: usize) -> usize {
    function
        .labels
        .iter()
        .map(|label| label.index)
        .filter(|start| *start <= index)
        .max()
        .unwrap_or(0)
}

/// The number `mov REG, VALUE` puts in `register` as the last change to it in the straight-line
/// run before `index`, with REG the register whole or its low 32 bits: where it is, and the
/// number. On AArch64 that's also a `mov` and the `movk`s right after it, read at the last.
#[must_use]
pub fn constant_before(
    function: &MappedFunction<'_>,
    index: usize,
    register: Reg,
    abi: &Abi,
) -> Option<(usize, i64)> {
    let start = run_start(function, index);
    (start..index)
        .rev()
        .find_map(|position| {
            let step = &function.steps[position];
            let insn = &step.insn;
            if !insn.changes(register, abi) {
                return None;
            }
            let whole = |dst: Reg| dst.full() == register.full() && dst.bits() >= 32;
            Some(match (insn.op, &step.source.pieces) {
                (Op::MovImm { dst, value }, _) if whole(dst) => Some((position, value)),
                (_, Some(pieces)) if insn.dst.is_some_and(whole) => Some((position, pieces.value)),
                _ => None,
            })
        })
        .flatten()
}

/// Builds the control flow of a function and checks it (rule 1) and its stack (rules 3 and 4).
/// `exit_numbers` holds the system call numbers that end the program.
pub fn analyze(
    function: &MappedFunction<'_>,
    program: &Program<'_>,
    exit_numbers: &[i64],
    jumps: &[super::tables::TableJump],
    violations: &mut Vec<Violation>,
) -> Flow {
    let name = function.name();
    let exempt = name == ASSERT_FAIL;
    let count = function.steps.len();
    let abi = program.abi();
    let mut report = |rule: Rule, line: usize, problem: String, fix: &str| {
        if !exempt {
            violations.push(Violation {
                rule,
                line: Some(line),
                problem,
                fix: fix.to_owned(),
            });
        }
    };
    let mut exits = Vec::with_capacity(count);
    for (index, step) in function.steps.iter().enumerate() {
        let line = step.source.line;
        let exit = match step.insn.control {
            Control::Next => Exit::Next,
            Control::Jump(target) | Control::Branch { target, .. } => {
                match function.index_of(target) {
                    Some(target) if matches!(step.insn.control, Control::Branch { .. }) => {
                        Exit::Branch(target)
                    }
                    Some(target) => Exit::Jump(target),
                    None => {
                        report(
                            Rule::ControlFlow,
                            line,
                            format!("`{}` jumps outside {name}", step.source.text),
                            "jump only to labels inside the same function",
                        );
                        Exit::Stop
                    }
                }
            }
            Control::Return => Exit::Return,
            Control::Syscall => {
                let exit = constant_before(function, index, abi.syscall_number, abi)
                    .is_some_and(|(_, number)| exit_numbers.contains(&number));
                Exit::Syscall { exit }
            }
            Control::Call(target) => match program.function_at(target) {
                Some(callee) => Exit::Call {
                    callee: callee.name().to_owned(),
                },
                // lengua's: an operation, or one of the library's functions.
                None if program.lengua_at(target).is_some() => {
                    let name = program.lengua_at(target).map_or("", MappedFunction::name);
                    let in_lengua = program.lengua_at_name(function.name()).is_some();
                    if let Some(op) = crate::edge::Op::from_symbol(name) {
                        Exit::Edge { op }
                    } else if in_lengua || crate::edge::LIBRARY.contains(&name) {
                        Exit::Call {
                            callee: name.to_owned(),
                        }
                    } else {
                        report(
                            Rule::ControlFlow,
                            line,
                            format!(
                                "`{}` enters lengua's own code, {name}, which only lengua calls",
                                step.source.text
                            ),
                            "call only the operations (os_*) and assert_fail, fill and parallel_for",
                        );
                        Exit::Stop
                    }
                }
                None => {
                    report(
                        Rule::ControlFlow,
                        line,
                        format!(
                            "`{}` doesn't call the start of a function",
                            step.source.text
                        ),
                        "call functions by name",
                    );
                    Exit::Stop
                }
            },
            // A jump through a checked table goes to one of its cases; any other jump through a
            // register is already reported.
            Control::IndirectJump => jumps
                .iter()
                .find(|jump| jump.jump == index)
                .map_or(Exit::Stop, |jump| Exit::Table(jump.cases.clone())),
            Control::IndirectCall | Control::Other => Exit::Stop,
        };
        exits.push(exit);
    }

    let mut successors = vec![Vec::new(); count];
    for (index, exit) in exits.iter().enumerate() {
        let falls_through = match exit {
            Exit::Next | Exit::Branch(_) => true,
            Exit::Call { callee } => callee != ASSERT_FAIL,
            Exit::Syscall { exit } => !exit,
            Exit::Edge { op } => *op != crate::edge::Op::Exit,
            Exit::Jump(_) | Exit::Table(_) | Exit::Return | Exit::Stop => false,
        };
        if let Exit::Jump(target) | Exit::Branch(target) = exit {
            successors[index].push(*target);
        }
        if let Exit::Table(cases) = exit {
            successors[index].extend(cases);
        }
        if falls_through {
            if index + 1 < count {
                successors[index].push(index + 1);
            } else {
                report(
                    Rule::ControlFlow,
                    function.steps[index].source.line,
                    format!("control runs off the end of {name} after this line"),
                    "end every path with ret, jmp, a call to os_exit or an assertion",
                );
            }
        }
    }
    let mut predecessors = vec![Vec::new(); count];
    for (index, targets) in successors.iter().enumerate() {
        for target in targets {
            predecessors[*target].push(index);
        }
    }

    let entry_depth = if name == "_start" {
        0
    } else {
        abi.return_address
    };
    let mut reachable = vec![false; count];
    let mut depth: Vec<Option<i64>> = vec![None; count];
    let mut max_depth = entry_depth;
    let mut queue = VecDeque::new();
    if count > 0 {
        reachable[0] = true;
        depth[0] = Some(entry_depth);
        queue.push_back(0);
    }
    while let Some(index) = queue.pop_front() {
        let before = depth[index].unwrap_or(entry_depth);
        let step = &function.steps[index];
        let line = step.source.line;
        let after = match &step.insn.stack {
            Stack::Same => before,
            Stack::Moves(bytes) => before + bytes,
            Stack::Refused(problem) => {
                report(
                    Rule::Resources,
                    line,
                    problem.clone(),
                    "change rsp only with push, pop, call, ret, and add or sub rsp with a constant",
                );
                before
            }
        };
        if after < entry_depth {
            report(
                Rule::Resources,
                line,
                "this pops more than the function pushed".into(),
                "pop exactly what you push",
            );
        }
        if exits[index] == Exit::Return && before != abi.return_address {
            report(
                Rule::Functions,
                line,
                format!(
                    "ret runs with {} bytes still on the stack",
                    before - abi.return_address
                ),
                "undo every push and sub rsp before ret",
            );
        }
        let aligned_call = match &exits[index] {
            Exit::Call { callee } => callee != ASSERT_FAIL,
            Exit::Edge { op } => *op != crate::edge::Op::Exit,
            _ => false,
        };
        if aligned_call && before % 16 != 0 {
            report(
                Rule::Functions,
                line,
                format!(
                    "the stack isn't 16-byte aligned at this call ({before} bytes deep, counting \
                     the return address)"
                ),
                "adjust rsp by 8 (sub rsp, 8 … add rsp, 8, or an extra push) so calls happen at \
                 a multiple of 16",
            );
        }
        max_depth = max_depth.max(before).max(after);
        for &next in &successors[index] {
            reachable[next] = true;
            match depth[next] {
                None => {
                    depth[next] = Some(after);
                    queue.push_back(next);
                }
                Some(known) if known != after => report(
                    Rule::Resources,
                    function.steps[next].source.line,
                    format!(
                        "the stack is {known} bytes deep on one path here and {after} on another"
                    ),
                    "push and pop the same amount on every path",
                ),
                Some(_) => {}
            }
        }
    }
    let mut index = 0;
    while index < count {
        if reachable[index] {
            index += 1;
            continue;
        }
        report(
            Rule::ControlFlow,
            function.steps[index].source.line,
            "this line can never run".into(),
            "remove unreachable code, or jump to it",
        );
        while index < count && !reachable[index] {
            index += 1;
        }
    }
    Flow {
        exits,
        successors,
        predecessors,
        reachable,
        depth,
        max_depth,
    }
}

/// The deepest the stack gets, in bytes, over every call path from `name`, counting the return
/// address that called it.
#[must_use]
pub fn deepest(name: &str, program: &Program<'_>, flows: &BTreeMap<String, Flow>) -> i64 {
    fn walk(
        name: &str,
        program: &Program<'_>,
        flows: &BTreeMap<String, Flow>,
        memo: &mut BTreeMap<String, i64>,
        visiting: &mut BTreeSet<String>,
    ) -> i64 {
        if let Some(known) = memo.get(name) {
            return *known;
        }
        let (Some(_), Some(flow)) = (program.any_function(name), flows.get(name)) else {
            return 0;
        };
        if !visiting.insert(name.to_owned()) {
            return 0;
        }
        let mut most = flow.max_depth;
        for (index, exit) in flow.exits.iter().enumerate() {
            let callee = match exit {
                Exit::Call { callee } => callee.as_str(),
                Exit::Edge { op } => op.symbol(),
                _ => continue,
            };
            let at_call = flow.depth[index].unwrap_or(0);
            most = most.max(at_call + walk(callee, program, flows, memo, visiting));
        }
        visiting.remove(name);
        memo.insert(name.to_owned(), most);
        most
    }
    walk(
        name,
        program,
        flows,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}
