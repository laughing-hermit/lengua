use std::io::{Read as _, Write as _};

use wasip2::http::outgoing_handler;
use wasip2::http::types::{Fields, Method, OutgoingBody, OutgoingRequest, Scheme};

use crate::notes::{error, find, number, quoted};

const MODEL: &str = "ai.lengua";

pub fn summary(body: &str) -> (u16, String) {
    debug_assert!(body.len() < 1 << 20);
    let Some((_, title, text)) = number(body, "id").and_then(find) else {
        return (404, error("No note has this number."));
    };
    let prompt = format!("Write a summary of this note in one short line. {title}: {text}");
    let Some(answer) = ask(&prompt) else {
        return (503, error("The summary is not ready."));
    };
    let content = quoted(answer.trim());
    debug_assert!(content.starts_with('"'));
    (200, spec(&content))
}

fn spec(content: &str) -> String {
    let text = format!(
        "{{\"root\":\"card\",\"elements\":{{\"card\":{{\"type\":\"Card\",\"props\":{{\"title\":\"Summary\"}},\"children\":[\"text\"]}},\"text\":{{\"type\":\"Text\",\"props\":{{\"content\":{content}}},\"children\":[]}}}}}}"
    );
    debug_assert!(text.contains(content));
    debug_assert!(text.starts_with('{'));
    text
}

fn ask(prompt: &str) -> Option<String> {
    let request = OutgoingRequest::new(Fields::new());
    let _ = request.set_method(&Method::Post);
    let _ = request.set_scheme(Some(&Scheme::Http));
    let _ = request.set_authority(Some(MODEL));
    let _ = request.set_path_with_query(Some("/ask"));
    let outgoing = request.body().ok()?;
    let future = outgoing_handler::handle(request, None).ok()?;
    if let Ok(mut stream) = outgoing.write() {
        let _ = stream.write_all(prompt.as_bytes());
        let _ = stream.flush();
    }
    let _ = OutgoingBody::finish(outgoing, None);
    future.subscribe().block();
    let response = future.get()?.ok()?.ok()?;
    debug_assert!(!prompt.is_empty());
    debug_assert!(response.status() >= 100);
    if response.status() != 200 {
        return None;
    }
    let body = response.consume().ok()?;
    let mut bytes = Vec::new();
    if let Ok(mut stream) = body.stream() {
        let _ = stream.read_to_end(&mut bytes);
    }
    String::from_utf8(bytes).ok()
}
