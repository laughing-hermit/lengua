#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool,
    Number,
    Text,
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 64;

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

    fn word(&mut self, word: &str) -> Option<()> {
        let end = self.at + word.len();
        (self.bytes.get(self.at..end)? == word.as_bytes()).then(|| self.at = end)
    }

    fn text(&mut self) -> Option<String> {
        self.at += 1;
        let start = self.at;
        loop {
            match self.peek()? {
                b'"' => break,
                b'\\' => self.at += 2,
                _ => self.at += 1,
            }
        }
        let raw = std::str::from_utf8(self.bytes.get(start..self.at)?).ok()?;
        self.at += 1;
        Some(unescape(raw))
    }

    fn number(&mut self) -> Option<()> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.at += 1;
        }
        (self.at > start).then_some(())
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.space();
        match self.peek()? {
            b'n' => self.word("null").map(|()| Json::Null),
            b't' => self.word("true").map(|()| Json::Bool),
            b'f' => self.word("false").map(|()| Json::Bool),
            b'"' => self.text().map(|_| Json::Text),
            b'[' => self.list(depth),
            b'{' => self.object(depth),
            _ => self.number().map(|()| Json::Number),
        }
    }

    fn list(&mut self, depth: usize) -> Option<Json> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek()? == b']' {
            self.at += 1;
            return Some(Json::List(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Some(Json::List(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self, depth: usize) -> Option<Json> {
        self.at += 1;
        let mut fields = Vec::new();
        self.space();
        if self.peek()? == b'}' {
            self.at += 1;
            return Some(Json::Object(fields));
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
                    return Some(Json::Object(fields));
                }
                _ => return None,
            }
        }
    }
}

fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(found) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(found);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

pub(crate) fn parse(text: &str) -> Option<Json> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value(0)?;
    reader.space();
    (reader.at == text.len()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{Json, parse};

    #[test]
    fn values_lists_and_objects_are_read() {
        assert_eq!(parse("null"), Some(Json::Null));
        assert_eq!(parse(" 12.5e3 "), Some(Json::Number));
        assert_eq!(
            parse("[true, \"a\\\"b\"]"),
            Some(Json::List(vec![Json::Bool, Json::Text]))
        );
        assert_eq!(
            parse("{\"id\": 1, \"tags\": []}"),
            Some(Json::Object(vec![
                ("id".to_string(), Json::Number),
                ("tags".to_string(), Json::List(Vec::new())),
            ]))
        );
        assert_eq!(parse("{\"id\" 1}"), None);
        assert_eq!(parse("[1,"), None);
        assert_eq!(parse("<html>"), None);
        assert_eq!(parse(""), None);
    }
}
