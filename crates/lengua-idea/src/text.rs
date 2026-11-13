#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Word,
    Code,
    Literal,
    Number,
    Stop,
    Comma,
    Mark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: Kind,
    pub text: String,
    pub line: usize,
}

impl Token {
    pub(crate) fn counts(&self) -> bool {
        matches!(
            self.kind,
            Kind::Word | Kind::Code | Kind::Literal | Kind::Number
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub number: usize,
    pub text: String,
}

pub(crate) fn lines(body: &str, first: usize) -> Vec<Line> {
    body.lines()
        .enumerate()
        .map(|(i, text)| Line {
            number: first + i,
            text: text.to_string(),
        })
        .collect()
}

pub(crate) fn is_fence(text: &str) -> bool {
    text.trim_start().starts_with("```")
}

pub(crate) struct Part<'l> {
    pub head: &'l Line,
    pub body: &'l [Line],
}

pub(crate) fn split_headings<'l>(lines: &'l [Line], prefix: &str) -> (&'l [Line], Vec<Part<'l>>) {
    let mut starts = Vec::new();
    let mut fence = false;
    for (i, line) in lines.iter().enumerate() {
        if is_fence(&line.text) {
            fence = !fence;
        } else if !fence && line.text.starts_with(prefix) {
            starts.push(i);
        }
    }
    let first = starts.first().copied().unwrap_or(lines.len());
    let mut parts = Vec::new();
    for (k, &i) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(lines.len());
        parts.push(Part {
            head: &lines[i],
            body: &lines[i + 1..end],
        });
    }
    (&lines[..first], parts)
}

pub(crate) fn tokenize(text: &str, line: usize, out: &mut Vec<Token>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        let kind = if c == '`' {
            i = close(&chars, i + 1, '`');
            Kind::Code
        } else if c == '"' {
            i = close(&chars, i + 1, '"');
            Kind::Literal
        } else if c == '\u{201c}' {
            i = close(&chars, i + 1, '\u{201d}');
            Kind::Literal
        } else if c.is_ascii_digit() {
            i = number_end(&chars, i);
            Kind::Number
        } else if c.is_alphabetic() || c == '_' {
            i = word_end(&chars, i);
            Kind::Word
        } else if c == '.' || c == '?' || c == '!' {
            i += 1;
            Kind::Stop
        } else if c == ',' {
            i += 1;
            Kind::Comma
        } else if c.is_whitespace() {
            i += 1;
            continue;
        } else {
            i += 1;
            Kind::Mark
        };
        out.push(Token {
            kind,
            text: chars[start..i].iter().collect(),
            line,
        });
    }
}

fn close(chars: &[char], from: usize, end: char) -> usize {
    chars[from..]
        .iter()
        .position(|&c| c == end)
        .map_or(chars.len(), |p| from + p + 1)
}

fn number_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        let joined =
            (c == '.' || c == ',') && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if c.is_alphanumeric() || c == '_' || joined {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn word_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        let joiner = (c == '-' || c == '\'' || c == '\u{2019}')
            && chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
        if c.is_alphanumeric() || c == '_' || joiner {
            i += 1;
        } else {
            break;
        }
    }
    i
}

pub(crate) fn sentences(tokens: &[Token]) -> Vec<&[Token]> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Stop {
            out.push(&tokens[start..=i]);
            start = i + 1;
        }
    }
    if start < tokens.len() {
        out.push(&tokens[start..]);
    }
    out.retain(|s| s.iter().any(Token::counts));
    out
}

pub(crate) fn join(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut open = false;
    for token in tokens {
        let tight = matches!(token.kind, Kind::Stop | Kind::Comma)
            || (token.kind == Kind::Mark && matches!(token.text.as_str(), ":" | ";" | ")"));
        if !out.is_empty() && !tight && !open {
            out.push(' ');
        }
        out.push_str(&token.text);
        open = token.kind == Kind::Mark && token.text == "(";
    }
    out
}

pub(crate) fn backticked(text: &str) -> Option<&str> {
    let inner = text.trim().strip_prefix('`')?.strip_suffix('`')?;
    (!inner.is_empty() && !inner.contains('`')).then_some(inner)
}

pub(crate) fn split_key(text: &str) -> Option<(&str, &str)> {
    let mut code = false;
    for (i, c) in text.char_indices() {
        if c == '`' {
            code = !code;
        } else if c == ':' && !code {
            return Some((text[..i].trim(), text[i + 1..].trim()));
        }
    }
    None
}

pub(crate) fn snake(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub(crate) fn upper_camel(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

pub(crate) fn relative_path(path: &str) -> bool {
    let path = path.strip_suffix('/').unwrap_or(path);
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

pub(crate) fn literal(text: &str) -> Result<(String, &str), String> {
    let mut chars = text.char_indices();
    if !matches!(chars.next(), Some((_, '"'))) {
        return Err("a string starts with `\"`".to_string());
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &text[i + 1..])),
            '\\' => {
                let escape = chars.next().map(|(_, e)| e);
                match escape {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('0') => out.push('\0'),
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some('u') => {
                        let rest: String = chars.by_ref().map(|(_, h)| h).take_while(|&h| h != '}').collect();
                        let code = rest
                            .strip_prefix('{')
                            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                            .and_then(char::from_u32)
                            .ok_or_else(|| "write a character code as `\\u{hex}`".to_string())?;
                        out.push(code);
                    }
                    Some(other) => {
                        return Err(format!(
                            "`\\{other}` is not an escape; use `\\n`, `\\t`, `\\r`, `\\0`, `\\\\`, `\\\"` or `\\u{{hex}}`"
                        ));
                    }
                    None => return Err("the string stops after `\\`".to_string()),
                }
            }
            _ => out.push(c),
        }
    }
    Err("the string has no closing `\"`".to_string())
}

#[cfg(test)]
mod tests {
    use super::{Kind, join, literal, relative_path, sentences, split_key, tokenize};

    fn tokens(text: &str) -> Vec<super::Token> {
        let mut out = Vec::new();
        tokenize(text, 1, &mut out);
        out
    }

    #[test]
    fn code_literals_and_numbers_are_one_token_each() {
        let t = tokens("Read `a.b c` and \"x. y\" in 1.5 s.");
        let kinds: Vec<Kind> = t.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Word,
                Kind::Code,
                Kind::Word,
                Kind::Literal,
                Kind::Word,
                Kind::Number,
                Kind::Word,
                Kind::Stop
            ]
        );
    }

    #[test]
    fn sentences_split_at_full_stops_only_outside_code() {
        let t = tokens("It reads `a.b`. It writes 2.5 words. Then");
        assert_eq!(sentences(&t).len(), 3);
    }

    #[test]
    fn join_puts_back_the_spaces() {
        let t = tokens("It reads `x`, then stops.");
        assert_eq!(join(&t), "It reads `x`, then stops.");
    }

    #[test]
    fn literal_reads_escapes() {
        let (value, rest) = literal(r#""a\n\t\"\\\u{41}" tail"#).expect("a string");
        assert_eq!(value, "a\n\t\"\\A");
        assert_eq!(rest, " tail");
        assert!(literal(r#""a\q""#).is_err());
        assert!(literal(r#""open"#).is_err());
    }

    #[test]
    fn key_split_ignores_colons_in_code() {
        assert_eq!(split_key("File `a:b`: \"x\""), Some(("File `a:b`", "\"x\"")));
        assert_eq!(split_key("no colon"), None);
    }

    #[test]
    fn relative_paths_stay_inside() {
        assert!(relative_path("data/in.txt"));
        assert!(relative_path("out/"));
        assert!(!relative_path("/etc"));
        assert!(!relative_path("../x"));
        assert!(!relative_path("c:\\x"));
    }
}
