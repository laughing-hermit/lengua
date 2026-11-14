use std::fmt::Write as _;

const DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn at(&self, index: usize) -> Option<&Value> {
        match self {
            Self::List(items) => items.get(index),
            _ => None,
        }
    }

    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn number(&self) -> Option<u64> {
        match self {
            Self::Number(text) => text.parse().ok(),
            _ => None,
        }
    }

    pub(crate) fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Number(text) => out.push_str(text),
            Self::Text(text) => out.push_str(&quote(text)),
            Self::List(items) => {
                out.push('[');
                for (k, item) in items.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Self::Object(fields) => {
                out.push('{');
                for (k, (key, item)) in fields.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    out.push_str(&quote(key));
                    out.push(':');
                    item.write(out);
                }
                out.push('}');
            }
        }
    }
}

pub(crate) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Reader<'t> {
    bytes: &'t [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn word(&mut self, word: &str, value: Value) -> Option<Value> {
        let end = self.at + word.len();
        (self.bytes.get(self.at..end)? == word.as_bytes()).then(|| {
            self.at = end;
            value
        })
    }

    fn hex(&mut self) -> Option<u32> {
        let digits = std::str::from_utf8(self.bytes.get(self.at..self.at + 4)?).ok()?;
        self.at += 4;
        u32::from_str_radix(digits, 16).ok()
    }

    fn text(&mut self) -> Option<String> {
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let byte = self.peek()?;
            self.at += 1;
            match byte {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let escaped = self.peek()?;
                    self.at += 1;
                    let c = match escaped {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => self.unicode()?,
                        other => char::from(other),
                    };
                    let mut buffer = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
                other => out.push(other),
            }
        }
    }

    fn unicode(&mut self) -> Option<char> {
        let first = self.hex()?;
        if (0xd800..0xdc00).contains(&first) && self.bytes.get(self.at..self.at + 2) == Some(b"\\u")
        {
            self.at += 2;
            let second = self.hex()?;
            let joined = 0x10000 + ((first - 0xd800) << 10) + (second.checked_sub(0xdc00)?);
            return char::from_u32(joined);
        }
        Some(char::from_u32(first).unwrap_or('\u{fffd}'))
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.at += 1;
        }
        let text = std::str::from_utf8(self.bytes.get(start..self.at)?).ok()?;
        text.parse::<f64>()
            .ok()
            .map(|_| Value::Number(text.to_string()))
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > DEPTH {
            return None;
        }
        self.space();
        match self.peek()? {
            b'n' => self.word("null", Value::Null),
            b't' => self.word("true", Value::Bool(true)),
            b'f' => self.word("false", Value::Bool(false)),
            b'"' => self.text().map(Value::Text),
            b'[' => self.list(depth),
            b'{' => self.object(depth),
            _ => self.number(),
        }
    }

    fn list(&mut self, depth: usize) -> Option<Value> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek()? == b']' {
            self.at += 1;
            return Some(Value::List(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Some(Value::List(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self, depth: usize) -> Option<Value> {
        self.at += 1;
        let mut fields = Vec::new();
        self.space();
        if self.peek()? == b'}' {
            self.at += 1;
            return Some(Value::Object(fields));
        }
        loop {
            self.space();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.text()?;
            self.space();
            if self.peek()? != b':' {
                return None;
            }
            self.at += 1;
            fields.push((key, self.value(depth + 1)?));
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Some(Value::Object(fields));
                }
                _ => return None,
            }
        }
    }
}

pub(crate) fn parse(text: &str) -> Option<Value> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value(0)?;
    reader.space();
    (reader.at == reader.bytes.len()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{Value, parse, quote};

    #[test]
    fn json_is_read_and_written_back() {
        let text = r#"{"a":[1,2.5,{"b":"x\"é😀"}],"c":true,"d":null}"#;
        let value = parse(text).expect("JSON");
        assert_eq!(
            value
                .get("a")
                .and_then(|a| a.at(2))
                .and_then(|b| b.get("b"))
                .and_then(Value::text),
            Some("x\"é😀")
        );
        assert_eq!(
            value.get("a").and_then(|a| a.at(0)).and_then(Value::number),
            Some(1)
        );
        let mut out = String::new();
        value.write(&mut out);
        assert_eq!(
            out,
            "{\"a\":[1,2.5,{\"b\":\"x\\\"é😀\"}],\"c\":true,\"d\":null}"
        );
        assert!(parse("{\"a\":}").is_none());
        assert!(parse("[1] x").is_none());
        assert!(parse(&"[".repeat(100)).is_none());
        assert_eq!(quote("a\nb\u{1}"), "\"a\\nb\\u0001\"");
    }
}
