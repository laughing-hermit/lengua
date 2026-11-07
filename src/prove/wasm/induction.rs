//! A WebAssembly run's part in a proof by induction over a read loop
//! ([`crate::prove::induction`]). The loop's head is the call of lengua's `$fill` in a loop of
//! main.wat's own code, where each time round starts its read ([`Module::read_loop_head`]). It
//! must be the run's first read, as every read passes fill's own loop's head in machine code: the
//! step comes to it with no input read, as a real run does only where nothing read before. The
//! slots ([`Slot`]) are the globals, then each call's locals, the oldest call's first, then the
//! operands, as the run holds them there: a time round leaves the calls under way as they were,
//! so each slot is the same place every time round.
//!
//! At the first arrival the run takes what one buffer-full can change as any value, from the
//! hook, and notes where it stands, the world's part in the world ([`Arrival`]) and the rest
//! here ([`Snapshot`]); at the second it notes it again; and a step's run ends there once it has
//! read its buffer-fulls.
//!
//! [`Module::read_loop_head`]: super::Module::read_loop_head

use std::rc::Rc;

use super::image::{Module, Op};
use super::{Label, Wasm};
use crate::prove::exec::induction::{Changes, Hook, Looping, Slot};
use crate::prove::exec::{End, Memory, State};
use crate::prove::term::{Pool, Term};
use crate::prove::world::Arrival;

/// What a run held the first time at the read loop's head beyond its slots, which the world keeps
/// with the rest of where the run stood: what a time round the loop is compared with.
#[derive(Debug, Clone)]
pub(super) struct Snapshot {
    /// Linear memory.
    memory: Memory,
    /// The calls under way, without their locals: each one's function, next operator, operands'
    /// height and blocks.
    calls: Vec<Call>,
    /// How many operands the stack held.
    operands: usize,
    /// The memory's pages.
    pages: u64,
    /// Whether each data segment was dropped.
    dropped: Vec<bool>,
    /// The descriptors the program held.
    descriptors: Vec<u32>,
    /// The bytes it had written to files.
    written: u64,
}

/// One call under way, without its locals.
type Call = (u32, usize, usize, Vec<Label>);

/// The head `at`, operator `at` of the function `function`, as a hook names it.
pub(super) fn head_of(function: u32, at: usize) -> u64 {
    (u64::from(function) << 32) | u64::try_from(at).unwrap_or(u64::from(u32::MAX))
}

/// Whether the newest call of `core` is about to run the operator `head` names.
pub(super) fn at_head(core: &Wasm, head: u64) -> bool {
    core.frames.last().is_some_and(|frame| {
        u64::from(frame.function) == head >> 32
            && u64::try_from(frame.pc).is_ok_and(|pc| pc == head & u64::from(u32::MAX))
    })
}

/// Every slot's value, in their order: the globals, each call's locals, the operands.
fn slots(core: &Wasm) -> Vec<Term> {
    let mut slots = core.globals.clone();
    for frame in &core.frames {
        slots.extend(&frame.locals);
    }
    slots.extend(&core.stack);
    slots
}

/// What `slot` holds in `core`, when the run has a place for it.
fn slot_value(core: &Wasm, slot: Slot) -> Option<Term> {
    let mut at = slot.0;
    if let Some(global) = core.globals.get(at) {
        return Some(*global);
    }
    at -= core.globals.len();
    for frame in &core.frames {
        if let Some(local) = frame.locals.get(at) {
            return Some(*local);
        }
        at -= frame.locals.len();
    }
    core.stack.get(at).copied()
}

/// The place `slot` is in `core`, to set.
fn slot_place(core: &mut Wasm, slot: Slot) -> Option<&mut Term> {
    let mut at = slot.0;
    if at < core.globals.len() {
        return core.globals.get_mut(at);
    }
    at -= core.globals.len();
    for frame in &mut core.frames {
        if at < frame.locals.len() {
            return frame.locals.get_mut(at);
        }
        at -= frame.locals.len();
    }
    core.stack.get_mut(at)
}

/// Sets `slot` to `value`, of the slot's own width.
///
/// # Errors
///
/// A slot the run has no place for, or a value of another width.
fn set_slot(pool: &Pool, core: &mut Wasm, slot: Slot, value: Term) -> Result<(), End> {
    let place = slot_place(core, slot)
        .ok_or_else(|| End::Unsupported(format!("the read loop's head has no slot {slot}")))?;
    if pool.width(*place) != pool.width(value) {
        return Err(End::Unsupported(format!(
            "slot {slot} at the read loop's head holds another width than its count's"
        )));
    }
    *place = value;
    Ok(())
}

/// The calls under way in `core`, without their locals.
fn calls(core: &Wasm) -> Vec<Call> {
    core.frames
        .iter()
        .map(|frame| (frame.function, frame.pc, frame.height, frame.labels.clone()))
        .collect()
}

/// Where the run stands, as the world keeps it at the read loop's head: what its slots hold,
/// how far it has read and written, and what it has made of the working folder.
fn arrival(state: &State<Wasm>) -> Arrival {
    state.world.arrival(slots(&state.core))
}

/// What the run holds beyond its slots, to compare a time round with.
fn snapshot(core: &Wasm) -> Snapshot {
    Snapshot {
        memory: core.memory.clone(),
        calls: calls(core),
        operands: core.stack.len(),
        pages: core.pages,
        dropped: core.dropped.clone(),
        descriptors: core.fds.keys().copied().collect(),
        written: core.written,
    }
}

/// The room `$fill`'s call at the head reads into, its third operand, and the descriptor, its
/// first, when the head is such a call and both are known.
fn read_at_head(module: &Module, core: &Wasm, pool: &Pool) -> Option<(u64, u64)> {
    let frame = core.frames.last()?;
    let fill = module.fill?;
    if module.function(frame.function)?.ops.get(frame.pc) != Some(&Op::Call(fill)) {
        return None;
    }
    let known = |from_top: usize| {
        let at = core.stack.len().checked_sub(from_top)?;
        let value = pool.constant(*core.stack.get(at)?)?;
        u64::try_from(value).ok()
    };
    Some((known(4)?, known(2)?))
}

/// Takes the read loop's head for `hook`: counts the arrival; the first time, feeds the step's
/// input, makes what one buffer-full can change any value, and notes where the run stands; the
/// second time, notes it again; and once the step's run has read its buffer-fulls, ends it there.
///
/// # Errors
///
/// Ends the step's run back at the head, and a run that read before it came there, or whose
/// head isn't a read of standard input through `$fill` the model can follow.
pub(super) fn arrive(
    module: &Module,
    state: &mut State<Wasm>,
    pool: &mut Pool,
    hook: &Hook,
) -> Result<(), End> {
    state.core.arrivals += 1;
    let arrivals = state.core.arrivals;
    if hook.stop && arrivals == hook.fulls() + 1 {
        return Err(End::Looped);
    }
    if arrivals == 2 {
        // One buffer-full on: where a count that looks back is checked from.
        if state.core.head.is_some() {
            let changed = if hook.fulls() > 1 {
                Some(Box::new(Wasm::changes(state)))
            } else {
                None
            };
            state.core.changed = changed;
            state.world.second = Some(Box::new(arrival(state)));
        }
        return Ok(());
    }
    if arrivals != 1 {
        return Ok(());
    }
    // The step comes to the head from the program's start with no input, so a read before it
    // got nothing, where a real run's got the input's first bytes: the step's state at the head
    // would be none a real run is in. In machine code every read passes fill's own loop's head.
    if state.core.filled {
        return Err(End::Unsupported(
            "a call of $fill before the read loop's head, which the step can't start from".into(),
        ));
    }
    let Some((fd, room)) = read_at_head(module, &state.core, pool) else {
        return Err(End::Unsupported(
            "a read loop's head that isn't a call of $fill on a known buffer".into(),
        ));
    };
    if fd != 0 {
        return Err(End::Unsupported(
            "a read loop that reads another descriptor than standard input".into(),
        ));
    }
    let space = usize::try_from(room).unwrap_or(usize::MAX);
    let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
    if let Some(characters) = &hook.feed
        && state.world.stdin_read == 0
    {
        let input = crate::prove::text_input(
            pool,
            "induction.chunk",
            space.saturating_mul(fulls).saturating_add(1),
            characters,
        );
        state.world.path.extend(input.constraints);
        state.world.stdin = Rc::new(input.text);
    }
    if let Some(characters) = &hook.lead {
        let lead = crate::prove::text_input(pool, "induction.lead", space, characters);
        let world = &mut state.world;
        // Before a text whose length the input chooses, the check holds every byte of standard
        // input to the characters, so the path needn't.
        if world.stdin_length.is_none() {
            world.path.extend(lead.constraints);
        }
        let read = world.stdin_read.min(world.stdin.len());
        if world.stdin_length.is_some() && read != world.stdin_from {
            return Err(End::Unsupported(
                "a text whose length the input chooses read in part before the read loop".into(),
            ));
        }
        let mut stdin = world.stdin[..read].to_vec();
        stdin.extend(lead.text);
        stdin.extend_from_slice(&world.stdin[read..]);
        if world.stdin_length.is_some() {
            world.stdin_from = read + space;
        }
        world.stdin = Rc::new(stdin);
    }
    for address in &hook.memory {
        let byte = pool.var(8, &format!("induction.m{address:x}"));
        state.core.memory.insert(*address, byte);
    }
    for slot in &hook.slots {
        let width = slot_value(&state.core, *slot).map_or(32, |value| pool.width(value));
        let value = pool.var(width, &format!("induction.r{slot}"));
        set_slot(pool, &mut state.core, *slot, value)?;
    }
    for (slot, start) in &hook.counters {
        let width = slot_value(&state.core, *slot).map_or(32, |value| pool.width(value));
        let value = pool.var_in(width, &format!("induction.c{slot}"), hook.low, *start);
        set_slot(pool, &mut state.core, *slot, value)?;
    }
    for (slot, fold) in &hook.folds {
        let value = fold.value(pool, *slot);
        set_slot(pool, &mut state.core, *slot, value)?;
    }
    // A phase's slot holds one of its values, each standing for a phase.
    for (slot, phase) in &hook.phases {
        let value = phase.value(pool, *slot);
        let known = phase.holds(pool, value);
        if pool.truth(known) != Some(true) {
            state.world.path.push(known);
        }
        set_slot(pool, &mut state.core, *slot, value)?;
    }
    state.core.head = Some(Box::new(snapshot(&state.core)));
    state.world.head = Some(Box::new(arrival(state)));
    Ok(())
}

/// WebAssembly's runs take the call of `$fill` in a read loop as the loop's head.
impl Looping for Wasm {
    fn held(state: &State<Self>, slot: Slot) -> Term {
        slot_value(&state.core, slot).unwrap_or(state.core.zero)
    }

    fn changes(state: &State<Self>) -> Changes {
        let mut changes = Changes::default();
        let (core, world) = (&state.core, &state.world);
        let (Some(start), Some(at_head)) = (core.head.as_deref(), world.head.as_deref()) else {
            return changes;
        };
        // The calls under way are the same ones, each at the same place: else a slot isn't the
        // same place from one time round to the next.
        if calls(core) != start.calls || core.stack.len() != start.operands {
            changes.other.push(
                "the calls under way differ from one time round the read loop to the next".into(),
            );
        }
        for (number, (now, then)) in slots(core).iter().zip(&at_head.slots).enumerate() {
            if now != then {
                changes.slots.insert(Slot(number));
            }
        }
        changes.memory = core.memory.differences(&start.memory);
        if core.pages != start.pages || core.dropped != start.dropped {
            changes
                .other
                .push("a time round the read loop grows the memory or drops a data segment".into());
        }
        let descriptors: Vec<u32> = core.fds.keys().copied().collect();
        if descriptors != start.descriptors
            || core.written != start.written
            || !world.written.is_empty()
            || !world.open.is_empty()
            || !world.sockets.is_empty()
        {
            changes
                .other
                .push("a time round the read loop opens files or connections".into());
        }
        // The working folder as a time round leaves it is what the next one finds there: a
        // folder removed the first time round is gone the second.
        if world.folder_changed(at_head) {
            changes
                .other
                .push("a time round the read loop changes the working folder".into());
        }
        if world.raw_reads {
            changes.other.push(
                "the read loop reads standard input with fd_read itself, not only through $fill"
                    .into(),
            );
        }
        changes
    }

    fn changed(state: &State<Self>) -> Option<&Changes> {
        state.core.changed.as_deref()
    }
}
