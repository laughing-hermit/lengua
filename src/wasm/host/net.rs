//! The network: the listener lengua binds for a server, and the connections lengua makes before
//! the program starts or the program accepts. The program never connects by itself. A connection
//! behaves as `crate::connect` says on every system: what the other end sends is read, its end of
//! sending is a read of 0, a connection lengua couldn't make or one that broke off is ended both
//! ways (reads give 0, writes `PIPE`), and over TLS lengua does the TLS, in Rust, so the program
//! reads and writes plain bytes. Every wait is bounded by the run's deadline and lengua's stop,
//! and none is the system's own timeout on a socket, which on Windows can lose or repeat what the
//! socket was sending or receiving when it runs out: a TCP stream never waits in the system, and
//! lengua naps between looks at it.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::fds::Descriptor;
use super::limits::{Limits, SLICE};
use super::{Fail, Host, memory};
use crate::connect::Place;
use crate::wasm::wasi::{errno, fdflags, sock};

/// The most a socket reads ahead at once. A read of a connection gives only what one take
/// brought in, so none gives more, and none goes past where a take ended: the prover's model
/// of WebAssembly cuts its reads where the host does.
pub const CHUNK: usize = 16 * 1024;

/// The first nap a wait on a TCP stream takes between two looks at it. Each nap is twice the one
/// before, up to [`LAST_NAP`], so an answer that comes soon is seen soon, and a long wait costs
/// little.
const FIRST_NAP: Duration = Duration::from_micros(50);

/// The longest nap.
const LAST_NAP: Duration = Duration::from_millis(1);

/// The nap after `nap`.
fn next_nap(nap: Duration) -> Duration {
    nap.saturating_mul(2).min(LAST_NAP)
}

/// Whether a stream that never waits said it has nothing to give or take yet.
fn nothing_yet(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted)
}

/// What a write comes to on a connection that ended after it took `taken` bytes: those, or
/// `PIPE` where it took none.
fn ended(taken: usize) -> Result<usize, Fail> {
    if taken > 0 {
        Ok(taken)
    } else {
        Err(Fail::Errno(errno::PIPE))
    }
}

/// The runtime that drives the connections lengua makes to places, TLS among them: one for the
/// process, on the thread of whichever call drives it.
#[must_use]
pub fn runtime() -> Option<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
}

/// How lengua reaches one of a program's connections. It stays in the crate, as the streams
/// lengua makes to places do ([`crate::connect`]).
pub(crate) enum Link {
    /// A TCP stream that never waits in the system, as [`Link::tcp`] makes it: one the program
    /// accepted, or one lengua made to a test's own server.
    Tcp(TcpStream),
    /// A stream lengua made to a place, over TLS or not, which `runtime` drives.
    Place(Box<dyn Place>, &'static tokio::runtime::Runtime),
    /// A connection lengua couldn't make, or one that ended: reads give 0, writes `PIPE`.
    Ended,
}

impl Link {
    /// A link over `stream`, which never waits in the system from now on: lengua does the
    /// waiting.
    ///
    /// # Errors
    ///
    /// When the system can't make it so.
    pub(crate) fn tcp(stream: TcpStream) -> std::io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self::Tcp(stream))
    }
}

/// One connection the program holds.
pub struct Socket {
    /// How lengua reaches it.
    link: Link,
    /// What has arrived and the program hasn't read yet.
    ahead: Vec<u8>,
    /// Whether the other end has ended what it sends.
    ended: bool,
    /// Whether the program has shut its reading.
    read_shut: bool,
    /// Whether the program has shut its writing.
    write_shut: bool,
    /// Whether a read that would wait answers `AGAIN` instead.
    nonblocking: bool,
}

impl Socket {
    /// A connection over `link`.
    #[must_use]
    pub(crate) fn new(link: Link) -> Self {
        Self {
            link,
            ahead: Vec::new(),
            ended: false,
            read_shut: false,
            write_shut: false,
            nonblocking: false,
        }
    }

    /// Tries to receive into `chunk`, waiting at most `wait`, or not at all for `None`: how many
    /// bytes arrived, 0 at the end, or `None` when nothing has yet. A TCP stream is looked at
    /// once, and the wait is a nap after it, when nothing was there.
    fn receive(&mut self, chunk: &mut [u8], wait: Option<Duration>) -> Option<usize> {
        match &mut self.link {
            Link::Ended => Some(0),
            Link::Tcp(stream) => match stream.read(chunk) {
                Ok(count) => Some(count),
                Err(error) if nothing_yet(&error) => {
                    if let Some(wait) = wait {
                        std::thread::sleep(wait);
                    }
                    None
                }
                // A connection the other end broke off ends, as lengua's relay ends it.
                Err(_) => Some(0),
            },
            Link::Place(place, runtime) => {
                let wait = wait.unwrap_or(Duration::ZERO);
                let received =
                    runtime.block_on(async { tokio::time::timeout(wait, place.read(chunk)).await });
                match received {
                    Ok(Ok(count)) => Some(count),
                    Ok(Err(_)) => Some(0),
                    Err(_) => None,
                }
            }
        }
    }

    /// Takes what arrived into what's ahead, waiting `wait` at most, or not at all for `None`.
    fn take_in(&mut self, wait: Option<Duration>) {
        let mut chunk = vec![0_u8; CHUNK];
        match self.receive(&mut chunk, wait) {
            Some(0) => self.ended = true,
            Some(count) => self.ahead.extend_from_slice(&chunk[..count]),
            None => {}
        }
    }

    /// Whether a read would give something now, bytes or the end, without waiting.
    pub fn readable(&mut self) -> bool {
        if self.ahead.is_empty() && !self.ended && !self.read_shut {
            self.take_in(None);
        }
        !self.ahead.is_empty() || self.ended || self.read_shut
    }

    /// How many bytes are there to read now, for `poll_oneoff`, and whether the other end ended.
    #[must_use]
    pub fn waiting_bytes(&self) -> (u64, bool) {
        (
            u64::try_from(self.ahead.len()).unwrap_or(u64::MAX),
            self.ended && self.ahead.is_empty(),
        )
    }

    /// Reads into `buffer`, waiting until something arrives or the other end ends: 0 at the end.
    /// A peek leaves what it read to be read again.
    ///
    /// # Errors
    ///
    /// `AGAIN` for a socket that doesn't wait, and a stop at the deadline.
    pub fn read(&mut self, buffer: &mut [u8], limits: &Limits, peek: bool) -> Result<usize, Fail> {
        if buffer.is_empty() || self.read_shut {
            return Ok(0);
        }
        if self.nonblocking && !self.readable() {
            return Err(Fail::Errno(errno::AGAIN));
        }
        let mut nap = FIRST_NAP;
        while self.ahead.is_empty() && !self.ended {
            let slice = limits.slice(SLICE)?;
            // A TCP stream naps between looks; a place's runtime waits for what arrives.
            let wait = match self.link {
                Link::Tcp(_) => nap.min(slice),
                Link::Place(..) | Link::Ended => slice,
            };
            self.take_in(Some(wait));
            nap = next_nap(nap);
        }
        let count = buffer.len().min(self.ahead.len());
        buffer[..count].copy_from_slice(&self.ahead[..count]);
        if !peek {
            let left = self.ahead.len() - count;
            self.ahead.copy_within(count.., 0);
            self.ahead.truncate(left);
        }
        Ok(count)
    }

    /// Sends every byte of `bytes`, waiting while the connection takes them, as a blocking send
    /// does: a program's one write sends all it gives, however much room the system's buffers
    /// have, so it does the same on every system. Fewer only where the connection ends after it
    /// took some. Over TLS, the bytes TLS takes are taken once, however many slices it takes to
    /// send them on.
    ///
    /// # Errors
    ///
    /// `PIPE` for a connection that ended, or the program shut, before it took any, and a stop
    /// at the deadline.
    pub fn write(&mut self, bytes: &[u8], limits: &Limits) -> Result<usize, Fail> {
        if self.write_shut {
            return Err(Fail::Errno(errno::PIPE));
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        let mut nap = FIRST_NAP;
        // How many of `bytes` the connection has taken. Over TLS, TLS takes them before the
        // socket does, so a slice that runs out while it sends them on leaves them taken, and the
        // next slice only sends them on: written again, they would go twice.
        let mut taken = 0;
        loop {
            let wait = limits.slice(SLICE)?;
            match &mut self.link {
                Link::Ended => return ended(taken),
                Link::Tcp(stream) => match stream.write(&bytes[taken..]) {
                    Ok(0) => return ended(taken),
                    Ok(count) => {
                        taken += count;
                        if taken == bytes.len() {
                            return Ok(taken);
                        }
                    }
                    Err(error) if nothing_yet(&error) => {
                        std::thread::sleep(nap.min(wait));
                        nap = next_nap(nap);
                    }
                    Err(_) => return ended(taken),
                },
                Link::Place(place, runtime) => {
                    let sent = runtime.block_on(async {
                        tokio::time::timeout(wait, async {
                            while taken < bytes.len() {
                                let count = place.write(&bytes[taken..]).await?;
                                if count == 0 {
                                    return Err(std::io::Error::from(ErrorKind::WriteZero));
                                }
                                taken += count;
                            }
                            place.flush().await?;
                            Ok::<(), std::io::Error>(())
                        })
                        .await
                    });
                    match sent {
                        Ok(Ok(())) => return Ok(taken),
                        Ok(Err(_)) => return ended(taken),
                        // The slice ran out: what was taken is sent on in the next.
                        Err(_) => {}
                    }
                }
            }
        }
    }

    /// `sock_shutdown`: `how` holds `SHUT_RD`, `SHUT_WR` or both.
    ///
    /// # Errors
    ///
    /// `INVAL` for anything else.
    pub fn shutdown(&mut self, how: u8) -> Result<(), Fail> {
        if how == 0 || how & !(sock::SHUT_RD | sock::SHUT_WR) != 0 {
            return Err(Fail::Errno(errno::INVAL));
        }
        if how & sock::SHUT_RD != 0 {
            self.read_shut = true;
            self.ahead.clear();
        }
        if how & sock::SHUT_WR != 0 && !self.write_shut {
            self.write_shut = true;
            match &mut self.link {
                Link::Tcp(stream) => {
                    let _ = stream.shutdown(Shutdown::Write);
                }
                Link::Place(place, runtime) => {
                    let _ = runtime.block_on(async {
                        tokio::time::timeout(Duration::from_secs(2), place.shutdown()).await
                    });
                }
                Link::Ended => {}
            }
        }
        Ok(())
    }

    /// Closes it: what the program sent ends there.
    pub fn close(&mut self) {
        let _ = self.shutdown(sock::SHUT_RD | sock::SHUT_WR);
    }
}

/// The listener a server's connections come in on.
pub struct Listener {
    /// The listening socket, which never waits: lengua does the waiting.
    listener: TcpListener,
    /// Connections taken while looking whether one was there, in order.
    queued: VecDeque<TcpStream>,
}

impl Listener {
    /// Listens on `port` of this computer's loopback, or of every address when `anywhere`; 0
    /// for any free port.
    ///
    /// # Errors
    ///
    /// When the port can't be had, such as one that's taken.
    pub fn bind(port: u16, anywhere: bool) -> std::io::Result<Self> {
        let address = if anywhere {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let listener = TcpListener::bind((address, port))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            queued: VecDeque::new(),
        })
    }

    /// The port it listens on.
    ///
    /// # Errors
    ///
    /// When the system can't say.
    pub fn port(&self) -> std::io::Result<u16> {
        Ok(self.listener.local_addr()?.port())
    }

    /// Whether a connection is there to take now.
    pub fn ready(&mut self) -> bool {
        if !self.queued.is_empty() {
            return true;
        }
        match self.listener.accept() {
            Ok((stream, _)) => {
                self.queued.push_back(stream);
                true
            }
            Err(_) => false,
        }
    }

    /// The next connection, waiting for it within the run's limits.
    ///
    /// # Errors
    ///
    /// A stop at the deadline, and `IO` when the system fails.
    pub fn accept(&mut self, limits: &Limits) -> Result<TcpStream, Fail> {
        loop {
            if let Some(stream) = self.queued.pop_front() {
                return Ok(stream);
            }
            match self.listener.accept() {
                Ok((stream, _)) => return Ok(stream),
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    limits.check()?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return Err(Fail::Errno(errno::IO)),
            }
        }
    }
}

/// Whether the program holds as many descriptors as every system lets it.
fn full(host: &Host) -> bool {
    u64::try_from(host.fds.len()).unwrap_or(u64::MAX) >= crate::runner::OPEN_FILES
}

/// `sock_accept(fd, flags, out)`: the next connection on the listener, descriptor 4, as a new
/// descriptor; `flags` may make it one whose reads don't wait.
///
/// # Errors
///
/// A buffer outside memory, and a stop at the deadline.
pub fn sock_accept(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    flags: u16,
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    if full(host) {
        return Err(Fail::Errno(errno::MFILE));
    }
    let listener = match host.fds.get_mut(fd) {
        Some(Descriptor::Listener(listener)) => listener,
        Some(Descriptor::Socket(_)) => return Err(Fail::Errno(errno::INVAL)),
        Some(_) => return Err(Fail::Errno(errno::NOTSOCK)),
        None => return Err(Fail::Errno(errno::BADF)),
    };
    let waits = !listener.ready();
    if waits {
        host.notes.wait(true);
    }
    let accepted = listener.accept(&host.limits);
    if waits {
        host.notes.wait(false);
    }
    // Whatever way of waiting it got from the listener, it never waits in the system.
    let link = Link::tcp(accepted?).map_err(|_| Fail::Errno(errno::IO))?;
    let mut socket = Socket::new(link);
    socket.nonblocking = flags & fdflags::NONBLOCK != 0;
    let accepted = host
        .fds
        .add(Descriptor::Socket(Box::new(socket)))
        .map_err(Fail::Errno)?;
    memory::write_u32(memory, out, accepted)
}

/// The connection `fd` is, for a socket's function, with the run's limits its waits keep to.
fn socket(host: &mut Host, fd: u32) -> Result<(&mut Socket, &Limits), Fail> {
    let Host { fds, limits, .. } = host;
    match fds.get_mut(fd) {
        Some(Descriptor::Socket(socket)) => Ok((socket.as_mut(), &*limits)),
        Some(Descriptor::Listener(_)) => Err(Fail::Errno(errno::INVAL)),
        Some(_) => Err(Fail::Errno(errno::NOTSOCK)),
        None => Err(Fail::Errno(errno::BADF)),
    }
}

/// `sock_recv(fd, iovs, count, flags, received, out_flags)`: what arrived, into the first buffer
/// with room, or into all of them, in order, with `RECV_WAITALL`.
///
/// # Errors
///
/// A buffer outside memory, and a stop at the deadline.
pub fn sock_recv(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    iovs: (u32, u32),
    flags: u16,
    out: (u32, u32),
) -> Result<(), Fail> {
    memory::check(memory, out.0, 4)?;
    memory::check(memory, out.1, 2)?;
    let buffers = memory::iovecs(memory, iovs.0, iovs.1)?;
    if flags & !(sock::RECV_PEEK | sock::RECV_WAITALL) != 0 {
        return Err(Fail::Errno(errno::INVAL));
    }
    let peek = flags & sock::RECV_PEEK != 0;
    let all = flags & sock::RECV_WAITALL != 0 && !peek;
    let (socket, limits) = socket(host, fd)?;
    let mut total = 0;
    'buffers: for range in buffers.into_iter().filter(|range| !range.is_empty()) {
        let mut at = range.start;
        while at < range.end {
            let count = socket.read(&mut memory[at..range.end], limits, peek)?;
            if count == 0 {
                break 'buffers;
            }
            at += count;
            total += count;
            if !all {
                break 'buffers;
            }
        }
    }
    memory::write_u32(memory, out.0, u32::try_from(total).unwrap_or(u32::MAX))?;
    memory::write_u16(memory, out.1, 0)
}

/// `sock_send(fd, iovs, count, flags, sent)`: every byte of the buffers, in order, as a blocking
/// send takes them.
///
/// # Errors
///
/// A buffer outside memory, and a stop at the deadline.
pub fn sock_send(
    host: &mut Host,
    memory: &mut [u8],
    fd: u32,
    iovs: (u32, u32),
    out: u32,
) -> Result<(), Fail> {
    memory::check(memory, out, 4)?;
    let buffers = memory::iovecs(memory, iovs.0, iovs.1)?;
    let (socket, limits) = socket(host, fd)?;
    let mut total = 0;
    for range in buffers {
        let wanted = range.len();
        let sent = match socket.write(&memory[range], limits) {
            Ok(sent) => sent,
            Err(Fail::Errno(_)) if total > 0 => break,
            Err(failure) => return Err(failure),
        };
        total += sent;
        if sent < wanted {
            break;
        }
    }
    memory::write_u32(memory, out, u32::try_from(total).unwrap_or(u32::MAX))
}

/// `sock_shutdown(fd, how)`.
///
/// # Errors
///
/// `INVAL` for a `how` that isn't one.
pub fn sock_shutdown(host: &mut Host, fd: u32, how: u8) -> Result<(), Fail> {
    let (socket, _) = socket(host, fd)?;
    socket.shutdown(how)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn limits() -> Limits {
        Limits::new(
            Some(Duration::from_secs(10)),
            Arc::new(AtomicBool::new(false)),
            100,
            None,
            None,
            0,
        )
    }

    /// A connection to a listener of the test's, and the test's end of it.
    fn connected() -> (Socket, TcpStream) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let port = listener.local_addr().expect("an address").port();
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("a connection");
        let (other, _) = listener.accept().expect("its other end");
        let link = Link::tcp(stream).expect("a stream that never waits");
        (Socket::new(link), other)
    }

    #[test]
    fn a_connection_reads_what_arrives_and_then_the_end() {
        let (mut socket, mut other) = connected();
        let limits = limits();
        std::io::Write::write_all(&mut other, b"hello").expect("sent");
        let mut buffer = [0_u8; 3];
        assert_eq!(socket.read(&mut buffer, &limits, true).ok(), Some(3));
        assert_eq!(&buffer, b"hel");
        assert_eq!(socket.read(&mut buffer, &limits, false).ok(), Some(3));
        assert_eq!(&buffer, b"hel", "a peek leaves what it read");
        assert_eq!(socket.read(&mut buffer, &limits, false).ok(), Some(2));
        drop(other);
        assert_eq!(socket.read(&mut buffer, &limits, false).ok(), Some(0));
        assert!(socket.readable());
    }

    /// A read gives at most what one take of what arrived brought in, and no more than that take
    /// has left, as the prover's model has it: 40,000 bytes there at once are read as 16 KiB,
    /// then 10,000 and 6,384 bytes from the next 16 KiB, then the rest.
    #[test]
    fn a_read_gives_only_what_one_take_brought_in() {
        let (mut socket, mut other) = connected();
        let limits = limits();
        std::io::Write::write_all(&mut other, &[7; 40_000]).expect("sent");
        // Everything there before the first take.
        std::thread::sleep(Duration::from_millis(200));
        let mut buffer = vec![0_u8; 32_768];
        let mut counts = Vec::new();
        for room in [32_768, 10_000, 32_768, 32_768] {
            counts.push(socket.read(&mut buffer[..room], &limits, false).ok());
        }
        let wanted = [Some(16_384), Some(10_000), Some(6_384), Some(7_232)];
        assert_eq!(counts, wanted);
    }

    #[test]
    fn an_ended_connection_reads_nothing_and_writes_a_broken_pipe() {
        let mut ended = Socket::new(Link::Ended);
        let limits = limits();
        let mut buffer = [0_u8; 4];
        assert_eq!(ended.read(&mut buffer, &limits, false).ok(), Some(0));
        assert!(matches!(
            ended.write(b"x", &limits),
            Err(Fail::Errno(errno::PIPE))
        ));
        assert!(matches!(ended.shutdown(9), Err(Fail::Errno(errno::INVAL))));
    }

    /// A write over TLS that waits on its socket longer than a slice sends its bytes once: TLS
    /// takes them before the socket does, and a slice that runs out mustn't make the write give
    /// them again. The test's server waits before it reads, so the writes fill the sockets
    /// between them, and then counts what arrives.
    #[test]
    fn a_write_over_tls_that_waits_on_its_socket_sends_its_bytes_once() {
        use tokio::io::AsyncReadExt as _;

        let made = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
            .expect("a certificate");
        let certificate = made.cert.der().clone();
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(made.signing_key.serialize_der()),
        );
        let server = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_no_client_auth()
        .with_single_cert(vec![certificate.clone()], key)
        .expect("the server's certificate");
        let mut roots = rustls::RootCertStore::empty();
        roots.add(certificate).expect("a root");
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let port = listener.local_addr().expect("an address").port();
        let counted = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("the server's runtime");
            runtime.block_on(async move {
                listener
                    .set_nonblocking(true)
                    .expect("a listener that never waits");
                let listener =
                    tokio::net::TcpListener::from_std(listener).expect("a listener in the runtime");
                let (stream, _) = listener.accept().await.expect("the client");
                let mut stream = acceptor.accept(stream).await.expect("TLS");
                // Long past a slice, while the client's writes fill the sockets.
                tokio::time::sleep(Duration::from_millis(300)).await;
                let mut chunk = vec![0_u8; CHUNK];
                let mut total = 0;
                loop {
                    match stream.read(&mut chunk).await {
                        Ok(0) | Err(_) => break total,
                        Ok(count) => total += count,
                    }
                }
            })
        });
        let runtime = runtime().expect("the connections' runtime");
        let reach = crate::connect::Reach::Tls("localhost".to_owned(), port);
        let trust = Arc::new(crate::connect::client_config(roots));
        let connecting = crate::connect::make_with(&reach, trust);
        let place = runtime.block_on(connecting).expect("a TLS connection");
        let mut socket = Socket::new(Link::Place(place, runtime));
        let limits = limits();
        // More than the sockets between them hold.
        let bytes = vec![7_u8; 64 << 20];
        let mut sent = 0;
        while sent < bytes.len() {
            sent += socket.write(&bytes[sent..], &limits).expect("a write");
        }
        // Its end, and the connection gone, so the server's reading ends however the end went.
        socket.close();
        drop(socket);
        assert_eq!(counted.join().expect("the server"), bytes.len());
    }

    /// A write over TCP waits until the connection has taken every byte, as a blocking send
    /// does, so one write sends all it gives, on every system, as the prover's model has it: the
    /// other end waits before it reads, so the sockets between them fill first.
    #[test]
    fn a_write_takes_every_byte_while_the_other_end_waits_to_read() {
        let (mut socket, mut other) = connected();
        let counted = std::thread::spawn(move || {
            // Long past a slice, while the write fills the sockets.
            std::thread::sleep(Duration::from_millis(300));
            let mut chunk = vec![0_u8; CHUNK];
            let mut total = 0;
            loop {
                match std::io::Read::read(&mut other, &mut chunk) {
                    Ok(0) | Err(_) => break total,
                    Ok(count) => total += count,
                }
            }
        });
        let limits = limits();
        // More than the sockets between them hold.
        let bytes = vec![7_u8; 16 << 20];
        assert_eq!(socket.write(&bytes, &limits).ok(), Some(bytes.len()));
        // Its end, so the other end's reading ends.
        socket.close();
        drop(socket);
        assert_eq!(counted.join().expect("the other end"), bytes.len());
    }

    #[test]
    fn a_listener_on_port_zero_takes_a_free_port_and_hands_connections_on() {
        let mut listener = Listener::bind(0, false).expect("a listener");
        let port = listener.port().expect("its port");
        assert_ne!(port, 0);
        assert!(!listener.ready());
        let _client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("a client");
        let limits = limits();
        assert!(listener.accept(&limits).is_ok());
    }
}
