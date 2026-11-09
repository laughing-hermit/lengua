//! Round 9's guard G1, for the WebAssembly lengua: the checker's report on every program in the
//! repository and on hundreds of small mutants of each, recorded at one commit and compared with
//! the recording made at another, so that a change to the checker that moves any report shows.
//! The native lengua's guard, on main.s for x86-64 and ARM64, is at the tag `native-final`.
//!
//! The programs are the folders under examples/ and tests/ that hold an idea.md and a main.wat,
//! found by walking both, so a program that moves from one place to another is still recorded. A
//! mutant changes main.wat's text in one place, each line kept at its number: a comparison
//! turned to its opposite (`i32.lt_s` and `i32.ge_s`, `i32.eq` and `i32.ne`), an arithmetic
//! operator swapped for a close one (`add` and `sub`, `and` and `or`), a constant or a memory
//! offset moved by one, one local read in place of another of the same type, a `br_if`'s
//! condition inverted with `i32.eqz`, or a bounds check removed; at most [`MAX_MUTANTS`] of a
//! program, taken evenly from all of its candidates.
//!
//! `LENGUA_GOLDEN=write cargo test --release --lib checker::golden -- --ignored` records the
//! reports under `target/golden/`, in checker-wasm.txt; without the variable the test compares
//! and, when anything differs, writes what it found beside the recording and names the first
//! differences. A run that finds no recording, such as a fresh checkout's, makes one and says in
//! its summary ([`crate::skiplog`]) that it compared nothing.
//!
//! The recordings aren't committed. A baseline is made on GitHub's runners: a `golden` dispatch
//! of ci.yml with `golden_base` at the commit W11 lands on (or a later one) records this golden
//! and the prover's ([`crate::prove`]'s golden) at that commit and at the dispatched ref, each on
//! a runner of its own, and scripts/ci/golden-diff.sh compares them, byte for byte or as the
//! proofs' verdicts alone.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::wasm::{Context, FILE, Report, check};
use crate::hash::Sha256Hex;
use crate::idea::Idea;
use crate::wasm::text::build;

/// The recording's name, under `target/golden/`.
const RECORDING: &str = "checker-wasm.txt";

/// The most mutants made of one program, taken evenly from all of its candidates.
const MAX_MUTANTS: usize = 300;

/// The most checks that run at once, each on a thread of its own.
const AT_ONCE: usize = 16;

/// The fewest programs the goldens take: the 43 that the repository ships, the 15 examples and
/// the 28 fixtures.
const AT_LEAST: usize = 43;

/// Where the programs are, from the repository's root: every folder beneath these that holds an
/// idea.md and a main.wat.
const PLACES: [&str; 2] = ["examples", "tests"];

/// The kinds of mutant, in the order the mutants made at one token are listed.
const KINDS: [&str; 6] = ["brif", "bound", "cmp", "arith", "const", "local"];

/// The types an operator's name starts with: the numbers', and SIMD128's lanes'.
const TYPES: [&str; 11] = [
    "i32", "i64", "f32", "f64", "v128", "i8x16", "i16x8", "i32x4", "i64x2", "f32x4", "f64x2",
];

/// Each comparison's opposite, by the operator's name after its type: the integers', the
/// floats' and the lanes'.
const OPPOSITES: [(&str, &str); 14] = [
    ("eq", "ne"),
    ("ne", "eq"),
    ("lt_s", "ge_s"),
    ("ge_s", "lt_s"),
    ("lt_u", "ge_u"),
    ("ge_u", "lt_u"),
    ("gt_s", "le_s"),
    ("le_s", "gt_s"),
    ("gt_u", "le_u"),
    ("le_u", "gt_u"),
    ("lt", "ge"),
    ("ge", "lt"),
    ("gt", "le"),
    ("le", "gt"),
];

/// The arithmetic operators a mutant swaps for a close one, by the operator's name after its
/// type. Each is an operator of every type that has the first.
const SWAPS: [(&str, &str); 25] = [
    ("add", "sub"),
    ("sub", "add"),
    ("and", "or"),
    ("or", "and"),
    ("xor", "or"),
    ("mul", "add"),
    ("shl", "shr_u"),
    ("shr_u", "shl"),
    ("shr_s", "shr_u"),
    ("rotl", "rotr"),
    ("rotr", "rotl"),
    ("div_s", "div_u"),
    ("div_u", "div_s"),
    ("rem_s", "rem_u"),
    ("rem_u", "rem_s"),
    ("min", "max"),
    ("max", "min"),
    ("min_s", "max_s"),
    ("max_s", "min_s"),
    ("min_u", "max_u"),
    ("max_u", "min_u"),
    ("add_sat_s", "sub_sat_s"),
    ("sub_sat_s", "add_sat_s"),
    ("add_sat_u", "sub_sat_u"),
    ("sub_sat_u", "add_sat_u"),
];

/// The numbers an `i32.const` is written with: from the least signed value to the most unsigned.
const I32_RANGE: (i128, i128) = (-(1 << 31), (1 << 32) - 1);

/// The numbers an `i64.const` is written with.
const I64_RANGE: (i128, i128) = (-(1 << 63), (1 << 64) - 1);

/// The offsets a memory access is written with.
const OFFSET_RANGE: (i128, i128) = (0, (1 << 32) - 1);

/// A program of the repository's: its main.wat, and its idea, which says what it may reach.
pub(crate) struct Program {
    /// Its main.wat's path from the repository's root, with `/` between folders on every system.
    pub(crate) name: String,
    /// The name of its folder.
    pub(crate) folder: String,
    /// main.wat's text.
    pub(crate) text: String,
    /// Its idea, or why it doesn't parse.
    pub(crate) idea: Result<Idea, String>,
}

/// The repository's root.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Puts in `found` every folder beneath `folder` that holds a program: an idea.md and a main.wat.
fn walk(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let path = entry.path();
        walk(&path, found);
        if path.join("main.wat").is_file() && path.join("idea.md").is_file() {
            found.push(path);
        }
    }
}

/// `path`'s name from the repository's `root`, with `/` between folders on every system.
fn name_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every program in the repository, in the order of their folders: each folder under examples/
/// and tests/ with an idea.md and a main.wat.
pub(crate) fn programs() -> Vec<Program> {
    let root = root();
    let mut folders = Vec::new();
    for place in PLACES {
        walk(&root.join(place), &mut folders);
    }
    folders.sort();
    let found: Vec<Program> = folders
        .into_iter()
        .map(|folder| {
            let read = |file: &str| {
                std::fs::read_to_string(folder.join(file))
                    .unwrap_or_else(|error| panic!("{}/{file}: {error}", folder.display()))
            };
            let idea =
                crate::idea::parse(&read("idea.md")).map_err(|problems| format!("{problems:?}"));
            Program {
                name: name_of(&root, &folder.join("main.wat")),
                folder: folder
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                text: read("main.wat"),
                idea,
            }
        })
        .collect();
    assert!(
        found.len() >= AT_LEAST,
        "the goldens found {} programs under {PLACES:?}",
        found.len()
    );
    found
}

/// What a token of main.wat is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `(`, which opens a form.
    Open,
    /// `)`, which closes one.
    Close,
    /// A string, `"…"`.
    Text,
    /// Anything else: a keyword, a name, a number.
    Atom,
}

/// A token of main.wat: what it is, where it starts and ends, and its line, from 1.
#[derive(Debug, Clone, Copy)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
    line: usize,
}

/// Whether `byte` ends an atom.
const fn ends_atom(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' | b'"' | b';'
    )
}

/// main.wat's tokens, without its comments and its white space.
fn tokens(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let (mut at, mut line) = (0, 1);
    while let Some(&byte) = bytes.get(at) {
        let (start, first) = (at, line);
        let kind = match (byte, bytes.get(at + 1).copied()) {
            (b'\n', _) => {
                line += 1;
                at += 1;
                continue;
            }
            (b' ' | b'\t' | b'\r', _) => {
                at += 1;
                continue;
            }
            (b';', Some(b';')) => {
                while bytes.get(at).is_some_and(|byte| *byte != b'\n') {
                    at += 1;
                }
                continue;
            }
            (b'(', Some(b';')) => {
                // A block comment, which may hold others.
                let mut depth = 0_usize;
                while let Some(&inside) = bytes.get(at) {
                    if bytes[at..].starts_with(b"(;") {
                        depth += 1;
                        at += 2;
                    } else if bytes[at..].starts_with(b";)") {
                        depth = depth.saturating_sub(1);
                        at += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if inside == b'\n' {
                            line += 1;
                        }
                        at += 1;
                    }
                }
                continue;
            }
            (b'(', _) => {
                at += 1;
                Kind::Open
            }
            (b')', _) => {
                at += 1;
                Kind::Close
            }
            (b'"', _) => {
                at += 1;
                while let Some(&inside) = bytes.get(at) {
                    if inside == b'\n' {
                        line += 1;
                    }
                    at += if inside == b'\\' { 2 } else { 1 };
                    if inside == b'"' {
                        break;
                    }
                }
                at = at.min(bytes.len());
                Kind::Text
            }
            _ => {
                at += 1;
                while bytes.get(at).is_some_and(|byte| !ends_atom(*byte)) {
                    at += 1;
                }
                Kind::Atom
            }
        };
        found.push(Token {
            kind,
            start,
            end: at,
            line: first,
        });
    }
    found
}

/// For each of `tokens` that opens a form, the token that closes it.
fn closes(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut found = vec![None; tokens.len()];
    let mut open = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Open => open.push(index),
            Kind::Close => {
                if let Some(start) = open.pop() {
                    found[start] = Some(index);
                }
            }
            Kind::Text | Kind::Atom => {}
        }
    }
    found
}

/// main.wat read as forms: its text, its tokens, and where each form closes.
struct Forms<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    closes: Vec<Option<usize>>,
}

impl<'a> Forms<'a> {
    /// `text`, read.
    fn new(text: &'a str) -> Self {
        let tokens = tokens(text);
        let closes = closes(&tokens);
        Self {
            text,
            tokens,
            closes,
        }
    }

    /// The text of the token at `index`, or nothing past the last.
    fn word(&self, index: usize) -> &'a str {
        let text = self.text;
        self.tokens
            .get(index)
            .map_or("", |token| &text[token.start..token.end])
    }

    /// Whether the token at `index` opens a form.
    fn is_form(&self, index: usize) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.kind == Kind::Open)
    }

    /// The token that closes the form the token at `open` opens.
    fn close(&self, open: usize) -> Option<usize> {
        self.closes.get(open).copied().flatten()
    }

    /// The keyword of the form that the token at `open` opens: `br_if` for `(br_if …)`.
    fn head(&self, open: usize) -> &'a str {
        match self.tokens.get(open + 1) {
            Some(token) if token.kind == Kind::Atom => self.word(open + 1),
            _ => "",
        }
    }

    /// What the form that the token at `open` opens holds after its keyword: the index of each
    /// atom and string, and of the `(` of each form, in order.
    fn parts(&self, open: usize) -> Vec<usize> {
        let mut found = Vec::new();
        let Some(close) = self.close(open) else {
            return found;
        };
        let mut at = open + 2;
        while at < close {
            found.push(at);
            at = match self.tokens[at].kind {
                Kind::Open => self.close(at).map_or(close, |end| end + 1),
                Kind::Close | Kind::Text | Kind::Atom => at + 1,
            };
        }
        found
    }

    /// Whether the form that the token at `open` opens holds an atom that `wanted` takes.
    fn holds(&self, open: usize, wanted: impl Fn(&str) -> bool) -> bool {
        let close = self.close(open).unwrap_or(open);
        (open..close).any(|at| self.tokens[at].kind == Kind::Atom && wanted(self.word(at)))
    }

    /// Where the form that the token at `open` opens is in the text: from its `(` to past its
    /// `)`.
    fn span(&self, open: usize) -> Option<(usize, usize)> {
        let close = self.close(open)?;
        Some((self.tokens[open].start, self.tokens[close].end))
    }
}

/// `word` with its operator, the part after its type, made what `pairs` gives it: `i32.lt_s` to
/// `i32.ge_s` by [`OPPOSITES`].
fn swapped(word: &str, pairs: &[(&str, &str)]) -> Option<String> {
    let (kind, operator) = word.split_once('.')?;
    if !TYPES.contains(&kind) {
        return None;
    }
    let (_, other) = pairs.iter().find(|(name, _)| *name == operator)?;
    Some(format!("{kind}.{other}"))
}

/// Whether `word` is a comparison.
fn compares(word: &str) -> bool {
    swapped(word, &OPPOSITES).is_some()
}

/// Whether `word` names a region's size, `$x.size` or `$x.rosize`.
fn region_size(word: &str) -> bool {
    word.starts_with('$') && (word.ends_with(".size") || word.ends_with(".rosize"))
}

/// The number `word` moved by one, written as it was, in decimal or in hexadecimal: up where that
/// stays at most `high`, otherwise down; none where `word` isn't a whole number, or where moving
/// it leaves `low..=high`.
fn moved(word: &str, low: i128, high: i128) -> Option<String> {
    let (negative, unsigned) = match word.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, word.strip_prefix('+').unwrap_or(word)),
    };
    let (hex, digits) = match unsigned.strip_prefix("0x") {
        Some(rest) => (true, rest),
        None => (false, unsigned),
    };
    let digits: String = digits.chars().filter(|digit| *digit != '_').collect();
    let magnitude = i128::from_str_radix(&digits, if hex { 16 } else { 10 }).ok()?;
    let value = if negative { -magnitude } else { magnitude };
    let new = if value < high { value + 1 } else { value - 1 };
    if new < low {
        return None;
    }
    Some(match (hex, new < 0) {
        (true, true) => format!("-0x{:x}", new.unsigned_abs()),
        (true, false) => format!("0x{new:x}"),
        (false, _) => new.to_string(),
    })
}

/// A mutant before it's made: the line it's named for, the token it's made at, its kind, and the
/// changes that make it, each a span of main.wat and what replaces it, in order.
struct Candidate {
    line: usize,
    anchor: usize,
    kind: &'static str,
    changes: Vec<(usize, usize, String)>,
}

/// A mutant of one change, `new` in place of `span`, named for `line`, made at the token
/// `anchor`.
fn single(
    kind: &'static str,
    anchor: usize,
    line: usize,
    span: (usize, usize),
    new: String,
) -> Candidate {
    Candidate {
        line,
        anchor,
        kind,
        changes: vec![(span.0, span.1, new)],
    }
}

/// The locals of the function that the token at `open` opens, its parameters first, each with its
/// type, in order: those that have names.
fn locals<'a>(forms: &Forms<'a>, open: usize) -> Vec<(&'a str, &'a str)> {
    forms
        .parts(open)
        .into_iter()
        .filter(|part| forms.is_form(*part) && matches!(forms.head(*part), "param" | "local"))
        .filter_map(|part| match forms.parts(part).as_slice() {
            [name, kind] if forms.word(*name).starts_with('$') => {
                Some((forms.word(*name), forms.word(*kind)))
            }
            _ => None,
        })
        .collect()
}

/// The local a mutant reads in place of `name`: the next of `locals` after it, round to the
/// first, that has the same type.
fn other_local<'a>(locals: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    let at = locals.iter().position(|(local, _)| *local == name)?;
    let kind = locals[at].1;
    (1..locals.len())
        .map(|step| locals[(at + step) % locals.len()])
        .find(|(other, other_kind)| *other_kind == kind && *other != name)
        .map(|(other, _)| other)
}

/// The mutants of the function that the token at `open` opens that read one local in place of
/// another of the same type: one for each `local.get` of a local that has such another.
fn swapped_locals(forms: &Forms<'_>, open: usize) -> Vec<Candidate> {
    let locals = locals(forms, open);
    let close = forms.close(open).unwrap_or(open);
    (open..close)
        .filter(|at| forms.tokens[*at].kind == Kind::Atom && forms.word(*at) == "local.get")
        .filter_map(|at| {
            let name = forms
                .tokens
                .get(at + 1)
                .filter(|next| next.kind == Kind::Atom)?;
            let other = other_local(&locals, forms.word(at + 1))?;
            Some(single(
                "local",
                at,
                name.line,
                (name.start, name.end),
                other.to_owned(),
            ))
        })
        .collect()
}

/// The mutant of the `br_if` that the token at `open` opens with its condition inverted: the last
/// form it holds, its condition, wrapped in `i32.eqz`; or, where it holds none and takes its
/// condition from the stack, `i32.eqz` before it.
fn inverted(forms: &Forms<'_>, open: usize) -> Candidate {
    let condition = forms
        .parts(open)
        .into_iter()
        .rev()
        .find(|part| forms.is_form(*part));
    match condition.and_then(|part| forms.span(part).map(|span| (part, span))) {
        Some((part, (start, end))) => Candidate {
            line: forms.tokens[part].line,
            anchor: open,
            kind: "brif",
            changes: vec![
                (start, start, "(i32.eqz ".to_owned()),
                (end, end, ")".to_owned()),
            ],
        },
        None => {
            let start = forms.tokens[open].start;
            single(
                "brif",
                open,
                forms.tokens[open].line,
                (start, start),
                "i32.eqz ".to_owned(),
            )
        }
    }
}

/// Where the `if` or `br_if` that the token at `open` opens is, when it's a bounds check: its
/// condition compares a value with a region's size; and an `if` that is one has no `else` and no
/// result, and its `then` ends the path, by an assertion, a branch or a return.
fn bounds_check(forms: &Forms<'_>, open: usize) -> Option<(usize, usize)> {
    let held: Vec<usize> = forms
        .parts(open)
        .into_iter()
        .filter(|part| forms.is_form(*part))
        .collect();
    let condition = if forms.head(open) == "br_if" {
        *held.last()?
    } else {
        if held
            .iter()
            .any(|part| matches!(forms.head(*part), "else" | "result"))
        {
            return None;
        }
        let then = *held.iter().find(|part| forms.head(**part) == "then")?;
        let ends = forms.holds(then, |word| {
            matches!(word, "unreachable" | "br" | "return" | "$assert_fail")
        });
        if !ends {
            return None;
        }
        *held
            .iter()
            .find(|part| !matches!(forms.head(**part), "then" | "param" | "type"))?
    };
    if forms.holds(condition, compares) && forms.holds(condition, region_size) {
        forms.span(open)
    } else {
        None
    }
}

/// `text` with `changes` made: each a span of it, in order, and what replaces it.
fn changed(text: &str, changes: &[(usize, usize, String)]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut at = 0;
    for (start, end, new) in changes {
        out.push_str(&text[at..*start]);
        out.push_str(new);
        at = *end;
    }
    out.push_str(&text[at..]);
    out
}

/// The mutants of main.wat's `text`, each with its name: `L<line>:<kind>`, with a number after
/// the kind for the second of its kind on a line and on. For every comparison, its opposite; for
/// every arithmetic operator of [`SWAPS`], its neighbour; for every `i32.const` and `i64.const`
/// and every memory offset, the number moved by one; for every `local.get`, the next local of its
/// function that has the same type; for every `br_if`, its condition inverted; and for every
/// bounds check, the check removed, its lines left empty. Each keeps every line of `text` at its
/// number. At most [`MAX_MUTANTS`], taken evenly from all of them.
fn mutants(text: &str) -> Vec<(String, String)> {
    let forms = Forms::new(text);
    let mut found = Vec::new();
    for (at, token) in forms.tokens.iter().enumerate() {
        let whole = (token.start, token.end);
        match token.kind {
            Kind::Open => {
                let head = forms.head(at);
                if head == "br_if" {
                    found.push(inverted(&forms, at));
                }
                if matches!(head, "if" | "br_if")
                    && let Some((start, end)) = bounds_check(&forms, at)
                {
                    let newlines = text[start..end].matches('\n').count();
                    found.push(single(
                        "bound",
                        at,
                        token.line,
                        (start, end),
                        "\n".repeat(newlines),
                    ));
                }
                if head == "func" {
                    found.extend(swapped_locals(&forms, at));
                }
            }
            Kind::Atom => {
                let word = forms.word(at);
                if let Some(opposite) = swapped(word, &OPPOSITES) {
                    found.push(single("cmp", at, token.line, whole, opposite));
                }
                if let Some(other) = swapped(word, &SWAPS) {
                    found.push(single("arith", at, token.line, whole, other));
                }
                let range = match word {
                    "i32.const" => Some(I32_RANGE),
                    "i64.const" => Some(I64_RANGE),
                    _ => None,
                };
                if let Some((low, high)) = range
                    && let Some(number) = forms
                        .tokens
                        .get(at + 1)
                        .filter(|next| next.kind == Kind::Atom)
                    && let Some(new) = moved(forms.word(at + 1), low, high)
                {
                    let span = (number.start, number.end);
                    found.push(single("const", at, number.line, span, new));
                }
                if let Some(offset) = word.strip_prefix("offset=")
                    && let Some(new) = moved(offset, OFFSET_RANGE.0, OFFSET_RANGE.1)
                {
                    let new = format!("offset={new}");
                    found.push(single("const", at, token.line, whole, new));
                }
                // A `br_if` written flat, after its condition: `(br_if …)`'s is the form's.
                let folded = at
                    .checked_sub(1)
                    .is_some_and(|before| forms.tokens[before].kind == Kind::Open);
                if word == "br_if" && !folded {
                    let before = (token.start, token.start);
                    found.push(single(
                        "brif",
                        at,
                        token.line,
                        before,
                        "i32.eqz ".to_owned(),
                    ));
                }
            }
            Kind::Close | Kind::Text => {}
        }
    }
    found.sort_by_key(|candidate| {
        let order = KINDS.iter().position(|kind| *kind == candidate.kind);
        (candidate.line, candidate.anchor, order)
    });
    let mut counted: BTreeMap<(usize, &str), usize> = BTreeMap::new();
    let named: Vec<(String, &Candidate)> = found
        .iter()
        .map(|candidate| {
            let count = counted.entry((candidate.line, candidate.kind)).or_default();
            *count += 1;
            let number = if *count == 1 {
                String::new()
            } else {
                count.to_string()
            };
            let id = format!("L{}:{}{number}", candidate.line, candidate.kind);
            (id, candidate)
        })
        .collect();
    let step = named.len().div_ceil(MAX_MUTANTS).max(1);
    named
        .into_iter()
        .step_by(step)
        .map(|(id, candidate)| (id, changed(text, &candidate.changes)))
        .collect()
}

/// The line a mutant is named for: 12 for `L12:cmp`.
fn named_line(id: &str) -> Option<usize> {
    id.strip_prefix('L')?.split_once(':')?.0.parse().ok()
}

/// What the checker says, line by line: each violation, how many more it hides, what the rules
/// learned, and the unsafe blocks.
fn describe(report: &Report) -> String {
    let mut text = String::new();
    for violation in &report.violations {
        let _ = writeln!(text, "{}", violation.in_file(FILE));
    }
    let _ = writeln!(text, "hidden {}", report.hidden);
    let _ = writeln!(text, "facts {:?}", report.facts);
    let _ = writeln!(text, "unsafe {:?}", report.unsafe_blocks);
    text
}

/// The checker's report on main.wat's `text`, with what `idea` allows, as [`describe`] writes it,
/// or why it doesn't build.
fn report(text: &str, idea: &Idea) -> String {
    let built = match build(text) {
        Ok(built) => built,
        Err(problem) => return format!("build failed: {problem}\n"),
    };
    let context = Context {
        uses: &idea.uses,
        network: &idea.network,
        secrets: &idea.secrets,
    };
    describe(&check(text, &built, &context))
}

/// The report on each of `texts`, by its key, with what `idea` allows: as many at once as the
/// computer has threads, up to [`AT_ONCE`].
fn reports(texts: &BTreeMap<&str, &str>, idea: &Idea) -> BTreeMap<String, String> {
    let jobs: Vec<(&str, &str)> = texts.iter().map(|(key, text)| (*key, *text)).collect();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(AT_ONCE);
    let (jobs, next) = (&jobs, &next);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(move || {
                    let mut found = Vec::new();
                    while let Some(&(key, text)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                        found.push((key.to_owned(), report(text, idea)));
                    }
                    found
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a check ran"))
            .collect()
    })
}

/// The recording: each program's report in full, then a hash of each of its mutants'.
fn record() -> String {
    let mut recording = String::new();
    for program in programs() {
        let idea = match &program.idea {
            Ok(idea) => idea,
            Err(why) => {
                let _ = writeln!(recording, "== {} (0 mutants)", program.name);
                let _ = writeln!(recording, "  the idea doesn't parse: {why}");
                continue;
            }
        };
        let base = ("base".to_owned(), program.text.clone());
        let all: Vec<(String, String)> = std::iter::once(base)
            .chain(mutants(&program.text))
            .collect();
        // Every text is checked once, by its hash, whichever of the mutants share it.
        let keys: Vec<String> = all
            .iter()
            .map(|(_, text)| Sha256Hex::of(text.as_bytes()).as_str().to_owned())
            .collect();
        let texts: BTreeMap<&str, &str> = keys
            .iter()
            .zip(&all)
            .map(|(key, (_, text))| (key.as_str(), text.as_str()))
            .collect();
        let reports = reports(&texts, idea);
        let _ = writeln!(recording, "== {} ({} mutants)", program.name, all.len() - 1);
        for ((id, _), key) in all.iter().zip(&keys) {
            let found = &reports[key];
            if id == "base" {
                for line in found.lines() {
                    let _ = writeln!(recording, "  {line}");
                }
            } else {
                let hash = Sha256Hex::of(found.as_bytes());
                let _ = writeln!(recording, "{id} {}", &hash.as_str()[..16]);
            }
        }
    }
    recording
}

/// Whether this run records the goldens afresh: `LENGUA_GOLDEN=write`.
fn writing() -> bool {
    std::env::var("LENGUA_GOLDEN").as_deref() == Ok("write")
}

/// Where the recording called `name` lives.
fn recording_path(name: &str) -> PathBuf {
    root().join("target/golden").join(name)
}

/// Writes `found` as the recording at `path`.
fn record_at(path: &Path, found: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("make target/golden");
    std::fs::write(path, found).expect("record");
    eprintln!(
        "recorded {} ({} lines)",
        path.display(),
        found.lines().count()
    );
}

/// Compares `found` with the recording called `name`, or records it under
/// `LENGUA_GOLDEN=write`. Where there is no recording, as on a fresh checkout, it makes one and
/// notes in the run's summary that it compared nothing: the run is a smoke test of the guard's
/// machinery, and the guard around a change needs the recording of the commit before it.
pub(crate) fn compare_or_record(name: &str, found: &str) {
    let path = recording_path(name);
    if writing() {
        record_at(&path, found);
        return;
    }
    let Ok(recorded) = std::fs::read_to_string(&path) else {
        record_at(&path, found);
        crate::skiplog::note(&format!(
            "{name} had no recording to compare with (LENGUA_GOLDEN=write makes one at the commit \
             before a change): this run recorded it and compared nothing"
        ));
        return;
    };
    if recorded == found {
        return;
    }
    let beside = path.with_extension("found.txt");
    std::fs::write(&beside, found).expect("write what was found");
    let differences: Vec<String> = recorded
        .lines()
        .zip(found.lines())
        .filter(|(old, new)| old != new)
        .take(30)
        .map(|(old, new)| format!("- {old}\n+ {new}"))
        .collect();
    panic!(
        "{name} differs from its recording ({} lines then, {} now); what was found is in {}:\n{}",
        recorded.lines().count(),
        found.lines().count(),
        beside.display(),
        differences.join("\n")
    );
}

/// The guard: every program's report, and those of its mutants, are as recorded. Where there is
/// no recording to compare with, the reports are made twice and must be the same, which at least
/// shows the guard is deterministic.
#[test]
#[ignore = "the checker's golden: run with --ignored around a change to the checker"]
fn every_report_is_as_recorded() {
    let fresh = !writing() && !recording_path(RECORDING).is_file();
    let found = record();
    compare_or_record(RECORDING, &found);
    if fresh {
        let again = record();
        assert!(
            again == found,
            "{RECORDING}: two runs of the checker on the same programs gave different reports"
        );
    }
}

/// The skill's template, whose mutants the next test looks at.
const TEMPLATE: &str = include_str!("../../plugin/skills/compile/assets/main-template.wat");

#[test]
fn mutants_are_made_the_same_way_every_time() {
    let (first, second) = (mutants(TEMPLATE), mutants(TEMPLATE));
    assert_eq!(first, second);
    assert!(
        first.len() > 50 && first.len() <= MAX_MUTANTS,
        "{} mutants",
        first.len()
    );
    let kinds: BTreeSet<&str> = first
        .iter()
        .filter_map(|(id, _)| id.split_once(':'))
        .map(|(_, kind)| kind.trim_end_matches(|character: char| character.is_ascii_digit()))
        .collect();
    for kind in KINDS {
        assert!(kinds.contains(kind), "{kind} in {kinds:?}");
    }
    // Each mutant keeps every line at its number, and changes the line it's named for.
    let lines: Vec<&str> = TEMPLATE.lines().collect();
    for (id, mutant) in &first {
        let mutated: Vec<&str> = mutant.lines().collect();
        assert_eq!(mutated.len(), lines.len(), "{id} keeps every line");
        let line = named_line(id).expect("a line in its name");
        assert_ne!(mutated[line - 1], lines[line - 1], "{id} changes its line");
    }
}

/// A function small enough that the next test names its every mutant.
const SMALL: &str = "(module
  (global $x.size i32 (i32.const 16))
  (func $f (param $a i32) (param $b i32) (result i32)
    (block $out
      (br_if $out (i32.ge_u (local.get $a) (global.get $x.size))))
    (i32.add (local.get $a) (i32.const 41))))
";

/// Each mutant changes what it's named for, on its line: the constant moved by one, the `br_if`'s
/// condition inverted, the bounds check removed, the comparison turned round, the other `i32`
/// local read, and the addition made a subtraction.
#[test]
fn each_mutant_changes_what_it_is_named_for() {
    let found = mutants(SMALL);
    let ids: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "L2:const", "L5:brif", "L5:bound", "L5:cmp", "L5:local", "L6:arith", "L6:local",
            "L6:const"
        ]
    );
    let line = |id: &str| -> String {
        let (_, mutant) = found
            .iter()
            .find(|(name, _)| name == id)
            .expect("the mutant");
        let number = named_line(id).expect("a line in its name");
        mutant
            .lines()
            .nth(number - 1)
            .unwrap_or_default()
            .to_owned()
    };
    assert_eq!(line("L2:const"), "  (global $x.size i32 (i32.const 17))");
    assert_eq!(
        line("L5:brif"),
        "      (br_if $out (i32.eqz (i32.ge_u (local.get $a) (global.get $x.size)))))"
    );
    assert_eq!(line("L5:bound"), "      )");
    assert_eq!(
        line("L5:cmp"),
        "      (br_if $out (i32.lt_u (local.get $a) (global.get $x.size))))"
    );
    assert_eq!(
        line("L5:local"),
        "      (br_if $out (i32.ge_u (local.get $b) (global.get $x.size))))"
    );
    assert_eq!(
        line("L6:arith"),
        "    (i32.sub (local.get $a) (i32.const 41))))"
    );
    assert_eq!(
        line("L6:local"),
        "    (i32.add (local.get $b) (i32.const 41))))"
    );
    assert_eq!(
        line("L6:const"),
        "    (i32.add (local.get $a) (i32.const 42))))"
    );
}

/// A constant moves by one as it's written, and back where up would leave its type's range.
#[test]
fn a_constant_moves_by_one_as_it_is_written() {
    assert_eq!(moved("41", I32_RANGE.0, I32_RANGE.1).as_deref(), Some("42"));
    assert_eq!(moved("-1", I32_RANGE.0, I32_RANGE.1).as_deref(), Some("0"));
    assert_eq!(
        moved("0x7f", I32_RANGE.0, I32_RANGE.1).as_deref(),
        Some("0x80")
    );
    assert_eq!(
        moved("-0x10", I32_RANGE.0, I32_RANGE.1).as_deref(),
        Some("-0xf")
    );
    assert_eq!(
        moved("1_000", I64_RANGE.0, I64_RANGE.1).as_deref(),
        Some("1001")
    );
    assert_eq!(
        moved("0xffffffff", I32_RANGE.0, I32_RANGE.1).as_deref(),
        Some("0xfffffffe")
    );
    assert_eq!(moved("1.5", I32_RANGE.0, I32_RANGE.1), None);
}

/// The goldens take every program folder once, each by its main.wat's path, and every idea
/// parses.
#[test]
fn the_goldens_take_every_program_folder_once() {
    let found = programs();
    let names: BTreeSet<&str> = found.iter().map(|program| program.name.as_str()).collect();
    assert_eq!(names.len(), found.len(), "{names:?}");
    for program in &found {
        assert!(program.name.ends_with("/main.wat"), "{}", program.name);
        assert!(
            program.idea.is_ok(),
            "{}: {:?}",
            program.name,
            program.idea.as_ref().err()
        );
    }
}

/// Prints the report on one program or mutant, for a golden that differs:
/// `LENGUA_GOLDEN_MUTANT="examples/sort/main.wat L217:cmp"` (or `base`), with the
/// mutant's line beside the program's. Without the variable it notes so and ends, as in a run of
/// every ignored test.
#[test]
#[ignore = "explains a golden's difference"]
fn explains_a_mutant() {
    let Ok(wanted) = std::env::var("LENGUA_GOLDEN_MUTANT") else {
        crate::skiplog::note("explains_a_mutant: LENGUA_GOLDEN_MUTANT names no mutant to explain");
        return;
    };
    let (name, id) = wanted.split_once(' ').expect("a program and a mutant");
    let program = programs()
        .into_iter()
        .find(|program| program.name == name)
        .expect("a program in the golden");
    let idea = program.idea.as_ref().expect("its idea parses");
    let text = if id == "base" {
        program.text.clone()
    } else {
        let (_, mutant) = mutants(&program.text)
            .into_iter()
            .find(|(found, _)| found == id)
            .expect("a mutant of it");
        if let Some(line) = named_line(id) {
            let at = |text: &str| {
                text.lines()
                    .nth(line.saturating_sub(1))
                    .unwrap_or_default()
                    .to_owned()
            };
            eprintln!("main.wat:{line}:   {}", at(&program.text));
            eprintln!("the mutant's: {}", at(&mutant));
        }
        mutant
    };
    eprintln!("{}", report(&text, idea));
}
