use std::collections::BTreeSet;

use crate::Sink;
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{
    Line, Token, backticked, is_fence, literal, relative_path, split_headings, split_key, tokenize,
};

const KEYWORDS: [&str; 4] = ["Given", "When", "Then", "And"];
const PROGRAM_KEYS: &str = "`Standard input`, `Arguments`, `File `path``, `Expect standard output`, `Expect standard error`, `Expect exit code`, `Expect file `path``";
const SCREEN_KEYS: &str =
    "`State `field``, `Click`, `Type into `element``, `Expect state `field``, `Expect text`";
pub(crate) const MAX_TEXT_LENGTH: u64 = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Key {
    Input,
    Arguments,
    File(String),
    Output,
    ErrorOutput,
    ExitCode,
    OutputFile(String),
    State(String),
    Click,
    TypeInto(String),
    ExpectState(String),
    ExpectText,
    Range(String),
}

impl Key {
    fn parse(text: &str) -> Option<Self> {
        let named = |prefix: &str| {
            text.strip_prefix(prefix)
                .and_then(backticked)
                .map(str::to_string)
        };
        Some(match text {
            "Standard input" => Self::Input,
            "Arguments" => Self::Arguments,
            "Expect standard output" => Self::Output,
            "Expect standard error" => Self::ErrorOutput,
            "Expect exit code" => Self::ExitCode,
            "Click" => Self::Click,
            "Expect text" => Self::ExpectText,
            _ => {
                if let Some(path) = named("Expect file ") {
                    Self::OutputFile(path)
                } else if let Some(path) = named("File ") {
                    Self::File(path)
                } else if let Some(field) = named("Expect state ") {
                    Self::ExpectState(field)
                } else if let Some(field) = named("State ") {
                    Self::State(field)
                } else if let Some(element) = named("Type into ") {
                    Self::TypeInto(element)
                } else {
                    Self::Range(named("Range ")?)
                }
            }
        })
    }

    fn screen(&self) -> Option<bool> {
        match self {
            Self::Input
            | Self::Arguments
            | Self::File(_)
            | Self::Output
            | Self::ErrorOutput
            | Self::ExitCode
            | Self::OutputFile(_) => Some(false),
            Self::State(_)
            | Self::Click
            | Self::TypeInto(_)
            | Self::ExpectState(_)
            | Self::ExpectText => Some(true),
            Self::Range(_) => None,
        }
    }

    fn expects(&self) -> bool {
        matches!(
            self,
            Self::Output
                | Self::ErrorOutput
                | Self::ExitCode
                | Self::OutputFile(_)
                | Self::ExpectState(_)
                | Self::ExpectText
        )
    }
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

fn range(item: &Item, value: &str, sink: &mut Sink) {
    let problem = if let Some(bounds) = value.strip_prefix("integers from ") {
        integer_range(bounds)
    } else if let Some(bounds) = value.strip_prefix("text of ") {
        text_range(bounds)
    } else {
        Some(
            "a range is `integers from <low> to <high>` or `text of <low> to <high> characters from \"<characters>\"`",
        )
    };
    if let Some(message) = problem {
        sink.add(item.line, "case-range", message.to_string());
    }
}

fn integer_range(bounds: &str) -> Option<&'static str> {
    let usage = "write `integers from <low> to <high>`";
    let Some((low, high)) = bounds.split_once(" to ") else {
        return Some(usage);
    };
    let parsed = low
        .trim()
        .parse::<i64>()
        .ok()
        .zip(high.trim().parse::<i64>().ok());
    let Some((low, high)) = parsed else {
        return Some(usage);
    };
    (low > high).then_some("the first bound is greater than the second")
}

fn text_range(bounds: &str) -> Option<&'static str> {
    let usage = "write `text of <low> to <high> characters from \"<characters>\"`";
    let Some((low, rest)) = bounds.split_once(" to ") else {
        return Some(usage);
    };
    let Some((high, alphabet)) = rest.split_once(" characters from ") else {
        return Some(usage);
    };
    let parsed = low
        .trim()
        .parse::<u64>()
        .ok()
        .zip(high.trim().parse::<u64>().ok());
    let Some((low, high)) = parsed else {
        return Some(usage);
    };
    if low > high || high > MAX_TEXT_LENGTH {
        return Some(
            "the lengths are from 0 to 65536, and the first is not greater than the second",
        );
    }
    let good = matches!(literal(alphabet.trim()), Ok((chars, rest)) if !chars.is_empty() && rest.trim().is_empty());
    (!good).then_some("the characters are one string in quotes, with one character or more")
}

fn values(key: &Key, item: &Item, value: &str, sink: &mut Sink) -> Vec<String> {
    match key {
        Key::Input
        | Key::File(_)
        | Key::Output
        | Key::ErrorOutput
        | Key::OutputFile(_)
        | Key::TypeInto(_)
        | Key::ExpectText => one_string(item, value, sink).into_iter().collect(),
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
        Key::Click => {
            if backticked(value).is_none() {
                sink.add(
                    item.line,
                    "example-value",
                    "write the element in backticks, for example `- Click: `add``".to_string(),
                );
            }
            Vec::new()
        }
        Key::Range(_) => {
            range(item, value, sink);
            Vec::new()
        }
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
}

fn block(kind: IdeaKind, cases: bool, lines: &[Line], head: usize, sink: &mut Sink) {
    let screen = kind == IdeaKind::Screen;
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
        }
        let found = values(&key, &item, value, sink);
        texts.extend(found.into_iter().map(|v| (v, item.line)));
        keys.push(key);
    }
    if !screen && !keys.contains(&Key::ExitCode) {
        sink.add(
            head,
            "example-exit-code-missing",
            "this block has no `- Expect exit code:` item".to_string(),
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
) {
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
            block(kind, cases, part.body, part.head.number, sink);
        }
    }
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
}
