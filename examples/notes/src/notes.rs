use std::fs;

const FILE: &str = "data/notes.txt";
const FIRST: [(u32, &str, &str); 2] = [
    (1, "Milk", "Buy two bottles."),
    (2, "Call", "Call the shop at noon."),
];

pub type Note = (u32, String, String);

pub fn route(method: &str, path: &str, body: &str, token: &str) -> (u16, String) {
    debug_assert!(path.starts_with("/api/"));
    debug_assert!(method.len() < 1 << 16);
    match (method, path) {
        ("GET", "/api/notes") => (200, all()),
        ("POST", "/api/notes") => add(body),
        ("POST", "/api/ui") => crate::summary::summary(body),
        ("POST", "/api/register") => crate::accounts::register(body),
        ("POST", "/api/login") => crate::accounts::login(body),
        ("GET", "/api/me") => crate::accounts::me(token),
        ("GET", _) => match path.strip_prefix("/api/notes/") {
            Some(id) => one(id),
            None => (404, error("No such route.")),
        },
        _ => (404, error("No such route.")),
    }
}

pub fn load() -> Vec<Note> {
    let Ok(text) = fs::read_to_string(FILE) else {
        return FIRST
            .iter()
            .map(|(id, title, text)| (*id, (*title).to_string(), (*text).to_string()))
            .collect();
    };
    let notes: Vec<Note> = text.lines().filter_map(note_line).collect();
    debug_assert!(notes.len() <= text.lines().count());
    debug_assert!(notes.iter().all(|(id, _, _)| *id > 0));
    notes
}

fn note_line(line: &str) -> Option<Note> {
    let mut parts = line.splitn(3, '\t');
    let id = parts.next()?.parse::<u32>().ok().filter(|n| *n > 0)?;
    let title = parts.next()?.to_string();
    let text = parts.next().unwrap_or_default().to_string();
    debug_assert!(id > 0);
    debug_assert!(title.len() < line.len());
    Some((id, title, text))
}

fn save(notes: &[Note]) -> bool {
    let text: String = notes
        .iter()
        .map(|(id, title, text)| format!("{id}\t{title}\t{text}\n"))
        .collect();
    debug_assert_eq!(text.lines().count(), notes.len());
    debug_assert!(notes.len() < 1 << 20);
    fs::write(FILE, text).is_ok()
}

fn all() -> String {
    let parts: Vec<String> = load()
        .iter()
        .map(|(id, title, text)| note_json(*id, title, text))
        .collect();
    let list = format!("[{}]", parts.join(","));
    debug_assert!(list.starts_with('['));
    debug_assert!(list.ends_with(']'));
    list
}

fn one(id: &str) -> (u16, String) {
    debug_assert!(id.len() < 1 << 10);
    let found = id.parse::<u32>().ok().and_then(find);
    let answer = match found {
        Some((id, title, text)) => (200, note_json(id, &title, &text)),
        None => (404, error("No note has this number.")),
    };
    debug_assert!(answer.0 == 200 || answer.0 == 404);
    answer
}

fn clean(text: &str) -> String {
    let out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    debug_assert!(!out.contains('\t'));
    debug_assert!(!out.contains('\n'));
    out
}

fn add(body: &str) -> (u16, String) {
    debug_assert!(body.len() < 1 << 20);
    let title = clean(&field(body, "title").unwrap_or_default());
    let text = clean(&field(body, "text").unwrap_or_default());
    let problem = titles::title_error(&title);
    if !problem.is_empty() {
        return (400, error(&problem));
    }
    let mut notes = load();
    let next = notes
        .iter()
        .map(|(id, _, _)| *id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    debug_assert!(next > 0);
    notes.push((next, title.clone(), text.clone()));
    if !save(&notes) {
        return (500, error("The note was not saved."));
    }
    (201, note_json(next, &title, &text))
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

pub fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    debug_assert!(out.len() >= 2);
    debug_assert!(out.starts_with('"'));
    out
}

pub fn field(body: &str, name: &str) -> Option<String> {
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

pub fn number(body: &str, name: &str) -> Option<u32> {
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

pub fn error(message: &str) -> String {
    debug_assert!(message.len() < 1 << 16);
    let text = format!("{{\"error\":{}}}", quoted(message));
    debug_assert!(text.contains(message));
    text
}

pub fn find(id: u32) -> Option<Note> {
    let notes = load();
    let found = notes.into_iter().find(|(n, _, _)| *n == id);
    debug_assert!(found.as_ref().is_none_or(|(n, _, _)| *n == id));
    debug_assert!(
        found
            .as_ref()
            .is_none_or(|(_, title, _)| !title.contains('\t'))
    );
    found
}
