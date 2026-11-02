//! The network as a test sets it up: its own servers, each on one of the connections lengua made
//! for the program before it started, or its own client, which connects to the program's
//! listener. Each peer behaves as the test runner's does: the server reads exactly what it must
//! receive before it replies, and the client sends, closes its side, and waits for the program
//! to close. What the peer sends arrives in the pieces the run's delivery cuts it into.

use std::rc::Rc;

use super::{Cpu, End, Step, Stop, unsupported};
use crate::prove::term::Term;

/// The test's network for one run.
#[derive(Debug, Clone, Default)]
pub struct Network {
    /// The test server's connections in order: what it must receive, and its reply in the
    /// pieces it arrives in.
    pub server: Vec<(Vec<Term>, Vec<Vec<Term>>)>,
    /// The connections lengua makes before the program starts, at descriptors 4 and on in
    /// order: each the place in `server` of the test's server that answers it, or `None` for
    /// one handed over already ended.
    pub made: Vec<Option<usize>>,
    /// The test client's connections in order.
    pub client: Vec<Client>,
    /// Whether the program has its listener at file descriptor 3.
    pub listening: bool,
    /// Whether the test's clients connect at once, each right after the one before, rather
    /// than each once the one before has its answer.
    pub at_once: bool,
    /// Whether a server that polls sees one way the clients' steps can come, each client taking
    /// every step open to it before each poll, as for a test's sample, rather than every way.
    pub eager: bool,
}

/// One connection of the test's client, which behaves as curl and browsers do: it sends, keeps
/// its side open until the program has started answering, then closes it and reads the rest.
#[derive(Debug, Clone, Default)]
pub struct Client {
    /// What it sends, in the pieces it arrives in.
    pub pieces: Vec<Vec<Term>>,
    /// Whether it waits for an answer before closing its side: the test expects one.
    pub waits: bool,
    /// Whether its last piece waits until every client after it has its whole answer, as the
    /// test's clients at once hold their last byte.
    pub held: bool,
}

/// The states a run came back from poll in, the latest first.
#[derive(Debug)]
pub struct History {
    key: Vec<u64>,
    before: Option<Rc<History>>,
}

/// Where one of the test's clients stands, for a server that waits on its connections with
/// poll: the steps it has taken, connecting and then each piece of what it sends arriving, and
/// whether it has closed its side. Between two polls each client takes any of the steps open
/// to it, so the runs from one poll cover every order the clients' steps can come in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Peer {
    /// 0 before it connects, 1 once it has, and one more for each piece that has arrived.
    pub steps: usize,
    /// Whether it has closed its side.
    pub closed: bool,
}

/// One connection as the program used it.
#[derive(Debug, Clone)]
pub struct Connection {
    /// Which of the test's connections it is, counting the server's or the client's.
    pub exchange: usize,
    /// Whether the program accepted it, rather than connected.
    pub accepted: bool,
    /// Whether lengua handed it over already ended, as a connection it couldn't make: reads
    /// give 0 and writes fail with `EPIPE`.
    pub ended: bool,
    /// What the program sent on it.
    pub sent: Vec<Term>,
    /// Whether the program has finished with it: closed it, or shut its sending side.
    pub finished: bool,
    /// The next piece to arrive, and how much of it the program has read.
    piece: usize,
    taken: usize,
}

impl Connection {
    /// A connection lengua made before the program started: answered by the test server's
    /// connection `script` in [`Network::server`], or ended when there's none.
    #[must_use]
    pub fn made(script: Option<usize>) -> Self {
        Self {
            exchange: script.unwrap_or(0),
            accepted: false,
            ended: script.is_none(),
            sent: Vec::new(),
            finished: false,
            piece: 0,
            taken: 0,
        }
    }
}

/// The bytes of one `struct pollfd`, and the events poll is asked for: something to read, and
/// the peer closing its side.
const POLLFD_LEN: u64 = 8;
const POLLIN: u128 = 1;
const POLLRDHUP: u128 = 0x2000;

/// Whether the program has taken the test's client numbered `client`.
fn taken(state: &super::State, client: usize) -> bool {
    state
        .connections
        .iter()
        .any(|connection| connection.accepted && connection.exchange == client)
}

/// Whether the test's client numbered `client` has its whole answer: the program has taken its
/// connection and finished with it.
fn answered(state: &super::State, client: usize) -> bool {
    state.connections.iter().any(|connection| {
        connection.accepted && connection.exchange == client && connection.finished
    })
}

/// Whether the client's next step can come now: connecting, after the client before it has
/// connected (at once) or has its answer; each piece of what it sends, its held last piece once
/// every client after it has its answer; then closing its side once its answer has begun, or
/// at once when it waits for none.
fn can_step(state: &super::State, client: usize) -> bool {
    let network = &state.network;
    let Some(peer) = state.peers.as_ref().map(|peers| peers[client]) else {
        return false;
    };
    let them = &network.client[client];
    let pieces = them.pieces.len();
    if peer.closed {
        return false;
    }
    match peer.steps {
        0 if client == 0 => true,
        0 if network.at_once => state
            .peers
            .as_ref()
            .is_some_and(|peers| peers[client - 1].steps >= 1),
        0 => answered(state, client - 1),
        steps if steps <= pieces => {
            !(them.held && steps == pieces)
                || (client + 1..network.client.len()).all(|newer| answered(state, newer))
        }
        _ => {
            !them.waits
                || state.connections.iter().any(|connection| {
                    connection.accepted
                        && connection.exchange == client
                        && !connection.sent.is_empty()
                })
        }
    }
}

/// The client's next step.
fn step_client(state: &mut super::State, client: usize) {
    let pieces = state.network.client[client].pieces.len();
    if let Some(peers) = state.peers.as_mut() {
        let peer = &mut peers[client];
        if peer.steps <= pieces {
            peer.steps += 1;
        } else {
            peer.closed = true;
        }
    }
}

/// Whether a read of `fd` has something: the listener a client that has connected and isn't
/// taken, a connection bytes that have arrived and aren't read, or its client's side closed.
fn readable(state: &super::State, fd: u64) -> bool {
    let Some(peers) = state.peers.as_ref() else {
        return false;
    };
    if state.network.listening && fd == 3 {
        return (0..peers.len()).any(|client| peers[client].steps >= 1 && !taken(state, client));
    }
    let Some(Some(number)) = state.sockets.get(&fd) else {
        return false;
    };
    let connection = &state.connections[*number];
    if !connection.accepted {
        return false;
    }
    let peer = peers[connection.exchange];
    let arrived = peer.steps.saturating_sub(1);
    peer.closed || arrived > connection.piece
}

/// How a server that polls ends when nothing it waits on can come any more: serving, once
/// every client has its whole answer, or stuck, where one is still waiting.
fn idle(state: &super::State) -> End {
    let clients = state.network.client.len();
    match (0..clients).find(|client| !answered(state, *client)) {
        None => End::Serving,
        Some(client) if !taken(state, client) => End::Stuck(format!(
            "it waits while the test's client {} has connected and isn't taken",
            client + 1
        )),
        Some(client) => End::Stuck(format!(
            "it waits while the test's client {} waits for the rest of its answer, or for its \
             connection to close",
            client + 1
        )),
    }
}

const AF_INET: u128 = 2;
const MSG_NOSIGNAL: u64 = 0x4000;
const ENOTCONN: i64 = 107;
const EBADF: i64 = 9;
const EPIPE: i64 = 32;

/// A system call's result as the register value it leaves.
const fn returning(value: i64) -> u64 {
    value.cast_unsigned()
}

impl Cpu<'_> {
    /// The lowest file descriptor nothing holds.
    pub(super) fn free_descriptor(&self) -> u64 {
        (3..)
            .find(|fd| {
                !self.state.open.contains_key(fd)
                    && !self.state.sockets.contains_key(fd)
                    && !(self.state.network.listening && *fd == 3)
            })
            .unwrap_or(3)
    }

    /// `accept4` on the listener: the test client's next connection, once the program has
    /// finished with the last one, which the client waits for. With none left, the program is
    /// serving: the run ends there. For a server that polls, the next client that has connected.
    pub(super) fn accept(&mut self, fd: u64, address: u64, length: u64) -> Result<u64, Stop> {
        if !self.state.network.listening || fd != 3 {
            return Ok(returning(-EBADF));
        }
        if self.state.peers.is_some() {
            return self.accept_polled(address, length);
        }
        if self
            .state
            .connections
            .iter()
            .any(|connection| connection.accepted && !connection.finished)
        {
            return Err(End::Stuck(
                "it waits for the next connection before closing the last, which the test's \
                 client waits for"
                    .into(),
            ));
        }
        let accepted = self
            .state
            .connections
            .iter()
            .filter(|connection| connection.accepted)
            .count();
        if accepted >= self.state.network.client.len() {
            return Err(End::Serving);
        }
        self.take_client(accepted, address, length)
    }

    /// The test's client numbered `exchange`, connected: its address saved where the program
    /// asked, and its connection on the lowest free descriptor.
    fn take_client(&mut self, exchange: usize, address: u64, length: u64) -> Result<u64, Stop> {
        if address != 0 {
            // The client's address: this computer, from a port the kernel chose.
            let fresh = self.pool.len();
            let family = self.pool.bv(16, AF_INET);
            self.save(address, family)?;
            let port = self.pool.var(16, &format!("client{fresh}.port"));
            self.save(address + 2, port)?;
            let host = self.pool.bv(32, 0x0100_007f);
            self.save(address + 4, host)?;
            let zero = self.pool.bv(64, 0);
            self.save(address + 8, zero)?;
            if length != 0 {
                let sixteen = self.pool.bv(32, 16);
                self.save(length, sixteen)?;
            }
        }
        let number = self.state.connections.len();
        self.state.connections.push(Connection {
            exchange,
            accepted: true,
            ended: false,
            sent: Vec::new(),
            finished: false,
            piece: 0,
            taken: 0,
        });
        let new = self.free_descriptor();
        self.state.sockets.insert(new, Some(number));
        Ok(new)
    }

    /// `accept4` for a server that polls: the first client that has connected and isn't taken.
    fn accept_polled(&mut self, address: u64, length: u64) -> Result<u64, Stop> {
        let peers = self.state.peers.clone().unwrap_or_default();
        let waiting = (0..peers.len())
            .find(|client| peers[*client].steps >= 1 && !taken(self.state, *client));
        match waiting {
            Some(client) => self.take_client(client, address, length),
            None => Err(idle(self.state)),
        }
    }

    /// The connection a socket descriptor holds: `Ok(None)` for a socket not connected, `Err`
    /// for a descriptor that isn't a socket.
    pub(super) fn connection_of(&self, fd: u64) -> Result<Option<usize>, ()> {
        self.state.sockets.get(&fd).copied().ok_or(())
    }

    /// Sends `count` bytes from `buffer` on a connection.
    pub(super) fn send(
        &mut self,
        number: usize,
        buffer: u64,
        count: u64,
        flags: u64,
    ) -> Result<u64, Stop> {
        if flags & !MSG_NOSIGNAL != 0 {
            return Err(unsupported(format!("sending with flags {flags:#x}")));
        }
        if self.state.connections[number].ended {
            return Ok(returning(-EPIPE));
        }
        if count > 1 << 20 {
            return Err(unsupported("sending more than a mebibyte at once"));
        }
        for offset in 0..count {
            let byte = self.load(buffer + offset, 1)?;
            self.state.connections[number].sent.push(byte);
        }
        Ok(count)
    }

    /// Receives up to `count` bytes into `buffer` from a connection: the rest of the piece that
    /// has arrived, or 0 once the peer has closed its side. Reading the test server's reply
    /// before sending all it must receive waits forever: the server waits too.
    pub(super) fn receive(&mut self, number: usize, buffer: u64, count: u64) -> Result<u64, Stop> {
        let connection = self.state.connections[number].clone();
        if connection.ended {
            return Ok(0);
        }
        if self.state.peers.is_some() && connection.accepted {
            return self.receive_polled(number, buffer, count);
        }
        let network = std::rc::Rc::clone(&self.state.network);
        let pieces = if connection.accepted {
            let client = &network.client[connection.exchange];
            // Everything sent, and no answer yet: the client waits with its side open.
            if connection.piece >= client.pieces.len() && client.waits && connection.sent.is_empty()
            {
                return Err(End::Stuck(
                    "it reads again after the test's client sent its whole request, but the \
                     client, as curl and browsers do, keeps the connection open while it waits \
                     for the answer: answer once the request is complete (its newline or blank \
                     line), without waiting for the client to close its side"
                        .into(),
                ));
            }
            &client.pieces
        } else {
            let (receives, reply) = &network.server[connection.exchange];
            if connection.sent.len() < receives.len() {
                return Err(End::Stuck(format!(
                    "it reads the test server's reply after sending {} of the {} bytes the \
                     server waits for",
                    connection.sent.len(),
                    receives.len()
                )));
            }
            reply
        };
        let Some(piece) = pieces.get(connection.piece) else {
            return Ok(0);
        };
        let left = (piece.len() - connection.taken) as u64;
        let taken = count.min(left);
        for offset in 0..taken {
            let byte = piece[connection.taken + offset as usize];
            self.save(buffer + offset, byte)?;
        }
        let connection = &mut self.state.connections[number];
        connection.taken += taken as usize;
        if connection.taken == piece.len() {
            connection.piece += 1;
            connection.taken = 0;
        }
        Ok(taken)
    }

    /// For a server that polls: everything that has arrived on a connection and isn't read, up
    /// to `count` bytes; nothing once the client has closed its side. With nothing there yet, a
    /// read waits, while every other client waits too.
    fn receive_polled(&mut self, number: usize, buffer: u64, count: u64) -> Result<u64, Stop> {
        let connection = self.state.connections[number].clone();
        let network = Rc::clone(&self.state.network);
        let pieces = &network.client[connection.exchange].pieces;
        let peer = self
            .state
            .peers
            .as_ref()
            .map_or(Peer::default(), |peers| peers[connection.exchange]);
        let arrived = peer.steps.saturating_sub(1).min(pieces.len());
        let mut bytes = Vec::new();
        for (index, piece) in pieces
            .iter()
            .enumerate()
            .take(arrived)
            .skip(connection.piece)
        {
            let start = if index == connection.piece {
                connection.taken
            } else {
                0
            };
            bytes.extend_from_slice(&piece[start..]);
        }
        if bytes.is_empty() {
            if peer.closed {
                return Ok(0);
            }
            return Err(End::Stuck(format!(
                "it reads connection {} before anything more has arrived on it, which waits \
                 while every other client waits too: read a connection once poll says it's ready",
                connection.exchange + 1
            )));
        }
        let taken = count.min(bytes.len() as u64);
        for (offset, byte) in bytes.iter().take(taken as usize).enumerate() {
            self.save(buffer + offset as u64, *byte)?;
        }
        self.consume(number, taken as usize);
        Ok(taken)
    }

    /// `poll` for a server: waits until the listener or one of the connections it watches has
    /// something, the clients taking any of the steps open to them first, each way its own run.
    /// Runs that come to the same state are followed once.
    pub(super) fn poll_network(&mut self, table: u64, entries: u64) -> Result<Step, Stop> {
        if self.state.peers.is_none() {
            if !self.state.connections.is_empty() {
                return Err(unsupported(
                    "poll over connections taken before the program first polled",
                ));
            }
            self.state.peers = Some(vec![Peer::default(); self.state.network.client.len()]);
        }
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
        let eager = self.state.network.eager;
        let mut states = vec![self.state.clone()];
        for client in 0..self.state.network.client.len() {
            let mut next = Vec::new();
            for state in states {
                let mut moved = state.clone();
                if !eager {
                    next.push(state);
                }
                while can_step(&moved, client) {
                    step_client(&mut moved, client);
                    if !eager {
                        next.push(moved.clone());
                    }
                }
                if eager {
                    next.push(moved);
                }
            }
            states = next;
        }
        let after = self.next;
        let mut choices = Vec::new();
        let mut any = false;
        for mut state in states {
            let mut ready = 0;
            let mut revents = Vec::new();
            for (at, fd, events) in &watched {
                let has = *fd >= 0 && events & POLLIN != 0 && readable(&state, *fd as u64);
                ready += u64::from(has);
                revents.push((*at, if has { POLLIN } else { 0 }));
            }
            if ready == 0 {
                continue;
            }
            any = true;
            std::mem::swap(self.state, &mut state);
            for (at, value) in revents {
                let value = self.pool.bv(16, value);
                self.save(at + 6, value)?;
            }
            self.state.returns(self.pool, ready, after);
            self.state.rip = after;
            std::mem::swap(self.state, &mut state);
            let advanced = state.peers != self.state.peers;
            if state.come_back(advanced) {
                choices.push((self.pool.boolean(true), state));
            }
        }
        // Nothing it waits on can come: it waits forever. Every way that can come already
        // followed from another run: this one goes no further.
        if !any {
            return Err(idle(self.state));
        }
        Ok(Step::Choose(choices))
    }

    /// Everything yet to arrive on a connection, and whether the peer then waits with its side
    /// open: the test's client, before the program has answered anything.
    pub(super) fn unread(&self, number: usize) -> Result<(Vec<Term>, bool), Stop> {
        let connection = &self.state.connections[number];
        if connection.ended {
            return Ok((Vec::new(), false));
        }
        if self.state.peers.is_some() && connection.accepted {
            return Err(unsupported(
                "fill on a connection of a server that polls, which waits on that client alone",
            ));
        }
        let network = &self.state.network;
        let (pieces, waits) = if connection.accepted {
            let client = &network.client[connection.exchange];
            (&client.pieces, client.waits && connection.sent.is_empty())
        } else {
            let (receives, reply) = &network.server[connection.exchange];
            if connection.sent.len() < receives.len() {
                return Err(End::Stuck(format!(
                    "it reads the test server's reply after sending {} of the {} bytes the \
                     server waits for",
                    connection.sent.len(),
                    receives.len()
                )));
            }
            (reply, false)
        };
        let mut left = Vec::new();
        for (index, piece) in pieces.iter().enumerate().skip(connection.piece) {
            let start = if index == connection.piece {
                connection.taken
            } else {
                0
            };
            left.extend_from_slice(&piece[start.min(piece.len())..]);
        }
        Ok((left, waits))
    }

    /// How many bytes of what the peer sends on a connection the program has read so far.
    pub(super) fn consumed(&self, number: usize) -> usize {
        let connection = &self.state.connections[number];
        if connection.ended {
            return 0;
        }
        let network = &self.state.network;
        let pieces = if connection.accepted {
            &network.client[connection.exchange].pieces
        } else {
            &network.server[connection.exchange].1
        };
        pieces
            .iter()
            .take(connection.piece)
            .map(Vec::len)
            .sum::<usize>()
            + connection.taken
    }

    /// Marks `count` more bytes of a connection read, across its pieces.
    pub(super) fn consume(&mut self, number: usize, mut count: usize) {
        let network = std::rc::Rc::clone(&self.state.network);
        let connection = &mut self.state.connections[number];
        if connection.ended {
            return;
        }
        let pieces = if connection.accepted {
            &network.client[connection.exchange].pieces
        } else {
            &network.server[connection.exchange].1
        };
        while count > 0 {
            let Some(piece) = pieces.get(connection.piece) else {
                break;
            };
            let left = piece.len() - connection.taken;
            let taken = count.min(left);
            connection.taken += taken;
            count -= taken;
            if connection.taken == piece.len() {
                connection.piece += 1;
                connection.taken = 0;
            }
        }
    }

    /// `shutdown`: the program's side of a connection is finished.
    pub(super) fn shutdown(&mut self, fd: u64, how: u64) -> Result<u64, Stop> {
        match self.connection_of(fd) {
            Err(()) => Ok(returning(-EBADF)),
            Ok(None) => Ok(returning(-ENOTCONN)),
            Ok(Some(number)) => {
                if matches!(how, 1 | 2) {
                    self.state.connections[number].finished = true;
                }
                Ok(0)
            }
        }
    }

    /// `close` of a socket.
    pub(super) fn close_socket(&mut self, fd: u64) -> bool {
        match self.state.sockets.remove(&fd) {
            Some(Some(number)) => {
                self.state.connections[number].finished = true;
                true
            }
            Some(None) => true,
            None => false,
        }
    }

    /// The not-connected error, for a socket used before it's connected.
    pub(super) fn not_connected(&mut self) -> u64 {
        returning(-ENOTCONN)
    }
}

impl super::State {
    /// Whether the program waited on its connections with poll, so the test's clients took
    /// their steps in every order.
    #[must_use]
    pub const fn polled(&self) -> bool {
        self.peers.is_some()
    }

    /// Whether a run that came back from poll in this state goes on from it. One back where it
    /// came back from before, with no client able to take a step, goes round its loop forever
    /// while a client waits: it ends stuck there; with a client able to, the runs where one
    /// does go on instead. Where the clients took steps since the last poll, a run that comes
    /// to a state another run already came to goes no further: that one goes on for both. A
    /// loop can't take in such a state, as the clients' steps only add up, so every loop is
    /// still found on one run. A state with files or threads in hand is always followed.
    pub(super) fn come_back(&mut self, advanced: bool) -> bool {
        let Some(key) = self.poll_key() else {
            return true;
        };
        let mut before = self.history.as_deref();
        while let Some(history) = before {
            if history.key == key {
                let clients = self.network.client.len();
                if (0..clients).any(|client| can_step(self, client)) {
                    return false;
                }
                self.end = Some(match idle(self) {
                    End::Serving => End::Stuck(
                        "it goes round its loop forever, poll saying a connection is ready that \
                         it never reads"
                            .into(),
                    ),
                    End::Stuck(why) => End::Stuck(format!("{why}, going round its loop forever")),
                    other => other,
                });
                return true;
            }
            before = history.before.as_deref();
        }
        if advanced && !self.polled.borrow_mut().insert(key.clone()) {
            return false;
        }
        self.history = Some(Rc::new(History {
            key,
            before: self.history.take(),
        }));
        true
    }

    /// Everything about the state that what follows depends on, but how many instructions and
    /// calls it took to get there.
    fn poll_key(&self) -> Option<Vec<u64>> {
        if !self.written.is_empty()
            || !self.open.is_empty()
            || !self.made.is_empty()
            || !self.removed.is_empty()
            || !self.suspended.is_empty()
        {
            return None;
        }
        let none = u64::MAX;
        let term = |term: Term| term.index() as u64;
        let owner = |owner: Option<super::Owner>| match owner {
            None => none,
            Some(super::Owner::Symbol(place)) => 1 << 60 | place as u64,
            Some(super::Owner::Frame(number)) => 2 << 60 | number,
            Some(super::Owner::Argument(place)) => 3 << 60 | place as u64,
        };
        let mut key = vec![self.rip];
        key.extend(self.registers.iter().map(|value| term(*value)));
        key.extend(
            [self.carry, self.zero, self.sign, self.overflow, self.parity]
                .iter()
                .map(|flag| term(*flag)),
        );
        key.extend(match self.last {
            super::Last::Compare(a, b) => [0, term(a), term(b)],
            super::Last::Logic(result) => [1, term(result), 0],
            super::Last::Other => [2, 0, 0],
        });
        for vector in &self.vectors {
            key.extend(vector.iter().map(|byte| term(*byte)));
        }
        key.extend(self.masks.iter().map(|mask| term(*mask)));
        for (page, bytes) in &self.memory.pages {
            key.push(*page);
            key.extend(bytes.iter().map(|byte| byte.map_or(none, term)));
        }
        key.push(self.stored.len() as u64);
        for (address, value) in &self.stored {
            key.extend([*address, term(*value)]);
        }
        key.push(self.logs.len() as u64);
        for log in self.logs.iter() {
            key.extend([log.start, log.end, term(log.common)]);
            for (address, byte) in &log.others {
                key.extend([*address, term(*byte)]);
            }
            key.push(log.entries.len() as u64);
            for (address, byte) in &log.entries {
                key.extend([term(*address), term(*byte)]);
            }
        }
        key.push(self.path.len() as u64);
        key.extend(self.path.iter().map(|condition| term(*condition)));
        key.push(self.stdin_read as u64);
        for output in [&self.stdout, &self.stderr] {
            key.push(output.len() as u64);
            key.extend(output.iter().map(|byte| term(*byte)));
        }
        key.push(self.connections.len() as u64);
        for connection in &self.connections {
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
        key.push(self.sockets.len() as u64);
        for (fd, number) in &self.sockets {
            key.extend([*fd, number.map_or(none, |number| number as u64)]);
        }
        for peer in self.peers.iter().flatten() {
            key.extend([peer.steps as u64, u64::from(peer.closed)]);
        }
        key.push(self.returns.len() as u64);
        for back in &self.returns {
            key.extend([back.top, back.back, back.callee, back.frame]);
        }
        key.extend(self.owners.iter().map(|held| owner(*held)));
        key.push(self.pointers.len() as u64);
        for (address, held) in &self.pointers {
            key.extend([*address, owner(Some(*held))]);
        }
        key.push(self.obligations.len() as u64);
        key.extend(
            self.obligations
                .iter()
                .map(|(condition, _)| term(*condition)),
        );
        key.extend([
            u64::from(self.raw_reads),
            self.fill_cut.map_or(none, |cut| cut as u64),
            u64::from(self.raw_cuts),
            self.threads,
            self.stack_top.unwrap_or(none),
            self.base_frame,
            u64::from(self.arrivals),
            self.random_used as u64,
            self.readings_used as u64,
        ]);
        Some(key)
    }
}
