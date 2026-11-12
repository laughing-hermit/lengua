//! Named data: where each named text of main.wat's data is, and the numbers that say so.
//!
//! main.wat stays plain WebAssembly text, and the one text that is checked and run. Its comments
//! carry the names:
//!
//! - `;; text: NAME` on the line of one string of a `data` segment names that string's bytes; on
//!   the line where a segment starts, when no string of it is on that line, it names all of the
//!   segment's bytes.
//! - `;; at: NAME`, `;; offset: NAME` and `;; len: NAME` on the line of a constant global say
//!   what its number is: the text's address, how many bytes it is from the start of its region,
//!   or how many bytes it has.
//!
//! The checker computes each named text's place and length from the module it built (a
//! segment's address and its bytes) and from the strings of the segment as `wast` read them, so
//! no one adds lengths by hand; it holds every marked number to them, a number that differs
//! being a violation of rule 8 that says the right one; and `check`'s reply carries the table
//! ([`Table::lines`]). A marker is a comment: it changes nothing of the module, and a number
//! with no marker is checked by nothing here, as before.

use std::collections::BTreeMap;

use super::model::{Global, Module};
use crate::checker::{Rule, Violation};

/// The marker that names a text.
const TEXT: &str = "text:";

/// A number's markers, each with what it says the number is.
const NUMBERS: [(&str, Kind); 3] = [
    ("at:", Kind::Address),
    ("offset:", Kind::Offset),
    ("len:", Kind::Length),
];

/// The most names a problem lists of the texts main.wat has.
const MAX_LISTED: usize = 10;

/// What a marked number says of its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Its address.
    Address,
    /// How many bytes it is from the start of its region.
    Offset,
    /// How many bytes it has.
    Length,
}

/// A named text of main.wat's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// Its name, as `;; text:` gives it.
    pub name: String,
    /// The line of the string, or of the segment, that it names.
    pub line: usize,
    /// Its first byte's address.
    pub address: u64,
    /// Its region, the one that starts nearest at or below its address, by name without the `$`,
    /// with how many bytes it is from the region's start; `None` for data below every region. Data
    /// that a read-only region doesn't hold is rule M6's to report.
    pub region: Option<(String, u64)>,
    /// The data segment it is in, by index.
    pub segment: u32,
    /// How many bytes it has.
    pub len: u64,
}

/// The named texts of main.wat, and how its marked numbers stand to them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    /// The named texts, in the order of their addresses.
    pub texts: Vec<Text>,
    /// How many numbers are marked with a text's name and are right.
    pub right: usize,
    /// How many markers are wrong: a number that differs, a name no text has, a name used
    /// twice, or a marker that names nothing.
    pub wrong: usize,
    /// How many texts no marked number names.
    pub unused: usize,
}

impl Table {
    /// The table for `check`'s reply: nothing for a main.wat with no named text; one line when
    /// every text is named by a number and every marked number is right; else one line for each
    /// text, with its address, its place in its region and its length, so that the numbers can
    /// be written from the reply.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        if self.texts.is_empty() {
            return Vec::new();
        }
        let count = self.texts.len();
        let texts = if count == 1 { "text" } else { "texts" };
        if self.wrong == 0 && self.unused == 0 {
            return vec![format!(
                "{count} named {texts}: every number marked `;; at:`, `;; offset:` or `;; len:` is \
                 its text's ({} marked)",
                self.right
            )];
        }
        let mut lines = vec![format!(
            "{count} named {texts}, each with its address, its offset in its region and its \
             length in bytes: write each number marked `;; at:`, `;; offset:` or `;; len:` from \
             its line"
        )];
        lines.extend(self.texts.iter().map(|text| {
            let place = match &text.region {
                Some((region, offset)) => format!("offset {offset} in ${region}"),
                None => "in no region".to_owned(),
            };
            format!(
                "{}: at {}, {place}, len {}",
                text.name, text.address, text.len
            )
        }));
        lines
    }
}

/// The name a marker gives: the first word after it.
fn name_of(rest: &str) -> Option<&str> {
    rest.split_whitespace().next()
}

/// The named texts of `module`: each `;; text:` marker with the bytes it names.
#[allow(clippy::too_many_lines)]
fn named(module: &Module<'_>, violations: &mut Vec<Violation>, wrong: &mut usize) -> Vec<Text> {
    let mut problem = |line: usize, problem: String, fix: &str| {
        *wrong += 1;
        violations.push(Violation {
            rule: Rule::Constants,
            line: Some(line),
            problem,
            fix: fix.to_owned(),
        });
    };
    // The region a text is in: the one that starts nearest at or below its address, whether or
    // not its size holds the text yet, since the size may be a number still to write.
    let region_of = |address: u64| {
        module
            .regions
            .iter()
            .filter(|region| region.start <= address)
            .max_by_key(|region| region.start)
            .map(|region| (region.name.clone(), address - region.start))
    };
    let marked = |line: usize| {
        module
            .notes
            .marker(line, TEXT)
            .map(|rest| name_of(rest).unwrap_or_default().to_owned())
    };
    let mut texts: Vec<Text> = Vec::new();
    // The lines whose marker names something.
    let mut used: Vec<usize> = Vec::new();
    for data in module.data.iter().filter(|data| data.own) {
        let Some((_, Some(start))) = data.at else {
            continue;
        };
        // Each string of the segment, with how far into the segment it starts.
        let mut strings: Vec<(usize, u64, u64)> = Vec::new();
        let mut offset = 0_u64;
        for piece in &data.pieces {
            if let Some(line) = piece.line {
                strings.push((line, offset, piece.len));
            }
            offset = offset.saturating_add(piece.len);
        }
        let mut by_line: BTreeMap<usize, Vec<(u64, u64)>> = BTreeMap::new();
        for (line, offset, len) in strings {
            by_line.entry(line).or_default().push((offset, len));
        }
        // The segment's own line, with no string of it: the marker names all of it.
        if let Some(line) = data.line
            && !by_line.contains_key(&line)
            && let Some(name) = marked(line)
        {
            used.push(line);
            if data.pieces.is_empty() && data.len > 0 {
                // Its strings weren't read one for one, so a string of it may be on this line.
                problem(
                    line,
                    format!(
                        "lengua can't tell the strings of this data segment apart, so `;; text: \
                         {name}` names nothing"
                    ),
                    "write the segment as `(data (i32.const <address>) \"…\")`, with no name \
                     annotation",
                );
            } else {
                texts.push(Text {
                    name,
                    line,
                    address: start,
                    region: region_of(start),
                    segment: data.index,
                    len: data.len,
                });
            }
        }
        for (line, on_line) in by_line {
            let Some(name) = marked(line) else {
                continue;
            };
            used.push(line);
            if let [(offset, len)] = on_line.as_slice() {
                let address = start.saturating_add(*offset);
                texts.push(Text {
                    name,
                    line,
                    address,
                    region: region_of(address),
                    segment: data.index,
                    len: *len,
                });
            } else {
                problem(
                    line,
                    format!(
                        "`;; text: {name}` is on a line with {} strings of a data segment, so it \
                         names none of them",
                        on_line.len()
                    ),
                    "put each string that has a name on a line of its own, with its `;; text: \
                     NAME`",
                );
            }
        }
    }
    for line in module.notes.comments.keys() {
        if marked(*line).is_some() && !used.contains(line) {
            problem(
                *line,
                "this `;; text:` names nothing: no string of a data segment is on its line, and \
                 no data segment starts there"
                    .to_owned(),
                "put `;; text: NAME` on the line of one string of a `data` segment, or on the \
                 line where a segment starts, to name all of it",
            );
        }
    }
    texts.sort_by_key(|text| (text.address, text.line));
    let mut first: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept = Vec::new();
    for text in texts {
        if text.name.is_empty() {
            problem(
                text.line,
                "this `;; text:` gives no name".to_owned(),
                "write a name after it, as in `;; text: greeting`",
            );
        } else if let Some(line) = first.get(&text.name) {
            problem(
                text.line.max(*line),
                format!(
                    "the name `{}` is given to two texts (also on line {})",
                    text.name,
                    text.line.min(*line)
                ),
                "give each named text a name of its own",
            );
        } else {
            first.insert(text.name.clone(), text.line);
            kept.push(text);
        }
    }
    // Two named texts of two segments that share bytes: one segment is written over the other,
    // as when a text grew and the segment after it kept its address.
    for (at, text) in kept.iter().enumerate() {
        let end = text.address.saturating_add(text.len);
        for next in kept.iter().skip(at + 1) {
            if next.address >= end {
                break;
            }
            if next.segment != text.segment {
                problem(
                    next.line,
                    format!(
                        "the text `{}` starts at address {}, inside the text `{}`, which ends \
                         before {end}",
                        next.name, next.address, text.name
                    ),
                    "start its segment where the text before it ends, or write the strings in \
                     one `data` segment, where each starts where the one before it ends",
                );
            }
        }
    }
    kept
}

/// The number that `kind` says a global marked with `text`'s name holds; `None` for the offset
/// of a text in no region.
fn expected(kind: Kind, text: &Text) -> Option<u64> {
    match kind {
        Kind::Address => Some(text.address),
        Kind::Offset => text.region.as_ref().map(|(_, offset)| *offset),
        Kind::Length => Some(text.len),
    }
}

/// What a number that differs is told: what `global` holds, and what `text` has.
fn differs(kind: Kind, global: &Global, held: Option<u64>, text: &Text) -> String {
    let shown = global.shown();
    let held = held.map_or_else(|| "no constant".to_owned(), |held| held.to_string());
    let name = &text.name;
    match (kind, &text.region) {
        (Kind::Address, _) => format!(
            "{shown} is {held}, and the text `{name}` is at address {}",
            text.address
        ),
        (Kind::Offset, Some((region, offset))) => {
            format!("{shown} is {held}, and the text `{name}` is {offset} bytes into ${region}")
        }
        (Kind::Offset, None) => {
            format!("{shown} is marked `;; offset: {name}`, and the text `{name}` is in no region")
        }
        (Kind::Length, _) => {
            format!(
                "{shown} is {held}, and the text `{name}` has {} bytes",
                text.len
            )
        }
    }
}

/// Reads main.wat's named texts, holds every marked number to them (each that differs is a
/// violation of rule 8 that says the right number), and gives the table.
#[allow(clippy::too_many_lines)]
pub fn check(module: &Module<'_>, violations: &mut Vec<Violation>) -> Table {
    let mut wrong = 0;
    let texts = named(module, violations, &mut wrong);
    let mut right = 0;
    let mut named_by: Vec<&str> = Vec::new();
    let mut problem = |line: usize, problem: String, fix: String| {
        wrong += 1;
        violations.push(Violation {
            rule: Rule::Constants,
            line: Some(line),
            problem,
            fix,
        });
    };
    let known = || {
        let mut names: Vec<&str> = texts
            .iter()
            .take(MAX_LISTED)
            .map(|text| text.name.as_str())
            .collect();
        if texts.len() > MAX_LISTED {
            names.push("…");
        }
        names.join(", ")
    };
    // The lines whose marker a global took.
    let mut used: Vec<usize> = Vec::new();
    for global in module.globals.iter().filter(|global| !global.imported) {
        let Some(line) = global.line else {
            continue;
        };
        for (key, kind) in NUMBERS {
            let Some(rest) = module.notes.marker(line, key) else {
                continue;
            };
            used.push(line);
            let name = name_of(rest).unwrap_or_default();
            let Some(text) = texts.iter().find(|text| text.name == name) else {
                let fix = if texts.is_empty() {
                    format!("name a string of a `data` segment with `;; text: {name}` on its line")
                } else {
                    format!(
                        "name a string of a `data` segment with `;; text: {name}` on its line, \
                         or use the name of a text main.wat has: {}",
                        known()
                    )
                };
                problem(
                    line,
                    format!("`;; {key} {name}` names a text that main.wat doesn't have"),
                    fix,
                );
                continue;
            };
            let held = global.unsigned();
            let wanted = expected(kind, text);
            if held.is_some() && held == wanted {
                right += 1;
                named_by.push(text.name.as_str());
                continue;
            }
            let fix = match wanted {
                Some(wanted) if global.constant() => format!(
                    "write `({}.const {wanted})`: `check`'s reply lists every named text's \
                     address, offset and length under `data`",
                    if global.ty == wasmparser::ValType::I64 {
                        "i64"
                    } else {
                        "i32"
                    }
                ),
                Some(_) => "mark a global that is a constant and never changes".to_owned(),
                None => "place the text's data in a read-only region".to_owned(),
            };
            problem(line, differs(kind, global, held, text), fix);
        }
    }
    for (line, comments) in &module.notes.comments {
        if used.contains(line) {
            continue;
        }
        for (key, _) in NUMBERS {
            if comments.iter().any(|comment| comment.starts_with(key)) {
                problem(
                    *line,
                    format!("this `;; {key}` marks nothing: no global is defined on its line"),
                    format!(
                        "put `;; {key} NAME` on the line of the global whose constant is that \
                         number, as in `(global $GREETING i32 (i32.const 0))  ;; {key} greeting`"
                    ),
                );
            }
        }
    }
    let unused = texts
        .iter()
        .filter(|text| !named_by.contains(&text.name.as_str()))
        .count();
    Table {
        texts,
        right,
        wrong,
        unused,
    }
}
