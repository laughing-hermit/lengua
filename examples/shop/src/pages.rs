use crate::{basket, catalog};

pub fn route(path: &str) -> (u16, String) {
    let (page, query) = path.split_once('?').unwrap_or((path, ""));
    let answer = match page {
        "/items" => (200, json::write(&catalog::listing())),
        "/total" => total(query),
        _ => error(404, "no page"),
    };
    debug_assert!(answer.0 >= 200);
    debug_assert_ne!(answer.1.len(), 0);
    answer
}

pub fn total(query: &str) -> (u16, String) {
    let lines = match basket::parse_query(query) {
        Ok(lines) => lines,
        Err(message) => return error(400, &message),
    };
    let net = basket::net(&lines).unwrap_or(i64::MAX);
    let tax = crate::tax_of(net);
    let sum = decimal::add(net, tax).unwrap_or(i64::MAX);
    debug_assert!(sum >= net);
    debug_assert!(tax >= 0);
    let fields = [("net", net), ("tax", tax), ("total", sum)]
        .iter()
        .map(|(name, cents)| {
            (
                (*name).to_string(),
                json::Value::Text(decimal::format_fixed(*cents, 2)),
            )
        })
        .collect();
    (200, json::write(&json::Value::Object(fields)))
}

pub fn error(status: u16, message: &str) -> (u16, String) {
    let body = json::write(&json::Value::Object(vec![(
        "error".to_string(),
        json::Value::Text(message.to_string()),
    )]));
    debug_assert!(status >= 400);
    debug_assert!(body.starts_with('{'));
    (status, body)
}
