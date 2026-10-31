//! The lengua Power of 10 rules that need control flow or the whole program: loops, assertions,
//! system calls, checked results, data scope, named constants, checked indexes, and the
//! canonical assertion handler.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{FlowControl, InstructionInfoFactory, OpAccess, OpKind, Register};

use super::cfg::{
    ASSERT_FAIL, CALLEE_SAVED, Exit, Flow, deepest, rax_value_before, reads, run_start, writes,
};
use super::decode::{MappedFunction, Program, mnemonic};
use super::source::{Operand, Source};
use super::{Rule, Violation};
use crate::platform::Profile;
use crate::spec::{ASSERTION_EXIT_CODE, Syscall};

/// The most instructions a function may have (assert_fail excluded).
pub const MAX_INSTRUCTIONS: usize = 60;

/// The fewest assertions per function, on average.
pub const MIN_ASSERTIONS_PER_FUNCTION: usize = 2;

/// What the rules learned about the program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The deepest the stack can get, in bytes, across every call path from `_start`.
    pub max_stack_bytes: u64,
    /// Assertions in the program.
    pub assertions: usize,
    /// Functions, assert_fail excluded.
    pub functions: usize,
    /// The CPU features beyond the x86-64 baseline its instructions need.
    pub features: Vec<&'static str>,
    /// The data symbol each `[rip + name]` operand names, by its instruction's address.
    pub named: BTreeMap<u64, String>,
    /// The cases' addresses of each jump through a checked table, by the jump's address.
    pub tables: BTreeMap<u64, Vec<u64>>,
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
}

/// The canonical reading function, and the constants its body names.
pub const FILL: &str = "fill";
const FILL_CONSTANTS: [&str; 3] = ["SYS_READ", "ERROR_IO", "MAX_FILL_READS"];

/// Checks every rule that needs control flow, given each function's flow.
pub fn check(
    program: &Program<'_>,
    flows: &BTreeMap<String, Flow>,
    syscalls: &[Syscall],
    profile: Profile,
    [template, fill_template]: [&Source; 2],
    violations: &mut Vec<Violation>,
) -> Facts {
    let mut checker = Checker {
        program,
        flows,
        factory: InstructionInfoFactory::new(),
        violations,
    };
    let mut ids: BTreeMap<i64, usize> = BTreeMap::new();
    let mut assertions = 0;
    let mut users: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for function in &program.functions {
        let Some(flow) = flows.get(function.name()) else {
            continue;
        };
        checker.data_access(function, &mut users);
        if function.name() == ASSERT_FAIL {
            continue;
        }
        checker.size_and_returns(function, flow);
        checker.callee_saved(function, flow);
        checker.red_zone(function);
        // The thread code's clone and futex calls aren't the spec's to list: the spec allows
        // threads instead, and the code is checked word for word against the template.
        if !super::threads::FUNCTIONS.contains(&function.name()) {
            checker.syscalls(function, flow, syscalls, profile);
        }
        checker.exit_statuses(function, flow);
        checker.results(function, flow);
        checker.indexes(function, flow);
        checker.chains(function, flow);
        checker.loops(function, flow, syscalls.contains(&Syscall::Accept4));
        assertions += checker.assertions(function, flow, &mut ids);
    }
    checker.recursion();
    checker.shared_data(&users);
    checker.lengths();
    checker.canonical(template);
    checker.canonical_fill(fill_template);
    let functions = program
        .functions
        .iter()
        .filter(|function| function.name() != ASSERT_FAIL)
        .count();
    if assertions < MIN_ASSERTIONS_PER_FUNCTION * functions {
        let line = program
            .functions
            .first()
            .map_or(1, |function| function.source.line);
        checker.violation(
            Rule::Assertions,
            line,
            format!(
                "main.s has {assertions} assertions in {functions} functions; the dialect needs \
                 at least {MIN_ASSERTIONS_PER_FUNCTION} per function on average"
            ),
            "add assertions that check what must be true, such as pointers that aren't null, \
             indexes in range, or results the code relies on",
        );
    }
    Facts {
        max_stack_bytes: checker.max_stack(),
        assertions,
        functions,
        features: Vec::new(),
        named: BTreeMap::new(),
        tables: BTreeMap::new(),
    }
}

/// Where a function over the limit can be split: the smallest loop that, called instead, brings
/// it within the limit, or failing that the smallest run of code from one label to the next.
fn split_point(function: &MappedFunction<'_>, flow: &Flow) -> Option<String> {
    let count = function.steps.len();
    // Taking `size` instructions out for one call leaves `count - size + 1`.
    let needed = count.saturating_sub(MAX_INSTRUCTIONS) + 1;
    let mut candidates = Vec::new();
    for (place, label) in function.labels.iter().enumerate() {
        let start = label.index;
        if start >= count {
            continue;
        }
        let back = (start..count)
            .rev()
            .find(|&from| flow.successors[from].contains(&start));
        if let (Some(end), Some(_)) = (back, label.label.bound) {
            candidates.push((true, start, end, label));
        }
        let next = function
            .labels
            .get(place + 1)
            .map_or(count, |next| next.index);
        if next > start {
            candidates.push((false, start, next - 1, label));
        }
    }
    let (is_loop, start, end, label) = candidates
        .into_iter()
        .filter(|(_, start, end, _)| (needed..=MAX_INSTRUCTIONS).contains(&(end - start + 1)))
        .min_by_key(|(is_loop, start, end, _)| (!is_loop, end - start))?;
    let what = if is_loop { "the loop" } else { "the code from" };
    Some(format!(
        "lines {}–{}, {what} `{}` ({} instructions), can be a function of its own",
        label.label.line,
        function.steps[end].source.line,
        label.label.name,
        end - start + 1
    ))
}

impl Checker<'_, '_> {
    /// Rule 4: at most 60 instructions, one ret (none in _start).
    fn size_and_returns(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        let name = function.name();
        if function.steps.len() > MAX_INSTRUCTIONS {
            let point =
                split_point(function, flow).map_or_else(String::new, |point| format!(": {point}"));
            self.violation(
                Rule::Functions,
                function.source.line,
                format!("{name} has {} instructions", function.steps.len()),
                format!(
                    "split it into functions of at most {MAX_INSTRUCTIONS} instructions{point}"
                ),
            );
        }
        let returns: Vec<usize> = flow
            .exits
            .iter()
            .enumerate()
            .filter(|(_, exit)| **exit == Exit::Return)
            .map(|(index, _)| index)
            .collect();
        if name == "_start" {
            if let Some(first) = returns.first() {
                self.violation(
                    Rule::Functions,
                    function.steps[*first].source.line,
                    "_start returns, but it has nothing to return to",
                    "end _start with the exit system call",
                );
            }
        } else if returns.len() != 1 {
            self.violation(
                Rule::Functions,
                function.source.line,
                format!("{name} has {} ret instructions", returns.len()),
                "give each function exactly one ret, after a single cleanup path; jump to it",
            );
        }
    }

    /// Rule 4: callee-saved registers the function changes are pushed first and popped, in
    /// mirror order, right before its ret.
    fn callee_saved(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        if function.name() == "_start" {
            return;
        }
        let steps = &function.steps;
        let register_operand = |index: usize, name: &str| {
            let instruction = &steps[index].machine;
            (mnemonic(instruction) == name && instruction.op_kind(0) == OpKind::Register)
                .then(|| instruction.op_register(0).full_register())
        };
        let pushed: Vec<Register> = (0..steps.len())
            .map_while(|index| register_operand(index, "push"))
            .collect();
        let Some(ret) = flow.exits.iter().position(|exit| *exit == Exit::Return) else {
            return;
        };
        let popped: Vec<Register> = (0..ret)
            .rev()
            .map_while(|index| register_operand(index, "pop"))
            .collect();
        let saved_region = pushed.len()..ret - popped.len();
        let mut changed = BTreeSet::new();
        for index in saved_region {
            for register in CALLEE_SAVED {
                if writes(&steps[index].machine, register, &mut self.factory) {
                    changed.insert(format!("{register:?}").to_ascii_lowercase());
                }
            }
        }
        let pushed_names: BTreeSet<String> = pushed
            .iter()
            .map(|register| format!("{register:?}").to_ascii_lowercase())
            .collect();
        let unsaved: Vec<&String> = changed.difference(&pushed_names).collect();
        if !unsaved.is_empty() {
            let list: Vec<&str> = unsaved.iter().map(|name| name.as_str()).collect();
            self.violation(
                Rule::Functions,
                function.source.line,
                format!(
                    "{} changes {} without saving it",
                    function.name(),
                    list.join(", ")
                ),
                "push callee-saved registers (rbx, rbp, r12-r15) at the start and pop them in \
                 reverse order right before ret",
            );
        }
        if popped != pushed {
            self.violation(
                Rule::Functions,
                steps[ret].source.line,
                format!(
                    "{}'s pops before ret don't mirror its pushes at the start",
                    function.name()
                ),
                "pop the same registers in reverse order, immediately before ret",
            );
        }
    }

    /// Rule 3: nothing below rsp (no red zone).
    fn red_zone(&mut self, function: &MappedFunction<'_>) {
        for step in &function.steps {
            let instruction = &step.machine;
            let has_memory = (0..instruction.op_count())
                .any(|index| instruction.op_kind(index) == OpKind::Memory);
            let displacement =
                i64::from_ne_bytes(instruction.memory_displacement64().to_ne_bytes());
            if has_memory && instruction.memory_base() == Register::RSP && displacement < 0 {
                self.violation(
                    Rule::Resources,
                    step.source.line,
                    format!("`{}` uses memory below rsp", step.source.text),
                    "reserve stack space with sub rsp first; the dialect has no red zone",
                );
            }
        }
    }

    /// Rule 3 and 8: each system call is set up by `mov eax, SYS_NAME` in its straight-line
    /// run, for a call the spec allows.
    fn syscalls(
        &mut self,
        function: &MappedFunction<'_>,
        flow: &Flow,
        allowed: &[Syscall],
        profile: Profile,
    ) {
        for (index, exit) in flow.exits.iter().enumerate() {
            if !matches!(exit, Exit::Syscall { .. }) {
                continue;
            }
            let line = function.steps[index].source.line;
            let allowed_names: Vec<&str> = allowed.iter().map(|syscall| syscall.name()).collect();
            let Some((setter, number)) = rax_value_before(function, index, &mut self.factory)
            else {
                self.violation(
                    Rule::Resources,
                    line,
                    "this system call isn't chosen by a `mov eax, SYS_NAME` in the same \
                     straight-line code",
                    "put `mov eax, SYS_NAME` right before the syscall, with no label or call \
                     between them",
                );
                continue;
            };
            let Some(syscall) = allowed
                .iter()
                .find(|syscall| profile.syscall_number(**syscall) == number)
            else {
                // What the spec would have to name for this call, when it's one lengua knows.
                let known = Syscall::ALL
                    .into_iter()
                    .find(|syscall| profile.syscall_number(*syscall) == number);
                let (problem, fix) = match known {
                    Some(call) => {
                        let needs = match crate::spec::Use::of(call) {
                            Some(found) => format!("`{}` in its `- Uses:` line", found.name()),
                            None => "a `- Network:` line".to_owned(),
                        };
                        (
                            format!(
                                "system call {} isn't one the spec allows: it needs {needs}, \
                                 which spec.md doesn't name",
                                call.name()
                            ),
                            format!(
                                "use only what spec.md's Constraints name, or, when the program \
                                 needs it, add {needs} to spec.md and tell the user"
                            ),
                        )
                    }
                    None => (
                        format!(
                            "system call {number} isn't one the spec allows (it allows {})",
                            allowed_names.join(", ")
                        ),
                        "use only what spec.md's Constraints name".to_owned(),
                    ),
                };
                self.violation(Rule::Resources, line, problem, fix);
                continue;
            };
            let expected = format!("SYS_{}", syscall.name().to_ascii_uppercase());
            let named = matches!(
                function.steps[setter].source.operands.get(1),
                Some(Operand::Immediate { name: Some(name), .. }) if *name == expected
            );
            if !named {
                self.violation(
                    Rule::Constants,
                    function.steps[setter].source.line,
                    format!("system call {} isn't named", syscall.name()),
                    format!(
                        "write `mov eax, {expected}` with `.equ {expected}, {number}` at the top"
                    ),
                );
            }
        }
    }

    /// Rule 8: every exit status comes from `mov edi, NAME` on every path to the exit.
    fn exit_statuses(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        for (index, exit) in flow.exits.iter().enumerate() {
            if *exit != (Exit::Syscall { exit: true }) {
                continue;
            }
            let mut seen = BTreeSet::new();
            let mut pending: Vec<usize> = flow.predecessors[index].clone();
            let mut unset = flow.predecessors[index].is_empty();
            let mut bad = BTreeSet::new();
            while let Some(position) = pending.pop() {
                if !seen.insert(position) {
                    continue;
                }
                let step = &function.steps[position];
                if writes(&step.machine, Register::RDI, &mut self.factory) {
                    let named = mnemonic(&step.machine) == "mov"
                        && matches!(
                            step.source.operands.get(1),
                            Some(Operand::Immediate { name: Some(_), value })
                                if (0..=255).contains(value) && *value != i64::from(ASSERTION_EXIT_CODE)
                        );
                    if !named {
                        bad.insert(step.source.line);
                    }
                    continue;
                }
                if flow.predecessors[position].is_empty() {
                    unset = true;
                }
                pending.extend(&flow.predecessors[position]);
            }
            let line = function.steps[index].source.line;
            for bad_line in bad {
                self.violation(
                    Rule::Constants,
                    bad_line,
                    "this sets the exit status without a named constant (or to 134, which \
                     assertions use)",
                    "write `mov edi, EXIT_NAME` with `.equ EXIT_NAME, <0-255>` at the top",
                );
            }
            if unset {
                self.violation(
                    Rule::Constants,
                    line,
                    "on some path the exit status is never set",
                    "set edi with `mov edi, EXIT_NAME` on every path to the exit",
                );
            }
        }
    }

    /// Rule 7: results are tested before anything else touches rax.
    fn results(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        for (index, exit) in flow.exits.iter().enumerate() {
            let what = match exit {
                Exit::Syscall { exit: false } => "this system call's result".to_owned(),
                Exit::Call { callee } => match self.program.source.function(callee) {
                    Some(target) if target.returns_result => format!("{callee}'s result"),
                    _ => continue,
                },
                _ => continue,
            };
            let line = function.steps[index].source.line;
            let mut verdict = Err("nothing checks it");
            for next in index + 1..function.steps.len() {
                if function.label_before(next).is_some() {
                    verdict = Err("a label comes before the check");
                    break;
                }
                let instruction = &function.steps[next].machine;
                if reads(instruction, Register::RAX, &mut self.factory)
                    || writes(instruction, Register::RAX, &mut self.factory)
                {
                    let is_check = matches!(mnemonic(instruction).as_str(), "test" | "cmp");
                    let then_jumps = function.steps.get(next + 1).is_some_and(|after| {
                        after.machine.flow_control() == FlowControl::ConditionalBranch
                    });
                    verdict = if is_check && then_jumps {
                        Ok(())
                    } else {
                        Err("rax is used before it's checked")
                    };
                    break;
                }
                if instruction.flow_control() != FlowControl::Next {
                    verdict = Err("control moves on before the check");
                    break;
                }
            }
            if let Err(reason) = verdict {
                self.violation(
                    Rule::Results,
                    line,
                    format!("{what} isn't checked: {reason}"),
                    "follow it directly with `test rax, rax` and a conditional jump (js) to the \
                     error path",
                );
            }
        }
    }

    /// Rule 9: an indexed access needs a `cmp` of its index and a conditional jump earlier in
    /// the same straight-line code, with no change to the index in between.
    fn indexes(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        for (index, step) in function.steps.iter().enumerate() {
            let instruction = &step.machine;
            let has_memory = (0..instruction.op_count())
                .any(|operand| instruction.op_kind(operand) == OpKind::Memory);
            // `lea` only computes an address and never touches memory, like `add`.
            if !has_memory
                || mnemonic(instruction) == "lea"
                || instruction.memory_index() == Register::None
                || step.source.unsafe_block.is_some()
            {
                continue;
            }
            let register = instruction.memory_index().full_register();
            let name = format!("{register:?}").to_ascii_lowercase();
            let mut check = None;
            for position in (run_start(function, index)..index).rev() {
                let candidate = &function.steps[position].machine;
                let compares = mnemonic(candidate) == "cmp"
                    && candidate.op_kind(0) == OpKind::Register
                    && candidate.op_register(0).full_register() == register
                    && function.steps.get(position + 1).is_some_and(|next| {
                        next.machine.flow_control() == FlowControl::ConditionalBranch
                    });
                if compares {
                    check = Some(position);
                    break;
                }
                if writes(candidate, register, &mut self.factory) {
                    break;
                }
            }
            let Some(position) = check else {
                self.violation(
                    Rule::Indirection,
                    step.source.line,
                    format!(
                        "`{}` indexes memory with {name}, which isn't bounds-checked first",
                        step.source.text
                    ),
                    format!(
                        "since the last label, compare {name} with the length (cmp {name}, LEN) \
                         and jump away when it's out of range (jae), without changing {name} \
                         before the access; or wrap a proven-safe access in `# unsafe: <reason>` \
                         … `# end unsafe`"
                    ),
                );
                continue;
            };
            if let Some(problem) = self.compare_problem(function, position, register, &name, index)
            {
                self.violation(
                    Rule::Bounds,
                    function.steps[position].source.line,
                    problem,
                    format!(
                        "compare the whole of {name} with a limit from 0 up, as `cmp {name}, LEN`, \
                         after the last change to it"
                    ),
                );
                continue;
            }
            let jump = mnemonic(&function.steps[position + 1].machine);
            if !matches!(jump.as_str(), "jae" | "ja") {
                self.violation(
                    Rule::Indirection,
                    function.steps[position + 1].source.line,
                    format!("the bounds check of {name} jumps with {jump}"),
                    format!(
                        "jump away with jae (or ja), which compares without sign, so a {name} \
                         that's too big or negative never reaches the access"
                    ),
                );
                continue;
            }
            self.check_buffer_size(function, flow, index, position, jump == "ja");
        }
    }

    /// M2: why the compare at `position` doesn't bound all of `register`, if it doesn't: it
    /// compares only part of the register, whose upper bits nothing has cleared, or with a
    /// negative limit, which an unsigned jump reads as a huge one.
    fn compare_problem(
        &mut self,
        function: &MappedFunction<'_>,
        position: usize,
        register: Register,
        name: &str,
        access: usize,
    ) -> Option<String> {
        let compare = &function.steps[position].machine;
        let compared = compare.op_register(0);
        if !matches!(compare.op_kind(1), OpKind::Register | OpKind::Memory) {
            let limit = i64::from_ne_bytes(compare.immediate(1).to_ne_bytes());
            if limit < 0 {
                return Some(format!(
                    "the bounds check of {name} compares with {limit}, which an unsigned jump \
                     reads as a huge number, so every index passes"
                ));
            }
        }
        let bits = compared.size() * 8;
        if bits == 64 {
            return None;
        }
        // A 32-bit write zeroes the upper half, so a 32-bit compare then bounds the whole.
        let cleared = (run_start(function, position)..position)
            .rev()
            .find(|&at| writes(&function.steps[at].machine, register, &mut self.factory))
            .is_some_and(|at| {
                let written = &function.steps[at].machine;
                written.op_count() >= 1
                    && written.op_kind(0) == OpKind::Register
                    && written.op_register(0).full_register() == register
                    && written.op_register(0).size() == 4
            });
        if bits == 32 && cleared {
            return None;
        }
        Some(format!(
            "the bounds check of `{}` compares only the low {bits} bits of {name}, whose upper \
             bits nothing in this run has cleared",
            function.steps[access].source.text
        ))
    }

    /// When an indexed access goes through a register that only ever holds the address of one
    /// data symbol, the bounds check's limit must keep every access inside that symbol.
    fn check_buffer_size(
        &mut self,
        function: &MappedFunction<'_>,
        flow: &Flow,
        index: usize,
        check: usize,
        inclusive: bool,
    ) {
        let access = &function.steps[index].machine;
        let compare = &function.steps[check].machine;
        if matches!(compare.op_kind(1), OpKind::Register | OpKind::Memory) {
            return;
        }
        let limit = i64::from_ne_bytes(compare.immediate(1).to_ne_bytes());
        let Some((symbol, base_offset)) = self.buffer(function, flow, index, access.memory_base())
        else {
            return;
        };
        let Some(size) = self
            .program
            .data
            .get(&symbol)
            .and_then(|data| i64::try_from(data.size).ok())
        else {
            return;
        };
        let largest_index = if inclusive { limit } else { limit - 1 };
        let width = i64::try_from(access.memory_size().size())
            .unwrap_or(1)
            .max(1);
        let displacement = i64::from_ne_bytes(access.memory_displacement64().to_ne_bytes());
        let first = base_offset + displacement;
        let last = first + largest_index * i64::from(access.memory_index_scale()) + width - 1;
        if first < 0 || last >= size {
            self.violation(
                Rule::Indirection,
                function.steps[check].source.line,
                format!(
                    "this check lets `{}` reach byte {last} of {symbol}, which has {size} bytes",
                    function.steps[index].source.text
                ),
                format!("compare with a limit that keeps every access inside {symbol}"),
            );
        }
    }

    /// The data symbol (and offset into it) that `register` holds before the instruction at
    /// `index`, when every path sets it with `lea register, [rip + symbol]`.
    fn buffer(
        &mut self,
        function: &MappedFunction<'_>,
        flow: &Flow,
        index: usize,
        register: Register,
    ) -> Option<(String, i64)> {
        if register == Register::None {
            return None;
        }
        let mut found: Option<(String, i64)> = None;
        let mut seen = BTreeSet::new();
        let mut pending = flow.predecessors[index].clone();
        if pending.is_empty() {
            return None;
        }
        while let Some(position) = pending.pop() {
            if !seen.insert(position) {
                continue;
            }
            let instruction = &function.steps[position].machine;
            if !writes(instruction, register, &mut self.factory) {
                if flow.predecessors[position].is_empty() {
                    // The register comes from the caller: its buffer is unknown.
                    return None;
                }
                pending.extend(&flow.predecessors[position]);
                continue;
            }
            let loads_address = mnemonic(instruction) == "lea"
                && instruction.op_register(0).full_register() == register.full_register()
                && instruction.memory_base() == Register::RIP;
            if !loads_address {
                return None;
            }
            let (name, _, offset) = self.program.named(&function.steps[position])?;
            match &found {
                Some(known) if *known != (name.to_owned(), offset) => return None,
                Some(_) => {}
                None => found = Some((name.to_owned(), offset)),
            }
        }
        found
    }

    /// Rule 6: rip-relative operands name data symbols; read-only data isn't written; and who
    /// uses each writable symbol.
    fn data_access(
        &mut self,
        function: &MappedFunction<'_>,
        users: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for step in &function.steps {
            let instruction = &step.machine;
            if !instruction.is_ip_rel_memory_operand() {
                continue;
            }
            let Some((name, data, offset)) = self.program.named(step) else {
                self.violation(
                    Rule::Scope,
                    step.source.line,
                    format!(
                        "`{}` addresses something that isn't a data symbol",
                        step.source.text
                    ),
                    "address only data symbols, as [rip + name]",
                );
                continue;
            };
            // Every byte of the access stays inside the symbol it names, vector widths included,
            // even where the next symbol starts just past its end; lea may point just past the
            // end, as a limit does.
            let lea = mnemonic(instruction) == "lea";
            let width = if lea {
                0
            } else {
                i64::try_from(instruction.memory_size().size()).unwrap_or(0)
            };
            let size = i64::try_from(data.size).unwrap_or(i64::MAX);
            let (reaches, fix) = if lea {
                (
                    "points",
                    format!("point inside {name}, or just past its end"),
                )
            } else {
                ("reaches", format!("keep the whole access inside {name}"))
            };
            let bytes = |count: i64| format!("{count} byte{}", if count == 1 { "" } else { "s" });
            if offset < 0 {
                self.violation(
                    Rule::Scope,
                    step.source.line,
                    format!(
                        "`{}` {reaches} {} before the start of {name}",
                        step.source.text,
                        bytes(-offset)
                    ),
                    fix,
                );
            } else if offset + width > size {
                self.violation(
                    Rule::Scope,
                    step.source.line,
                    format!(
                        "`{}` {reaches} {} past the end of {name}, which has {size} bytes",
                        step.source.text,
                        bytes(offset + width - size)
                    ),
                    fix,
                );
            }
            let (name, writable) = (name.to_owned(), data.writable);
            let writes_memory = self
                .factory
                .info(instruction)
                .used_memory()
                .iter()
                .any(|memory| {
                    matches!(
                        memory.access(),
                        OpAccess::Write
                            | OpAccess::ReadWrite
                            | OpAccess::CondWrite
                            | OpAccess::ReadCondWrite
                    )
                });
            if writes_memory && !writable {
                self.violation(
                    Rule::Scope,
                    step.source.line,
                    format!("`{}` writes to read-only {name}", step.source.text),
                    "move data the program changes to .data or .bss",
                );
            }
            if writable && function.name() != ASSERT_FAIL {
                users
                    .entry(name)
                    .or_default()
                    .insert(function.name().to_owned());
            }
        }
    }

    /// Rule 6: each writable symbol belongs to one function unless marked `# shared:`.
    fn shared_data(&mut self, users: &BTreeMap<String, BTreeSet<String>>) {
        for (name, functions) in users {
            let Some(data) = self
                .program
                .source
                .data
                .iter()
                .find(|data| data.name == *name)
            else {
                continue;
            };
            if functions.len() > 1 && !data.shared {
                let list: Vec<&str> = functions.iter().map(String::as_str).collect();
                self.violation(
                    Rule::Scope,
                    data.line,
                    format!("{name} is used by {}", list.join(", ")),
                    "let one function own it and pass a pointer and length to the others, or \
                     mark it `# shared: <reason>` above its .type line",
                );
            }
        }
    }

    /// Rule 8: a constant NAME_LEN matches the size of the data symbol `name`.
    fn lengths(&mut self) {
        let source = self.program.source;
        let mut problems = Vec::new();
        for (constant, value) in &source.constants {
            let Some(symbol) = constant.strip_suffix("_LEN").map(str::to_ascii_lowercase) else {
                continue;
            };
            let Some(data) = self.program.data.get(&symbol) else {
                continue;
            };
            if u64::try_from(value.value).ok() != Some(data.size) {
                problems.push((value.line, constant.clone(), value.value, symbol, data.size));
            }
        }
        for (line, constant, value, symbol, size) in problems {
            self.violation(
                Rule::Constants,
                line,
                format!("{constant} is {value}, but {symbol} is {size} bytes"),
                format!("set `.equ {constant}, {size}`"),
            );
        }
    }

    /// Rule 1: no recursion, directly or through other functions.
    fn recursion(&mut self) {
        let calls: BTreeMap<&str, Vec<(String, usize)>> = self
            .program
            .functions
            .iter()
            .map(|function| {
                let callees = self
                    .flows
                    .get(function.name())
                    .map_or_else(Vec::new, |flow| {
                        flow.exits
                            .iter()
                            .enumerate()
                            .filter_map(|(index, exit)| match exit {
                                Exit::Call { callee } => {
                                    Some((callee.clone(), function.steps[index].source.line))
                                }
                                _ => None,
                            })
                            .collect()
                    });
                (function.name(), callees)
            })
            .collect();
        let mut cycles = Vec::new();
        for (start, callees) in &calls {
            for (callee, line) in callees {
                // Does `callee` reach `start` again?
                let mut seen = BTreeSet::new();
                let mut pending = vec![callee.as_str()];
                while let Some(current) = pending.pop() {
                    if current == *start {
                        cycles.push((*line, (*start).to_owned(), callee.clone()));
                        break;
                    }
                    if seen.insert(current)
                        && let Some(next) = calls.get(current)
                    {
                        pending.extend(next.iter().map(|(name, _)| name.as_str()));
                    }
                }
            }
        }
        for (line, caller, callee) in cycles {
            self.violation(
                Rule::ControlFlow,
                line,
                format!("{caller} calls {callee}, which leads back to {caller}: recursion"),
                "rewrite the recursion as a counted loop",
            );
        }
    }

    /// Rule 1: a chain of two or more `cmp R, constant` + `je` on one register ends with a
    /// jump to an assertion, so no case goes unhandled silently.
    fn chains(&mut self, function: &MappedFunction<'_>, flow: &Flow) {
        let steps = &function.steps;
        let compare_on = |index: usize| -> Option<Register> {
            let instruction = &steps.get(index)?.machine;
            (mnemonic(instruction) == "cmp"
                && instruction.op_kind(0) == OpKind::Register
                && !matches!(instruction.op_kind(1), OpKind::Register | OpKind::Memory))
            .then(|| instruction.op_register(0).full_register())
        };
        let is = |index: usize, name: &str| {
            steps
                .get(index)
                .is_some_and(|step| mnemonic(&step.machine) == name)
        };
        let mut index = 0;
        while index < steps.len() {
            let Some(register) = compare_on(index).filter(|_| is(index + 1, "je")) else {
                index += 1;
                continue;
            };
            let mut end = index;
            while compare_on(end) == Some(register) && is(end + 1, "je") {
                end += 2;
            }
            let pairs = (end - index) / 2;
            if pairs >= 2 {
                let assertion_at = |position: usize| match flow.exits.get(position) {
                    Some(Exit::Jump(target) | Exit::Branch(target)) => {
                        self.is_assertion(function, *target)
                    }
                    _ => false,
                };
                let defaulted = (is(end, "jmp") && assertion_at(end))
                    || (compare_on(end) == Some(register)
                        && is(end + 1, "jne")
                        && assertion_at(end + 1));
                if !defaulted {
                    let name = format!("{register:?}").to_ascii_lowercase();
                    self.violation(
                        Rule::ControlFlow,
                        steps[index].source.line,
                        format!("this chain of {pairs} comparisons on {name} has no default"),
                        "end the chain with `jmp` to an assertion (or make the last comparison \
                         `jne` to one), so an unexpected value can't slip through",
                    );
                }
            }
            index = end.max(index + 1);
        }
    }

    /// Whether the instruction at `index` starts an assertion block.
    fn is_assertion(&self, function: &MappedFunction<'_>, index: usize) -> bool {
        let steps = &function.steps;
        steps
            .get(index)
            .is_some_and(|step| mnemonic(&step.machine) == "mov")
            && steps.get(index + 1).is_some_and(|step| {
                step.machine.near_branch_target()
                    == self
                        .program
                        .function(ASSERT_FAIL)
                        .map_or(0, |assert| assert.address)
                    && mnemonic(&step.machine) == "call"
            })
    }

    /// Rule 2: every backward jump closes a counted loop, except in a server: its loop that takes
    /// connections, `# bound: forever`, closes with an unconditional jump, so it provably never
    /// ends by itself (NASA's exception for loops meant not to terminate).
    fn loops(&mut self, function: &MappedFunction<'_>, flow: &Flow, server: bool) {
        let steps = &function.steps;
        let fix = "write loops as: mov R, NAME (a constant of at most the bound) right before the \
                   head label `.Lname:  # bound: N`, the body, then dec R and jnz .Lname; enter \
                   only through the head and never change R inside";
        for (back, exit) in flow.exits.iter().enumerate() {
            let head = match exit {
                Exit::Jump(target) | Exit::Branch(target) if *target <= back => *target,
                _ => continue,
            };
            let line = steps[back].source.line;
            let forever = function
                .labels
                .iter()
                .any(|label| label.index == head && label.label.forever);
            if forever {
                if !server {
                    self.violation(
                        Rule::Loops,
                        steps[head].source.line,
                        "only a server's loop that takes connections may run forever",
                        "give the loop a counter and `# bound: N`; `# bound: forever` is for a \
                         program whose spec says `- Network: listens on TCP port N`",
                    );
                } else if !matches!(exit, Exit::Jump(_)) {
                    self.violation(
                        Rule::Loops,
                        line,
                        "a loop that runs forever closes with an unconditional `jmp` to its head",
                        "end the loop with `jmp` to its `# bound: forever` label, so it provably \
                         never ends by itself",
                    );
                }
                self.entered_through_head(steps, flow, head, back);
                continue;
            }
            let counter = back
                .checked_sub(1)
                .map(|index| &steps[index].machine)
                .filter(|decrement| {
                    decrement.op_kind(0) == OpKind::Register
                        && (mnemonic(decrement) == "dec"
                            || (mnemonic(decrement) == "sub"
                                && decrement.op_count() == 2
                                && decrement.op_kind(1) != OpKind::Register
                                && decrement.immediate(1) == 1))
                })
                .map(|decrement| decrement.op_register(0).full_register());
            let Some(counter) = counter.filter(|_| mnemonic(&steps[back].machine) == "jne") else {
                self.violation(
                    Rule::Loops,
                    line,
                    "this backward jump isn't the end of a counted loop",
                    fix,
                );
                continue;
            };
            let init = head.checked_sub(1).and_then(|index| {
                let step = &steps[index];
                let instruction = &step.machine;
                let sets_counter = mnemonic(instruction) == "mov"
                    && instruction.op_kind(0) == OpKind::Register
                    && instruction.op_register(0).full_register() == counter;
                match step.source.operands.get(1) {
                    Some(Operand::Immediate {
                        name: Some(_),
                        value,
                    }) if sets_counter => Some(*value),
                    _ => None,
                }
            });
            let head_line = steps[head].source.line;
            let Some(count) = init else {
                self.violation(
                    Rule::Loops,
                    head_line,
                    "the loop counter isn't set by `mov R, NAME` right before the loop's head",
                    fix,
                );
                continue;
            };
            let bound = function
                .labels
                .iter()
                .filter(|label| label.index == head)
                .find_map(|label| label.label.bound);
            match bound {
                None => self.violation(
                    Rule::Loops,
                    head_line,
                    "the loop's head label has no `# bound: N`",
                    "write the most iterations after the head label, such as `.Lloop:  # bound: 64`",
                ),
                Some(bound) if count < 1 || u64::try_from(count).map_or(true, |count| count > bound) => self.violation(
                    Rule::Loops,
                    head_line,
                    format!("the counter starts at {count}, outside 1 to the bound {bound}"),
                    "start the counter between 1 and the bound",
                ),
                Some(_) => {}
            }
            self.entered_through_head(steps, flow, head, back);
            for step in &steps[head..back.saturating_sub(1)] {
                if writes(&step.machine, counter, &mut self.factory) {
                    let name = format!("{counter:?}").to_ascii_lowercase();
                    self.violation(
                        Rule::Loops,
                        step.source.line,
                        format!("this changes the loop counter {name} inside the loop"),
                        "keep the counter in a register nothing in the body changes; calls change \
                         rax, rcx, rdx, rsi, rdi and r8-r11, and syscall changes rax, rcx and r11, \
                         so loops with calls count in rbx or r12-r15",
                    );
                }
            }
        }
    }

    /// Nothing jumps into a loop's body from outside: it's entered by falling into its head.
    fn entered_through_head(
        &mut self,
        steps: &[super::decode::Step<'_>],
        flow: &Flow,
        head: usize,
        back: usize,
    ) {
        for position in head..=back {
            for &from in &flow.predecessors[position] {
                let inside = (head..=back).contains(&from);
                let entering_head = position == head && from + 1 == head;
                if !inside && !entering_head {
                    self.violation(
                        Rule::Loops,
                        steps[from].source.line,
                        "this jumps into the middle of a loop",
                        "enter loops only through their head, right after the counter is set",
                    );
                }
            }
        }
    }

    /// Rule 5: assertions are `label: mov edi, <1-999>; call assert_fail`, each reached from
    /// one conditional place, with unique numbers. Returns how many the function has.
    fn assertions(
        &mut self,
        function: &MappedFunction<'_>,
        flow: &Flow,
        ids: &mut BTreeMap<i64, usize>,
    ) -> usize {
        let steps = &function.steps;
        let mut count = 0;
        for (index, exit) in flow.exits.iter().enumerate() {
            if !matches!(exit, Exit::Call { callee } if callee == ASSERT_FAIL) {
                continue;
            }
            let line = steps[index].source.line;
            let shape = "an assertion is exactly a label, `mov edi, <number 1-999>` and `call \
                         assert_fail`";
            let Some(start) = index.checked_sub(1) else {
                self.violation(
                    Rule::Assertions,
                    line,
                    "this call to assert_fail isn't an assertion block",
                    shape,
                );
                continue;
            };
            let id = match (
                mnemonic(&steps[start].machine).as_str(),
                steps[start].source.operands.as_slice(),
            ) {
                (
                    "mov",
                    [
                        Operand::Register(Register::EDI),
                        Operand::Immediate { value, name: None },
                    ],
                ) if (1..=999).contains(value) => *value,
                _ => {
                    self.violation(
                        Rule::Assertions,
                        line,
                        "this call to assert_fail isn't an assertion block",
                        shape,
                    );
                    continue;
                }
            };
            let id_line = steps[start].source.line;
            if function.label_before(start).is_none() {
                self.violation(
                    Rule::Assertions,
                    id_line,
                    format!("assertion {id} has no label of its own"),
                    shape,
                );
                continue;
            }
            if let Some(first) = ids.insert(id, id_line) {
                self.violation(
                    Rule::Assertions,
                    id_line,
                    format!("assertion number {id} is used twice (also on line {first})"),
                    "number each assertion uniquely, from 1 to 999",
                );
            }
            let entries = &flow.predecessors[start];
            match entries.as_slice() {
                [from] => {
                    let conditional = match &flow.exits[*from] {
                        Exit::Branch(_) => true,
                        Exit::Jump(_) => from
                            .checked_sub(1)
                            .is_some_and(|before| matches!(flow.exits[before], Exit::Branch(_))),
                        _ => false,
                    };
                    if conditional {
                        count += 1;
                    } else {
                        self.violation(
                            Rule::Assertions,
                            id_line,
                            format!("assertion {id} always fails"),
                            "reach an assertion only through a conditional jump after the check",
                        );
                    }
                }
                [] => {}
                _ => self.violation(
                    Rule::Assertions,
                    id_line,
                    format!("assertion {id} is reached from {} places", entries.len()),
                    "give each check its own assertion block and number",
                ),
            }
        }
        count
    }

    /// assert_fail and its message are copied word for word from the template.
    fn canonical(&mut self, template: &Source) {
        let source = self.program.source;
        let expected_function = template
            .function(ASSERT_FAIL)
            .map(|function| &function.body);
        match source.function(ASSERT_FAIL) {
            Some(function) if Some(&function.body) == expected_function => {}
            Some(function) => {
                let line = function.line;
                self.violation(
                    Rule::Assertions,
                    line,
                    "assert_fail differs from the template",
                    "copy assert_fail word for word from assets/main-template.s",
                );
            }
            None => {
                let line = source.functions.last().map_or(1, |function| function.line);
                self.violation(
                    Rule::Assertions,
                    line,
                    "main.s has no assert_fail",
                    "copy assert_fail and assert_message from assets/main-template.s",
                );
            }
        }
        let message = |source: &Source| {
            source
                .data
                .iter()
                .find(|data| data.name == "assert_message")
                .map(|data| (data.body.clone(), data.section, data.line))
        };
        match (message(source), message(template)) {
            (Some((body, section, _)), Some((expected, expected_section, _)))
                if body == expected && section == expected_section => {}
            (found, _) => {
                let line = found.map_or(1, |(_, _, line)| line);
                self.violation(
                    Rule::Assertions,
                    line,
                    "assert_message differs from the template",
                    "copy assert_message word for word into .rodata from assets/main-template.s",
                );
            }
        }
    }

    /// fill, when main.s has it, is copied word for word with its constants: the proof reads
    /// it as a whole, for every way its input can arrive.
    fn canonical_fill(&mut self, template: &Source) {
        let source = self.program.source;
        let Some(function) = source.function(FILL) else {
            return;
        };
        if Some(&function.body) != template.function(FILL).map(|expected| &expected.body) {
            let line = function.line;
            self.violation(
                Rule::Assertions,
                line,
                "fill differs from the template",
                "copy fill word for word from assets/fill.s",
            );
        }
        for name in FILL_CONSTANTS {
            let expected = template.constants.get(name).map(|constant| constant.value);
            let found = source.constants.get(name);
            if found.map(|constant| constant.value) != expected {
                let line = found.map_or(function.line, |constant| constant.line);
                self.violation(
                    Rule::Constants,
                    line,
                    format!(
                        "{name} must be {}, as fill needs",
                        expected.unwrap_or_default()
                    ),
                    format!("copy `.equ {name}` from assets/fill.s"),
                );
            }
        }
    }

    /// The deepest stack over every call path from `_start`.
    fn max_stack(&self) -> u64 {
        u64::try_from(deepest("_start", self.program, self.flows)).unwrap_or(0)
    }
}
