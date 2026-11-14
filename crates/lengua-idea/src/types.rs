use std::fmt::Write as _;

use crate::json::Json;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    Unknown,
    Null,
    Bool,
    Number,
    Text,
    List(Box<Ty>),
    Object(Vec<(String, Ty, bool)>),
    Union(Vec<Ty>),
}

pub(crate) fn of(value: &Json) -> Ty {
    match value {
        Json::Null => Ty::Null,
        Json::Bool => Ty::Bool,
        Json::Number => Ty::Number,
        Json::Text => Ty::Text,
        Json::List(items) => Ty::List(Box::new(
            items.iter().map(of).fold(Ty::Unknown, |a, b| merge(&a, &b)),
        )),
        Json::Object(fields) => Ty::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), of(v), false))
                .collect(),
        ),
    }
}

fn members(ty: &Ty) -> Vec<Ty> {
    match ty {
        Ty::Union(list) => list.clone(),
        Ty::Unknown => Vec::new(),
        other => vec![other.clone()],
    }
}

fn merge_objects(a: &[(String, Ty, bool)], b: &[(String, Ty, bool)]) -> Ty {
    let mut out: Vec<(String, Ty, bool)> = Vec::new();
    for (key, ty, optional) in a {
        match b.iter().find(|(k, _, _)| k == key) {
            Some((_, other, other_optional)) => {
                out.push((key.clone(), merge(ty, other), *optional || *other_optional));
            }
            None => out.push((key.clone(), ty.clone(), true)),
        }
    }
    for (key, ty, _) in b {
        if !a.iter().any(|(k, _, _)| k == key) {
            out.push((key.clone(), ty.clone(), true));
        }
    }
    Ty::Object(out)
}

pub(crate) fn merge(a: &Ty, b: &Ty) -> Ty {
    match (a, b) {
        (Ty::Unknown, other) | (other, Ty::Unknown) => other.clone(),
        (Ty::List(x), Ty::List(y)) => Ty::List(Box::new(merge(x, y))),
        (Ty::Object(x), Ty::Object(y)) => merge_objects(x, y),
        _ if a == b => a.clone(),
        _ => {
            let mut list: Vec<Ty> = Vec::new();
            for member in members(a).into_iter().chain(members(b)) {
                let place = list.iter().position(|m| same_kind(m, &member));
                match place {
                    Some(i) => {
                        if let Some(slot) = list.get_mut(i) {
                            *slot = merge(slot, &member);
                        }
                    }
                    None => list.push(member),
                }
            }
            if list.len() == 1 {
                list.pop().unwrap_or(Ty::Unknown)
            } else {
                Ty::Union(list)
            }
        }
    }
}

fn same_kind(a: &Ty, b: &Ty) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b) && !matches!(a, Ty::Union(_))
}

pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn render(ty: &Ty) -> String {
    match ty {
        Ty::Unknown => "unknown".to_string(),
        Ty::Null => "null".to_string(),
        Ty::Bool => "boolean".to_string(),
        Ty::Number => "number".to_string(),
        Ty::Text => "string".to_string(),
        Ty::List(item) => match item.as_ref() {
            Ty::Union(_) => format!("({})[]", render(item)),
            other => format!("{}[]", render(other)),
        },
        Ty::Object(fields) => {
            if fields.is_empty() {
                return "Record<string, never>".to_string();
            }
            let parts: Vec<String> = fields
                .iter()
                .map(|(key, ty, optional)| {
                    let mark = if *optional { "?" } else { "" };
                    format!("{}{mark}: {}", quoted(key), render(ty))
                })
                .collect();
            format!("{{ {} }}", parts.join("; "))
        }
        Ty::Union(list) => list.iter().map(render).collect::<Vec<_>>().join(" | "),
    }
}

pub(crate) fn check(ty: &Ty, value: &str, depth: usize) -> String {
    match ty {
        Ty::Unknown => "true".to_string(),
        Ty::Null => format!("{value} === null"),
        Ty::Bool => format!("typeof {value} === \"boolean\""),
        Ty::Number => format!("typeof {value} === \"number\""),
        Ty::Text => format!("typeof {value} === \"string\""),
        Ty::List(item) if **item == Ty::Unknown => format!("Array.isArray({value})"),
        Ty::List(item) => {
            let inner = format!("v{}", depth + 1);
            format!(
                "Array.isArray({value}) && {value}.every(({inner}: unknown) => {})",
                check(item, &inner, depth + 1)
            )
        }
        Ty::Object(fields) => {
            let mut parts = vec![format!(
                "typeof {value} === \"object\" && {value} !== null && !Array.isArray({value})"
            )];
            for (key, field, optional) in fields {
                let name = quoted(key);
                let access = format!("{value}[{name}]");
                let inner = check(field, &access, depth);
                if *optional {
                    parts.push(format!("(!({name} in {value}) || ({inner}))"));
                } else {
                    parts.push(format!("{name} in {value} && ({inner})"));
                }
            }
            parts.join(" && ")
        }
        Ty::Union(list) => {
            let parts: Vec<String> = list
                .iter()
                .map(|t| format!("({})", check(t, value, depth)))
                .collect();
            parts.join(" || ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Ty, check, merge, of, render};
    use crate::json::parse;

    fn ty(text: &str) -> Ty {
        of(&parse(text).expect("json"))
    }

    #[test]
    fn examples_merge_into_one_type() {
        let a = ty("[{\"id\": 1, \"title\": \"a\"}]");
        let b = ty("[{\"id\": 2, \"title\": \"b\", \"done\": true}]");
        let both = merge(&a, &b);
        assert_eq!(
            render(&both),
            "{ \"id\": number; \"title\": string; \"done\"?: boolean }[]"
        );
        assert_eq!(render(&merge(&ty("1"), &ty("null"))), "number | null");
        assert_eq!(render(&ty("[]")), "unknown[]");
    }

    #[test]
    fn a_check_tests_the_shape_at_run_time() {
        let text = check(&ty("{\"tags\": [\"a\"]}"), "value", 0);
        assert!(text.contains("\"tags\" in value"), "{text}");
        assert!(
            text.contains("value[\"tags\"].every((v1: unknown) => typeof v1 === \"string\")"),
            "{text}"
        );
    }
}
