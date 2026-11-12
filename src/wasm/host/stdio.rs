//! Standard input, output and error. In a test or a `run`, input is given up front and then ends,
//! arriving in the pieces the test cuts it in, each read returning at most what one pipe write
//! holds, so a program that reads as if it came whole fails on every system alike; output is kept,
//! up to a cap. A closed standard output fails every write with `PIPE`. A run from a shell reads
//! and writes this process's own.

use std::collections::VecDeque;
use std::io::{IsTerminal as _, Read as _, Write as _};
use std::sync::mpsc::{Receiver, TryRecvError};

use super::{Fail, Stop};
use crate::runner::PIPE_WRITE;
use crate::wasm::wasi::errno;

/// Standard input.
#[derive(Debug)]
pub enum Input {
    /// Bytes given up front, then the end.
    Given {
        /// The bytes.
        bytes: Vec<u8>,
        /// How many have been read.
        at: usize,
        /// The lengths of the pieces still to come after the current one.
        pieces: VecDeque<usize>,
        /// The most bytes one read gives: what one pipe write holds.
        most: usize,
        /// What's left of the current piece.
        piece: usize,
    },
    /// A piece's share of the bytes its program split, lent for the call that runs it.
    Share {
        /// What the program lent.
        bytes: lengua_os::lent::Lent,
        /// How far the piece has read, among them.
        at: usize,
        /// Where its share ends, among them.
        end: usize,
    },
    /// This process's own standard input, for a run from a shell.
    Here,
    /// This process's own standard input, read by a thread of lengua's: what a terminal the
    /// program made raw gives, so that a wait for a key can end at its time.
    Keys {
        /// What the thread read, a read at a time; it ends with the input.
        from: Receiver<Vec<u8>>,
        /// The bytes read and not yet given to the program.
        waiting: VecDeque<u8>,
        /// Whether the input has ended.
        ended: bool,
    },
}

impl Input {
    /// `bytes`, arriving in pieces of the lengths in `pieces`, or whole when it has fewer than two.
    #[must_use]
    pub fn given(bytes: Vec<u8>, pieces: &[usize]) -> Self {
        let mut pieces: VecDeque<usize> = if pieces.len() < 2 {
            VecDeque::new()
        } else {
            pieces.iter().copied().collect()
        };
        let piece = pieces.pop_front().unwrap_or(bytes.len());
        Self::Given {
            bytes,
            at: 0,
            pieces,
            most: PIPE_WRITE,
            piece,
        }
    }

    /// A piece's share of the bytes its program split: the bytes from `at` to `end` of what
    /// the program lent for the call, read straight from the program's memory, so a read is
    /// their one copy and gives as much as its buffer holds.
    #[must_use]
    pub fn share(bytes: lengua_os::lent::Lent, at: usize, end: usize) -> Self {
        let end = end.min(bytes.len());
        Self::Share {
            bytes,
            at: at.min(end),
            end,
        }
    }

    /// Reads into `buffer`: 0 at the end.
    ///
    /// # Errors
    ///
    /// `IO` when this process's own input fails.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, u16> {
        match self {
            Self::Given {
                bytes,
                at,
                pieces,
                most,
                piece,
            } => {
                if *piece == 0 {
                    *piece = pieces.pop_front().unwrap_or(bytes.len() - *at);
                }
                let count = buffer.len().min(*piece).min(*most).min(bytes.len() - *at);
                buffer[..count].copy_from_slice(&bytes[*at..*at + count]);
                *at += count;
                *piece -= count;
                Ok(count)
            }
            Self::Share { bytes, at, end } => {
                let room = buffer.len().min(*end - *at);
                let count = bytes.copy_to(*at, &mut buffer[..room]);
                *at += count;
                Ok(count)
            }
            Self::Here => std::io::stdin().lock().read(buffer).map_err(|_| errno::IO),
            Self::Keys {
                from,
                waiting,
                ended,
            } => {
                if buffer.is_empty() {
                    return Ok(0);
                }
                // A read waits for the first byte, as a read of the terminal does.
                if waiting.is_empty() && !*ended {
                    match from.recv() {
                        Ok(bytes) => waiting.extend(bytes),
                        Err(_) => *ended = true,
                    }
                }
                let count = buffer.len().min(waiting.len());
                for (place, byte) in buffer.iter_mut().zip(waiting.drain(..count)) {
                    *place = byte;
                }
                Ok(count)
            }
        }
    }

    /// How many bytes a read would give now without waiting, and whether the input has ended,
    /// for `poll_oneoff`; this process's own input says it is ready, and its read waits.
    #[must_use]
    pub fn ready(&self) -> (u64, bool) {
        match self {
            Self::Given { bytes, at, .. } => {
                let left = bytes.len() - *at;
                (u64::try_from(left).unwrap_or(u64::MAX), left == 0)
            }
            Self::Share { at, end, .. } => {
                let left = *end - *at;
                (u64::try_from(left).unwrap_or(u64::MAX), left == 0)
            }
            Self::Here => (1, false),
            Self::Keys { waiting, ended, .. } => {
                (u64::try_from(waiting.len()).unwrap_or(u64::MAX), *ended)
            }
        }
    }

    /// What a wait for input finds now: how many bytes a read would give without waiting,
    /// and whether the input has ended, or `None` when a read would wait. Only input a thread
    /// of lengua's reads can say that it would wait.
    pub fn pending(&mut self) -> Option<(u64, bool)> {
        if let Self::Keys {
            from,
            waiting,
            ended,
        } = self
        {
            while !*ended {
                match from.try_recv() {
                    Ok(bytes) => waiting.extend(bytes),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => *ended = true,
                }
            }
            if waiting.is_empty() && !*ended {
                return None;
            }
        }
        Some(self.ready())
    }

    /// Has a thread of lengua's read this process's own standard input from now on, so that
    /// a wait for a key can end at its time; nothing for input given up front. The thread
    /// ends with the input, or with the process.
    pub fn watch(&mut self) {
        if !matches!(self, Self::Here) {
            return;
        }
        let (read, from) = std::sync::mpsc::channel();
        let started = std::thread::Builder::new()
            .name("lengua keys".to_owned())
            .spawn(move || {
                let mut buffer = [0_u8; 1024];
                loop {
                    match std::io::stdin().lock().read(&mut buffer) {
                        Ok(0) | Err(_) => return,
                        Ok(count) => {
                            if read.send(buffer[..count].to_vec()).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
        if started.is_ok() {
            *self = Self::Keys {
                from,
                waiting: VecDeque::new(),
                ended: false,
            };
        }
    }

    /// Whether this is a terminal.
    #[must_use]
    pub fn terminal(&self) -> bool {
        matches!(self, Self::Here | Self::Keys { .. }) && std::io::stdin().is_terminal()
    }
}

/// Which of this process's own streams a program's output goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Standard output.
    Out,
    /// Standard error.
    Err,
}

/// Standard output or error.
#[derive(Debug)]
pub enum Output {
    /// Kept in memory, up to a cap: past it, the program is stopped.
    Kept {
        /// What it wrote.
        bytes: Vec<u8>,
        /// The most it may write.
        cap: usize,
    },
    /// A pipe nobody reads, as after `prog | head -1` has read its line.
    Closed,
    /// This process's own stream, for a run from a shell.
    Here(Stream),
}

impl Output {
    /// Output kept up to `cap` bytes.
    #[must_use]
    pub const fn kept(cap: usize) -> Self {
        Self::Kept {
            bytes: Vec::new(),
            cap,
        }
    }

    /// Writes `bytes`, all of them.
    ///
    /// # Errors
    ///
    /// `PIPE` for a closed stream, `IO` for this process's own that fails, and a stop once the
    /// output passes its cap.
    pub fn write(&mut self, written: &[u8]) -> Result<usize, Fail> {
        match self {
            Self::Kept { bytes, cap } => {
                let room = cap.saturating_sub(bytes.len());
                if written.len() > room {
                    bytes.extend_from_slice(&written[..room]);
                    return Err(Fail::Stop(Stop::OutputTooLarge));
                }
                bytes.extend_from_slice(written);
                Ok(written.len())
            }
            Self::Closed => Err(Fail::Errno(errno::PIPE)),
            Self::Here(stream) => {
                let result = match stream {
                    Stream::Out => {
                        let mut out = std::io::stdout().lock();
                        out.write_all(written).and_then(|()| out.flush())
                    }
                    Stream::Err => std::io::stderr().lock().write_all(written),
                };
                match result {
                    Ok(()) => Ok(written.len()),
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                        Err(Fail::Errno(errno::PIPE))
                    }
                    Err(_) => Err(Fail::Errno(errno::IO)),
                }
            }
        }
    }

    /// What it kept.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Kept { bytes, .. } => bytes,
            Self::Closed | Self::Here(_) => Vec::new(),
        }
    }

    /// Whether this is a terminal.
    #[must_use]
    pub fn terminal(&self) -> bool {
        match self {
            Self::Here(Stream::Out) => std::io::stdout().is_terminal(),
            Self::Here(Stream::Err) => std::io::stderr().is_terminal(),
            Self::Kept { .. } | Self::Closed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_all(input: &mut Input, size: usize) -> Vec<usize> {
        let mut reads = Vec::new();
        let mut buffer = vec![0_u8; size];
        loop {
            let count = input.read(&mut buffer).expect("a read");
            if count == 0 {
                return reads;
            }
            reads.push(count);
        }
    }

    #[test]
    fn input_arrives_in_its_pieces_and_then_ends() {
        let mut whole = Input::given(b"abcdef".to_vec(), &[]);
        assert_eq!(read_all(&mut whole, 100), [6]);
        let mut pieces = Input::given(b"abcdef".to_vec(), &[1, 2, 3]);
        assert_eq!(read_all(&mut pieces, 100), [1, 2, 3]);
        // A read smaller than a piece takes it in parts.
        let mut small = Input::given(b"abcdef".to_vec(), &[1, 5]);
        assert_eq!(read_all(&mut small, 2), [1, 2, 2, 1]);
        // One read gives at most what a pipe write holds.
        let mut long = Input::given(vec![7; PIPE_WRITE + 10], &[]);
        assert_eq!(read_all(&mut long, PIPE_WRITE * 2), [PIPE_WRITE, 10]);
        assert_eq!(long.ready(), (0, true));
    }

    /// Keys a thread of lengua's reads, as a raw terminal's are: a wait finds nothing until a
    /// key is there, then what is there, and the end once the thread has ended and every key
    /// was read. A read gives what has come, and never more than the buffer holds.
    #[test]
    fn keys_wait_until_one_is_there_and_end_with_their_thread() {
        let (keys, from) = std::sync::mpsc::channel();
        let mut input = Input::Keys {
            from,
            waiting: VecDeque::new(),
            ended: false,
        };
        assert_eq!(input.pending(), None);
        keys.send(b"ab".to_vec()).expect("the input takes keys");
        keys.send(b"\x1b[A".to_vec()).expect("the input takes keys");
        assert_eq!(input.pending(), Some((5, false)));
        let mut one = [0_u8; 1];
        assert_eq!(input.read(&mut one), Ok(1));
        assert_eq!(&one, b"a");
        let mut rest = [0_u8; 16];
        assert_eq!(input.read(&mut rest), Ok(4));
        assert_eq!(&rest[..4], b"b\x1b[A");
        assert_eq!(input.pending(), None);
        drop(keys);
        assert_eq!(input.pending(), Some((0, true)));
        assert_eq!(input.read(&mut rest), Ok(0));
        // Input given up front never waits, and watching it changes nothing.
        let mut given = Input::given(b"x".to_vec(), &[]);
        given.watch();
        assert_eq!(given.pending(), Some((1, false)));
        assert!(matches!(given, Input::Given { .. }));
    }

    #[test]
    fn output_is_kept_up_to_its_cap_and_a_closed_stream_is_a_broken_pipe() {
        let mut kept = Output::kept(4);
        assert_eq!(kept.write(b"ab").ok(), Some(2));
        assert!(matches!(
            kept.write(b"cde"),
            Err(Fail::Stop(Stop::OutputTooLarge))
        ));
        assert_eq!(kept.into_bytes(), b"abcd");
        let mut closed = Output::Closed;
        assert!(matches!(closed.write(b"x"), Err(Fail::Errno(errno::PIPE))));
    }
}
