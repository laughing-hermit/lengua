use crate::Source;
use crate::code::{self, Code};
use crate::lex::Kind;
use crate::model::{self, Function};
use crate::rules::ASSERTS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    pub file: String,
    pub line: usize,
    pub function: String,
    pub function_text: String,
    pub site: usize,
    pub from: String,
    pub to: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mutation {
    pub mutants: Vec<Mutant>,
    pub sites: usize,
    pub skeleton: String,
}

struct Site {
    line: usize,
    from: String,
    to: String,
    edits: Vec<Edit>,
}

const OPERATORS: [(&str, &str); 17] = [
    ("+", "-"),
    ("-", "+"),
    ("*", "/"),
    ("/", "*"),
    ("%", "*"),
    ("<", "<="),
    ("<=", "<"),
    (">", ">="),
    (">=", ">"),
    ("==", "!="),
    ("!=", "=="),
    ("&&", "||"),
    ("||", "&&"),
    ("+=", "-="),
    ("-=", "+="),
    ("*=", "/="),
    ("/=", "*="),
];

const DEFAULTS: [&str; 19] = [
    "bool",
    "char",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "f32",
    "f64",
    "String",
    "&str",
    "&'static str",
];

#[must_use]
pub fn apply(text: &str, edits: &[Edit]) -> String {
    let mut sorted: Vec<&Edit> = edits.iter().collect();
    sorted.sort_by(|a, b| b.start.cmp(&a.start).then(b.end.cmp(&a.end)));
    let mut out = text.to_string();
    for edit in sorted {
        let fits = edit.start <= edit.end
            && edit.end <= out.len()
            && out.is_char_boundary(edit.start)
            && out.is_char_boundary(edit.end);
        if fits {
            out.replace_range(edit.start..edit.end, &edit.text);
        }
    }
    out
}

fn joined(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn line_start(text: &str, line: usize) -> usize {
    text.split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum()
}

fn replaced(code: &Code, at: usize, to: String) -> Option<Site> {
    let token = code.tokens.get(at)?;
    Some(Site {
        line: token.line,
        from: token.text.clone(),
        to: to.clone(),
        edits: vec![Edit {
            start: token.start,
            end: token.end,
            text: to,
        }],
    })
}

fn operand_end(code: &Code, at: usize) -> bool {
    match code.kind(at) {
        Some(Kind::Integer | Kind::Literal) => true,
        Some(Kind::Ident) => {
            code.name(at).is_some() || matches!(code.text(at), "self" | "Self" | "true" | "false")
        }
        Some(Kind::Punct) => matches!(code.text(at), ")" | "]" | "?"),
        _ => false,
    }
}

fn operand_start(code: &Code, at: usize) -> bool {
    match code.kind(at) {
        Some(Kind::Integer | Kind::Literal | Kind::Ident) => true,
        Some(Kind::Punct) => matches!(code.text(at), "(" | "[" | "!" | "-" | "&" | "*"),
        _ => false,
    }
}

fn operator(text: &str, code: &Code, at: usize) -> Option<Site> {
    let first = code.tokens.get(at)?;
    let glued = at > 0
        && code
            .tokens
            .get(at - 1)
            .is_some_and(|p| p.kind == Kind::Punct && p.joint && p.end == first.start);
    if glued {
        return None;
    }
    let mut last = at;
    let mut op = first.text.clone();
    while op.len() < 3 {
        let (Some(this), Some(after)) = (code.tokens.get(last), code.tokens.get(last + 1)) else {
            break;
        };
        if !(this.joint && after.kind == Kind::Punct && after.start == this.end) {
            break;
        }
        op.push_str(&after.text);
        last += 1;
    }
    let (_, to) = OPERATORS.iter().find(|(from, _)| *from == op)?;
    let end = code.tokens.get(last)?.end;
    let bytes = text.as_bytes();
    let spaced = first.start > 0
        && bytes
            .get(first.start - 1)
            .is_some_and(u8::is_ascii_whitespace)
        && bytes.get(end).is_some_and(u8::is_ascii_whitespace);
    if !spaced || at == 0 || !operand_end(code, at - 1) || !operand_start(code, last + 1) {
        return None;
    }
    Some(Site {
        line: first.line,
        from: op,
        to: (*to).to_string(),
        edits: vec![Edit {
            start: first.start,
            end,
            text: (*to).to_string(),
        }],
    })
}

fn integer(code: &Code, at: usize) -> Option<Site> {
    let token = code.tokens.get(at)?;
    let based = ["0x", "0o", "0b"].iter().any(|p| token.text.starts_with(p));
    if based || code.after_dot(at) {
        return None;
    }
    let split = token
        .text
        .find(|c: char| !(c.is_ascii_digit() || c == '_'))
        .unwrap_or(token.text.len());
    let (digits, suffix) = token.text.split_at(split);
    let value: u128 = digits.replace('_', "").parse().ok()?;
    replaced(code, at, format!("{}{suffix}", value.checked_add(1)?))
}

fn negate(text: &str, code: &Code, at: usize) -> Option<Site> {
    if code.is(at + 1, "let") {
        return None;
    }
    let mut scan = at + 1;
    let open = loop {
        if scan >= code.len() {
            return None;
        }
        if code.is(scan, "{") {
            break scan;
        }
        let stop = [";", ",", ")", "]", "}"].iter().any(|s| code.is(scan, s))
            || code.joint(scan, "=", ">");
        if stop {
            return None;
        }
        scan = code.after(scan);
    };
    if open <= at + 1 {
        return None;
    }
    let first = code.tokens.get(at + 1)?;
    let last = code.tokens.get(open - 1)?;
    let condition = joined(text.get(first.start..last.end)?);
    Some(Site {
        line: first.line,
        from: format!("if {condition}"),
        to: format!("if !({condition})"),
        edits: vec![
            Edit {
                start: first.start,
                end: first.start,
                text: "!(".to_string(),
            },
            Edit {
                start: last.end,
                end: last.end,
                text: ")".to_string(),
            },
        ],
    })
}

fn default_return(text: &str, code: &Code, function: &Function) -> Option<Site> {
    let (_, params_close) = function.params;
    let (open, close) = function.body;
    if !code.joint(params_close + 1, "-", ">") {
        return None;
    }
    let first = code.tokens.get(params_close + 3)?;
    let mut last = params_close + 3;
    let mut at = last;
    while at < open && !code.is(at, "where") {
        last = at;
        at += 1;
    }
    let end = code.tokens.get(last)?.end;
    let written = joined(text.get(first.start..end)?);
    let fits = DEFAULTS.contains(&written.as_str())
        || written.starts_with("Option<")
        || written.starts_with("Vec<");
    if !fits {
        return None;
    }
    let open_token = code.tokens.get(open)?;
    let close_token = code.tokens.get(close)?;
    Some(Site {
        line: function.line,
        from: "the body".to_string(),
        to: "Default::default()".to_string(),
        edits: vec![
            Edit {
                start: open_token.end,
                end: open_token.end,
                text: " let _ = {".to_string(),
            },
            Edit {
                start: close_token.start,
                end: close_token.start,
                text: "}; Default::default() ".to_string(),
            },
        ],
    })
}

fn site(text: &str, code: &Code, at: usize) -> Option<Site> {
    let token = code.tokens.get(at)?;
    match token.kind {
        Kind::Integer => integer(code, at),
        Kind::Ident if token.text == "true" => replaced(code, at, "false".to_string()),
        Kind::Ident if token.text == "false" => replaced(code, at, "true".to_string()),
        Kind::Ident if token.text == "if" => negate(text, code, at),
        Kind::Punct => operator(text, code, at),
        _ => None,
    }
}

fn assertion_spans(code: &Code, open: usize, close: usize) -> Vec<(usize, usize)> {
    (open + 1..close)
        .filter(|at| {
            code.name(*at).is_some_and(|n| ASSERTS.contains(&n))
                && code.macro_call(at + 1)
                && !code.after_dot(*at)
        })
        .map(|at| (at, code.end(at + 2)))
        .collect()
}

fn function_mutants(source: &Source, code: &Code, function: &Function) -> Vec<Mutant> {
    let (open, close) = function.body;
    let text = source.text.as_str();
    let begin = line_start(text, function.line);
    let finish = code.tokens.get(close).map_or(text.len(), |t| t.end);
    let function_text = text.get(begin..finish).unwrap_or("").to_string();
    let mut found: Vec<Site> = default_return(text, code, function).into_iter().collect();
    let skipped = assertion_spans(code, open, close);
    for at in open + 1..close {
        if skipped.iter().any(|(a, b)| at >= *a && at <= *b) {
            continue;
        }
        if let Some(one) = site(text, code, at) {
            found.push(one);
        }
    }
    found
        .into_iter()
        .enumerate()
        .map(|(site, one)| Mutant {
            file: source.file.clone(),
            line: one.line,
            function: function.key.clone(),
            function_text: function_text.clone(),
            site,
            from: one.from,
            to: one.to,
            edits: one.edits,
        })
        .collect()
}

#[must_use]
pub fn mutants(sources: &[Source], cap: usize) -> Mutation {
    let mut groups: Vec<Vec<Mutant>> = Vec::new();
    let mut skeleton = String::new();
    for source in sources {
        let Ok(read) = code::read(&source.text) else {
            continue;
        };
        let codes = [Some(read)];
        let files = [source.file.clone()];
        let model = model::read(&codes, &files);
        let Some(Some(code)) = codes.first() else {
            continue;
        };
        let mut bodies = Vec::new();
        for function in &model.functions {
            let (open, close) = function.body;
            if let (Some(a), Some(b)) = (code.tokens.get(open), code.tokens.get(close)) {
                bodies.push(Edit {
                    start: a.start,
                    end: b.end,
                    text: "{}".to_string(),
                });
            }
            groups.push(function_mutants(source, code, function));
        }
        skeleton.push_str(&source.file);
        skeleton.push('\n');
        skeleton.push_str(&apply(&source.text, &bodies));
    }
    let sites = groups.iter().map(Vec::len).sum();
    let longest = groups.iter().map(Vec::len).max().unwrap_or(0);
    let mut chosen: Vec<Mutant> = Vec::new();
    'rounds: for round in 0..longest {
        for group in &groups {
            if chosen.len() >= cap {
                break 'rounds;
            }
            if let Some(one) = group.get(round) {
                chosen.push(one.clone());
            }
        }
    }
    chosen.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.site.cmp(&b.site))
    });
    Mutation {
        mutants: chosen,
        sites,
        skeleton,
    }
}

#[cfg(test)]
mod tests {
    use super::{apply, mutants};
    use crate::Source;

    fn source(text: &str) -> Source {
        Source {
            file: "src/main.rs".to_string(),
            idea: "idea.md".to_string(),
            text: text.to_string(),
            structure: Vec::new(),
        }
    }

    fn pairs(text: &str, cap: usize) -> Vec<(String, String)> {
        mutants(&[source(text)], cap)
            .mutants
            .into_iter()
            .map(|m| (m.from, m.to))
            .collect()
    }

    fn pair(from: &str, to: &str) -> (String, String) {
        (from.to_string(), to.to_string())
    }

    const MAIN: &str = "fn main() {\n    let a = 1 + 2;\n    if a < 3 {\n        println!(\"{}\", true);\n    }\n    assert!(a > 0);\n}\n";

    #[test]
    fn operators_constants_booleans_and_conditions_are_mutated_and_assertions_are_not() {
        assert_eq!(
            pairs(MAIN, 40),
            [
                pair("1", "2"),
                pair("+", "-"),
                pair("2", "3"),
                pair("if a < 3", "if !(a < 3)"),
                pair("<", "<="),
                pair("3", "4"),
                pair("true", "false"),
            ]
        );
    }

    #[test]
    fn a_mutant_applies_its_edits_to_the_text() {
        let found = mutants(&[source(MAIN)], 40);
        let negated = found
            .mutants
            .iter()
            .find(|m| m.from.starts_with("if "))
            .expect("a negated condition");
        let text = apply(MAIN, &negated.edits);
        assert!(text.contains("    if !(a < 3) {\n"), "{text}");
        assert_eq!(negated.line, 3);
        assert_eq!(negated.function, "main");
        assert!(negated.function_text.starts_with("fn main() {\n"));
        assert!(negated.function_text.ends_with("assert!(a > 0);\n}"));
        assert_eq!(found.skeleton, "src/main.rs\nfn main() {}\n");
    }

    #[test]
    fn generics_tuple_fields_and_paths_are_not_operators() {
        let text = "fn f(v: Vec<u8>, t: (u8, u8)) -> usize {\n    let w: Vec<u8> = v;\n    let n = t.0;\n    w.len() + usize::from(n)\n}\n";
        assert_eq!(
            pairs(text, 40),
            [pair("the body", "Default::default()"), pair("+", "-")]
        );
        let found = mutants(&[source(text)], 40);
        let body = apply(text, &found.mutants[0].edits);
        assert!(
            body.starts_with("fn f(v: Vec<u8>, t: (u8, u8)) -> usize { let _ = {\n"),
            "{body}"
        );
        assert!(body.ends_with("}; Default::default() }\n"), "{body}");
    }

    #[test]
    fn ranges_suffixes_and_compound_operators() {
        let text = "fn main() {\n    let mut n = 0_u8;\n    for _ in 0..10 {\n        n += 1;\n    }\n    if n >= 2 && n != 3 {\n        n -= 1;\n    }\n}\n";
        let found = pairs(text, 40);
        for wanted in [
            pair("0_u8", "1u8"),
            pair("0", "1"),
            pair("10", "11"),
            pair("+=", "-="),
            pair(">=", ">"),
            pair("&&", "||"),
            pair("!=", "=="),
            pair("-=", "+="),
        ] {
            assert!(found.contains(&wanted), "{wanted:?} in {found:?}");
        }
        assert!(!found.iter().any(|(from, _)| from == ".."), "{found:?}");
    }

    #[test]
    fn the_cap_spreads_the_mutants_over_the_functions() {
        let text = "fn main() {\n    let a = 1 + 2 + 3 + 4;\n    println!(\"{a}\");\n}\n\nfn other() -> bool {\n    assert!(true);\n    assert!(true);\n    false\n}\n";
        let found = mutants(&[source(text)], 3);
        assert_eq!(found.sites, 9);
        let functions: Vec<&str> = found.mutants.iter().map(|m| m.function.as_str()).collect();
        assert_eq!(functions, ["main", "main", "other"]);
    }
}
