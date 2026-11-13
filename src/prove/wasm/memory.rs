//! Linear memory: bytes as terms at fixed addresses, so loads and stores of every width are
//! little-endian over bytes, as a CPU's memory is. A byte nothing wrote is the data segments' or
//! zero. An access whose place the input chooses is a choice of each place it can be, and one that
//! may leave the memory is a trap the proof must show never happens.

use super::image::{Module, PAGE};
use super::{Wasm, must_not};
use crate::prove::exec::{End, State, unsupported};
use crate::prove::term::{Pool, Term};

/// What the engine says of an access outside the memory.
pub(super) const OUTSIDE: &str = "it reached memory outside its own";

/// The most places an access whose place the input chooses may be: a table's entries for each
/// value of an input byte.
const MAX_PLACES: u128 = 256;

/// Whether the `bytes` bytes from `address` are inside the memory.
pub(super) fn inside(core: &Wasm, address: u64, bytes: u64) -> bool {
    address
        .checked_add(bytes)
        .is_some_and(|end| end <= core.pages.saturating_mul(PAGE))
}

/// The byte at `address`: what the program wrote there, or the data segments', or zero.
fn byte(module: &Module, core: &Wasm, pool: &mut Pool, address: u64) -> Term {
    if let Some(byte) = core.memory.get(address) {
        return byte;
    }
    let initial = usize::try_from(address)
        .ok()
        .and_then(|index| module.initial.get(index))
        .copied()
        .unwrap_or(0);
    pool.bv(8, u128::from(initial))
}

/// The `bytes` bytes at `address`, which are inside the memory, as one little-endian value: the
/// value stored there whole, when its bytes are still the ones it was split into.
pub(super) fn read(
    module: &Module,
    core: &Wasm,
    pool: &mut Pool,
    address: u64,
    bytes: u32,
) -> Term {
    let parts: Vec<Term> = (0..u64::from(bytes))
        .map(|offset| byte(module, core, pool, address + offset))
        .collect();
    if parts.len() > 1
        && let Some(whole) = pool.split_of(&parts)
    {
        return whole;
    }
    let Some((first, rest)) = parts.split_first() else {
        return pool.bv(8, 0);
    };
    rest.iter()
        .fold(*first, |value, part| pool.concat(*part, value))
}

/// `value`'s bytes, lowest first, remembered as its own, so reading them back gives it.
fn split(pool: &mut Pool, value: Term) -> Vec<Term> {
    let bytes = pool.width(value) / 8;
    let parts: Vec<Term> = (0..bytes)
        .map(|byte| pool.extract(value, byte * 8 + 7, byte * 8))
        .collect();
    if parts.len() > 1 {
        pool.remember_split(&parts, value);
    }
    parts
}

/// Writes `value` little-endian at `address`, where it fits inside the memory.
pub(super) fn write(core: &mut Wasm, pool: &mut Pool, address: u64, value: Term) {
    let parts = split(pool, value);
    for (offset, part) in (0_u64..).zip(parts) {
        core.memory.insert(address + offset, part);
    }
}

/// An access's address: its base, an i32, plus the operator's `offset`, as 64 bits, so the sum
/// never wraps, as WebAssembly's doesn't.
fn effective(pool: &mut Pool, base: Term, offset: u64) -> Term {
    let wide = pool.zero_extend(base, 32);
    let offset = pool.bv(64, u128::from(offset));
    pool.add(wide, offset)
}

/// The places an access of `bytes` bytes at `address` may be inside the memory, each with the
/// condition that it's there. Where the access may leave the memory, the run ends at the trap
/// where it surely does, and the proof must show it never does where the input decides.
///
/// # Errors
///
/// The trap, and an access among more places than the model follows.
fn places(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
    address: Term,
    bytes: u64,
) -> Result<Vec<(Term, u64)>, End> {
    let size = state.core.pages.saturating_mul(PAGE);
    let Some(last) = size.checked_sub(bytes) else {
        return Err(state.core.trap(module, at, OUTSIDE));
    };
    if let Some(value) = pool.constant(address) {
        return match u64::try_from(value).ok().filter(|start| *start <= last) {
            Some(start) => Ok(vec![(pool.boolean(true), start)]),
            None => Err(state.core.trap(module, at, OUTSIDE)),
        };
    }
    let bound = pool.bv(64, u128::from(last));
    let fails = pool.ult(bound, address);
    must_not(module, state, pool, at, fails, OUTSIDE)?;
    let (low, high) = pool.range(address);
    let high = high.min(u128::from(last));
    let (step, remainder) = pool.stride(address);
    // The first place from `low` up that the address's stride lands on.
    let first = low + (remainder + step - low % step) % step;
    if first > high {
        return Err(state.core.trap(module, at, OUTSIDE));
    }
    if (high - first) / step >= MAX_PLACES {
        let what = format!(
            "an access whose place the input chooses among more than {MAX_PLACES} places, which \
             the model doesn't follow"
        );
        return Err(unsupported(state.core.placed(module, at, &what)));
    }
    let mut found = Vec::new();
    let mut place = first;
    while place <= high {
        let constant = pool.bv(64, place);
        let here = pool.eq(address, constant);
        if pool.truth(here) != Some(false) {
            found.push((here, u64::try_from(place).unwrap_or(u64::MAX)));
        }
        place += step;
    }
    if found.is_empty() {
        return Err(state.core.trap(module, at, OUTSIDE));
    }
    Ok(found)
}

/// A load of `bytes` bytes at `base` plus `offset`, at the operator `at`.
///
/// # Errors
///
/// A trap outside the memory.
pub(super) fn load(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
    (base, offset): (Term, u64),
    bytes: u32,
) -> Result<Term, End> {
    let address = effective(pool, base, offset);
    let found = places(module, state, pool, at, address, u64::from(bytes))?;
    // A proof of one function alone: every load stays inside the bytes the function's contract
    // says its addresses hold, so nothing else of memory can reach what the function gives.
    if let Some(windows) = &state.core.window {
        let inside = found.iter().all(|(_, place)| {
            windows.iter().any(|(start, end)| {
                start <= place && place.saturating_add(u64::from(bytes)) <= *end
            })
        });
        if !inside {
            return Err(unsupported(state.core.placed(
                module,
                at,
                super::unit::OUTSIDE_ITS_TEXT,
            )));
        }
    }
    let mut value: Option<Term> = None;
    for (here, place) in found.iter().rev() {
        let there = read(module, &state.core, pool, *place, bytes);
        value = Some(match value {
            None => there,
            Some(elsewhere) => pool.ite(*here, there, elsewhere),
        });
    }
    value.ok_or_else(|| state.core.trap(module, at, OUTSIDE))
}

/// A store of `value`'s low `bytes` bytes at `base` plus `offset`, at the operator `at`.
///
/// # Errors
///
/// A trap outside the memory.
pub(super) fn store(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
    (base, offset): (Term, u64),
    (value, bytes): (Term, u32),
) -> Result<(), End> {
    let address = effective(pool, base, offset);
    let found = places(module, state, pool, at, address, u64::from(bytes))?;
    let bits = bytes * 8;
    let value = if pool.width(value) > bits {
        pool.extract(value, bits - 1, 0)
    } else {
        value
    };
    if let [(_, place)] = found.as_slice() {
        write(&mut state.core, pool, *place, value);
        return Ok(());
    }
    // Each place it may be keeps its byte, but where the address is that place.
    let parts = split(pool, value);
    for (here, place) in found {
        for (offset, part) in (0_u64..).zip(&parts) {
            let address = place + offset;
            let old = byte(module, &state.core, pool, address);
            let new = pool.ite(here, *part, old);
            state.core.memory.insert(address, new);
        }
    }
    Ok(())
}

/// The value on top of the stack, taken off.
fn take(state: &mut State<Wasm>) -> Result<Term, End> {
    state
        .core
        .stack
        .pop()
        .ok_or_else(|| unsupported("a memory operator with no value on the stack"))
}

/// The value of `term`, a place or a length the input mustn't choose, at the operator `at`.
fn known(
    module: &Module,
    state: &State<Wasm>,
    pool: &Pool,
    at: usize,
    term: Term,
) -> Result<u64, End> {
    pool.constant(term)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| {
            unsupported(state.core.placed(
                module,
                at,
                "a memory operator on a place or a length the input chooses, which the model \
                 doesn't follow",
            ))
        })
}

/// `memory.grow`: the pages it had, with more where the memory may grow, else -1.
///
/// # Errors
///
/// A growth the input chooses.
pub(super) fn grow(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
) -> Result<(), End> {
    let delta = take(state)?;
    let delta = known(module, state, pool, at, delta)?;
    let old = state.core.pages;
    let result = match old
        .checked_add(delta)
        .filter(|pages| *pages <= module.most_pages)
    {
        Some(pages) => {
            state.core.pages = pages;
            u128::from(old)
        }
        None => u128::from(u32::MAX),
    };
    let result = pool.bv(32, result);
    state.core.stack.push(result);
    Ok(())
}

/// `memory.fill`: `n` bytes from `d` set to `val`'s low byte.
///
/// # Errors
///
/// A trap past the memory, and a place or length the input chooses.
pub(super) fn fill(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
) -> Result<(), End> {
    let length = take(state)?;
    let value = take(state)?;
    let destination = take(state)?;
    let length = known(module, state, pool, at, length)?;
    let destination = known(module, state, pool, at, destination)?;
    if !inside(&state.core, destination, length) {
        return Err(state.core.trap(module, at, OUTSIDE));
    }
    let byte = pool.extract(value, 7, 0);
    for offset in 0..length {
        state.core.memory.insert(destination + offset, byte);
    }
    Ok(())
}

/// `memory.copy`: `n` bytes from `s` to `d`, as if through a buffer, so the two may overlap.
///
/// # Errors
///
/// A trap past the memory, and a place or length the input chooses.
pub(super) fn copy(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
) -> Result<(), End> {
    let length = take(state)?;
    let source = take(state)?;
    let destination = take(state)?;
    let length = known(module, state, pool, at, length)?;
    let source = known(module, state, pool, at, source)?;
    let destination = known(module, state, pool, at, destination)?;
    if !inside(&state.core, source, length) || !inside(&state.core, destination, length) {
        return Err(state.core.trap(module, at, OUTSIDE));
    }
    let bytes: Vec<Term> = (0..length)
        .map(|offset| byte(module, &state.core, pool, source + offset))
        .collect();
    for (offset, part) in (0_u64..).zip(bytes) {
        state.core.memory.insert(destination + offset, part);
    }
    Ok(())
}

/// `memory.init`: `n` bytes of data segment `segment` from `s`, to `d`. A dropped segment has
/// none, as an active one has once the program has started.
///
/// # Errors
///
/// A trap past the segment or the memory, and a place or length the input chooses.
pub(super) fn init(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    at: usize,
    segment: u32,
) -> Result<(), End> {
    let length = take(state)?;
    let source = take(state)?;
    let destination = take(state)?;
    let length = known(module, state, pool, at, length)?;
    let source = known(module, state, pool, at, source)?;
    let destination = known(module, state, pool, at, destination)?;
    let index = usize::try_from(segment).unwrap_or(usize::MAX);
    let data = module
        .data
        .get(index)
        .ok_or_else(|| unsupported("memory.init of a data segment the module doesn't have"))?;
    let dropped = state.core.dropped.get(index).copied().unwrap_or(true);
    let available = if dropped {
        0
    } else {
        u64::try_from(data.len()).unwrap_or(u64::MAX)
    };
    let within = source
        .checked_add(length)
        .is_some_and(|end| end <= available);
    if !within || !inside(&state.core, destination, length) {
        return Err(state.core.trap(module, at, OUTSIDE));
    }
    for offset in 0..length {
        let from = usize::try_from(source + offset).unwrap_or(usize::MAX);
        let value = data.get(from).copied().unwrap_or(0);
        let constant = pool.bv(8, u128::from(value));
        state.core.memory.insert(destination + offset, constant);
    }
    Ok(())
}
