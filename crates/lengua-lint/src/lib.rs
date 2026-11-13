mod code;
mod graph;
mod lex;
mod loops;
mod model;
mod rules;

use std::collections::BTreeSet;

use crate::code::Code;
use crate::model::Model;
use crate::rules::{ASSERTS, Context};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub file: String,
    pub idea: String,
    pub text: String,
    pub structure: Vec<(usize, String)>,
}

pub const LINES: usize = 60;

const NESTED: [&str; 4] = ["impl", "trait", "mod", "macro_rules"];

fn file_rules(
    model: &Model,
    crates: &[String],
    file: usize,
    code: &Code,
    name: &str,
    out: &mut Vec<Finding>,
) {
    let context = Context {
        model,
        crates,
        file,
        code,
    };
    let mut hits = Vec::new();
    context.comments(&mut hits);
    context.tokens(&mut hits);
    context.macro_definitions(&mut hits);
    for found in model.macros.iter().filter(|m| m.file == file) {
        for problem in loops::check(code, found.body.0 + 1, found.body.1) {
            hits.push(rules::Hit {
                line: code.line(problem.at),
                code: "rule-2-loop",
                message: problem.message,
            });
        }
    }
    out.extend(hits.into_iter().map(|h| Finding {
        file: name.to_string(),
        line: h.line,
        code: h.code,
        message: h.message,
    }));
}

fn finding(file: &str, line: usize, code: &'static str, message: String) -> Finding {
    Finding {
        file: file.to_string(),
        line,
        code,
        message,
    }
}

fn function_rules(model: &Model, codes: &[Option<Code>], out: &mut Vec<Finding>) {
    for function in &model.functions {
        let Some(code) = codes.get(function.file).and_then(Option::as_ref) else {
            continue;
        };
        let file = model.files.get(function.file).map_or("", String::as_str);
        let (open, close) = function.body;
        for problem in loops::check(code, open + 1, close) {
            out.push(finding(
                file,
                code.line(problem.at),
                "rule-2-loop",
                problem.message,
            ));
        }
        for at in open + 1..close {
            let nested_fn = code.is(at, "fn") && code.name(at + 1).is_some();
            let nested_item = code.ident(at).is_some_and(|w| NESTED.contains(&w));
            if nested_fn || nested_item {
                out.push(finding(
                    file,
                    code.line(at),
                    "rule-12-nested",
                    format!("`{}` inside the function `{}`: write functions, impls, traits, modules and macros at the module's top level, so that `# Structure` names them", code.text(at), function.key),
                ));
            }
        }
        let lines = function.end + 1 - function.line;
        if lines > LINES {
            out.push(finding(
                file,
                function.line,
                "rule-4-length",
                format!("the function `{}` has {lines} lines, and a function has {LINES} or fewer; split it into smaller functions", function.key),
            ));
        }
    }
}

fn assertions_in(code: &Code, open: usize, close: usize) -> usize {
    (open + 1..close)
        .filter(|at| {
            code.name(*at).is_some_and(|n| ASSERTS.contains(&n))
                && code.macro_call(at + 1)
                && !code.after_dot(*at)
                && !code.after_path(*at)
        })
        .count()
}

fn assertions(model: &Model, codes: &[Option<Code>], out: &mut Vec<Finding>) {
    let mut counted: Vec<(usize, usize)> = Vec::new();
    for (id, function) in model.functions.iter().enumerate() {
        if function.owner.is_none() && function.key == "main" {
            continue;
        }
        if let Some(code) = codes.get(function.file).and_then(Option::as_ref) {
            counted.push((id, assertions_in(code, function.body.0, function.body.1)));
        }
    }
    let total: usize = counted.iter().map(|(_, n)| n).sum();
    let wanted = 2 * counted.len();
    if total >= wanted {
        return;
    }
    let Some((id, few)) = counted.iter().find(|(_, n)| *n < 2) else {
        return;
    };
    let Some(function) = model.functions.get(*id) else {
        return;
    };
    out.push(finding(
        model.files.get(function.file).map_or("", String::as_str),
        function.line,
        "rule-5-assertions",
        format!(
            "the program has {} functions besides `main` and {total} assertions (`assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, ...) in them, and it needs two a function on average, so {wanted}; `{}` has {few}",
            counted.len(),
            function.key
        ),
    ));
}

fn recursion(model: &Model, codes: &[Option<Code>], out: &mut Vec<Finding>) {
    let index = graph::Index::new(model, codes);
    let edges: Vec<BTreeSet<usize>> = (0..model.functions.len())
        .map(|id| index.edges(id))
        .collect();
    for (id, function) in model.functions.iter().enumerate() {
        let Some(path) = graph::cycle(&edges, id) else {
            continue;
        };
        let names: Vec<String> = path
            .iter()
            .skip(1)
            .filter_map(|p| model.functions.get(*p))
            .map(|f| format!("`{}`", f.key))
            .collect();
        let how = if names.is_empty() {
            "calls itself".to_string()
        } else {
            format!("calls itself again through {}", names.join(", then "))
        };
        out.push(finding(
            model.files.get(function.file).map_or("", String::as_str),
            function.line,
            "rule-1-recursion",
            format!("the function `{}` {how}, and code has no recursion; write it with a loop. lengua reads a method call on a value whose type it does not know as a call to every method of that name, so if this is not a recursion, call the method as `Type::method(...)`, or on `self`, a field of `self` or a parameter", function.key),
        ));
    }
}

fn structure(sources: &[Source], model: &Model, codes: &[Option<Code>], out: &mut Vec<Finding>) {
    for (file, source) in sources.iter().enumerate() {
        if codes.get(file).and_then(Option::as_ref).is_none() {
            continue;
        }
        let mut keys: Vec<(&str, usize)> = Vec::new();
        for function in model.functions.iter().filter(|f| f.file == file) {
            if !keys.iter().any(|(k, _)| *k == function.key.as_str()) {
                keys.push((function.key.as_str(), function.line));
            }
        }
        for (key, line) in &keys {
            if !source.structure.iter().any(|(_, n)| n.as_str() == *key) {
                out.push(finding(
                    &source.file,
                    *line,
                    "rule-12-unnamed",
                    format!("the function `{key}` is not in the `# Structure` of `{}`; add ``- `{key}`: <one sentence>.``", source.idea),
                ));
            }
        }
        for (line, name) in &source.structure {
            if !keys.iter().any(|(k, _)| *k == name.as_str()) {
                out.push(finding(
                    &source.idea,
                    *line,
                    "rule-12-unknown",
                    format!("`{name}` is in the `# Structure`, and `{}` has no function `{name}`; remove it, or write the function", source.file),
                ));
            }
        }
    }
}

#[must_use]
pub fn lint(sources: &[Source], crates: &[String]) -> Vec<Finding> {
    let files: Vec<String> = sources.iter().map(|s| s.file.clone()).collect();
    let mut findings = Vec::new();
    let mut codes: Vec<Option<Code>> = Vec::new();
    for source in sources {
        match code::read(&source.text) {
            Ok(read) => codes.push(Some(read)),
            Err(error) => {
                findings.push(finding(
                    &source.file,
                    error.line,
                    "rust-read",
                    format!(
                        "lengua's reader of Rust can't read the file: {}",
                        error.message
                    ),
                ));
                codes.push(None);
            }
        }
    }
    let mut model = model::read(&codes, &files);
    findings.append(&mut model.findings);
    for (file, code) in codes.iter().enumerate() {
        if let (Some(code), Some(name)) = (code, files.get(file)) {
            file_rules(&model, crates, file, code, name, &mut findings);
        }
    }
    function_rules(&model, &codes, &mut findings);
    assertions(&model, &codes, &mut findings);
    recursion(&model, &codes, &mut findings);
    structure(sources, &model, &codes, &mut findings);
    findings.sort();
    findings.dedup();
    findings
}
