use json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotJson,
    NotObject,
    Missing(String),
}

#[must_use]
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    for _ in 0..template.len() {
        let Some(open) = rest.find(['{', '}']) else {
            break;
        };
        out.push_str(rest.get(..open).unwrap_or_default());
        let after = rest.get(open..).unwrap_or_default();
        let (written, used) = placeholder(after, values);
        out.push_str(&written);
        rest = after.get(used..).unwrap_or_default();
    }
    out.push_str(rest);
    debug_assert!(rest.len() <= template.len());
    debug_assert!(values.len() < usize::MAX);
    out
}

fn placeholder(text: &str, values: &[(&str, &str)]) -> (String, usize) {
    debug_assert!(text.starts_with(['{', '}']));
    if text.starts_with("{{") || text.starts_with("}}") {
        return (text.get(..1).unwrap_or_default().to_string(), 2);
    }
    let name = text
        .strip_prefix('{')
        .and_then(|t| t.split_once('}'))
        .map(|(name, _)| name);
    let found = name.and_then(|n| {
        values
            .iter()
            .find(|(key, _)| *key == n)
            .map(|(_, v)| (n, *v))
    });
    debug_assert!(!text.is_empty());
    match found {
        Some((name, value)) => (value.to_string(), name.len() + 2),
        None => (text.get(..1).unwrap_or_default().to_string(), 1),
    }
}

#[must_use]
pub fn then(template: &str, answer: &str) -> String {
    let text = fill(template, &[("answer", answer)]);
    debug_assert!(!template.contains("{answer}") || text.contains(answer));
    debug_assert!(template.contains("{answer}") || text.len() <= template.len());
    text
}

fn plain(value: &Value) -> Option<String> {
    let text = match value {
        Value::Text(text) | Value::Number(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        _ => return None,
    };
    debug_assert!(text.len() < usize::MAX);
    debug_assert!(!matches!(value, Value::Null));
    Some(text)
}

pub fn fields(answer: &str, names: &[&str]) -> Result<Vec<(String, String)>, Error> {
    let value = json::parse(answer.trim()).map_err(|_| Error::NotJson)?;
    if !matches!(value, Value::Object(_)) {
        return Err(Error::NotObject);
    }
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let text = json::get(&value, name)
            .and_then(plain)
            .ok_or_else(|| Error::Missing((*name).to_string()))?;
        out.push(((*name).to_string(), text));
    }
    debug_assert_eq!(out.len(), names.len());
    debug_assert!(answer.len() >= 2);
    Ok(out)
}

#[must_use]
pub fn retryable(status: u16) -> bool {
    let again = matches!(status, 500 | 502 | 503 | 504);
    debug_assert!(!again || status >= 500);
    debug_assert!(status < 1000 || !again);
    again
}

#[must_use]
pub fn estimate_tokens(text: &str) -> u64 {
    let tokens = u64::try_from(text.len().div_ceil(4)).unwrap_or(u64::MAX);
    debug_assert!(tokens <= u64::try_from(text.len()).unwrap_or(u64::MAX));
    debug_assert!(text.is_empty() || tokens > 0);
    tokens
}
