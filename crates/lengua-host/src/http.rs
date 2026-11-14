use std::convert::Infallible;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use hyper::body::{Body, Bytes, Frame};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi_http::WasiHttpView;
use wasmtime_wasi_http::p2::bindings::http::types::{ErrorCode, Scheme};
use wasmtime_wasi_http::p2::bindings::sync::Proxy;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;

use crate::{Ending, Host, Input, OUTPUT, Outcome, Prepared, Program, Streams, trap_ending};

const HEAD_LIMIT: usize = 64 << 10;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(5);

struct Whole(Option<Bytes>);

impl Body for Whole {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Ready(self.0.take().map(|bytes| Ok(Frame::data(bytes))))
    }
}

struct Unpark(thread::Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn head_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &str) -> usize {
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

fn chunked(head: &str) -> bool {
    head.lines()
        .filter_map(|l| l.split_once(':'))
        .any(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
        })
}

fn dechunk(mut rest: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("a chunk has no size line")?;
        let line = String::from_utf8_lossy(rest.get(..end).unwrap_or_default()).to_string();
        let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| format!("`{line}` is not a chunk size"))?;
        let data = rest
            .get(end + 2..end + 2 + size)
            .ok_or("a chunk is short")?;
        if size == 0 {
            return Ok(out);
        }
        out.extend_from_slice(data);
        rest = rest
            .get(end + 4 + size..)
            .ok_or("a chunk has no line end")?;
    }
}

fn parse(bytes: &[u8]) -> Result<hyper::Request<Whole>, String> {
    let end = head_end(bytes).ok_or("the request has no blank line after its head")?;
    let head = std::str::from_utf8(bytes.get(..end).unwrap_or_default())
        .map_err(|_| "the head of the request is not UTF-8".to_string())?;
    let rest = bytes.get(end + 4..).unwrap_or_default();
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or_default();
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(format!("`{first}` is not a request line"));
    };
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(format!("`{version}` is not HTTP/1.1"));
    }
    let mut builder = hyper::Request::builder().method(method).uri(target);
    let mut host = false;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("`{line}` is not a header"))?;
        host |= name.trim().eq_ignore_ascii_case("host");
        builder = builder.header(name.trim(), value.trim());
    }
    if !host {
        builder = builder.header("host", "localhost");
    }
    let body = if chunked(head) {
        dechunk(rest)?
    } else {
        rest.get(..content_length(head).min(rest.len()))
            .unwrap_or_default()
            .to_vec()
    };
    builder
        .body(Whole(Some(Bytes::from(body))))
        .map_err(|e| format!("the request is not one lengua can read: {e}"))
}

#[must_use]
pub fn response_bytes(status: u16, headers: &[(String, Vec<u8>)], body: &[u8]) -> Vec<u8> {
    let reason = hyper::StatusCode::from_u16(status)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("");
    let mut out = format!("HTTP/1.1 {status} {reason}\r\n").into_bytes();
    for (name, value) in headers {
        let skip = ["content-length", "transfer-encoding", "connection"];
        if skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(
        format!(
            "content-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    out.extend_from_slice(body);
    out
}

fn collect(mut body: HyperOutgoingBody, deadline: Instant) -> Result<Vec<u8>, String> {
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut out = Vec::new();
    loop {
        match Pin::new(&mut body).poll_frame(&mut cx) {
            Poll::Ready(None) => return Ok(out),
            Poll::Ready(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    out.extend_from_slice(&data);
                }
                if out.len() > OUTPUT {
                    return Err("the body of the answer is larger than 16 MiB".to_string());
                }
            }
            Poll::Ready(Some(Err(error))) => return Err(format!("{error:?}")),
            Poll::Pending => {
                if Instant::now() >= deadline {
                    return Err("the body of the answer did not end in time".to_string());
                }
                thread::park_timeout(Duration::from_millis(50));
            }
        }
    }
}

type Answer = Result<hyper::Response<HyperOutgoingBody>, ErrorCode>;

fn answer_bytes(response: Option<Answer>, deadline: Instant) -> Vec<u8> {
    match response {
        Some(Ok(response)) => {
            let (parts, body) = response.into_parts();
            let headers: Vec<(String, Vec<u8>)> = parts
                .headers
                .iter()
                .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
                .collect();
            match collect(body, deadline) {
                Ok(bytes) => response_bytes(parts.status.as_u16(), &headers, &bytes),
                Err(message) => response_bytes(500, &[], message.as_bytes()),
            }
        }
        Some(Err(code)) => response_bytes(500, &[], format!("{code:?}").as_bytes()),
        None => response_bytes(500, &[], b"the server ended with no answer to the request"),
    }
}

impl Host {
    pub(crate) fn answer(&self, run: &Program, input: &Input) -> Outcome {
        let stdout = MemoryOutputPipe::new(OUTPUT);
        let stderr = MemoryOutputPipe::new(OUTPUT);
        let request = match parse(&input.stdin) {
            Ok(request) => request,
            Err(message) => {
                return Outcome {
                    ending: Ending::Exited(0),
                    stdout: response_bytes(400, &[], message.as_bytes()),
                    stderr: Vec::new(),
                };
            }
        };
        let (ending, bytes) = match self.prepare(run, input, Streams::Memory, (&stdout, &stderr)) {
            Ok(prepared) => Self::handle(run, prepared, request),
            Err(ending) => (ending, Vec::new()),
        };
        Outcome {
            ending,
            stdout: bytes,
            stderr: stderr.contents().to_vec(),
        }
    }

    fn handle(
        run: &Program,
        prepared: Prepared,
        request: hyper::Request<Whole>,
    ) -> (Ending, Vec<u8>) {
        let Prepared { mut store, linker } = prepared;
        let proxy = match Proxy::instantiate(&mut store, run.component, &linker) {
            Ok(proxy) => proxy,
            Err(error) => {
                return (
                    Ending::Refused(format!(
                        "the host refused the server, since it links only the world of the idea: {error:#}"
                    )),
                    Vec::new(),
                );
            }
        };
        let (send, receive) = mpsc::channel::<Answer>();
        let req = match store
            .data_mut()
            .http()
            .new_incoming_request(Scheme::Http, request)
        {
            Ok(req) => req,
            Err(error) => return (Ending::Trapped(format!("{error:#}")), Vec::new()),
        };
        let out = match store
            .data_mut()
            .http()
            .new_response_outparam_from_callback(move |answer| {
                let _ = send.send(answer);
            }) {
            Ok(out) => out,
            Err(error) => return (Ending::Trapped(format!("{error:#}")), Vec::new()),
        };
        let limit = Duration::from_millis(run.build.grants.time_ms);
        let deadline = Instant::now() + limit + Duration::from_secs(2);
        thread::scope(|scope| {
            let guest = scope.spawn(|| {
                let result = proxy
                    .wasi_http_incoming_handler()
                    .call_handle(&mut store, req, out);
                (result, store.data().limits.hit)
            });
            let mut response = None;
            loop {
                match receive.recv_timeout(POLL) {
                    Ok(found) => {
                        response = Some(found);
                        break;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {
                        if guest.is_finished() || Instant::now() >= deadline {
                            response = receive.try_recv().ok();
                            break;
                        }
                    }
                }
            }
            let bytes = answer_bytes(response, deadline);
            let ending = match guest.join() {
                Ok((Ok(()), _)) => Ending::Exited(0),
                Ok((Err(error), hit)) => trap_ending(&error, hit),
                Err(_) => Ending::Trapped("the server's thread ended with a panic".to_string()),
            };
            (ending, bytes)
        })
    }

    pub fn serve(&self, run: &Program, address: &str, fixed: bool) -> Result<(), String> {
        let listener = TcpListener::bind(address)
            .map_err(|e| format!("lengua could not listen on `{address}`: {e}"))?;
        let gate = RwLock::new(());
        let writes = run.build.grants.folders.iter().any(|f| f.write);
        let (send, receive) = mpsc::sync_channel::<TcpStream>(WORKERS * 4);
        let receive = Mutex::new(receive);
        thread::scope(|scope| {
            for _ in 0..WORKERS {
                scope.spawn(|| {
                    loop {
                        let next = receive
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .recv();
                        let Ok(mut stream) = next else {
                            break;
                        };
                        self.serve_one(run, &mut stream, fixed, (&gate, writes));
                    }
                });
            }
            for stream in listener.incoming().flatten() {
                if send.send(stream).is_err() {
                    break;
                }
            }
            drop(send);
        });
        Ok(())
    }

    fn serve_one(
        &self,
        run: &Program,
        stream: &mut TcpStream,
        fixed: bool,
        gate: (&RwLock<()>, bool),
    ) {
        let started = Instant::now();
        let request = read_request(stream);
        let first = request_line(&request);
        let method = first.split(' ').next().unwrap_or_default().to_string();
        let input = Input {
            stdin: request,
            fixed,
            ..Input::default()
        };
        let reads = !gate.1 || matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS");
        let outcome = if reads {
            let _shared = gate.0.read().unwrap_or_else(PoisonError::into_inner);
            self.answer(run, &input)
        } else {
            let _alone = gate.0.write().unwrap_or_else(PoisonError::into_inner);
            self.answer(run, &input)
        };
        let mut bytes = outcome.stdout;
        let problem = match &outcome.ending {
            Ending::Exited(_) => String::new(),
            Ending::OutOfTime(m) | Ending::Trapped(m) | Ending::Refused(m) => m.clone(),
            Ending::OutOfMemory => "the server ran out of memory".to_string(),
        };
        if bytes.is_empty() {
            bytes = response_bytes(500, &[], problem.as_bytes());
        }
        let _ = stream.write_all(&bytes);
        let _ = stream.flush();
        log_line(&first, &bytes, started, &problem, &outcome.stderr);
    }
}

const WORKERS: usize = 8;

fn request_line(request: &[u8]) -> String {
    let end = request
        .windows(2)
        .position(|w| w == b"\r\n")
        .unwrap_or(request.len().min(200));
    String::from_utf8_lossy(request.get(..end).unwrap_or_default()).to_string()
}

fn log_line(first: &str, answer: &[u8], started: Instant, problem: &str, stderr: &[u8]) {
    let status = String::from_utf8_lossy(answer.get(9..12).unwrap_or_default()).to_string();
    let target: Vec<&str> = first.split(' ').take(2).collect();
    let mut line = format!(
        "lengua-run: {} {} {}ms",
        target.join(" "),
        status,
        started.elapsed().as_millis()
    );
    if !problem.is_empty() {
        line.push_str(": ");
        line.push_str(problem);
    }
    let mut out = std::io::stderr().lock();
    let _ = writeln!(out, "{line}");
    for program_line in String::from_utf8_lossy(stderr).lines() {
        let _ = writeln!(out, "lengua-run:   {program_line}");
    }
}

pub fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut wanted: Option<usize> = None;
    loop {
        if let Some(total) = wanted
            && bytes.len() >= total
        {
            break;
        }
        let Ok(count) = stream.read(&mut buffer) else {
            break;
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(buffer.get(..count).unwrap_or_default());
        if wanted.is_none()
            && let Some(end) = head_end(&bytes)
        {
            let head = String::from_utf8_lossy(bytes.get(..end).unwrap_or_default()).to_string();
            if chunked(&head) {
                if bytes.ends_with(b"0\r\n\r\n") {
                    break;
                }
            } else {
                wanted = Some(end + 4 + content_length(&head));
            }
        }
        if bytes.len() > HEAD_LIMIT + OUTPUT {
            break;
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{dechunk, parse, response_bytes};

    #[test]
    fn a_request_is_read_with_its_headers_and_body() {
        let request =
            parse(b"POST /items?a=1 HTTP/1.1\r\nHost: shop\r\nContent-Length: 3\r\n\r\nteaextra")
                .expect("a request");
        assert_eq!(request.method(), "POST");
        assert_eq!(request.uri().path(), "/items");
        assert_eq!(request.headers()["host"], "shop");
        assert!(parse(b"GET /\r\n\r\n").is_err());
        assert!(parse(b"GET / HTTP/1.1\r\n").is_err());
        let chunked = dechunk(b"3\r\ntea\r\n2\r\n!!\r\n0\r\n\r\n").expect("a body");
        assert_eq!(chunked, b"tea!!");
        assert!(dechunk(b"z\r\n").is_err());
    }

    #[test]
    fn an_answer_has_its_length_and_closes() {
        let bytes = response_bytes(
            200,
            &[
                ("content-type".to_string(), b"text/plain".to_vec()),
                ("content-length".to_string(), b"99".to_vec()),
            ],
            b"hi",
        );
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\nconnection: close\r\n\r\nhi"
        );
    }
}
