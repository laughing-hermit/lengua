use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::code::Code;
use crate::lex::Kind;
use crate::model::{Function, Model, segment_end};

pub(crate) struct Index<'a> {
    model: &'a Model,
    codes: &'a [Option<Code>],
    free: BTreeMap<String, Vec<usize>>,
    keyed: BTreeMap<String, Vec<usize>>,
    methods: BTreeMap<String, Vec<usize>>,
    implementors: BTreeMap<String, Vec<String>>,
    implemented: BTreeMap<String, Vec<String>>,
}

fn method_name(key: &str) -> &str {
    key.rsplit("::").next().unwrap_or(key)
}

impl<'a> Index<'a> {
    pub(crate) fn new(model: &'a Model, codes: &'a [Option<Code>]) -> Self {
        let mut index = Index {
            model,
            codes,
            free: BTreeMap::new(),
            keyed: BTreeMap::new(),
            methods: BTreeMap::new(),
            implementors: BTreeMap::new(),
            implemented: BTreeMap::new(),
        };
        for (id, function) in model.functions.iter().enumerate() {
            if function.owner.is_some() {
                index.keyed.entry(function.key.clone()).or_default().push(id);
                index
                    .methods
                    .entry(method_name(&function.key).to_string())
                    .or_default()
                    .push(id);
            } else {
                index.free.entry(function.key.clone()).or_default().push(id);
            }
        }
        for owner in &model.owners {
            if let Some(name) = owner.trait_name.as_ref().filter(|_| !owner.is_trait) {
                index
                    .implementors
                    .entry(name.clone())
                    .or_default()
                    .push(owner.target.clone());
                index
                    .implemented
                    .entry(owner.target.clone())
                    .or_default()
                    .push(name.clone());
            }
        }
        index
    }

    fn keyed(&self, key: &str, out: &mut BTreeSet<usize>) {
        if let Some(ids) = self.keyed.get(key) {
            out.extend(ids.iter().copied());
        }
    }

    fn every_method(&self, method: &str, out: &mut BTreeSet<usize>) {
        if let Some(ids) = self.methods.get(method) {
            out.extend(ids.iter().copied());
        }
    }

    fn on_type(&self, name: &str, method: &str, out: &mut BTreeSet<usize>) {
        self.keyed(&format!("{name}::{method}"), out);
        for found in self.implemented.get(name).into_iter().flatten() {
            self.keyed(&format!("{found}::{method}"), out);
        }
    }

    fn on_trait(&self, name: &str, method: &str, out: &mut BTreeSet<usize>) {
        self.keyed(&format!("{name}::{method}"), out);
        for target in self.implementors.get(name).into_iter().flatten() {
            self.keyed(&format!("{target}::{method}"), out);
        }
    }

    fn on_self(&self, function: &Function, method: &str, out: &mut BTreeSet<usize>) {
        let Some(owner) = function.owner.and_then(|o| self.model.owners.get(o)) else {
            return;
        };
        if owner.is_trait {
            self.on_trait(&owner.target, method, out);
        } else {
            self.on_type(&owner.target, method, out);
        }
    }

    fn generic(&self, function: &Function, name: &str) -> bool {
        function.generics.iter().any(|g| g == name)
            || function
                .owner
                .and_then(|o| self.model.owners.get(o))
                .is_some_and(|o| o.generics.iter().any(|g| g == name))
    }

    fn on_name(&self, function: &Function, name: &str, method: &str, out: &mut BTreeSet<usize>) {
        if name == "Self" {
            self.on_self(function, method, out);
        } else if self.generic(function, name) || self.model.traits.contains(name) {
            self.every_method(method, out);
        } else if let Some(names) = self.model.aliases.get(name) {
            for aliased in names {
                self.on_type(aliased, method, out);
            }
        } else {
            self.on_type(name, method, out);
        }
    }
}

struct Scope<'a> {
    function: &'a Function,
    bindings: BTreeSet<String>,
    params: BTreeMap<String, Option<Vec<String>>>,
}

fn pattern_names(code: &Code, from: usize, to: usize, out: &mut BTreeSet<String>) {
    for at in from..to {
        if let Some(name) = code.name(at)
            && !code.open(at + 1)
            && !code.path_sep(at + 1)
            && !code.after_path(at)
        {
            out.insert(name.to_string());
        }
    }
}

fn let_end(code: &Code, from: usize, to: usize) -> usize {
    let mut at = from;
    while at < to {
        if code.single_eq(at) || code.is(at, ";") || code.colon(at) || code.is(at, "else") {
            return at;
        }
        at = code.after(at);
    }
    to
}

fn closure_start(code: &Code, at: usize) -> bool {
    if !code.is(at, "|") || at == 0 {
        return false;
    }
    let before = at - 1;
    match code.kind(before) {
        Some(Kind::Punct) => matches!(code.text(before), "(" | "," | "=" | "{" | ";" | "[" | ">"),
        Some(Kind::Ident) => matches!(code.text(before), "move" | "return" | "in" | "else"),
        _ => false,
    }
}

fn arms(code: &Code, open: usize, out: &mut BTreeSet<String>) {
    let close = code.end(open);
    let mut at = open + 1;
    while at < close {
        let mut arrow = at;
        while arrow < close && !code.joint(arrow, "=", ">") {
            arrow = code.after(arrow);
        }
        pattern_names(code, at, arrow, out);
        let mut next = arrow + 2;
        if code.is(next, "{") {
            next = code.after(next);
            if code.is(next, ",") {
                next += 1;
            }
        } else {
            while next < close && !code.is(next, ",") {
                next = code.after(next);
            }
            next += 1;
        }
        at = next;
    }
}

fn bindings(code: &Code, function: &Function) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let (from, to) = (function.body.0 + 1, function.body.1);
    for at in from..to {
        match code.ident(at) {
            Some("let") => pattern_names(code, at + 1, let_end(code, at + 1, to), &mut out),
            Some("for") => {
                let end = code.find(at + 1, to, "in").unwrap_or(at + 1);
                pattern_names(code, at + 1, end, &mut out);
            }
            Some("match") => {
                if let Some(open) = code.find(at + 1, to, "{") {
                    arms(code, open, &mut out);
                }
            }
            _ if closure_start(code, at) && !code.joint(at, "|", "|") => {
                let end = (at + 1..to).find(|k| code.is(*k, "|")).unwrap_or(at + 1);
                pattern_names(code, at + 1, end, &mut out);
            }
            _ => {}
        }
    }
    out
}

fn params(code: &Code, function: &Function) -> BTreeMap<String, Option<Vec<String>>> {
    let mut out = BTreeMap::new();
    let (open, close) = function.params;
    let mut at = open + 1;
    while at < close {
        let end = segment_end(code, at, close);
        let colon = (at..end).find(|k| code.colon(*k)).unwrap_or(end);
        let mut names = BTreeSet::new();
        pattern_names(code, at, colon, &mut names);
        let opaque = (colon..end).any(|k| code.is(k, "impl") || code.is(k, "dyn"));
        let types = (!opaque).then(|| code.names(colon + 1, end));
        for name in names {
            out.insert(name, types.clone());
        }
        at = end + 1;
    }
    out
}

impl Index<'_> {
    fn scope<'f>(&self, function: &'f Function) -> Option<Scope<'f>> {
        let code = self.codes.get(function.file)?.as_ref()?;
        Some(Scope {
            function,
            bindings: bindings(code, function),
            params: params(code, function),
        })
    }

    fn field_types(&self, scope: &Scope, field: &str) -> Option<Vec<String>> {
        let owner = scope
            .function
            .owner
            .and_then(|o| self.model.owners.get(o))
            .filter(|o| !o.is_trait)?;
        let def = self.model.types.get(&owner.target)?;
        let types = def
            .fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, types)| types.clone())?;
        let generic = types.iter().any(|t| def.generics.contains(t));
        (!generic).then_some(types)
    }

    fn receiver(&self, code: &Code, scope: &Scope, dot: usize) -> Option<Vec<String>> {
        let before = dot.checked_sub(1)?;
        if code.is(before, "self") && !code.after_dot(before) {
            return Some(vec!["Self".to_string()]);
        }
        let field = code.name(before).or_else(|| {
            (code.kind(before) == Some(Kind::Integer)).then(|| code.text(before))
        })?;
        if before >= 2 && code.after_dot(before) && code.is(before - 2, "self") && !code.after_dot(before - 2) {
            return self.field_types(scope, field);
        }
        if code.after_dot(before) || code.after_path(before) {
            return None;
        }
        scope.params.get(field).cloned().flatten()
    }

    fn method(&self, code: &Code, scope: &Scope, dot: usize, method: &str, out: &mut BTreeSet<usize>) {
        match self.receiver(code, scope, dot) {
            Some(types) if !types.is_empty() => {
                for name in &types {
                    self.on_name(scope.function, name, method, out);
                }
            }
            _ => self.every_method(method, out),
        }
    }

    fn path(&self, scope: &Scope, segments: &[String], out: &mut BTreeSet<usize>) {
        let (Some(last), Some(head)) = (segments.last(), segments.iter().rev().nth(1)) else {
            return;
        };
        let first = segments.first().map_or("", String::as_str);
        if head.starts_with(|c: char| c.is_uppercase()) {
            self.on_name(scope.function, head, last, out);
        } else if !matches!(first, "std" | "core" | "alloc")
            && let Some(ids) = self.free.get(last)
        {
            out.extend(ids.iter().copied());
        }
    }

    fn single(&self, code: &Code, scope: &Scope, at: usize, end: usize, out: &mut BTreeSet<usize>) {
        let name = code.text(at);
        let Some(ids) = self.free.get(name.strip_prefix("r#").unwrap_or(name)) else {
            return;
        };
        let called = code.is(end, "(");
        let value = !scope.bindings.contains(name)
            && !scope.params.contains_key(name)
            && !code.colon(end)
            && !code.single_eq(end)
            && !matches!(code.ident(at.wrapping_sub(1)), Some("let" | "mut" | "ref" | "fn"));
        if called || value {
            out.extend(ids.iter().copied());
        }
    }

    fn scan(&self, code: &Code, scope: &Scope, range: (usize, usize), inline: bool, out: &mut BTreeSet<usize>) {
        for at in range.0..range.1 {
            if code.is(at, ".") && !code.after_dot(at) && !code.joint(at, ".", ".") {
                if let Some(method) = code.name(at + 1)
                    && (code.is(at + 2, "(") || code.path_sep(at + 2))
                {
                    self.method(code, scope, at, method, out);
                }
                continue;
            }
            if code.segment(at).is_none() || code.after_dot(at) || code.after_path(at) {
                continue;
            }
            let (segments, end) = code.path(at);
            if code.macro_call(end) {
                if inline && segments.len() == 1 {
                    self.inline_macro(scope, &segments[0], out);
                }
                continue;
            }
            if segments.len() == 1 {
                self.single(code, scope, at, end, out);
            } else {
                self.path(scope, &segments, out);
            }
        }
    }

    fn inline_macro(&self, scope: &Scope, name: &str, out: &mut BTreeSet<usize>) {
        for found in self.model.macros.iter().filter(|m| m.name == name) {
            if let Some(code) = self.codes.get(found.file).and_then(Option::as_ref) {
                self.scan(code, scope, (found.body.0 + 1, found.body.1), false, out);
            }
        }
    }

    pub(crate) fn edges(&self, id: usize) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        let Some(function) = self.model.functions.get(id) else {
            return out;
        };
        let (Some(scope), Some(code)) = (
            self.scope(function),
            self.codes.get(function.file).and_then(Option::as_ref),
        ) else {
            return out;
        };
        self.scan(code, &scope, (function.body.0 + 1, function.body.1), true, &mut out);
        out
    }
}

pub(crate) fn cycle(graph: &[BTreeSet<usize>], start: usize) -> Option<Vec<usize>> {
    let mut parent: BTreeMap<usize, usize> = BTreeMap::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    for next in graph.get(start).into_iter().flatten() {
        if *next == start {
            return Some(vec![start]);
        }
        if parent.insert(*next, start).is_none() {
            queue.push_back(*next);
        }
    }
    while let Some(node) = queue.pop_front() {
        for next in graph.get(node).into_iter().flatten() {
            if *next == start {
                let mut path = vec![node];
                let mut at = node;
                while let Some(back) = parent.get(&at).copied().filter(|b| *b != start) {
                    path.push(back);
                    at = back;
                }
                path.push(start);
                path.reverse();
                return Some(path);
            }
            if !parent.contains_key(next) {
                parent.insert(*next, node);
                queue.push_back(*next);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::cycle;

    #[test]
    fn a_cycle_is_found_with_its_path() {
        let graph: Vec<BTreeSet<usize>> = vec![
            [1].into_iter().collect(),
            [2].into_iter().collect(),
            [0, 3].into_iter().collect(),
            BTreeSet::new(),
            [4].into_iter().collect(),
        ];
        assert_eq!(cycle(&graph, 0), Some(vec![0, 1, 2]));
        assert_eq!(cycle(&graph, 3), None);
        assert_eq!(cycle(&graph, 4), Some(vec![4]));
    }
}
