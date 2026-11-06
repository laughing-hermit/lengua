//! A run's part in a proof by induction over a streaming program's read loop, whose head is
//! fill's. The base, every input up to one buffer-full, runs with the head made arbitrary
//! wherever one more buffer-full can change it; the step runs one buffer-full from that same
//! arbitrary head back to it, where what changed is compared with where it started.
//!
//! What the induction follows from one time round to the next is kept in the core's slots
//! ([`Slot`]): a general register of the CPU's here. Where the run stood each time it reached
//! the head, as far as the checks of the run need, the world keeps ([`World::at_head`]); what
//! else the core held there, which a time round is compared with, the core keeps.
//!
//! [`World::at_head`]: crate::prove::world::World::at_head

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use super::{End, Last, Memory, Owner, Return, STACK_SIZE, STACK_TOP, State, VECTOR_BYTES};
use crate::prove::formula::Phase;
use crate::prove::term::{Pool, Term};
use crate::prove::world::Arrival;

/// A place a run's core keeps a value in, which a proof by induction follows from one time round
/// the read loop to the next, by its number among the core's places: for x86-64's and AArch64's
/// code, a general register, numbered as the prover numbers them, rax to r15 and x0 to x30 and
/// sp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slot(pub usize);

impl fmt::Display for Slot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// How a run takes the read loop's head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hook {
    /// fill's loop head.
    pub head: u64,
    /// The memory bytes one buffer-full can change, made arbitrary at the head.
    pub memory: BTreeSet<u64>,
    /// The slots it can change, other than the bound counters.
    pub slots: BTreeSet<Slot>,
    /// The vector registers it can change.
    pub vectors: BTreeSet<usize>,
    /// The mask registers it can change.
    pub masks: BTreeSet<usize>,
    /// The loop's bound counters, by slot, each with the value it starts at: made any value from
    /// `low` to that.
    pub counters: BTreeMap<Slot, u128>,
    /// The least a bound counter is at the head: enough for the loop to take what's left.
    pub low: u128,
    /// The counts the loop keeps as it goes, as `len` and `count` fold the text, by slot.
    pub folds: BTreeMap<Slot, Fold>,
    /// Where the loop keeps the matcher's phase of a count of a part that overlaps itself, by
    /// slot.
    pub phases: BTreeMap<Slot, PhaseFold>,
    /// Whether the run stops when it comes back to the head, as the step's does.
    pub stop: bool,
    /// How many buffer-fulls a stopping run reads from the head before it stops there: two for
    /// a count that needs the bytes before each buffer-full, whose second buffer-full is then
    /// checked after the first's last bytes. None given means one.
    pub fulls: u32,
    /// For the step: the characters of the input it reads from the head, fresh bytes one more
    /// than the buffer-fulls fill is given there, so the loop goes round again.
    pub feed: Option<Vec<u8>>,
    /// For the base of a count that needs the bytes before each buffer-full: the characters of
    /// a fresh buffer-full put before what's left of standard input at the head, so each final
    /// piece follows a whole buffer-full read from any state the loop can be in.
    pub lead: Option<Vec<u8>>,
}

impl Hook {
    /// The buffer-fulls a stopping run reads from the head.
    #[must_use]
    pub fn fulls(&self) -> u32 {
        self.fulls.max(1)
    }
}

/// A count a read loop keeps in a slot as it goes: at the head, what it starts at plus the count
/// of everything read so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fold {
    /// What the slot holds the first time at the head.
    pub start: u128,
    /// The most the count of what was read before the head can be.
    pub most: u128,
    /// The output's placeholder it counts for, by its place among the template's pieces.
    pub placeholder: usize,
}

impl Fold {
    /// The slot's value at the head, any from its start to its start and its most.
    pub fn value(&self, pool: &mut Pool, slot: Slot) -> Term {
        pool.var_in(
            64,
            &format!("induction.f{slot}"),
            self.start,
            self.start + self.most,
        )
    }
}

/// Where a read loop keeps the phase of a count of a part that can overlap itself: at the head,
/// one of the values the slot took the first time round, each standing for how much of the part
/// the text since its last match ends with. The step shows each time round keeps it so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseFold {
    /// The slot's values at the head, each with the matcher's state it stands for: how many
    /// bytes of the part the text since its last match ends with.
    pub states: BTreeMap<u128, usize>,
    /// The output's placeholder whose count it carries the phase of.
    pub placeholder: usize,
}

impl PhaseFold {
    /// The slot's value at the head: any from its least value to its largest, and one of them
    /// where [`Self::holds`] says so.
    pub fn value(&self, pool: &mut Pool, slot: Slot) -> Term {
        let low = self.states.keys().next().copied().unwrap_or(0);
        let high = self.states.keys().next_back().copied().unwrap_or(0);
        pool.var_in(64, &format!("induction.p{slot}"), low, high)
    }

    /// That `value` is one of the slot's values.
    pub fn holds(&self, pool: &mut Pool, value: Term) -> Term {
        let ways: Vec<Term> = self
            .states
            .keys()
            .map(|known| {
                let known = pool.bv(64, *known);
                pool.eq(value, known)
            })
            .collect();
        pool.any(&ways)
    }

    /// The matcher's phase `value` stands for, for a part of `length` bytes.
    pub fn phase(&self, pool: &mut Pool, value: Term, length: usize) -> Phase {
        let conditions = (1..length)
            .map(|matched| {
                let ways: Vec<Term> = self
                    .states
                    .iter()
                    .filter(|(_, state)| **state == matched)
                    .map(|(known, _)| {
                        let known = pool.bv(64, *known);
                        pool.eq(value, known)
                    })
                    .collect();
                pool.any(&ways)
            })
            .collect();
        Phase::of(conditions)
    }
}

/// What a run's core held the first time it reached the head, beyond its slots, which the world
/// keeps with the rest of where the run stood: what a time round the loop is compared with.
#[derive(Debug, Clone)]
pub struct Snapshot {
    memory: Memory,
    vectors: Vec<Rc<[Term; VECTOR_BYTES]>>,
    masks: [Term; 8],
    owners: [Option<Owner>; super::SLOTS],
    pointers: BTreeMap<u64, Owner>,
    returns: Vec<Return>,
    threads: u64,
    /// Where the stack's free part ends: the stack pointer, when it's known.
    stack: Option<u64>,
}

/// What differs between where a run stood at the head and where it stands back at it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// Memory bytes, other than the stack below the head's stack pointer, which is free.
    pub memory: BTreeSet<u64>,
    /// The core's slots: its general registers.
    pub slots: BTreeSet<Slot>,
    /// Vector registers.
    pub vectors: BTreeSet<usize>,
    /// Mask registers.
    pub masks: BTreeSet<usize>,
    /// What else differs, which no hook makes arbitrary: the calls under way, threads, files.
    pub other: Vec<String>,
}

impl Memory {
    /// Forgets every byte from `start` up to `end`, going through the pages that hold some.
    fn clear(&mut self, start: u64, end: u64) {
        let pages: Vec<u64> = self
            .pages
            .range(start / super::PAGE as u64..=end.saturating_sub(1) / super::PAGE as u64)
            .map(|(page, _)| *page)
            .collect();
        for page in pages {
            let base = page * super::PAGE as u64;
            if let Some(bytes) = self.pages.get_mut(&page) {
                let bytes = Rc::make_mut(bytes);
                for (offset, byte) in bytes.iter_mut().enumerate() {
                    let address = base + offset as u64;
                    if address >= start && address < end {
                        *byte = None;
                    }
                }
            }
        }
    }
}

/// The stack below `stack`, which holds nothing the run can read before writing it.
fn free_below(stack: Option<u64>, address: u64) -> bool {
    stack.is_some_and(|stack| address < stack && address >= STACK_TOP - STACK_SIZE)
}

impl State {
    /// Takes the read loop's head for `hook`: counts the arrival; the first time, feeds the
    /// step's input, makes what one buffer-full can change arbitrary, and notes where the run
    /// stands; the second time, notes it again; and once the step's run has read its
    /// buffer-fulls, ends it there.
    ///
    /// # Errors
    ///
    /// Ends the step's run back at the head.
    pub(super) fn arrive(&mut self, pool: &mut Pool, hook: &Hook) -> Result<(), End> {
        self.core.arrivals += 1;
        if hook.stop && self.core.arrivals == hook.fulls() + 1 {
            return Err(End::Looped);
        }
        if self.core.arrivals == 2 {
            // One buffer-full on: where a count that looks back is checked from.
            if self.core.head.is_some() {
                let changed = (hook.fulls() > 1).then(|| self.changes());
                self.core.changed = changed.map(Box::new);
                self.world.second = Some(Box::new(self.arrival()));
            }
            return Ok(());
        }
        if self.core.arrivals != 1 {
            return Ok(());
        }
        let constant = |pool: &Pool, term: Term| {
            pool.constant(term)
                .and_then(|value| usize::try_from(value).ok())
        };
        // Where fill keeps how much room its buffer has left.
        let room = super::fill::FillAbi::of(self.core.isa).space;
        if let Some(characters) = &hook.feed
            && self.world.stdin_read == 0
            && let Some(space) = constant(pool, self.core.registers[room.0])
        {
            let fulls = usize::try_from(hook.fulls()).unwrap_or(1);
            let input =
                crate::prove::text_input(pool, "induction.chunk", space * fulls + 1, characters);
            self.world.path.extend(input.constraints);
            self.world.stdin = Rc::new(input.text);
        }
        if let Some(characters) = &hook.lead
            && let Some(space) = constant(pool, self.core.registers[room.0])
        {
            let lead = crate::prove::text_input(pool, "induction.lead", space, characters);
            let world = &mut self.world;
            // Before a text whose length the input chooses, the check holds every byte of
            // standard input to the characters, so the path needn't: each branch would meet a
            // buffer-full of conditions.
            if world.stdin_length.is_none() {
                world.path.extend(lead.constraints);
            }
            let read = world.stdin_read.min(world.stdin.len());
            // A text whose length the input chooses is read all at once, after the lead.
            if world.stdin_length.is_some() && read != world.stdin_from {
                return Err(End::Unsupported(
                    "a text whose length the input chooses read in part before the read loop"
                        .into(),
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
        let core = &mut self.core;
        for address in &hook.memory {
            let byte = pool.var(8, &format!("induction.m{address:x}"));
            core.memory.insert(*address, byte);
            core.pointers.remove(address);
        }
        core.stored.clear();
        // The stack below the head's stack pointer holds what earlier calls left, which differs
        // from one time round to the next: nothing there can be read before it's written.
        let stack = pool
            .constant(core.registers[super::calls::Convention::of(core.isa).stack])
            .and_then(|value| u64::try_from(value).ok());
        if let Some(stack) = stack {
            let bottom = STACK_TOP - STACK_SIZE;
            core.memory.clear(bottom, stack);
            core.pointers.retain(|at, _| !free_below(Some(stack), *at));
        }
        for slot in &hook.slots {
            core.registers[slot.0] = pool.var(64, &format!("induction.r{slot}"));
            core.owners[slot.0] = None;
        }
        for (slot, start) in &hook.counters {
            core.registers[slot.0] =
                pool.var_in(64, &format!("induction.c{slot}"), hook.low, *start);
            core.owners[slot.0] = None;
        }
        for (slot, fold) in &hook.folds {
            core.registers[slot.0] = fold.value(pool, *slot);
            core.owners[slot.0] = None;
        }
        // A phase's slot holds one of its values, each standing for a phase.
        for (slot, phase) in &hook.phases {
            let value = phase.value(pool, *slot);
            let known = phase.holds(pool, value);
            if pool.truth(known) != Some(true) {
                self.world.path.push(known);
            }
            core.registers[slot.0] = value;
            core.owners[slot.0] = None;
        }
        for vector in &hook.vectors {
            let bytes: [Term; VECTOR_BYTES] =
                std::array::from_fn(|at| pool.var(8, &format!("induction.v{vector}.{at}")));
            core.vectors[*vector] = Rc::new(bytes);
        }
        for mask in &hook.masks {
            core.masks[*mask] = pool.var(64, &format!("induction.k{mask}"));
        }
        // The flags don't outlive the call that reaches the head.
        core.carry = pool.bool_var("induction.cf");
        core.zero = pool.bool_var("induction.zf");
        core.sign = pool.bool_var("induction.sf");
        core.overflow = pool.bool_var("induction.of");
        core.parity = pool.bool_var("induction.pf");
        core.last = Last::Other;
        core.head = Some(Box::new(Snapshot {
            memory: core.memory.clone(),
            vectors: core.vectors.clone(),
            masks: core.masks,
            owners: core.owners,
            pointers: core.pointers.clone(),
            returns: core.returns.clone(),
            threads: core.threads,
            stack,
        }));
        self.world.head = Some(Box::new(self.arrival()));
        Ok(())
    }

    /// Where the run stands, as the world keeps it at the read loop's head: what its slots hold,
    /// and how far it has read and written.
    fn arrival(&self) -> Arrival {
        Arrival {
            slots: self.core.registers.to_vec(),
            stdout: self.world.stdout.len(),
            stderr: self.world.stderr.len(),
            stdin_read: self.world.stdin_read,
        }
    }

    /// What differs between where the run stood the first time at the head and where it stands
    /// now: nothing, for a run that never reached the head.
    #[must_use]
    pub fn changes(&self) -> Changes {
        let mut changes = Changes::default();
        let start = self.core.head.as_deref();
        let (Some(start), Some(at_head)) = (start, self.world.head.as_deref()) else {
            return changes;
        };
        let (core, world) = (&self.core, &self.world);
        // The calls under way are the same ones, each a frame of its own: a frame made since
        // stands where the one it replaces stood.
        let same_calls = core.returns.len() == start.returns.len()
            && core.returns.iter().zip(&start.returns).all(|(now, then)| {
                (now.top, now.saved, now.back, now.callee)
                    == (then.top, then.saved, then.back, then.callee)
            });
        if !same_calls {
            changes.other.push(
                "the calls under way differ from one time round the read loop to the next".into(),
            );
        }
        let frame = |owner: Option<Owner>, returns: &[Return]| match owner {
            Some(Owner::Frame(number)) => returns
                .iter()
                .position(|back| back.frame == number)
                .map_or(Some(Owner::Frame(number)), |depth| {
                    Some(Owner::Frame(u64::MAX - depth as u64))
                }),
            other => other,
        };
        let same_owner = |now: Option<Owner>, then: Option<Owner>| {
            frame(now, &core.returns) == frame(then, &start.returns)
        };
        let stack = super::calls::Convention::of(core.isa).stack;
        for (number, (now, then)) in core.registers.iter().zip(&at_head.slots).enumerate() {
            if now != then
                || (number != stack && !same_owner(core.owners[number], start.owners[number]))
            {
                changes.slots.insert(Slot(number));
            }
        }
        for (number, (now, then)) in core.vectors.iter().zip(&start.vectors).enumerate() {
            if !Rc::ptr_eq(now, then) && now[..] != then[..] {
                changes.vectors.insert(number);
            }
        }
        for (number, (now, then)) in core.masks.iter().zip(&start.masks).enumerate() {
            if now != then {
                changes.masks.insert(number);
            }
        }
        let free = |address: u64| free_below(start.stack, address);
        let pages: BTreeSet<u64> = core
            .memory
            .pages
            .keys()
            .chain(start.memory.pages.keys())
            .copied()
            .collect();
        for page in pages {
            let (now, then) = (core.memory.pages.get(&page), start.memory.pages.get(&page));
            if let (Some(now), Some(then)) = (now, then)
                && Rc::ptr_eq(now, then)
            {
                continue;
            }
            for offset in 0..super::PAGE as u64 {
                let address = page * super::PAGE as u64 + offset;
                if core.memory.get(address) != start.memory.get(address) && !free(address) {
                    changes.memory.insert(address);
                }
            }
        }
        // A pointer stored in memory keeps its owner, or its bytes count as changed.
        let stored: BTreeSet<u64> = core
            .pointers
            .keys()
            .chain(start.pointers.keys())
            .copied()
            .collect();
        for address in stored {
            let (now, then) = (core.pointers.get(&address), start.pointers.get(&address));
            if !free(address) && !same_owner(now.copied(), then.copied()) {
                changes.memory.extend(address..address + 8);
            }
        }
        if core.threads != start.threads || !core.suspended.is_empty() {
            changes
                .other
                .push("a time round the read loop starts threads".into());
        }
        if !world.written.is_empty() || !world.open.is_empty() || !world.sockets.is_empty() {
            changes
                .other
                .push("a time round the read loop opens files or connections".into());
        }
        if world.raw_reads {
            changes
                .other
                .push("the read loop reads standard input with read itself, not only fill".into());
        }
        if !core.logs.is_empty() {
            changes.other.push(
                "a time round the read loop reaches memory the input chooses among more places \
                 than a proof follows one by one"
                    .into(),
            );
        }
        changes
    }

    /// What one buffer-full changed, by the second time the run reached the head, for a run
    /// that reads two from it.
    #[must_use]
    pub fn changed(&self) -> Option<&Changes> {
        self.core.changed.as_deref()
    }

    /// What `slot` holds now.
    #[must_use]
    pub const fn held(&self, slot: Slot) -> Term {
        self.core.registers[slot.0]
    }
}
