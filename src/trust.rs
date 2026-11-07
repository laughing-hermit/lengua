//! An idea's trust record, as the Open Knowledge Format keeps it (OKF v0.2 §5.2): `generated`
//! says who wrote the idea, and `verified` lists who confirmed it, each entry an actor and a
//! time. lengua adds its own entry to `verified` once an idea's code passes its checks, with the
//! hashes of the idea and the code it checked, and keeps everyone else's.
//!
//! The record isn't the idea, so lengua's hash of an idea leaves it out: writing `verified` or
//! `generated` changes no lock and no header. Each edit here changes lines of the frontmatter's
//! text and never writes its YAML again, so the other keys and the body stay byte for byte, and
//! so do the comments: only a comment inside a flow collection goes on a line of its own, and a
//! comment at the margin that would no longer be the record's moves in.

use serde::Deserialize;

use crate::hash::Sha256Hex;

/// The top-level frontmatter keys that hold the trust record, which an idea's hash leaves out.
pub const TRUST_KEYS: [&str; 2] = ["verified", "generated"];

/// lengua as an actor (OKF §7): its name and its version, such as `lengua/0.2.0`.
pub const ACTOR: &str = concat!("lengua/", env!("CARGO_PKG_VERSION"));

/// How the actor of each of lengua's entries starts, whatever its version.
pub const LENGUA_PREFIX: &str = "lengua/";

/// The key lengua's entry goes under.
const VERIFIED: &str = "verified";

/// The indentation of the dashes in a list that lengua starts.
const INDENT: &str = "  ";

/// The ways YAML writes no value.
const NULLS: [&str; 4] = ["~", "null", "Null", "NULL"];

/// What a key written plain can't start with: a space, a tab, and YAML's indicators.
const NOT_KEYS: &str = " \t#[]{},?:!&*|>%@`";

/// What YAML separates the tokens on a line with: a space and a tab, and no other white space.
const SPACES: [char; 2] = [' ', '\t'];

/// One entry of `verified`: who confirmed the idea, when, and the idea and code they checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The actor: lengua and its version, [`ACTOR`].
    pub by: String,
    /// When, in RFC 3339, in UTC, to the second, as [`utc`] writes it: `2026-11-06T23:00:00Z`.
    pub at: String,
    /// lengua's hash of the idea it checked: [`idea_hash`].
    pub idea: Sha256Hex,
    /// lengua's hash of the code it checked.
    pub code: Sha256Hex,
}

impl Verified {
    /// lengua's entry for the idea `idea` and the code `code`, checked at `at`, a time as [`utc`]
    /// writes it: the caller reads the clock, so that a test can fix it.
    #[must_use]
    pub fn new(at: impl Into<String>, idea: Sha256Hex, code: Sha256Hex) -> Self {
        Self {
            by: ACTOR.to_owned(),
            at: at.into(),
            idea,
            code,
        }
    }

    /// The entry as one line of YAML, a flow mapping: `{ by: lengua/0.2.0, at:
    /// 2026-11-06T23:00:00Z, idea: "sha256:…", code: "sha256:…" }`.
    #[must_use]
    pub fn item(&self) -> String {
        format!(
            "{{ by: {}, at: {}, idea: \"{}\", code: \"{}\" }}",
            scalar(&self.by),
            scalar(&self.at),
            self.idea.label(),
            self.code.label()
        )
    }
}

/// The time `seconds` after 1970-01-01T00:00:00Z as `verified` writes it: RFC 3339, in UTC, to
/// the second, such as `2026-11-06T23:00:00Z`.
#[must_use]
pub fn utc(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or_default();
    let time = seconds % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        crate::graph::Day::from_unix_days(days),
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
}

/// lengua's hash of an idea: SHA-256 of idea.md without its frontmatter's top-level `verified`
/// and `generated` keys, each key's line with the lines its value goes on to (indented lines and
/// list items, up to the next top-level key or the closing `---`). Everything else counts byte for
/// byte, line endings too, so a file with no frontmatter, or with neither key, hashes as its bytes
/// do.
#[must_use]
pub fn idea_hash(bytes: &[u8]) -> Sha256Hex {
    let Some(page) = std::str::from_utf8(bytes).ok().and_then(Page::of) else {
        return Sha256Hex::of(bytes);
    };
    let record: Vec<&Block> = page
        .blocks
        .iter()
        .filter(|block| TRUST_KEYS.contains(&block.key.as_str()))
        .collect();
    if record.is_empty() {
        return Sha256Hex::of(bytes);
    }
    let kept: String = page
        .lines
        .iter()
        .enumerate()
        .filter(|(index, _)| !record.iter().any(|block| block.holds(*index)))
        .map(|(_, line)| *line)
        .collect();
    Sha256Hex::of(kept.as_bytes())
}

/// `text`, an idea, with lengua's entry `entry` in its frontmatter's `verified`: every other
/// entry as it was, lengua's earlier ones gone, and `entry` last. With no `verified` key, the key
/// and the entry go just before the closing `---`; a bare mapping, which OKF lets `verified` be,
/// becomes the list's first entry. A flow list's entries each go on a line of their own, and its
/// comments too, each after the entry it follows. Only those lines change, in the file's line
/// endings, so [`idea_hash`] stays the same. A text with no frontmatter comes back as it is.
#[must_use]
pub fn with_lengua_verified(text: &str, entry: &Verified) -> String {
    let Some(page) = Page::of(text) else {
        return text.to_owned();
    };
    let eol = page.eol();
    let item = entry.item();
    let Some(block) = page.block(VERIFIED) else {
        let lines = format!("{VERIFIED}:{eol}{INDENT}- {item}{eol}");
        return page.splice(page.close, page.close, &lines);
    };
    let record = Record::read(&page, block);
    let lines = format!(
        "{}{}{}- {item}{eol}",
        record.head,
        record.kept.concat(),
        " ".repeat(record.indent)
    );
    page.splice(block.start, block.end, &lines)
}

/// `text`, an idea, without lengua's entries in its frontmatter's `verified`, and without the
/// key when no entry is left; everyone else's entries stay as they were, and [`idea_hash`] stays
/// the same. `None` when there was no entry of lengua's to take away.
#[must_use]
pub fn without_lengua_verified(text: &str) -> Option<String> {
    let page = Page::of(text)?;
    let block = page.block(VERIFIED)?;
    let record = Record::read(&page, block);
    if !record.lengua {
        return None;
    }
    if record.others == 0 {
        return Some(page.splice(block.start, block.end, ""));
    }
    let mut kept = record.kept;
    // A blank line, last, would no longer be the key's, and goes; a comment at the margin, last,
    // moves in, where it stays the key's.
    while kept.last().is_some_and(|line| blank(line)) {
        kept.pop();
    }
    if let Some(last) = kept.last_mut()
        && neutral(last)
    {
        *last = moved(last);
    }
    let lines = format!("{}{}", record.head, kept.concat());
    Some(page.splice(block.start, block.end, &lines))
}

/// lengua's entries in `text`'s `verified`, as YAML reads the frontmatter, in order: each one
/// whose `by` starts `lengua/` and that names an idea and a code by their `sha256:` hashes.
/// `verified` may be a list or one mapping. An entry that doesn't read so is none of lengua's.
#[must_use]
pub fn lengua_entries(text: &str) -> Vec<Verified> {
    /// The key lengua reads, as any YAML.
    #[derive(Deserialize)]
    struct Front {
        /// The list of entries, or one.
        verified: Option<serde_json::Value>,
    }
    let Some(page) = Page::of(text) else {
        return Vec::new();
    };
    let yaml = page.lines[1..page.close].concat();
    let Ok(front) = serde_saphyr::from_str::<Front>(&yaml) else {
        return Vec::new();
    };
    let entries = match front.verified {
        Some(serde_json::Value::Array(entries)) => entries,
        Some(entry @ serde_json::Value::Object(_)) => vec![entry],
        _ => Vec::new(),
    };
    entries
        .iter()
        .filter_map(|entry| {
            let field = |key: &str| entry.get(key).and_then(serde_json::Value::as_str);
            let by = field("by").filter(|by| by.starts_with(LENGUA_PREFIX))?;
            let hash = |key: &str| Sha256Hex::parse(field(key)?.strip_prefix("sha256:")?);
            Some(Verified {
                by: by.to_owned(),
                at: field("at").unwrap_or_default().to_owned(),
                idea: hash("idea")?,
                code: hash("code")?,
            })
        })
        .collect()
}

/// A page's lines, each with its ending, and its frontmatter's top-level keys.
struct Page<'a> {
    /// The lines, each with its line ending.
    lines: Vec<&'a str>,
    /// The line of the frontmatter's closing `---`; the opening one is line 0.
    close: usize,
    /// The frontmatter's top-level keys, each with its lines.
    blocks: Vec<Block>,
}

impl<'a> Page<'a> {
    /// `text` as a page; `None` when it has no frontmatter.
    fn of(text: &'a str) -> Option<Self> {
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let close = closing(&lines)?;
        let blocks = blocks(&lines, 1, close);
        Some(Self {
            lines,
            close,
            blocks,
        })
    }

    /// The line ending new lines take: the first line's.
    fn eol(&self) -> &'static str {
        match self.lines.first() {
            Some(line) if line.ends_with("\r\n") => "\r\n",
            _ => "\n",
        }
    }

    /// The first block of the top-level key `key`.
    fn block(&self, key: &str) -> Option<&Block> {
        self.blocks.iter().find(|block| block.key == key)
    }

    /// The page, its lines from `start` up to `end` replaced by `text`.
    fn splice(&self, start: usize, end: usize, text: &str) -> String {
        let mut out = self.lines[..start].concat();
        out.push_str(text);
        out.push_str(&self.lines[end..].concat());
        out
    }
}

/// A top-level key of a frontmatter, with the lines its value takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    /// The key, without quotes.
    pub(crate) key: String,
    /// The key's line.
    pub(crate) start: usize,
    /// One past its value's last line.
    pub(crate) end: usize,
}

impl Block {
    /// Whether the line `line` is one of the block's.
    fn holds(&self, line: usize) -> bool {
        (self.start..self.end).contains(&line)
    }
}

/// Where the frontmatter that opens `lines` closes: the index of its second `---` line. `None`
/// when the first line isn't `---`, or no line closes it.
pub(crate) fn closing(lines: &[&str]) -> Option<usize> {
    if lines.first()?.trim_end() != "---" {
        return None;
    }
    lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")
        .map(|at| at + 1)
}

/// The top-level keys among the frontmatter lines `lines[from..to]`, in order, each from its line
/// to its value's last: the indented lines and the list items after it, up to the next top-level
/// key, but not the blank lines and the comments at the margin after its value. The lines may
/// keep their endings or not.
pub(crate) fn blocks(lines: &[&str], from: usize, to: usize) -> Vec<Block> {
    let mut found: Vec<Block> = Vec::new();
    for (index, line) in lines.iter().enumerate().take(to).skip(from) {
        if let Some((key, _)) = key_of(text_of(line)) {
            found.push(Block {
                key: key.to_owned(),
                start: index,
                end: index + 1,
            });
        } else if !neutral(line)
            && let Some(block) = found.last_mut()
        {
            block.end = index + 1;
        }
    }
    found
}

/// The entries of a list written an entry a line or more in `lines[from..to]`: each one's dash
/// line, and one past its last line, the lines under it indented further. Empty when the first
/// line there that holds something isn't a list's item.
pub(crate) fn entries(lines: &[&str], from: usize, to: usize) -> Vec<(usize, usize)> {
    let Some(first) = (from..to).find(|&at| content(lines[at])) else {
        return Vec::new();
    };
    let indent = indentation(lines[first]);
    if !dash_at(lines[first], indent) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut at = first;
    while at < to {
        if dash_at(lines[at], indent) {
            let mut end = at + 1;
            while end < to
                && !dash_at(lines[end], indent)
                && (blank(lines[end]) || indentation(lines[end]) > indent)
            {
                end += 1;
            }
            found.push((at, end));
            at = end;
        } else {
            at += 1;
        }
    }
    found
}

/// The first of `lines[from..to]` that starts the key `key`, after its indentation and a list's
/// dash.
pub(crate) fn key_line(lines: &[&str], from: usize, to: usize, key: &str) -> Option<usize> {
    (from..to).find(|&at| {
        let text = text_of(lines[at]).trim_start_matches(' ');
        let text = if is_dash(text) {
            text[1..].trim_start_matches([' ', '\t'])
        } else {
            text
        };
        key_of(text).is_some_and(|(found, _)| found == key)
    })
}

/// The key `text` starts, without quotes, and the byte just past its colon: a plain key up to the
/// first `:` before a space, a tab or the end, or a quoted one. `None` when `text` starts no key,
/// as an indented line, a list's item, a comment or a blank line doesn't.
fn key_of(text: &str) -> Option<(&str, usize)> {
    let first = text.chars().next()?;
    match first {
        '"' | '\'' => {
            let close = text[1..].find(first)? + 1;
            let after = &text[close + 1..];
            let colon = text.len() - after.trim_start_matches([' ', '\t']).len();
            let rest = text[colon..].strip_prefix(':')?;
            (rest.is_empty() || rest.starts_with([' ', '\t']))
                .then_some((&text[1..close], colon + 1))
        }
        _ if is_dash(text) || NOT_KEYS.contains(first) => None,
        _ => {
            let bytes = text.as_bytes();
            let colon = (0..bytes.len()).find(|&at| {
                bytes[at] == b':' && matches!(bytes.get(at + 1), None | Some(b' ' | b'\t'))
            })?;
            let key = text[..colon].trim_end_matches(SPACES);
            // A comment can't start before a key's colon.
            let commented = key.contains(" #") || key.contains("\t#");
            (!key.is_empty() && !commented).then_some((key, colon + 1))
        }
    }
}

/// A line without its line ending.
fn text_of(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

/// Whether a line holds nothing but spaces and tabs.
fn blank(line: &str) -> bool {
    text_of(line).trim_matches([' ', '\t']).is_empty()
}

/// Whether a line is a comment, at any indentation.
fn comment(line: &str) -> bool {
    text_of(line)
        .trim_start_matches([' ', '\t'])
        .starts_with('#')
}

/// Whether a line holds something: it is neither blank nor a comment.
fn content(line: &str) -> bool {
    !blank(line) && !comment(line)
}

/// Whether a frontmatter line is no part of a key's value: blank, or a comment at the margin.
fn neutral(line: &str) -> bool {
    blank(line) || text_of(line).starts_with('#')
}

/// How many spaces start `line`.
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Whether `text` starts with a list's dash: `-` alone, or before a space or a tab.
fn is_dash(text: &str) -> bool {
    text == "-" || text.starts_with("- ") || text.starts_with("-\t")
}

/// Whether `line` is a list's item whose dash is `indent` spaces in.
fn dash_at(line: &str, indent: usize) -> bool {
    indentation(line) == indent && is_dash(&text_of(line)[indent..])
}

/// `lines` joined, each without as many as `indent` of the spaces it starts with.
fn dedent(lines: &[&str], indent: usize) -> String {
    lines
        .iter()
        .map(|line| &line[indentation(line).min(indent)..])
        .collect()
}

/// A line moved two spaces further in, unless it's blank.
fn moved(line: &str) -> String {
    if blank(line) {
        line.to_owned()
    } else {
        format!("{INDENT}{line}")
    }
}

/// `lines`, one value whose first line is `indent` spaces in, as a list's entry: a dash before
/// its first line, and its other lines two spaces further in.
fn as_entry(lines: &[&str], indent: usize) -> Vec<String> {
    let Some((first, rest)) = lines.split_first() else {
        return Vec::new();
    };
    let spaces = indentation(first).min(indent);
    let mut entry = vec![format!("{}- {}", &first[..spaces], &first[spaces..])];
    entry.extend(rest.iter().map(|line| moved(line)));
    entry
}

/// `text` as a value in a flow mapping: plain when YAML reads that back as the same text, and
/// otherwise in double quotes, escaped as JSON escapes a string (which YAML reads too).
fn scalar(text: &str) -> String {
    let safe = |character: char| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':' | '+')
    };
    let starts = text.starts_with(|first: char| first.is_ascii_alphanumeric());
    if starts && !text.ends_with(':') && text.chars().all(safe) {
        text.to_owned()
    } else {
        serde_json::Value::String(text.to_owned()).to_string()
    }
}

/// What the `verified` key holds, read line by line so that it can be written back.
struct Record {
    /// The key's line as it is written back: the key, its colon, any comment after its value, and
    /// the line's ending.
    head: String,
    /// The lines that stay under it, each with its ending: someone else's entries, as a list's
    /// items, and the comments among them.
    kept: Vec<String>,
    /// How many entries of someone else's `kept` holds.
    others: usize,
    /// Whether one of the entries was lengua's.
    lengua: bool,
    /// How many spaces in the list's dashes are.
    indent: usize,
}

impl Record {
    /// A record with nothing under its key yet.
    fn new(head: String, indent: usize) -> Self {
        Self {
            head,
            kept: Vec::new(),
            others: 0,
            lengua: false,
            indent,
        }
    }

    /// Reads the block of `verified` in `page`.
    fn read(page: &Page<'_>, block: &Block) -> Self {
        let eol = page.eol();
        let line = page.lines[block.start];
        let text = text_of(line);
        let ending = match &line[text.len()..] {
            "" => eol,
            ending => ending,
        };
        let below = &page.lines[block.start + 1..block.end];
        let colon = key_of(text).map_or(text.len(), |(_, colon)| colon);
        let (prefix, rest) = text.split_at(colon);
        let (value, comment) = split_comment(rest);
        let head = format!("{prefix}{comment}{ending}");
        match value.chars().next() {
            None => Self::under(line.to_owned(), below, eol),
            Some('[' | '{') => match flow(rest.trim_start_matches(SPACES), below) {
                Some(mut collection) => {
                    let head = if collection.closes == 0 {
                        // The comment after the collection, on the key's line, stays there.
                        let (_, comment) = split_comment(&collection.after);
                        format!("{prefix}{}{ending}", spaced(comment))
                    } else {
                        // The key's line keeps its own comment, which is the collection's.
                        head
                    };
                    collection.comments.retain(|note| note.line > 0);
                    let mut record = Self::new(head, INDENT.len());
                    record.add_flow(&collection, eol);
                    record.keep(&below[collection.closes.min(below.len())..]);
                    record
                }
                // Never closed: kept as it is written.
                None => Self::scalar(head, value, below, eol),
            },
            Some(_) if NULLS.contains(&value) => Self::empty(head, below),
            Some(_) => Self::scalar(head, value, below, eol),
        }
    }

    /// `verified:` with nothing after it: a list under it, written an entry a line or more or as
    /// a flow list, a bare mapping, or no entries.
    fn under(head: String, below: &[&str], eol: &str) -> Self {
        let entries = entries(below, 0, below.len());
        if let Some(&(first, _)) = entries.first() {
            return Self::list(head, below, &entries, indentation(below[first]));
        }
        let Some(first) = below.iter().position(|line| content(line)) else {
            return Self::empty(head, below);
        };
        let start = text_of(below[first]).trim_start_matches(SPACES);
        if start.starts_with('[')
            && let Some(collection) = flow(start, &below[first + 1..])
        {
            let mut record = Self::new(head, INDENT.len());
            record.keep(&below[..first]);
            record.add_flow(&collection, eol);
            record.keep(&below[(first + 1 + collection.closes).min(below.len())..]);
            return record;
        }
        Self::mapping(head, below, first)
    }

    /// A list written an entry a line or more, its dashes `indent` spaces in: someone else's
    /// entries keep their lines.
    fn list(head: String, below: &[&str], entries: &[(usize, usize)], indent: usize) -> Self {
        let mut record = Self::new(head, indent);
        let mut at = 0;
        for &(start, end) in entries {
            record.keep(&below[at..start]);
            let lines = &below[start..end];
            if lengua_list(&dedent(lines, indent)) {
                record.lengua = true;
            } else {
                record.others += 1;
                for line in lines {
                    record.kept.push((*line).to_owned());
                }
            }
            at = end;
        }
        record.keep(&below[at..]);
        record
    }

    /// A bare mapping under `verified:`, from the line `first` on: it becomes the list's first
    /// entry, unless it is lengua's.
    fn mapping(head: String, below: &[&str], first: usize) -> Self {
        let indent = indentation(below[first]);
        let mut record = Self::new(head, indent);
        record.keep(&below[..first]);
        let lines = &below[first..];
        if lengua_mapping(&dedent(lines, indent)) {
            record.lengua = true;
        } else {
            record.others += 1;
            record.kept.extend(as_entry(lines, indent));
        }
        record
    }

    /// The entries of a flow list, or a bare flow mapping, each made an item on a line of its own,
    /// and each of its comments a line of its own after the entry it follows.
    fn add_flow(&mut self, flow: &Flow, eol: &str) {
        self.add_notes(flow, 0, eol);
        for (index, item) in flow.entries.iter().enumerate() {
            if lengua_mapping(item) {
                self.lengua = true;
            } else {
                self.others += 1;
                self.kept.push(format!("{INDENT}- {item}{eol}"));
            }
            self.add_notes(flow, index + 1, eol);
        }
    }

    /// The comments of `flow` that follow its first `count` entries, each on a line of its own,
    /// indented as the entries are, so that it stays the key's.
    fn add_notes(&mut self, flow: &Flow, count: usize, eol: &str) {
        let last = flow.entries.len();
        for note in &flow.comments {
            if note.follows.min(last) == count {
                self.kept.push(format!("{INDENT}{}{eol}", note.text));
            }
        }
    }

    /// A value that OKF doesn't let `verified` be, such as a word: it stays, as the list's first
    /// entry.
    fn scalar(head: String, value: &str, below: &[&str], eol: &str) -> Self {
        let mut record = Self::new(head, INDENT.len());
        record.others = 1;
        record.kept.push(format!("{INDENT}- {value}{eol}"));
        record.kept.extend(below.iter().map(|line| moved(line)));
        record
    }

    /// `verified` with no entries: what is under it stays.
    fn empty(head: String, below: &[&str]) -> Self {
        let mut record = Self::new(head, INDENT.len());
        record.keep(below);
        record
    }

    /// Keeps `lines` as they are: comments and blank lines, and anything else, which counts as an
    /// entry of someone else's.
    fn keep(&mut self, lines: &[&str]) {
        for line in lines {
            if content(line) {
                self.others += 1;
            }
            self.kept.push((*line).to_owned());
        }
    }
}

/// The part of an entry lengua reads: who made it.
#[derive(Deserialize)]
struct Actor {
    /// The actor.
    by: Option<String>,
}

impl Actor {
    /// Whether lengua made the entry.
    fn is_lengua(&self) -> bool {
        self.by
            .as_deref()
            .is_some_and(|by| by.starts_with(LENGUA_PREFIX))
    }
}

/// Whether the YAML mapping `text` is an entry of lengua's.
fn lengua_mapping(text: &str) -> bool {
    serde_saphyr::from_str::<Actor>(text).is_ok_and(|actor| actor.is_lengua())
}

/// Whether the YAML list `text`, which holds one entry, holds one of lengua's.
fn lengua_list(text: &str) -> bool {
    serde_saphyr::from_str::<Vec<Actor>>(text)
        .is_ok_and(|actors| actors.iter().any(Actor::is_lengua))
}

/// What a character of YAML text is part of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The text around the scalars in quotes and the comments.
    Plain,
    /// A scalar in quotes, the quotes among it.
    Quoted,
    /// A comment, its `#` among it.
    Comment,
}

/// Whether YAML reads `character` as the space between two tokens, after which a comment or a
/// scalar in quotes can start: a space, a tab or a line's end, and no other white space.
fn separates(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n')
}

/// Each character of `text`, with its byte and what it is part of: a scalar in quotes (`"…"` with
/// its backslash escapes, or `'…'` with `''` for a quote), a comment (from a `#` after a space, a
/// tab or a line's start, to the line's end), or the plain text around them.
fn parts(text: &str) -> Vec<(usize, char, Part)> {
    let mut found = Vec::with_capacity(text.len());
    let mut characters = text.char_indices().peekable();
    let mut previous = '\n';
    while let Some((at, character)) = characters.next() {
        if character == '#' && separates(previous) {
            found.push((at, character, Part::Comment));
            while let Some(&(at, next)) = characters.peek() {
                if next == '\n' {
                    break;
                }
                found.push((at, next, Part::Comment));
                characters.next();
            }
            previous = '#';
            continue;
        }
        let opens = matches!(character, '"' | '\'')
            && (separates(previous) || matches!(previous, '[' | '{' | ',' | ':'));
        previous = character;
        if !opens {
            found.push((at, character, Part::Plain));
            continue;
        }
        found.push((at, character, Part::Quoted));
        while let Some((at, inner)) = characters.next() {
            found.push((at, inner, Part::Quoted));
            if character == '"' && inner == '\\' {
                if let Some((at, escaped)) = characters.next() {
                    found.push((at, escaped, Part::Quoted));
                }
            } else if inner == character {
                if character == '\'' && characters.peek().is_some_and(|&(_, next)| next == '\'') {
                    if let Some((at, next)) = characters.next() {
                        found.push((at, next, Part::Quoted));
                    }
                } else {
                    break;
                }
            }
        }
    }
    found
}

/// `rest`, what follows a key's colon on its line, as its value without the spaces and tabs around
/// it, and its comment with the spaces before it: `("[a]", "  # note")` for ` [a]  # note`.
fn split_comment(rest: &str) -> (&str, &str) {
    let comment = parts(rest)
        .into_iter()
        .find(|&(_, _, part)| part == Part::Comment)
        .map(|(at, _, _)| at);
    match comment {
        Some(at) => {
            let value = rest[..at].trim_end_matches(SPACES);
            (value.trim_start_matches(SPACES), &rest[value.len()..])
        }
        None => (rest.trim_matches(SPACES), ""),
    }
}

/// `comment`, as it follows a key's colon on its line, with a space before it when nothing
/// separates it from the colon, so that the line still starts the key.
fn spaced(comment: &str) -> String {
    if comment.is_empty() || comment.starts_with(SPACES) {
        comment.to_owned()
    } else {
        format!(" {comment}")
    }
}

/// A flow collection, `[…]` or `{…}`, read from the line it opens on to the line it closes on.
struct Flow {
    /// Its entries, each on one line and without comments: a list's, or the mapping itself.
    entries: Vec<String>,
    /// Its comments in order, the one after it on the line it closes on among them.
    comments: Vec<Note>,
    /// The line it closes on: 0 for the line it opens on, `n` for the `n`th line after that.
    closes: usize,
    /// What follows it on the line it closes on.
    after: String,
}

/// A comment in a flow collection, or after it on the line it closes on.
struct Note {
    /// The comment, from its `#` to the end of its line.
    text: String,
    /// The line it's on: 0 for the line the collection opens on.
    line: usize,
    /// How many of the collection's entries begin before it: it goes after the last of them.
    follows: usize,
}

/// The flow collection that `start` opens with `[` or `{`, read on into the lines `below`; `None`
/// when it never closes.
fn flow(start: &str, below: &[&str]) -> Option<Flow> {
    let mut text = text_of(start).to_owned();
    text.push('\n');
    for line in below {
        text.push_str(text_of(line));
        text.push('\n');
    }
    let list = text.starts_with('[');
    let mut entries: Vec<String> = Vec::new();
    // The entry being read: a list's since its `[` or its last `,`, or all of a mapping.
    let mut entry = String::new();
    let mut comments: Vec<Note> = Vec::new();
    let mut depth = 0_usize;
    let mut line = 0;
    let mut noting = false;
    for (at, character, part) in parts(&text) {
        if part == Part::Comment {
            if !noting {
                let begun = usize::from(!one_line(&entry).is_empty());
                let follows = if list { entries.len() + begun } else { 1 };
                comments.push(Note {
                    text: String::new(),
                    line,
                    follows,
                });
            }
            noting = true;
            if let Some(note) = comments.last_mut() {
                note.text.push(character);
            }
            continue;
        }
        noting = false;
        line += usize::from(character == '\n');
        if part == Part::Plain {
            match character {
                '[' | '{' => {
                    depth += 1;
                    // A list's own bracket is no part of its first entry.
                    if list && depth == 1 {
                        continue;
                    }
                }
                ']' | '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if !list {
                            entry.push(character);
                        }
                        add_entry(&mut entries, &entry);
                        let end = at + character.len_utf8();
                        let after = text[end..].split('\n').next().unwrap_or_default();
                        let (_, comment) = split_comment(after);
                        if !comment.is_empty() {
                            comments.push(Note {
                                text: comment.trim_start_matches(SPACES).to_owned(),
                                line,
                                follows: entries.len(),
                            });
                        }
                        return Some(Flow {
                            entries,
                            comments,
                            closes: line,
                            after: after.to_owned(),
                        });
                    }
                }
                ',' if list && depth == 1 => {
                    add_entry(&mut entries, &std::mem::take(&mut entry));
                    continue;
                }
                _ => {}
            }
        }
        entry.push(character);
    }
    None
}

/// Adds `entry`, a flow list's entry as it is written, to `entries` on one line, unless it's
/// empty, as the text after a list's last `,` can be.
fn add_entry(entries: &mut Vec<String>, entry: &str) {
    let entry = one_line(entry);
    if !entry.is_empty() {
        entries.push(entry);
    }
}

/// `text` on one line: each of its lines without the spaces and tabs at its ends, joined with a
/// space.
fn one_line(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_matches(SPACES))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The add idea as an Attested Computation, with no trust record.
    const IDEA: &str = include_str!("../tests/fixtures-wasm/add/idea.md");

    /// An entry of someone else's.
    const ANA: &str = "{ by: human:ana, at: 2026-06-25T09:00:00Z }";

    /// The frontmatter's keys that the tests read.
    #[derive(Debug, Deserialize)]
    struct Front {
        verified: Vec<Listed>,
    }

    /// An entry of `verified`, as YAML reads it.
    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct Listed {
        by: String,
        at: Option<String>,
        idea: Option<String>,
        code: Option<String>,
    }

    /// lengua's entry for the idea `idea` and the code `code`, checked at `at`.
    fn entry(at: &str) -> Verified {
        Verified::new(at, Sha256Hex::of(b"idea"), Sha256Hex::of(b"code"))
    }

    /// The entry the tests write.
    fn lengua() -> Verified {
        entry("2026-11-06T23:00:00Z")
    }

    /// `text` with `lines` just before its frontmatter's closing `---`.
    fn with(text: &str, lines: &str) -> String {
        assert!(text.contains("\n---\n"), "{text}");
        text.replacen("\n---\n", &format!("\n{lines}---\n"), 1)
    }

    /// The frontmatter's YAML.
    fn yaml(text: &str) -> String {
        let page = Page::of(text).expect("a frontmatter");
        page.lines[1..page.close].concat()
    }

    /// What follows the frontmatter.
    fn body(text: &str) -> String {
        let page = Page::of(text).expect("a frontmatter");
        page.lines[page.close..].concat()
    }

    /// The entries of `verified`, as YAML reads the frontmatter, which must read with `verified`
    /// a list.
    fn listed(text: &str) -> Vec<Listed> {
        serde_saphyr::from_str::<Front>(&yaml(text))
            .unwrap_or_else(|error| panic!("{error}\n{text}"))
            .verified
    }

    /// Who made each entry of `verified`, in order.
    fn actors(text: &str) -> Vec<String> {
        listed(text).into_iter().map(|entry| entry.by).collect()
    }

    /// `text` with lengua's entry written, after checking what every write keeps: the idea's
    /// hash, a frontmatter that reads with `verified` a list and lengua's entry last, the body,
    /// and the text itself when the entry is written again.
    fn written(text: &str) -> String {
        let once = with_lengua_verified(text, &lengua());
        assert_eq!(
            idea_hash(once.as_bytes()),
            idea_hash(text.as_bytes()),
            "{once}"
        );
        assert_eq!(
            actors(&once).last().map(String::as_str),
            Some(ACTOR),
            "{once}"
        );
        assert_eq!(body(&once), body(text), "the body is untouched");
        assert_eq!(
            with_lengua_verified(&once, &lengua()),
            once,
            "writing twice is writing once"
        );
        once
    }

    #[test]
    fn the_hash_leaves_out_the_trust_record() {
        let plain = idea_hash(IDEA.as_bytes());
        // A file with no trust record hashes as its bytes do, and so does one with no frontmatter.
        assert_eq!(plain, Sha256Hex::of(IDEA.as_bytes()));
        assert_eq!(idea_hash(b"# Purpose\n"), Sha256Hex::of(b"# Purpose\n"));
        assert_eq!(idea_hash(b"\xff\xfe"), Sha256Hex::of(b"\xff\xfe"));
        let records = [
            format!("verified:\n  - {ANA}\n"),
            format!("verified: [{ANA}]\n"),
            format!("verified: {ANA}\n"),
            "verified:\n  by: human:ana\n  at: 2026-06-25T09:00:00Z\n".to_owned(),
            "verified:\n- by: human:ana\n  at: 2026-06-25T09:00:00Z\n\n- by: process:nightly\n  \
             at: 2026-06-26T02:00:00Z\n"
                .to_owned(),
            "generated: { by: agent/1.0, at: 2026-06-20T22:53:05Z }\n".to_owned(),
            format!(
                "generated:\n  by: agent/1.0\n  at: 2026-06-20T22:53:05Z\nverified:\n  - {ANA}\n"
            ),
        ];
        for record in records {
            // Added, it counts for nothing; changed, it still doesn't.
            let recorded = with(IDEA, &record);
            assert_eq!(idea_hash(recorded.as_bytes()), plain, "{record}");
            let changed = recorded.replace("2026-06-2", "2026-07-0");
            assert_ne!(changed, recorded);
            assert_eq!(idea_hash(changed.as_bytes()), plain, "{record}");
            // In a file whose lines end in CRLF, too.
            let crlf = recorded.replace('\n', "\r\n");
            assert_eq!(
                idea_hash(crlf.as_bytes()),
                idea_hash(IDEA.replace('\n', "\r\n").as_bytes()),
                "{record}"
            );
        }
        // Every other change counts, line endings among them.
        let changes = [
            IDEA.replace("title: add", "title: sum"),
            IDEA.replace("prints the sum.", "prints the sum"),
            IDEA.replace("- Uses: nothing", "- Uses: the clock"),
            IDEA.replacen('\n', "\r\n", 1),
            with(IDEA, "# a comment\n"),
            with(IDEA, &format!("verified:\n  - {ANA}\n# after it\n")),
            with(IDEA, "verified_by: human:ana\n"),
            // A key of that name inside another key's value is that key's.
            IDEA.replace(
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md\n",
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md\n  \
                 verified: yes\n",
            ),
        ];
        for changed in changes {
            assert_ne!(changed, IDEA);
            assert_ne!(idea_hash(changed.as_bytes()), plain, "{changed}");
        }
    }

    #[test]
    fn an_entry_is_one_line_of_yaml() {
        assert_eq!(
            lengua().item(),
            format!(
                "{{ by: lengua/{}, at: 2026-11-06T23:00:00Z, idea: \"sha256:{}\", code: \
                 \"sha256:{}\" }}",
                env!("CARGO_PKG_VERSION"),
                Sha256Hex::of(b"idea"),
                Sha256Hex::of(b"code")
            )
        );
        // A time with YAML's own marks in it goes in quotes, and reads back the same.
        let odd = Verified {
            at: "soon, maybe".into(),
            ..lengua()
        };
        let item = odd.item();
        assert!(item.contains("at: \"soon, maybe\","), "{item}");
        let quoted = with_lengua_verified(IDEA, &odd);
        assert_eq!(listed(&quoted)[0].at.as_deref(), Some("soon, maybe"));
        // The caller's clock, as `verified` writes a time.
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_794_006_000), "2026-11-06T23:00:00Z");
        assert_eq!(utc(951_782_400 + 3_661), "2000-02-29T01:01:01Z");
    }

    #[test]
    fn writes_lengua_s_entry_where_there_is_none() {
        let once = written(IDEA);
        assert_eq!(
            once,
            with(IDEA, &format!("verified:\n  - {}\n", lengua().item()))
        );
        assert_eq!(
            listed(&once),
            [Listed {
                by: ACTOR.to_owned(),
                at: Some("2026-11-06T23:00:00Z".to_owned()),
                idea: Some(Sha256Hex::of(b"idea").label()),
                code: Some(Sha256Hex::of(b"code").label()),
            }]
        );
        // A key with no entries is the same.
        for empty in ["verified:\n", "verified: ~\n", "verified: []\n"] {
            assert_eq!(written(&with(IDEA, empty)), once, "{empty}");
        }
        // A text with no frontmatter isn't an idea, and comes back as it is.
        assert_eq!(
            with_lengua_verified("# Purpose\n", &lengua()),
            "# Purpose\n"
        );
    }

    #[test]
    fn keeps_everyone_else_s_entries() {
        // A list with someone else's entry: it stays, and lengua's goes after it.
        let once = written(&with(IDEA, &format!("verified:\n  - {ANA}\n")));
        assert_eq!(
            once,
            with(
                IDEA,
                &format!("verified:\n  - {ANA}\n  - {}\n", lengua().item())
            )
        );
        assert_eq!(actors(&once), ["human:ana", ACTOR]);
        // A bare mapping becomes the list's first entry, as it was written.
        assert_eq!(written(&with(IDEA, &format!("verified: {ANA}\n"))), once);
        let block = with(
            IDEA,
            "verified:\n  by: human:ana\n  at: 2026-06-25T09:00:00Z\n",
        );
        assert_eq!(
            written(&block),
            with(
                IDEA,
                &format!(
                    "verified:\n  - by: human:ana\n    at: 2026-06-25T09:00:00Z\n  - {}\n",
                    lengua().item()
                )
            )
        );
        // A flow list's entries each go on a line of their own, and its comment stays.
        let flow = with(IDEA, &format!("verified: [{ANA}, {ANA}]  # reviewed\n"));
        assert_eq!(
            written(&flow),
            with(
                IDEA,
                &format!(
                    "verified:  # reviewed\n  - {ANA}\n  - {ANA}\n  - {}\n",
                    lengua().item()
                )
            )
        );
    }

    #[test]
    fn replaces_lengua_s_older_entry() {
        let older = Verified {
            by: "lengua/0.0.9".into(),
            ..entry("2026-01-01T00:00:00Z")
        };
        let text = with(
            IDEA,
            &format!("verified:\n  - {}\n  - {ANA}\n", older.item()),
        );
        let once = written(&text);
        assert_eq!(
            once,
            with(
                IDEA,
                &format!("verified:\n  - {ANA}\n  - {}\n", lengua().item())
            )
        );
        assert_eq!(actors(&once), ["human:ana", ACTOR]);
    }

    #[test]
    fn block_entries_keep_their_lines() {
        let block = "verified:\n- by: human:ana\n  at: 2026-06-25T09:00:00Z\n\n- by: \
                     lengua/0.0.9\n  at: 2026-01-01T00:00:00Z\n- by: process:nightly  # each \
                     night\n  at: 2026-06-26T02:00:00Z\n";
        let once = written(&with(IDEA, block));
        assert_eq!(
            once,
            with(
                IDEA,
                &format!(
                    "verified:\n- by: human:ana\n  at: 2026-06-25T09:00:00Z\n\n- by: \
                     process:nightly  # each night\n  at: 2026-06-26T02:00:00Z\n- {}\n",
                    lengua().item()
                )
            )
        );
        assert_eq!(actors(&once), ["human:ana", "process:nightly", ACTOR]);
    }

    #[test]
    fn keeps_the_files_line_endings() {
        let text = with(IDEA, &format!("verified:\n  - {ANA}\n")).replace('\n', "\r\n");
        let once = written(&text);
        assert_eq!(
            once,
            with(
                IDEA,
                &format!("verified:\n  - {ANA}\n  - {}\n", lengua().item())
            )
            .replace('\n', "\r\n")
        );
        let none = written(&IDEA.replace('\n', "\r\n"));
        assert!(
            none.contains(&format!("verified:\r\n  - {}\r\n---\r\n", lengua().item())),
            "{none}"
        );
        assert!(!none.replace("\r\n", "").contains('\n'), "{none}");
    }

    #[test]
    fn keeps_comments_and_other_keys() {
        let comments = format!(
            "# Ana read it by hand.\nverified:  # who checked it\n  # Ana first.\n  - {ANA}  # \
             by hand\nstatus: stable\n"
        );
        let once = written(&with(IDEA, &comments));
        assert_eq!(
            once,
            with(
                IDEA,
                &format!(
                    "# Ana read it by hand.\nverified:  # who checked it\n  # Ana first.\n  - \
                     {ANA}  # by hand\n  - {}\nstatus: stable\n",
                    lengua().item()
                )
            )
        );
    }

    #[test]
    fn takes_lengua_s_entries_away() {
        // Its only entry: the key goes too.
        let only = with(IDEA, &format!("verified:\n  - {}\n", lengua().item()));
        assert_eq!(without_lengua_verified(&only).as_deref(), Some(IDEA));
        let bare = with(IDEA, &format!("verified: {}\n", lengua().item()));
        assert_eq!(without_lengua_verified(&bare).as_deref(), Some(IDEA));
        // Beside others: they stay, and so does what follows the key.
        let beside = with(
            IDEA,
            &format!(
                "verified:\n  - {ANA}\n\n  - {}\n# signed\nstatus: stable\n",
                lengua().item()
            ),
        );
        let removed = without_lengua_verified(&beside).expect("lengua's entry was there");
        assert_eq!(
            removed,
            with(
                IDEA,
                &format!("verified:\n  - {ANA}\n# signed\nstatus: stable\n")
            )
        );
        assert_eq!(idea_hash(removed.as_bytes()), idea_hash(beside.as_bytes()));
        assert_eq!(actors(&removed), ["human:ana"]);
        // A comment at the margin just above lengua's entry moves in, and stays the record's.
        let item = lengua().item();
        let signed = with(
            IDEA,
            &format!("verified:\n  - {ANA}\n# signed\n  - {item}\nstatus: stable\n"),
        );
        let kept = without_lengua_verified(&signed).expect("lengua's entry was there");
        assert_eq!(
            kept,
            with(
                IDEA,
                &format!("verified:\n  - {ANA}\n  # signed\nstatus: stable\n")
            )
        );
        assert_eq!(idea_hash(kept.as_bytes()), idea_hash(signed.as_bytes()));
        // Nothing to take away.
        let others = with(IDEA, &format!("verified:\n  - {ANA}\n"));
        assert_eq!(without_lengua_verified(&others), None);
        assert_eq!(without_lengua_verified(IDEA), None);
        assert_eq!(without_lengua_verified("# Purpose\n"), None);
    }

    /// lengua's entries read back as they were written, in a list beside others or as a bare
    /// mapping; an entry without both hashes, or someone else's, is none of lengua's.
    #[test]
    fn reads_lengua_s_entries_back() {
        assert!(lengua_entries(IDEA).is_empty());
        assert!(lengua_entries("# Purpose\n").is_empty());
        let once = with_lengua_verified(IDEA, &lengua());
        assert_eq!(lengua_entries(&once), [lengua()]);
        let others = with(IDEA, &format!("verified:\n  - {ANA}\n"));
        assert!(lengua_entries(&others).is_empty());
        let both = with_lengua_verified(&others, &lengua());
        assert_eq!(lengua_entries(&both), [lengua()]);
        let bare = with(IDEA, &format!("verified: {}\n", lengua().item()));
        assert_eq!(lengua_entries(&bare), [lengua()]);
        let unhashed = with(
            IDEA,
            &format!("verified:\n  - {{ by: {ACTOR}, at: 2026-11-06T23:00:00Z }}\n"),
        );
        assert!(lengua_entries(&unhashed).is_empty());
        let without = without_lengua_verified(&both).expect("lengua's entry was there");
        assert!(lengua_entries(&without).is_empty());
    }

    /// A flow collection's comments each go on a line of their own, after the entry they follow,
    /// and the key's line keeps its own.
    #[test]
    fn a_flow_s_comments_stay_on_lines_of_their_own() {
        let item = lengua().item();
        let text = format!(
            "verified: [  # who checked it\n  {ANA},  # by hand\n  # and then\n  {ANA}\n]  # \
             the end\n"
        );
        let once = written(&with(IDEA, &text));
        assert_eq!(
            once,
            with(
                IDEA,
                &format!(
                    "verified:  # who checked it\n  - {ANA}\n  # by hand\n  # and then\n  - \
                     {ANA}\n  # the end\n  - {item}\n"
                )
            )
        );
        assert_eq!(actors(&once), ["human:ana", "human:ana", ACTOR]);
        // A bare mapping on lines of its own, with its comments.
        let bare = "verified: { by: human:ana,  # Ana\n  at: 2026-06-25T09:00:00Z }  # by \
                    hand\n";
        assert_eq!(
            written(&with(IDEA, bare)),
            with(
                IDEA,
                &format!("verified:  # Ana\n  - {ANA}\n  # by hand\n  - {item}\n")
            )
        );
        // A flow list on the line under the key: its entries are the list's, and lengua's goes.
        let under = with(IDEA, &format!("verified:\n  [{ANA}, {item}]\n"));
        let others = with(IDEA, &format!("verified:\n  - {ANA}\n"));
        assert_eq!(
            written(&under),
            with(IDEA, &format!("verified:\n  - {ANA}\n  - {item}\n"))
        );
        assert_eq!(without_lengua_verified(&under), Some(others));
    }

    /// A `#` starts a comment only after a space, a tab or a line's start, as YAML reads it, and
    /// lengua's entry leaves the key's line a key's.
    #[test]
    fn a_comment_starts_where_yaml_starts_one() {
        let item = lengua().item();
        let cases = [
            // A `#` after a no-break space is part of the scalar.
            ("verified: x\u{a0}# y\n", "verified:\n  - x\u{a0}# y\n"),
            // A `#` just after a flow list: a space keeps the comment from the colon.
            ("verified: [a]#x\n", "verified: #x\n  - a\n"),
        ];
        for (record, kept) in cases {
            let text = with(IDEA, record);
            let once = with_lengua_verified(&text, &lengua());
            assert_eq!(
                once,
                with(IDEA, &format!("{kept}  - {item}\n")),
                "{record:?}"
            );
            assert_eq!(
                idea_hash(once.as_bytes()),
                idea_hash(text.as_bytes()),
                "{record:?}"
            );
            let removed = without_lengua_verified(&once).expect("lengua's entry was there");
            assert_eq!(removed, with(IDEA, kept));
        }
    }

    #[test]
    fn finds_each_top_level_key_with_its_lines() {
        let lines: Vec<&str> = IDEA.lines().collect();
        let close = closing(&lines).expect("a frontmatter");
        let keys: Vec<(String, usize, usize)> = blocks(&lines, 1, close)
            .into_iter()
            .map(|block| (block.key, block.start, block.end))
            .collect();
        let expected = [
            ("type", 1, 2),
            ("title", 2, 3),
            ("description", 3, 4),
            ("runtime", 4, 5),
            ("computation", 5, 6),
            ("parameters", 6, 9),
            ("executor", 9, 12),
            ("attester", 12, 14),
        ]
        .map(|(key, start, end)| (key.to_owned(), start, end));
        assert_eq!(keys, expected);
        assert_eq!(entries(&lines, 7, 9), [(7, 8), (8, 9)]);
        assert_eq!(key_line(&lines, 10, 12, "receipt"), Some(11));
        assert_eq!(key_line(&lines, 10, 12, "verified"), None);
        // Quoted keys, and values with colons in them.
        assert_eq!(key_of("\"verified\" : [a]"), Some(("verified", 12)));
        assert_eq!(key_of("http://x: y"), Some(("http://x", 9)));
        for text in [
            "  title: add",
            "- a: b",
            "# c: d",
            "[a, b]",
            "word",
            "a:b",
            "",
        ] {
            assert_eq!(key_of(text), None, "{text:?}");
        }
    }
}
