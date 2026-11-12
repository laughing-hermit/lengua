//! What lengua's server and a WebAssembly worker (`lengua-mcp --wasm-worker`) say to each other:
//! length-prefixed JSON, each message four bytes of its length, little-endian, then the JSON.
//! The server sends a [`Request`]; the worker answers with [`Event`]s, the last of a job its
//! [`Done`]. Bytes travel as lowercase hex.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::reach::Stand;
use crate::runner::Trap;
use crate::wasm::engine::Setup;
use crate::wasm::host::policy::Policy;

/// The longest message either side takes: a job's output is 8 MiB a stream at most, twice that
/// in hex.
pub const MAX_MESSAGE: usize = 64 * 1024 * 1024;

/// What the server asks of a worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Run this job, then say how it went.
    Run(Box<Job>),
    /// Stop the job with this number now, as lengua stops a server at the end of its test.
    Stop(u64),
}

/// What a worker tells the server while a job runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// The job's program serves on this port, with the connections lengua made for it first.
    Listening {
        /// The job.
        id: u64,
        /// The port.
        port: u16,
        /// What became of each connection, in order, as the job's [`Done`] says it again.
        connections: Vec<Reached>,
    },
    /// The job's program waits for a connection now, or is back from waiting.
    Waiting {
        /// The job.
        id: u64,
        /// Whether it waits.
        waiting: bool,
    },
    /// The job is done.
    Done(Box<Done>),
}

/// One connection lengua makes for the program before it starts, in the idea's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Connection {
    /// The test's own server, listening on this port of the loopback.
    Test(u16),
    /// A place: a port of this computer when `host` is `None`, over TLS when `tls`.
    Place {
        /// The computer, by name or address.
        host: Option<String>,
        /// Its port.
        port: u16,
        /// Whether lengua does TLS on it.
        tls: bool,
    },
    /// Nowhere: handed over already ended, for this reason.
    Ended(String),
}

/// One connection, with the place it goes to in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Made {
    /// The place, in words, as the run's report says it.
    pub place: String,
    /// How lengua reaches it.
    pub connection: Connection,
}

/// A test's own internet, which stands in for the real one in everything a program opens
/// itself: a program in a test reaches the test's own servers and nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Internet {
    /// The computers the test stands in for, each with the port of the loopback where the
    /// test's own server for it answers. Any other computer has nothing there.
    pub places: Vec<(Stand, u16)>,
    /// The names the test gives addresses, each with its addresses. Any other name has none.
    pub names: Vec<(String, Vec<String>)>,
    /// What the test says each command gives, in place of starting a program: a test starts
    /// nothing, and a command none of these names isn't there.
    #[serde(default)]
    pub programs: Vec<crate::starts::Stood>,
}

/// How far a job may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Its time, in milliseconds, if it has a deadline.
    pub timeout_ms: Option<u64>,
    /// The most bytes standard output, and standard error, may hold.
    pub max_output: usize,
    /// The most bytes a file may hold, if there's a limit.
    pub max_file: Option<u64>,
    /// The most bytes it may write to files in all, if there's a limit.
    pub max_written: Option<u64>,
}

/// One run of a module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Its number, which the worker's events about it carry.
    pub id: u64,
    /// The module's file.
    pub module: PathBuf,
    /// The module's SHA-256, in hex: the worker keeps compiled modules by it.
    pub hash: String,
    /// The engine's configuration it runs in.
    pub setup: Setup,
    /// What the program's idea lets it reach.
    pub policy: Policy,
    /// Its arguments, after its name.
    pub args: Vec<String>,
    /// Its environment variables.
    pub env: BTreeMap<String, String>,
    /// Its standard input, in hex.
    pub stdin: String,
    /// The pieces standard input arrives in, by length; whole when fewer than two.
    pub stdin_pieces: Vec<usize>,
    /// Whether its standard output is a pipe nobody reads.
    pub stdout_closed: bool,
    /// Its working folder, when it may use files.
    pub folder: Option<PathBuf>,
    /// The connections lengua makes for it, in order.
    pub connections: Vec<Made>,
    /// The port it serves on, 0 for any free one, and whether every address may reach it.
    pub listen: Option<(u16, bool)>,
    /// A test's own internet, in place of the real one, for what the program opens itself;
    /// `None` for a run that reaches the real one.
    #[serde(default)]
    pub internet: Option<Internet>,
    /// lengua's own folders, with their links followed, which no file or folder an argument
    /// names may be, lie in or hold: the data folder and the folder of compiled modules.
    #[serde(default)]
    pub own: Vec<PathBuf>,
    /// The random bytes a test supplies, in hex.
    pub random: Option<String>,
    /// The clock readings a test supplies: seconds and nanoseconds.
    pub clock: Vec<(i64, u32)>,
    /// Whether each folder lists turned round, as in a test's second run.
    pub reversed: bool,
    /// Whether a sleep ends at once, as in a test.
    pub instant_sleeps: bool,
    /// How far it may go.
    pub limits: Limits,
    /// The tools the run is asked for: none, unless the server asks.
    #[serde(default)]
    pub tools: crate::wasm::tools::Tools,
}

/// How a job's program ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ending {
    /// It exited with this code.
    Exited(i32),
    /// It ran past its time.
    TimedOut,
    /// It printed past its cap.
    OutputTooLarge,
    /// lengua stopped it.
    Stopped,
    /// It trapped, at these offsets of its module, as far as the engine knows them: each frame's,
    /// the innermost first. The first in main.wat's own code is its place, so a trap in lengua's
    /// library is placed at the call of it main.wat made.
    Trapped(Trap, Vec<u64>),
    /// It never started: the job's `problem` says why.
    Unstarted,
}

/// What became of one connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reached {
    /// The place, in words.
    pub place: String,
    /// Why lengua couldn't make it, when it couldn't.
    pub problem: Option<String>,
}

/// How a job went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// The job.
    pub id: u64,
    /// Its standard output, in hex.
    pub stdout: String,
    /// Its standard error, in hex.
    pub stderr: String,
    /// How it ended.
    pub ending: Ending,
    /// The WebAssembly operations it executed, in a test.
    pub instructions: Option<u64>,
    /// Its time on the clock, in nanoseconds.
    pub wall_time_ns: u64,
    /// How many random bytes it asked for, when the test supplied them.
    pub random_asked: Option<usize>,
    /// Whether it listed a folder.
    pub listed: bool,
    /// Whether it split its work into pieces.
    #[serde(default)]
    pub split: bool,
    /// What became of each connection, in order.
    pub connections: Vec<Reached>,
    /// Why lengua couldn't run it as asked, if it couldn't.
    pub problem: Option<String>,
    /// What the run's tools found, for a run that was asked for any.
    #[serde(default)]
    pub found: Option<Box<crate::wasm::tools::Found>>,
}

/// `bytes` in lowercase hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    crate::hash::hex(bytes)
}

/// The bytes lowercase or uppercase hex `text` writes.
///
/// # Errors
///
/// When it isn't pairs of hex digits.
pub fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("hex of an odd length".to_owned());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| {
            text.get(at..at + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| format!("{:?} isn't hex", text.get(at..at + 2)))
        })
        .collect()
}

/// Writes `message`, its length first.
///
/// # Errors
///
/// When it can't be encoded or written.
pub fn write_message(writer: &mut impl Write, message: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
    let length = u32::try_from(bytes.len())
        .ok()
        .filter(|length| usize::try_from(*length).is_ok_and(|length| length <= MAX_MESSAGE))
        .ok_or_else(|| io::Error::other("a message too long"))?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Reads one message, or `None` at the end.
///
/// # Errors
///
/// When it can't be read or isn't a message.
pub fn read_message<T: DeserializeOwned>(reader: &mut impl Read) -> io::Result<Option<T>> {
    let mut length = [0_u8; 4];
    match reader.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = usize::try_from(u32::from_le_bytes(length)).unwrap_or(usize::MAX);
    if length > MAX_MESSAGE {
        return Err(io::Error::other("a message too long"));
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(io::Error::other)
}

/// [`write_message`], for the server's side.
///
/// # Errors
///
/// When it can't be encoded or written.
pub async fn send(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    message: &impl Serialize,
) -> io::Result<()> {
    use tokio::io::AsyncWriteExt as _;
    let mut bytes = Vec::new();
    write_message(&mut bytes, message)?;
    writer.write_all(&bytes).await?;
    writer.flush().await
}

/// [`read_message`], for the server's side.
///
/// # Errors
///
/// When it can't be read or isn't a message.
pub async fn receive<T: DeserializeOwned>(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> io::Result<Option<T>> {
    use tokio::io::AsyncReadExt as _;
    let mut length = [0_u8; 4];
    match reader.read_exact(&mut length).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = usize::try_from(u32::from_le_bytes(length)).unwrap_or(usize::MAX);
    if length > MAX_MESSAGE {
        return Err(io::Error::other("a message too long"));
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_go_as_hex_and_come_back() {
        assert_eq!(hex(&[0, 0x2a, 0xff]), "002aff");
        assert_eq!(unhex("002aFF"), Ok(vec![0, 0x2a, 0xff]));
        assert!(unhex("0").is_err());
        assert!(unhex("zz").is_err());
    }

    #[test]
    fn a_message_is_read_back_as_it_was_written() {
        let messages = [
            Request::Stop(7),
            Request::Run(Box::new(Job {
                id: 3,
                module: PathBuf::from("main.wasm"),
                hash: "ab".repeat(32),
                setup: Setup::Test,
                policy: Policy::default(),
                args: vec!["first".to_owned()],
                env: BTreeMap::from([("NAME".to_owned(), "value".to_owned())]),
                stdin: hex(b"abc"),
                stdin_pieces: vec![1, 2],
                stdout_closed: false,
                folder: None,
                connections: vec![Made {
                    place: "the test's server".to_owned(),
                    connection: Connection::Test(4000),
                }],
                listen: Some((0, false)),
                internet: Some(Internet {
                    places: vec![(
                        Stand {
                            datagrams: false,
                            host: "example.org".to_owned(),
                            port: 80,
                        },
                        4001,
                    )],
                    names: vec![("example.org".to_owned(), vec!["192.0.2.7".to_owned()])],
                    programs: vec![crate::starts::Stood {
                        program: "hostname".to_owned(),
                        arguments: vec!["-s".to_owned()],
                        output: "box\n".to_owned(),
                        code: 0,
                    }],
                }),
                own: vec![PathBuf::from("data")],
                random: Some(hex(&[1, 2])),
                clock: vec![(1, 2)],
                reversed: true,
                instant_sleeps: true,
                limits: Limits {
                    timeout_ms: Some(2000),
                    max_output: 100,
                    max_file: Some(64),
                    max_written: None,
                },
                tools: crate::wasm::tools::Tools::default(),
            })),
        ];
        let mut wire = Vec::new();
        for message in &messages {
            write_message(&mut wire, message).expect("written");
        }
        let mut reader = wire.as_slice();
        for message in &messages {
            let read: Option<Request> = read_message(&mut reader).expect("read");
            assert_eq!(read.as_ref(), Some(message));
        }
        let end: Option<Request> = read_message(&mut reader).expect("the end");
        assert_eq!(end, None);
    }
}
