use crate::{Error, Value};

const MAX_DEPTH: usize = 512;

enum Token {
    Open(char),
    Close(char),
    Comma,
    Colon,
    Scalar(Value),
}

enum Frame {
    List(Vec<Value>),
    Object(Vec<(String, Value)>, Option<String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    Value,
    ValueOrClose,
    Key,
    KeyOrClose,
    Colon,
    CommaOrClose,
    Nothing,
}

pub fn parse(text: &str) -> Result<Value, Error> {
    let chars: Vec<char> = text.chars().collect();
    let tokens = tokens(&chars)?;
    debug_assert!(tokens.len() <= chars.len());
    debug_assert!(tokens.iter().all(|(at, _)| *at < chars.len()));
    assemble(&tokens)
}

fn tokens(chars: &[char]) -> Result<Vec<(usize, Token)>, Error> {
    let mut out = Vec::new();
    let mut at = 0;
    for _ in 0..chars.len() {
        let Some(&character) = chars.get(at) else {
            break;
        };
        let (token, next) = match character {
            ' ' | '\t' | '\n' | '\r' => {
                at += 1;
                continue;
            }
            '[' | '{' => (Token::Open(character), at + 1),
            ']' | '}' => (Token::Close(character), at + 1),
            ',' => (Token::Comma, at + 1),
            ':' => (Token::Colon, at + 1),
            '"' => {
                let (text, next) = string_at(chars, at)?;
                (Token::Scalar(Value::Text(text)), next)
            }
            '-' | '0'..='9' => {
                let (number, next) = number_at(chars, at)?;
                (Token::Scalar(Value::Number(number)), next)
            }
            _ => word_at(chars, at)?,
        };
        debug_assert!(next > at);
        out.push((at, token));
        at = next;
    }
    debug_assert!(at >= chars.len());
    Ok(out)
}

fn word_at(chars: &[char], at: usize) -> Result<(Token, usize), Error> {
    let words = [
        ("null", Value::Null),
        ("true", Value::Bool(true)),
        ("false", Value::Bool(false)),
    ];
    debug_assert!(at < chars.len());
    for (word, value) in words {
        let length = word.len();
        let found: String = chars
            .get(at..at + length)
            .map(|part| part.iter().collect())
            .unwrap_or_default();
        debug_assert!(length <= 5);
        if found == word {
            return Ok((Token::Scalar(value), at + length));
        }
    }
    Err(Error::Unexpected(at))
}

fn digits(chars: &[char], from: usize) -> usize {
    let count = chars
        .get(from..)
        .unwrap_or_default()
        .iter()
        .take_while(|character| character.is_ascii_digit())
        .count();
    debug_assert!(count <= chars.len());
    debug_assert!(count == 0 || from < chars.len());
    count
}

fn number_at(chars: &[char], at: usize) -> Result<(String, usize), Error> {
    let mut end = at + usize::from(chars.get(at) == Some(&'-'));
    let whole = digits(chars, end);
    if whole == 0 || (whole > 1 && chars.get(end) == Some(&'0')) {
        return Err(Error::Number(at));
    }
    end += whole;
    if chars.get(end) == Some(&'.') {
        let fraction = digits(chars, end + 1);
        if fraction == 0 {
            return Err(Error::Number(at));
        }
        end += fraction + 1;
    }
    if matches!(chars.get(end), Some('e' | 'E')) {
        end += 1 + usize::from(matches!(chars.get(end + 1), Some('+' | '-')));
        let exponent = digits(chars, end);
        if exponent == 0 {
            return Err(Error::Number(at));
        }
        end += exponent;
    }
    debug_assert!(end > at);
    debug_assert!(end <= chars.len());
    Ok((chars.get(at..end).unwrap_or_default().iter().collect(), end))
}

fn string_at(chars: &[char], at: usize) -> Result<(String, usize), Error> {
    let mut out = String::new();
    let mut next = at + 1;
    for _ in 0..chars.len() {
        let character = *chars.get(next).ok_or(Error::End)?;
        match character {
            '"' => return Ok((out, next + 1)),
            '\\' => {
                let (decoded, after) = escape_at(chars, next)?;
                out.push(decoded);
                next = after;
            }
            control if u32::from(control) < 0x20 => return Err(Error::Unexpected(next)),
            other => {
                out.push(other);
                next += 1;
            }
        }
        debug_assert!(next <= chars.len());
    }
    debug_assert!(next >= at);
    Err(Error::End)
}

fn hex4(chars: &[char], from: usize) -> Option<u32> {
    let text: String = chars.get(from..from + 4)?.iter().collect();
    let valid = text.chars().all(|character| character.is_ascii_hexdigit());
    debug_assert_eq!(text.chars().count(), 4);
    debug_assert!(from >= 2);
    u32::from_str_radix(&text, 16).ok().filter(|_| valid)
}

fn escape_at(chars: &[char], at: usize) -> Result<(char, usize), Error> {
    let kind = *chars.get(at + 1).ok_or(Error::End)?;
    let simple = match kind {
        '"' | '\\' | '/' => Some(kind),
        'b' => Some('\u{8}'),
        'f' => Some('\u{c}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        _ => None,
    };
    if let Some(found) = simple {
        return Ok((found, at + 2));
    }
    if kind != 'u' {
        return Err(Error::Escape(at));
    }
    let first = hex4(chars, at + 2).ok_or(Error::Escape(at))?;
    debug_assert!(first <= 0xffff);
    if (0xd800..0xdc00).contains(&first) {
        let marked = chars.get(at + 6) == Some(&'\\') && chars.get(at + 7) == Some(&'u');
        let low = hex4(chars, at + 8)
            .filter(|low| marked && (0xdc00..0xe000).contains(low))
            .ok_or(Error::Escape(at))?;
        let code = 0x1_0000 + ((first - 0xd800) << 10) + (low - 0xdc00);
        return char::from_u32(code)
            .map(|found| (found, at + 12))
            .ok_or(Error::Escape(at));
    }
    debug_assert_eq!(kind, 'u');
    char::from_u32(first)
        .map(|found| (found, at + 6))
        .ok_or(Error::Escape(at))
}

fn place(stack: &mut [Frame], done: &mut Option<Value>, value: Value) -> Want {
    let depth = stack.len();
    let want = match stack.last_mut() {
        None => {
            *done = Some(value);
            Want::Nothing
        }
        Some(Frame::List(items)) => {
            items.push(value);
            Want::CommaOrClose
        }
        Some(Frame::Object(fields, key)) => {
            fields.push((key.take().unwrap_or_default(), value));
            Want::CommaOrClose
        }
    };
    debug_assert_eq!(stack.len(), depth);
    debug_assert!(want != Want::Nothing || depth == 0);
    want
}

fn close(
    stack: &mut Vec<Frame>,
    done: &mut Option<Value>,
    bracket: char,
    at: usize,
) -> Result<Want, Error> {
    let depth = stack.len();
    let value = match (stack.pop(), bracket) {
        (Some(Frame::List(items)), ']') => Value::List(items),
        (Some(Frame::Object(fields, None)), '}') => Value::Object(fields),
        _ => return Err(Error::Unexpected(at)),
    };
    debug_assert_eq!(stack.len() + 1, depth);
    debug_assert!(bracket == ']' || bracket == '}');
    Ok(place(stack, done, value))
}

fn assemble(tokens: &[(usize, Token)]) -> Result<Value, Error> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut done: Option<Value> = None;
    let mut want = Want::Value;
    for (at, token) in tokens {
        want = match (want, token) {
            (Want::Value | Want::ValueOrClose, Token::Open(bracket)) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::Deep);
                }
                if *bracket == '[' {
                    stack.push(Frame::List(Vec::new()));
                    Want::ValueOrClose
                } else {
                    stack.push(Frame::Object(Vec::new(), None));
                    Want::KeyOrClose
                }
            }
            (Want::Value | Want::ValueOrClose, Token::Scalar(value)) => {
                place(&mut stack, &mut done, value.clone())
            }
            (Want::Key | Want::KeyOrClose, Token::Scalar(Value::Text(name))) => {
                if let Some(Frame::Object(_, key)) = stack.last_mut() {
                    *key = Some(name.clone());
                }
                Want::Colon
            }
            (Want::Colon, Token::Colon) => Want::Value,
            (Want::CommaOrClose, Token::Comma) => match stack.last() {
                Some(Frame::List(_)) => Want::Value,
                _ => Want::Key,
            },
            (Want::ValueOrClose | Want::KeyOrClose | Want::CommaOrClose, Token::Close(bracket)) => {
                close(&mut stack, &mut done, *bracket, *at)?
            }
            _ => return Err(Error::Unexpected(*at)),
        };
    }
    debug_assert!(want != Want::Nothing || stack.is_empty());
    debug_assert!(done.is_none() || want == Want::Nothing);
    done.ok_or(Error::End)
}
