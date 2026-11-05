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
//!
//! A connection over TLS is the same to the program: lengua does TLS on it, in Rust, outside the
//! program, which reads and writes the plain bytes and holds no crypto. lengua checks the
//! computer's certificate against Mozilla's roots, built in, so every system trusts the same
//! certificates; one it can't check is a connection lengua couldn't make.

use std::io;
#[cfg(unix)]
use std::os::fd::AsFd;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[cfg(unix)]
use tokio::io::AsyncWriteExt as _;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::sync::oneshot;

/// The descriptor a program's first connection has: 3 is a server's listener, so each
/// connection's number depends only on its place in the spec.
pub const FIRST_DESCRIPTOR: i32 = 4;

/// The most connections a spec may name.
pub const MAX_CONNECTIONS: usize = 16;

/// How long lengua tries to make a connection before the program starts without it.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long lengua waits, once the program has ended, for what it sent last to reach the place.
#[cfg(unix)]
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// Where one connection goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// A port of this computer: its loopback, 127.0.0.1 and then ::1.
    ThisComputer(u16),
    /// A port of the computer with this name or address.
    Host(String, u16),
    /// A port of the computer with this name, over TLS: lengua checks its certificate for the
    /// name and does the TLS, so the program sends and reads plain bytes.
    Tls(String, u16),
    /// A port of the computer the program's argument `index`, from 1, names, over TLS when
    /// `tls`: [`resolved`] reads it when the program starts.
    Argument {
        /// Which argument, from 1.
        index: usize,
        /// The port.
        port: u16,
        /// Whether over TLS.
        tls: bool,
    },
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
            Self::Tls(host, port) => format!("{host}:{port} over TLS"),
            Self::Argument { index, port, tls } => format!(
                "port {port} of the computer argument {index} names{}",
                if *tls { ", over TLS" } else { "" }
            ),
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

/// lengua's end of a connection that goes to the test's own server, which speaks on it itself.
#[cfg(unix)]
pub type Peer = UnixStream;

/// Windows has no such stream yet (round 4 of the roadmap): this has no values, so the code that
/// serves a peer compiles there and never runs.
#[cfg(not(unix))]
pub enum Peer {}

#[cfg(not(unix))]
impl AsyncRead for Peer {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match *self {}
    }
}

#[cfg(not(unix))]
impl AsyncWrite for Peer {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match *self {}
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match *self {}
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        match *self {}
    }
}

/// One connection's two ends: the program's, which goes to the launcher, and lengua's.
#[cfg(unix)]
pub(crate) struct Pair {
    /// The program's end, blocking, as a program's descriptors are.
    pub program: std::os::unix::net::UnixStream,
    /// lengua's end.
    pub lengua: UnixStream,
}

/// A new connection's two ends.
#[cfg(unix)]
pub(crate) fn pair() -> io::Result<Pair> {
    let (program, lengua) = std::os::unix::net::UnixStream::pair()?;
    lengua.set_nonblocking(true)?;
    Ok(Pair {
        program,
        lengua: UnixStream::from_std(lengua)?,
    })
}

/// A stream lengua relays a program's connection to: TCP, or TLS over it.
pub(crate) trait Place: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> Place for T {}

/// How lengua checks a computer's certificate: against Mozilla's roots, the same on every
/// system, with ring's cryptography.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    Arc::clone(CONFIG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(client_config(roots))
    }))
}

/// A TLS client that trusts `roots`, with ring's cryptography and its safe protocol versions.
pub(crate) fn client_config(roots: rustls::RootCertStore) -> rustls::ClientConfig {
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap_or_else(|_| {
            rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap_or_else(|_| unreachable!("ring supports TLS 1.3"))
        })
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Makes the connection `reach` names, trying each of its addresses in turn and, over TLS,
/// checking the computer's certificate, until [`CONNECT_TIMEOUT`] has passed, or says why it
/// couldn't.
pub(crate) async fn make(reach: &Reach) -> Result<Box<dyn Place>, String> {
    make_with(reach, tls_config()).await
}

/// [`make`], with `tls` checking certificates.
pub(crate) async fn make_with(
    reach: &Reach,
    tls: Arc<rustls::ClientConfig>,
) -> Result<Box<dyn Place>, String> {
    match reach {
        Reach::Tls(host, port) => {
            let name = rustls::pki_types::ServerName::try_from(host.clone())
                .map_err(|_| format!("{host} isn't a name TLS can check a certificate for"))?;
            let attempt = async {
                let stream = connect_tcp(Some(host), *port).await?;
                tokio_rustls::TlsConnector::from(tls)
                    .connect(name, stream)
                    .await
                    .map_err(|error| format!("TLS with {host}: {error}"))
            };
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, attempt)
                .await
                .unwrap_or_else(|_| Err(no_answer()))?;
            Ok(Box::new(stream))
        }
        Reach::ThisComputer(port) => {
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, connect_tcp(None, *port))
                .await
                .unwrap_or_else(|_| Err(no_answer()))?;
            Ok(Box::new(stream))
        }
        Reach::Host(host, port) => {
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, connect_tcp(Some(host), *port))
                .await
                .unwrap_or_else(|_| Err(no_answer()))?;
            Ok(Box::new(stream))
        }
        Reach::Argument { index, .. } => Err(format!(
            "the computer argument {index} names wasn't read from the program's arguments"
        )),
        Reach::Peer | Reach::Nowhere => Err("there's nothing there".into()),
    }
}

/// Where `reach` goes in a run given `args`, the program's arguments: the computer an argument
/// names, read now, before the program starts, and any other place as it is. An argument that's
/// missing, or names this computer or no computer at all, is a connection lengua can't make, and
/// the error says why; the program then gets the connection already ended, as for a computer
/// that doesn't answer.
///
/// # Errors
///
/// Why lengua can't connect where the argument says.
pub fn resolved(reach: &Reach, args: &[String]) -> Result<Reach, String> {
    let Reach::Argument { index, port, tls } = reach else {
        return Ok(reach.clone());
    };
    let Some(given) = index.checked_sub(1).and_then(|at| args.get(at)) else {
        return Err(format!(
            "the program has no argument {index}, which names the computer its connection on \
             port {port} goes to"
        ));
    };
    let host = given.to_ascii_lowercase();
    match crate::spec::host_kind(&host) {
        crate::spec::HostKind::Other => Ok(if *tls {
            Reach::Tls(host, *port)
        } else {
            Reach::Host(host, *port)
        }),
        crate::spec::HostKind::ThisComputer => Err(format!(
            "argument {index}, `{}`, is this computer, which a connection to the computer an \
             argument names never goes to",
            given.escape_debug()
        )),
        crate::spec::HostKind::NotAName => Err(format!(
            "argument {index}, `{}`, isn't a computer's name or address",
            given.escape_debug()
        )),
    }
}

/// Why a connection that took too long wasn't made.
fn no_answer() -> String {
    format!("no answer within {} seconds", CONNECT_TIMEOUT.as_secs())
}

/// A TCP connection to `port` of `host`, or of this computer, trying each address in turn.
async fn connect_tcp(host: Option<&str>, port: u16) -> Result<TcpStream, String> {
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
}

/// Shuts `stream` both ways: the program's reads give 0 once what had arrived is read, and its
/// writes fail with `EPIPE`.
#[cfg(unix)]
pub(crate) fn shut(stream: &impl AsFd) {
    let _ = rustix::net::shutdown(stream, rustix::net::Shutdown::Both);
}

/// A connection lengua relays between the program's stream and its place.
#[cfg(unix)]
pub(crate) struct Relay {
    task: tokio::task::JoinHandle<()>,
    /// Says when everything the program sent has gone to the place, with its end of sending.
    sent: Option<oneshot::Receiver<()>>,
}

#[cfg(unix)]
impl Relay {
    /// Relays `program`, lengua's end of the program's stream, and `place` both ways until both
    /// have ended. Where either breaks off, the program's stream is shut both ways and held
    /// until the relay is dropped.
    pub(crate) fn start(program: UnixStream, place: Box<dyn Place>) -> Self {
        let (done, sent) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut from_program, mut to_program) = program.into_split();
            let (mut from_place, mut to_place) = tokio::io::split(place);
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

#[cfg(unix)]
impl Drop for Relay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer an argument names is read when the program starts: a name or another
    /// computer's address goes there, over TLS when the spec says so; an argument that's missing,
    /// names this computer, or names nothing is a connection lengua can't make.
    #[test]
    fn the_computer_an_argument_names_is_read_when_the_program_starts() {
        let typed = |index, tls| Reach::Argument {
            index,
            port: 443,
            tls,
        };
        let args = |given: &[&str]| {
            given
                .iter()
                .map(|arg| (*arg).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            resolved(&typed(1, true), &args(&["Example.COM"])),
            Ok(Reach::Tls("example.com".into(), 443))
        );
        assert_eq!(
            resolved(&typed(2, false), &args(&["-v", "192.0.2.7"])),
            Ok(Reach::Host("192.0.2.7".into(), 443))
        );
        assert_eq!(
            resolved(&Reach::ThisComputer(8080), &[]),
            Ok(Reach::ThisComputer(8080)),
            "every other place as it is"
        );
        let problem = |reach, given: &[&str]| {
            resolved(&reach, &args(given)).expect_err("a connection lengua can't make")
        };
        assert!(problem(typed(2, false), &["example.com"]).contains("no argument 2"));
        for this in ["localhost", "127.0.0.1", "::1", "0.0.0.0", "app.localhost"] {
            assert!(
                problem(typed(1, true), &[this]).contains("is this computer"),
                "{this}"
            );
        }
        for nothing in ["", "two words", "a_b.com", "-rf", "x..y"] {
            assert!(
                problem(typed(1, false), &[nothing]).contains("isn't a computer's name"),
                "{nothing:?}"
            );
        }
    }
}

#[cfg(all(test, unix))]
mod relay {
    use std::io::{Read as _, Write as _};

    use tokio::io::AsyncReadExt as _;

    use super::*;

    /// A TLS server for `localhost` on a free port of this computer, with a certificate of its
    /// own, and a client that trusts that certificate alone.
    fn server_and_trust() -> (tokio_rustls::TlsAcceptor, Arc<rustls::ClientConfig>) {
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
        (
            tokio_rustls::TlsAcceptor::from(Arc::new(server)),
            Arc::new(client_config(roots)),
        )
    }

    /// Listens on a free port of this computer and runs `serve` on the first TLS connection.
    async fn tls_server<F, Fut>(acceptor: tokio_rustls::TlsAcceptor, serve: F) -> u16
    where
        F: FnOnce(tokio_rustls::server::TlsStream<TcpStream>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port");
        let port = listener.local_addr().expect("an address").port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("a connection");
            if let Ok(stream) = acceptor.accept(stream).await {
                serve(stream).await;
            }
        });
        port
    }

    /// Relays `place` to a program's end of a stream, which the test then reads and writes as
    /// the program would, blocking.
    fn as_program(place: Box<dyn Place>) -> (std::os::unix::net::UnixStream, Relay) {
        let pair = pair().expect("a stream pair");
        let relay = Relay::start(pair.lengua, place);
        (pair.program, relay)
    }

    #[tokio::test]
    async fn the_program_speaks_plain_bytes_while_lengua_does_tls() {
        let (acceptor, trust) = server_and_trust();
        let port = tls_server(acceptor, |mut stream| async move {
            let mut asked = [0_u8; 4];
            stream.read_exact(&mut asked).await.expect("the request");
            assert_eq!(&asked, b"ping");
            stream.write_all(b"pong").await.expect("the answer");
            // close_notify, then the end: a clean close.
            stream.shutdown().await.expect("close");
        })
        .await;
        let place = make_with(&Reach::Tls("localhost".into(), port), trust)
            .await
            .expect("a TLS connection");
        let (program, _relay) = as_program(place);
        let mut program = program;
        tokio::task::spawn_blocking(move || {
            program.write_all(b"ping").expect("send");
            let mut answer = Vec::new();
            program.read_to_end(&mut answer).expect("the answer");
            assert_eq!(answer, b"pong", "and then the end");
        })
        .await
        .expect("the program");
    }

    #[tokio::test]
    async fn a_certificate_lengua_cant_check_is_a_connection_it_couldnt_make() {
        // Mozilla's roots don't vouch for a certificate of the test's own.
        let (acceptor, trust) = server_and_trust();
        let port = tls_server(acceptor.clone(), |_| async {}).await;
        let refused = make(&Reach::Tls("localhost".into(), port)).await;
        let Err(problem) = refused else {
            panic!("an untrusted certificate was accepted");
        };
        assert!(problem.contains("TLS with localhost"), "{problem}");
        // A name the certificate isn't for: refused too, however trusted the certificate is.
        let port = tls_server(acceptor, |_| async {}).await;
        let wrong = make_with(&Reach::Tls("127.0.0.1".into(), port), trust).await;
        assert!(
            wrong.is_err(),
            "a certificate for another name was accepted"
        );
    }

    #[tokio::test]
    async fn a_tls_stream_cut_off_ends_the_program_s_connection_both_ways() {
        let (acceptor, trust) = server_and_trust();
        let port = tls_server(acceptor, |stream| async move {
            // The end with no close_notify, which a TLS client takes as a cut, not an end.
            let (tcp, _) = stream.into_inner();
            drop(tcp);
        })
        .await;
        let place = make_with(&Reach::Tls("localhost".into(), port), trust)
            .await
            .expect("a TLS connection");
        let (program, _relay) = as_program(place);
        let mut program = program;
        tokio::task::spawn_blocking(move || {
            let mut answer = Vec::new();
            program
                .read_to_end(&mut answer)
                .expect("the end, not a reset");
            assert!(answer.is_empty(), "{answer:?}");
            // Shut both ways: a write fails with EPIPE rather than a signal.
            let wrote = program.write(b"more");
            assert!(
                wrote
                    .as_ref()
                    .is_err_and(|error| error.raw_os_error() == Some(32)),
                "{wrote:?}"
            );
        })
        .await
        .expect("the program");
    }
}

#[cfg(all(test, unix))]
mod internet {
    use std::io::{Read as _, Write as _};

    use super::*;

    /// A real HTTPS host, checked against Mozilla's roots: run by hand with `--ignored`, since a
    /// test of lengua's own never depends on the internet.
    #[tokio::test]
    #[ignore = "reaches example.com over the internet"]
    async fn reaches_a_real_https_host() {
        let place = make(&Reach::Tls("example.com".into(), 443))
            .await
            .expect("example.com over TLS");
        let pair = pair().expect("a stream pair");
        let _relay = Relay::start(pair.lengua, place);
        let mut program = pair.program;
        let answer = tokio::task::spawn_blocking(move || {
            program
                .write_all(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
                .expect("send");
            let mut answer = Vec::new();
            let _ = program.read_to_end(&mut answer);
            answer
        })
        .await
        .expect("the program");
        let text = String::from_utf8_lossy(&answer);
        assert!(text.starts_with("HTTP/1."), "{text}");
        assert!(text.contains("Example Domain"), "{text}");
    }
}
