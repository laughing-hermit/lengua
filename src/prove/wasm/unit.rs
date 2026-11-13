//! One function of a module as a unit: the rules a function keeps for a proof of it alone to
//! hold at every call ([`Module::unit_problem`]), the state such a proof's runs start in
//! ([`begin`]) and what a run of it gave ([`end`]), and a call that takes what the function's
//! contract says it gives in place of following its body ([`reuse`]). The claims themselves, and
//! their proofs, are [`crate::prove::contract`]'s.
//!
//! The rules, each read off the function's own operators and those of the functions it calls:
//! - no store, no bulk memory operator, no `memory.grow`, no `global.set`, and no read of a
//!   global that changes: it changes nothing a later run could see, and reads nothing an
//!   earlier one could have changed;
//! - no call of the host, and none of lengua's library but `$assert_fail`, where it stops the
//!   program: a unit's run ends there, and the proof must show no argument in range reaches it;
//! - an address parameter is only ever the address of a load, with numbers added to it. It is
//!   never compared, stored, copied to a local, passed on, branched on or given back, and never
//!   crosses a block's edge on the stack. So nothing the function computes depends on where the
//!   bytes stand, only on the bytes: a proof with them at one place holds with them at any;
//! - every load is at such an address, and a function it calls takes no address and reads no
//!   memory.
//!
//! A unit's run reads only inside the bytes its claim says each address holds: a load outside
//! them ends the proof ([`OUTSIDE_ITS_TEXT`]), so the formula's bytes are all the memory the
//! function's result can depend on.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::image::{Binary, Module, Op, PAGE};
use super::{Frame, Wasm, float, function_label, memory, vector};
use crate::prove::contract::{self, Claim, Role};
use crate::prove::exec::{End, State};
use crate::prove::term::{Pool, Term};

/// Where the first text of a unit's proof stands in memory: past the first bytes, which
/// lengua's library keeps.
pub(in crate::prove) const TEXTS_FROM: u64 = 4096;

/// The bytes between two texts of a unit's proof, which no text holds.
pub(in crate::prove) const GAP: u64 = 64;

/// What a unit's run ends on at an assertion of its function.
pub(super) const STOPS: &str = "an assertion of the function stops the program";

/// What a unit's run ends on at a load outside the bytes its claim names.
pub(super) const OUTSIDE_ITS_TEXT: &str =
    "the function reads outside the bytes its contract says an address holds";

/// How deep a unit's function may call: the dialect has no recursion, so this only bounds a
/// module the rules would have refused.
const MAX_CALLS: u32 = 64;

/// A claim a module's proofs take at the calls of its function, with how often they did.
#[derive(Debug)]
pub(super) struct Held {
    /// The claim.
    claim: Claim,
    /// The calls that took the formula's value.
    used: AtomicUsize,
    /// The calls that followed the function's body, their arguments not known to be in the
    /// claim's ranges.
    followed: AtomicUsize,
}

/// Takes `count` values off `stack`, an account of which of a function's operands are
/// addresses: whether any was one. Below what the account holds, no value is an address.
fn pop(stack: &mut Vec<bool>, count: usize) -> bool {
    let mut any = false;
    for _ in 0..count {
        any |= stack.pop().unwrap_or(false);
    }
    any
}

impl Module {
    /// main.wat's own function named `name`, without the `$`, by index.
    pub(in crate::prove) fn own_function(&self, name: &str) -> Option<u32> {
        (0_u32..)
            .zip(&self.functions)
            .find_map(|(index, function)| {
                (function.own
                    && function.import.is_none()
                    && function.name.as_deref() == Some(name))
                .then_some(index)
            })
    }

    /// Why a proof of the function `index` alone wouldn't hold `claim` at every call of it, or
    /// `None` when it would: the function isn't main.wat's own, doesn't take the parameters the
    /// claim names or give one value, or breaks one of a unit's rules, which this module's own
    /// words state.
    pub(in crate::prove) fn unit_problem(&self, index: u32, claim: &Claim) -> Option<String> {
        let Some(function) = self.function(index) else {
            return Some("the module has no such function".into());
        };
        if !function.own || function.import.is_some() {
            return Some("the function isn't one of main.wat's own".into());
        }
        if function.params != claim.params.len() || function.results != 1 {
            return Some(
                "the function's parameters and result aren't the ones its contract names".into(),
            );
        }
        self.straight(index, &claim.pointers(), 0)
    }

    /// Why the function `index`, whose parameters at the places `pointers` are addresses, breaks
    /// one of a unit's rules, with the functions it calls, `depth` calls down.
    #[allow(clippy::too_many_lines)]
    fn straight(&self, index: u32, pointers: &[usize], depth: u32) -> Option<String> {
        let Some(function) = self.function(index) else {
            return Some("it calls a function the module doesn't have".into());
        };
        let name = function
            .name
            .clone()
            .unwrap_or_else(|| format!("function {index}"));
        let refuse = |what: &str| {
            Some(format!(
                "${name} {what}, which a proof of a function alone doesn't take"
            ))
        };
        if depth > MAX_CALLS {
            return refuse("calls too deep");
        }
        let is_pointer =
            |local: u32| usize::try_from(local).is_ok_and(|local| pointers.contains(&local));
        // Which of the operands are addresses, since the last block's edge.
        let mut stack: Vec<bool> = Vec::new();
        for op in &function.ops {
            match op {
                Op::Nop => {}
                Op::Unreachable
                | Op::Block(_)
                | Op::Loop(_)
                | Op::Else
                | Op::End
                | Op::Br(_)
                | Op::Return => {
                    if stack.contains(&true) {
                        return refuse("keeps an address on the stack across a block's edge");
                    }
                }
                Op::If(_) | Op::BrIf(_) | Op::BrTable(..) => {
                    if pop(&mut stack, 1) || stack.contains(&true) {
                        return refuse(
                            "branches on an address, or keeps one on the stack across a block's \
                             edge",
                        );
                    }
                }
                Op::Call(callee) => {
                    let Some(called) = self.function(*callee) else {
                        return refuse("calls a function the module doesn't have");
                    };
                    if pop(&mut stack, called.params) {
                        return refuse("passes an address on to another function");
                    }
                    let assertion = !called.own
                        && called.import.is_none()
                        && called.name.as_deref() == Some("assert_fail");
                    if !assertion {
                        if !called.own || called.import.is_some() {
                            return refuse("calls the host or lengua's library");
                        }
                        if let Some(problem) = self.straight(*callee, &[], depth + 1) {
                            return Some(problem);
                        }
                    }
                    stack.extend(std::iter::repeat_n(false, called.results));
                }
                Op::Drop => {
                    pop(&mut stack, 1);
                }
                Op::Select => {
                    if pop(&mut stack, 3) {
                        return refuse("chooses between addresses");
                    }
                    stack.push(false);
                }
                Op::LocalGet(local) => stack.push(is_pointer(*local)),
                Op::LocalSet(local) => {
                    if pop(&mut stack, 1) || is_pointer(*local) {
                        return refuse("copies or changes an address");
                    }
                }
                Op::LocalTee(local) => {
                    if stack.last().copied().unwrap_or(false) || is_pointer(*local) {
                        return refuse("copies or changes an address");
                    }
                }
                Op::GlobalGet(global) => {
                    let changes = usize::try_from(*global)
                        .ok()
                        .and_then(|global| self.mutable.get(global))
                        .copied()
                        .unwrap_or(true);
                    if changes {
                        return refuse("reads a global that changes");
                    }
                    stack.push(false);
                }
                Op::GlobalSet(_) => return refuse("sets a global"),
                Op::Load { .. } => {
                    if !pop(&mut stack, 1) {
                        return refuse(
                            "reads memory at an address that isn't a parameter its contract says \
                             holds bytes",
                        );
                    }
                    stack.push(false);
                }
                Op::Store { .. }
                | Op::MemoryGrow
                | Op::MemoryCopy
                | Op::MemoryFill
                | Op::MemoryInit(_)
                | Op::DataDrop(_) => return refuse("writes or changes memory"),
                Op::MemorySize | Op::Const { .. } => stack.push(false),
                Op::Unary(_) => {
                    if pop(&mut stack, 1) {
                        return refuse("computes with an address");
                    }
                    stack.push(false);
                }
                Op::Binary(Binary::Add) => {
                    let b = stack.pop().unwrap_or(false);
                    let a = stack.pop().unwrap_or(false);
                    if a && b {
                        return refuse("adds two addresses");
                    }
                    stack.push(a || b);
                }
                Op::Binary(_) => {
                    if pop(&mut stack, 2) {
                        return refuse("computes with an address other than adding a number to it");
                    }
                    stack.push(false);
                }
                Op::Float(operation) => {
                    let count = if float::binary(*operation) { 2 } else { 1 };
                    if pop(&mut stack, count) {
                        return refuse("computes with an address");
                    }
                    stack.push(false);
                }
                Op::Vector(operation) => {
                    if pop(&mut stack, vector::operands(operation)) {
                        return refuse("computes with an address");
                    }
                    stack.push(false);
                }
                Op::Other(_) => return refuse("has an operator the model doesn't follow"),
            }
        }
        None
    }

    /// The module with `claims`, each proven of its function alone ([`contract::settle`]): its
    /// runs take a claim's value at each call of the claim's function whose arguments are in the
    /// claim's ranges, in place of following the function's body. A claim whose function the
    /// module doesn't have, or whose function breaks a unit's rules, is left out.
    #[must_use]
    pub fn with_claims(mut self, claims: &[Claim]) -> Self {
        for claim in claims {
            if let Some(index) = self.own_function(&claim.function)
                && self.unit_problem(index, claim).is_none()
            {
                self.claims.insert(
                    index,
                    Held {
                        claim: claim.clone(),
                        used: AtomicUsize::new(0),
                        followed: AtomicUsize::new(0),
                    },
                );
            }
        }
        self
    }

    /// For each claim the module's runs take: its function's name, how many calls took the
    /// claim's value, and how many followed the function's body.
    #[must_use]
    pub fn claims_taken(&self) -> Vec<(String, usize, usize)> {
        self.claims
            .values()
            .map(|held| {
                (
                    held.claim.function.clone(),
                    held.used.load(Ordering::Relaxed),
                    held.followed.load(Ordering::Relaxed),
                )
            })
            .collect()
    }
}

/// The state a run of the function `index` alone starts in: `args` its arguments, its other
/// locals zero, each of `texts` in memory at its address, and every load held inside them.
/// `None` where the memory has no room for the texts.
pub(in crate::prove) fn begin(
    module: &Module,
    pool: &mut Pool,
    index: u32,
    args: &[Term],
    texts: &[(u64, Vec<Term>)],
) -> Option<State<Wasm>> {
    let function = module.function(index)?;
    let mut state = super::begin(module, pool, &[], &[], Vec::new());
    let size = module.pages.saturating_mul(PAGE);
    let mut windows = Vec::new();
    for (address, bytes) in texts {
        let end = address.checked_add(u64::try_from(bytes.len()).ok()?)?;
        if end > size {
            return None;
        }
        for (offset, byte) in (0_u64..).zip(bytes) {
            state.core.memory.insert(address + offset, *byte);
        }
        windows.push((*address, end));
    }
    let mut locals: Vec<Term> = args.to_vec();
    for kind in &function.locals {
        locals.push(pool.bv(kind.width(), 0));
    }
    state.core.stack.clear();
    state.core.frames = vec![Frame {
        function: index,
        locals,
        height: 0,
        labels: vec![function_label(0, function.results, function.ops.len())],
        pc: 0,
    }];
    state.core.window = Some(Rc::new(windows));
    Some(state)
}

/// What `run`, a finished run of a function alone, gave: the value it returned.
///
/// # Errors
///
/// How the run ended where it didn't return, or what it changed that a unit changes nothing of.
pub(in crate::prove) fn end(
    module: &Module,
    pool: &mut Pool,
    run: &State<Wasm>,
) -> Result<Term, String> {
    let returned = run.core.frames.is_empty() && matches!(run.world.end, Some(End::Exit(_)));
    if !returned {
        return Err(match &run.world.end {
            Some(End::Unsafe(what) | End::Unsupported(what) | End::Stuck(what)) => {
                format!("for some argument in its ranges, {what}")
            }
            Some(End::TooManyPaths) => {
                "more paths through the function than a proof of it alone follows".into()
            }
            _ => "a path through the function that doesn't return".into(),
        });
    }
    for ((width, value), now) in module.globals.iter().zip(&run.core.globals) {
        if pool.bv(*width, *value) != *now {
            return Err("the function changes a global".into());
        }
    }
    let world = &run.world;
    if !world.stdout.is_empty()
        || !world.stderr.is_empty()
        || !world.written.is_empty()
        || world.stdin_read != 0
    {
        return Err("the function reaches the host".into());
    }
    run.core
        .stack
        .last()
        .copied()
        .ok_or_else(|| "the function gives no value".to_owned())
}

/// The value the claim `claim` gives for the call the newest `params` operands of `state` are
/// the arguments of, where every argument is known to be in the claim's ranges and each address
/// and its length are numbers the run knows. `None` for any other call, and inside a unit's own
/// run.
fn value_at(
    module: &Module,
    state: &State<Wasm>,
    pool: &mut Pool,
    claim: &Claim,
    params: usize,
) -> Option<Term> {
    if state.core.window.is_some() {
        return None;
    }
    let start = state.core.stack.len().checked_sub(params)?;
    let args: Vec<Term> = state.core.stack.get(start..)?.to_vec();
    if args.len() != claim.params.len() {
        return None;
    }
    let mut texts = BTreeMap::new();
    for (param, arg) in claim.params.iter().zip(&args) {
        match param.role {
            Role::Number { low, high } => {
                let inside = if low < 0 {
                    let (least, most) = pool.signed_range(*arg);
                    least >= low && most <= high
                } else {
                    let (least, most) = pool.range(*arg);
                    i128::try_from(least).is_ok_and(|least| least >= low)
                        && i128::try_from(most).is_ok_and(|most| most <= high)
                };
                if !inside {
                    return None;
                }
            }
            Role::Text { length } => {
                let address = u64::try_from(pool.constant(*arg)?).ok()?;
                let bytes = u64::try_from(pool.constant(*args.get(length)?)?).ok()?;
                if !memory::inside(&state.core, address, bytes) {
                    return None;
                }
                let text: Vec<Term> = (0..bytes)
                    .map(|offset| memory::read(module, &state.core, pool, address + offset, 1))
                    .collect();
                texts.insert(param.name.clone(), text);
            }
            Role::Free => {}
        }
    }
    contract::value(pool, claim, &args, &texts).ok()
}

/// A call of the function `index`, which takes `params` operands: where the module holds a
/// claim about it and the call's arguments are in the claim's ranges, the arguments come off the
/// stack and the claim's value goes on it, and this says so. The function's body isn't run: its
/// proof alone covers every such call.
pub(super) fn reuse(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    index: u32,
    params: usize,
) -> bool {
    let Some(held) = module.claims.get(&index) else {
        return false;
    };
    let Some(value) = value_at(module, state, pool, &held.claim, params) else {
        held.followed.fetch_add(1, Ordering::Relaxed);
        return false;
    };
    let keep = state.core.stack.len().saturating_sub(params);
    state.core.stack.truncate(keep);
    state.core.stack.push(value);
    held.used.fetch_add(1, Ordering::Relaxed);
    module.enter(index);
    true
}
