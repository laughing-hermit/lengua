use std::io::Write;

use wasip2::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

struct Greet;

wasip2::http::proxy::export!(Greet);

impl wasip2::exports::http::incoming_handler::Guest for Greet {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let path = request.path_with_query().unwrap_or_default();
        let (status, text) = answer(&path);
        debug_assert!(text.len() < 1 << 20);
        debug_assert!((100..600).contains(&status));
        let headers = Fields::new();
        let _ = headers.set("content-type", &[b"text/plain".to_vec()]);
        let response = OutgoingResponse::new(headers);
        let _ = response.set_status_code(status);
        let Ok(body) = response.body() else {
            return;
        };
        ResponseOutparam::set(response_out, Ok(response));
        if let Ok(mut out) = body.write() {
            let _ = out.write_all(text.as_bytes());
            let _ = out.flush();
        }
        let _ = OutgoingBody::finish(body, None);
    }
}

fn answer(path: &str) -> (u16, String) {
    debug_assert!(path.len() < 1 << 16);
    let reply = match path.strip_prefix("/hello?name=") {
        Some(name) if !name.is_empty() => (200, format!("Hello, {name}!")),
        _ => (404, "not found".to_string()),
    };
    debug_assert!(reply.0 == 200 || reply.0 == 404);
    reply
}
