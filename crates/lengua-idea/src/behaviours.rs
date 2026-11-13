use std::collections::BTreeSet;

use crate::Sink;
use crate::cases::{End, Range, StateValue, Test};
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{
    Line, Token, backticked, is_fence, literal, relative_path, split_headings, split_key, tokenize,
};

const KEYWORDS: [&str; 4] = ["Given", "When", "Then", "And"];
const PROGRAM_KEYS: &str = "`Standard input`, `Arguments`, `File `path``, `Server `host:port``, `Client `port``, `Call`, `Request`, `Request header `name``, `Request body`, `Expect standard output`, `Expect standard error`, `Expect exit code`, `Expect end`, `Expect file `path``, `Expect answer`, `Expect value`, `Expect status`, `Expect header `name``, `Expect body`";
type Make = fn(String) -> Key;

const METHODS: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"];
const SCREEN_KEYS: &str =
    "`State `field``, `Click`, `Type into `element``, `Expect state `field``, `Expect text`, `Expect label`";
pub(crate) const MAX_TEXT_LENGTH: u64 = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Key {
    Input,
    Arguments,
    File(String),
    Output,
    ErrorOutput,
    ExitCode,
    End,
    OutputFile(String),
    State(String),
    Click,
    TypeInto(String),
    ExpectState(String),
    ExpectText,
    ExpectLabel,
    Range(String),
    Server(String),
    Client(String),
    Answer,
    Call,
    Value,
    Request,
    RequestHeader(String),
    RequestBody,
    Status,
    ExpectHeader(String),
    ExpectBody,
}

impl Key {
    fn exact(text: &str) -> Option<Self> {
        Some(match text {
            "Standard input" => Self::Input,
            "Arguments" => Self::Arguments,
            "Expect standard output" => Self::Output,
            "Expect standard error" => Self::ErrorOutput,
            "Expect exit code" => Self::ExitCode,
            "Expect end" => Self::End,
            "Click" => Self::Click,
            "Expect text" => Self::ExpectText,
            "Expect label" => Self::ExpectLabel,
            "Expect answer" => Self::Answer,
            "Call" => Self::Call,
            "Expect value" => Self::Value,
            "Request" => Self::Request,
            "Request body" => Self::RequestBody,
            "Expect status" => Self::Status,
            "Expect body" => Self::ExpectBody,
            _ => return None,
        })
    }

    fn parse(text: &str) -> Option<Self> {
        if let Some(found) = Self::exact(text) {
            return Some(found);
        }
        let named = |prefix: &str| {
            text.strip_prefix(prefix)
                .and_then(backticked)
                .map(str::to_string)
        };
        let shapes: [(&str, Make); 10] = [
            ("Expect file ", Self::OutputFile),
            ("File ", Self::File),
            ("Expect state ", Self::ExpectState),
            ("State ", Self::State),
            ("Type into ", Self::TypeInto),
            ("Range ", Self::Range),
            ("Server ", Self::Server),
            ("Client ", Self::Client),
            ("Request header ", Self::RequestHeader),
            ("Expect header ", Self::ExpectHeader),
        ];
        shapes
            .into_iter()
            .find_map(|(prefix, make)| named(prefix).map(make))
    }

    fn screen(&self) -> Option<bool> {
        match self {
            Self::Input
            | Self::Arguments
            | Self::File(_)
            | Self::Output
            | Self::ErrorOutput
            | Self::ExitCode
            | Self::End
            | Self::OutputFile(_)
            | Self::Server(_)
            | Self::Client(_)
            | Self::Answer
            | Self::Call
            | Self::Value
            | Self::Request
            | Self::RequestHeader(_)
            | Self::RequestBody
            | Self::Status
            | Self::ExpectHeader(_)
            | Self::ExpectBody => Some(false),
            Self::State(_)
            | Self::Click
            | Self::TypeInto(_)
            | Self::ExpectState(_)
            | Self::ExpectText
            | Self::ExpectLabel => Some(true),
            Self::Range(_) => None,
        }
    }

    fn expects(&self) -> bool {
        matches!(
            self,
            Self::Output
                | Self::ErrorOutput
                | Self::ExitCode
                | Self::End
                | Self::OutputFile(_)
                | Self::ExpectState(_)
                | Self::ExpectText
                | Self::ExpectLabel
                | Self::Answer
                | Self::Value
                | Self::Status
                | Self::ExpectHeader(_)
                | Self::ExpectBody
        )
    }
}

fn request_line(value: &str) -> Option<String> {
    let inner = backticked(value)?;
    let (method, target) = inner.split_once(' ')?;
    let good = METHODS.contains(&method)
        && target.starts_with('/')
        && !target.contains(char::is_whitespace);
    good.then(|| inner.to_string())
}

fn port(text: &str) -> Option<u16> {
    text.parse::<u16>().ok().filter(|p| *p > 0)
}

fn address(text: &str) -> bool {
    text.rsplit_once(':').is_some_and(|(host, p)| {
        port(p).is_some()
            && !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    })
}

struct Item<'l> {
    line: usize,
    text: String,
    more: Vec<&'l Line>,
}

fn items<'l>(lines: &'l [Line], sink: &mut Sink) -> Vec<Item<'l>> {
    let mut out: Vec<Item<'l>> = Vec::new();
    let mut fence = false;
    for line in lines {
        let text = line.text.as_str();
        let inside = fence || text.starts_with(' ') || text.trim().is_empty() || is_fence(text);
        if is_fence(text) {
            fence = !fence;
        }
        if let Some(rest) = text.strip_prefix("- ").filter(|_| !inside) {
            out.push(Item {
                line: line.number,
                text: rest.trim().to_string(),
                more: Vec::new(),
            });
        } else if let Some(last) = out.last_mut().filter(|_| inside) {
            last.more.push(line);
        } else if !text.trim().is_empty() {
            sink.add(
                line.number,
                "example-text",
                "an example has only list items, each one `- Key: value`".to_string(),
            );
        }
    }
    out
}

fn fenced(item: &Item, sink: &mut Sink) -> Option<String> {
    let mut lines = item.more.iter().skip_while(|l| l.text.trim().is_empty());
    let Some(open) = lines.next() else {
        sink.add(
            item.line,
            "example-value",
            "the value is empty; write a string in quotes, or a fenced block on the lines below"
                .to_string(),
        );
        return None;
    };
    if !is_fence(&open.text) {
        sink.add(
            open.number,
            "example-value",
            "a value on the lines below an item is a fenced block that starts with three backticks"
                .to_string(),
        );
        return None;
    }
    let indent = open.text.len() - open.text.trim_start().len();
    let mut out = String::new();
    for line in lines {
        if is_fence(&line.text) {
            return Some(out);
        }
        let text = line.text.as_str();
        let content = if text.chars().take(indent).all(char::is_whitespace) {
            text.get(indent..).unwrap_or("")
        } else {
            text.trim_start()
        };
        out.push_str(content);
        out.push('\n');
    }
    sink.add(
        open.number,
        "example-fence-unclosed",
        "this fenced block has no closing three backticks".to_string(),
    );
    None
}

fn one_string(item: &Item, value: &str, sink: &mut Sink) -> Option<String> {
    if value.is_empty() {
        return fenced(item, sink);
    }
    match literal(value) {
        Ok((text, rest)) if rest.trim().is_empty() => Some(text),
        Ok(_) => {
            sink.add(
                item.line,
                "example-value",
                "write one string in quotes and nothing after it".to_string(),
            );
            None
        }
        Err(message) => {
            sink.add(item.line, "example-string", message);
            None
        }
    }
}

fn strings(item: &Item, value: &str, sink: &mut Sink) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        match literal(rest) {
            Ok((text, tail)) => {
                out.push(text);
                rest = tail.trim_start();
            }
            Err(message) => {
                sink.add(item.line, "example-string", message);
                break;
            }
        }
    }
    if out.is_empty() && value.trim().is_empty() {
        sink.add(
            item.line,
            "example-value",
            "write each argument as a string in quotes, with a space between them".to_string(),
        );
    }
    out
}

fn state_value(item: &Item, value: &str, sink: &mut Sink) -> Vec<String> {
    if value == "true" || value == "false" || value.parse::<i64>().is_ok() {
        return Vec::new();
    }
    one_string(item, value, sink).into_iter().collect()
}

fn template_names(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for n in chars.by_ref() {
                    if n == '}' {
                        closed = true;
                        break;
                    }
                    name.push(n);
                }
                if !closed {
                    return Err("a `{` has no closing `}`; write `{{` for a brace".to_string());
                }
                out.push(name);
            }
            '}' => return Err("a `}` has no opening `{`; write `}}` for a brace".to_string()),
            _ => {}
        }
    }
    Ok(out)
}

fn range(name: &str, item: &Item, value: &str, sink: &mut Sink) -> Option<Range> {
    let read = if let Some(bounds) = value.strip_prefix("integers from ") {
        integer_range(name, bounds)
    } else if let Some(bounds) = value.strip_prefix("text of ") {
        text_range(name, bounds)
    } else {
        Err(
            "a range is `integers from <low> to <high>` or `text of <low> to <high> characters from \"<characters>\"`",
        )
    };
    match read {
        Ok(found) => Some(found),
        Err(message) => {
            sink.add(item.line, "case-range", message.to_string());
            None
        }
    }
}

fn integer_range(name: &str, bounds: &str) -> Result<Range, &'static str> {
    let usage = "write `integers from <low> to <high>`";
    let (low, high) = bounds.split_once(" to ").ok_or(usage)?;
    let low = low.trim().parse::<i64>().map_err(|_| usage)?;
    let high = high.trim().parse::<i64>().map_err(|_| usage)?;
    if low > high {
        return Err("the first bound is greater than the second");
    }
    Ok(Range::Integers {
        name: name.to_string(),
        low,
        high,
    })
}

fn text_range(name: &str, bounds: &str) -> Result<Range, &'static str> {
    let usage = "write `text of <low> to <high> characters from \"<characters>\"`";
    let (low, rest) = bounds.split_once(" to ").ok_or(usage)?;
    let (high, alphabet) = rest.split_once(" characters from ").ok_or(usage)?;
    let low = low.trim().parse::<u64>().map_err(|_| usage)?;
    let high = high.trim().parse::<u64>().map_err(|_| usage)?;
    if low > high || high > MAX_TEXT_LENGTH {
        return Err(
            "the lengths are from 0 to 65536, and the first is not greater than the second",
        );
    }
    match literal(alphabet.trim()) {
        Ok((characters, rest)) if !characters.is_empty() && rest.trim().is_empty() => {
            Ok(Range::Text {
                name: name.to_string(),
                low,
                high,
                characters: characters.chars().collect(),
            })
        }
        _ => Err("the characters are one string in quotes, with one character or more"),
    }
}

fn values(key: &Key, item: &Item, value: &str, sink: &mut Sink) -> Vec<String> {
    match key {
        Key::Input
        | Key::File(_)
        | Key::Output
        | Key::ErrorOutput
        | Key::OutputFile(_)
        | Key::TypeInto(_)
        | Key::ExpectText
        | Key::Server(_)
        | Key::Client(_)
        | Key::Answer
        | Key::RequestHeader(_)
        | Key::RequestBody
        | Key::ExpectHeader(_)
        | Key::ExpectBody => one_string(item, value, sink).into_iter().collect(),
        Key::Call => {
            let found = backticked(value).map(str::to_string);
            if found.is_none() {
                sink.add(
                    item.line,
                    "example-value",
                    "write the call in backticks, for example ``- Call: `parse(\"1\")` ``"
                        .to_string(),
                );
            }
            found.into_iter().collect()
        }
        Key::Value => match backticked(value) {
            Some(text) => vec![text.to_string()],
            None => one_string(item, value, sink).into_iter().collect(),
        },
        Key::Request => {
            let found = request_line(value);
            if found.is_none() {
                sink.add(
                    item.line,
                    "example-value",
                    format!(
                        "write the method and the path in backticks, for example ``- Request: `GET /items` ``; the methods are {}",
                        METHODS.join(", ")
                    ),
                );
            }
            found.into_iter().collect()
        }
        Key::Status => {
            if !value.parse::<u16>().is_ok_and(|s| (100..=599).contains(&s)) {
                sink.add(
                    item.line,
                    "example-value",
                    "a status is a number from 100 to 599".to_string(),
                );
            }
            Vec::new()
        }
        Key::Arguments => strings(item, value, sink),
        Key::State(_) | Key::ExpectState(_) => state_value(item, value, sink),
        Key::ExitCode => {
            if value.parse::<u8>().is_err() {
                sink.add(
                    item.line,
                    "example-value",
                    "an exit code is a number from 0 to 255".to_string(),
                );
            }
            Vec::new()
        }
        Key::End => {
            if End::parse(value).is_none() {
                sink.add(
                    item.line,
                    "example-end",
                    "a run ends `exited`, `out of time`, `out of memory` or `trapped`".to_string(),
                );
            }
            Vec::new()
        }
        Key::Click | Key::ExpectLabel => {
            let found = backticked(value).map(str::to_string);
            if found.is_none() {
                sink.add(
                    item.line,
                    "example-value",
                    "write the element in backticks, for example `- Click: `add``".to_string(),
                );
            }
            found.into_iter().collect()
        }
        Key::Range(_) => Vec::new(),
    }
}

fn state(value: &str, found: &[String]) -> StateValue {
    match found.first() {
        Some(text) => StateValue {
            field: String::new(),
            value: text.clone(),
            text: true,
        },
        None => StateValue {
            field: String::new(),
            value: value.to_string(),
            text: false,
        },
    }
}

fn record(key: &Key, value: &str, found: &[String], test: &mut Test) {
    let first = found.first().cloned();
    match key {
        Key::State(field) => test.state.push(StateValue {
            field: field.clone(),
            ..state(value, found)
        }),
        Key::ExpectState(field) => test.expect_state.push(StateValue {
            field: field.clone(),
            ..state(value, found)
        }),
        Key::TypeInto(element) => test
            .typed
            .push((element.clone(), first.unwrap_or_default())),
        Key::Click => test.click = first,
        Key::ExpectText => test.expect_text = first,
        Key::ExpectLabel => test.expect_label = first,
        Key::Input => test.stdin = first,
        Key::Arguments => test.arguments = Some(found.to_vec()),
        Key::File(path) => test.files.push((path.clone(), first.unwrap_or_default())),
        Key::Output => test.stdout = first,
        Key::ErrorOutput => test.stderr = first,
        Key::OutputFile(path) => test
            .expect_files
            .push((path.clone(), first.unwrap_or_default())),
        Key::ExitCode => test.exit_code = value.parse().ok(),
        Key::End => test.end = End::parse(value).unwrap_or_default(),
        Key::Server(at) => test.servers.push((at.clone(), first.unwrap_or_default())),
        Key::Client(at) => {
            if let Some(number) = port(at) {
                test.clients.push((number, first.unwrap_or_default()));
            }
        }
        Key::Answer => test.answer = first,
        Key::Call => test.call = first,
        Key::Value => test.value = first,
        Key::Request => test.request = first,
        Key::RequestHeader(name) => test
            .request_headers
            .push((name.clone(), first.unwrap_or_default())),
        Key::RequestBody => test.request_body = first,
        Key::Status => test.status = value.parse().ok(),
        Key::ExpectHeader(name) => test
            .expect_headers
            .push((name.to_lowercase(), first.unwrap_or_default())),
        Key::ExpectBody => test.expect_body = first,
    }
}

fn key_place(key: &Key, key_text: &str, screen: bool, cases: bool, line: usize, sink: &mut Sink) {
    match key.screen() {
        Some(for_screen) if for_screen != screen => {
            let other = if screen {
                "a program or module idea"
            } else {
                "a screen idea"
            };
            sink.add(
                line,
                "example-key",
                format!("`{key_text}` is a key for {other}"),
            );
        }
        None if !cases => sink.add(
            line,
            "example-range",
            "a range is only in a `### Cases:` block".to_string(),
        ),
        _ => {}
    }
    let path = match key {
        Key::File(p) | Key::OutputFile(p) => Some(p.as_str()),
        _ => None,
    };
    if let Some(path) = path.filter(|p| !relative_path(p)) {
        sink.add(
            line,
            "example-path",
            format!("`{path}` is not a relative path inside the program's folder"),
        );
    }
    let bad_place = match key {
        Key::Server(at) => !address(at),
        Key::Client(at) => port(at).is_none(),
        _ => false,
    };
    if bad_place {
        sink.add(
            line,
            "example-address",
            format!("`{key_text}` names a server as `host:port` and a client by its port, from 1 to 65535"),
        );
    }
}

fn shape_rules(head: usize, keys: &[Key], test: &Test, sink: &mut Sink) {
    let call = keys.contains(&Key::Call);
    let request = keys.contains(&Key::Request);
    let exited = test.end == End::Exited;
    let mut add = |code: &'static str, message: &str| sink.add(head, code, message.to_string());
    if call && request {
        add(
            "example-mixed",
            "a block is a call of a library, a request to a server, or a run of a program, not two of them",
        );
    }
    if call || request {
        if keys.contains(&Key::ExitCode) {
            add(
                "example-exit-code-unused",
                "a call or a request has no exit code; remove `- Expect exit code:`",
            );
        }
    } else if exited && !keys.contains(&Key::ExitCode) {
        add(
            "example-exit-code-missing",
            "this block has no `- Expect exit code:` item",
        );
    }
    if call && exited && !keys.contains(&Key::Value) {
        add(
            "example-value-missing",
            "a call has an `- Expect value:` item, the value as Rust's `{:?}` writes it",
        );
    }
    if keys.contains(&Key::Value) && !call {
        add(
            "example-call-missing",
            "`- Expect value:` goes with a `- Call:` item",
        );
    }
    if request && exited && !keys.contains(&Key::Status) {
        add(
            "example-status-missing",
            "a request has an `- Expect status:` item",
        );
    }
    let http_keys = keys.iter().any(|k| {
        matches!(
            k,
            Key::Status
                | Key::ExpectHeader(_)
                | Key::ExpectBody
                | Key::RequestHeader(_)
                | Key::RequestBody
        )
    });
    if http_keys && !request {
        add(
            "example-request-missing",
            "the keys of a request go with a `- Request:` item",
        );
    }
    if keys.contains(&Key::Answer) && test.clients.len() != 1 {
        add(
            "example-client-missing",
            "`- Expect answer:` goes with one `- Client `port`:` item",
        );
    }
}

fn block(
    kind: IdeaKind,
    cases: bool,
    lines: &[Line],
    head: usize,
    test: &mut Test,
    sink: &mut Sink,
) {
    let screen = kind == IdeaKind::Screen;
    test.screen = screen;
    let mut keys: Vec<Key> = Vec::new();
    let mut ranges: Vec<(String, usize)> = Vec::new();
    let mut texts: Vec<(String, usize)> = Vec::new();
    for item in items(lines, sink) {
        let Some((key_text, value)) = split_key(&item.text) else {
            sink.add(
                item.line,
                "example-item",
                "write each item as `- Key: value`".to_string(),
            );
            continue;
        };
        let Some(key) = Key::parse(key_text) else {
            let list = if screen { SCREEN_KEYS } else { PROGRAM_KEYS };
            sink.add(
                item.line,
                "example-key",
                format!("`{key_text}` is not a key of an example; the keys are {list}"),
            );
            continue;
        };
        key_place(&key, key_text, screen, cases, item.line, sink);
        if keys.contains(&key) {
            let code = if matches!(key, Key::Range(_)) {
                "case-range-duplicate"
            } else {
                "example-key-duplicate"
            };
            sink.add(
                item.line,
                code,
                format!("`{key_text}` is in this block two times"),
            );
        }
        if let Key::Range(name) = &key {
            ranges.push((name.clone(), item.line));
            test.ranges.extend(range(name, &item, value, sink));
        }
        let found = values(&key, &item, value, sink);
        record(&key, value, &found, test);
        texts.extend(found.into_iter().map(|v| (v, item.line)));
        keys.push(key);
    }
    let exited = test.end == End::Exited;
    if !screen {
        shape_rules(head, &keys, test, sink);
    }
    if !screen && !exited && keys.contains(&Key::ExitCode) {
        sink.add(
            head,
            "example-exit-code-end",
            format!(
                "a run that ends `{}` has no exit code; remove `- Expect exit code:`",
                test.end.name()
            ),
        );
    }
    if screen && !keys.iter().any(Key::expects) {
        sink.add(
            head,
            "example-expect-missing",
            "this block has no `- Expect ...:` item".to_string(),
        );
    }
    if cases {
        templates(head, &ranges, &texts, sink);
    }
}

fn templates(head: usize, ranges: &[(String, usize)], texts: &[(String, usize)], sink: &mut Sink) {
    if ranges.is_empty() {
        sink.add(
            head,
            "case-range-missing",
            "a `### Cases:` block has one `- Range `name`:` item or more".to_string(),
        );
    }
    let mut used = BTreeSet::new();
    for (text, line) in texts {
        match template_names(text) {
            Ok(names) => {
                for name in names {
                    if !ranges.iter().any(|(n, _)| *n == name) {
                        sink.add(
                            *line,
                            "case-variable-unknown",
                            format!("`{{{name}}}` names no range of this block"),
                        );
                    }
                    used.insert(name);
                }
            }
            Err(message) => sink.add(*line, "case-template", message),
        }
    }
    for (name, line) in ranges {
        if !used.contains(name) {
            sink.add(
                *line,
                "case-range-unused",
                format!(
                    "the range `{name}` is not used in a value; write `{{{name}}}` where it goes"
                ),
            );
        }
    }
}

fn statement(head: usize, lines: &[Line], dictionary: &Dictionary, sink: &mut Sink) {
    let mut clauses: Vec<(&str, usize, Vec<Token>)> = Vec::new();
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        let keyword = KEYWORDS
            .iter()
            .find(|k| text.strip_prefix(**k).is_some_and(|r| r.starts_with(' ')));
        if let Some(keyword) = keyword {
            let mut tokens = Vec::new();
            tokenize(&text[keyword.len()..], line.number, &mut tokens);
            clauses.push((*keyword, line.number, tokens));
        } else if let Some(last) = clauses.last_mut() {
            tokenize(text, line.number, &mut last.2);
        } else {
            sink.add(
                line.number,
                "behaviour-text",
                "a behaviour starts with a `Given` line".to_string(),
            );
        }
    }
    let mut stage = 0;
    let mut seen = [false; 3];
    for (keyword, line, tokens) in &clauses {
        dictionary.description(tokens, sink);
        let want = match *keyword {
            "Given" => 1,
            "When" => 2,
            "Then" => 3,
            _ => 0,
        };
        if want == 0 {
            if stage == 0 {
                sink.add(
                    *line,
                    "behaviour-order",
                    "`And` comes after a `Given`, `When` or `Then` line".to_string(),
                );
            }
            continue;
        }
        if want <= stage {
            sink.add(
                *line,
                "behaviour-order",
                format!(
                    "`{keyword}` is out of order; write `Given`, `When` and `Then` one time each, in that order, and `And` to add to one"
                ),
            );
        }
        seen[want - 1] = true;
        stage = stage.max(want);
    }
    let missing = [
        "behaviour-given-missing",
        "behaviour-when-missing",
        "behaviour-then-missing",
    ];
    for ((code, keyword), seen) in missing.into_iter().zip(KEYWORDS).zip(seen) {
        if !seen {
            sink.add(
                head,
                code,
                format!("this behaviour has no `{keyword}` line"),
            );
        }
    }
}

fn name_words(line: &Line, text: &str, dictionary: &Dictionary, sink: &mut Sink) {
    let mut tokens = Vec::new();
    tokenize(text, line.number, &mut tokens);
    dictionary.words(&tokens, sink);
}

pub(crate) fn parse(
    kind: IdeaKind,
    heading: usize,
    lines: &[Line],
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Vec<Test> {
    let mut tests = Vec::new();
    let (before, behaviours) = split_headings(lines, "## ");
    for line in before.iter().filter(|l| !l.text.trim().is_empty()) {
        sink.add(
            line.number,
            "behaviour-text",
            "each behaviour starts with a `## name` heading".to_string(),
        );
    }
    if behaviours.is_empty() {
        sink.add(
            heading,
            "behaviours-empty",
            "the idea has no behaviours; write one `## name` with Given, When and Then".to_string(),
        );
    }
    let mut names: Vec<String> = Vec::new();
    for behaviour in behaviours {
        let head = behaviour.head;
        let name = head.text[3..].trim();
        if name.is_empty() {
            sink.add(
                head.number,
                "behaviour-name-empty",
                "the behaviour has no name".to_string(),
            );
        } else if names.contains(&name.to_lowercase()) {
            sink.add(
                head.number,
                "behaviour-duplicate",
                format!("two behaviours have the name `{name}`"),
            );
        }
        names.push(name.to_lowercase());
        name_words(head, name, dictionary, sink);
        let (said, blocks) = split_headings(behaviour.body, "### ");
        statement(head.number, said, dictionary, sink);
        if blocks.is_empty() {
            sink.add(
                head.number,
                "behaviour-example-missing",
                "this behaviour has no `### Example:` and no `### Cases:` block".to_string(),
            );
        }
        for part in blocks {
            let title = part.head.text[4..].trim();
            let (cases, label) = if let Some(label) = title.strip_prefix("Example:") {
                (false, label)
            } else if let Some(label) = title.strip_prefix("Cases:") {
                (true, label)
            } else {
                sink.add(
                    part.head.number,
                    "example-heading",
                    format!("`### {title}` is not `### Example: name` or `### Cases: name`"),
                );
                continue;
            };
            if label.trim().is_empty() {
                sink.add(
                    part.head.number,
                    "example-name-empty",
                    "the block has no name".to_string(),
                );
            }
            name_words(part.head, label, dictionary, sink);
            let mut test = Test {
                file: sink.file.clone(),
                line: part.head.number,
                behaviour: name.to_string(),
                name: label.trim().to_string(),
                ..Test::default()
            };
            block(kind, cases, part.body, part.head.number, &mut test, sink);
            tests.push(test);
        }
    }
    tests
}

#[cfg(test)]
mod tests {
    use super::{parse, template_names};
    use crate::Sink;
    use crate::idea::IdeaKind;
    use crate::ste::Dictionary;
    use crate::text::lines;

    fn codes(kind: IdeaKind, text: &str) -> Vec<&'static str> {
        let mut sink = Sink::new("idea.md");
        parse(kind, 1, &lines(text, 2), &Dictionary::load(), &mut sink);
        sink.findings.iter().map(|f| f.code).collect()
    }

    const GOOD: &str = "## Count the words\n\nGiven a text with two words,\nAnd the text is short,\nWhen the program runs,\nThen it writes 2.\n\n### Example: two words\n- Standard input: \"a b\\n\"\n- Arguments: \"-w\" \"--all\"\n- File `in/a.txt`: \"x\"\n- Expect standard output:\n  ```\n  2\n  ```\n- Expect exit code: 0\n- Expect file `out.txt`: \"\"\n\n### Cases: one word\n- Range `w`: text of 1 to 8 characters from \"abc\"\n- Range `n`: integers from 0 to 9\n- Standard input: \"{w}\"\n- Arguments: \"{n}\"\n- Expect standard output: \"1\\n\"\n- Expect exit code: 0\n";

    #[test]
    fn a_good_behaviour_has_no_findings() {
        assert_eq!(codes(IdeaKind::Program, GOOD), Vec::<&str>::new());
    }

    #[test]
    fn a_missing_then_and_an_example_are_findings() {
        let text = "## Count\nGiven a text,\nWhen the program runs.\n";
        assert_eq!(
            codes(IdeaKind::Program, text),
            ["behaviour-then-missing", "behaviour-example-missing"]
        );
    }

    #[test]
    fn keywords_out_of_order_are_findings() {
        let text = "## Count\nWhen the program runs,\nGiven a text,\nThen it stops.\n### Example: a\n- Expect exit code: 0\n";
        assert_eq!(codes(IdeaKind::Program, text), ["behaviour-order"]);
    }

    #[test]
    fn bad_items_are_findings() {
        let text = "## Count\nGiven a text,\nWhen the program runs,\nThen it stops.\n### Example: a\n- Standard input: \"open\n- Standard input: \"a\" \"b\"\n- Colour: red\n- Click: `b`\n- File `/etc/x`: \"\"\n- Range `n`: integers from 1 to 2\nloose text\n";
        let found = codes(IdeaKind::Program, text);
        for code in [
            "example-string",
            "example-value",
            "example-key-duplicate",
            "example-key",
            "example-path",
            "example-range",
            "example-text",
            "example-exit-code-missing",
        ] {
            assert!(found.contains(&code), "{code} in {found:?}");
        }
    }

    #[test]
    fn bad_cases_are_findings() {
        let text = "## Count\nGiven a text,\nWhen the program runs,\nThen it stops.\n### Cases: a\n- Range `n`: integers from 9 to 1\n- Range `m`: text of 1 to 2 characters from \"\"\n- Standard input: \"{x}\"\n- Expect exit code: 0\n### Cases: b\n- Standard input: \"{\"\n- Expect exit code: 0\n### Other: c\n";
        let found = codes(IdeaKind::Program, text);
        for code in [
            "case-range",
            "case-variable-unknown",
            "case-range-unused",
            "case-range-missing",
            "case-template",
            "example-heading",
        ] {
            assert!(found.contains(&code), "{code} in {found:?}");
        }
    }

    #[test]
    fn screen_examples_use_screen_keys() {
        let text = "## Add\nGiven an empty basket,\nWhen the user clicks the button,\nThen the basket shows one item.\n### Example: one\n- State `items`: 0\n- Click: `add`\n- Type into `name`: \"tea\"\n- Expect state `items`: 1\n- Expect text: \"1 item\"\n";
        assert_eq!(codes(IdeaKind::Screen, text), Vec::<&str>::new());
        assert!(codes(IdeaKind::Program, text).contains(&"example-key"));
    }

    #[test]
    fn templates_read_names_and_braces() {
        assert_eq!(
            template_names("{a} {{b}} {c}").expect("a template"),
            ["a", "c"]
        );
        assert!(template_names("{a").is_err());
        assert!(template_names("a}").is_err());
    }

    #[test]
    fn the_end_of_a_run_is_a_key_and_an_exit_code_goes_with_exited_only() {
        let base = "## Stop\nGiven no input,\nWhen the program runs,\nThen the host stops it.\n### Example: a\n";
        let with = |items: &str| codes(IdeaKind::Program, &format!("{base}{items}"));
        assert_eq!(with("- Expect end: out of time\n"), Vec::<&str>::new());
        assert_eq!(
            with("- Expect end: out of time\n- Expect exit code: 1\n"),
            ["example-exit-code-end"]
        );
        assert_eq!(
            with("- Expect end: exited\n"),
            ["example-exit-code-missing"]
        );
        assert_eq!(
            with("- Expect end: later\n"),
            ["example-end", "example-exit-code-missing"]
        );
    }

    #[test]
    fn each_block_is_a_test_with_its_input_and_its_expectations() {
        let mut sink = Sink::new("idea.md");
        let tests = parse(
            IdeaKind::Program,
            1,
            &lines(GOOD, 2),
            &Dictionary::load(),
            &mut sink,
        );
        assert_eq!(sink.findings, []);
        assert_eq!(tests.len(), 2);
        let first = &tests[0];
        assert_eq!(first.file, "idea.md");
        assert_eq!(first.behaviour, "Count the words");
        assert_eq!(first.name, "two words");
        assert_eq!(first.line, 9);
        assert_eq!(first.stdin.as_deref(), Some("a b\n"));
        assert_eq!(
            first.arguments,
            Some(vec!["-w".to_string(), "--all".to_string()])
        );
        assert_eq!(first.files, [("in/a.txt".to_string(), "x".to_string())]);
        assert_eq!(first.stdout.as_deref(), Some("2\n"));
        assert_eq!(first.exit_code, Some(0));
        assert_eq!(first.expect_files, [("out.txt".to_string(), String::new())]);
        assert_eq!(tests[1].ranges.len(), 2);
        assert_eq!(tests[1].arguments, Some(vec!["{n}".to_string()]));
    }

    #[test]
    fn calls_requests_and_network_fixtures_are_examples() {
        let base = "## Use it\nGiven a value,\nWhen the program runs,\nThen it gives the result.\n";
        let call = format!(
            "{base}### Example: a call\n- Call: `parse(\"[1]\")`\n- Expect value: `Ok(List([Number(\"1\")]))`\n"
        );
        let request = format!(
            "{base}### Example: a request\n- Request: `POST /items?a=1`\n- Request header `Content-Type`: \"text/plain\"\n- Request body: \"tea\"\n- Expect status: 201\n- Expect header `Content-Type`: \"application/json\"\n- Expect body: \"{{}}\"\n"
        );
        let network = format!(
            "{base}### Example: a server and a client\n- Server `127.0.0.1:7878`: \"pong\"\n- Client `7879`: \"ping\"\n- Expect answer: \"PING\"\n- Expect exit code: 0\n"
        );
        for text in [&call, &request, &network] {
            assert_eq!(codes(IdeaKind::Program, text), Vec::<&str>::new(), "{text}");
        }
        let mut sink = Sink::new("idea.md");
        let tests = parse(
            IdeaKind::Program,
            1,
            &lines(&format!("{call}{}", &request[base.len()..]), 2),
            &Dictionary::load(),
            &mut sink,
        );
        assert_eq!(sink.findings, []);
        assert_eq!(tests[0].call.as_deref(), Some("parse(\"[1]\")"));
        assert_eq!(tests[0].value.as_deref(), Some("Ok(List([Number(\"1\")]))"));
        assert_eq!(tests[1].request.as_deref(), Some("POST /items?a=1"));
        assert_eq!(tests[1].status, Some(201));
        assert_eq!(
            tests[1].expect_headers,
            [("content-type".to_string(), "application/json".to_string())]
        );
        assert_eq!(tests[1].expect_body.as_deref(), Some("{}"));
    }

    #[test]
    fn a_call_or_a_request_has_its_expectation_and_no_exit_code() {
        let base = "## Use it\nGiven a value,\nWhen the program runs,\nThen it gives the result.\n### Example: a\n";
        let with = |items: &str| codes(IdeaKind::Module, &format!("{base}{items}"));
        assert_eq!(with("- Call: `f()`\n"), ["example-value-missing"]);
        assert_eq!(
            with("- Call: `f()`\n- Expect value: `1`\n- Expect exit code: 0\n"),
            ["example-exit-code-unused"]
        );
        assert_eq!(
            with("- Expect value: `1`\n- Expect exit code: 0\n"),
            ["example-call-missing"]
        );
        assert_eq!(with("- Request: `GET /`\n"), ["example-status-missing"]);
        assert_eq!(
            with("- Request: `FETCH /`\n- Expect status: 200\n"),
            ["example-value"]
        );
        assert_eq!(
            with("- Expect status: 200\n- Expect exit code: 0\n"),
            ["example-request-missing"]
        );
        assert_eq!(
            with("- Call: `f()`\n- Expect value: `1`\n- Request: `GET /`\n- Expect status: 200\n"),
            ["example-mixed"]
        );
        assert_eq!(
            with("- Server `host`: \"a\"\n- Expect exit code: 0\n"),
            ["example-address"]
        );
        assert_eq!(
            with("- Expect answer: \"a\"\n- Expect exit code: 0\n"),
            ["example-client-missing"]
        );
    }
}
