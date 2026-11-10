//! The connections lengua makes for a program. A program never connects by itself: for each
//! place its idea names, lengua's host makes the connection before the program starts and hands
//! it over, so the program holds no network of its own, on every system, and a connection
//! behaves the same everywhere. A connection lengua couldn't make is handed over already ended,
//! and the run's report says why: the program can't tell a place that refused it from one that
//! broke off.
//!
//! A connection over TLS is the same to the program: lengua does TLS on it, in Rust, outside the
//! program, which reads and writes the plain bytes and holds no crypto. lengua checks the
//! computer's certificate against Mozilla's roots, built in, so every system trusts the same
//! certificates; one it can't check is a connection lengua couldn't make.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

/// The descriptor the prover's world gives a program's first connection, as Linux would number
/// them: 3 is a server's listener, so each connection's number depends only on its place in the
/// idea. lengua's host gives the program its own numbers for them, by role.
pub const FIRST_DESCRIPTOR: i32 = 4;

/// The most connections an idea may name.
pub const MAX_CONNECTIONS: usize = 16;

/// How long lengua tries to make a connection before the program starts without it.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// The test's own server, which speaks on the program's connection itself.
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

/// A stream lengua made for a program's connection: TCP, or TLS over it.
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
    match crate::idea::host_kind(&host) {
        crate::idea::HostKind::Other => Ok(if *tls {
            Reach::Tls(host, *port)
        } else {
            Reach::Host(host, *port)
        }),
        crate::idea::HostKind::ThisComputer => Err(format!(
            "argument {index}, `{}`, is this computer, which a connection to the computer an \
             argument names never goes to",
            given.escape_debug()
        )),
        crate::idea::HostKind::NotAName => Err(format!(
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer an argument names is read when the program starts: a name or another
    /// computer's address goes there, over TLS when the idea says so; an argument that's missing,
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

    /// An argument that names this computer by another spelling of its address, or by a number
    /// only a system's older parsing reads as an address, is a connection lengua can't make.
    #[test]
    fn an_argument_never_names_this_computer_by_another_spelling() {
        let typed = Reach::Argument {
            index: 1,
            port: 6379,
            tls: false,
        };
        for given in ["::ffff:127.0.0.1", "2130706433", "127.1", "0x7f.1"] {
            let refused = resolved(&typed, &[given.to_owned()]);
            assert!(refused.is_err(), "{given}: {refused:?}");
        }
    }

    /// A named computer whose address turns out to be this computer's loopback is a connection
    /// lengua couldn't make, whatever the name: the address a name resolves to is looked at, not
    /// only how the name is spelled. Something listens there, so only the refusal keeps the
    /// connection from being made.
    #[tokio::test]
    async fn a_named_computer_that_resolves_to_this_one_is_not_reached() {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("a free port");
        let port = listener.local_addr().expect("its address").port();
        for name in ["::ffff:127.0.0.1", "127.0.0.1"] {
            let made = make(&Reach::Host(name.to_owned(), port)).await;
            let Err(problem) = made else {
                panic!("{name}: a connection to this computer's loopback was made");
            };
            assert!(problem.contains("this computer"), "{name}: {problem}");
        }
    }
}

#[cfg(all(test, unix))]
mod tls {
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
}

#[cfg(test)]
mod internet {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    /// A real HTTPS host, checked against Mozilla's roots: run by hand with `--ignored`, since a
    /// test of lengua's own never depends on the internet.
    #[tokio::test]
    #[ignore = "reaches example.com over the internet"]
    async fn reaches_a_real_https_host() {
        let mut place = make(&Reach::Tls("example.com".into(), 443))
            .await
            .expect("example.com over TLS");
        place
            .write_all(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
            .await
            .expect("send");
        let mut answer = Vec::new();
        let _ = place.read_to_end(&mut answer).await;
        let text = String::from_utf8_lossy(&answer);
        assert!(text.starts_with("HTTP/1."), "{text}");
        assert!(text.contains("Example Domain"), "{text}");
    }
}
