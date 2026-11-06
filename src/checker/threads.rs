//! Fork-join threads: the threads the idea allows, and what a worker may touch. lengua's own
//! `parallel_for` starts them and calls the program's `worker`, which and every function it calls
//! write only their own stack frames and the worker's slot, through `rbx`, each index bounded by a
//! constant, and read the other threads' parts of the per-thread data never, so no thread writes
//! a byte another thread reads or writes (M7): the guarantee Rust's `&mut` slices give, checked on
//! the machine code.

use std::collections::{BTreeMap, BTreeSet};

use super::cfg::{ASSERT_FAIL, Exit, Flow, constant_before, deepest, run_start};
use super::decode::{MappedFunction, Program};
use super::{Rule, Violation};
use crate::idea::MAX_THREADS;
use crate::isa::{Abi, Cond, Control, MemRef, Op, Operand, Reg, Value};

/// The data symbols the program gives the threads, with the alignment each needs and its size per
/// thread, given the constant that sets it.
const DATA: [(&str, u64, Size); 2] = [
    ("results", 8, Size::Bytes(8)),
    ("slots", 64, Size::Constant("SLOT_SIZE")),
];

/// How big each thread's part of a data symbol is.
#[derive(Clone, Copy)]
enum Size {
    Bytes(u64),
    Constant(&'static str),
}

/// The function every thread runs.
pub const WORKER: &str = "worker";

/// What lengua's `parallel_for` is called by.
const LIBRARY_PARALLEL_FOR: &str = "parallel_for";

struct Checker<'a, 'p> {
    program: &'a Program<'p>,
    flows: &'a BTreeMap<String, Flow>,
    violations: &'a mut Vec<Violation>,
}

impl Checker<'_, '_> {
    fn violation(
        &mut self,
        rule: Rule,
        line: usize,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.violations.push(Violation {
            rule,
            line: Some(line),
            problem: problem.into(),
            fix: fix.into(),
        });
    }

    fn constant(&self, name: &str) -> Option<i64> {
        self.program
            .source
            .constants
            .get(name)
            .map(|constant| constant.value)
    }
}

/// Checks a program that starts threads: the idea allows them, its sizes and alignments hold, and
/// workers write only their own stack and slot. `allowed` is how many threads the idea allows at
/// once.
pub fn check(
    program: &Program<'_>,
    flows: &BTreeMap<String, Flow>,
    allowed: Option<u32>,
    violations: &mut Vec<Violation>,
) {
    let mut checker = Checker {
        program,
        flows,
        violations,
    };
    // Where main.s starts threads: its calls of lengua's parallel_for.
    let Some(line) = checker.starts().first().copied() else {
        return;
    };
    let Some(allowed) = allowed else {
        checker.violation(
            Rule::Resources,
            line,
            "main.s starts threads with parallel_for, but idea.md doesn't allow threads",
            "add `- Threads: up to N` under `# Constraints` in idea.md and call check again, \
             or do the work on one thread",
        );
        return;
    };
    checker.worker_defined(line);
    checker.sizes(allowed);
    checker.counts();
    let region = checker.region();
    checker.callers(&region);
    checker.stack();
    for name in &region {
        if let Some(function) = program.function(name) {
            checker.worker_memory(function);
        }
    }
}

impl Checker<'_, '_> {
    /// The lines of main.s's calls of lengua's parallel_for.
    fn starts(&self) -> Vec<usize> {
        self.calls()
            .into_iter()
            .filter(|(_, callee, _)| callee == LIBRARY_PARALLEL_FOR)
            .map(|(_, _, line)| line)
            .collect()
    }

    /// lengua's parallel_for calls main.s's `worker`, so a program that starts threads needs one.
    fn worker_defined(&mut self, line: usize) {
        if self.program.source.function(WORKER).is_none() {
            self.violation(
                Rule::Resources,
                line,
                "main.s calls parallel_for but has no worker function",
                "write `worker`: it gets its index in rdi and its slot in rsi, and returns a \
                 result in rax",
            );
        }
    }

    /// Each call of parallel_for hands it what the threads' data is sized for:
    /// a count of at most THREADS, and the program's own `slots` and `results`, with SLOT_SIZE
    /// for each slot, so lengua's code never reaches past them.
    fn counts(&mut self) {
        let abi = self.program.abi();
        let threads = self.constant("THREADS").unwrap_or(0);
        let slot_size = self.constant("SLOT_SIZE").unwrap_or(0);
        let mut found = Vec::new();
        for function in &self.program.functions {
            let Some(flow) = self.flows.get(function.name()) else {
                continue;
            };
            for (index, exit) in flow.exits.iter().enumerate() {
                if !matches!(exit, Exit::Call { callee } if callee == LIBRARY_PARALLEL_FOR) {
                    continue;
                }
                let line = function.steps[index].source.line;
                if !count_at_most(function, index, threads, abi) {
                    found.push((
                        line,
                        format!(
                            "the count given to parallel_for isn't shown to be at most THREADS, \
                             {threads}"
                        ),
                        "compare the count with THREADS and jump to an assertion when it is \
                         above, in the same straight-line code as the call, before the count \
                         is moved to its argument register"
                            .to_owned(),
                    ));
                }
                if constant_before(function, index, abi.arguments[2], abi).map(|(_, value)| value)
                    != Some(slot_size)
                {
                    found.push((
                        line,
                        "parallel_for's slot size isn't SLOT_SIZE".to_owned(),
                        "set the third argument to SLOT_SIZE, as `mov edx, SLOT_SIZE`".to_owned(),
                    ));
                }
                for (position, name) in [(1, "slots"), (3, "results")] {
                    let set = symbol_before(self.program, function, index, abi.arguments[position]);
                    if set.as_deref() != Some(name) {
                        found.push((
                            line,
                            format!("parallel_for's argument {} isn't {name}", position + 1),
                            format!("pass the address of {name}, as the template does"),
                        ));
                    }
                }
            }
        }
        for (line, problem, fix) in found {
            self.violation(Rule::Resources, line, problem, fix);
        }
    }

    /// THREADS is at most what the idea allows; each slot is a multiple of 64 bytes; and the data
    /// symbols are as big and as aligned as the threads need.
    fn sizes(&mut self, allowed: u32) {
        let source = self.program.source;
        let line = |name: &str| {
            source
                .constants
                .get(name)
                .map_or(1, |constant| constant.line)
        };
        let threads = self.constant("THREADS").unwrap_or(0);
        let most = i64::from(allowed.min(MAX_THREADS));
        if !(1..=most).contains(&threads) {
            self.violation(
                Rule::Resources,
                line("THREADS"),
                format!("THREADS is {threads}, but the idea allows from 1 to {most} threads"),
                format!("set `.equ THREADS, N` with N from 1 to {most}"),
            );
            return;
        }
        // The stacks are lengua's, so only the slots are the program's to size.
        let slot_size = self.constant("SLOT_SIZE").unwrap_or(0);
        if slot_size <= 0 || slot_size % 64 != 0 {
            self.violation(
                Rule::Resources,
                line("SLOT_SIZE"),
                format!("SLOT_SIZE is {slot_size}, which isn't a positive multiple of 64"),
                "make SLOT_SIZE a multiple of 64, so each thread's part starts on its own cache \
                 line",
            );
        }
        for (name, alignment, size) in DATA {
            let Some(data) = self.program.data.get(name) else {
                continue;
            };
            let each = match size {
                Size::Bytes(bytes) => i64::try_from(bytes).unwrap_or(0),
                Size::Constant(constant) => self.constant(constant).unwrap_or(0),
            };
            let expected = threads.checked_mul(each).unwrap_or(-1);
            let data_line = source
                .data
                .iter()
                .find(|data| data.name == name)
                .map_or(1, |data| data.line);
            if i64::try_from(data.size).ok() != Some(expected) {
                self.violation(
                    Rule::Resources,
                    data_line,
                    format!(
                        "{name} has {} bytes, but THREADS threads need {expected}",
                        data.size
                    ),
                    format!("size {name} as the template does, from THREADS"),
                );
            }
            if data.address % alignment != 0 {
                self.violation(
                    Rule::Resources,
                    data_line,
                    format!("{name} starts at an address that isn't a multiple of {alignment}"),
                    format!("put `.balign {alignment}` between `.type {name}` and `{name}:`"),
                );
            }
        }
    }

    /// The worker and every function it calls, assert_fail aside.
    fn region(&self) -> BTreeSet<String> {
        let mut region = BTreeSet::new();
        let mut pending = vec![WORKER.to_owned()];
        while let Some(name) = pending.pop() {
            if name == ASSERT_FAIL || !region.insert(name.clone()) {
                continue;
            }
            if let Some(flow) = self.flows.get(&name) {
                for exit in &flow.exits {
                    if let Exit::Call { callee } = exit {
                        pending.push(callee.clone());
                    }
                }
            }
        }
        region
    }

    /// Who calls each function, with the line of the call.
    fn calls(&self) -> Vec<(String, String, usize)> {
        let mut calls = Vec::new();
        for function in &self.program.functions {
            let Some(flow) = self.flows.get(function.name()) else {
                continue;
            };
            for (index, exit) in flow.exits.iter().enumerate() {
                if let Exit::Call { callee } = exit {
                    calls.push((
                        function.name().to_owned(),
                        callee.clone(),
                        function.steps[index].source.line,
                    ));
                }
            }
        }
        calls
    }

    /// Only lengua's parallel_for starts and joins threads and calls worker, whose functions run
    /// on worker threads alone, so main.s never calls worker.
    fn callers(&mut self, region: &BTreeSet<String>) {
        for (caller, callee, line) in self.calls() {
            let allowed = match callee.as_str() {
                WORKER => false,
                _ if region.contains(&callee) => region.contains(&caller),
                _ => true,
            };
            if !allowed {
                self.violation(
                    Rule::Resources,
                    line,
                    format!("{caller} calls {callee}, which only the thread code may call"),
                    if region.contains(&callee) {
                        format!(
                            "{callee} runs on worker threads, so only worker and the functions \
                             it calls may call it; give the main thread its own function"
                        )
                    } else {
                        "start threads with parallel_for only".to_owned()
                    },
                );
            }
        }
    }

    /// Each thread's stack holds the worker's deepest call path.
    fn stack(&mut self) {
        // lengua gives each thread its stack, and a worker starts at its top.
        let needed = deepest(WORKER, self.program, self.flows);
        if needed > crate::edge::THREAD_STACK - crate::edge::THREAD_STACK_HEAD {
            let line = self
                .program
                .source
                .function(WORKER)
                .map_or(1, |function| function.line);
            self.violation(
                Rule::Resources,
                line,
                format!(
                    "a worker's stack can reach {needed} bytes, but each thread has {} bytes to \
                     use",
                    crate::edge::THREAD_STACK - crate::edge::THREAD_STACK_HEAD
                ),
                "use less stack in the worker",
            );
        }
    }

    /// A worker's function writes only its own stack frame and the worker's slot, reads the slot
    /// only inside it, makes no system calls, and leaves rbx, which holds the slot, alone.
    fn worker_memory(&mut self, function: &MappedFunction<'_>) {
        let Some(flow) = self.flows.get(function.name()) else {
            return;
        };
        let flow = flow.clone();
        let abi = self.program.abi();
        let is_worker = function.name() == WORKER;
        let slot_set = is_worker.then(|| slot_copy(function, &flow, abi)).flatten();
        let slot_size = self.constant("SLOT_SIZE").unwrap_or(0);
        for (index, step) in function.steps.iter().enumerate() {
            let insn = &step.insn;
            let line = step.source.line;
            // A call that reaches the OS: a system call, or, in the edge form, an operation or a
            // library function that makes one.
            let reaches_the_os = match &flow.exits[index] {
                Exit::Syscall { .. } | Exit::Edge { .. } => true,
                Exit::Call { callee } => {
                    callee != ASSERT_FAIL && crate::edge::LIBRARY.contains(&callee.as_str())
                }
                _ => false,
            };
            if reaches_the_os {
                self.violation(
                    Rule::Resources,
                    line,
                    format!("{} makes a system call on a worker thread", function.name()),
                    "do reading and writing on the main thread, before and after parallel_for",
                );
            }
            let sets_slot = Some(index) == slot_set;
            let restores = is_worker
                && matches!(insn.op, Op::Pop(_))
                && flow.exits.get(index + 1) == Some(&Exit::Return);
            if !sets_slot && !restores && insn.changes(abi.worker_slot, abi) {
                self.violation(
                    Rule::Scope,
                    line,
                    format!(
                        "`{}` changes rbx, which holds the worker's slot",
                        step.source.text
                    ),
                    "keep rbx as worker sets it, `mov rbx, rsi` before any call; use other \
                     registers for everything else",
                );
            }
            let Some(memory) = insn.memory else {
                continue;
            };
            // The function's own saves and restores make its frame and undo it, as x86-64's
            // push and pop do, rather than reaching into it.
            if matches!(insn.op, Op::Push(_) | Op::Pop(_)) {
                continue;
            }
            // M7: other threads write the per-thread data, so a worker reaches only its own part,
            // through rbx: never through the symbol's address.
            if memory.pc_relative
                && let Some((name, ..)) = self.program.named(step)
                && DATA.iter().any(|(known, ..)| *known == name)
            {
                let name = name.to_owned();
                self.violation(
                    Rule::Races,
                    line,
                    format!(
                        "`{}` reaches {name} on a worker thread, where other threads write their \
                         parts of it",
                        step.source.text
                    ),
                    "a worker reaches only its own slot, through rbx, as worker sets it",
                );
                continue;
            }
            if matches!(insn.op, Op::AddressOf { .. }) {
                continue;
            }
            let written = memory.writes;
            match memory.base {
                Some(base) if base == abi.stack_pointer => {
                    let frame = flow.depth[index].map_or(0, |depth| depth - abi.return_address);
                    self.inside(function, index, frame, "its own stack frame");
                }
                Some(base) if base == abi.worker_slot => {
                    if is_worker && slot_set.is_none_or(|set| index < set) {
                        self.violation(
                            Rule::Scope,
                            line,
                            format!("`{}` uses rbx before it holds the slot", step.source.text),
                            "start worker with `push rbx`, then `mov rbx, rsi`",
                        );
                        continue;
                    }
                    self.inside(function, index, slot_size, "the worker's slot");
                }
                _ if memory.pc_relative && written => {
                    let name = self
                        .program
                        .named(step)
                        .map_or_else(|| "data".to_owned(), |(name, ..)| name.to_owned());
                    self.violation(
                        Rule::Scope,
                        line,
                        format!(
                            "`{}` writes {name} on a worker thread, where other threads could \
                             write it too",
                            step.source.text
                        ),
                        "write results into the slot, [rbx + …], or return them in rax",
                    );
                }
                base if written => {
                    let register = base.map_or_else(|| "none".to_owned(), |base| base.to_string());
                    self.violation(
                        Rule::Scope,
                        line,
                        format!(
                            "`{}` writes through {register} on a worker thread",
                            step.source.text
                        ),
                        "a worker writes only its own stack, [rsp + …], and its slot, [rbx + …]",
                    );
                }
                _ => {}
            }
        }
    }

    /// The memory access at `index` stays inside the first `size` bytes past its base register.
    fn inside(&mut self, function: &MappedFunction<'_>, index: usize, size: i64, what: &str) {
        let step = &function.steps[index];
        let Some(memory) = step.insn.memory else {
            return;
        };
        let width = i64::try_from(memory.width).unwrap_or(1).max(1);
        let displacement = memory.displacement;
        let largest_index = if memory.index.is_none() {
            Some(0)
        } else {
            index_limit(function, index, memory, self.program.abi())
        };
        // M7: an index bounded only by a register could reach another thread's part.
        let Some(largest_index) = largest_index else {
            self.violation(
                Rule::Races,
                step.source.line,
                format!(
                    "`{}` indexes {what} with no constant limit, so it could reach another \
                     thread's part",
                    step.source.text
                ),
                format!(
                    "since the last label, compare the index with a constant that keeps the \
                     access inside {what} (cmp index, LIMIT; jae), right before the access"
                ),
            );
            return;
        };
        let last = displacement + largest_index * i64::from(memory.scale) + width - 1;
        if displacement < 0 || last >= size {
            self.violation(
                Rule::Scope,
                step.source.line,
                format!(
                    "`{}` can reach byte {last} past its base, outside {what}, which has {size} \
                     bytes",
                    step.source.text
                ),
                format!("keep every access inside {what}"),
            );
        }
    }
}

/// Where worker copies its slot into the worker's register (x86-64's rbx, AArch64's x19) from its
/// second argument: `mov rbx, rsi` before any label or call.
fn slot_copy(function: &MappedFunction<'_>, flow: &Flow, abi: &Abi) -> Option<usize> {
    function
        .steps
        .iter()
        .enumerate()
        .take_while(|(index, _)| {
            run_start(function, *index) == 0 && !matches!(flow.exits[*index], Exit::Call { .. })
        })
        .find(|(_, step)| {
            matches!(step.insn.op, Op::Move)
                && step.insn.dst == Some(abi.worker_slot)
                && matches!(
                    step.insn.value,
                    Value::Copy(Operand::Reg(lent)) if lent == abi.arguments[1]
                )
        })
        .map(|(index, _)| index)
}

/// The largest index the bounds check before `index` lets through, from its `cmp` with a
/// constant and its `jae` or `ja`.
fn index_limit(
    function: &MappedFunction<'_>,
    index: usize,
    access: MemRef,
    abi: &Abi,
) -> Option<i64> {
    register_limit(function, index, access.index?.full(), abi)
}

/// Whether the count in the first argument register at the call at `index` is at most `most`: a
/// number moved there, or a register that a comparison with a constant bounded before it was
/// copied.
fn count_at_most(function: &MappedFunction<'_>, index: usize, most: i64, abi: &Abi) -> bool {
    let count = abi.arguments[0];
    if let Some((_, value)) = constant_before(function, index, count, abi) {
        return value <= most;
    }
    for position in (run_start(function, index)..index).rev() {
        let insn = &function.steps[position].insn;
        if insn.changes(count, abi) {
            return matches!(insn.op, Op::Move)
                && matches!(
                    insn.value,
                    Value::Copy(Operand::Reg(source))
                        if register_limit(function, position, source.full(), abi)
                            .is_some_and(|limit| limit <= most)
                );
        }
    }
    false
}

/// The data symbol whose address the last instruction to set `register` made, since the last
/// label before `index`: an x86-64 `lea [rip + name]`, or AArch64's `add :lo12:name`.
fn symbol_before(
    program: &Program<'_>,
    function: &MappedFunction<'_>,
    index: usize,
    register: Reg,
) -> Option<String> {
    let abi = program.abi();
    (run_start(function, index)..index)
        .rev()
        .find_map(|position| {
            let step = &function.steps[position];
            step.insn
                .changes(register, abi)
                .then(|| program.named(step).map(|(name, ..)| name.to_owned()))
        })
        .flatten()
}

/// The largest value `register` holds at `index`, from the comparison with a constant, and its
/// `jae` or `ja`, in the same run of code before it.
fn register_limit(
    function: &MappedFunction<'_>,
    index: usize,
    register: Reg,
    abi: &Abi,
) -> Option<i64> {
    for position in (run_start(function, index)..index).rev() {
        let candidate = &function.steps[position].insn;
        // The limit a number, or a register holding a named constant, as AArch64 holds one too
        // wide for `cmp`.
        if let Op::Compare {
            left: Operand::Reg(left),
            right,
        } = candidate.op
            && left.full() == register
            && let Some(limit) = match right {
                Operand::Imm(limit) => Some(limit),
                Operand::Reg(holder) => {
                    constant_before(function, position, holder, abi).map(|(_, value)| value)
                }
                _ => None,
            }
        {
            let Control::Branch { cond, .. } = function.steps.get(position + 1)?.insn.control
            else {
                return None;
            };
            return match cond {
                Cond::AboveEqual => Some(limit - 1),
                Cond::Above => Some(limit),
                _ => None,
            };
        }
        if candidate.changes(register, abi) {
            return None;
        }
    }
    None
}
