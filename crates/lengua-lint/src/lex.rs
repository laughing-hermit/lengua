#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Ident,
    Lifetime,
    Integer,
    Literal,
    Punct,
    Comment,
    Shebang,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: Kind,
    pub text: String,
    pub line: usize,
    pub joint: bool,
}

impl Token {
    pub(crate) fn is(&self, text: &str) -> bool {
        matches!(self.kind, Kind::Ident | Kind::Punct) && self.text == text
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub line: usize,
    pub message: String,
}

const PUNCT: &str = ";,.(){}[]@#~?:$=!<>-&|+*/^%";

fn ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn digit(c: char) -> bool {
    c.is_ascii_digit() || c == '_'
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    line: usize,
    out: Vec<Token>,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.at += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn bump_n(&mut self, count: usize) {
        for _ in 0..count {
            self.bump();
        }
    }

    fn eat_while(&mut self, keep: impl Fn(char) -> bool) {
        while self.peek(0).is_some_and(&keep) {
            self.bump();
        }
    }

    fn push(&mut self, kind: Kind, start: usize, line: usize) {
        let text: String = self
            .chars
            .get(start..self.at)
            .unwrap_or_default()
            .iter()
            .collect();
        let joint = kind == Kind::Punct && self.peek(0).is_some_and(|c| PUNCT.contains(c));
        self.out.push(Token {
            kind,
            text,
            line,
            joint,
        });
    }

    fn shebang(&mut self) {
        if self.peek(0) != Some('#') || self.peek(1) != Some('!') {
            return;
        }
        let next = self
            .chars
            .iter()
            .skip(self.at + 2)
            .find(|c| !c.is_whitespace());
        if next == Some(&'[') {
            return;
        }
        let start = self.at;
        self.eat_while(|c| c != '\n');
        self.push(Kind::Shebang, start, 1);
    }

    fn token(&mut self, c: char) -> Result<(), Error> {
        let start = self.at;
        let line = self.line;
        if c.is_whitespace() {
            self.bump();
            return Ok(());
        }
        if c == '/' && self.peek(1) == Some('/') {
            self.eat_while(|c| c != '\n');
            self.push(Kind::Comment, start, line);
            return Ok(());
        }
        if c == '/' && self.peek(1) == Some('*') {
            return self.block_comment(start, line);
        }
        if let Some(prefix) = self.raw_prefix() {
            return self.raw_string(start, line, prefix);
        }
        if matches!(c, 'b' | 'c') && self.peek(1) == Some('"') {
            self.bump();
            return self.string(start, line);
        }
        if c == 'b' && self.peek(1) == Some('\'') {
            self.bump();
            return self.quote(start, line);
        }
        if c == 'r' && self.peek(1) == Some('#') && self.peek(2).is_some_and(ident_start) {
            self.bump_n(2);
            self.eat_while(ident_continue);
            self.push(Kind::Ident, start, line);
            return Ok(());
        }
        if ident_start(c) {
            self.eat_while(ident_continue);
            self.push(Kind::Ident, start, line);
            return Ok(());
        }
        if c.is_ascii_digit() {
            self.number(start, line);
            return Ok(());
        }
        if c == '"' {
            return self.string(start, line);
        }
        if c == '\'' {
            return self.quote(start, line);
        }
        if PUNCT.contains(c) {
            self.bump();
            self.push(Kind::Punct, start, line);
            return Ok(());
        }
        Err(Error {
            line,
            message: format!("the character `{c}` is not one that Rust's grammar allows here"),
        })
    }

    fn block_comment(&mut self, start: usize, line: usize) -> Result<(), Error> {
        self.bump_n(2);
        let mut depth = 1_usize;
        while depth > 0 {
            match (self.peek(0), self.peek(1)) {
                (Some('/'), Some('*')) => {
                    self.bump_n(2);
                    depth += 1;
                }
                (Some('*'), Some('/')) => {
                    self.bump_n(2);
                    depth -= 1;
                }
                (Some(_), _) => {
                    self.bump();
                }
                (None, _) => return Err(unterminated(line, "a block comment")),
            }
        }
        self.push(Kind::Comment, start, line);
        Ok(())
    }

    fn raw_prefix(&self) -> Option<usize> {
        let length = match (self.peek(0), self.peek(1)) {
            (Some('r'), _) => 1,
            (Some('b' | 'c'), Some('r')) => 2,
            _ => return None,
        };
        let mut at = length;
        while self.peek(at) == Some('#') {
            at += 1;
        }
        (self.peek(at) == Some('"')).then_some(length)
    }

    fn raw_string(&mut self, start: usize, line: usize, prefix: usize) -> Result<(), Error> {
        self.bump_n(prefix);
        let mut hashes = 0;
        while self.peek(0) == Some('#') {
            self.bump();
            hashes += 1;
        }
        self.bump();
        loop {
            match self.bump() {
                None => return Err(unterminated(line, "a raw string")),
                Some('"') => {
                    let mut found = 0;
                    while found < hashes && self.peek(found) == Some('#') {
                        found += 1;
                    }
                    if found == hashes {
                        self.bump_n(hashes);
                        break;
                    }
                }
                Some(_) => {}
            }
        }
        self.eat_while(ident_continue);
        self.push(Kind::Literal, start, line);
        Ok(())
    }

    fn string(&mut self, start: usize, line: usize) -> Result<(), Error> {
        self.bump();
        loop {
            match self.bump() {
                None => return Err(unterminated(line, "a string")),
                Some('\\') => {
                    self.bump();
                }
                Some('"') => break,
                Some(_) => {}
            }
        }
        self.eat_while(ident_continue);
        self.push(Kind::Literal, start, line);
        Ok(())
    }

    fn quote(&mut self, start: usize, line: usize) -> Result<(), Error> {
        self.bump();
        match (self.peek(0), self.peek(1)) {
            (Some('\\'), _) => {
                self.bump_n(2);
                while let Some(c) = self.peek(0) {
                    if c == '\'' || c == '\n' {
                        break;
                    }
                    self.bump();
                }
                if self.peek(0) != Some('\'') {
                    return Err(unterminated(line, "a character literal"));
                }
                self.bump();
                self.push(Kind::Literal, start, line);
            }
            (Some(_), Some('\'')) => {
                self.bump_n(2);
                self.push(Kind::Literal, start, line);
            }
            (Some(c), _) if ident_start(c) => {
                if c == 'r' && self.peek(1) == Some('#') {
                    self.bump_n(2);
                }
                self.eat_while(ident_continue);
                self.push(Kind::Lifetime, start, line);
            }
            _ => {
                return Err(Error {
                    line,
                    message: "a quote that is not a character literal or a lifetime".to_string(),
                });
            }
        }
        Ok(())
    }

    fn exponent(&mut self) -> bool {
        let first = if matches!(self.peek(1), Some('+' | '-')) {
            2
        } else {
            1
        };
        if !self.peek(first).is_some_and(|c| c.is_ascii_digit()) {
            return false;
        }
        self.bump_n(first);
        self.eat_while(digit);
        true
    }

    fn number(&mut self, start: usize, line: usize) {
        let mut float = false;
        if self.peek(0) == Some('0') && matches!(self.peek(1), Some('x' | 'o' | 'b')) {
            self.bump_n(2);
            self.eat_while(|c| c.is_ascii_hexdigit() || c == '_');
        } else {
            self.eat_while(digit);
            if self.peek(0) == Some('.') && self.peek(1).is_none_or(|c| c != '.' && !ident_start(c))
            {
                float = true;
                self.bump();
                self.eat_while(digit);
            }
            if matches!(self.peek(0), Some('e' | 'E')) && self.exponent() {
                float = true;
            }
            if self.peek(0) == Some('f') {
                float = true;
            }
        }
        self.eat_while(ident_continue);
        let kind = if float { Kind::Literal } else { Kind::Integer };
        self.push(kind, start, line);
    }
}

fn unterminated(line: usize, what: &str) -> Error {
    Error {
        line,
        message: format!("{what} that starts here has no end"),
    }
}

pub(crate) fn tokens(text: &str) -> Result<Vec<Token>, Error> {
    let mut lexer = Lexer {
        chars: text.chars().collect(),
        at: 0,
        line: 1,
        out: Vec::new(),
    };
    if lexer.peek(0) == Some('\u{feff}') {
        lexer.at = 1;
    }
    lexer.shebang();
    while let Some(c) = lexer.peek(0) {
        lexer.token(c)?;
    }
    Ok(lexer.out)
}

#[cfg(test)]
mod tests {
    use super::{Kind, tokens};

    fn texts(text: &str) -> Vec<String> {
        tokens(text)
            .expect("tokens")
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    fn kinds(text: &str) -> Vec<Kind> {
        tokens(text)
            .expect("tokens")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lifetimes_and_characters_are_told_apart() {
        assert_eq!(
            kinds("'a' 'a '\\'' 'static '\\u{1F600}' b'x' 'r#t '_"),
            [
                Kind::Literal,
                Kind::Lifetime,
                Kind::Literal,
                Kind::Lifetime,
                Kind::Literal,
                Kind::Literal,
                Kind::Lifetime,
                Kind::Lifetime
            ]
        );
        assert_eq!(
            texts("impl<'a> X<'a>"),
            ["impl", "<", "'a", ">", "X", "<", "'a", ">"]
        );
    }

    #[test]
    fn raw_byte_and_c_strings_are_one_token() {
        let text = "r\"a\" r#\"b\"c\"# br##\"d\"#e\"## c\"f\" b\"g\\\"h\" cr#\"i\"#";
        assert_eq!(
            texts(text),
            [
                "r\"a\"",
                "r#\"b\"c\"#",
                "br##\"d\"#e\"##",
                "c\"f\"",
                "b\"g\\\"h\"",
                "cr#\"i\"#"
            ]
        );
    }

    #[test]
    fn block_comments_nest() {
        let found = tokens("a /* one /* two */ still */ b").expect("tokens");
        assert_eq!(found.len(), 3);
        assert_eq!(found[1].kind, Kind::Comment);
        assert!(tokens("/* /* */").is_err());
    }

    #[test]
    fn numbers_keep_their_suffixes_and_ranges_split() {
        assert_eq!(texts("1..2"), ["1", ".", ".", "2"]);
        assert_eq!(texts("1.max(2)"), ["1", ".", "max", "(", "2", ")"]);
        assert_eq!(
            kinds("0x1f_u8 1e-3 1.5f32 2f64 7usize 1_000"),
            [
                Kind::Integer,
                Kind::Literal,
                Kind::Literal,
                Kind::Literal,
                Kind::Integer,
                Kind::Integer
            ]
        );
        assert_eq!(texts("x.0.1"), ["x", ".", "0.1"]);
    }

    #[test]
    fn a_shebang_is_one_token_and_an_inner_attribute_is_not() {
        assert_eq!(kinds("#!/bin/run\nfn")[0], Kind::Shebang);
        assert_eq!(texts("#![forbid(x)]")[0], "#");
    }

    #[test]
    fn raw_identifiers_and_lines() {
        let found = tokens("r#fn\n\nx").expect("tokens");
        assert_eq!(found[0].text, "r#fn");
        assert_eq!(found[0].kind, Kind::Ident);
        assert_eq!(found[1].line, 3);
    }

    #[test]
    fn punctuation_knows_when_it_is_joined() {
        let found = tokens("a += b ::c").expect("tokens");
        assert!(found[1].joint);
        assert!(!found[2].joint);
        assert!(found[4].joint);
    }

    const PIECES: [&str; 24] = [
        "r#\"a\"b\"#",
        "br##\"x\"#y\"##",
        "'a'",
        "'\\''",
        "'\\u{1F600}'",
        "'a",
        "'static",
        "b'\\\\'",
        "\"a\\\"b\"",
        "0x1f_u8",
        "1e-3",
        "1.5f32",
        "r#fn",
        "c\"x\"",
        "/* a /* b */ c */",
        "// a line comment \" '",
        "'r#k",
        "\"two\nlines\"",
        "1_000u64",
        "\u{e9}t\u{e9}",
        "_",
        "r\"\\\"",
        "'\"'",
        "\"'\"",
    ];

    fn next(state: &mut u64) -> usize {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        let count = u64::try_from(PIECES.len()).unwrap_or(1);
        usize::try_from(*state % count).unwrap_or(0)
    }

    #[test]
    fn tricky_pieces_are_one_token_each_and_no_slice_panics() {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..400 {
            let chosen: Vec<&str> = (0..8).map(|_| PIECES[next(&mut state)]).collect();
            let text = chosen.join("\n");
            let found: Vec<String> = tokens(&text)
                .expect("tokens")
                .into_iter()
                .map(|t| t.text)
                .collect();
            assert_eq!(found, chosen, "{text}");
            for (at, _) in text.char_indices() {
                let _ = tokens(&text[..at]);
                let _ = tokens(&text[at..]);
                let _ = crate::code::read(&text[at..]);
            }
        }
    }

    #[test]
    fn broken_input_is_an_error_with_its_line() {
        assert_eq!(tokens("\n\"open").map_err(|e| e.line), Err(2));
        assert!(tokens("r#\"open\"").is_err());
        assert!(tokens("'").is_err());
        assert!(tokens("a \u{1} b").is_err());
    }
}
