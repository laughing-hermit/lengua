//! Standard input, output and error. In a test or a `run`, input is given up front and then ends,
//! arriving in the pieces the test cuts it in, each read returning at most what one pipe write
//! holds, so a program that reads as if it came whole fails on every system alike; output is kept,
//! up to a cap. A closed standard output fails every write with `PIPE`. A run from a shell reads
//! and writes this process's own.

use std::collections::VecDeque;
use std::io::{IsTerminal as _, Read as _, Write as _};

use super::{Fail, Stop};
use crate::edge::PIPE_WRITE;
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
        /// What's left of the current piece.
        piece: usize,
    },
    /// This process's own standard input, for a run from a shell.
    Here,
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
            piece,
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
                piece,
            } => {
                if *piece == 0 {
                    *piece = pieces.pop_front().unwrap_or(bytes.len() - *at);
                }
                let count = buffer
                    .len()
                    .min(*piece)
                    .min(PIPE_WRITE)
                    .min(bytes.len() - *at);
                buffer[..count].copy_from_slice(&bytes[*at..*at + count]);
                *at += count;
                *piece -= count;
                Ok(count)
            }
            Self::Here => std::io::stdin().lock().read(buffer).map_err(|_| errno::IO),
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
            Self::Here => (1, false),
        }
    }

    /// Whether this is a terminal.
    #[must_use]
    pub fn terminal(&self) -> bool {
        matches!(self, Self::Here) && std::io::stdin().is_terminal()
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
