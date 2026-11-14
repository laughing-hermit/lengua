use std::io::{Read, Write};

use wasip2::http::outgoing_handler;
use wasip2::http::types::{Fields, Method, OutgoingBody, OutgoingRequest, Scheme};
use wasip2::io::streams::StreamError;

const MODEL: &str = "ai.lengua";
const SERVICE: &str = "localhost:7813";
const MOST_PIECES: usize = 4096;
const READ: u64 = 65_536;

fn main() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let (mode, text) = input.split_once('\n').unwrap_or((input.as_str(), ""));
    println!("{}", answer(mode.trim(), text.trim_end()));
}

fn answer(mode: &str, text: &str) -> String {
    debug_assert!(mode.len() < 1 << 16);
    debug_assert!(text.len() < 1 << 20);
    let prompt = ai::fill(
        "Give a short title for this note: {text}",
        &[("text", text)],
    );
    match mode {
        "ask" => reply(send(MODEL, "/ask", Some(&prompt), &[]), |pieces| {
            pieces.concat()
        }),
        "stream" => reply(send(MODEL, "/stream", Some(&prompt), &[]), streamed),
        "json" => reply(
            send(MODEL, "/ask", Some(&prompt), &[("x-lengua-reply", "json")]),
            titled,
        ),
        "embed" => reply(send(MODEL, "/embed", Some(text), &[]), compared),
        "thrice" => statuses(&prompt),
        "call" => called(),
        _ => format!("`{mode}` is not a mode"),
    }
}

fn reply(sent: Result<(u16, Vec<String>), String>, good: impl Fn(&[String]) -> String) -> String {
    debug_assert!(sent.is_err() || sent.as_ref().is_ok_and(|(s, _)| *s >= 100));
    let (status, pieces) = match sent {
        Ok(found) => found,
        Err(message) => return format!("the request failed: {message}"),
    };
    debug_assert!(pieces.len() <= MOST_PIECES);
    if status == 200 {
        good(&pieces)
    } else if ai::retryable(status) {
        format!("the model failed for a short time ({status})")
    } else {
        format!("the model refused ({status}): {}", pieces.concat())
    }
}

fn streamed(pieces: &[String]) -> String {
    debug_assert!(pieces.len() <= MOST_PIECES);
    let how = if pieces.len() > 1 {
        "more than one piece"
    } else {
        "one piece"
    };
    debug_assert!(how.ends_with("piece"));
    format!("streamed in {how}: {}", pieces.concat())
}

fn titled(pieces: &[String]) -> String {
    let found = ai::fields(&pieces.concat(), &["title"]);
    debug_assert!(pieces.len() <= MOST_PIECES);
    debug_assert!(found.is_err() || found.as_ref().is_ok_and(|f| f.len() == 1));
    match found {
        Ok(fields) => fields
            .first()
            .map_or_else(String::new, |(name, value)| format!("{name}: {value}")),
        Err(ai::Error::Missing(name)) => format!("the answer has no field `{name}`"),
        Err(error) => format!("the answer is bad: {error:?}"),
    }
}

fn compared(pieces: &[String]) -> String {
    let found = vectors::parse_vector(&pieces.concat());
    debug_assert!(pieces.len() <= MOST_PIECES);
    debug_assert!(found.as_ref().is_none_or(|v| v.len() < 1 << 20));
    match found {
        Some(vector) => format!("{:?}", vectors::cosine(&vector, &[1.0, 0.0])),
        None => "the answer is not a vector".to_string(),
    }
}

fn statuses(prompt: &str) -> String {
    let mut found = Vec::new();
    for _ in 0..3 {
        let status = send(MODEL, "/ask", Some(prompt), &[]).map_or(0, |(s, _)| s);
        found.push(status.to_string());
    }
    debug_assert_eq!(found.len(), 3);
    debug_assert!(prompt.len() < 1 << 20);
    found.join(" ")
}

fn called() -> String {
    let key = std::env::var("SERVICE_KEY").unwrap_or_default();
    let bearer = format!("Bearer {key}");
    let sent = send(SERVICE, "/hello", None, &[("authorization", &bearer)]);
    debug_assert!(bearer.starts_with("Bearer "));
    debug_assert!(key.len() < 1 << 16);
    match sent {
        Ok((status, pieces)) => format!(
            "{status} {}, and the key has {} letters",
            pieces.concat(),
            key.len()
        ),
        Err(message) => format!("the request failed: {message}"),
    }
}

fn send(
    authority: &str,
    path: &str,
    body: Option<&str>,
    headers: &[(&str, &str)],
) -> Result<(u16, Vec<String>), String> {
    let fields = Fields::new();
    for (name, value) in headers {
        let _ = fields.append(name, value.as_bytes());
    }
    let request = OutgoingRequest::new(fields);
    let method = if body.is_some() {
        Method::Post
    } else {
        Method::Get
    };
    let _ = request.set_method(&method);
    let _ = request.set_scheme(Some(&Scheme::Http));
    let _ = request.set_authority(Some(authority));
    let _ = request.set_path_with_query(Some(path));
    let outgoing = request
        .body()
        .map_err(|()| "the request has no body".to_string())?;
    let future = outgoing_handler::handle(request, None).map_err(|e| format!("{e:?}"))?;
    write_body(&outgoing, body.unwrap_or_default());
    let _ = OutgoingBody::finish(outgoing, None);
    future.subscribe().block();
    let response = match future.get() {
        Some(Ok(Ok(response))) => response,
        Some(Ok(Err(code))) => return Err(format!("{code:?}")),
        _ => return Err("no answer".to_string()),
    };
    debug_assert!(authority.contains(':') || authority == MODEL);
    debug_assert!(path.starts_with('/'));
    Ok((response.status(), read_pieces(&response)))
}

fn write_body(outgoing: &OutgoingBody, text: &str) {
    debug_assert!(text.len() < 1 << 20);
    if let Ok(mut stream) = outgoing.write() {
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.flush();
    }
    debug_assert!(text.is_char_boundary(0));
}

fn read_pieces(response: &wasip2::http::types::IncomingResponse) -> Vec<String> {
    let mut pieces = Vec::new();
    let Ok(body) = response.consume() else {
        return pieces;
    };
    if let Ok(stream) = body.stream() {
        for _ in 0..MOST_PIECES {
            match stream.blocking_read(READ) {
                Ok(bytes) if bytes.is_empty() => {}
                Ok(bytes) => pieces.push(String::from_utf8_lossy(&bytes).to_string()),
                Err(StreamError::Closed | StreamError::LastOperationFailed(_)) => break,
            }
        }
    }
    debug_assert!(pieces.len() <= MOST_PIECES);
    debug_assert!(pieces.iter().all(|p| !p.is_empty()));
    pieces
}
