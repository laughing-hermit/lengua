mod notes;

use std::io::{Read, Write};

use wasip2::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

const LIMIT: u64 = 1 << 16;
const JSON: &str = "application/json";

struct Notes;

wasip2::http::proxy::export!(Notes);

impl wasip2::exports::http::incoming_handler::Guest for Notes {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let path = request.path_with_query().unwrap_or_default();
        let post = matches!(request.method(), Method::Post);
        let body = if post { body_of(&request) } else { Vec::new() };
        let (status, kind, bytes) = answer(&path, post, &body);
        debug_assert!((100..600).contains(&status));
        debug_assert_ne!(kind.len(), 0);
        send(response_out, status, kind, &bytes);
    }
}

fn body_of(request: &IncomingRequest) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Ok(body) = request.consume() {
        if let Ok(stream) = body.stream() {
            let _ = stream.take(LIMIT).read_to_end(&mut bytes);
        }
    }
    debug_assert!(bytes.len() <= usize::try_from(LIMIT).unwrap_or(usize::MAX));
    debug_assert!(bytes.capacity() >= bytes.len());
    bytes
}

fn answer(path: &str, post: bool, body: &[u8]) -> (u16, &'static str, Vec<u8>) {
    let page = path.split('?').next().unwrap_or(path);
    let found = if page.starts_with("/api/") {
        let (status, text) = notes::route(page, post, body);
        (status, JSON, text.into_bytes())
    } else if let Some((kind, bytes)) = web_files::asset(page).filter(|_| !post) {
        (200, kind, bytes.to_vec())
    } else {
        let (status, text) = notes::error(404, "no page");
        (status, JSON, text.into_bytes())
    };
    debug_assert!(found.0 == 200 || found.1 == JSON);
    debug_assert_ne!(found.2.len(), 0);
    found
}

fn send(response_out: ResponseOutparam, status: u16, kind: &str, bytes: &[u8]) {
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
    debug_assert!(status >= 100);
    debug_assert_ne!(kind.len(), 0);
    let _ = OutgoingBody::finish(out, None);
}
