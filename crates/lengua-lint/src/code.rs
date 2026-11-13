use crate::lex::{self, Error, Kind, Token};

pub(crate) const KEYWORDS: [&str; 41] = [
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "static", "struct", "super", "trait", "true", "type", "unsafe",
];

const MORE_KEYWORDS: [&str; 7] = ["use", "where", "while", "yield", "try", "typeof", "priv"];

pub(crate) fn keyword(word: &str) -> bool {
    KEYWORDS.contains(&word) || MORE_KEYWORDS.contains(&word)
}

pub(crate) struct Code {
    pub tokens: Vec<Token>,
    pub close: Vec<usize>,
    pub comments: Vec<Token>,
}

fn closer(open: &str) -> &'static str {
    match open {
        "(" => ")",
        "[" => "]",
        _ => "}",
    }
}

pub(crate) fn read(text: &str) -> Result<Code, Error> {
    let all = lex::tokens(text)?;
    let (comments, tokens): (Vec<Token>, Vec<Token>) = all
        .into_iter()
        .partition(|t| matches!(t.kind, Kind::Comment | Kind::Shebang));
    let mut close: Vec<usize> = (0..tokens.len()).collect();
    let mut stack: Vec<usize> = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text.as_str() {
            "(" | "[" | "{" => stack.push(at),
            ")" | "]" | "}" => {
                let Some(open) = stack.pop() else {
                    return Err(Error {
                        line: token.line,
                        message: format!("`{}` closes nothing", token.text),
                    });
                };
                let opener = &tokens[open];
                if closer(&opener.text) != token.text {
                    return Err(Error {
                        line: token.line,
                        message: format!(
                            "`{}` does not close the `{}` of line {}",
                            token.text, opener.text, opener.line
                        ),
                    });
                }
                close[open] = at;
                close[at] = open;
            }
            _ => {}
        }
    }
    if let Some(open) = stack.pop() {
        let opener = &tokens[open];
        return Err(Error {
            line: opener.line,
            message: format!("the `{}` here is never closed", opener.text),
        });
    }
    Ok(Code {
        tokens,
        close,
        comments,
    })
}

impl Code {
    pub(crate) fn len(&self) -> usize {
        self.tokens.len()
    }

    pub(crate) fn is(&self, at: usize, text: &str) -> bool {
        self.tokens.get(at).is_some_and(|t| t.is(text))
    }

    pub(crate) fn kind(&self, at: usize) -> Option<Kind> {
        self.tokens.get(at).map(|t| t.kind)
    }

    pub(crate) fn text(&self, at: usize) -> &str {
        self.tokens.get(at).map_or("", |t| t.text.as_str())
    }

    pub(crate) fn line(&self, at: usize) -> usize {
        self.tokens
            .get(at)
            .or_else(|| self.tokens.last())
            .map_or(1, |t| t.line)
    }

    pub(crate) fn open(&self, at: usize) -> bool {
        self.is(at, "(") || self.is(at, "[") || self.is(at, "{")
    }

    pub(crate) fn after(&self, at: usize) -> usize {
        if self.open(at) {
            self.close.get(at).copied().unwrap_or(at) + 1
        } else {
            at + 1
        }
    }

    pub(crate) fn end(&self, open: usize) -> usize {
        self.close.get(open).copied().unwrap_or(open)
    }

    pub(crate) fn joint(&self, at: usize, first: &str, second: &str) -> bool {
        self.tokens.get(at).is_some_and(|t| t.joint && t.is(first)) && self.is(at + 1, second)
    }

    pub(crate) fn path_sep(&self, at: usize) -> bool {
        self.joint(at, ":", ":")
    }

    pub(crate) fn colon(&self, at: usize) -> bool {
        self.is(at, ":") && !self.path_sep(at) && !(at > 0 && self.path_sep(at - 1))
    }

    pub(crate) fn single_eq(&self, at: usize) -> bool {
        self.is(at, "=")
            && !self.joint(at, "=", "=")
            && !self.joint(at, "=", ">")
            && !(at > 0 && self.tokens.get(at - 1).is_some_and(|t| t.joint && t.kind == Kind::Punct))
    }

    pub(crate) fn arrow_close(&self, at: usize) -> bool {
        at > 0 && self.joint(at - 1, "-", ">")
    }

    pub(crate) fn ident(&self, at: usize) -> Option<&str> {
        self.tokens
            .get(at)
            .filter(|t| t.kind == Kind::Ident)
            .map(|t| t.text.as_str())
    }

    pub(crate) fn name(&self, at: usize) -> Option<&str> {
        let word = self.ident(at)?;
        if let Some(raw) = word.strip_prefix("r#") {
            return Some(raw);
        }
        (!keyword(word)).then_some(word)
    }

    pub(crate) fn segment(&self, at: usize) -> Option<&str> {
        let word = self.ident(at)?;
        if matches!(word, "Self" | "self" | "crate" | "super") {
            return Some(word);
        }
        self.name(at)
    }

    pub(crate) fn after_dot(&self, at: usize) -> bool {
        at > 0 && self.is(at - 1, ".") && !(at > 1 && self.joint(at - 2, ".", "."))
    }

    pub(crate) fn after_path(&self, at: usize) -> bool {
        at > 1 && self.path_sep(at - 2)
    }

    pub(crate) fn path(&self, start: usize) -> (Vec<String>, usize) {
        let mut segments: Vec<String> = Vec::new();
        let mut at = start;
        if let Some(first) = self.segment(at) {
            segments.push(first.to_string());
            at += 1;
        }
        while self.path_sep(at) {
            if self.is(at + 2, "<") {
                at = self.angle_end(at + 2, self.len()) + 1;
                continue;
            }
            let Some(next) = self.segment(at + 2) else {
                break;
            };
            segments.push(next.to_string());
            at += 3;
        }
        (segments, at)
    }

    pub(crate) fn angle_end(&self, open: usize, to: usize) -> usize {
        let mut depth = 0_usize;
        let mut at = open;
        while at < to {
            if self.is(at, "<") {
                depth += 1;
            } else if self.is(at, ">") && !self.arrow_close(at) {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return at;
                }
            }
            at = self.after(at);
        }
        to
    }

    pub(crate) fn names(&self, from: usize, to: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for at in from..to.min(self.len()) {
            if let Some(name) = self.name(at)
                && !out.iter().any(|n| n == name)
            {
                out.push(name.to_string());
            }
        }
        out
    }

    pub(crate) fn macro_call(&self, end: usize) -> bool {
        self.is(end, "!") && !self.joint(end, "!", "=") && self.open(end + 1)
    }

    pub(crate) fn split(&self, from: usize, to: usize, first: &str, second: &str) -> Vec<(usize, usize)> {
        let mut parts = Vec::new();
        let mut start = from;
        let mut at = from;
        while at < to {
            if self.joint(at, first, second) {
                parts.push((start, at));
                at += 2;
                start = at;
                continue;
            }
            at = self.after(at);
        }
        parts.push((start, to));
        parts
    }

    pub(crate) fn find(&self, from: usize, to: usize, text: &str) -> Option<usize> {
        let mut at = from;
        while at < to {
            if self.is(at, text) {
                return Some(at);
            }
            at = self.after(at);
        }
        None
    }
}
