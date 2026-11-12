//! One step of a WebAssembly run: one operator, with WebAssembly's semantics over terms. A branch
//! whose condition the input decides splits the run, each side keeping its condition, and a
//! `br_table` whose index the input decides is a choice of every way it can go.

use super::host::{self, Handle};
use super::image::{Arity, Binary, Function, Module, NOWHERE, Op, Unary};
use super::{Frame, Label, Wasm, fill, float, function_label, induction, memory, must_not, vector};
use crate::prove::exec::{End, MAX_STEPS, State, Step, TOO_LONG, unsupported};
use crate::prove::term::{Pool, Term};

/// What a module the validator took never does, which the model reports rather than panics on.
const MALFORMED: &str = "a module the validator should have refused";

/// The end of a run that met [`MALFORMED`].
fn malformed() -> End {
    unsupported(MALFORMED)
}

/// The ones of a value `width` bits wide.
const fn mask(width: u32) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    }
}

/// Runs the next operator of `state`.
///
/// # Errors
///
/// How the run ends, where the operator ends it.
pub(super) fn execute(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
) -> Result<Step<Wasm>, End> {
    state.world.steps += 1;
    if state.world.steps > MAX_STEPS {
        return Err(unsupported(TOO_LONG));
    }
    if !state.core.handed {
        hand_over(module, state);
    }
    // The read loop's head, in a proof by induction, where every way the input is cut is taken.
    if let Some(hook) = state.core.hook.clone()
        && state.world.every_cut
        && induction::at_head(&state.core, hook.head)
    {
        induction::arrive(module, state, pool, &hook)?;
    }
    let Some(frame) = state.core.frames.last_mut() else {
        // `_start` returned: the program exits with 0, as the host's run has it.
        return Err(End::Exit(pool.bv(8, 0)));
    };
    let function = module.function(frame.function).ok_or_else(malformed)?;
    let at = frame.pc;
    let op = function.ops.get(at).ok_or_else(malformed)?;
    frame.pc += 1;
    operate(module, function, state, pool, op, at)
}

/// Puts the descriptors lengua hands the program over in place, as the host does before the
/// program starts: the listener at 4 for an idea that listens, and the connections lengua made
/// at 5 and on, each the world's socket for it; the program's own take the numbers after them.
fn hand_over(module: &Module, state: &mut State<Wasm>) {
    let core = &mut state.core;
    core.handed = true;
    let network = &state.world.network;
    if module.policy.listens && network.listening {
        core.fds
            .insert(crate::wasm::host::fds::LISTENER, Handle::Listener);
    }
    let first = crate::wasm::host::fds::FIRST_CONNECTION;
    let made = u32::try_from(network.made.len()).unwrap_or(0);
    for index in 0..made {
        core.fds.insert(first + index, host::made(index));
    }
    core.first = first + made;
}

/// Runs `op`, the operator at `at` of `function`, which the newest call runs.
#[allow(clippy::too_many_lines)]
fn operate(
    module: &Module,
    function: &Function,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    op: &Op,
    at: usize,
) -> Result<Step<Wasm>, End> {
    match op {
        Op::Unreachable => return Err(state.core.trap(module, at, "it reached `unreachable`")),
        Op::Nop => {}
        Op::Block(arity) => {
            let end = *function.ends.get(at).ok_or_else(malformed)?;
            enter(&mut state.core, *arity, false, end.saturating_add(1))?;
        }
        Op::Loop(arity) => enter(&mut state.core, *arity, true, at + 1)?,
        Op::If(arity) => return choose(function, state, pool, *arity, at),
        Op::Else => {
            // The end of an `if`'s first arm: on to its `end`, which leaves the block.
            let end = *function.ends.get(at).ok_or_else(malformed)?;
            frame(&mut state.core)?.pc = end;
        }
        Op::End => {
            let frame = frame(&mut state.core)?;
            frame.labels.pop();
            if frame.labels.is_empty() {
                finish(module, &mut state.core, pool)?;
            }
        }
        Op::Br(depth) => branch(module, &mut state.core, pool, *depth)?,
        Op::BrIf(depth) => return branch_if(module, state, pool, *depth),
        Op::BrTable(depths, default) => {
            return branch_table(module, state, pool, depths, *default);
        }
        Op::Return => finish(module, &mut state.core, pool)?,
        Op::Call(index) => return call(module, state, pool, *index, at),
        Op::Drop => {
            pop(&mut state.core)?;
        }
        Op::Select => {
            let core = &mut state.core;
            let condition = pop(core)?;
            let otherwise = pop(core)?;
            let then = pop(core)?;
            let value = match pool.constant(condition) {
                Some(0) => otherwise,
                Some(_) => then,
                None => {
                    let holds = nonzero(pool, condition);
                    pool.ite(holds, then, otherwise)
                }
            };
            core.stack.push(value);
        }
        Op::LocalGet(index) => {
            let core = &mut state.core;
            let value = local(core, *index)?;
            core.stack.push(value);
        }
        Op::LocalSet(index) => {
            let core = &mut state.core;
            let value = pop(core)?;
            *local_mut(core, *index)? = value;
        }
        Op::LocalTee(index) => {
            let core = &mut state.core;
            let value = *core.stack.last().ok_or_else(malformed)?;
            *local_mut(core, *index)? = value;
        }
        Op::GlobalGet(index) => {
            let core = &mut state.core;
            let value = *usize::try_from(*index)
                .ok()
                .and_then(|index| core.globals.get(index))
                .ok_or_else(malformed)?;
            core.stack.push(value);
        }
        Op::GlobalSet(index) => {
            let core = &mut state.core;
            let value = pop(core)?;
            let global = usize::try_from(*index)
                .ok()
                .and_then(|index| core.globals.get_mut(index))
                .ok_or_else(malformed)?;
            *global = value;
        }
        Op::Load {
            bytes,
            width,
            signed,
            offset,
        } => {
            let address = pop(&mut state.core)?;
            let value = memory::load(module, state, pool, at, (address, *offset), *bytes)?;
            let value = widened(pool, value, *width, *signed);
            state.core.stack.push(value);
        }
        Op::Store { bytes, offset } => {
            let value = pop(&mut state.core)?;
            let address = pop(&mut state.core)?;
            memory::store(module, state, pool, at, (address, *offset), (value, *bytes))?;
        }
        Op::MemorySize => {
            let pages = pool.bv(32, u128::from(state.core.pages));
            state.core.stack.push(pages);
        }
        Op::MemoryGrow => memory::grow(module, state, pool, at)?,
        Op::MemoryCopy => memory::copy(module, state, pool, at)?,
        Op::MemoryFill => memory::fill(module, state, pool, at)?,
        Op::MemoryInit(segment) => memory::init(module, state, pool, at, *segment)?,
        Op::DataDrop(segment) => {
            let dropped = usize::try_from(*segment)
                .ok()
                .and_then(|segment| state.core.dropped.get_mut(segment))
                .ok_or_else(malformed)?;
            *dropped = true;
        }
        Op::Const { width, value } => {
            let constant = pool.bv(*width, *value);
            state.core.stack.push(constant);
        }
        Op::Unary(operation) => {
            let value = pop(&mut state.core)?;
            let result = unary(pool, *operation, value);
            state.core.stack.push(result);
        }
        Op::Binary(operation) => {
            let b = pop(&mut state.core)?;
            let a = pop(&mut state.core)?;
            if operation.divides() {
                guard_division(module, state, pool, at, *operation, (a, b))?;
            }
            let result = binary(pool, *operation, a, b);
            state.core.stack.push(result);
        }
        Op::Float(operation) => {
            let core = &mut state.core;
            let b = pop(core)?;
            let (a, b) = if float::binary(*operation) {
                (pop(core)?, b)
            } else {
                (b, b)
            };
            let result = float::operate(pool, *operation, a, b);
            core.stack.push(result);
        }
        Op::Vector(operation) => {
            let core = &mut state.core;
            let start = core
                .stack
                .len()
                .checked_sub(vector::operands(operation))
                .ok_or_else(malformed)?;
            let operands: Vec<Term> = core.stack.drain(start..).collect();
            let result = vector::operate(pool, operation, &operands).ok_or_else(malformed)?;
            core.stack.push(result);
        }
        Op::Other(name) => {
            let what = format!("`{name}`, which the WebAssembly model doesn't follow yet");
            return Err(unsupported(state.core.placed(module, at, &what)));
        }
    }
    Ok(Step::Next)
}

/// The newest call.
fn frame(core: &mut Wasm) -> Result<&mut Frame, End> {
    core.frames.last_mut().ok_or_else(malformed)
}

/// The value on top of the stack, taken off.
fn pop(core: &mut Wasm) -> Result<Term, End> {
    core.stack.pop().ok_or_else(malformed)
}

/// The newest call's local `index`.
fn local(core: &mut Wasm, index: u32) -> Result<Term, End> {
    let frame = frame(core)?;
    usize::try_from(index)
        .ok()
        .and_then(|index| frame.locals.get(index))
        .copied()
        .ok_or_else(malformed)
}

/// The newest call's local `index`, to set.
fn local_mut(core: &mut Wasm, index: u32) -> Result<&mut Term, End> {
    let frame = frame(core)?;
    usize::try_from(index)
        .ok()
        .and_then(|index| frame.locals.get_mut(index))
        .ok_or_else(malformed)
}

/// Whether `value` isn't zero, as `if`, `br_if` and `select` read a condition.
pub(super) fn nonzero(pool: &mut Pool, value: Term) -> Term {
    let width = pool.width(value);
    let zero = pool.bv(width, 0);
    let is_zero = pool.eq(value, zero);
    pool.bool_not(is_zero)
}

/// 1 where `holds`, else 0, as an i32: what a comparison gives.
fn truth(pool: &mut Pool, holds: Term) -> Term {
    let (one, zero) = (pool.bv(32, 1), pool.bv(32, 0));
    pool.ite(holds, one, zero)
}

/// A block, loop or `if` entered with `arity`: its label, for a branch to it, that goes to
/// `target`, back to its start for a loop.
fn enter(core: &mut Wasm, arity: Arity, looping: bool, target: usize) -> Result<(), End> {
    let height = core
        .stack
        .len()
        .checked_sub(arity.params)
        .ok_or_else(malformed)?;
    let carries = if looping { arity.params } else { arity.results };
    frame(core)?.labels.push(Label {
        looping,
        height,
        carries,
        target,
    });
    Ok(())
}

/// An `if` at `at`: its first arm where its condition holds, its `else` or its `end` where it
/// doesn't, and both, a run each, where the input decides.
fn choose(
    function: &Function,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    arity: Arity,
    at: usize,
) -> Result<Step<Wasm>, End> {
    let condition = pop(&mut state.core)?;
    let end = *function.ends.get(at).ok_or_else(malformed)?;
    let other = *function.elses.get(at).ok_or_else(malformed)?;
    enter(&mut state.core, arity, false, end.saturating_add(1))?;
    // Where it doesn't hold: past the `else`, or to the `end`, which leaves the block.
    let otherwise = if other == NOWHERE { end } else { other + 1 };
    match pool.constant(condition) {
        Some(0) => frame(&mut state.core)?.pc = otherwise,
        Some(_) => {}
        None => {
            let holds = nonzero(pool, condition);
            let mut skipped = state.clone();
            frame(&mut skipped.core)?.pc = otherwise;
            return Ok(Step::Split(holds, Box::new(skipped)));
        }
    }
    Ok(Step::Next)
}

/// Returns from the newest call: its results on its caller's stack. `_start`'s return ends the
/// program with 0.
///
/// # Errors
///
/// The program's exit, when `_start` returns.
fn finish(module: &Module, core: &mut Wasm, pool: &mut Pool) -> Result<(), End> {
    let frame = core.frames.pop().ok_or_else(malformed)?;
    let results = module
        .function(frame.function)
        .map_or(0, |found| found.results);
    let start = core
        .stack
        .len()
        .checked_sub(results)
        .ok_or_else(malformed)?;
    let values: Vec<Term> = core.stack.drain(start..).collect();
    core.stack.truncate(frame.height);
    core.stack.extend(values);
    if core.frames.is_empty() {
        return Err(End::Exit(pool.bv(8, 0)));
    }
    Ok(())
}

/// A branch to the label `depth` blocks out: what it carries kept, the rest of the blocks' values
/// gone, and on where it goes. A branch out of the function's body returns.
///
/// # Errors
///
/// The program's exit, when it returns from `_start`.
fn branch(module: &Module, core: &mut Wasm, pool: &mut Pool, depth: u32) -> Result<(), End> {
    let labels = frame(core)?.labels.len();
    let index = usize::try_from(depth)
        .ok()
        .and_then(|depth| labels.checked_sub(depth.checked_add(1)?))
        .ok_or_else(malformed)?;
    if index == 0 {
        return finish(module, core, pool);
    }
    let label = frame(core)?.labels[index];
    let start = core
        .stack
        .len()
        .checked_sub(label.carries)
        .ok_or_else(malformed)?;
    let values: Vec<Term> = core.stack.drain(start..).collect();
    core.stack.truncate(label.height);
    core.stack.extend(values);
    let frame = frame(core)?;
    frame
        .labels
        .truncate(if label.looping { index + 1 } else { index });
    frame.pc = label.target;
    Ok(())
}

/// A branch that ends the run, as one out of `_start` does, on a side of a fork: that side's
/// end, which the exploration finds there.
fn branched(module: &Module, state: &mut State<Wasm>, pool: &mut Pool, depth: u32) {
    if let Err(end) = branch(module, &mut state.core, pool, depth) {
        state.world.end = Some(end);
    }
}

/// `br_if`: the branch where its condition holds, on where it doesn't, and both, a run each,
/// where the input decides.
fn branch_if(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    depth: u32,
) -> Result<Step<Wasm>, End> {
    let condition = pop(&mut state.core)?;
    match pool.constant(condition) {
        Some(0) => {}
        Some(_) => branch(module, &mut state.core, pool, depth)?,
        None => {
            let holds = nonzero(pool, condition);
            let stays = state.clone();
            branched(module, state, pool, depth);
            return Ok(Step::Split(holds, Box::new(stays)));
        }
    }
    Ok(Step::Next)
}

/// `br_table`: the branch its index picks, its default past its labels, and every one of them,
/// a run each, where the input decides the index.
fn branch_table(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    depths: &[u32],
    default: u32,
) -> Result<Step<Wasm>, End> {
    let index = pop(&mut state.core)?;
    if let Some(value) = pool.constant(index) {
        let depth = usize::try_from(value)
            .ok()
            .and_then(|value| depths.get(value))
            .copied()
            .unwrap_or(default);
        branch(module, &mut state.core, pool, depth)?;
        return Ok(Step::Next);
    }
    let mut choices = Vec::new();
    for (value, depth) in depths.iter().enumerate() {
        let constant = pool.bv(32, value as u128);
        let picked = pool.eq(index, constant);
        if pool.truth(picked) == Some(false) {
            continue;
        }
        let mut choice = state.clone();
        branched(module, &mut choice, pool, *depth);
        choices.push((picked, choice));
    }
    let count = pool.bv(32, depths.len() as u128);
    let past = pool.ule(count, index);
    if pool.truth(past) != Some(false) {
        let mut choice = state.clone();
        branched(module, &mut choice, pool, default);
        choices.push((past, choice));
    }
    Ok(Step::Choose(choices))
}

/// A call of the function `index`: lengua's host for an import, else a new call with its
/// parameters off the stack and its other locals zero. The stack holds it: the module's deepest
/// calls fit in the engine's, or the model wouldn't run it.
///
/// # Errors
///
/// How a host call ends the run.
fn call(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    index: u32,
    at: usize,
) -> Result<Step<Wasm>, End> {
    let callee = module.function(index).ok_or_else(malformed)?;
    // A function of a module the program calls: the model follows no other module, and doesn't
    // know what the call gives back, so the run stops here.
    if callee.called {
        return Err(unsupported(format!(
            "a call of {}, a function of a module it calls, which the model doesn't follow: the \
             program is tested, not proven, where it depends on the call",
            callee.name.as_deref().unwrap_or("a function")
        )));
    }
    if let Some(function) = callee.import {
        return host::call(module, state, pool, function, at);
    }
    // Every read a proof by induction takes is a call of `$fill`: the read loop's must be the
    // first.
    if module.fill == Some(index) {
        state.core.filled = true;
    }
    // lengua's `$fill` on standard input, read as a whole, where a proof takes every way the
    // input can be cut.
    if module.fill == Some(index)
        && state.world.every_cut
        && let Some(step) = fill::whole(module, state, pool)?
    {
        return Ok(step);
    }
    module.enter(index);
    let core = &mut state.core;
    let start = core
        .stack
        .len()
        .checked_sub(callee.params)
        .ok_or_else(malformed)?;
    let mut locals: Vec<Term> = core.stack.drain(start..).collect();
    for kind in &callee.locals {
        locals.push(pool.bv(kind.width(), 0));
    }
    let height = core.stack.len();
    core.frames.push(Frame {
        function: index,
        locals,
        height,
        labels: vec![function_label(height, callee.results, callee.ops.len())],
        pc: 0,
    });
    Ok(Step::Next)
}

/// Holds a division or remainder to its traps: a divisor of zero, and for `div_s`, the least
/// value divided by -1.
fn guard_division(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
    operation: Binary,
    (a, b): (Term, Term),
) -> Result<(), End> {
    let width = pool.width(b);
    let zero = pool.bv(width, 0);
    let by_zero = pool.eq(b, zero);
    must_not(
        module,
        state,
        pool,
        at,
        by_zero,
        "an integer division by zero",
    )?;
    if operation == Binary::DivS {
        let least = pool.bv(width, 1 << (width - 1));
        let minus_one = pool.bv(width, mask(width));
        let is_least = pool.eq(a, least);
        let by_minus_one = pool.eq(b, minus_one);
        let overflows = pool.bool_and(is_least, by_minus_one);
        must_not(
            module,
            state,
            pool,
            at,
            overflows,
            "a signed division whose result doesn't fit",
        )?;
    }
    Ok(())
}

/// A shift or rotation's count: modulo the value's width, as WebAssembly takes it.
fn count_of(pool: &mut Pool, count: Term, width: u32) -> Term {
    let below = pool.bv(width, u128::from(width - 1));
    pool.and(count, below)
}

/// `a` rotated by `by`, to the left or the right.
fn rotate(pool: &mut Pool, a: Term, by: Term, left: bool) -> Term {
    let width = pool.width(a);
    let count = count_of(pool, by, width);
    let full = pool.bv(width, u128::from(width));
    let rest = pool.sub(full, count);
    let back = count_of(pool, rest, width);
    let (first, second) = if left {
        (pool.shl(a, count), pool.lshr(a, back))
    } else {
        (pool.lshr(a, count), pool.shl(a, back))
    };
    pool.or(first, second)
}

/// `operation` on two integers of one width: its value, a comparison's as an i32. A division's
/// value where it would trap is SMT-LIB's, which no run that passes its obligations reads.
fn binary(pool: &mut Pool, operation: Binary, a: Term, b: Term) -> Term {
    let width = pool.width(a);
    let compared = match operation {
        Binary::Add => return pool.add(a, b),
        Binary::Sub => return pool.sub(a, b),
        Binary::Mul => return pool.mul(a, b),
        Binary::DivS => return pool.sdiv(a, b),
        Binary::DivU => return pool.udiv(a, b),
        Binary::RemS => return pool.srem(a, b),
        Binary::RemU => return pool.urem(a, b),
        Binary::And => return pool.and(a, b),
        Binary::Or => return pool.or(a, b),
        Binary::Xor => return pool.xor(a, b),
        Binary::Shl => {
            let count = count_of(pool, b, width);
            return pool.shl(a, count);
        }
        Binary::ShrS => {
            let count = count_of(pool, b, width);
            return pool.ashr(a, count);
        }
        Binary::ShrU => {
            let count = count_of(pool, b, width);
            return pool.lshr(a, count);
        }
        Binary::Rotl => return rotate(pool, a, b, true),
        Binary::Rotr => return rotate(pool, a, b, false),
        Binary::Eq => pool.eq(a, b),
        Binary::Ne => {
            let same = pool.eq(a, b);
            pool.bool_not(same)
        }
        Binary::LtS => pool.slt(a, b),
        Binary::LtU => pool.ult(a, b),
        Binary::GtS => pool.slt(b, a),
        Binary::GtU => pool.ult(b, a),
        Binary::LeS => pool.sle(a, b),
        Binary::LeU => pool.ule(a, b),
        Binary::GeS => pool.sle(b, a),
        Binary::GeU => pool.ule(b, a),
    };
    truth(pool, compared)
}

/// `operation` on one integer.
fn unary(pool: &mut Pool, operation: Unary, a: Term) -> Term {
    let width = pool.width(a);
    // A known value's bits are counted at once.
    if let Some(value) = pool.constant(a) {
        let counted = match operation {
            Unary::Clz => Some(value.leading_zeros() - (128 - width)),
            Unary::Ctz => Some(if value == 0 {
                width
            } else {
                value.trailing_zeros()
            }),
            Unary::Popcnt => Some(value.count_ones()),
            _ => None,
        };
        if let Some(counted) = counted {
            return pool.bv(width, u128::from(counted));
        }
    }
    match operation {
        Unary::Eqz => {
            let zero = pool.bv(width, 0);
            let holds = pool.eq(a, zero);
            truth(pool, holds)
        }
        Unary::Clz => {
            // From the lowest bit up, so the highest bit that's set decides.
            let mut count = pool.bv(width, u128::from(width));
            for bit in 0..width {
                let set = pool.bit_is_set(a, bit);
                let here = pool.bv(width, u128::from(width - 1 - bit));
                count = pool.ite(set, here, count);
            }
            count
        }
        Unary::Ctz => {
            // From the highest bit down, so the lowest bit that's set decides.
            let mut count = pool.bv(width, u128::from(width));
            for bit in (0..width).rev() {
                let set = pool.bit_is_set(a, bit);
                let here = pool.bv(width, u128::from(bit));
                count = pool.ite(set, here, count);
            }
            count
        }
        Unary::Popcnt => {
            let bits: Vec<Term> = (0..width).map(|bit| pool.bit_is_set(a, bit)).collect();
            pool.count(width, &bits)
        }
        Unary::Extend8 => {
            let low = pool.extract(a, 7, 0);
            pool.sign_extend(low, width - 8)
        }
        Unary::Extend16 => {
            let low = pool.extract(a, 15, 0);
            pool.sign_extend(low, width - 16)
        }
        Unary::Extend32 => {
            let low = pool.extract(a, 31, 0);
            pool.sign_extend(low, width - 32)
        }
        Unary::Wrap => pool.extract(a, 31, 0),
        Unary::Signed => pool.sign_extend(a, 32),
        Unary::Unsigned => pool.zero_extend(a, 32),
    }
}

/// A loaded value of fewer bits than `width`, extended to it, signed or not.
fn widened(pool: &mut Pool, value: Term, width: u32, signed: bool) -> Term {
    let from = pool.width(value);
    if from >= width {
        return value;
    }
    if signed {
        pool.sign_extend(value, width - from)
    } else {
        pool.zero_extend(value, width - from)
    }
}
