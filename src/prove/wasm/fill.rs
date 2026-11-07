//! lengua's library's `$fill`, read as a whole where a proof takes every way the input can be
//! cut, as the native prover reads its own `fill` ([`crate::prove::exec`]'s fill).
//!
//! `$fill(fd, ptr, cap, stop)` reads until its `cap` bytes are full, the input ends, or a read
//! ends with the stop byte. A read can end after any byte, so `$fill` of standard input can stop
//! after any stop byte that has arrived, or else ends full or at the input's end: each way is a
//! run of its own, and with no stop byte (-1) there is just the one, `cap` bytes or all that is
//! left, however the input arrives. So where a program reads standard input only so
//! ([`Module::reads_whole`]), one step of the model takes those bytes at once, for every way the
//! input can be cut, and standard input of any length is proven, not only what one pipe write
//! holds. What a cut changes beyond that, the model leaves to no one value: the iovec and the
//! count `$fill` leaves in lengua's own bytes, 0 to 11, are any value after it. Where anything
//! of the call is other than that, the call runs as the code it is.
//!
//! [`Module::reads_whole`]: super::Module::reads_whole

use super::host::Handle;
use super::image::Module;
use super::{Wasm, memory};
use crate::prove::exec::{End, State, Step, unsupported};
use crate::prove::term::{Pool, Term};

/// The bytes of lengua's own a call of `$fill` writes: an iovec, then the count each read gives.
const SCRATCH: u64 = 12;

/// The least address of a buffer `$fill` takes: below it, lengua's own bytes, which `$fill`
/// refuses with an assertion.
const FIRST_BUFFER: u64 = 16;

/// One way `$fill`'s reads can go: where it holds, and how many bytes it took.
type Outcome = (Term, usize);

/// `$fill`'s call having read `bytes` into `buffer`: the bytes in place, standard input read to
/// `read_to`, lengua's bytes any value, and `count` on the stack for the four operands, which
/// start at `start`.
fn take(
    state: &mut State<Wasm>,
    pool: &mut Pool,
    (start, buffer): (usize, u64),
    bytes: &[Term],
    (read_to, count): (usize, Term),
) {
    for (offset, byte) in (0_u64..).zip(bytes) {
        state.core.memory.insert(buffer + offset, *byte);
    }
    state.world.stdin_read = read_to;
    // The iovec and the counts a cut leaves in lengua's bytes are any value.
    let fresh = pool.len();
    for offset in 0..SCRATCH {
        let byte = pool.var(8, &format!("fill{fresh}.scratch{offset}"));
        state.core.memory.insert(offset, byte);
    }
    state.core.stack.truncate(start);
    state.core.stack.push(count);
}

/// `$fill`'s call, its four operands on the stack, read as a whole: every way its reads can go,
/// each a run of its own, when the call reads standard input on a buffer the model knows, which
/// the host would lend it, and stops at a byte it knows or at none. `None` where the call runs
/// as the code it is.
///
/// # Errors
///
/// Standard input of a length the input chooses, read other than all of it at once.
pub(super) fn whole(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
) -> Result<Option<Step<Wasm>>, End> {
    let core = &state.core;
    let Some(start) = core.stack.len().checked_sub(4) else {
        return Ok(None);
    };
    let known = |term: &Term| {
        pool.constant(*term)
            .and_then(|value| u64::try_from(value).ok())
    };
    let operands: Option<Vec<u64>> = core.stack[start..].iter().map(known).collect();
    let Some([fd, buffer, cap, stop]) =
        operands.and_then(|values| <[u64; 4]>::try_from(values).ok())
    else {
        return Ok(None);
    };
    // What `$fill` and the host check first, the code runs as it is: then neither reads.
    let lent = buffer >= FIRST_BUFFER
        && cap > 0
        && memory::inside(core, buffer, cap)
        && memory::inside(core, 0, SCRATCH);
    if fd != 0
        || !lent
        || !module.exports_memory
        || !module.policy.reads_input()
        || !matches!(core.fds.get(&0), Some(Handle::Stdin))
    {
        return Ok(None);
    }
    // A stop past a byte's values is none: no byte read is ever it.
    let stop = u8::try_from(stop).ok();
    let world = &state.world;
    let read = world.stdin_read.min(world.stdin.len());
    let mut available: Vec<Term> = world.stdin[read..].to_vec();
    let room = usize::try_from(cap).unwrap_or(usize::MAX);
    // Standard input of a length the input chooses, none of it read yet and all of it room for
    // the buffer: `$fill` takes all of it, its count that length. What comes before it, as the
    // buffer-full an induction's base puts first, is read as any input of its own length is.
    if let Some(length) = world.stdin_length
        && read < world.stdin.len()
    {
        let from = world.stdin_from;
        if read < from {
            available.truncate(from - read);
        } else if stop.is_some() || read > from || room < available.len() {
            return Err(unsupported(
                "standard input of a length the input chooses read other than all at once",
            ));
        } else {
            let everything = world.stdin.len();
            let count = pool.extract(length, 31, 0);
            take(state, pool, (start, buffer), &available, (everything, count));
            return Ok(Some(Step::Next));
        }
    }
    let last = available.len().min(room);
    // A read can end right after any stop byte that has arrived, and `$fill` stops there;
    // otherwise it reads on until its buffer is full or nothing is left.
    let mut outcomes: Vec<Outcome> = Vec::new();
    if let Some(stop) = stop {
        let wanted = pool.bv(8, u128::from(stop));
        for taken in 1..last {
            let stops = pool.eq(available[taken - 1], wanted);
            if pool.truth(stops) != Some(false) {
                outcomes.push((stops, taken));
            }
        }
    }
    let always = pool.boolean(true);
    outcomes.push((always, last));
    if let [(_, taken)] = outcomes.as_slice() {
        let count = pool.bv(32, *taken as u128);
        take(
            state,
            pool,
            (start, buffer),
            &available[..*taken],
            (read + taken, count),
        );
        return Ok(Some(Step::Next));
    }
    let mut choices = Vec::with_capacity(outcomes.len());
    for (condition, taken) in outcomes {
        let mut choice = state.clone();
        let count = pool.bv(32, taken as u128);
        take(
            &mut choice,
            pool,
            (start, buffer),
            &available[..taken],
            (read + taken, count),
        );
        // Where it stopped short of everything that had arrived: the cut a real run needs to
        // stop there too.
        if taken < last {
            choice.world.fill_cut.get_or_insert(read + taken);
        }
        choices.push((condition, choice));
    }
    Ok(Some(Step::Choose(choices)))
}
