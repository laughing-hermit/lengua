mod accounts;
mod notes;
mod summary;

use std::io::{Read, Write};

use wasip2::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

struct Notes;

wasip2::http::proxy::export!(Notes);

impl wasip2::exports::http::incoming_handler::Guest for Notes {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = method_name(&request.method());
        let path = request.path_with_query().unwrap_or_default();
        let token = header(&request, "authorization");
        let body = body_of(&request);
        debug_assert!(method.len() < 1 << 16);
        debug_assert!(body.len() < 1 << 20);
        let (status, kind, bytes) = answer(method, &path, &body, &token);
        respond(response_out, status, kind, &bytes);
    }
}

fn answer(method: &str, path: &str, body: &str, token: &str) -> (u16, &'static str, Vec<u8>) {
    debug_assert!(method.len() < 1 << 16);
    debug_assert!(path.len() < 1 << 16);
    if path.starts_with("/api/") {
        let (status, text) = notes::route(method, path, body, token);
        return (status, "application/json", text.into_bytes());
    }
    match lengua_web::asset(path) {
        Some((kind, bytes)) if method == "GET" => (200, kind, bytes.to_vec()),
        _ => (404, "text/plain", b"not found".to_vec()),
    }
}

fn method_name(method: &Method) -> &'static str {
    let name = match method {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Delete => "DELETE",
        Method::Patch => "PATCH",
        Method::Head => "HEAD",
        Method::Options => "OPTIONS",
        _ => "OTHER",
    };
    debug_assert!(name.len() < 1 << 16);
    debug_assert!(name.chars().all(|c| c.is_ascii_uppercase()));
    name
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

fn header(request: &IncomingRequest, name: &str) -> String {
    let values = request.headers().get(name);
    let text = values
        .first()
        .map(|value| String::from_utf8_lossy(value).to_string())
        .unwrap_or_default();
    debug_assert!(name.len() < 1 << 10);
    debug_assert!(text.len() < 1 << 16);
    text
}
