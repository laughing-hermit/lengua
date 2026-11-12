//! The rules of work split into pieces ([`crate::split`]), for an idea with a `- Splits:` line,
//! as far as main.wat alone shows them: the program and the most pieces its idea grants, each
//! with a memory of the module's size, take no more memory together than one program may; each
//! call splits into a named number of pieces, which the idea's line covers; and a module that
//! splits asks whether it is a piece, since a piece is a run of the same module. That a module
//! without the line imports neither function is rule 3's, with every other import. What a call
//! does while the program runs, the host holds to the line.

use wasmparser::Operator;

use super::flow::{Flows, callee_at};
use super::model::{Function, Kind, MAX_PAGES, Module};
use crate::checker::{Rule, Violation};
use crate::split::Grant;
use crate::wasm::Policy;
use crate::wasm::wasi::SPLIT_MODULE;

/// The argument of `split` that holds how many pieces, counted from 0.
const PIECES: usize = 2;

/// Whether `function` is main.wat's import of `name` from lengua's module for split work.
fn imports(function: &Function<'_>, name: &str) -> bool {
    function.kind == Kind::Import
        && function
            .import
            .as_ref()
            .is_some_and(|(module, field)| module == SPLIT_MODULE && field == name)
}

/// The constant the operand `operand` of the operator at `at` is, when a named constant put it
/// there: a `global.get` of an immutable global set to a number.
fn named_constant(
    module: &Module<'_>,
    function: &Function<'_>,
    flows: &Flows,
    at: usize,
    operand: usize,
) -> Option<u64> {
    let shape = flows.shapes.get(&function.index)?;
    let producer = shape.producers.get(at)?.get(operand).copied().flatten()?;
    match function.body.get(producer)?.operator {
        Operator::GlobalGet { global_index } => module.global(global_index)?.unsigned(),
        _ => None,
    }
}

/// Checks main.wat's use of split work against what `policy`'s idea grants.
pub fn check(module: &Module<'_>, flows: &Flows, policy: &Policy, violations: &mut Vec<Violation>) {
    let Some(grant) = policy.reach.get().split else {
        return;
    };
    let Some(split) = module
        .functions
        .iter()
        .find(|function| imports(function, "split"))
    else {
        return;
    };
    if !module
        .functions
        .iter()
        .any(|function| imports(function, "piece"))
    {
        violations.push(Violation {
            rule: Rule::Resources,
            line: split.line,
            problem: "main.wat splits its work and never asks whether it is a piece: each piece \
                      is a run of this same module from `_start`, which would split again and \
                      be refused"
                .to_owned(),
            fix: "import `piece` from `lengua_split_1` and call it first in `_start`: where it \
                  gives 0, the run is a piece, which reads its bytes from standard input and \
                  prints its answer"
                .to_owned(),
        });
    }
    memory(module, &grant, violations);
    for function in module.own() {
        for (at, op) in function.body.iter().enumerate() {
            if !callee_at(module, function, at).is_some_and(|callee| callee.index == split.index) {
                continue;
            }
            let pieces = named_constant(module, function, flows, at, PIECES);
            if pieces.is_some_and(|pieces| (1..=u64::from(grant.most)).contains(&pieces)) {
                continue;
            }
            let problem = match pieces {
                Some(pieces) => format!(
                    "this call splits its work into {pieces} pieces, and the idea's `- Splits:` \
                     line grants 1 to {}",
                    grant.most
                ),
                None => "this call splits its work into a number of pieces that isn't a named \
                         constant, so the rules can't hold it to the idea's `- Splits:` line"
                    .to_owned(),
            };
            violations.push(Violation {
                rule: Rule::Resources,
                line: op.line,
                problem,
                fix: format!(
                    "give the number of pieces as a named constant of 1 to {0}, such as `(global \
                     $PIECES i32 (i32.const {0}))`, or change the idea's line and tell the user",
                    grant.most
                ),
            });
        }
    }
}

/// The program and its pieces, each with a memory of the module's size, hold at most
/// [`MAX_PAGES`] together when every piece the idea grants runs at once.
fn memory(module: &Module<'_>, grant: &Grant, violations: &mut Vec<Violation>) {
    let Some(memory) = module.memories.iter().find(|memory| !memory.imported) else {
        return;
    };
    let runs = u64::from(grant.most) + 1;
    let pages = memory.ty.initial;
    if pages.saturating_mul(runs) <= MAX_PAGES {
        return;
    }
    violations.push(Violation {
        rule: Rule::Resources,
        line: memory.line,
        problem: format!(
            "the memory has {pages} pages, and the idea's `- Splits:` line grants {} pieces, each \
             a run with a memory of its own: with the program's that is {} pages, and a program \
             holds at most {MAX_PAGES} pages, 1 GiB, with its pieces",
            grant.most,
            pages.saturating_mul(runs)
        ),
        fix: format!(
            "declare at most {} pages, or grant fewer pieces in idea.md and tell the user",
            MAX_PAGES / runs
        ),
    });
}
