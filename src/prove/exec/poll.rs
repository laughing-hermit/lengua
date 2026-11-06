//! A server's wait with poll on its listener and connections, as x86-64's and AArch64's code
//! makes it: every way the test's clients' steps can come before the poll comes back, each its
//! own run ([`World::polls`]), and the key a run that comes back is known by, so that a state
//! two runs come to is followed once.
//!
//! [`World::polls`]: crate::prove::world::World::polls

use super::{Cpu, Last, Owner, POLLFD_LEN, POLLIN, State, Step, Stop, unsupported};
use crate::prove::term::Term;

/// The event poll is asked for besides something to read: the peer closing its side.
const POLLRDHUP: u128 = 0x2000;

impl Cpu<'_> {
    /// `poll` for a server: waits until the listener or one of the connections it watches has
    /// something, the clients taking any of the steps open to them first, each way its own run.
    /// Runs that come to the same state are followed once.
    pub(super) fn poll_network(&mut self, table: u64, entries: u64) -> Result<Step, Stop> {
        self.world.start_polling()?;
        let mut watched = Vec::new();
        for entry in 0..entries {
            let at = table + entry * POLLFD_LEN;
            let fd = self.load(at, 4)?;
            let events = self.load(at + 4, 2)?;
            let (Some(fd), Some(events)) = (self.pool.constant(fd), self.pool.constant(events))
            else {
                return Err(unsupported("poll over descriptors the input chooses"));
            };
            let fd = i64::from(fd as u32 as i32);
            if events & !(POLLIN | POLLRDHUP) != 0 && fd >= 0 {
                return Err(unsupported(format!("poll for events {events:#x}")));
            }
            watched.push((at, fd, events));
        }
        let states: Vec<State> = self
            .world
            .polls()
            .into_iter()
            .map(|world| State {
                core: self.core.clone(),
                world,
            })
            .collect();
        let after = self.next;
        let mut choices = Vec::new();
        let mut any = false;
        for mut state in states {
            let mut ready = 0;
            let mut revents = Vec::new();
            for (at, fd, events) in &watched {
                let has = *fd >= 0 && events & POLLIN != 0 && state.world.readable(*fd as u64);
                ready += u64::from(has);
                revents.push((*at, if has { POLLIN } else { 0 }));
            }
            if ready == 0 {
                continue;
            }
            any = true;
            self.swap(&mut state);
            for (at, value) in revents {
                let value = self.pool.bv(16, value);
                self.save(at + 6, value)?;
            }
            self.core.returns(self.pool, ready, after);
            self.core.rip = after;
            self.swap(&mut state);
            let advanced = state.world.peers != self.world.peers;
            let key = state.poll_key();
            if state.world.come_back(key, advanced) {
                choices.push((self.pool.boolean(true), state));
            }
        }
        // Nothing it waits on can come: it waits forever. Every way that can come already
        // followed from another run: this one goes no further.
        if !any {
            return Err(self.world.idle());
        }
        Ok(Step::Choose(choices))
    }
}

impl State {
    /// Everything about the state that what follows depends on, but how many instructions and
    /// calls it took to get there; `None` for a run with files or threads in hand, which poll
    /// always follows.
    fn poll_key(&self) -> Option<Vec<u64>> {
        let (core, world) = (&self.core, &self.world);
        if !world.written.is_empty()
            || !world.open.is_empty()
            || !world.made.is_empty()
            || !world.removed.is_empty()
            || !core.suspended.is_empty()
        {
            return None;
        }
        let none = u64::MAX;
        let term = |term: Term| term.index() as u64;
        let owner = |owner: Option<Owner>| match owner {
            None => none,
            Some(Owner::Symbol(place)) => 1 << 60 | place as u64,
            Some(Owner::Frame(number)) => 2 << 60 | number,
            Some(Owner::Argument(place)) => 3 << 60 | place as u64,
        };
        let mut key = vec![core.rip];
        key.extend(core.registers.iter().map(|value| term(*value)));
        key.extend(
            [core.carry, core.zero, core.sign, core.overflow, core.parity]
                .iter()
                .map(|flag| term(*flag)),
        );
        key.extend(match core.last {
            Last::Compare(a, b) => [0, term(a), term(b)],
            Last::Logic(result) => [1, term(result), 0],
            Last::Other => [2, 0, 0],
        });
        for vector in &core.vectors {
            key.extend(vector.iter().map(|byte| term(*byte)));
        }
        key.extend(core.masks.iter().map(|mask| term(*mask)));
        for (page, bytes) in &core.memory.pages {
            key.push(*page);
            key.extend(bytes.iter().map(|byte| byte.map_or(none, term)));
        }
        key.push(core.stored.len() as u64);
        for (address, value) in &core.stored {
            key.extend([*address, term(*value)]);
        }
        key.push(core.logs.len() as u64);
        for log in core.logs.iter() {
            key.extend([log.start, log.end, term(log.common)]);
            for (address, byte) in &log.others {
                key.extend([*address, term(*byte)]);
            }
            key.push(log.entries.len() as u64);
            for (address, byte) in &log.entries {
                key.extend([term(*address), term(*byte)]);
            }
        }
        key.push(world.path.len() as u64);
        key.extend(world.path.iter().map(|condition| term(*condition)));
        key.push(world.stdin_read as u64);
        for output in [&world.stdout, &world.stderr] {
            key.push(output.len() as u64);
            key.extend(output.iter().map(|byte| term(*byte)));
        }
        key.push(world.connections.len() as u64);
        for connection in &world.connections {
            key.extend([
                connection.exchange as u64,
                u64::from(connection.accepted),
                u64::from(connection.finished),
                connection.piece as u64,
                connection.taken as u64,
                connection.sent.len() as u64,
            ]);
            key.extend(connection.sent.iter().map(|byte| term(*byte)));
        }
        key.push(world.sockets.len() as u64);
        for (fd, number) in &world.sockets {
            key.extend([*fd, number.map_or(none, |number| number as u64)]);
        }
        for peer in world.peers.iter().flatten() {
            key.extend([peer.steps as u64, u64::from(peer.closed)]);
        }
        key.push(core.returns.len() as u64);
        for back in &core.returns {
            key.extend([back.top, back.back, back.callee, back.frame]);
        }
        key.extend(core.owners.iter().map(|held| owner(*held)));
        key.push(core.pointers.len() as u64);
        for (address, held) in &core.pointers {
            key.extend([*address, owner(Some(*held))]);
        }
        key.push(world.obligations.len() as u64);
        key.extend(
            world
                .obligations
                .iter()
                .map(|(condition, _)| term(*condition)),
        );
        key.extend([
            u64::from(world.raw_reads),
            world.fill_cut.map_or(none, |cut| cut as u64),
            u64::from(world.raw_cuts),
            core.threads,
            core.stack_top.unwrap_or(none),
            core.base_frame,
            u64::from(core.arrivals),
            world.random_used as u64,
            world.readings_used as u64,
        ]);
        Some(key)
    }
}
