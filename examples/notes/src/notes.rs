const NOTES: [(u32, &str, &str); 2] = [
    (1, "Milk", "Buy two bottles."),
    (2, "Call", "Call the shop at noon."),
];

pub fn route(method: &str, path: &str, body: &str) -> (u16, String) {
    debug_assert!(path.starts_with("/api/"));
    debug_assert!(method.len() < 1 << 16);
    match (method, path) {
        ("GET", "/api/notes") => (200, all()),
        ("POST", "/api/notes") => add(body),
        ("POST", "/api/ui") => summary(body),
        ("GET", _) => match path.strip_prefix("/api/notes/") {
            Some(id) => one(id),
            None => (404, error("No such route.")),
        },
        _ => (404, error("No such route.")),
    }
}

fn all() -> String {
    let parts: Vec<String> = NOTES
        .iter()
        .map(|(id, title, text)| note_json(*id, title, text))
        .collect();
    debug_assert_eq!(parts.len(), NOTES.len());
    let list = format!("[{}]", parts.join(","));
    debug_assert!(list.starts_with('['));
    list
}

fn one(id: &str) -> (u16, String) {
    debug_assert!(id.len() < 1 << 10);
    let found = id.parse::<u32>().ok().and_then(find);
    let answer = match found {
        Some((id, title, text)) => (200, note_json(id, title, text)),
        None => (404, error("No note has this number.")),
    };
    debug_assert!(answer.0 == 200 || answer.0 == 404);
    answer
}

fn add(body: &str) -> (u16, String) {
    debug_assert!(body.len() < 1 << 20);
    let title = field(body, "title").unwrap_or_default();
    let text = field(body, "text").unwrap_or_default();
    let problem = titles::title_error(&title);
    if !problem.is_empty() {
        return (400, error(&problem));
    }
    let next = u32::try_from(NOTES.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    debug_assert!(next > 2);
    (201, note_json(next, &title, &text))
}

fn summary(body: &str) -> (u16, String) {
    debug_assert!(body.len() < 1 << 20);
    let Some((_, title, text)) = number(body, "id").and_then(find) else {
        return (404, error("No note has this number."));
    };
    let content = quoted(&format!("{title}: {text}"));
    let spec = format!(
        "{{\"root\":\"card\",\"elements\":{{\"card\":{{\"type\":\"Card\",\"props\":{{\"title\":\"Summary\"}},\"children\":[\"text\"]}},\"text\":{{\"type\":\"Text\",\"props\":{{\"content\":{content}}},\"children\":[]}}}}}}"
    );
    debug_assert!(spec.contains(&content));
    (200, spec)
}

fn note_json(id: u32, title: &str, text: &str) -> String {
    debug_assert!(id > 0);
    debug_assert!(title.len() < 1 << 16);
    format!(
        "{{\"id\":{id},\"title\":{},\"text\":{}}}",
        quoted(title),
        quoted(text)
    )
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    debug_assert!(out.len() >= 2);
    debug_assert!(out.starts_with('"'));
    out
}

fn field(body: &str, name: &str) -> Option<String> {
    debug_assert!(name.len() < 1 << 16);
    let key = format!("\"{name}\"");
    let rest = body.split_once(&key)?.1.trim_start().strip_prefix(':')?;
    let rest = rest.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut escaped = false;
    for c in rest.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            debug_assert!(out.len() <= rest.len());
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn number(body: &str, name: &str) -> Option<u32> {
    debug_assert!(name.len() < 1 << 16);
    let key = format!("\"{name}\"");
    let rest = body.split_once(&key)?.1.trim_start().strip_prefix(':')?;
    let digits: String = rest
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    debug_assert!(digits.len() <= rest.len());
    digits.parse().ok()
}

fn error(message: &str) -> String {
    debug_assert!(message.len() < 1 << 16);
    let text = format!("{{\"error\":{}}}", quoted(message));
    debug_assert!(text.contains(message));
    text
}

fn find(id: u32) -> Option<(u32, &'static str, &'static str)> {
    debug_assert_eq!(NOTES.len(), 2);
    let found = NOTES.iter().find(|(n, _, _)| *n == id).copied();
    debug_assert!(found.is_none_or(|(n, _, _)| n == id));
    found
}
