mod basket;
mod catalog;
mod pages;

use std::io::Write;

use wasip2::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

wit_bindgen::generate!({
    inline: "package lengua:tax;\n\ninterface rates {\n  tax-of: func(cents: u64) -> u64;\n}\n\nworld shop {\n  import rates;\n}\n",
});

struct Shop;

wasip2::http::proxy::export!(Shop);

impl wasip2::exports::http::incoming_handler::Guest for Shop {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let path = request.path_with_query().unwrap_or_default();
        let (status, body) = pages::route(&path);
        debug_assert!((100..600).contains(&status));
        debug_assert!(body.len() < 1 << 20);
        let headers = Fields::new();
        let _ = headers.set("content-type", &[b"application/json".to_vec()]);
        let response = OutgoingResponse::new(headers);
        let _ = response.set_status_code(status);
        let Ok(out) = response.body() else {
            return;
        };
        ResponseOutparam::set(response_out, Ok(response));
        if let Ok(mut stream) = out.write() {
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        }
        let _ = OutgoingBody::finish(out, None);
    }
}

fn tax_for(cents: i64) -> i64 {
    let unsigned = u64::try_from(cents).unwrap_or_default();
    let tax = i64::try_from(lengua::tax::rates::tax_of(unsigned)).unwrap_or(i64::MAX);
    debug_assert!(tax >= 0);
    debug_assert!(tax <= cents || cents <= 0);
    tax
}
