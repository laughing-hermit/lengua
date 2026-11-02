//! Memory an address the input chooses reaches among more places than a run follows one by one,
//! as a hash table's slots are: the object, a data symbol, keeps a log of its stores from then
//! on over its bytes as they were, and a load from it is the choice among them, newest first.
//! Every access to the object goes through its log from then on, at any address.

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use super::{Cpu, End, Held, MAX_PLACES, Object, Owner, PAGE, State, Stop, Through, unsupported};
use crate::prove::term::{Pool, Term};

/// The most bytes of an object, as its log begins, that may hold other than its common value.
const MAX_OTHERS: usize = 4096;

/// The most stores a log follows.
const MAX_ENTRIES: usize = 4096;

/// A data symbol whose bytes an address the input chooses reached: its stores since, oldest
/// first, over its bytes as they were.
#[derive(Debug, Clone)]
pub struct Log {
    /// The object's first address.
    pub start: u64,
    /// The address past its last.
    pub end: u64,
    /// The value most of its bytes held when the log began.
    pub common: Term,
    /// The bytes that held another value then, by address.
    pub others: BTreeMap<u64, Term>,
    /// The stores since, each byte's address and value, oldest first.
    pub entries: Vec<(Term, Term)>,
}

impl Log {
    /// The byte at `address`: the newest store that may be there, then what the object held
    /// there when the log began.
    fn load(&self, pool: &mut Pool, address: Term) -> Term {
        let mut value = match pool.constant(address).and_then(|at| u64::try_from(at).ok()) {
            Some(at) => self.others.get(&at).copied().unwrap_or(self.common),
            None => {
                let mut value = self.common;
                for (at, byte) in &self.others {
                    let here = pool.bv(64, u128::from(*at));
                    let chosen = pool.eq(address, here);
                    value = pool.ite(chosen, *byte, value);
                }
                value
            }
        };
        for (at, byte) in &self.entries {
            // Addresses a few bytes apart from two aligned ones never meet.
            if pool.apart(address, *at) {
                continue;
            }
            let chosen = pool.eq(address, *at);
            match pool.truth(chosen) {
                Some(true) => value = *byte,
                Some(false) => {}
                None => value = pool.ite(chosen, *byte, value),
            }
        }
        value
    }

    /// Stores `byte` at `address`: a store at the same address before it, which no load can see
    /// any more, goes.
    fn store(&mut self, address: Term, byte: Term) {
        self.entries.retain(|(at, _)| *at != address);
        self.entries.push((address, byte));
    }
}

impl State {
    /// The log of the object holding `address`, where it has one.
    pub(super) fn log_at(&self, address: u64) -> Option<usize> {
        self.logs
            .iter()
            .position(|log| (log.start..log.end).contains(&address))
    }

    /// The log `bytes` bytes at `address` all fall in, where any does.
    ///
    /// # Errors
    ///
    /// An access only partly inside a logged object.
    pub(super) fn log_span(&self, address: u64, bytes: u32) -> Result<Option<usize>, Stop> {
        if self.logs.is_empty() {
            return Ok(None);
        }
        let last = address.wrapping_add(u64::from(bytes.max(1)) - 1);
        let (first, end) = (self.log_at(address), self.log_at(last));
        if first != end
            || (first.is_none()
                && self
                    .logs
                    .iter()
                    .any(|log| address < log.start && last >= log.start))
        {
            return Err(unsupported(
                "an access partly inside memory the input chooses where",
            ));
        }
        Ok(first)
    }

    /// The byte at `address` in log `index`.
    pub(super) fn logged(&self, pool: &mut Pool, index: usize, address: Term) -> Term {
        self.logs[index].load(pool, address)
    }

    /// Stores `byte` at `address` in log `index`.
    pub(super) fn log_store(&mut self, index: usize, address: Term, byte: Term) {
        Rc::make_mut(&mut self.logs)[index].store(address, byte);
    }

    /// `value` stored at `address`, byte by byte, where some of its bytes fall in a log: those
    /// go to their log, the rest to memory. Returns whether any did.
    pub(super) fn store_logged(&mut self, pool: &mut Pool, address: u64, value: Term) -> bool {
        if self.logs.is_empty() {
            return false;
        }
        let bytes = pool.width(value) / 8;
        let logged: Vec<Option<usize>> = (0..u64::from(bytes))
            .map(|offset| self.log_at(address.wrapping_add(offset)))
            .collect();
        if logged.iter().all(Option::is_none) {
            return false;
        }
        for (offset, log) in logged.into_iter().enumerate() {
            let at = address.wrapping_add(offset as u64);
            let shift = u32::try_from(offset).unwrap_or(0) * 8;
            let part = pool.extract(value, shift + 7, shift);
            match log {
                Some(index) => {
                    let address = pool.bv(64, u128::from(at));
                    self.log_store(index, address, part);
                }
                None => self.memory.insert(at, part),
            }
        }
        true
    }
}

impl Cpu<'_> {
    /// Where an access at `address`, which the input chooses, reaches more places than a run
    /// follows one by one, the data symbol it's held to, which its log then follows; `None`
    /// where it reaches few enough. Its staying inside the symbol becomes a question for the
    /// solvers.
    ///
    /// # Errors
    ///
    /// An access too spread out that isn't held to a data symbol, or a write to one that's
    /// read-only.
    pub(super) fn spread(
        &mut self,
        address: Term,
        bytes: u32,
        writing: bool,
        through: Through,
    ) -> Result<Option<Object>, Stop> {
        let (low, high) = self.pool.range(address);
        let (step, _) = self.pool.stride(address);
        if (high - low) / step < MAX_PLACES {
            return Ok(None);
        }
        let too_many = || unsupported("an address that depends on the input in more than 256 ways");
        let Some(Held::Owner(owner @ Owner::Symbol(_))) = self.held_through(through) else {
            return Err(too_many());
        };
        let object = self.owned(owner).ok_or_else(too_many)?;
        match self.image.region(object.start) {
            Some(region) if region.writable || !writing => {}
            _ => {
                return Err(End::Unsafe(format!(
                    "{} writes {}, which is read-only, where the input chooses",
                    self.doing(),
                    object.name
                )));
            }
        }
        if self.state.threads > 0 {
            return Err(unsupported(
                "memory the input chooses where, while threads run",
            ));
        }
        // Inside the object, every byte of the access.
        let size = object.end - object.start;
        let reaches = u64::from(bytes);
        let problem = format!(
            "for some input, {} {} {reaches} byte{} outside {}, which has {size} bytes",
            self.doing(),
            if writing { "writes" } else { "reads" },
            if reaches == 1 { "" } else { "s" },
            object.name
        );
        let outside = if size < reaches {
            self.pool.boolean(true)
        } else {
            let first = self.pool.bv(64, u128::from(object.start));
            let last = self.pool.bv(64, u128::from(object.end - reaches));
            let after = self.pool.ule(first, address);
            let before = self.pool.ule(address, last);
            let inside = self.pool.bool_and(after, before);
            self.pool.bool_not(inside)
        };
        if self.pool.truth(outside) != Some(false) {
            self.state.obligations.push((outside, problem));
        }
        Ok(Some(object))
    }

    /// The log of `object`, begun where it has none: its bytes as they stand, then no stores.
    ///
    /// # Errors
    ///
    /// An object too full of different bytes to follow.
    pub(super) fn log_of(&mut self, object: &Object) -> Result<usize, Stop> {
        if let Some(index) = self
            .state
            .logs
            .iter()
            .position(|log| log.start == object.start)
        {
            if self.state.logs[index].entries.len() > MAX_ENTRIES {
                return Err(unsupported(
                    "more stores where the input chooses than a proof follows",
                ));
            }
            return Ok(index);
        }
        let (start, end) = (object.start, object.end);
        // What its bytes hold: what was written there, else what the image holds, else zero.
        let mut held: HashMap<u64, Term> = HashMap::new();
        for (at, byte) in &self.image.initial {
            if (start..end).contains(at) {
                held.insert(*at, self.pool.bv(8, u128::from(*byte)));
            }
        }
        let first_page = start / PAGE as u64;
        let last_page = (end - 1) / PAGE as u64;
        for (page, bytes) in self.state.memory.pages.range(first_page..=last_page) {
            for (offset, byte) in bytes.iter().enumerate() {
                let at = page * PAGE as u64 + offset as u64;
                if let Some(byte) = byte
                    && (start..end).contains(&at)
                {
                    held.insert(at, *byte);
                }
            }
        }
        let zero = self.pool.bv(8, 0);
        let common = if (held.len() as u64) < end - start {
            zero
        } else {
            let mut counts: HashMap<Term, usize> = HashMap::new();
            for byte in held.values() {
                *counts.entry(*byte).or_default() += 1;
            }
            counts
                .into_iter()
                .max_by_key(|(byte, count)| (*count, std::cmp::Reverse(*byte)))
                .map_or(zero, |(byte, _)| byte)
        };
        let others: BTreeMap<u64, Term> = held
            .into_iter()
            .filter(|(_, byte)| *byte != common)
            .collect();
        if others.len() > MAX_OTHERS {
            return Err(unsupported(
                "a table the input indexes that holds too many different bytes to follow",
            ));
        }
        // The values stored whole, and the pointers, in it are its log's now.
        self.state.stored.retain(|at, value| {
            *at >= end || *at + u64::from(self.pool.width(*value) / 8) <= start
        });
        self.state
            .pointers
            .retain(|at, _| *at >= end || *at + 8 <= start);
        Rc::make_mut(&mut self.state.logs).push(Log {
            start,
            end,
            common,
            others,
            entries: Vec::new(),
        });
        Ok(self.state.logs.len() - 1)
    }

    /// The `bytes` bytes at `address` in log `index`, the lowest first.
    pub(super) fn logged_value(&mut self, index: usize, address: Term, bytes: u32) -> Term {
        let mut value: Option<Term> = None;
        for offset in 0..bytes {
            let shift = self.pool.bv(64, u128::from(offset));
            let at = self.pool.add(address, shift);
            let byte = self.state.logged(self.pool, index, at);
            value = Some(match value {
                None => byte,
                Some(low) => self.pool.concat(byte, low),
            });
        }
        value.unwrap_or(self.pool.bv(8, 0))
    }

    /// Stores `value` at `address` in log `index`, byte by byte, the lowest first.
    pub(super) fn log_value(&mut self, index: usize, address: Term, value: Term) {
        let bytes = self.pool.width(value) / 8;
        for offset in 0..bytes {
            let shift = self.pool.bv(64, u128::from(offset));
            let at = self.pool.add(address, shift);
            let part = self.pool.extract(value, offset * 8 + 7, offset * 8);
            self.state.log_store(index, at, part);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use rand::rngs::ChaCha8Rng;
    use rand::{RngExt, SeedableRng};

    use super::Log;
    use crate::prove::term::{Pool, Term};

    /// Stores and loads at addresses the input chooses, read back through a log, give what a
    /// plain array holds after the same stores, for random addresses, values and orders.
    #[test]
    fn a_log_reads_back_what_was_stored() {
        let mut rng = ChaCha8Rng::seed_from_u64(0x10c5_10c5);
        let (start, size) = (0x4000_u64, 48_u64);
        for _ in 0..300 {
            let mut pool = Pool::default();
            // Four indices the input chooses, each to a byte or an aligned word of the object.
            let indices: Vec<Term> = (0..4)
                .map(|number| pool.var_in(64, &format!("i{number}"), 0, u128::from(size / 4 - 1)))
                .collect();
            let values: Vec<Term> = (0..4)
                .map(|number| pool.var(8, &format!("v{number}")))
                .collect();
            let base = pool.bv(64, u128::from(start));
            let four = pool.bv(64, 4);
            let mut log = Log {
                start,
                end: start + size,
                common: pool.bv(8, 0),
                others: BTreeMap::from([(start + 5, pool.bv(8, 0x5a))]),
                entries: Vec::new(),
            };
            let mut assignment: HashMap<String, u128> = HashMap::new();
            for number in 0..4 {
                assignment.insert(format!("i{number}"), rng.random_range(0..size / 4).into());
                assignment.insert(format!("v{number}"), rng.random_range(0..256));
            }
            let mut array = vec![0_u8; usize::try_from(size).expect("small")];
            array[5] = 0x5a;
            // An address at a chosen index, word-aligned, plus a byte into the word.
            let address = |pool: &mut Pool, index: usize, byte: u64| {
                let scaled = pool.mul(indices[index], four);
                let at = pool.add(base, scaled);
                let byte = pool.bv(64, u128::from(byte));
                pool.add(at, byte)
            };
            let concrete =
                |index: usize, byte: u64| (assignment[&format!("i{index}")] as u64) * 4 + byte;
            for _ in 0..rng.random_range(1..8) {
                let (index, byte) = (rng.random_range(0..4), rng.random_range(0..4));
                let at = address(&mut pool, index, byte);
                if rng.random_bool(0.5) {
                    let value = rng.random_range(0..4);
                    log.store(at, values[value]);
                    array[usize::try_from(concrete(index, byte)).expect("small")] =
                        assignment[&format!("v{value}")] as u8;
                } else {
                    let loaded = log.load(&mut pool, at);
                    let expected = array[usize::try_from(concrete(index, byte)).expect("small")];
                    assert_eq!(
                        pool.evaluate(loaded, &assignment),
                        Some(u128::from(expected)),
                        "{assignment:?}"
                    );
                }
            }
            // Every byte at the end, at constant addresses too.
            for offset in 0..size {
                let at = pool.bv(64, u128::from(start + offset));
                let loaded = log.load(&mut pool, at);
                assert_eq!(
                    pool.evaluate(loaded, &assignment),
                    Some(u128::from(array[usize::try_from(offset).expect("small")])),
                    "byte {offset} of {assignment:?}"
                );
            }
        }
    }
}
