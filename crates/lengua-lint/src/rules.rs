use std::collections::BTreeSet;

use crate::code::Code;
use crate::lex::Kind;
use crate::model::Model;

pub(crate) const STD_MACROS: [&str; 33] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "column",
    "compile_error",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "include_bytes",
    "include_str",
    "line",
    "matches",
    "module_path",
    "option_env",
    "panic",
    "print",
    "println",
    "stringify",
    "todo",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln",
];

pub(crate) const ASSERTS: [&str; 6] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
];

const STD_DERIVES: [&str; 9] = [
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Hash",
];

const ATTRIBUTES: [&str; 8] = [
    "must_use",
    "inline",
    "cold",
    "repr",
    "non_exhaustive",
    "track_caller",
    "forbid",
    "deny",
];

const CFG_WORDS: [&str; 15] = [
    "all",
    "any",
    "not",
    "target_arch",
    "target_os",
    "target_family",
    "target_env",
    "target_abi",
    "target_vendor",
    "target_pointer_width",
    "target_endian",
    "target_feature",
    "target_has_atomic",
    "unix",
    "windows",
];

const STATIC_TYPES: [&str; 18] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64", "bool", "char", "str", "Option",
];

const ITEM_WORDS: [&str; 13] = [
    "fn",
    "struct",
    "enum",
    "union",
    "impl",
    "trait",
    "mod",
    "static",
    "const",
    "type",
    "use",
    "extern",
    "macro_rules",
];

pub(crate) struct Context<'a> {
    pub model: &'a Model,
    pub crates: &'a [String],
    pub file: usize,
    pub code: &'a Code,
}

pub(crate) struct Hit {
    pub line: usize,
    pub code: &'static str,
    pub message: String,
}

fn hit(line: usize, code: &'static str, message: String) -> Hit {
    Hit {
        line,
        code,
        message,
    }
}

impl Context<'_> {
    fn declared(&self, name: &str) -> bool {
        self.crates.iter().any(|c| c.replace('-', "_") == name)
    }

    fn imported(&self, name: &str) -> bool {
        self.model.uses.iter().any(|u| {
            u.file == self.file
                && self.declared(&u.first)
                && (u.glob || u.names.iter().any(|n| n == name))
        })
    }

    fn local_macro(&self, name: &str) -> bool {
        self.model.macros.iter().any(|m| m.name == name)
    }

    fn line(&self, at: usize) -> usize {
        self.code.line(at)
    }

    pub(crate) fn comments(&self, out: &mut Vec<Hit>) {
        for comment in &self.code.comments {
            let what = if comment.kind == Kind::Shebang {
                "a shebang line"
            } else {
                "a comment"
            };
            out.push(hit(
                comment.line,
                "rule-11-comment",
                format!("the code has {what}; code files have no comments and no doc comments, and the idea is the documentation, so say it in the idea's `# Structure` or `# Purpose`"),
            ));
        }
    }

    fn cfg(&self, open: usize, out: &mut Vec<Hit>) {
        for at in open + 1..self.code.end(open) {
            if let Some(word) = self.code.ident(at)
                && !CFG_WORDS.contains(&word)
            {
                out.push(hit(
                    self.line(at),
                    "rule-8-cfg",
                    format!("`cfg` with `{word}`: a `cfg` names only the target (`target_os`, `target_arch`, `target_family`, ...), so that one build is the program"),
                ));
            }
        }
    }

    fn derive(&self, open: usize, out: &mut Vec<Hit>) {
        let code = self.code;
        let mut at = open + 1;
        let close = code.end(open);
        while at < close {
            let (segments, end) = code.path(at);
            let first = segments.first().map_or("", String::as_str);
            let last = segments.last().map_or("", String::as_str);
            let std = segments.len() == 1 || matches!(first, "std" | "core");
            let fine = (std && STD_DERIVES.contains(&last))
                || self.declared(first)
                || (segments.len() == 1 && self.imported(last));
            if !fine {
                out.push(hit(
                    self.line(at),
                    "rule-8-derive",
                    format!("the derive `{}` is not one of the standard library's and is not from a crate that the idea declares", segments.join("::")),
                ));
            }
            at = end.max(at + 1);
            while at < close && !code.is(at, ",") {
                at = code.after(at);
            }
            at += 1;
        }
    }

    fn attribute(&self, at: usize, out: &mut Vec<Hit>) {
        let code = self.code;
        let inner = code.is(at + 1, "!");
        let open = if inner { at + 2 } else { at + 1 };
        let (segments, end) = code.path(open + 1);
        let first = segments.first().map_or("", String::as_str);
        let line = self.line(at);
        match first {
            "allow" | "warn" | "expect" => out.push(hit(
                line,
                "rule-10-allow",
                format!("`#[{first}]` turns a warning off, and every warning is an error from the first build; change the code so that the warning goes away"),
            )),
            "doc" => out.push(hit(
                line,
                "rule-11-comment",
                "a `#[doc]` attribute is a doc comment; the idea is the documentation".to_string(),
            )),
            "cfg" if code.is(end, "(") => self.cfg(end, out),
            "derive" if !inner && code.is(end, "(") => self.derive(end, out),
            _ if (ATTRIBUTES.contains(&first) && (!inner || matches!(first, "forbid" | "deny")))
                || (!inner
                    && (self.declared(first) || (segments.len() == 1 && self.imported(first)))) => {}
            _ => out.push(hit(
                line,
                "rule-8-attribute",
                format!("the attribute `{}` is not one that lengua allows: only `derive`, `cfg` of the target, `must_use`, `inline`, `cold`, `repr`, `non_exhaustive`, `track_caller`, `forbid`, `deny`, and attributes of a crate that the idea declares", segments.join("::")),
            )),
        }
    }

    fn static_type(&self, name: &str, seen: &mut BTreeSet<String>) -> Option<String> {
        if STATIC_TYPES.contains(&name) || !seen.insert(name.to_string()) {
            return None;
        }
        let Some(def) = self.model.types.get(name) else {
            return Some(name.to_string());
        };
        def.fields
            .iter()
            .flat_map(|(_, types)| types.iter())
            .filter(|t| !def.generics.contains(t))
            .find_map(|t| self.static_type(t, seen))
    }

    fn static_item(&self, at: usize, out: &mut Vec<Hit>) {
        let code = self.code;
        let line = self.line(at);
        if code.is(at + 1, "mut") {
            out.push(hit(
                line,
                "rule-6-static",
                "a `static mut` is data that every function can change; keep the data in the smallest scope, and pass it".to_string(),
            ));
            return;
        }
        if !code.colon(at + 2) {
            return;
        }
        let mut end = at + 3;
        while end < code.len() && !code.single_eq(end) && !code.is(end, ";") {
            end = code.after(end);
        }
        let mut seen = BTreeSet::new();
        if let Some(name) = (at + 3..end)
            .filter(|k| !code.path_sep(k + 1))
            .filter_map(|k| code.name(k))
            .find_map(|n| self.static_type(n, &mut seen))
        {
            out.push(hit(
                line,
                "rule-6-static",
                format!("lengua can't tell that a `static` of `{name}` has no interior mutability (as `Cell`, `RefCell`, `Mutex`, atomics and `OnceLock` have); a `static` holds numbers, `bool`, `char`, `str`, `Option`, arrays, tuples and the program's own types of them, or use a `const`"),
            ));
        }
    }

    fn macro_use(&self, segments: &[String], open: usize, at: usize, out: &mut Vec<Hit>) {
        let first = segments.first().map_or("", String::as_str);
        let last = segments.last().map_or("", String::as_str);
        let line = self.line(at);
        let std = segments.len() == 1 || matches!(first, "std" | "core" | "alloc");
        let allowed = (std && STD_MACROS.contains(&last))
            || self.declared(first)
            || (segments.len() == 1 && (self.local_macro(last) || self.imported(last)))
            || (matches!(first, "crate" | "self" | "super") && self.local_macro(last));
        if std && last == "thread_local" {
            out.push(hit(
                line,
                "rule-6-static",
                "`thread_local!` makes a global with interior mutability; keep the data in the smallest scope, and pass it".to_string(),
            ));
        } else if std && last == "include" {
            out.push(hit(
                line,
                "rule-8-macro",
                "`include!` brings in code from a file with no idea; write the code in the module's own file".to_string(),
            ));
        } else if std && last == "cfg" {
            self.cfg(open, out);
        } else if !allowed {
            out.push(hit(
                line,
                "rule-8-macro",
                format!("the macro `{}!` is not one of the standard library's, of the program, or of a crate that the idea declares", segments.join("::")),
            ));
        }
    }

    pub(crate) fn tokens(&self, out: &mut Vec<Hit>) {
        let code = self.code;
        for at in 0..code.len() {
            let line = self.line(at);
            if code.is(at, "dyn") {
                out.push(hit(
                    line,
                    "rule-1-dyn",
                    "a `dyn` trait object calls code that lengua can't see in the call graph; use a generic or an `enum`".to_string(),
                ));
            } else if code.is(at, "fn") && code.is(at + 1, "(") {
                out.push(hit(
                    line,
                    "rule-1-fn-pointer",
                    "a function pointer calls code that lengua can't see in the call graph; call the function by its name, or use a generic or an `enum`".to_string(),
                ));
            } else if code.is(at, "#") && (code.is(at + 1, "[") || (code.is(at + 1, "!") && code.is(at + 2, "["))) {
                self.attribute(at, out);
            } else if code.is(at, "static") {
                self.static_item(at, out);
            } else if code.segment(at).is_some() && !code.after_path(at) && !code.after_dot(at) {
                let (segments, end) = code.path(at);
                if code.macro_call(end) && segments.first().is_some_and(|s| s != "macro_rules") {
                    self.macro_use(&segments, end + 1, at, out);
                }
            }
        }
    }

    pub(crate) fn macro_definitions(&self, out: &mut Vec<Hit>) {
        let code = self.code;
        for found in self.model.macros.iter().filter(|m| m.file == self.file) {
            let (open, close) = found.body;
            let items = (open + 1..close).find(|k| code.ident(*k).is_some_and(|w| ITEM_WORDS.contains(&w)));
            let nested = (open + 1..close).find(|k| {
                code.name(*k).is_some_and(|n| self.local_macro(n)) && code.macro_call(*k + 1)
            });
            if let Some(at) = items {
                out.push(hit(
                    found.line,
                    "rule-8-macro-rules",
                    format!("the macro `{}!` writes `{}`, so it defines items that lengua's reader can't see; write the items as code", found.name, code.text(at)),
                ));
            } else if nested.is_some() {
                out.push(hit(
                    found.line,
                    "rule-8-macro-rules",
                    format!("the macro `{}!` uses a macro of the program; a macro of the program uses only the standard library's macros", found.name),
                ));
            }
        }
    }
}
