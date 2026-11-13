use crate::Sink;
use crate::frontmatter::{Entry, Front};

pub(crate) const LENGUA_CHECK: &str = "process:lengua-check";
pub(crate) const GENERATED_BY: &str = concat!("lengua-mcp/", env!("CARGO_PKG_VERSION"));
const STATUSES: [&str; 3] = ["draft", "stable", "deprecated"];

pub(crate) type Mapping = Vec<(String, String)>;

pub(crate) fn unquote(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return inner.replace("\\\"", "\"").replace("\\\\", "\\");
    }
    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
    {
        return inner.replace("''", "'");
    }
    value.to_string()
}

fn plain(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(char::is_whitespace)
}

pub(crate) fn actor(value: &str) -> bool {
    if let Some(id) = value
        .strip_prefix("human:")
        .or_else(|| value.strip_prefix("process:"))
    {
        return plain(id);
    }
    value
        .split_once('/')
        .is_some_and(|(producer, version)| {
            plain(producer) && plain(version) && !producer.contains(':')
        })
}

fn leap(year: u64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in(year: u64, month: u64) -> u64 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn number(text: &str, from: usize, to: usize) -> Option<u64> {
    let part = text.get(from..to)?;
    if part.bytes().all(|b| b.is_ascii_digit()) {
        part.parse().ok()
    } else {
        None
    }
}

fn offset(rest: &str) -> bool {
    let bytes = rest.as_bytes();
    bytes.len() == 6
        && (bytes[0] == b'+' || bytes[0] == b'-')
        && bytes[3] == b':'
        && number(rest, 1, 3).is_some_and(|h| h <= 23)
        && number(rest, 4, 6).is_some_and(|m| m <= 59)
}

pub(crate) fn timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let marks = bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':';
    let parts = [
        number(value, 0, 4),
        number(value, 5, 7),
        number(value, 8, 10),
        number(value, 11, 13),
        number(value, 14, 16),
        number(value, 17, 19),
    ];
    let [Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)] = parts
    else {
        return false;
    };
    let ranges = marks
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in(year, month)
        && hour <= 23
        && minute <= 59
        && second <= 60;
    if !ranges {
        return false;
    }
    let mut rest = &value[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return false;
        }
        rest = &fraction[digits..];
    }
    rest == "Z" || offset(rest)
}

fn split_top(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '{' | '[') => depth += 1,
            (None, '}' | ']') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

fn pair(text: &str) -> Option<(String, String)> {
    let (key, value) = text.split_once(':')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_string(), unquote(value)))
}

fn flow(text: &str) -> Option<Mapping> {
    let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?;
    split_top(inner).into_iter().map(pair).collect()
}

pub(crate) fn mappings(entry: &Entry) -> Option<Vec<Mapping>> {
    let mut lines = entry.text.lines();
    let head = lines.next()?.get(entry.key.len() + 1..)?.trim();
    let mut out: Vec<Mapping> = Vec::new();
    if !head.is_empty() {
        if let Some(inner) = head.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            for part in split_top(inner) {
                out.push(flow(part)?);
            }
        } else {
            out.push(flow(head)?);
        }
        let rest_blank = lines.all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'));
        return rest_blank.then_some(out);
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let item = trimmed
            .strip_prefix("- ")
            .or_else(|| (trimmed == "-").then_some(""));
        match item {
            Some(item) if item.trim_start().starts_with('{') => out.push(flow(item)?),
            Some("") => out.push(Vec::new()),
            Some(item) => out.push(vec![pair(item)?]),
            None => out.last_mut()?.push(pair(trimmed)?),
        }
    }
    Some(out)
}

pub(crate) fn get<'m>(mapping: &'m Mapping, key: &str) -> Option<&'m str> {
    mapping
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn check_trust(entry: &Entry, required: bool, sink: &mut Sink) {
    let key = entry.key.as_str();
    let Some(maps) = mappings(entry) else {
        sink.add(
            entry.line,
            "okf-trust",
            format!("lengua could not read `{key}`: OKF writes it as `{{ by: <actor>, at: <time> }}`, or a list of them"),
        );
        return;
    };
    for map in &maps {
        match get(map, "by") {
            Some(by) if actor(by) => {}
            Some(by) => sink.add(
                entry.line,
                "okf-actor",
                format!("`{by}` in `{key}` is not an OKF actor: write `human:<id>`, `process:<id>` or `<producer>/<version>`"),
            ),
            None => sink.add(
                entry.line,
                "okf-actor",
                format!("each entry of `{key}` has `by`, an OKF actor"),
            ),
        }
        match get(map, "at") {
            Some(at) if timestamp(at) => {}
            Some(at) => sink.add(
                entry.line,
                "okf-time",
                format!("`{at}` in `{key}` is not an ISO 8601 time with its offset, such as `2026-11-13T14:02:11Z`"),
            ),
            None if required => sink.add(
                entry.line,
                "okf-time",
                format!("each entry of `{key}` has `at`, an ISO 8601 time with its offset"),
            ),
            None => {}
        }
    }
}

fn check_sources(entry: &Entry, sink: &mut Sink) {
    let Some(maps) = mappings(entry) else {
        sink.add(
            entry.line,
            "okf-sources",
            "lengua could not read `sources`: OKF writes it as a list of mappings, each with a `resource`".to_string(),
        );
        return;
    };
    for map in &maps {
        if get(map, "resource").is_none_or(str::is_empty) {
            sink.add(
                entry.line,
                "okf-sources",
                "each entry of `sources` has a `resource`".to_string(),
            );
        }
        if let Some(author) = get(map, "author").filter(|a| !actor(a)) {
            sink.add(
                entry.line,
                "okf-actor",
                format!("`{author}`, the `author` of a source, is not an OKF actor"),
            );
        }
        if let Some(time) = get(map, "last_modified").filter(|t| !timestamp(t)) {
            sink.add(
                entry.line,
                "okf-time",
                format!("`{time}`, the `last_modified` of a source, is not an ISO 8601 time with its offset"),
            );
        }
    }
}

pub(crate) fn check_values(front: &Front, idea: bool, sink: &mut Sink) {
    for entry in &front.entries {
        let value = || {
            let first = entry.text.lines().next().unwrap_or_default();
            unquote(first.get(entry.key.len() + 1..).unwrap_or_default())
        };
        match entry.key.as_str() {
            "status" => {
                let status = value();
                if !STATUSES.contains(&status.as_str()) {
                    sink.add(
                        entry.line,
                        "okf-status",
                        format!("`{status}` is not an OKF status: write `draft`, `stable` or `deprecated`"),
                    );
                } else if idea && status == "stable" {
                    sink.add(
                        entry.line,
                        "okf-status",
                        "an idea is `draft` until the person approves it (L11), so write `draft` or `deprecated`".to_string(),
                    );
                }
            }
            "stale_after" => {
                let time = value();
                if !timestamp(&time) {
                    sink.add(
                        entry.line,
                        "okf-time",
                        format!("`{time}`, the `stale_after` of the page, is not an ISO 8601 time with its offset"),
                    );
                }
            }
            "verified" => check_trust(entry, true, sink),
            "generated" if !idea => check_trust(entry, false, sink),
            "sources" => check_sources(entry, sink),
            _ => {}
        }
    }
}

fn code_spans(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '`' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|c| **c == '`').count();
        let mut j = i + run;
        let mut closed = None;
        while j < chars.len() {
            let here = chars[j..].iter().take_while(|c| **c == '`').count();
            if here == run {
                closed = Some(j + here);
                break;
            }
            j += here.max(1);
        }
        if let Some(end) = closed {
            i = end;
        } else {
            out.extend(&chars[i..i + run]);
            i += run;
        }
    }
    out
}

pub(crate) fn fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

pub(crate) fn links(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (i, line) in text.lines().enumerate() {
        if fence(line) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let plain = code_spans(line);
        let mut rest = plain.as_str();
        while let Some(at) = rest.find("](") {
            let after = &rest[at + 2..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = after[..end].split_whitespace().next().unwrap_or_default();
            out.push((i + 1, target.to_string()));
            rest = &after[end..];
        }
    }
    out
}

pub(crate) fn local(target: &str) -> Option<&str> {
    let outside = target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:");
    if outside {
        return None;
    }
    let path = target.split('#').next().unwrap_or_default();
    (!path.is_empty()).then_some(path)
}

pub(crate) fn normal(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

pub(crate) fn relative(from_file: &str, to: &str) -> String {
    let from_path = normal(from_file);
    let mut from: Vec<&str> = from_path.split('/').collect();
    from.pop();
    let target = normal(to);
    let to_parts: Vec<&str> = target.split('/').collect();
    let common = from
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b && **a != "..")
        .count();
    let mut out: Vec<&str> = vec![".."; from.len() - common];
    out.extend(&to_parts[common..]);
    out.join("/")
}

pub(crate) fn date_heading(text: &str) -> Option<(u64, u64, u64)> {
    let day = text.strip_prefix("## ")?.trim_end();
    if day.len() != 10 || day.as_bytes()[4] != b'-' || day.as_bytes()[7] != b'-' {
        return None;
    }
    let (year, month, date) = (number(day, 0, 4)?, number(day, 5, 7)?, number(day, 8, 10)?);
    ((1..=12).contains(&month) && date >= 1 && date <= days_in(year, month))
        .then_some((year, month, date))
}

pub(crate) fn index_problems(text: &str, root: bool) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut sink = Sink::new("index.md");
    let front = crate::frontmatter::split(text, &mut sink);
    for finding in sink.findings {
        out.push((finding.line, finding.message));
    }
    for entry in &front.entries {
        if !(root && entry.key == "okf_version") {
            out.push((
                entry.line,
                format!("an `index.md` has no frontmatter but `okf_version`, and only at a bundle's root; `{}` is not allowed", entry.key),
            ));
        }
    }
    let mut sections = 0;
    for (i, line) in front.body.lines().enumerate() {
        let number = front.body_line + i;
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        let item = trimmed.starts_with("* ") || trimmed.starts_with("- ");
        if trimmed.starts_with("# ") {
            sections += 1;
        } else if !item || sections == 0 {
            out.push((
                number,
                "an `index.md` has only `# <section>` headings and `* [<title>](<link>) - <description>` lines".to_string(),
            ));
        }
    }
    if sections == 0 {
        out.push((1, "an `index.md` has one `# <section>` or more".to_string()));
    }
    out
}

pub(crate) fn log_problems(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut title = false;
    let mut last: Option<(u64, u64, u64)> = None;
    for (i, line) in text.lines().enumerate() {
        let number = i + 1;
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("## ") {
            match date_heading(trimmed) {
                Some(day) if last.is_some_and(|l| l <= day) => out.push((
                    number,
                    "a log is newest first, so each `## YYYY-MM-DD` is older than the one above it".to_string(),
                )),
                Some(day) => last = Some(day),
                None => out.push((
                    number,
                    "a log's headings are dates, `## YYYY-MM-DD`".to_string(),
                )),
            }
        } else if trimmed.starts_with("# ") && !title && last.is_none() {
            title = true;
        } else if last.is_none() || !(trimmed.starts_with("* ") || line.starts_with(' ')) {
            out.push((
                number,
                "a log has a `# ` title, then `## YYYY-MM-DD` headings, each with `* ` entries".to_string(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{actor, links, log_problems, normal, relative, timestamp};

    #[test]
    fn actors_follow_the_okf_convention() {
        for good in ["human:ana", "process:lengua-check", "lengua-mcp/0.1.0"] {
            assert!(actor(good), "{good}");
        }
        for bad in ["ana", "human:", "process: x", "/0.1", "a:b/1", ""] {
            assert!(!actor(bad), "{bad}");
        }
    }

    #[test]
    fn times_have_an_offset() {
        for good in [
            "2026-11-13T14:02:11Z",
            "2026-11-13T14:02:11.5+01:00",
            "2024-02-29T00:00:00-05:30",
        ] {
            assert!(timestamp(good), "{good}");
        }
        for bad in [
            "2026-11-13",
            "2026-11-13T14:02:11",
            "2026-13-01T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-11-13 14:02:11Z",
            "2026-11-13T24:00:00Z",
        ] {
            assert!(!timestamp(bad), "{bad}");
        }
    }

    #[test]
    fn links_outside_code_are_found() {
        let text = "See [a](a.md) and `[b](b.md)` and ``[c](`c`)``.\n```\n[d](d.md)\n```\n[e](https://x.y)\n";
        assert_eq!(
            links(text),
            [(1, "a.md".to_string()), (5, "https://x.y".to_string())]
        );
    }

    #[test]
    fn paths_are_made_relative() {
        assert_eq!(normal("a/./b/../c"), "a/c");
        assert_eq!(relative("idea.md", "ideas/cart.md"), "ideas/cart.md");
        assert_eq!(relative("ideas/cart.md", "ideas/pages.md"), "pages.md");
        assert_eq!(relative("ideas/ui/basket.md", "idea.md"), "../../idea.md");
        assert_eq!(
            relative("ideas/cart.md", "../../libraries/json/idea.md"),
            "../../../libraries/json/idea.md"
        );
    }

    #[test]
    fn a_log_is_newest_first() {
        assert_eq!(
            log_problems("# Log\n\n## 2026-11-13\n\n* one\n  * two\n\n## 2026-11-12\n\n* three\n"),
            []
        );
        let problems = log_problems("# Log\n\n## 2026-11-12\n\n* one\n\n## 2026-11-13\n\nText.\n");
        assert_eq!(problems.len(), 2, "{problems:?}");
    }
}
