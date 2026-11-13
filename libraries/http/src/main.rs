use std::fmt::Write as _;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Incomplete,
    Line,
    Header(usize),
    Version,
    Status,
    Length,
    Chunk(usize),
}

fn line_end(bytes: &[u8], from: usize) -> Option<usize> {
    let found = bytes
        .get(from..)?
        .windows(2)
        .position(|pair| pair == b"\r\n")
        .map(|at| at + from);
    debug_assert!(found.is_none_or(|at| at >= from));
    debug_assert!(found.is_none_or(|at| at + 2 <= bytes.len()));
    found
}

fn split_head(bytes: &[u8]) -> Result<(Vec<String>, &[u8]), Error> {
    let end = bytes
        .windows(4)
        .position(|four| four == b"\r\n\r\n")
        .ok_or(Error::Incomplete)?;
    let head =
        std::str::from_utf8(bytes.get(..end).unwrap_or_default()).map_err(|_| Error::Line)?;
    let rest = bytes.get(end + 4..).unwrap_or_default();
    let lines: Vec<String> = head.split("\r\n").map(str::to_string).collect();
    debug_assert_ne!(lines.len(), 0);
    debug_assert_eq!(rest.len() + end + 4, bytes.len());
    Ok((lines, rest))
}

fn headers_of(lines: &[String]) -> Result<Vec<(String, String)>, Error> {
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate().skip(1) {
        let (name, value) = line.split_once(':').ok_or(Error::Header(at))?;
        if name.is_empty() || name.contains(' ') {
            return Err(Error::Header(at));
        }
        out.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }
    debug_assert!(out.len() < lines.len());
    debug_assert!(out.iter().all(|(name, _)| !name.contains(':')));
    Ok(out)
}

#[must_use]
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let found = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str());
    debug_assert!(found.is_none() || !headers.is_empty());
    debug_assert!(!name.contains(':'));
    found
}

fn body_of(headers: &[(String, String)], rest: &[u8], to_end: bool) -> Result<Vec<u8>, Error> {
    let chunked = header(headers, "transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"));
    debug_assert!(rest.len() < usize::MAX);
    if chunked {
        return decode_chunked(rest).map(|(body, _)| body);
    }
    let body = match header(headers, "content-length") {
        Some(text) => {
            let length: usize = text.parse().map_err(|_| Error::Length)?;
            rest.get(..length).ok_or(Error::Incomplete)?.to_vec()
        }
        None if to_end => rest.to_vec(),
        None => Vec::new(),
    };
    debug_assert!(body.len() <= rest.len());
    Ok(body)
}

pub fn parse_request(bytes: &[u8]) -> Result<Request, Error> {
    let (lines, rest) = split_head(bytes)?;
    let first: Vec<&str> = lines
        .first()
        .map_or("", String::as_str)
        .split(' ')
        .collect();
    let [method, target, version] = first.as_slice() else {
        return Err(Error::Line);
    };
    if *version != "HTTP/1.1" && *version != "HTTP/1.0" {
        return Err(Error::Version);
    }
    if method.is_empty() || !target.starts_with('/') {
        return Err(Error::Line);
    }
    let headers = headers_of(&lines)?;
    let body = body_of(&headers, rest, false)?;
    debug_assert_ne!(method.len(), 0);
    debug_assert!(target.starts_with('/'));
    Ok(Request {
        method: (*method).to_string(),
        target: (*target).to_string(),
        headers,
        body,
    })
}

pub fn parse_response(bytes: &[u8]) -> Result<Response, Error> {
    let (lines, rest) = split_head(bytes)?;
    let first = lines.first().map_or("", String::as_str);
    let mut parts = first.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(Error::Version);
    }
    let status: u16 = parts
        .next()
        .and_then(|code| code.parse().ok())
        .filter(|code| (100..=599).contains(code))
        .ok_or(Error::Status)?;
    let reason = parts.next().unwrap_or_default().to_string();
    let headers = headers_of(&lines)?;
    let body = body_of(&headers, rest, true)?;
    debug_assert!(status >= 100);
    debug_assert!(status <= 599);
    Ok(Response {
        status,
        reason,
        headers,
        body,
    })
}

fn head_and_body(first: &str, headers: &[(String, String)], body: &[u8]) -> Vec<u8> {
    let mut text = String::new();
    let _ = write!(text, "{first}\r\n");
    for (name, value) in headers {
        let _ = write!(text, "{name}: {value}\r\n");
    }
    let sized = header(headers, "content-length").is_some()
        || header(headers, "transfer-encoding").is_some();
    if !body.is_empty() && !sized {
        let _ = write!(text, "content-length: {}\r\n", body.len());
    }
    text.push_str("\r\n");
    let mut out = text.into_bytes();
    out.extend_from_slice(body);
    debug_assert!(out.len() >= body.len() + 4);
    debug_assert!(out.ends_with(body));
    out
}

#[must_use]
pub fn write_request(request: &Request) -> Vec<u8> {
    let first = format!("{} {} HTTP/1.1", request.method, request.target);
    let out = head_and_body(&first, &request.headers, &request.body);
    debug_assert!(out.starts_with(request.method.as_bytes()));
    debug_assert!(out.len() > first.len());
    out
}

#[must_use]
pub fn write_response(response: &Response) -> Vec<u8> {
    let first = format!("HTTP/1.1 {} {}", response.status, response.reason);
    let out = head_and_body(&first, &response.headers, &response.body);
    debug_assert!(out.starts_with(b"HTTP/1.1 "));
    debug_assert!(out.len() > first.len());
    out
}

pub fn decode_chunked(bytes: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    let mut out = Vec::new();
    let mut at = 0;
    for _ in 0..=bytes.len() {
        let end = line_end(bytes, at).ok_or(Error::Incomplete)?;
        let line = std::str::from_utf8(bytes.get(at..end).unwrap_or_default())
            .map_err(|_| Error::Chunk(at))?;
        let size_text = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| Error::Chunk(at))?;
        let start = end + 2;
        if size == 0 {
            return match bytes.get(start..start + 2) {
                Some(tail) if tail == b"\r\n" => Ok((out, start + 2)),
                Some(_) => Err(Error::Chunk(start)),
                None => Err(Error::Incomplete),
            };
        }
        let data = bytes.get(start..start + size).ok_or(Error::Incomplete)?;
        let closed = bytes
            .get(start + size..start + size + 2)
            .is_some_and(|tail| tail == b"\r\n");
        if !closed {
            return Err(Error::Chunk(start + size));
        }
        out.extend_from_slice(data);
        at = start + size + 2;
    }
    debug_assert!(at <= bytes.len());
    debug_assert!(out.len() <= bytes.len());
    Err(Error::Incomplete)
}

#[must_use]
pub fn encode_chunked(body: &[u8], size: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in body.chunks(size.max(1)) {
        out.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"0\r\n\r\n");
    debug_assert!(out.len() >= body.len() + 5);
    debug_assert!(out.ends_with(b"0\r\n\r\n"));
    out
}
