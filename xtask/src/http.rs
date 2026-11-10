//! Downloads over HTTPS, with the TLS lengua's own server uses: rustls with ring's cryptography,
//! each server's certificate checked against Mozilla's roots (webpki-roots), the same on every
//! runner, and with the same crates' features, so that a build of the whole workspace compiles
//! lengua-mcp as it ships. A download follows redirects, as GitHub's release downloads need, and
//! one that fails on the way is tried again, as `curl --retry 3` does.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// How many times a download is tried.
const TRIES: u64 = 4;

/// The most redirects one download follows.
const REDIRECTS: usize = 10;

/// How long a connection may stay silent.
const SILENCE: Duration = Duration::from_secs(120);

/// Why a request failed.
enum Failure {
    /// The network or the server failed on the way: another try may get the answer.
    Again(String),
    /// The answer is no, and another try would get it again.
    Final(String),
}

/// An HTTP answer.
struct Answer {
    /// Its status code.
    status: u16,
    /// Its header fields, each name in lowercase.
    headers: Vec<(String, String)>,
    /// Its body.
    body: Vec<u8>,
}

/// The bytes at `url`, an `https://` address.
pub fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut why = String::new();
    for attempt in 1..=TRIES {
        match follow(url) {
            Ok(body) => return Ok(body),
            Err(Failure::Final(failed)) => return Err(failed),
            Err(Failure::Again(failed)) => why = failed,
        }
        if attempt < TRIES {
            std::thread::sleep(Duration::from_secs(attempt));
        }
    }
    Err(format!("{why} ({TRIES} tries)"))
}

/// Asks for `url`, following its redirects to the answer.
fn follow(url: &str) -> Result<Vec<u8>, Failure> {
    let mut at = url.to_owned();
    for _ in 0..=REDIRECTS {
        let answer = request(&at)?;
        match answer.status {
            200 => return Ok(answer.body),
            301 | 302 | 303 | 307 | 308 => {
                let location = field(&answer.headers, "location")
                    .ok_or_else(|| Failure::Final(format!("{at}: a redirect to nowhere")))?;
                at = resolved(&at, location).map_err(Failure::Final)?;
            }
            408 | 429 | 500..=599 => {
                return Err(Failure::Again(format!("{at}: HTTP {}", answer.status)));
            }
            status => return Err(Failure::Final(format!("{at}: HTTP {status}"))),
        }
    }
    Err(Failure::Final(format!("{url}: more than {REDIRECTS} redirects")))
}

/// The first header field of `headers` named `name`, in lowercase.
fn field<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
}

/// `location`, where a redirect from `from` sends, as a whole `https://` address.
fn resolved(from: &str, location: &str) -> Result<String, String> {
    if location.starts_with("https://") {
        return Ok(location.to_owned());
    }
    if location.starts_with('/') {
        let (authority, _) = parts(from)?;
        return Ok(format!("https://{authority}{location}"));
    }
    Err(format!(
        "{from}: a redirect to {location}, which isn't an https:// address"
    ))
}

/// The host (with its port, when it names one) and the path of an `https://` address.
fn parts(url: &str) -> Result<(&str, &str), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("{url} isn't an https:// address"))?;
    let rest = rest.split_once('#').map_or(rest, |(before, _)| before);
    Ok(rest.find('/').map_or((rest, "/"), |slash| rest.split_at(slash)))
}

/// How every download checks its server's certificate: against Mozilla's roots, with ring's
/// cryptography and rustls's safe protocol versions, as lengua's own connections do.
fn config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    Arc::clone(CONFIG.get_or_init(|| {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Arc::new(client_config(roots))
    }))
}

/// A TLS client that trusts `roots`, with ring's cryptography and its safe protocol versions:
/// src/connect.rs's.
fn client_config(roots: rustls::RootCertStore) -> rustls::ClientConfig {
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

/// One request for `url`, over a connection of its own, and its whole answer.
fn request(url: &str) -> Result<Answer, Failure> {
    let (authority, path) = parts(url).map_err(Failure::Final)?;
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => {
            let port = port
                .parse::<u16>()
                .map_err(|_| Failure::Final(format!("{url}: no port {port}")))?;
            (host, port)
        }
        None => (authority, 443),
    };
    let name = rustls::pki_types::ServerName::try_from(host.to_owned())
        .map_err(|_| Failure::Final(format!("{host}: no name TLS can check a certificate of")))?;
    let connection = rustls::ClientConnection::new(config(), name)
        .map_err(|error| Failure::Final(format!("TLS with {host}: {error}")))?;
    let socket = TcpStream::connect((host, port))
        .map_err(|error| Failure::Again(format!("{url}: {error}")))?;
    let timed = socket
        .set_read_timeout(Some(SILENCE))
        .and_then(|()| socket.set_write_timeout(Some(SILENCE)));
    timed.map_err(|error| Failure::Again(format!("{url}: {error}")))?;
    let mut stream = rustls::StreamOwned::new(connection, socket);
    let asked = format!(
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: lengua-xtask\r\nAccept: */*\r\n\
         Connection: close\r\n\r\n"
    );
    let sent = stream.write_all(asked.as_bytes());
    sent.and_then(|()| stream.flush())
        .map_err(|error| Failure::Again(format!("{url}: {error}")))?;
    let mut received = Vec::new();
    // A server may close the connection without TLS's close_notify: the answer's own length says
    // whether all of it came.
    if let Err(error) = stream.read_to_end(&mut received)
        && error.kind() != std::io::ErrorKind::UnexpectedEof
    {
        return Err(Failure::Again(format!("{url}: {error}")));
    }
    parse(&received).map_err(|why| Failure::Again(format!("{url}: {why}")))
}

/// The answer in `received`: its status, its header fields and its body, all of it or an error.
fn parse(received: &[u8]) -> Result<Answer, String> {
    let end = received
        .windows(4)
        .position(|four| four == b"\r\n\r\n")
        .ok_or("an answer cut short in its header")?;
    let head = String::from_utf8_lossy(&received[..end]);
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or("an answer with no status")?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let rest = &received[end + 4..];
    let chunked = field(&headers, "transfer-encoding")
        .is_some_and(|coding| coding.to_ascii_lowercase().contains("chunked"));
    let body = if chunked {
        dechunked(rest)?
    } else if let Some(length) = field(&headers, "content-length") {
        let length: usize = length.parse().map_err(|_| format!("a length of {length:?}"))?;
        let whole = rest
            .get(..length)
            .ok_or_else(|| format!("an answer cut short: {} of {length} bytes", rest.len()))?;
        whole.to_vec()
    } else {
        rest.to_vec()
    };
    Ok(Answer {
        status,
        headers,
        body,
    })
}

/// A chunked body (HTTP/1.1's `Transfer-Encoding: chunked`), made whole.
fn dechunked(mut rest: &[u8]) -> Result<Vec<u8>, String> {
    let short = || "a chunked answer cut short".to_owned();
    let mut body = Vec::new();
    loop {
        let end = rest
            .windows(2)
            .position(|two| two == b"\r\n")
            .ok_or_else(short)?;
        let line = String::from_utf8_lossy(&rest[..end]);
        let size = line.split(';').next().unwrap_or_default().trim();
        let size =
            usize::from_str_radix(size, 16).map_err(|_| format!("a chunk's size of {size:?}"))?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Ok(body);
        }
        body.extend_from_slice(rest.get(..size).ok_or_else(short)?);
        rest = rest.get(size + 2..).ok_or_else(short)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An address's host and path, and a redirect's place made whole.
    #[test]
    fn addresses_split_and_redirects_resolve() {
        assert_eq!(
            parts("https://github.com/a/b?x=1#top"),
            Ok(("github.com", "/a/b?x=1"))
        );
        assert_eq!(parts("https://example.com"), Ok(("example.com", "/")));
        assert!(parts("http://example.com/").is_err());
        assert_eq!(
            resolved("https://github.com/a", "/b/c"),
            Ok("https://github.com/b/c".to_owned())
        );
        assert_eq!(
            resolved("https://github.com/a", "https://objects.example/x"),
            Ok("https://objects.example/x".to_owned())
        );
        assert!(resolved("https://github.com/a", "http://example.com/").is_err());
    }

    /// An answer's body is as long as its length says, or made whole from its chunks; one cut
    /// short is an error.
    #[test]
    fn an_answer_s_body_is_whole_or_an_error() {
        let sized = parse(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello").expect("whole");
        assert_eq!((sized.status, sized.body.as_slice()), (200, &b"hello"[..]));
        assert!(parse(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nhello").is_err());
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
                        4\r\nwiki\r\n6;x=y\r\npedia \r\n0\r\n\r\n";
        assert_eq!(parse(chunked).expect("whole").body, b"wikipedia ");
        let cut = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nwi";
        assert!(parse(cut).is_err());
        let moved = parse(b"HTTP/1.1 302 Found\r\nLocation: https://x.example/y\r\n\r\n")
            .expect("a redirect");
        assert_eq!(moved.status, 302);
        assert_eq!(
            field(&moved.headers, "location"),
            Some("https://x.example/y")
        );
        assert!(parse(b"HTTP/1.1 200 OK\r\n").is_err());
    }
}
