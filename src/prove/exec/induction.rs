//! A run's part in a proof by induction over a streaming program's read loop, whose head is its
//! call of `$fill`. The base, every input up to one buffer-full, runs with the head made
//! arbitrary wherever one more buffer-full can change it; the step runs one buffer-full from that
//! same arbitrary head back to it, where what changed is compared with where it started.
//!
//! What the induction follows from one time round to the next is kept in the core's slots
//! ([`Slot`]): a local, a global or an operand of WebAssembly's. Where the run stood each time it
//! reached the head, as far as the checks of the run need, the world keeps
//! ([`World::at_head`]), the one part every core shares; what else the core held there, which a
//! time round is compared with, and how it takes the head, each core keeps its own way
//! ([`Looping`]).
//!
//! [`World::at_head`]: crate::prove::world::World::at_head

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::{Core, State};
use crate::prove::formula::Phase;
use crate::prove::term::{Pool, Term};

/// A place a run's core keeps a value in, which a proof by induction follows from one time round
/// the read loop to the next, by its number among the core's places.
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
    /// The read loop's head.
    pub head: u64,
    /// The memory bytes one buffer-full can change, made arbitrary at the head.
    pub memory: BTreeSet<u64>,
    /// The slots it can change, other than the bound counters.
    pub slots: BTreeSet<Slot>,
    /// The slots it can change only within a range, as a byte carried from one buffer-full to
    /// the next stays a byte: each with its least and its most, made any value between them at
    /// the head. The step shows a time round from such values ends within them.
    pub ranged: BTreeMap<Slot, (u128, u128)>,
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
    /// How many bits the slot holds: a WebAssembly local's own width.
    pub width: u32,
}

impl Fold {
    /// The slot's value at the head, any from its start to its start and its most.
    pub fn value(&self, pool: &mut Pool, slot: Slot) -> Term {
        pool.var_in(
            self.width,
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
    /// How many bits the slot holds.
    pub width: u32,
}

impl PhaseFold {
    /// The slot's value at the head: any from its least value to its largest, and one of them
    /// where [`Self::holds`] says so.
    pub fn value(&self, pool: &mut Pool, slot: Slot) -> Term {
        let low = self.states.keys().next().copied().unwrap_or(0);
        let high = self.states.keys().next_back().copied().unwrap_or(0);
        pool.var_in(self.width, &format!("induction.p{slot}"), low, high)
    }

    /// That `value` is one of the slot's values.
    pub fn holds(&self, pool: &mut Pool, value: Term) -> Term {
        let ways: Vec<Term> = self
            .states
            .keys()
            .map(|known| {
                let known = pool.bv(self.width, *known);
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
                        let known = pool.bv(self.width, *known);
                        pool.eq(value, known)
                    })
                    .collect();
                pool.any(&ways)
            })
            .collect();
        Phase::of(conditions)
    }
}

/// A core whose runs take a read loop's head for a proof by induction, each core its own way:
/// what its slots hold, and what a time round the loop changed. Where the run stood at the head
/// ([`Arrival`](crate::prove::world::Arrival)) is the world's, shared by every core; the rest is
/// the core's.
pub trait Looping: Core {
    /// What `slot` holds in `state` now.
    fn held(state: &State<Self>, slot: Slot) -> Term;

    /// What differs between where `state` stood the first time at the head and where it stands
    /// now: nothing, for a run that never reached the head.
    fn changes(state: &State<Self>) -> Changes;

    /// What one buffer-full changed, by the second time `state` reached the head, for a run that
    /// reads two from it.
    fn changed(state: &State<Self>) -> Option<&Changes>;
}

/// A run's state, of any core that takes a read loop's head, as a proof by induction reads it.
pub trait Held {
    /// What `slot` holds now.
    fn held(&self, slot: Slot) -> Term;

    /// What differs between where the run stood the first time at the head and where it stands
    /// now.
    fn changes(&self) -> Changes;

    /// What one buffer-full changed, by the second time at the head.
    fn changed(&self) -> Option<&Changes>;
}

impl<C: Looping> Held for State<C> {
    fn held(&self, slot: Slot) -> Term {
        C::held(self, slot)
    }

    fn changes(&self) -> Changes {
        C::changes(self)
    }

    fn changed(&self) -> Option<&Changes> {
        C::changed(self)
    }
}

/// What differs between where a run stood at the head and where it stands back at it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// Memory bytes.
    pub memory: BTreeSet<u64>,
    /// The core's slots.
    pub slots: BTreeSet<Slot>,
    /// What else differs, which no hook makes arbitrary: the calls under way, files and the
    /// working folder.
    pub other: Vec<String>,
}
