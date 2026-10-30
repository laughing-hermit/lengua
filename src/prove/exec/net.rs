//! The network as a test sets it up: its own server, which the program connects to on the
//! port the kernel chose, or its own client, which connects to the program's listener. Each
//! peer behaves as the test runner's does: the server reads exactly what it must receive before
//! it replies, and the client sends, closes its side, and waits for the program to close. What
//! the peer sends arrives in the pieces the run's delivery cuts it into.

use super::{Cpu, End, Step, Stop, unsupported};
use crate::prove::term::Term;

/// The test's network for one run.
#[derive(Debug, Clone, Default)]
pub struct Network {
    /// The test server's port, an input, when the test has a server.
    pub port: Option<Term>,
    /// The test server's connections in order: what it must receive, and its reply in the
    /// pieces it arrives in.
    pub server: Vec<(Vec<Term>, Vec<Vec<Term>>)>,
    /// The test client's connections in order.
    pub client: Vec<Client>,
    /// Whether the program has its listener at file descriptor 3.
    pub listening: bool,
}

/// One connection of the test's client, which behaves as curl and browsers do: it sends, keeps
/// its side open until the program has started answering, then closes it and reads the rest.
#[derive(Debug, Clone, Default)]
pub struct Client {
    /// What it sends, in the pieces it arrives in.
    pub pieces: Vec<Vec<Term>>,
    /// Whether it waits for an answer before closing its side: the test expects one.
    pub waits: bool,
}

/// One connection as the program used it.
#[derive(Debug, Clone)]
pub struct Connection {
    /// Which of the test's connections it is, counting the server's or the client's.
    pub exchange: usize,
    /// Whether the program accepted it, rather than connected.
    pub accepted: bool,
    /// What the program sent on it.
    pub sent: Vec<Term>,
    /// Whether the program has finished with it: closed it, or shut its sending side.
    pub finished: bool,
    /// The next piece to arrive, and how much of it the program has read.
    piece: usize,
    taken: usize,
}

const AF_INET: u128 = 2;
const SOCK_STREAM: u64 = 1;
const SOCK_CLOEXEC: u64 = 0o2_000_000;
const MSG_NOSIGNAL: u64 = 0x4000;
const EACCES: i64 = 13;
const ENOTCONN: i64 = 107;
const EISCONN: i64 = 106;
const ECONNREFUSED: i64 = 111;
const EBADF: i64 = 9;

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

    /// `socket`: a TCP socket over IPv4.
    pub(super) fn socket(&mut self, domain: u64, kind: u64, protocol: u64) -> Result<u64, Stop> {
        if domain != 2 || kind & !SOCK_CLOEXEC != SOCK_STREAM || !matches!(protocol, 0 | 6) {
            return Err(unsupported("a socket other than TCP over IPv4"));
        }
        let fd = self.free_descriptor();
        self.state.sockets.insert(fd, None);
        Ok(fd)
    }

    /// `connect`: to this computer, on the test server's port; the kernel refuses any other
    /// port, as the test's rules allow only that one. Splits where the port depends on the
    /// input.
    pub(super) fn connect(&mut self, fd: u64, address: u64) -> Result<Step, Stop> {
        let Some(socket) = self.state.sockets.get(&fd).copied() else {
            let result = returning(-EBADF);
            return self.finish_call(result);
        };
        if socket.is_some() {
            let result = returning(-EISCONN);
            return self.finish_call(result);
        }
        let family = self.load(address, 2)?;
        if self.pool.constant(family) != Some(AF_INET) {
            return Err(unsupported("connecting to an address that isn't IPv4"));
        }
        let (high, low) = (self.load(address + 2, 1)?, self.load(address + 3, 1)?);
        let port = self.pool.concat(high, low);
        // A test's program reaches this computer only: lengua refuses any other, and the test's
        // server is at 127.0.0.1.
        let host = self.load(address + 4, 4)?;
        match self.pool.constant(host) {
            Some(0x0100_007f) => {}
            Some(other) if other & 0xff == 127 => {
                let result = returning(-ECONNREFUSED);
                return self.finish_call(result);
            }
            Some(_) => {
                let result = returning(-EACCES);
                return self.finish_call(result);
            }
            None => return Err(unsupported("connecting to a computer the input chooses")),
        }
        let connected = self
            .state
            .connections
            .iter()
            .filter(|connection| !connection.accepted)
            .count();
        let Some(server) = self.state.network.port else {
            let result = returning(-EACCES);
            return self.finish_call(result);
        };
        if connected >= self.state.network.server.len() {
            let result = returning(-ECONNREFUSED);
            return self.finish_call(result);
        }
        let wanted = self.pool.extract(server, 15, 0);
        let same = self.pool.eq(port, wanted);
        if std::env::var_os("LENGUA_PROVE_EXPLAIN").is_some() && self.pool.truth(same).is_none() {
            eprintln!(
                "connect compares {} with {}",
                self.pool.sketch(port, 12),
                self.pool.sketch(wanted, 12)
            );
        }
        let mut refused = None;
        match self.pool.truth(same) {
            Some(false) => {
                let result = returning(-EACCES);
                return self.finish_call(result);
            }
            Some(true) => {}
            None => {
                let mut other = self.state.clone();
                other.registers[0] = self.pool.bv(64, u128::from(returning(-EACCES)));
                other.registers[1] = self.pool.bv(64, u128::from(self.instruction.next_ip()));
                other.registers[11] = self.pool.bv(64, 0x202);
                refused = Some(other);
            }
        }
        let number = self.state.connections.len();
        self.state.connections.push(Connection {
            exchange: connected,
            accepted: false,
            sent: Vec::new(),
            finished: false,
            piece: 0,
            taken: 0,
        });
        self.state.sockets.insert(fd, Some(number));
        self.finish_call(0)?;
        Ok(match refused {
            Some(other) => Step::Split(same, Box::new(other)),
            None => Step::Next,
        })
    }

    /// `accept4` on the listener: the test client's next connection, once the program has
    /// finished with the last one, which the client waits for. With none left, the program is
    /// serving: the run ends there.
    pub(super) fn accept(&mut self, fd: u64, address: u64, length: u64) -> Result<u64, Stop> {
        if !self.state.network.listening || fd != 3 {
            return Ok(returning(-EBADF));
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
            exchange: accepted,
            accepted: true,
            sent: Vec::new(),
            finished: false,
            piece: 0,
            taken: 0,
        });
        let new = self.free_descriptor();
        self.state.sockets.insert(new, Some(number));
        Ok(new)
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

    /// Everything yet to arrive on a connection, and whether the peer then waits with its side
    /// open: the test's client, before the program has answered anything.
    pub(super) fn unread(&self, number: usize) -> Result<(Vec<Term>, bool), Stop> {
        let connection = &self.state.connections[number];
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

    /// Sets a system call's result and the registers the kernel changes, and goes on.
    pub(super) fn finish_call(&mut self, result: u64) -> Result<Step, Stop> {
        self.state.registers[0] = self.pool.bv(64, u128::from(result));
        self.state.registers[1] = self.pool.bv(64, u128::from(self.instruction.next_ip()));
        self.state.registers[11] = self.pool.bv(64, 0x202);
        Ok(Step::Next)
    }

    /// The not-connected error, for a socket used before it's connected.
    pub(super) fn not_connected(&mut self) -> u64 {
        returning(-ENOTCONN)
    }
}
