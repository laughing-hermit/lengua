//! Each function's control flow, and the call graph, for rules 1, 2 and 4.
//!
//! WebAssembly's control is structured already: blocks, loops and ifs nest, and a branch leaves
//! to a label around it. What the checker adds: no recursion (the call graph has no cycle, so
//! the deepest call path is known); `unreachable` only right after a call that never returns,
//! and every such call followed by it; a `br_table`'s default leading to an assertion or an
//! error; every loop counted (rule 2's pattern, `;; bound: N`) or a server's one accept loop,
//! `;; bound: forever`, which calls `sock_accept` and is closed by an unconditional branch back;
//! no `return`, nor branch to the function's own label, before a function's end (rule 4); and
//! `_start` ending every path in `proc_exit`, so the exit status is named.

use std::collections::{BTreeMap, BTreeSet};

use wasmparser::{BlockType, FrameKind, Operator};

use super::model::{Function, Kind, Module, to_usize};
use super::rules::returns_result;
use crate::checker::{Rule, Violation};
use crate::wasm::Policy;

/// A function's control structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shape {
    /// The `end` of each `block`, `loop` and `if`, by the opener's index.
    pub ends: BTreeMap<usize, usize>,
    /// The `else` of each `if` that has one, by the opener's index.
    pub elses: BTreeMap<usize, usize>,
    /// The opener each `end` and `else` closes, by its own index; the function's final `end`
    /// closes none.
    pub openers: BTreeMap<usize, usize>,
    /// Where each branch may go, by its index: the opener of the block it leaves or of the loop
    /// it repeats, or `None` for the function's body. A `br_table` lists its cases, then its
    /// default.
    pub targets: BTreeMap<usize, Vec<Option<usize>>>,
    /// The first operator of each run of code no path reaches.
    pub dead: Vec<usize>,
    /// Whether a path reaches the function's end.
    pub falls_through: bool,
    /// The most values on its operand stack at once.
    pub max_stack: usize,
    /// Each call it makes: the operator's index and the function it calls.
    pub calls: Vec<(usize, u32)>,
    /// For each operator, the operators that put the values it takes on the stack, the deepest
    /// first; `None` for a value a block gave.
    pub producers: Vec<Vec<Option<usize>>>,
}

/// The control flow of every function the checker reads, and the order to follow values in.
#[derive(Debug, Clone, Default)]
pub struct Flows {
    /// Each function's shape, main.wat's own and lengua's library's, by index.
    pub shapes: BTreeMap<u32, Shape>,
    /// main.wat's own functions with callees first, so a function's callees are followed before
    /// it is. In a cycle, which rule 1 refuses, the order is arbitrary.
    pub order: Vec<u32>,
}

/// A block being read: what it opened with, and how control reaches its end.
struct Open {
    /// Its opener's index, or `None` for the function's body.
    opener: Option<usize>,
    /// What kind of block it is.
    kind: FrameKind,
    /// Its type.
    ty: BlockType,
    /// The stack's height below its parameters.
    base: usize,
    /// Whether control reaches its start.
    entered: bool,
    /// For an `if` with an `else`: whether its `then` reaches the `else`.
    then_falls: bool,
    /// Whether a branch leaves to its end.
    branched: bool,
}

/// The index of the block `depth` levels out of the innermost of `frames`.
fn frame_at(frames: &[Open], depth: u32) -> Option<usize> {
    frames.len().checked_sub(to_usize(depth).checked_add(1)?)
}

/// `function`'s control structure.
#[must_use]
pub fn shape(module: &Module<'_>, function: &Function<'_>) -> Shape {
    let mut shape = Shape::default();
    let mut frames = vec![Open {
        opener: None,
        kind: FrameKind::Block,
        ty: BlockType::FuncType(function.type_index),
        base: 0,
        entered: true,
        then_falls: false,
        branched: false,
    }];
    let mut arity_frames = vec![Module::function_frame(function)];
    // Which operator put each value on the stack.
    let mut values: Vec<Option<usize>> = Vec::new();
    shape.producers = vec![Vec::new(); function.body.len()];
    let mut reachable = true;
    let mut in_dead_run = false;
    for (at, op) in function.body.iter().enumerate() {
        let operator = &op.operator;
        let closing = matches!(operator, Operator::End | Operator::Else);
        if closing {
            in_dead_run = false;
        } else if !reachable && !in_dead_run {
            shape.dead.push(at);
            in_dead_run = true;
        }
        match operator {
            Operator::Block { blockty } | Operator::Loop { blockty } | Operator::If { blockty } => {
                let kind = match operator {
                    Operator::Loop { .. } => FrameKind::Loop,
                    Operator::If { .. } => FrameKind::If,
                    _ => FrameKind::Block,
                };
                if reachable && kind == FrameKind::If {
                    // The condition.
                    shape.producers[at] = vec![values.pop().flatten()];
                }
                let (params, _) = module.block_arity(*blockty);
                frames.push(Open {
                    opener: Some(at),
                    kind,
                    ty: *blockty,
                    base: values.len().saturating_sub(params),
                    entered: reachable,
                    then_falls: false,
                    branched: false,
                });
                arity_frames.push((*blockty, kind));
            }
            Operator::Else => {
                if let Some(frame) = frames.last_mut() {
                    frame.then_falls = reachable;
                    frame.kind = FrameKind::Else;
                    reachable = frame.entered;
                    let (params, _) = module.block_arity(frame.ty);
                    values.truncate(frame.base);
                    values.resize(frame.base + params, None);
                    if let Some(opener) = frame.opener {
                        shape.elses.insert(opener, at);
                        shape.openers.insert(at, opener);
                    }
                }
                if let Some(last) = arity_frames.last_mut() {
                    last.1 = FrameKind::Else;
                }
            }
            Operator::End => {
                let Some(frame) = frames.pop() else {
                    continue;
                };
                arity_frames.pop();
                let (_, results) = module.block_arity(frame.ty);
                let after = match frame.kind {
                    FrameKind::Loop => reachable,
                    FrameKind::Else => reachable || frame.then_falls || frame.branched,
                    // An `if` without an `else` falls through when its condition is false.
                    FrameKind::If => reachable || frame.entered || frame.branched,
                    _ => reachable || frame.branched,
                };
                match frame.opener {
                    Some(opener) => {
                        shape.ends.insert(opener, at);
                        shape.openers.insert(at, opener);
                    }
                    None => shape.falls_through = after,
                }
                values.truncate(frame.base);
                values.resize(frame.base + results, None);
                reachable = after;
            }
            _ => {
                if reachable {
                    let (takes, gives) = module.arity(&arity_frames, operator);
                    let start = values.len().saturating_sub(takes);
                    let mut taken = values.split_off(start);
                    while taken.len() < takes {
                        taken.insert(0, None);
                    }
                    shape.producers[at] = taken;
                    values.extend(std::iter::repeat_n(Some(at), gives));
                    shape.max_stack = shape.max_stack.max(values.len());
                }
                step(&mut shape, &mut frames, at, operator, &mut reachable);
            }
        }
    }
    shape
}

/// Marks the block `depth` levels out of the innermost of `frames` as left by a branch, when the
/// branch is `reachable`, and gives its opener.
fn mark(frames: &mut [Open], depth: u32, reachable: bool) -> Option<usize> {
    let index = frame_at(frames, depth)?;
    let frame = &mut frames[index];
    if reachable {
        frame.branched = true;
    }
    frame.opener
}

/// What a plain operator does to control: a branch marks where it goes, and one that never falls
/// through ends the reachable code.
fn step(
    shape: &mut Shape,
    frames: &mut [Open],
    at: usize,
    operator: &Operator<'_>,
    reachable: &mut bool,
) {
    match operator {
        Operator::Br { relative_depth } => {
            let target = mark(frames, *relative_depth, *reachable);
            shape.targets.insert(at, vec![target]);
            *reachable = false;
        }
        Operator::BrIf { relative_depth } => {
            let target = mark(frames, *relative_depth, *reachable);
            shape.targets.insert(at, vec![target]);
        }
        Operator::BrTable { targets } => {
            let mut all = Vec::new();
            for depth in targets.targets().flatten() {
                all.push(mark(frames, depth, *reachable));
            }
            all.push(mark(frames, targets.default(), *reachable));
            shape.targets.insert(at, all);
            *reachable = false;
        }
        Operator::Return => {
            if *reachable && let Some(body) = frames.first_mut() {
                body.branched = true;
            }
            shape.targets.insert(at, vec![None]);
            *reachable = false;
        }
        Operator::Unreachable => *reachable = false,
        Operator::Call { function_index } => shape.calls.push((at, *function_index)),
        _ => {}
    }
}

/// Reads every function's control flow and checks rules 1, 2 and 4's `return`.
pub fn analyze(module: &Module<'_>, policy: &Policy, violations: &mut Vec<Violation>) -> Flows {
    let mut flows = Flows::default();
    for function in &module.functions {
        if matches!(function.kind, Kind::Own | Kind::Library) {
            flows.shapes.insert(function.index, shape(module, function));
        }
    }
    let mut checker = Checker {
        module,
        flows: &flows,
        violations,
    };
    let mut forever = 0_usize;
    for function in module.own() {
        let Some(shape) = flows.shapes.get(&function.index) else {
            continue;
        };
        checker.unreachable(function);
        checker.tables(function, shape);
        checker.returns(function, shape);
        for (at, op) in function.body.iter().enumerate() {
            if matches!(op.operator, Operator::Loop { .. }) {
                checker.counted(function, shape, at, policy, &mut forever);
            }
        }
    }
    checker.recursion();
    checker.entry();
    flows.order = order(module, &flows);
    flows
}

/// main.wat's own functions, callees first: a depth-first walk of the calls, each function after
/// everything it calls, kept on a stack of its own rather than the checker's.
fn order(module: &Module<'_>, flows: &Flows) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for root in module.own() {
        if !seen.insert(root.index) {
            continue;
        }
        // Each function on the walk, with the next of its calls to follow.
        let mut stack: Vec<(u32, usize)> = vec![(root.index, 0)];
        while let Some(&(current, next)) = stack.last() {
            let calls = flows
                .shapes
                .get(&current)
                .map_or(&[][..], |shape| shape.calls.as_slice());
            if let Some(&(_, callee)) = calls.get(next) {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                let own = module
                    .function(callee)
                    .is_some_and(|function| function.kind == Kind::Own);
                if own && seen.insert(callee) {
                    stack.push((callee, 0));
                }
            } else {
                order.push(current);
                stack.pop();
            }
        }
    }
    order
}

/// The function the operator at `at` of `function` calls, when it's a call.
#[must_use]
pub fn callee_at<'m, 'a>(
    module: &'m Module<'a>,
    function: &Function<'_>,
    at: usize,
) -> Option<&'m Function<'a>> {
    match function.body.get(at)?.operator {
        Operator::Call { function_index } => module.function(function_index),
        _ => None,
    }
}

/// The checks of this module, with what they report to.
struct Checker<'m, 'a, 'v> {
    /// The module.
    module: &'m Module<'a>,
    /// Every function's control flow.
    flows: &'m Flows,
    /// Where violations go.
    violations: &'v mut Vec<Violation>,
}

impl Checker<'_, '_, '_> {
    /// Reports a violation of `rule` on `line`.
    fn violation(
        &mut self,
        rule: Rule,
        line: Option<usize>,
        problem: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.violations.push(Violation {
            rule,
            line,
            problem: problem.into(),
            fix: fix.into(),
        });
    }

    /// Rule 1: `unreachable` comes only right after a call that never returns, and every such
    /// call is followed by it.
    fn unreachable(&mut self, function: &Function<'_>) {
        let module = self.module;
        for (at, op) in function.body.iter().enumerate() {
            if matches!(op.operator, Operator::Unreachable) {
                let after_exit = at
                    .checked_sub(1)
                    .and_then(|before| callee_at(module, function, before))
                    .is_some_and(Function::never_returns);
                if !after_exit {
                    self.violation(
                        Rule::ControlFlow,
                        op.line,
                        "`unreachable` traps here, but it may only follow a call that never \
                         returns",
                        "end a path with `(call $proc_exit …)` or `(call $assert_fail (i32.const \
                         N))` and then `unreachable`; leave a block with `br` otherwise",
                    );
                }
            }
            let Some(callee) = callee_at(module, function, at) else {
                continue;
            };
            let followed = function
                .body
                .get(at + 1)
                .is_some_and(|next| matches!(next.operator, Operator::Unreachable));
            if callee.never_returns() && !followed {
                self.violation(
                    Rule::ControlFlow,
                    op.line,
                    format!(
                        "{} never returns, but no `unreachable` follows it",
                        callee.shown()
                    ),
                    "write `unreachable` right after the call, so every path visibly ends there",
                );
            }
        }
    }

    /// Whether the code from `at` on, after an `end`, starts an assertion or an error path: a
    /// call to `$assert_fail`; an exit with a status that isn't 0; or, in a function marked
    /// `;; returns: result`, an error number from a named constant that is what the function
    /// gives, nothing but the ends of blocks between it and the function's end.
    fn leads_to_error(&self, function: &Function<'_>, mut at: usize) -> bool {
        let module = self.module;
        let ops = &function.body;
        let last = ops.len().saturating_sub(1);
        // The end of a block around it falls through to what follows that.
        while at < last && matches!(ops[at].operator, Operator::End) {
            at += 1;
        }
        if at >= last {
            return false;
        }
        let next_call = callee_at(module, function, at + 1);
        match ops[at].operator {
            Operator::I32Const { .. } => next_call.is_some_and(|callee| {
                callee.is_library("assert_fail") || callee.name.as_deref() == Some("assert_fail")
            }),
            Operator::GlobalGet { global_index } => {
                let failure = module.global(global_index).is_some_and(|global| {
                    global.unsigned().is_some_and(|value| value != 0)
                        && module.region_of(global_index).is_none()
                        && module.sized_by(global_index).is_none()
                });
                if !failure {
                    return false;
                }
                if next_call.is_some_and(|callee| callee.is_host("proc_exit")) {
                    return true;
                }
                let given = ops[at + 1..]
                    .iter()
                    .all(|op| matches!(op.operator, Operator::End));
                given && returns_result(module, function)
            }
            _ => false,
        }
    }

    /// Rule 1: a `br_table`'s default leads to an assertion or an error path.
    fn tables(&mut self, function: &Function<'_>, shape: &Shape) {
        for (at, op) in function.body.iter().enumerate() {
            if !matches!(op.operator, Operator::BrTable { .. }) {
                continue;
            }
            let default = shape
                .targets
                .get(&at)
                .and_then(|targets| targets.last().copied());
            let good = match default {
                Some(Some(opener)) => {
                    !matches!(function.body[opener].operator, Operator::Loop { .. })
                        && shape
                            .ends
                            .get(&opener)
                            .is_some_and(|end| self.leads_to_error(function, end + 1))
                }
                _ => false,
            };
            if !good {
                self.violation(
                    Rule::ControlFlow,
                    op.line,
                    "this br_table's default doesn't lead to an assertion or an error, so an \
                     unexpected value could slip through",
                    "make the default a block whose end is followed by an assertion, `(call \
                     $assert_fail (i32.const N)) unreachable`, or by an error: an exit with a \
                     status that isn't 0, or, as the result of a function marked `;; returns: \
                     result`, an error number from a named constant",
                );
            }
        }
    }

    /// Rule 4: a function ends once, at its end: no `return`, and no branch to the function's
    /// own label, which leaves it as a `return` does.
    fn returns(&mut self, function: &Function<'_>, shape: &Shape) {
        for (at, targets) in &shape.targets {
            let Some(op) = function.body.get(*at) else {
                continue;
            };
            if !targets.contains(&None) {
                continue;
            }
            let what = if matches!(op.operator, Operator::Return) {
                "`return`"
            } else {
                "this branch to the function's own label"
            };
            self.violation(
                Rule::Functions,
                op.line,
                format!("{what} leaves {} before its end", function.shown()),
                "end a function once, at its end: leave a block with `br` instead, and put the \
                 result on the stack before the function's end",
            );
        }
    }

    /// Rule 2: the loop that `loop` at `at` opens is counted, or it's a server's accept loop.
    fn counted(
        &mut self,
        function: &Function<'_>,
        shape: &Shape,
        at: usize,
        policy: &Policy,
        forever: &mut usize,
    ) {
        let module = self.module;
        let ops = &function.body;
        let Some(&end) = shape.ends.get(&at) else {
            return;
        };
        let line = ops[at].line;
        let fix = "write a loop as: `(local.set $n (global.get $MAX_…))` right before \
                   `(loop $name  ;; bound: N`, the body, then `(local.set $n (i32.sub (local.get \
                   $n) (i32.const 1)))` and `(br_if $name (local.get $n))` as its last two \
                   instructions";
        let marker = line.and_then(|line| module.notes.marker(line, "bound:"));
        let Some(marker) = marker else {
            self.violation(
                Rule::Loops,
                line,
                "this loop has no `;; bound: N` on its line",
                "write the most iterations on the loop's line, such as `(loop $read  ;; bound: \
                 64`",
            );
            return;
        };
        let word = marker.split_whitespace().next().unwrap_or_default();
        if word == "forever" {
            self.forever(function, shape, at, end, policy, forever);
            return;
        }
        let bound = match word.parse::<u64>() {
            Ok(bound) if bound >= 1 => bound,
            _ => {
                self.violation(
                    Rule::Loops,
                    line,
                    format!("`;; bound: {marker}` needs a whole number from 1"),
                    "write the most iterations, such as `;; bound: 64`",
                );
                return;
            }
        };
        let Some((counter, decrement)) = closing(ops, at, end) else {
            self.violation(
                Rule::Loops,
                ops[end.saturating_sub(1)].line.or(line),
                "this loop doesn't end by taking one from its counter and branching back while \
                 it isn't zero",
                fix,
            );
            return;
        };
        let counter_name = function.local_shown(counter);
        // Set from a named constant right before the loop.
        let constant = at.checked_sub(2).and_then(|first| {
            match (&ops[first].operator, &ops[first + 1].operator) {
                (Operator::GlobalGet { global_index }, Operator::LocalSet { local_index })
                    if *local_index == counter =>
                {
                    Some(*global_index)
                }
                _ => None,
            }
        });
        let start = constant
            .and_then(|global| module.global(global))
            .filter(|global| global.constant())
            .and_then(|global| global.value);
        match start {
            None => self.violation(
                Rule::Loops,
                line,
                format!(
                    "the counter {counter_name} isn't set from a named constant right before the \
                     loop"
                ),
                fix,
            ),
            Some(count) if count < 1 || u64::try_from(count).is_ok_and(|count| count > bound) => {
                self.violation(
                    Rule::Loops,
                    line,
                    format!("the counter starts at {count}, outside 1 to the bound {bound}"),
                    "start the counter from a constant between 1 and the bound",
                );
            }
            Some(_) => {}
        }
        for (index, op) in ops.iter().enumerate().take(end).skip(at + 1) {
            let writes = matches!(
                op.operator,
                Operator::LocalSet { local_index } | Operator::LocalTee { local_index }
                    if local_index == counter
            );
            if writes && index != decrement {
                self.violation(
                    Rule::Loops,
                    op.line,
                    format!("this changes the loop counter {counter_name} inside the loop"),
                    "change the counter only in the loop's last two instructions",
                );
            }
            let repeats = shape
                .targets
                .get(&index)
                .is_some_and(|targets| targets.contains(&Some(at)));
            if repeats && index != end - 1 {
                self.violation(
                    Rule::Loops,
                    op.line,
                    "this branches back to the loop's start, and the only backward branch is the \
                     loop's own last `br_if`",
                    "leave early with `br` to a block around the loop, and repeat only through \
                     its last `br_if`",
                );
            }
        }
    }

    /// Rule 2's exception: a server's one loop that takes connections runs forever, and ends with
    /// an unconditional branch back to itself.
    fn forever(
        &mut self,
        function: &Function<'_>,
        shape: &Shape,
        at: usize,
        end: usize,
        policy: &Policy,
        forever: &mut usize,
    ) {
        let ops = &function.body;
        let line = ops[at].line;
        if !policy.listens {
            self.violation(
                Rule::Loops,
                line,
                "only a server's loop that takes connections may run forever",
                "give the loop a counter and `;; bound: N`; `;; bound: forever` is for a program \
                 whose idea says `- Network: listens on TCP port N`",
            );
            return;
        }
        *forever += 1;
        if *forever > 1 {
            self.violation(
                Rule::Loops,
                line,
                "only one loop may run forever: the one that takes connections",
                "give this loop a counter and `;; bound: N`",
            );
        }
        let closes = end
            .checked_sub(1)
            .and_then(|before| shape.targets.get(&before).map(|targets| (before, targets)))
            .is_some_and(|(before, targets)| {
                matches!(ops[before].operator, Operator::Br { .. }) && targets == &[Some(at)]
            });
        if !closes {
            self.violation(
                Rule::Loops,
                ops[end.saturating_sub(1)].line.or(line),
                "a loop that runs forever ends with `br` back to itself",
                "end the loop with `(br $name)`, so it provably never ends by itself",
            );
        }
        if !self.accepts(function, at, end) {
            self.violation(
                Rule::Loops,
                line,
                "this loop runs forever but takes no connections: nothing in it calls \
                 `sock_accept`",
                "take each connection in the loop with `sock_accept`, itself or through a \
                 function it calls; give any other loop a counter and `;; bound: N`",
            );
        }
    }

    /// Whether the loop opened at `at` and ended at `end` takes connections: its body calls
    /// `sock_accept`, itself or through main.wat's functions it calls.
    fn accepts(&self, function: &Function<'_>, at: usize, end: usize) -> bool {
        let module = self.module;
        let mut pending: Vec<u32> = function
            .body
            .iter()
            .take(end)
            .skip(at + 1)
            .filter_map(|op| match op.operator {
                Operator::Call { function_index } => Some(function_index),
                _ => None,
            })
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(index) = pending.pop() {
            if !seen.insert(index) {
                continue;
            }
            let Some(callee) = module.function(index) else {
                continue;
            };
            if callee.is_host("sock_accept") {
                return true;
            }
            if callee.kind == Kind::Own
                && let Some(shape) = self.flows.shapes.get(&index)
            {
                pending.extend(shape.calls.iter().map(|(_, callee)| *callee));
            }
        }
        false
    }

    /// Rule 1: no recursion, directly or through other functions.
    fn recursion(&mut self) {
        let module = self.module;
        let flows = self.flows;
        let own: BTreeMap<u32, Vec<(usize, u32)>> = module
            .own()
            .filter_map(|function| {
                let shape = flows.shapes.get(&function.index)?;
                Some((function.index, shape.calls.clone()))
            })
            .collect();
        let mut cycles = Vec::new();
        for (caller, calls) in &own {
            for (at, callee) in calls {
                let mut seen = BTreeSet::new();
                let mut pending = vec![*callee];
                while let Some(current) = pending.pop() {
                    if current == *caller {
                        cycles.push((*caller, *at, *callee));
                        break;
                    }
                    if seen.insert(current)
                        && let Some(next) = own.get(&current)
                    {
                        pending.extend(next.iter().map(|(_, callee)| *callee));
                    }
                }
            }
        }
        for (caller, at, callee) in cycles {
            let (Some(caller), Some(callee)) = (module.function(caller), module.function(callee))
            else {
                continue;
            };
            let line = caller.body.get(at).and_then(|op| op.line);
            let (caller_name, callee_name) = (caller.shown(), callee.shown());
            self.violation(
                Rule::ControlFlow,
                line,
                format!(
                    "{caller_name} calls {callee_name}, which leads back to {caller_name}: \
                     recursion"
                ),
                "rewrite the recursion as a counted loop",
            );
        }
    }

    /// Rule 8: `_start` ends every path in a call that never returns, so the exit status is a
    /// named constant rather than the 0 of a return.
    fn entry(&mut self) {
        let module = self.module;
        let Some(entry) = module.entry().and_then(|index| module.function(index)) else {
            return;
        };
        if entry.kind != Kind::Own {
            return;
        }
        let falls = self
            .flows
            .shapes
            .get(&entry.index)
            .is_some_and(|shape| shape.falls_through);
        if falls {
            self.violation(
                Rule::Constants,
                entry.line,
                "`_start` can reach its end, and the program then exits with code 0, which no \
                 named constant says",
                "end every path of `_start` with `(call $proc_exit (global.get $EXIT_…))` and \
                 `unreachable`",
            );
        }
    }
}

/// The counter of a loop opened at `at` and ended at `end`, and the index of its decrement's
/// `local.set` or `local.tee`, when its last instructions take one from the counter and branch
/// back while it isn't zero: `local.get $n`, `i32.const 1`, `i32.sub`, then `local.set $n`,
/// `local.get $n` and `br_if 0`, or `local.tee $n` and `br_if 0`.
fn closing(ops: &[super::model::Op<'_>], at: usize, end: usize) -> Option<(u32, usize)> {
    let operator = |back: usize| {
        let index = end.checked_sub(back)?;
        (index > at).then(|| &ops[index].operator)
    };
    if !matches!(operator(1)?, Operator::BrIf { relative_depth: 0 }) {
        return None;
    }
    let (counter, decrement, sub) = match operator(2)? {
        Operator::LocalTee { local_index } => (*local_index, end - 2, 3),
        Operator::LocalGet { local_index } => match operator(3)? {
            Operator::LocalSet { local_index: set } if set == local_index => {
                (*local_index, end - 3, 4)
            }
            _ => return None,
        },
        _ => return None,
    };
    let takes_one = matches!(operator(sub)?, Operator::I32Sub)
        && matches!(operator(sub + 1)?, Operator::I32Const { value: 1 })
        && matches!(
            operator(sub + 2)?,
            Operator::LocalGet { local_index } if *local_index == counter
        );
    takes_one.then_some((counter, decrement))
}
