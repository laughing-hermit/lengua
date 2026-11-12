//! The lengua Power of 10's rules over each function: small functions (rule 4), assertions
//! (rule 5), smallest scope (rule 6), checked results (rule 7), named constants (rule 8),
//! checked indexes (rule 9) and the lints (rule 10).

use std::collections::{BTreeMap, BTreeSet};

use wasmparser::{Operator, ValType};

use super::Facts;
use super::flow::{Flows, Shape, callee_at};
use super::memory::{Analysis, Verdict, judge};
use super::model::{Function, Kind, Module, Op};
use crate::checker::{Rule, Violation};
use crate::idea::ASSERTION_EXIT_CODE;
use crate::wasm::wasi::Bytes;

/// The most operators a function may have, its final `end` aside: the native lengua's 60
/// instructions a function, at about three operators to an instruction, as `main-template.wat`
/// measures.
pub const MAX_OPERATORS: usize = 180;

/// The fewest assertions per function, on average.
pub const MIN_ASSERTIONS_PER_FUNCTION: usize = 2;

/// The highest number a program gives an assertion: lengua's own are 901 and up.
pub const MAX_ASSERTION: i64 = 899;

/// The highest exit status.
const MAX_EXIT_STATUS: u64 = 255;

/// The checks of this module, with what they report to.
struct Checker<'m, 'a, 'v> {
    /// The module.
    module: &'m Module<'a>,
    /// Every function's control flow.
    flows: &'m Flows,
    /// Where violations go.
    violations: &'v mut Vec<Violation>,
}

/// Whether `function` is marked `;; returns: result` on its line.
#[must_use]
pub fn returns_result(module: &Module<'_>, function: &Function<'_>) -> bool {
    function
        .line
        .and_then(|line| module.notes.marker(line, "returns:"))
        .is_some_and(|rest| rest.split_whitespace().next() == Some("result"))
}

/// Whether `callee` is lengua's `$assert_fail`, or a function main.wat calls by its name.
fn is_assert_fail(callee: &Function<'_>) -> bool {
    callee.name.as_deref() == Some("assert_fail")
        && matches!(callee.kind, Kind::Library | Kind::Own | Kind::Import)
}

/// Whether `operator` compares two i32s.
fn i32_comparison(operator: &Operator<'_>) -> bool {
    matches!(
        operator,
        Operator::I32Eq
            | Operator::I32Ne
            | Operator::I32LtS
            | Operator::I32LtU
            | Operator::I32GtS
            | Operator::I32GtU
            | Operator::I32LeS
            | Operator::I32LeU
            | Operator::I32GeS
            | Operator::I32GeU
    )
}

/// The operator that put operand `operand` of the operator at `at` on the stack.
fn produced<'o, 'a>(
    ops: &'o [Op<'a>],
    shape: &Shape,
    at: usize,
    operand: usize,
) -> Option<&'o Op<'a>> {
    let producer = shape.producers.get(at)?.get(operand).copied().flatten()?;
    ops.get(producer)
}

/// The most locals followed back to the bare number one was set from.
const MAX_FOLLOWED: usize = 8;

/// The write that gives local `local` its value at the operator `at`, on every path that
/// reaches it: the nearest `local.set` or `local.tee` of it before `at`, with nothing between
/// them but code that runs straight on and blocks skipped whole that don't write it. `None`
/// when there is none, as at a loop's start, which its end comes back to.
fn defined(ops: &[Op<'_>], shape: &Shape, at: usize, local: u32) -> Option<usize> {
    let writes = |index: usize| {
        matches!(
            ops[index].operator,
            Operator::LocalSet { local_index } | Operator::LocalTee { local_index }
                if local_index == local
        )
    };
    let mut index = at;
    while index > 0 {
        index -= 1;
        match ops[index].operator {
            _ if writes(index) => return Some(index),
            // A block that ends before: skipped whole, when it doesn't write the local.
            Operator::End => {
                let opener = *shape.openers.get(&index)?;
                if (opener..index).any(writes) {
                    return None;
                }
                index = opener;
            }
            // The start of an `else`: control came from its `if`.
            Operator::Else => index = *shape.openers.get(&index)?,
            Operator::Loop { .. } => return None,
            _ => {}
        }
    }
    None
}

/// The bare number the value the operator at `producer` gives is: an `i32.const` or an
/// `i64.const`, itself or kept in locals set from one right before.
fn literal_of(ops: &[Op<'_>], shape: &Shape, producer: usize) -> Option<i64> {
    let mut producer = producer;
    for _ in 0..MAX_FOLLOWED {
        match ops.get(producer)?.operator {
            Operator::I32Const { value } => return Some(i64::from(value)),
            Operator::I64Const { value } => return Some(value),
            Operator::LocalGet { local_index } => {
                let set = defined(ops, shape, producer, local_index)?;
                producer = shape.producers.get(set)?.first().copied().flatten()?;
            }
            _ => return None,
        }
    }
    None
}

/// The value of operand `operand` of the operator at `at`, when it's a bare constant, written
/// there or kept in a local right before.
fn bare(ops: &[Op<'_>], shape: &Shape, at: usize, operand: usize) -> Option<i64> {
    let producer = shape.producers.get(at)?.get(operand).copied().flatten()?;
    literal_of(ops, shape, producer)
}

/// Whether a call of `callee` gives an error number: a host call's, lengua's `$fill` and
/// `$write_all`, or main.wat's own function marked `;; returns: result`.
fn gives_error(module: &Module<'_>, callee: &Function<'_>) -> bool {
    match callee.kind {
        // A function of a module the idea calls gives a number of its own, not an error number.
        Kind::Import | Kind::LibraryImport => {
            callee.host().is_some() && callee.results == [ValType::I32]
        }
        Kind::Library => callee.is_library("fill") || callee.is_library("write_all"),
        Kind::Own => returns_result(module, callee),
    }
}

/// Whether operand `operand` of the operator at `at` of `function` is an error number: what a
/// call that gives one gave, itself or kept in a local right before.
fn error_number(
    module: &Module<'_>,
    function: &Function<'_>,
    shape: &Shape,
    at: usize,
    operand: usize,
) -> bool {
    let ops = &function.body;
    let mut producer = shape
        .producers
        .get(at)
        .and_then(|taken| taken.get(operand).copied());
    for _ in 0..MAX_FOLLOWED {
        let Some(index) = producer.flatten() else {
            return false;
        };
        match ops[index].operator {
            Operator::Call { .. } => {
                return callee_at(module, function, index)
                    .is_some_and(|callee| gives_error(module, callee));
            }
            Operator::LocalGet { local_index } => {
                producer = defined(ops, shape, index, local_index)
                    .and_then(|set| shape.producers.get(set)?.first().copied());
            }
            _ => return false,
        }
    }
    false
}

/// The arguments of a host function that are modes or flags, which are named like the rest.
fn modes(host: &str) -> &'static [usize] {
    match host {
        "path_open" => &[1, 4, 5, 6, 7],
        "path_filestat_get" | "sock_accept" | "sock_shutdown" => &[1],
        "fd_seek" => &[2],
        "clock_res_get" | "clock_time_get" => &[0],
        "sock_recv" | "sock_send" | "tcp_open" => &[3],
        "file_open" => &[1],
        "term_raw" => &[0],
        "split" => &[2],
        _ => &[],
    }
}

/// Whether the result of the call at `at` is tested by a branch before anything else uses it:
/// kept in a local and read back, or teed, then perhaps compared, then a `br_if` or an `if`.
fn tested(ops: &[Op<'_>], at: usize) -> Result<(), &'static str> {
    let operator = |index: usize| ops.get(index).map(|op| &op.operator);
    let mut next = at + 1;
    match operator(next) {
        Some(Operator::LocalSet { local_index }) => {
            let read_back = matches!(
                operator(next + 1),
                Some(Operator::LocalGet { local_index: read }) if read == local_index
            );
            if !read_back {
                return Err("it's kept in a local, and something else comes before the test");
            }
            next += 2;
        }
        Some(Operator::LocalTee { .. }) => next += 1,
        Some(Operator::Drop) => return Err("it's dropped"),
        None => return Err("nothing tests it"),
        _ => {}
    }
    match operator(next) {
        Some(Operator::I32Eqz) => next += 1,
        Some(Operator::I32Const { .. } | Operator::GlobalGet { .. })
            if operator(next + 1).is_some_and(i32_comparison) =>
        {
            next += 2;
        }
        _ => {}
    }
    match operator(next) {
        Some(Operator::BrIf { .. } | Operator::If { .. }) => Ok(()),
        _ => Err("something uses it before a branch tests it"),
    }
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

    /// Rule 4: at most [`MAX_OPERATORS`] operators.
    fn size(&mut self, function: &Function<'_>) {
        let count = function.body.len().saturating_sub(1);
        if count > MAX_OPERATORS {
            self.violation(
                Rule::Functions,
                function.line,
                format!("{} has {count} operators", function.shown()),
                format!(
                    "split it into functions of at most {MAX_OPERATORS} operators: a loop, or the \
                     code of one block, can be a function of its own"
                ),
            );
        }
    }

    /// Rule 5: each assertion is `(call $assert_fail (i32.const N))` with N from 1 to 899, unique,
    /// reached only when a check fails. Gives how many `function` has.
    fn assertions(
        &mut self,
        function: &Function<'_>,
        shape: &Shape,
        numbers: &mut BTreeMap<i64, usize>,
    ) -> usize {
        let module = self.module;
        let ops = &function.body;
        let shape_fix = "write an assertion as `(call $assert_fail (i32.const N))` and \
                         `unreachable`, with N from 1 to 899, in the `then` of an `if` whose \
                         condition is the check, or right after a `br_if` that leaves when the \
                         check holds";
        let mut count = 0;
        for (at, op) in ops.iter().enumerate() {
            if !callee_at(module, function, at).is_some_and(is_assert_fail) {
                continue;
            }
            let number = at
                .checked_sub(1)
                .and_then(|before| match ops[before].operator {
                    Operator::I32Const { value } => Some(i64::from(value)),
                    _ => None,
                });
            let Some(number) = number else {
                self.violation(
                    Rule::Assertions,
                    op.line,
                    "this call to $assert_fail has no constant number",
                    shape_fix,
                );
                continue;
            };
            let line = ops[at - 1].line.or(op.line);
            if !(1..=MAX_ASSERTION).contains(&number) {
                let problem = if (MAX_ASSERTION + 1..=999).contains(&number) {
                    format!("assertion number {number} is one lengua keeps for its own code")
                } else {
                    format!("assertion number {number} is outside 1 to {MAX_ASSERTION}")
                };
                self.violation(
                    Rule::Assertions,
                    line,
                    problem,
                    format!("number your assertions from 1 to {MAX_ASSERTION}"),
                );
                continue;
            }
            if let Some(first) = line.and_then(|line| numbers.insert(number, line)) {
                self.violation(
                    Rule::Assertions,
                    line,
                    format!("assertion number {number} is used twice (also on line {first})"),
                    "number each assertion uniquely, from 1 to 899",
                );
            }
            // Reached only when a check fails: in the `then` of an `if`, right after a `br_if`,
            // when a counted loop runs out, or as a `br_table`'s default.
            let before = at.checked_sub(2).map(|index| &ops[index].operator);
            let ended = at.checked_sub(2).and_then(|end| {
                let opener = *shape.openers.get(&end)?;
                matches!(ops[end].operator, Operator::End).then_some((end, opener))
            });
            let after_loop = ended.is_some_and(|(end, opener)| {
                matches!(ops[opener].operator, Operator::Loop { .. })
                    && end.checked_sub(1).is_some_and(|last| {
                        matches!(ops[last].operator, Operator::BrIf { relative_depth: 0 })
                    })
            });
            // The block a `br_table`'s default leaves, which control can't otherwise fall out of.
            let after_default = ended.is_some_and(|(end, opener)| {
                let defaulted = shape.targets.iter().any(|(index, targets)| {
                    matches!(ops[*index].operator, Operator::BrTable { .. })
                        && targets.last() == Some(&Some(opener))
                });
                let falls_out = end.checked_sub(1).is_none_or(|last| {
                    !matches!(
                        ops[last].operator,
                        Operator::Br { .. } | Operator::BrTable { .. } | Operator::Unreachable
                    )
                });
                defaulted && !falls_out
            });
            let checked = matches!(before, Some(Operator::If { .. } | Operator::BrIf { .. }))
                || after_loop
                || after_default;
            if checked {
                count += 1;
            } else {
                self.violation(
                    Rule::Assertions,
                    line,
                    format!("assertion {number} isn't reached only when a check fails"),
                    shape_fix,
                );
            }
        }
        count
    }

    /// Rule 6: a global that changes says why it's shared, and a writable region is one
    /// function's unless it says why it's shared.
    fn scope(&mut self) {
        let module = self.module;
        let reason_fix = "say after `;; shared:` why several functions keep it";
        for global in module
            .globals
            .iter()
            .filter(|global| global.mutable && !global.imported)
        {
            match global
                .line
                .and_then(|line| module.notes.marker(line, "shared:"))
            {
                None => self.violation(
                    Rule::Scope,
                    global.line,
                    format!(
                        "{} changes, but its line has no `;; shared: <reason>`",
                        global.shown()
                    ),
                    "keep values in locals; a global that changes holds only what the program \
                     keeps across calls, with `;; shared: <reason>` on its line",
                ),
                Some("") => self.violation(
                    Rule::Scope,
                    global.line,
                    "`;; shared:` needs a reason",
                    reason_fix,
                ),
                Some(_) => {}
            }
        }
        for region in module.regions.iter().filter(|region| !region.read_only) {
            if region.shared.as_deref() == Some("") {
                self.violation(
                    Rule::Scope,
                    region.line,
                    "`;; shared:` needs a reason",
                    reason_fix,
                );
                continue;
            }
            if region.shared.is_some() {
                continue;
            }
            let users: Vec<String> = module
                .own()
                .filter(|function| {
                    function.body.iter().any(|op| {
                        matches!(
                            op.operator,
                            Operator::GlobalGet { global_index } if global_index == region.global
                        )
                    })
                })
                .map(Function::shown)
                .collect();
            if users.len() > 1 {
                self.violation(
                    Rule::Scope,
                    region.line,
                    format!("${} is used by {}", region.name, users.join(", ")),
                    "let one function own it and lend it to the others as an address and a \
                     length, or mark it `;; shared: <reason>` on its line",
                );
            }
        }
    }

    /// Rule 7: every call whose result is an error number, or a function's marked `;; returns:
    /// result`, is tested by a branch before anything else uses its result.
    fn results(&mut self, function: &Function<'_>) {
        let module = self.module;
        let ops = &function.body;
        let marked = returns_result(module, function);
        if marked && function.results != [ValType::I32] {
            self.violation(
                Rule::Results,
                function.line,
                format!(
                    "{} is marked `;; returns: result` but doesn't give one i32",
                    function.shown()
                ),
                "give one i32: 0 for success, or an error number",
            );
        }
        for (at, op) in ops.iter().enumerate() {
            let Some(callee) = callee_at(module, function, at) else {
                continue;
            };
            if !gives_error(module, callee) {
                continue;
            }
            // Its result is the caller's own, which the caller's callers test.
            let passed_up = marked && at + 2 == ops.len();
            if passed_up {
                continue;
            }
            if let Err(reason) = tested(ops, at) {
                let what = if callee.kind == Kind::Own {
                    "result"
                } else {
                    "error number"
                };
                self.violation(
                    Rule::Results,
                    op.line,
                    format!("{}'s {what} isn't tested: {reason}", callee.shown()),
                    "test it before anything else: `(local.set $err (call …))`, then `(br_if \
                     $failed (local.get $err))`",
                );
            }
        }
    }

    /// Rule 8: exit statuses, descriptors, modes, sizes, bounds and error numbers are named
    /// constants, not bare numbers, nor bare numbers kept in a local right before.
    fn constants(&mut self, function: &Function<'_>, shape: &Shape) {
        let module = self.module;
        let ops = &function.body;
        let literal = |at: usize, operand: usize| bare(ops, shape, at, operand);
        for (at, op) in ops.iter().enumerate() {
            if let Some(callee) = callee_at(module, function, at) {
                if callee.is_host("proc_exit") {
                    self.exit_status(op, literal(at, 0), produced(ops, shape, at, 0));
                }
                let descriptor = callee.host().is_some()
                    && callee.import.as_ref().is_some_and(|(_, field)| {
                        field.starts_with("fd_")
                            || field.starts_with("sock_")
                            || field.starts_with("path_")
                            || field.starts_with("udp_send")
                            || field.starts_with("udp_recv")
                            || field == "term_is"
                    })
                    || callee.is_library("fill")
                    || callee.is_library("write_all");
                if descriptor && let Some(value) = literal(at, 0) {
                    self.violation(
                        Rule::Constants,
                        op.line,
                        format!("descriptor {value} is a bare number"),
                        "name it: `(global $STDOUT i32 (i32.const 1))`, then `(global.get \
                         $STDOUT)`",
                    );
                }
                let lengths: Vec<usize> = match callee.kind {
                    Kind::Import | Kind::LibraryImport => callee
                        .host()
                        .map(|host| {
                            host.lends
                                .iter()
                                .filter_map(|lend| match lend.bytes {
                                    Bytes::Argument(length) => Some(length),
                                    _ => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    Kind::Library
                        if callee.is_library("fill") || callee.is_library("write_all") =>
                    {
                        vec![2]
                    }
                    _ => Vec::new(),
                };
                for length in lengths {
                    if let Some(value) = literal(at, length) {
                        self.violation(
                            Rule::Constants,
                            op.line,
                            format!("the size {value} is a bare number"),
                            "name it: a region's `$x.size`, or a constant such as `(global \
                             $LINE_LEN i32 (i32.const 80))`",
                        );
                    }
                }
                let host = callee.host().map_or("", |host| host.name);
                for mode in modes(host) {
                    if let Some(value) = literal(at, *mode)
                        && value != 0
                    {
                        self.violation(
                            Rule::Constants,
                            op.line,
                            format!("the mode {value} is a bare number"),
                            format!(
                                "name it after what it means: `(global $… i32 (i32.const \
                                 {value}))`"
                            ),
                        );
                    }
                }
                continue;
            }
            let ordered = matches!(
                op.operator,
                Operator::I32LtU
                    | Operator::I32LeU
                    | Operator::I32GtU
                    | Operator::I32GeU
                    | Operator::I64LtU
                    | Operator::I64LeU
                    | Operator::I64GtU
                    | Operator::I64GeU
                    | Operator::I32LtS
                    | Operator::I32LeS
                    | Operator::I32GtS
                    | Operator::I32GeS
                    | Operator::I64LtS
                    | Operator::I64LeS
                    | Operator::I64GtS
                    | Operator::I64GeS
            );
            let equality = matches!(
                op.operator,
                Operator::I32Eq | Operator::I32Ne | Operator::I64Eq | Operator::I64Ne
            );
            for (local, other) in [(0, 1), (1, 0)] {
                let Some(value) = literal(at, other) else {
                    continue;
                };
                let reads_local = produced(ops, shape, at, local)
                    .is_some_and(|producer| matches!(producer.operator, Operator::LocalGet { .. }));
                // Only 0 and 1 may be bare in a bound: -1 is a number like any other.
                if ordered && reads_local && !(0..=1).contains(&value) {
                    self.violation(
                        Rule::Constants,
                        op.line,
                        format!("the bound {value} is a bare number"),
                        format!(
                            "name it: `(global $MAX_… i32 (i32.const {value}))`, or a region's \
                             `$x.size`"
                        ),
                    );
                }
                if equality && value != 0 && error_number(module, function, shape, at, local) {
                    self.violation(
                        Rule::Constants,
                        op.line,
                        format!("the error number {value} is a bare number"),
                        format!(
                            "name it as references/host.md does: `(global $… i32 (i32.const \
                             {value}))`"
                        ),
                    );
                }
            }
        }
        // What a function marked `;; returns: result` gives is a named error number too.
        if returns_result(module, function)
            && let Some(last) = ops.len().checked_sub(2)
            && let Some(value) = literal_of(ops, shape, last)
            && value != 0
        {
            self.violation(
                Rule::Constants,
                ops[last].line,
                format!("{} gives the bare error number {value}", function.shown()),
                format!("name it: `(global $ERROR_… i32 (i32.const {value}))`"),
            );
        }
    }

    /// Rule 8: the status of `proc_exit`'s call `op`, bare or from `producer`, is a named
    /// constant from 0 to 255, never 134.
    fn exit_status(&mut self, op: &Op<'_>, bare: Option<i64>, producer: Option<&Op<'_>>) {
        let module = self.module;
        let fix = "name the status: `(global $EXIT_… i32 (i32.const <0-255>))`, then `(call \
                   $proc_exit (global.get $EXIT_…))`";
        if let Some(value) = bare {
            self.violation(
                Rule::Constants,
                op.line,
                format!("the exit status is the bare number {value}"),
                fix,
            );
            return;
        }
        let value = match producer.map(|producer| &producer.operator) {
            Some(Operator::GlobalGet { global_index }) => module
                .global(*global_index)
                .filter(|global| global.constant())
                .and_then(super::model::Global::unsigned),
            _ => None,
        };
        match value {
            None => self.violation(
                Rule::Constants,
                op.line,
                "the exit status isn't a named constant",
                fix,
            ),
            Some(code) if code == u64::from(ASSERTION_EXIT_CODE) => self.violation(
                Rule::Constants,
                op.line,
                format!("{code} is the exit status of a failed assertion"),
                "exit with another status; 134 is lengua's for a failed assertion",
            ),
            Some(code) if code > MAX_EXIT_STATUS => self.violation(
                Rule::Constants,
                op.line,
                format!("the exit status {code} is past 255"),
                "exit with a status from 0 to 255",
            ),
            Some(_) => {}
        }
    }

    /// Rule 8: a read-only region's `$x.rosize` matches the data it holds.
    fn sizes(&mut self) {
        let module = self.module;
        for region in module.regions.iter().filter(|region| region.read_only) {
            let held: u64 = module
                .data
                .iter()
                .filter(|data| data.own)
                .filter_map(|data| {
                    let (_, start) = data.at?;
                    let start = start?;
                    (start >= region.start && start.saturating_add(data.len) <= region.end())
                        .then_some(data.len)
                })
                .sum();
            if held != region.size {
                let line = module
                    .global(region.size_global)
                    .and_then(|global| global.line)
                    .or(region.line);
                let name = &region.name;
                self.violation(
                    Rule::Constants,
                    line,
                    format!(
                        "${name}.rosize is {}, but the data in ${name} holds {held} bytes",
                        region.size
                    ),
                    format!("set `${name}.rosize` to {held}, the bytes its data holds"),
                );
            }
        }
    }

    /// Rule 9: an access with a computed index is preceded by an unsigned check that keeps every
    /// byte inside its region; and the comments marking unsafe blocks are well formed.
    fn indexes(&mut self, analysis: &Analysis) {
        let module = self.module;
        for (index, followed) in &analysis.followed {
            let Some(function) = module.function(*index) else {
                continue;
            };
            for access in &followed.accesses {
                for verdict in judge(module, function, access) {
                    match verdict {
                        Verdict::Unchecked(problem) => self.violation(
                            Rule::Indirection,
                            access.line,
                            problem,
                            "check the index first, unsigned, against a limit that keeps every \
                             byte inside the region, `(br_if $bad (i32.ge_u (local.get $i) \
                             (global.get $MAX)))`; or mark a provably safe access `;; unsafe: \
                             <reason>` … `;; end unsafe`",
                        ),
                        Verdict::Loose(problem) => self.violation(
                            Rule::Indirection,
                            access.line,
                            problem,
                            "check against a limit that keeps every byte of the access inside its \
                             region: for a w-byte access to an n-byte region the index stays \
                             below n - w + 1",
                        ),
                        _ => {}
                    }
                }
            }
        }
        for (line, problem, fix) in &module.unsafe_marks {
            self.violation(Rule::Indirection, Some(*line), problem.clone(), fix.clone());
        }
        let ranges: Vec<(usize, usize)> = module
            .own()
            .filter_map(|function| {
                let first = function.line?;
                let last = function
                    .body
                    .iter()
                    .filter_map(|op| op.line)
                    .max()
                    .unwrap_or(first);
                Some((first, last))
            })
            .collect();
        for block in &module.unsafe_blocks {
            let inside = ranges
                .iter()
                .any(|(first, last)| (*first..=*last).contains(&block.start));
            // Another of main.wat's functions starts before the block ends.
            let crosses = ranges
                .iter()
                .any(|(first, _)| block.start < *first && *first <= block.end);
            if !inside {
                self.violation(
                    Rule::Indirection,
                    Some(block.start),
                    "`;; unsafe:` is outside a function",
                    "put unsafe blocks around accesses inside a function",
                );
            } else if crosses {
                self.violation(
                    Rule::Indirection,
                    Some(block.start),
                    "this `;; unsafe:` block runs past the end of its function, into another",
                    "end the block with `;; end unsafe` inside the function it starts in",
                );
            }
        }
    }

    /// Rule 10: the lints, with no warnings: every function called, every import used, every
    /// constant and local read, and no code that never runs.
    fn lints(&mut self) {
        let module = self.module;
        let flows = self.flows;
        let fix_remove = "remove it, or use it";
        // The functions `_start` reaches.
        let mut reached = BTreeSet::new();
        // And the functions it exports for other programs, which an idea's `- Exports:` line
        // lists: another program calls them, whether or not `_start` does.
        let mut pending: Vec<u32> = module
            .exports
            .iter()
            .filter(|export| export.kind == wasmparser::ExternalKind::Func)
            .map(|export| export.index)
            .collect();
        while let Some(index) = pending.pop() {
            if !reached.insert(index) {
                continue;
            }
            if let Some(shape) = flows.shapes.get(&index) {
                pending.extend(shape.calls.iter().map(|(_, callee)| *callee));
            }
        }
        for function in &module.functions {
            match function.kind {
                Kind::Own if !reached.contains(&function.index) => self.violation(
                    Rule::Tools,
                    function.line,
                    format!("{} is never called", function.shown()),
                    fix_remove,
                ),
                Kind::Import if !reached.contains(&function.index) => self.violation(
                    Rule::Tools,
                    function.line,
                    format!("main.wat imports {} but never calls it", function.shown()),
                    fix_remove,
                ),
                _ => {}
            }
        }
        let read: BTreeSet<u32> = module
            .own()
            .flat_map(|function| function.body.iter())
            .filter_map(|op| match op.operator {
                Operator::GlobalGet { global_index } => Some(global_index),
                _ => None,
            })
            .collect();
        for global in &module.globals {
            let part_of_region =
                module.region_of(global.index).is_some() || module.sized_by(global.index).is_some();
            if !global.imported && !part_of_region && !read.contains(&global.index) {
                self.violation(
                    Rule::Tools,
                    global.line,
                    format!("{} is never read", global.shown()),
                    fix_remove,
                );
            }
        }
        for function in module.own() {
            let read: BTreeSet<u32> = function
                .body
                .iter()
                .filter_map(|op| match op.operator {
                    Operator::LocalGet { local_index } => Some(local_index),
                    _ => None,
                })
                .collect();
            let unread: Vec<String> = (0..function.locals.len())
                .map(|position| super::model::to_u32(function.params.len() + position))
                .filter(|local| !read.contains(local))
                .map(|local| function.local_shown(local))
                .collect();
            if let Some(first) = unread.first() {
                let what = match unread.len() {
                    1 => format!("local {first} is"),
                    count => format!("locals {first} and {} more are", count - 1),
                };
                self.violation(
                    Rule::Tools,
                    function.line,
                    format!("{}'s {what} never read", function.shown()),
                    fix_remove,
                );
            }
            let Some(shape) = flows.shapes.get(&function.index) else {
                continue;
            };
            for at in &shape.dead {
                self.violation(
                    Rule::Tools,
                    function.body.get(*at).and_then(|op| op.line),
                    "this code never runs: nothing reaches it",
                    "remove it",
                );
            }
        }
    }
}

/// Checks rules 4 to 10, and counts the assertions.
pub fn check(
    module: &Module<'_>,
    flows: &Flows,
    analysis: &Analysis,
    violations: &mut Vec<Violation>,
    facts: &mut Facts,
) {
    let mut checker = Checker {
        module,
        flows,
        violations,
    };
    let mut numbers = BTreeMap::new();
    let mut assertions = 0;
    let mut functions = 0;
    for function in module.own() {
        functions += 1;
        checker.size(function);
        checker.results(function);
        if let Some(shape) = flows.shapes.get(&function.index) {
            assertions += checker.assertions(function, shape, &mut numbers);
            checker.constants(function, shape);
        }
    }
    if assertions < MIN_ASSERTIONS_PER_FUNCTION * functions {
        let line = module.own().next().and_then(|function| function.line);
        checker.violation(
            Rule::Assertions,
            line,
            format!(
                "main.wat has {assertions} assertions in {functions} functions; the dialect needs \
                 at least {MIN_ASSERTIONS_PER_FUNCTION} per function on average"
            ),
            "add assertions that check what must be true, such as indexes in range or results \
             the code relies on",
        );
    }
    checker.scope();
    checker.sizes();
    checker.indexes(analysis);
    checker.lints();
    facts.assertions = assertions;
}
