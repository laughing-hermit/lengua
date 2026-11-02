//! The connections lengua makes for a program. A program never connects by itself: for each
//! place its spec names, lengua's runner, outside the sandbox, makes the connection before the
//! program starts and hands the program one end of a local stream, which it relays to the place
//! both ways. So the program holds no network at all, on every system, and a connection behaves
//! the same everywhere:
//! - what the place sends arrives on the stream, and the place's end of sending is the program's
//!   end of input: a read gives 0;
//! - the program's `shutdown` of its sending side is the place's end of input;
//! - a connection lengua couldn't make, or one the place broke off, is shut both ways: a read
//!   gives 0 once what had arrived is read, and a write fails with `EPIPE`.
//!
//! The program can't tell a place that refused it from one that broke off: lengua says which in
//! the run's report. Each end lengua keeps stays open until the program ends, since a stream
//! closed while it holds bytes the program sent would make the program's next read fail with
//! `ECONNRESET` instead of ending.

use std::io;
use std::os::fd::AsFd;
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::oneshot;

/// The descriptor a program's first connection has: 3 is a server's listener, so each
/// connection's number depends only on its place in the spec.
pub const FIRST_DESCRIPTOR: i32 = 4;

/// The most connections a spec may name.
pub const MAX_CONNECTIONS: usize = 16;

/// How long lengua tries to make a connection before the program starts without it.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long lengua waits, once the program has ended, for what it sent last to reach the place.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Where one connection goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// A port of this computer: its loopback, 127.0.0.1 and then ::1.
    ThisComputer(u16),
    /// A port of the computer with this name or address.
    Host(String, u16),
    /// The test's own server, which speaks on the program's stream itself.
    Peer,
    /// Nowhere: the connection is handed over already ended, as one lengua couldn't make is.
    Nowhere,
}

impl Reach {
    /// The place in words, for a report: `port 8080 of this computer`, `example.com:443`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::ThisComputer(port) => format!("port {port} of this computer"),
            Self::Host(host, port) => format!("{host}:{port}"),
            Self::Peer => "the test's server".into(),
            Self::Nowhere => "nowhere".into(),
        }
    }
}

/// What became of one connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// The place, in words.
    pub place: String,
    /// Why lengua couldn't make it, when it couldn't: the program got it already ended.
    pub problem: Option<String>,
}

/// One connection's two ends: the program's, which goes to the launcher, and lengua's.
pub(crate) struct Pair {
    /// The program's end, blocking, as a program's descriptors are.
    pub program: std::os::unix::net::UnixStream,
    /// lengua's end.
    pub lengua: UnixStream,
}

/// A new connection's two ends.
pub(crate) fn pair() -> io::Result<Pair> {
    let (program, lengua) = std::os::unix::net::UnixStream::pair()?;
    lengua.set_nonblocking(true)?;
    Ok(Pair {
        program,
        lengua: UnixStream::from_std(lengua)?,
    })
}

/// Makes the connection `reach` names, trying each of its addresses in turn until
/// [`CONNECT_TIMEOUT`] has passed, or says why it couldn't.
pub(crate) async fn make(reach: &Reach) -> Result<TcpStream, String> {
    let (host, port) = match reach {
        Reach::ThisComputer(port) => (None, *port),
        Reach::Host(host, port) => (Some(host.as_str()), *port),
        Reach::Peer | Reach::Nowhere => return Err("there's nothing there".into()),
    };
    let attempt = async {
        let addresses: Vec<std::net::SocketAddr> = match host {
            None => vec![
                (std::net::Ipv4Addr::LOCALHOST, port).into(),
                (std::net::Ipv6Addr::LOCALHOST, port).into(),
            ],
            Some(host) => tokio::net::lookup_host((host, port))
                .await
                .map_err(|error| format!("{host} isn't a computer lengua can find: {error}"))?
                .collect(),
        };
        let mut last = String::from("it has no address");
        for address in addresses {
            match TcpStream::connect(address).await {
                Ok(stream) => return Ok(stream),
                Err(error) => last = format!("{address}: {error}"),
            }
        }
        Err(last)
    };
    tokio::time::timeout(CONNECT_TIMEOUT, attempt)
        .await
        .unwrap_or_else(|_| {
            Err(format!(
                "no answer within {} seconds",
                CONNECT_TIMEOUT.as_secs()
            ))
        })
}

/// Shuts `stream` both ways: the program's reads give 0 once what had arrived is read, and its
/// writes fail with `EPIPE`.
pub(crate) fn shut(stream: &impl AsFd) {
    let _ = rustix::net::shutdown(stream, rustix::net::Shutdown::Both);
}

/// A connection lengua relays between the program's stream and its place.
pub(crate) struct Relay {
    task: tokio::task::JoinHandle<()>,
    /// Says when everything the program sent has gone to the place, with its end of sending.
    sent: Option<oneshot::Receiver<()>>,
}

impl Relay {
    /// Relays `program`, lengua's end of the program's stream, and `place` both ways until both
    /// have ended. Where either breaks off, the program's stream is shut both ways and held
    /// until the relay is dropped.
    pub(crate) fn start(program: UnixStream, place: TcpStream) -> Self {
        let (done, sent) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut from_program, mut to_program) = program.into_split();
            let (mut from_place, mut to_place) = place.into_split();
            let outward = async {
                let copied = tokio::io::copy(&mut from_program, &mut to_place).await;
                let ended = to_place.shutdown().await;
                let _ = done.send(());
                let clean = copied.is_ok() && ended.is_ok();
                if !clean {
                    shut(from_program.as_ref());
                }
                clean
            };
            let inward = async {
                match tokio::io::copy(&mut from_place, &mut to_program).await {
                    Ok(_) => to_program.shutdown().await.is_ok(),
                    Err(_) => {
                        shut(to_program.as_ref());
                        false
                    }
                }
            };
            let (out, back) = tokio::join!(outward, inward);
            if !(out && back) {
                // Held until the run ends: closing it now could turn the program's end of input
                // into ECONNRESET.
                std::future::pending::<()>().await;
            }
            drop((from_program, to_program, from_place, to_place));
        });
        Self {
            task,
            sent: Some(sent),
        }
    }

    /// Waits a moment for what the program sent last to reach the place, now that the program
    /// has ended, then stops relaying.
    pub(crate) async fn finish(&mut self) {
        if let Some(sent) = self.sent.take() {
            let _ = tokio::time::timeout(FLUSH_TIMEOUT, sent).await;
        }
        self.task.abort();
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}
