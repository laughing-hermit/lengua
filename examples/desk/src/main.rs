use std::io::{Read, Write};

use json::Value;
use wasip2::http::outgoing_handler;
use wasip2::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingRequest, OutgoingResponse,
    ResponseOutparam, Scheme,
};

const MODEL: &str = "model.native";
const LONGEST: usize = 40;
const TOO_LONG: &str = "Give a prompt of 40 characters or fewer.";

struct Desk;

wasip2::http::proxy::export!(Desk);

impl wasip2::exports::http::incoming_handler::Guest for Desk {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let post = matches!(request.method(), Method::Post);
        let path = request.path_with_query().unwrap_or_default();
        let body = body_of(&request);
        debug_assert!(path.len() < 1 << 16);
        debug_assert!(body.len() < 1 << 24);
        let (status, kind, bytes) = answer(post, &path, &body);
        respond(response_out, status, kind, &bytes);
    }
}

fn answer(post: bool, path: &str, body: &str) -> (u16, &'static str, Vec<u8>) {
    debug_assert!(path.len() < 1 << 16);
    debug_assert!(body.len() < 1 << 24);
    if post && path == "/api/write" {
        let (status, text) = write(body);
        return (status, "application/json", text.into_bytes());
    }
    match lengua_web::asset(path) {
        Some((kind, bytes)) if !post => (200, kind, bytes.to_vec()),
        _ => (404, "text/plain", b"not found".to_vec()),
    }
}

fn prompt_of(body: &str) -> Option<String> {
    let value = json::parse(body).ok()?;
    let prompt = match json::get(&value, "prompt") {
        Some(Value::Text(text)) => text.clone(),
        _ => return None,
    };
    debug_assert!(body.len() >= prompt.len());
    debug_assert!(body.starts_with('{'));
    let fits = !prompt.is_empty() && prompt.chars().count() <= LONGEST;
    fits.then_some(prompt)
}

fn write(body: &str) -> (u16, String) {
    let Some(prompt) = prompt_of(body) else {
        return (400, error(TOO_LONG));
    };
    let Some(text) = ask(&prompt) else {
        return (503, error("The model did not answer."));
    };
    let reply = json::write(&Value::Object(vec![(
        "text".to_string(),
        Value::Text(text.trim().to_string()),
    )]));
    debug_assert!(reply.starts_with("{\"text\":"));
    debug_assert!(prompt.chars().count() <= LONGEST);
    (200, reply)
}

fn error(message: &str) -> String {
    let text = json::write(&Value::Object(vec![(
        "error".to_string(),
        Value::Text(message.to_string()),
    )]));
    debug_assert!(text.starts_with("{\"error\":"));
    debug_assert!(text.len() > message.len());
    text
}

fn ask(prompt: &str) -> Option<String> {
    let request = OutgoingRequest::new(Fields::new());
    let _ = request.set_method(&Method::Post);
    let _ = request.set_scheme(Some(&Scheme::Http));
    let _ = request.set_authority(Some(MODEL));
    let _ = request.set_path_with_query(Some("/"));
    let outgoing = request.body().ok()?;
    let future = outgoing_handler::handle(request, None).ok()?;
    if let Ok(mut stream) = outgoing.write() {
        let _ = stream.write_all(prompt.as_bytes());
        let _ = stream.flush();
    }
    let _ = OutgoingBody::finish(outgoing, None);
    future.subscribe().block();
    let response = future.get()?.ok()?.ok()?;
    debug_assert!(prompt.len() <= 4 * LONGEST);
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

fn body_of(request: &IncomingRequest) -> String {
    let Ok(body) = request.consume() else {
        return String::new();
    };
    let mut bytes = Vec::new();
    if let Ok(mut stream) = body.stream() {
        let _ = stream.read_to_end(&mut bytes);
    }
    debug_assert!(bytes.len() < 1 << 24);
    let text = String::from_utf8(bytes).unwrap_or_default();
    debug_assert!(text.len() < 1 << 24);
    text
}

fn respond(response_out: ResponseOutparam, status: u16, kind: &str, bytes: &[u8]) {
    debug_assert!((100..600).contains(&status));
    debug_assert!(kind.len() < 1 << 16);
    let headers = Fields::new();
    let _ = headers.set("content-type", &[kind.as_bytes().to_vec()]);
    let response = OutgoingResponse::new(headers);
    let _ = response.set_status_code(status);
    let Ok(out) = response.body() else {
        return;
    };
    ResponseOutparam::set(response_out, Ok(response));
    if let Ok(mut stream) = out.write() {
        let _ = stream.write_all(bytes);
        let _ = stream.flush();
    }
    let _ = OutgoingBody::finish(out, None);
}
