//! lengua's library's `$fill`, read as a whole where a proof takes every way the input can be
//! cut, as the native prover reads its own `fill` ([`crate::prove::exec`]'s fill).
//!
//! `$fill(fd, ptr, cap, stop)` with stop -1 reads until its `cap` bytes are full or the input
//! ends: whatever pieces standard input arrives in, it ends with the same bytes, `cap` of them or
//! all that is left, and gives their count. So where a program reads standard input only so
//! ([`Module::reads_whole`]), one step of the model takes those bytes at once, for every way the
//! input can be cut, and standard input of any length is proven, not only what one pipe write
//! holds. Two things a cut does change, the model leaves to no one value: the iovec and the count
//! `$fill` leaves in lengua's own bytes, 0 to 11, which are any value after it. Where anything of
//! the call is other than that, the call runs as the code it is.
//!
//! [`Module::reads_whole`]: super::Module::reads_whole

use super::host::Handle;
use super::image::Module;
use super::{Wasm, memory};
use crate::prove::exec::{End, State, unsupported};
use crate::prove::term::{Pool, Term};

/// The bytes of lengua's own a call of `$fill` writes: an iovec, then the count each read gives.
const SCRATCH: u64 = 12;

/// The least address of a buffer `$fill` takes: below it, lengua's own bytes, which `$fill`
/// refuses with an assertion.
const FIRST_BUFFER: u64 = 16;

/// `$fill`'s stop byte for none: -1, as an i32's bits.
const NO_STOP: u128 = 0xffff_ffff;

/// `$fill`'s call, its four operands on the stack, read as a whole: the bytes it would read put
/// in place, standard input read on past them, and their count on the stack for the four
/// operands, when the call reads standard input to its end, on a buffer the model knows, which
/// the host would lend it.
///
/// # Errors
///
/// Standard input of a length the input chooses, read other than all of it at once.
pub(super) fn whole(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
) -> Result<bool, End> {
    let core = &state.core;
    let Some(start) = core.stack.len().checked_sub(4) else {
        return Ok(false);
    };
    let known = |term: &Term| {
        pool.constant(*term)
            .and_then(|value| u64::try_from(value).ok())
    };
    let operands: Option<Vec<u64>> = core.stack[start..].iter().map(known).collect();
    let Some([fd, buffer, cap, stop]) =
        operands.and_then(|values| <[u64; 4]>::try_from(values).ok())
    else {
        return Ok(false);
    };
    // What `$fill` and the host check first, and what reads more than standard input to its
    // end, the code runs as it is.
    let lent = buffer >= FIRST_BUFFER
        && cap > 0
        && memory::inside(core, buffer, cap)
        && memory::inside(core, 0, SCRATCH);
    if fd != 0
        || u128::from(stop) != NO_STOP
        || !lent
        || !module.exports_memory
        || !module.policy.reads_input()
        || !matches!(core.fds.get(&0), Some(Handle::Stdin))
    {
        return Ok(false);
    }
    let world = &mut state.world;
    let read = world.stdin_read.min(world.stdin.len());
    let mut available: Vec<Term> = world.stdin[read..].to_vec();
    let room = usize::try_from(cap).unwrap_or(usize::MAX);
    // Standard input of a length the input chooses, none of it read yet and all of it room for
    // the buffer: `$fill` takes all of it, its count that length. What comes before it, as the
    // buffer-full an induction's base puts first, is read as any input of its own length is.
    let mut count = None;
    if let Some(length) = world.stdin_length
        && read < world.stdin.len()
    {
        let from = world.stdin_from;
        if read < from {
            available.truncate(from - read);
        } else if read > from || room < available.len() {
            return Err(unsupported(
                "standard input of a length the input chooses read other than all at once",
            ));
        } else {
            count = Some(pool.extract(length, 31, 0));
        }
    }
    let taken = available.len().min(room);
    let bytes = &available[..taken];
    for (offset, byte) in (0_u64..).zip(bytes) {
        state.core.memory.insert(buffer + offset, *byte);
    }
    let world = &mut state.world;
    world.stdin_read = if count.is_some() {
        world.stdin.len()
    } else {
        read + taken
    };
    // The iovec and the counts a cut leaves in lengua's bytes are any value.
    let fresh = pool.len();
    for offset in 0..SCRATCH {
        let byte = pool.var(8, &format!("fill{fresh}.scratch{offset}"));
        state.core.memory.insert(offset, byte);
    }
    let count = count.unwrap_or_else(|| pool.bv(32, taken as u128));
    state.core.stack.truncate(start);
    state.core.stack.push(count);
    Ok(true)
}
