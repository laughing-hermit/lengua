use json::Value;

pub const TITLE_LIMIT: usize = 40;

pub fn listing() -> Value {
    let notes = [
        ("1", "Milk", "Buy two bottles."),
        ("2", "Call", "Call the shop at noon."),
    ];
    let items: Vec<Value> = notes
        .iter()
        .map(|(id, title, text)| {
            Value::Object(vec![
                ("id".to_string(), Value::Text((*id).to_string())),
                ("title".to_string(), Value::Text((*title).to_string())),
                ("text".to_string(), Value::Text((*text).to_string())),
            ])
        })
        .collect();
    debug_assert_eq!(items.len(), notes.len());
    debug_assert_ne!(items.len(), 0);
    Value::List(items)
}

pub fn route(page: &str, post: bool, body: &[u8]) -> (u16, String) {
    let answer = match (page, post) {
        ("/api/notes", false) => (200, json::write(&listing())),
        ("/api/notes", true) => create(body),
        _ => error(404, "no page"),
    };
    debug_assert!(answer.0 >= 200);
    debug_assert!(answer.1.starts_with('{') || answer.1.starts_with('['));
    answer
}

pub fn create(body: &[u8]) -> (u16, String) {
    let Ok(text) = std::str::from_utf8(body) else {
        return error(400, "The body is not text.");
    };
    let Ok(value) = json::parse(text) else {
        return error(400, "The body is not JSON.");
    };
    let title = text_of(&value, "title");
    if let Some(problem) = check(&title) {
        return error(422, problem);
    }
    let note = Value::Object(vec![
        ("id".to_string(), Value::Text("3".to_string())),
        ("title".to_string(), Value::Text(title.clone())),
        ("text".to_string(), Value::Text(text_of(&value, "text"))),
    ]);
    debug_assert_ne!(title.trim().len(), 0);
    debug_assert!(title.chars().count() <= TITLE_LIMIT);
    (201, json::write(&note))
}

pub fn check(title: &str) -> Option<&'static str> {
    let found = if title.trim().is_empty() {
        Some("The title is empty.")
    } else if title.chars().count() > TITLE_LIMIT {
        Some("The title is too long.")
    } else {
        None
    };
    debug_assert!(found.is_some() || !title.trim().is_empty());
    debug_assert!(found.is_some() || title.chars().count() <= TITLE_LIMIT);
    found
}

pub fn text_of(value: &Value, field: &str) -> String {
    let text = if let Some(Value::Text(text)) = json::get(value, field) {
        text.clone()
    } else {
        String::new()
    };
    debug_assert_ne!(field.len(), 0);
    debug_assert!(text.len() <= 1 << 16);
    text
}

pub fn error(status: u16, message: &str) -> (u16, String) {
    let body = json::write(&Value::Object(vec![(
        "error".to_string(),
        Value::Text(message.to_string()),
    )]));
    debug_assert!(status >= 400);
    debug_assert!(body.starts_with('{'));
    (status, body)
}
