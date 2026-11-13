use std::collections::{BTreeMap, BTreeSet};

use crate::Finding;
use crate::code::Code;

#[derive(Clone, Debug)]
pub(crate) struct Function {
    pub file: usize,
    pub key: String,
    pub owner: Option<usize>,
    pub line: usize,
    pub end: usize,
    pub params: (usize, usize),
    pub body: (usize, usize),
    pub generics: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Owner {
    pub target: String,
    pub trait_name: Option<String>,
    pub is_trait: bool,
    pub generics: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TypeDef {
    pub fields: Vec<(String, Vec<String>)>,
    pub generics: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Macro {
    pub file: usize,
    pub name: String,
    pub line: usize,
    pub body: (usize, usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Use {
    pub file: usize,
    pub first: String,
    pub names: Vec<String>,
    pub glob: bool,
}

#[derive(Default)]
pub(crate) struct Model {
    pub functions: Vec<Function>,
    pub owners: Vec<Owner>,
    pub types: BTreeMap<String, TypeDef>,
    pub traits: BTreeSet<String>,
    pub aliases: BTreeMap<String, Vec<String>>,
    pub macros: Vec<Macro>,
    pub uses: Vec<Use>,
    pub findings: Vec<Finding>,
    pub files: Vec<String>,
}

struct Reader<'a> {
    code: &'a Code,
    file: usize,
    model: &'a mut Model,
}

pub(crate) fn skip_attributes(code: &Code, start: usize) -> usize {
    let mut at = start;
    while code.is(at, "#") {
        if code.is(at + 1, "!") && code.is(at + 2, "[") {
            at = code.after(at + 2);
        } else if code.is(at + 1, "[") {
            at = code.after(at + 1);
        } else {
            break;
        }
    }
    at
}

fn qualifier_then_item(code: &Code, at: usize) -> bool {
    matches!(
        code.ident(at),
        Some("fn" | "unsafe" | "async" | "extern" | "impl" | "trait" | "const" | "safe")
    )
}

fn skip_qualifiers(code: &Code, start: usize) -> usize {
    let mut at = start;
    loop {
        let word = code.ident(at).unwrap_or("");
        if matches!(word, "const" | "async" | "unsafe" | "safe" | "default")
            && qualifier_then_item(code, at + 1)
        {
            at += 1;
        } else if word == "extern"
            && code.kind(at + 1) == Some(crate::lex::Kind::Literal)
            && qualifier_then_item(code, at + 2)
        {
            at += 2;
        } else if word == "extern" && code.is(at + 1, "fn") {
            at += 1;
        } else {
            return at;
        }
    }
}

fn generic_names(code: &Code, open: usize, end: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0_usize;
    let mut fresh = true;
    let mut at = open + 1;
    while at < end {
        if code.is(at, "<") {
            depth += 1;
        } else if code.is(at, ">") && !code.arrow_close(at) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && code.is(at, ",") {
            fresh = true;
        } else if depth == 0 && fresh && code.is(at, "const") {
            at += 1;
            continue;
        } else if depth == 0 && fresh {
            if let Some(name) = code.name(at) {
                out.push(name.to_string());
            }
            fresh = false;
        }
        at = code.after(at);
    }
    out
}

pub(crate) fn segment_end(code: &Code, from: usize, to: usize) -> usize {
    let mut depth = 0_usize;
    let mut at = from;
    while at < to {
        if code.is(at, "<") {
            depth += 1;
        } else if code.is(at, ">") && !code.arrow_close(at) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && code.is(at, ",") {
            return at;
        }
        at = code.after(at);
    }
    to
}

fn named_fields(code: &Code, open: usize) -> Vec<(String, Vec<String>)> {
    let close = code.end(open);
    let mut out = Vec::new();
    let mut at = open + 1;
    while at < close {
        at = skip_attributes(code, at);
        if code.is(at, "pub") {
            at += 1;
            if code.is(at, "(") {
                at = code.after(at);
            }
        }
        let end = segment_end(code, at, close);
        let name = code.name(at).map(str::to_string).unwrap_or_default();
        let types = if code.colon(at + 1) {
            code.names(at + 2, end)
        } else {
            code.names(at, end)
        };
        out.push((name, types));
        at = end + 1;
    }
    out
}

fn tuple_fields(code: &Code, open: usize) -> Vec<(String, Vec<String>)> {
    let close = code.end(open);
    let mut out = Vec::new();
    let mut at = open + 1;
    while at < close {
        let end = segment_end(code, at, close);
        out.push((out.len().to_string(), code.names(at, end)));
        at = end + 1;
    }
    out
}

fn variant_types(code: &Code, open: usize) -> Vec<(String, Vec<String>)> {
    let close = code.end(open);
    let mut names: Vec<String> = Vec::new();
    let mut at = open + 1;
    while at < close {
        if code.open(at) {
            let fields = if code.is(at, "{") {
                named_fields(code, at)
            } else {
                tuple_fields(code, at)
            };
            for (_, types) in fields {
                names.extend(types);
            }
        }
        at = code.after(at);
    }
    vec![(String::new(), names)]
}

impl Reader<'_> {
    fn finding(&mut self, at: usize, code: &'static str, message: String) {
        let file = self.model.files.get(self.file).cloned().unwrap_or_default();
        self.model.findings.push(Finding {
            file,
            line: self.code.line(at),
            code,
            message,
        });
    }

    fn unreadable(&mut self, at: usize, to: usize, what: &str) -> usize {
        self.finding(
            at,
            "rust-read",
            format!("lengua's reader of Rust can't read {what}, so it can't check the rules of the code here; write it in a plainer form"),
        );
        to
    }

    fn items(&mut self, from: usize, to: usize, owner: Option<usize>) {
        let mut at = from;
        while at < to {
            at = self.item(at, to, owner);
        }
    }

    fn item(&mut self, start: usize, to: usize, owner: Option<usize>) -> usize {
        let code = self.code;
        let mut at = skip_attributes(code, start);
        if at >= to {
            return to;
        }
        if code.is(at, "pub") {
            at += 1;
            if code.is(at, "(") {
                at = code.after(at);
            }
        }
        at = skip_qualifiers(code, at);
        match code.ident(at).unwrap_or("") {
            "fn" => self.function(at, to, owner),
            "struct" | "enum" | "union" if code.name(at + 1).is_some() => self.type_def(at, to),
            "trait" => self.trait_def(at, to),
            "auto" if code.is(at + 1, "trait") => self.trait_def(at + 1, to),
            "impl" => self.impl_def(at, to),
            "mod" => self.module(at, to),
            "use" => self.use_item(at, to),
            "static" | "const" => self.to_semicolon(at, to),
            "type" => self.alias(at, to),
            "macro_rules" if code.is(at + 1, "!") => self.macro_def(at, to),
            "extern" => self.unreadable(at, to, "an `extern` item"),
            _ if code.is(at, ";") => at + 1,
            _ if code.segment(at).is_some() => self.macro_item(at, to),
            _ => self.unreadable(at, to, "this item"),
        }
    }

    fn to_semicolon(&mut self, at: usize, to: usize) -> usize {
        match self.code.find(at, to, ";") {
            Some(end) => end + 1,
            None => self.unreadable(at, to, "an item with no `;` at its end"),
        }
    }

    fn function(&mut self, at: usize, to: usize, owner: Option<usize>) -> usize {
        let code = self.code;
        let Some(name) = code.name(at + 1) else {
            return self.unreadable(at, to, "a `fn` with no name");
        };
        let mut next = at + 2;
        let mut generics = Vec::new();
        if code.is(next, "<") {
            let end = code.angle_end(next, to);
            generics = generic_names(code, next, end);
            next = end + 1;
        }
        if !code.is(next, "(") {
            return self.unreadable(at, to, "a `fn` with no parameters in parentheses");
        }
        let params = (next, code.end(next));
        next = code.after(next);
        while next < to && !code.is(next, "{") && !code.is(next, ";") {
            next = code.after(next);
        }
        if code.is(next, ";") {
            return next + 1;
        }
        if next >= to {
            return self.unreadable(at, to, "a `fn` with no body");
        }
        let body = (next, code.end(next));
        let key = owner.and_then(|o| self.model.owners.get(o)).map_or_else(
            || name.to_string(),
            |o| format!("{}::{name}", o.target),
        );
        self.model.functions.push(Function {
            file: self.file,
            key,
            owner,
            line: code.line(at),
            end: code.line(body.1),
            params,
            body,
            generics,
        });
        body.1 + 1
    }

    fn type_def(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let name = code.name(at + 1).unwrap_or_default().to_string();
        let is_enum = code.is(at, "enum");
        let mut next = at + 2;
        let mut def = TypeDef::default();
        if code.is(next, "<") {
            let end = code.angle_end(next, to);
            def.generics = generic_names(code, next, end);
            next = end + 1;
        }
        while next < to {
            if code.is(next, ";") {
                next += 1;
                break;
            }
            if code.is(next, "{") {
                def.fields = if is_enum {
                    variant_types(code, next)
                } else {
                    named_fields(code, next)
                };
                next = code.after(next);
                break;
            }
            if code.is(next, "(") {
                def.fields = tuple_fields(code, next);
            }
            next = code.after(next);
        }
        self.model.types.insert(name, def);
        next
    }

    fn brace(&mut self, from: usize, to: usize) -> Option<usize> {
        let mut at = from;
        while at < to && !self.code.is(at, "{") {
            if self.code.is(at, ";") {
                return None;
            }
            at = self.code.after(at);
        }
        (at < to).then_some(at)
    }

    fn trait_def(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let Some(name) = code.name(at + 1) else {
            return self.unreadable(at, to, "a `trait` with no name");
        };
        let mut generics = Vec::new();
        if code.is(at + 2, "<") {
            let end = code.angle_end(at + 2, to);
            generics = generic_names(code, at + 2, end);
        }
        let Some(open) = self.brace(at + 2, to) else {
            return self.unreadable(at, to, "a `trait` with no body");
        };
        self.model.traits.insert(name.to_string());
        self.model.owners.push(Owner {
            target: name.to_string(),
            trait_name: Some(name.to_string()),
            is_trait: true,
            generics,
        });
        let owner = self.model.owners.len() - 1;
        self.items(open + 1, code.end(open), Some(owner));
        code.end(open) + 1
    }

    fn impl_def(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let mut head = at + 1;
        let mut generics = Vec::new();
        if code.is(head, "<") {
            let end = code.angle_end(head, to);
            generics = generic_names(code, head, end);
            head = end + 1;
        }
        let Some(open) = self.brace(head, to) else {
            return self.unreadable(at, to, "an `impl` with no body");
        };
        if code.is(head, "!") {
            return self.unreadable(at, to, "a negative `impl`");
        }
        let (trait_part, target_part) = impl_parts(code, head, open);
        let trait_name = trait_part.and_then(|(from, end)| last_name(code, from, end));
        let target = target_name(code, target_part.0, target_part.1);
        let close = code.end(open);
        let Some(target) = target else {
            self.finding(
                at,
                "rule-12-impl",
                "lengua names a method as `Type::name`, and this `impl` is for a type with no name of that form; implement it for a struct or enum of the program".to_string(),
            );
            return close + 1;
        };
        if generics.contains(&target) || !target.starts_with(|c: char| c.is_uppercase()) {
            self.finding(
                at,
                "rule-12-impl",
                format!("lengua names a method as `Type::name`, and `{target}` is not a type of the program; implement it for a struct or enum of the program"),
            );
            return close + 1;
        }
        self.model.owners.push(Owner {
            target,
            trait_name,
            is_trait: false,
            generics,
        });
        let owner = self.model.owners.len() - 1;
        self.items(open + 1, close, Some(owner));
        close + 1
    }

    fn module(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        if code.name(at + 1).is_some() && code.is(at + 2, ";") {
            return at + 3;
        }
        if code.is(at + 2, "{") {
            self.finding(
                at,
                "rule-12-module",
                "a module is a file of its own with an idea of its own; move this module to `src/<name>.rs` and write `ideas/<name>.md`".to_string(),
            );
            return code.after(at + 2);
        }
        self.unreadable(at, to, "this `mod` item")
    }

    fn use_item(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let Some(end) = code.find(at, to, ";") else {
            return self.unreadable(at, to, "a `use` with no `;` at its end");
        };
        let first = code.segment(at + 1).unwrap_or_default().to_string();
        let glob = (at..end).any(|k| code.is(k, "*"));
        self.model.uses.push(Use {
            file: self.file,
            first,
            names: code.names(at + 1, end),
            glob,
        });
        end + 1
    }

    fn alias(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let Some(end) = code.find(at, to, ";") else {
            return self.unreadable(at, to, "a `type` with no `;` at its end");
        };
        if let Some(name) = code.name(at + 1)
            && let Some(eq) = (at..end).find(|k| code.single_eq(*k))
        {
            self.model
                .aliases
                .insert(name.to_string(), code.names(eq + 1, end));
        }
        end + 1
    }

    fn macro_def(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let Some(name) = code.name(at + 2) else {
            return self.unreadable(at, to, "a `macro_rules!` with no name");
        };
        if !code.open(at + 3) {
            return self.unreadable(at, to, "a `macro_rules!` with no body");
        }
        self.model.macros.push(Macro {
            file: self.file,
            name: name.to_string(),
            line: code.line(at),
            body: (at + 3, code.end(at + 3)),
        });
        let next = code.after(at + 3);
        if code.is(next, ";") { next + 1 } else { next }
    }

    fn macro_item(&mut self, at: usize, to: usize) -> usize {
        let code = self.code;
        let (_, end) = code.path(at);
        if !code.macro_call(end) {
            return self.unreadable(at, to, "this item");
        }
        let next = code.after(end + 1);
        if code.is(next, ";") { next + 1 } else { next }
    }
}

fn impl_parts(code: &Code, head: usize, open: usize) -> (Option<(usize, usize)>, (usize, usize)) {
    let mut depth = 0_usize;
    let mut split = None;
    let mut end = open;
    let mut at = head;
    while at < open {
        if code.is(at, "<") {
            depth += 1;
        } else if code.is(at, ">") && !code.arrow_close(at) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && code.is(at, "where") {
            end = at;
            break;
        } else if depth == 0 && split.is_none() && code.is(at, "for") && !code.is(at + 1, "<") {
            split = Some(at);
        }
        at = code.after(at);
    }
    match split {
        Some(split) => (Some((head, split)), (split + 1, end)),
        None => (None, (head, end)),
    }
}

fn last_name(code: &Code, from: usize, to: usize) -> Option<String> {
    let mut depth = 0_usize;
    let mut found = None;
    let mut at = from;
    while at < to {
        if code.is(at, "<") {
            depth += 1;
        } else if code.is(at, ">") && !code.arrow_close(at) {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && let Some(name) = code.name(at)
        {
            found = Some(name.to_string());
        }
        at = code.after(at);
    }
    found
}

fn target_name(code: &Code, from: usize, to: usize) -> Option<String> {
    let mut at = from;
    while at < to
        && (code.is(at, "&") || code.is(at, "mut") || code.kind(at) == Some(crate::lex::Kind::Lifetime))
    {
        at += 1;
    }
    code.name(at)?;
    last_name(code, at, to)
}

pub(crate) fn read(codes: &[Option<Code>], files: &[String]) -> Model {
    let mut model = Model {
        files: files.to_vec(),
        ..Model::default()
    };
    for (file, code) in codes.iter().enumerate() {
        if let Some(code) = code {
            let mut reader = Reader {
                code,
                file,
                model: &mut model,
            };
            reader.items(0, code.len(), None);
        }
    }
    model
}
