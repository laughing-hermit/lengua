//! The broker: lengua's one service for what a program may not do for itself. It runs in the
//! process that started the program (the server, or `lengua-mcp run`), with a thread for each
//! program, and talks to the program's edge over a channel the program inherits at descriptor
//! [`crate::edge::CHANNEL`]. Every message the program sends is hostile input: the parser refuses
//! whatever isn't exactly one of the messages below, and the service ends there.
//!
//! **Names.** `os_open`, `os_mkdir`, `os_remove` and `os_rename` are requests: the broker holds
//! each name to what every system takes ([`crate::names`]), matches it against the folder's
//! entries so that a new name differing from one in its folder only in case or normalization is
//! refused, answers `EBUSY` for a file the program holds open and then renames or removes (which
//! Windows refuses), resolves every path beneath the program's folder without following links
//! (a link or a way out is `EACCES`), and hands a file's descriptor back with the reply.
//!
//! **Tests.** In a test the broker also answers the clock and random bytes from the test's
//! supply, and is told when the program waits and when it lists a folder; sleeps and the order
//! of listings are the edge's, from the flags in the first message.
//!
//! **The wire.** Integers are little-endian. The broker speaks first, with [`Hello`]. A request
//! is [`REQUEST_LEN`] bytes, then its names, with no terminator. A reply is [`REPLY_LEN`] bytes:
//! the result, which is the negative of an error from [`crate::edge::error`], then how many
//! bytes of payload follow. A file's descriptor comes with its reply as `SCM_RIGHTS`.

use crate::names;

/// What the first four bytes of a channel say it is: "LNGB".
pub const MAGIC: u32 = u32::from_le_bytes(*b"LNGB");

/// The version of the messages below.
pub const VERSION: u32 = 1;

/// How long [`Hello`] is, in bytes.
pub const HELLO_LEN: usize = 32;

/// How long a request's fixed part is, in bytes.
pub const REQUEST_LEN: usize = 16;

/// How long a reply's fixed part is, in bytes.
pub const REPLY_LEN: usize = 16;

/// The most bytes of names a request carries: two paths.
pub const MAX_BODY: usize = 2 * names::MAX_PATH;

/// Whether the program is under test or run by someone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run: the broker handles names alone.
    Run,
    /// Test: the broker also supplies the clock and random bytes.
    Test,
}

/// What [`Hello::flags`] says.
pub mod flag {
    /// The program lists a folder's entries turned round, as a test's second run does.
    pub const REVERSED: u32 = 1;
    /// The broker supplies random bytes: the edge asks it for them.
    pub const RANDOM: u32 = 2;
    /// The broker supplies clock readings: the edge asks it for them.
    pub const CLOCK: u32 = 4;
}

/// The first message, from the broker to the program, before the program starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// A test or a run.
    pub mode: Mode,
    /// What the edge does differently, from [`flag`].
    pub flags: u32,
    /// The most bytes a file may hold, or 0 for no limit.
    pub file_limit: u64,
}

impl Hello {
    /// The message: the magic, the version, the mode (0 for a run, 1 for a test), the flags, the
    /// file limit, and 8 bytes of zeros.
    #[must_use]
    pub fn encode(&self) -> [u8; HELLO_LEN] {
        let mut bytes = [0u8; HELLO_LEN];
        bytes[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[4..8].copy_from_slice(&VERSION.to_le_bytes());
        let mode: u32 = match self.mode {
            Mode::Run => 0,
            Mode::Test => 1,
        };
        bytes[8..12].copy_from_slice(&mode.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.flags.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.file_limit.to_le_bytes());
        bytes
    }

    /// Reads the message, for a client.
    ///
    /// # Errors
    ///
    /// When the bytes aren't a first message of this version.
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        let Ok(bytes) = <[u8; HELLO_LEN]>::try_from(bytes) else {
            return Err(Refusal::Short);
        };
        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        if word(0) != MAGIC || word(4) != VERSION {
            return Err(Refusal::Magic);
        }
        let mode = match word(8) {
            0 => Mode::Run,
            1 => Mode::Test,
            _ => return Err(Refusal::Magic),
        };
        let flags = word(12);
        if flags & !(flag::REVERSED | flag::RANDOM | flag::CLOCK) != 0 || bytes[24..] != [0; 8] {
            return Err(Refusal::Magic);
        }
        let mut limit = [0u8; 8];
        limit.copy_from_slice(&bytes[16..24]);
        Ok(Self {
            mode,
            flags,
            file_limit: u64::from_le_bytes(limit),
        })
    }
}

/// What a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `os_open(name, mode)`: the argument is the mode, 0 to 4; the reply carries the descriptor
    /// and a token for the file, to tell the broker when it closes.
    Open = 1,
    /// `os_mkdir(name)`.
    Mkdir = 2,
    /// `os_remove(name, kind)`: the argument is 0 for a file and 1 for a folder.
    Remove = 3,
    /// `os_rename(from, to)`: two names.
    Rename = 4,
    /// A note, with no reply: the file with this token, the value, is closed.
    Closed = 5,
    /// A note, with no reply: the program waits for a connection or a key, blocked until one comes.
    Waiting = 6,
    /// A note, with no reply: the program lists a folder.
    Listed = 7,
    /// A note, with no reply: the program is back from what it waited in.
    Resumed = 10,
    /// The clock: the argument is 0 or 1; the reply's payload is the seconds and the nanoseconds.
    Clock = 8,
    /// Random bytes: the value is how many, from 1 to [`crate::edge::RANDOM_MOST`].
    Random = 9,
}

impl Kind {
    /// The kind a byte names.
    fn of(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::Open,
            2 => Self::Mkdir,
            3 => Self::Remove,
            4 => Self::Rename,
            5 => Self::Closed,
            6 => Self::Waiting,
            7 => Self::Listed,
            10 => Self::Resumed,
            8 => Self::Clock,
            9 => Self::Random,
            _ => return None,
        })
    }

    /// Whether the broker answers it.
    #[must_use]
    pub const fn has_reply(self) -> bool {
        !matches!(
            self,
            Self::Closed | Self::Waiting | Self::Listed | Self::Resumed
        )
    }
}

/// What a program's idea lets it ask the broker for: names (opening, making, removing and
/// renaming files and folders), the clock and random bytes. The broker does these in the server,
/// outside the program's confinement, so the kernel can't hold the program to its idea for them:
/// the broker does. A note, which asks for nothing, is always allowed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Permit {
    /// Opening, making, removing and renaming: the idea names files.
    pub names: bool,
    /// The clock: the idea names it.
    pub clock: bool,
    /// Random bytes: the idea names them.
    pub random: bool,
}

impl Permit {
    /// Everything: for a test of the broker or of the edge, which asks for each thing.
    pub const ALL: Self = Self {
        names: true,
        clock: true,
        random: true,
    };

    /// What an idea allows, from the system calls it allows: names when it allows any call that
    /// makes or finds a path (an idea that names files allows them all), the clock when it allows
    /// reading it, random bytes when it allows drawing them.
    #[must_use]
    pub fn of(calls: &[crate::idea::Syscall]) -> Self {
        use crate::idea::Syscall;
        Self {
            names: calls.iter().any(|call| {
                matches!(
                    call,
                    Syscall::Openat | Syscall::Mkdirat | Syscall::Unlinkat | Syscall::Renameat
                )
            }),
            clock: calls.contains(&Syscall::ClockGettime),
            random: calls.contains(&Syscall::Getrandom),
        }
    }

    /// Whether a request of `kind` is allowed.
    #[must_use]
    pub const fn allows(self, kind: Kind) -> bool {
        match kind {
            Kind::Open | Kind::Mkdir | Kind::Remove | Kind::Rename => self.names,
            Kind::Clock => self.clock,
            Kind::Random => self.random,
            Kind::Closed | Kind::Waiting | Kind::Listed | Kind::Resumed => true,
        }
    }
}

/// The fixed part of a request, once it is read: what to read next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// What it asks for.
    pub kind: Kind,
    /// The mode, the kind of thing, or the clock.
    pub argument: u8,
    /// How many bytes the first name has.
    pub first: usize,
    /// How many bytes the second name has.
    pub second: usize,
    /// A token or a count.
    pub value: u64,
}

impl Header {
    /// How many bytes of names follow the fixed part.
    #[must_use]
    pub const fn body(&self) -> usize {
        self.first + self.second
    }
}

/// A request, whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// What it asks for.
    pub kind: Kind,
    /// The mode, the kind of thing, or the clock.
    pub argument: u8,
    /// A token or a count.
    pub value: u64,
    /// The first name.
    pub first: Vec<u8>,
    /// The second name, for a rename.
    pub second: Vec<u8>,
}

impl Request {
    /// The bytes a client sends for it.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(REQUEST_LEN + self.first.len() + self.second.len());
        bytes.push(self.kind as u8);
        bytes.push(self.argument);
        bytes.extend_from_slice(
            &u16::try_from(self.first.len())
                .unwrap_or(u16::MAX)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u16::try_from(self.second.len())
                .unwrap_or(u16::MAX)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&self.first);
        bytes.extend_from_slice(&self.second);
        bytes
    }
}

/// Why bytes aren't a message of the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer or more bytes than the message has.
    Short,
    /// A first message that isn't this version's.
    Magic,
    /// A kind no request has.
    Kind(u8),
    /// An argument the kind doesn't take.
    Argument(u8),
    /// A value the kind doesn't take.
    Value(u64),
    /// Names of a length the kind doesn't take.
    Length(usize),
    /// A name with a NUL in it.
    Nul,
    /// Bytes the protocol keeps at zero, not zero.
    Reserved,
}

/// Reads the fixed part of a request.
///
/// # Errors
///
/// When the bytes aren't the start of a request: the wrong length, a kind, argument, value or
/// length the protocol doesn't have, or bytes that must be zero and aren't.
pub fn parse_header(bytes: &[u8]) -> Result<Header, Refusal> {
    let Ok(bytes) = <[u8; REQUEST_LEN]>::try_from(bytes) else {
        return Err(Refusal::Short);
    };
    let kind = Kind::of(bytes[0]).ok_or(Refusal::Kind(bytes[0]))?;
    let argument = bytes[1];
    let first = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let second = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    if bytes[6] != 0 || bytes[7] != 0 {
        return Err(Refusal::Reserved);
    }
    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes[8..16]);
    let value = u64::from_le_bytes(value);
    // What each kind takes: its argument, its value and the lengths of its names.
    let (arguments, values, names): (u8, std::ops::RangeInclusive<u64>, usize) = match kind {
        Kind::Open => (4, 0..=0, 1),
        Kind::Mkdir => (0, 0..=0, 1),
        Kind::Remove => (1, 0..=0, 1),
        Kind::Rename => (0, 0..=0, 2),
        Kind::Closed => (0, 1..=u64::from(u32::MAX), 0),
        Kind::Waiting | Kind::Listed | Kind::Resumed => (0, 0..=0, 0),
        Kind::Clock => (1, 0..=0, 0),
        Kind::Random => (0, 1..=crate::edge::RANDOM_MOST, 0),
    };
    if argument > arguments {
        return Err(Refusal::Argument(argument));
    }
    if !values.contains(&value) {
        return Err(Refusal::Value(value));
    }
    let fits = |length: usize| (1..=names::MAX_PATH).contains(&length);
    let lengths_fit = match names {
        0 => first == 0 && second == 0,
        1 => fits(first) && second == 0,
        _ => fits(first) && fits(second),
    };
    if !lengths_fit {
        return Err(Refusal::Length(first + second));
    }
    Ok(Header {
        kind,
        argument,
        first,
        second,
        value,
    })
}

/// Reads a whole request: its fixed part and exactly the names it says follow.
///
/// # Errors
///
/// When the bytes aren't exactly one request, or a name has a NUL in it.
pub fn parse_request(bytes: &[u8]) -> Result<Request, Refusal> {
    let (fixed, body) = bytes.split_at_checked(REQUEST_LEN).ok_or(Refusal::Short)?;
    let header = parse_header(fixed)?;
    if body.len() != header.body() {
        return Err(Refusal::Length(body.len()));
    }
    let (first, second) = body.split_at(header.first);
    if first.contains(&0) || second.contains(&0) {
        return Err(Refusal::Nul);
    }
    Ok(Request {
        kind: header.kind,
        argument: header.argument,
        value: header.value,
        first: first.to_vec(),
        second: second.to_vec(),
    })
}

/// The bytes of a reply: the result, how long the payload is, and the payload.
#[must_use]
pub fn encode_reply(result: i64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(REPLY_LEN + payload.len());
    bytes.extend_from_slice(&result.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(payload.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(payload);
    bytes
}

/// Reads a reply's fixed part: the result and how many bytes of payload follow, for a client.
///
/// # Errors
///
/// When the bytes aren't a reply's fixed part.
pub fn parse_reply(bytes: &[u8]) -> Result<(i64, usize), Refusal> {
    let Ok(bytes) = <[u8; REPLY_LEN]>::try_from(bytes) else {
        return Err(Refusal::Short);
    };
    let mut result = [0u8; 8];
    result.copy_from_slice(&bytes[0..8]);
    let length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if bytes[12..] != [0; 4] {
        return Err(Refusal::Reserved);
    }
    Ok((
        i64::from_le_bytes(result),
        usize::try_from(length).unwrap_or(usize::MAX),
    ))
}

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{Answer, Broker, Notes, Outcome, answer_all, greet, serve};

#[cfg(test)]
mod tests {
    use super::*;

    /// A request of each kind, with what it takes.
    fn requests() -> Vec<Request> {
        let request = |kind, argument, value, first: &str, second: &str| Request {
            kind,
            argument,
            value,
            first: first.as_bytes().to_vec(),
            second: second.as_bytes().to_vec(),
        };
        vec![
            request(Kind::Open, 0, 0, "notes.txt", ""),
            request(Kind::Open, 4, 0, ".", ""),
            request(Kind::Mkdir, 0, 0, "a/b", ""),
            request(Kind::Remove, 1, 0, "a", ""),
            request(Kind::Rename, 0, 0, "a", "b"),
            request(Kind::Closed, 0, 7, "", ""),
            request(Kind::Waiting, 0, 0, "", ""),
            request(Kind::Listed, 0, 0, "", ""),
            request(Kind::Resumed, 0, 0, "", ""),
            request(Kind::Clock, 1, 0, "", ""),
            request(Kind::Random, 0, 256, "", ""),
        ]
    }

    #[test]
    fn every_request_is_read_back_as_it_was_written() {
        for request in requests() {
            assert_eq!(
                parse_request(&request.encode()),
                Ok(request.clone()),
                "{request:?}"
            );
        }
    }

    #[test]
    fn the_first_message_is_read_back_as_it_was_written() {
        for hello in [
            Hello {
                mode: Mode::Run,
                flags: 0,
                file_limit: 0,
            },
            Hello {
                mode: Mode::Test,
                flags: flag::REVERSED | flag::RANDOM | flag::CLOCK,
                file_limit: 1 << 20,
            },
        ] {
            assert_eq!(Hello::decode(&hello.encode()), Ok(hello));
        }
        let mut changed = Hello {
            mode: Mode::Run,
            flags: 0,
            file_limit: 0,
        }
        .encode();
        changed[0] ^= 1;
        assert_eq!(Hello::decode(&changed), Err(Refusal::Magic));
        assert_eq!(Hello::decode(&changed[..31]), Err(Refusal::Short));
    }

    #[test]
    fn a_reply_is_read_back_as_it_was_written() {
        let bytes = encode_reply(-2, b"abc");
        assert_eq!(parse_reply(&bytes[..REPLY_LEN]), Ok((-2, 3)));
        assert_eq!(&bytes[REPLY_LEN..], b"abc");
    }

    /// Each way a message can be wrong is refused, whatever else it holds.
    #[test]
    fn hostile_messages_are_refused() {
        let good = Request {
            kind: Kind::Open,
            argument: 1,
            value: 0,
            first: b"a".to_vec(),
            second: Vec::new(),
        }
        .encode();
        assert!(parse_request(&good).is_ok());
        let with = |at: usize, byte: u8| {
            let mut bytes = good.clone();
            bytes[at] = byte;
            bytes
        };
        let refused: [(Vec<u8>, Refusal); 14] = [
            (Vec::new(), Refusal::Short),
            (good[..REQUEST_LEN - 1].to_vec(), Refusal::Short),
            (good[..REQUEST_LEN].to_vec(), Refusal::Length(0)),
            ([good.clone(), vec![b'x']].concat(), Refusal::Length(2)),
            (with(0, 0), Refusal::Kind(0)),
            (with(0, 11), Refusal::Kind(11)),
            (with(0, 255), Refusal::Kind(255)),
            (with(1, 5), Refusal::Argument(5)),
            (with(2, 0), Refusal::Length(0)),
            (with(6, 1), Refusal::Reserved),
            (with(7, 1), Refusal::Reserved),
            (with(8, 1), Refusal::Value(1)),
            (with(REQUEST_LEN, 0), Refusal::Nul),
            (
                [with(2, 255), vec![b'x'; 255]].concat(),
                Refusal::Length(256),
            ),
        ];
        for (bytes, why) in refused {
            assert_eq!(parse_request(&bytes), Err(why.clone()), "{why:?}");
        }
        // A name past a path's length, a rename missing its second name, a count of random bytes
        // past what one call gives, and a close of no file.
        let long = Request {
            kind: Kind::Mkdir,
            argument: 0,
            value: 0,
            first: vec![b'a'; names::MAX_PATH + 1],
            second: Vec::new(),
        };
        assert_eq!(
            parse_request(&long.encode()),
            Err(Refusal::Length(names::MAX_PATH + 1))
        );
        let one_name = Request {
            kind: Kind::Rename,
            argument: 0,
            value: 0,
            first: b"a".to_vec(),
            second: Vec::new(),
        };
        assert_eq!(parse_request(&one_name.encode()), Err(Refusal::Length(1)));
        let many = Request {
            kind: Kind::Random,
            argument: 0,
            value: crate::edge::RANDOM_MOST + 1,
            first: Vec::new(),
            second: Vec::new(),
        };
        assert_eq!(
            parse_request(&many.encode()),
            Err(Refusal::Value(crate::edge::RANDOM_MOST + 1))
        );
        let none = Request {
            kind: Kind::Closed,
            argument: 0,
            value: 0,
            first: Vec::new(),
            second: Vec::new(),
        };
        assert_eq!(parse_request(&none.encode()), Err(Refusal::Value(0)));
    }
}
