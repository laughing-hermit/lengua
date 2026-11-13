use std::fmt::Write as _;

use crate::Value;

enum Task<'a> {
    Value(&'a Value),
    Raw(&'static str),
    Key(&'a str),
}

#[must_use]
pub fn write(value: &Value) -> String {
    let mut out = String::new();
    let mut tasks = vec![Task::Value(value)];
    for _ in 0..usize::MAX {
        let Some(task) = tasks.pop() else {
            break;
        };
        match task {
            Task::Raw(text) => out.push_str(text),
            Task::Key(key) => {
                quote(key, &mut out);
                out.push(':');
            }
            Task::Value(item) => expand(item, &mut out, &mut tasks),
        }
    }
    debug_assert!(tasks.is_empty());
    debug_assert!(!out.is_empty());
    out
}

fn expand<'a>(value: &'a Value, out: &mut String, tasks: &mut Vec<Task<'a>>) {
    let before = tasks.len();
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(truth) => out.push_str(if *truth { "true" } else { "false" }),
        Value::Number(number) => out.push_str(number),
        Value::Text(text) => quote(text, out),
        Value::List(items) => {
            out.push('[');
            tasks.push(Task::Raw("]"));
            for (at, item) in items.iter().enumerate().rev() {
                tasks.push(Task::Value(item));
                if at > 0 {
                    tasks.push(Task::Raw(","));
                }
            }
        }
        Value::Object(fields) => {
            out.push('{');
            tasks.push(Task::Raw("}"));
            for (at, (key, item)) in fields.iter().enumerate().rev() {
                tasks.push(Task::Value(item));
                tasks.push(Task::Key(key));
                if at > 0 {
                    tasks.push(Task::Raw(","));
                }
            }
        }
    }
    debug_assert!(tasks.len() >= before);
    debug_assert!(!out.is_empty());
}

fn quote(text: &str, out: &mut String) {
    let before = out.len();
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if u32::from(control) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(control));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    debug_assert!(out.len() >= before + 2);
    debug_assert!(out.ends_with('"'));
}
