//! The network as a test sets it up: its own servers, each on one of the connections lengua made
//! for the program before it started, or its own client, which connects to the program's
//! listener. Each peer behaves as the test runner's does: the server reads exactly what it must
//! receive before it replies, and the client sends, closes its side, and waits for the program
//! to close. What the peer sends arrives in the pieces the run's delivery cuts it into.

use std::rc::Rc;

use super::{Host, World};
use crate::prove::exec::{End, Stop, unsupported};
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
    pub(in crate::prove) piece: usize,
    pub(in crate::prove) taken: usize,
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

const AF_INET: u128 = 2;
const MSG_NOSIGNAL: u64 = 0x4000;
const ENOTCONN: i64 = 107;
const EBADF: i64 = 9;
const EPIPE: i64 = 32;

/// A system call's result as the register value it leaves.
const fn returning(value: i64) -> u64 {
    value.cast_unsigned()
}

impl World {
    /// Whether the program has taken the test's client numbered `client`.
    fn taken(&self, client: usize) -> bool {
        self.connections
            .iter()
            .any(|connection| connection.accepted && connection.exchange == client)
    }

    /// Whether the test's client numbered `client` has its whole answer: the program has taken
    /// its connection and finished with it.
    fn answered(&self, client: usize) -> bool {
        self.connections.iter().any(|connection| {
            connection.accepted && connection.exchange == client && connection.finished
        })
    }

    /// How a server that polls ends when nothing it waits on can come any more: serving, once
    /// every client has its whole answer, or stuck, where one is still waiting.
    pub(in crate::prove) fn idle(&self) -> End {
        let clients = self.network.client.len();
        match (0..clients).find(|client| !self.answered(*client)) {
            None => End::Serving,
            Some(client) if !self.taken(client) => End::Stuck(format!(
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

    /// Whether the program waited on its connections with poll, so the test's clients took
    /// their steps in every order.
    #[must_use]
    pub const fn polled(&self) -> bool {
        self.peers.is_some()
    }

    /// The lowest file descriptor nothing holds.
    pub(in crate::prove) fn free_descriptor(&self) -> u64 {
        (3..)
            .find(|fd| {
                !self.open.contains_key(fd)
                    && !self.sockets.contains_key(fd)
                    && !(self.network.listening && *fd == 3)
            })
            .unwrap_or(3)
    }

    /// `accept4` on the listener: the test client's next connection, once the program has
    /// finished with the last one, which the client waits for, its address put at `address`
    /// and its length at `length` where the program asks. With none left, the program is
    /// serving: the run ends there. For a server that polls, the next client that has
    /// connected.
    ///
    /// # Errors
    ///
    /// How the run ends: serving, waiting for what never comes, or where the program may not
    /// write the address.
    pub(in crate::prove) fn accept(
        host: &mut impl Host,
        fd: u64,
        address: u64,
        length: u64,
    ) -> Result<u64, Stop> {
        let world = host.world();
        if !world.network.listening || fd != 3 {
            return Ok(returning(-EBADF));
        }
        if world.peers.is_some() {
            return Self::accept_polled(host, address, length);
        }
        if world
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
        let accepted = world
            .connections
            .iter()
            .filter(|connection| connection.accepted)
            .count();
        if accepted >= world.network.client.len() {
            return Err(End::Serving);
        }
        Self::take_client(host, accepted, address, length)
    }

    /// The test's client numbered `exchange`, connected: its address saved where the program
    /// asked, and its connection on the lowest free descriptor.
    fn take_client(
        host: &mut impl Host,
        exchange: usize,
        address: u64,
        length: u64,
    ) -> Result<u64, Stop> {
        if address != 0 {
            // The client's address: this computer, from a port the kernel chose.
            let fresh = host.pool().len();
            let family = host.pool().bv(16, AF_INET);
            host.save(address, family)?;
            let port = host.pool().var(16, &format!("client{fresh}.port"));
            host.save(address + 2, port)?;
            let local = host.pool().bv(32, 0x0100_007f);
            host.save(address + 4, local)?;
            let zero = host.pool().bv(64, 0);
            host.save(address + 8, zero)?;
            if length != 0 {
                let sixteen = host.pool().bv(32, 16);
                host.save(length, sixteen)?;
            }
        }
        let world = host.world();
        let number = world.connections.len();
        world.connections.push(Connection {
            exchange,
            accepted: true,
            ended: false,
            sent: Vec::new(),
            finished: false,
            piece: 0,
            taken: 0,
        });
        let new = world.free_descriptor();
        world.sockets.insert(new, Some(number));
        Ok(new)
    }

    /// `accept4` for a server that polls: the first client that has connected and isn't taken.
    fn accept_polled(host: &mut impl Host, address: u64, length: u64) -> Result<u64, Stop> {
        let world = host.world();
        let peers = world.peers.clone().unwrap_or_default();
        let waiting =
            (0..peers.len()).find(|client| peers[*client].steps >= 1 && !world.taken(*client));
        match waiting {
            Some(client) => Self::take_client(host, client, address, length),
            None => Err(host.world().idle()),
        }
    }

    /// The connection a socket descriptor holds: `Ok(None)` for a socket not connected, `Err`
    /// for a descriptor that isn't a socket.
    pub(in crate::prove) fn connection_of(&self, fd: u64) -> Result<Option<usize>, ()> {
        self.sockets.get(&fd).copied().ok_or(())
    }

    /// Sends the `count` bytes at `buffer` on connection `number`.
    ///
    /// # Errors
    ///
    /// How the run ends where the program may not read the bytes there, or sends with flags or
    /// as much as the model doesn't follow.
    pub(in crate::prove) fn send(
        host: &mut impl Host,
        number: usize,
        buffer: u64,
        count: u64,
        flags: u64,
    ) -> Result<u64, Stop> {
        if flags & !MSG_NOSIGNAL != 0 {
            return Err(unsupported(format!("sending with flags {flags:#x}")));
        }
        if host.world().connections[number].ended {
            return Ok(returning(-EPIPE));
        }
        if count > 1 << 20 {
            return Err(unsupported("sending more than a mebibyte at once"));
        }
        for offset in 0..count {
            let byte = host.load(buffer + offset, 1)?;
            host.world().connections[number].sent.push(byte);
        }
        Ok(count)
    }

    /// Receives up to `count` bytes into `buffer` from connection `number`: the rest of the
    /// piece that has arrived, or 0 once the peer has closed its side. Reading the test server's
    /// reply before sending all it must receive waits forever: the server waits too.
    ///
    /// # Errors
    ///
    /// How the run ends: waiting for what never comes, or where the program may not write the
    /// bytes there.
    pub(in crate::prove) fn receive(
        host: &mut impl Host,
        number: usize,
        buffer: u64,
        count: u64,
    ) -> Result<u64, Stop> {
        let world = host.world();
        let connection = world.connections[number].clone();
        if connection.ended {
            return Ok(0);
        }
        if world.peers.is_some() && connection.accepted {
            return Self::receive_polled(host, number, buffer, count);
        }
        let network = Rc::clone(&world.network);
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
            host.save(buffer + offset, byte)?;
        }
        let connection = &mut host.world().connections[number];
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
    fn receive_polled(
        host: &mut impl Host,
        number: usize,
        buffer: u64,
        count: u64,
    ) -> Result<u64, Stop> {
        let world = host.world();
        let connection = world.connections[number].clone();
        let network = Rc::clone(&world.network);
        let pieces = &network.client[connection.exchange].pieces;
        let peer = world
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
            host.save(buffer + offset as u64, *byte)?;
        }
        host.world().consume(number, taken as usize);
        Ok(taken)
    }

    /// Marks `count` more bytes of a connection read, across its pieces.
    pub(in crate::prove) fn consume(&mut self, number: usize, mut count: usize) {
        let network = Rc::clone(&self.network);
        let connection = &mut self.connections[number];
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
    pub(in crate::prove) fn shutdown(&mut self, fd: u64, how: u64) -> u64 {
        match self.connection_of(fd) {
            Err(()) => returning(-EBADF),
            Ok(None) => returning(-ENOTCONN),
            Ok(Some(number)) => {
                if matches!(how, 1 | 2) {
                    self.connections[number].finished = true;
                }
                0
            }
        }
    }

    /// `close` of a socket.
    pub(in crate::prove) fn close_socket(&mut self, fd: u64) -> bool {
        match self.sockets.remove(&fd) {
            Some(Some(number)) => {
                self.connections[number].finished = true;
                true
            }
            Some(None) => true,
            None => false,
        }
    }

    /// The not-connected error, for a socket used before it's connected.
    pub(in crate::prove) const fn not_connected() -> u64 {
        returning(-ENOTCONN)
    }
}
