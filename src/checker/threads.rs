//! Fork-join threads: the canonical `parallel_for` code, the threads the spec allows, and what a
//! worker may touch. A worker and every function it calls write only their own stack frames and
//! the worker's slot, through `rbx`, so no two threads ever write the same byte: the same
//! guarantee Rust's `&mut` slices give, checked on the machine code.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{FlowControl, Instruction, InstructionInfoFactory, OpAccess, OpKind, Register};

use super::cfg::{ASSERT_FAIL, Exit, Flow, deepest, run_start, writes};
use super::decode::{MappedFunction, Program, mnemonic};
use super::source::Source;
use super::{Rule, Violation};
use crate::spec::MAX_THREADS;

/// The canonical functions, copied word for word from the parallel template.
pub const FUNCTIONS: [&str; 3] = ["parallel_for", "start_worker", "join_worker"];

/// The canonical data symbols, with the alignment each needs and its size per thread, given the
/// constant that sets it.
const DATA: [(&str, u64, Size); 4] = [
    ("stacks", 64, Size::Constant("STACK_SIZE")),
    ("tids", 4, Size::Bytes(4)),
    ("results", 8, Size::Bytes(8)),
    ("slots", 64, Size::Constant("SLOT_SIZE")),
];

/// How big each thread's part of a canonical data symbol is.
#[derive(Clone, Copy)]
enum Size {
    Bytes(u64),
    Constant(&'static str),
}

/// Constants the canonical code depends on, which must hold the template's values.
const FIXED_CONSTANTS: [&str; 7] = [
    "SYS_CLONE",
    "SYS_FUTEX",
    "SYS_EXIT",
    "CLONE_FLAGS",
    "FUTEX_WAIT",
    "THREAD_EXIT",
    "MAX_WAITS",
];

/// The function every thread runs.
pub const WORKER: &str = "worker";

/// The register that holds a worker's slot.
const SLOT_REGISTER: Register = Register::RBX;

/// Where the canonical code is, for messages.
const TEMPLATE_PATH: &str = "assets/parallel-template.s";

/// Whether main.s starts threads.
#[must_use]
pub fn used(source: &Source) -> bool {
    FUNCTIONS.iter().any(|name| source.function(name).is_some())
}

struct Checker<'a, 'p> {
    program: &'a Program<'p>,
    flows: &'a BTreeMap<String, Flow>,
    factory: InstructionInfoFactory,
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

/// Checks a program that starts threads: the spec allows them, the canonical code is copied
/// exactly, its sizes and alignments hold, and workers write only their own stack and slot.
/// `allowed` is how many threads the spec allows at once.
pub fn check(
    program: &Program<'_>,
    flows: &BTreeMap<String, Flow>,
    template: &Source,
    allowed: Option<u32>,
    violations: &mut Vec<Violation>,
) {
    let source = program.source;
    if !used(source) {
        return;
    }
    let mut checker = Checker {
        program,
        flows,
        factory: InstructionInfoFactory::new(),
        violations,
    };
    let line = FUNCTIONS
        .iter()
        .filter_map(|name| source.function(name))
        .map(|function| function.line)
        .min()
        .unwrap_or(1);
    let Some(allowed) = allowed else {
        checker.violation(
            Rule::Resources,
            line,
            "main.s starts threads with parallel_for, but spec.md doesn't allow threads",
            "add `- Threads: up to N` under `# Constraints` in spec.md and call check again, \
             or do the work on one thread",
        );
        return;
    };
    if !checker.canonical(template) {
        return;
    }
    checker.sizes(allowed);
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
    /// The canonical functions, data and constants match the template. Returns whether they do,
    /// since nothing else can be trusted otherwise.
    fn canonical(&mut self, template: &Source) -> bool {
        let source = self.program.source;
        let before = self.violations.len();
        for name in FUNCTIONS {
            let expected = template.function(name).map(|function| &function.body);
            match source.function(name) {
                Some(function) if Some(&function.body) == expected => {}
                Some(function) => {
                    let line = function.line;
                    self.violation(
                        Rule::Resources,
                        line,
                        format!("{name} differs from the template"),
                        format!("copy {name} word for word from {TEMPLATE_PATH}"),
                    );
                }
                None => {
                    let line = source.functions.last().map_or(1, |function| function.line);
                    self.violation(
                        Rule::Resources,
                        line,
                        format!("main.s starts threads but has no {name}"),
                        format!(
                            "copy parallel_for, start_worker and join_worker word for word from \
                             {TEMPLATE_PATH}"
                        ),
                    );
                }
            }
        }
        if source.function(WORKER).is_none() {
            let line = source.functions.last().map_or(1, |function| function.line);
            self.violation(
                Rule::Resources,
                line,
                "main.s calls parallel_for but has no worker function",
                "write `worker`: it gets its index in rdi and its slot in rsi, and returns a \
                 result in rax",
            );
        }
        for (name, _, _) in DATA {
            let found = source.data.iter().find(|data| data.name == name);
            let expected = template.data.iter().find(|data| data.name == name);
            let same = match (found, expected) {
                (Some(found), Some(expected)) => {
                    found.body == expected.body && found.section == expected.section
                }
                _ => false,
            };
            if !same {
                let line = found.map_or(1, |data| data.line);
                self.violation(
                    Rule::Resources,
                    line,
                    format!("{name} differs from the template, or is missing"),
                    format!("copy {name} word for word into .bss from {TEMPLATE_PATH}"),
                );
            }
        }
        for name in FIXED_CONSTANTS {
            let expected = template.constants.get(name).map(|constant| constant.value);
            let found = source.constants.get(name);
            if found.map(|constant| constant.value) != expected {
                let line = found.map_or(1, |constant| constant.line);
                self.violation(
                    Rule::Resources,
                    line,
                    format!(
                        "{name} must be {}, as in the template",
                        expected.unwrap_or_default()
                    ),
                    format!("copy `.equ {name}` from {TEMPLATE_PATH}"),
                );
            }
        }
        self.violations.len() == before
    }

    /// THREADS is at most what the spec allows; each stack and slot is a multiple of 64 bytes;
    /// and the data symbols are as big and as aligned as the threads need.
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
                format!("THREADS is {threads}, but the spec allows from 1 to {most} threads"),
                format!("set `.equ THREADS, N` with N from 1 to {most}"),
            );
            return;
        }
        for name in ["STACK_SIZE", "SLOT_SIZE"] {
            let value = self.constant(name).unwrap_or(0);
            if value <= 0 || value % 64 != 0 {
                self.violation(
                    Rule::Resources,
                    line(name),
                    format!("{name} is {value}, which isn't a positive multiple of 64"),
                    format!(
                        "make {name} a multiple of 64, so each thread's part starts on its own \
                         cache line"
                    ),
                );
            }
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

    /// Only parallel_for starts and joins threads and only the canonical code calls worker,
    /// whose functions run on worker threads alone.
    fn callers(&mut self, region: &BTreeSet<String>) {
        for (caller, callee, line) in self.calls() {
            let allowed = match callee.as_str() {
                "start_worker" | "join_worker" => caller == "parallel_for",
                WORKER => caller == "parallel_for" || caller == "start_worker",
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
        let Some(stack_size) = self.constant("STACK_SIZE") else {
            return;
        };
        let needed = deepest(WORKER, self.program, self.flows);
        if needed > stack_size {
            let line = self
                .program
                .source
                .constants
                .get("STACK_SIZE")
                .map_or(1, |constant| constant.line);
            self.violation(
                Rule::Resources,
                line,
                format!(
                    "a worker's stack can reach {needed} bytes, but STACK_SIZE is {stack_size}"
                ),
                format!(
                    "raise STACK_SIZE to a multiple of 64 of at least {needed}, or use less stack \
                     in the worker"
                ),
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
        let is_worker = function.name() == WORKER;
        let slot_set = is_worker.then(|| slot_copy(function, &flow)).flatten();
        let slot_size = self.constant("SLOT_SIZE").unwrap_or(0);
        for (index, step) in function.steps.iter().enumerate() {
            let instruction = &step.machine;
            let line = step.source.line;
            if matches!(flow.exits[index], Exit::Syscall { .. }) {
                self.violation(
                    Rule::Resources,
                    line,
                    format!("{} makes a system call on a worker thread", function.name()),
                    "do reading and writing on the main thread, before and after parallel_for",
                );
            }
            let sets_slot = Some(index) == slot_set;
            let restores = is_worker
                && mnemonic(instruction) == "pop"
                && flow.exits.get(index + 1) == Some(&Exit::Return);
            if !sets_slot && !restores && writes(instruction, SLOT_REGISTER, &mut self.factory) {
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
            let Some(operand) = (0..instruction.op_count())
                .find(|operand| instruction.op_kind(*operand) == OpKind::Memory)
            else {
                continue;
            };
            if mnemonic(instruction) == "lea" {
                continue;
            }
            let written = matches!(
                self.factory.info(instruction).op_access(operand),
                OpAccess::Write
                    | OpAccess::ReadWrite
                    | OpAccess::CondWrite
                    | OpAccess::ReadCondWrite
            );
            match instruction.memory_base() {
                Register::RSP => {
                    let frame = flow.depth[index].map_or(0, |depth| depth - 8);
                    self.inside(function, index, frame, "its own stack frame");
                }
                SLOT_REGISTER => {
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
                Register::RIP if written => {
                    let name = self
                        .program
                        .data_at(instruction.memory_displacement64())
                        .map_or_else(|| "data".to_owned(), |(name, _)| name.to_owned());
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
                    let register = format!("{base:?}").to_ascii_lowercase();
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
        let instruction = &step.machine;
        let width = i64::try_from(instruction.memory_size().size())
            .unwrap_or(1)
            .max(1);
        let displacement = i64::from_ne_bytes(instruction.memory_displacement64().to_ne_bytes());
        let largest_index = if instruction.memory_index() == Register::None {
            Some(0)
        } else {
            index_limit(function, index, instruction, &mut self.factory)
        };
        // Without a bounds check, rule 9 has already reported the access.
        let Some(largest_index) = largest_index else {
            return;
        };
        let last =
            displacement + largest_index * i64::from(instruction.memory_index_scale()) + width - 1;
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

/// Where worker copies its slot into rbx: `mov rbx, rsi` before any label or call.
fn slot_copy(function: &MappedFunction<'_>, flow: &Flow) -> Option<usize> {
    function
        .steps
        .iter()
        .enumerate()
        .take_while(|(index, _)| {
            run_start(function, *index) == 0 && !matches!(flow.exits[*index], Exit::Call { .. })
        })
        .find(|(_, step)| {
            let instruction = &step.machine;
            mnemonic(instruction) == "mov"
                && instruction.op_count() == 2
                && instruction.op_kind(0) == OpKind::Register
                && instruction.op_register(0) == Register::RBX
                && instruction.op_kind(1) == OpKind::Register
                && instruction.op_register(1) == Register::RSI
        })
        .map(|(index, _)| index)
}

/// The largest index the bounds check before `index` lets through, from its `cmp` with a
/// constant and its `jae` or `ja`.
fn index_limit(
    function: &MappedFunction<'_>,
    index: usize,
    access: &Instruction,
    factory: &mut InstructionInfoFactory,
) -> Option<i64> {
    let register = access.memory_index().full_register();
    for position in (run_start(function, index)..index).rev() {
        let candidate = &function.steps[position].machine;
        let compares = mnemonic(candidate) == "cmp"
            && candidate.op_kind(0) == OpKind::Register
            && candidate.op_register(0).full_register() == register
            && !matches!(candidate.op_kind(1), OpKind::Register | OpKind::Memory);
        if compares {
            let next = &function.steps.get(position + 1)?.machine;
            if next.flow_control() != FlowControl::ConditionalBranch {
                return None;
            }
            let limit = i64::from_ne_bytes(candidate.immediate(1).to_ne_bytes());
            return match mnemonic(next).as_str() {
                "jae" => Some(limit - 1),
                "ja" => Some(limit),
                _ => None,
            };
        }
        if writes(candidate, register, factory) {
            return None;
        }
    }
    None
}
