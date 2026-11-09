//! The Open Knowledge Format's text as lengua reads and writes it: a page's frontmatter, the
//! markdown links on its lines, the entries lengua keeps in a bundle's `index.md` (OKF v0.2 §8)
//! and the entries of its `log.md` (OKF §9), under headings that name their day.

use std::fmt;

use serde::Deserialize;

use crate::idea::{builds_on_item, find_exact_run, heading};
use crate::state::IDEA_FILE;

/// The OKF version that the root `index.md` lengua makes declares.
pub const OKF_VERSION: &str = "0.2";

/// An index file's name: the list of what its folder holds.
pub const INDEX_FILE: &str = "index.md";

/// The log file's name: what changed in the bundle, newest first.
pub const LOG_FILE: &str = "log.md";

/// The heading over the list of programs in an index that lengua makes.
pub const INDEX_HEADING: &str = "# Programs";

/// The heading that a log lengua makes starts with.
pub const LOG_HEADING: &str = "# Update log";

/// The frontmatter keys lengua reads from a page; every other key is the page's own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Frontmatter {
    /// The page's type, which every page but `index.md` and `log.md` must have.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Its title.
    pub title: Option<String>,
    /// Its summary, in one sentence.
    pub description: Option<String>,
    /// The OKF version a bundle's root index declares, which makes its folder the root.
    pub okf_version: Option<String>,
}

/// The frontmatter at the start of `text`: a line `---`, YAML, and a line `---`. `None` when
/// `text` has none, or its YAML can't be read.
#[must_use]
pub fn frontmatter(text: &str) -> Option<Frontmatter> {
    let yaml = frontmatter_yaml(text)?;
    if yaml.trim().is_empty() {
        return Some(Frontmatter::default());
    }
    serde_saphyr::from_str(yaml).ok()
}

/// The YAML between `text`'s first line, when it is `---`, and the next line that is.
fn frontmatter_yaml(text: &str) -> Option<&str> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut end = first.len();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(&text[first.len()..end]);
        }
        end += line.len();
    }
    None
}

/// The markdown links on `line`, outside its code spans, each as its text and its target: the
/// target as written, without a title or angle brackets. Images are left out.
#[must_use]
pub fn links(line: &str) -> Vec<(String, String)> {
    spans(line)
        .into_iter()
        .map(|(_, text, target)| (text, target))
        .collect()
}

/// [`links`], each with the byte of `line` its `[` is at.
fn spans(line: &str) -> Vec<(usize, String, String)> {
    let prose = prose(line);
    let bytes = prose.as_bytes();
    let brackets = pairs(bytes, b'[', b']');
    let parentheses = pairs(bytes, b'(', b')');
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(offset) = bytes[at..].iter().position(|&byte| byte == b'[') {
        let open = at + offset;
        at = open + 1;
        if open > 0 && matches!(bytes[open - 1], b'!' | b'\\') {
            continue;
        }
        let Some(close) = brackets[open] else {
            continue;
        };
        if bytes.get(close + 1) != Some(&b'(') {
            continue;
        }
        let Some(end) = parentheses[close + 1] else {
            continue;
        };
        let target = target_of(&prose[close + 2..end]);
        if !target.is_empty() {
            found.push((open, prose[open + 1..close].trim().to_owned(), target));
        }
        at = end + 1;
    }
    found
}

/// `line` with each code span blanked out byte for byte, so that what is written as code isn't
/// read as a link and every other byte keeps its place.
fn prose(line: &str) -> String {
    let mut prose = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        prose.push_str(&rest[..start]);
        let after = &rest[start..];
        let run = after.bytes().take_while(|&byte| byte == b'`').count();
        match find_exact_run(&after[run..], run) {
            Some(end) => {
                let span = run + end + run;
                prose.extend(std::iter::repeat_n(' ', span));
                rest = &after[span..];
            }
            // Backticks that open no span are text.
            None => {
                prose.push_str(&after[..run]);
                rest = &after[run..];
            }
        }
    }
    prose.push_str(rest);
    prose
}

/// For each byte of `bytes` that opens a pair of `left` and `right`, the byte that closes it,
/// counting the pairs inside and passing over what a backslash escapes. One pass, so a long
/// line costs only its length.
fn pairs(bytes: &[u8], left: u8, right: u8) -> Vec<Option<usize>> {
    let mut closes = vec![None; bytes.len()];
    let mut open = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            index += 2;
            continue;
        }
        if byte == left {
            open.push(index);
        } else if byte == right
            && let Some(start) = open.pop()
        {
            closes[start] = Some(index);
        }
        index += 1;
    }
    closes
}

/// A link's target, from what its parentheses hold: without angle brackets or a title.
fn target_of(inner: &str) -> String {
    let inner = inner.trim();
    match inner.strip_prefix('<') {
        Some(rest) => rest.split('>').next().unwrap_or_default().to_owned(),
        None => inner
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned(),
    }
}

/// The links of an idea's text, each with its line, outside its frontmatter and code blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdeaLinks {
    /// The targets of its `- Builds on:` lines under `# Constraints`: its parts.
    pub parts: Vec<(usize, String)>,
    /// The targets of every other link.
    pub links: Vec<(usize, String)>,
}

/// The links of the idea `text`, read as the graph reads every idea, even one with problems.
#[must_use]
pub fn idea_links(text: &str) -> IdeaLinks {
    let lines: Vec<&str> = text.lines().collect();
    let body = match lines.first() {
        Some(first) if first.trim_end() == "---" => lines
            .iter()
            .skip(1)
            .position(|line| line.trim_end() == "---")
            .map_or(0, |at| at + 2),
        _ => 0,
    };
    let mut found = IdeaLinks::default();
    let mut fenced = false;
    let mut constraints = false;
    for (index, line) in lines.iter().enumerate().skip(body) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(name) = heading(line, 1) {
            constraints = name.eq_ignore_ascii_case("constraints");
        }
        let parts = constraints && builds_on_item(line).is_some();
        for (_, target) in links(line) {
            if parts {
                found.parts.push((index + 1, target));
            } else {
                found.links.push((index + 1, target));
            }
        }
    }
    found
}

/// The entry lengua keeps in an index for a program: its title linked to its idea, its
/// description as a sentence, and a link to its code when it has some.
///
/// `folder` is the program's folder from the index's, with `/` between names, and empty for the
/// index's own folder.
#[must_use]
pub fn index_entry(
    title: &str,
    folder: &str,
    description: Option<&str>,
    code: Option<&str>,
) -> String {
    let mut entry = format!("* [{}]({})", link_text(title), link_path(folder, IDEA_FILE));
    let mut about = Vec::new();
    if let Some(description) = description
        .map(one_line)
        .filter(|description| !description.is_empty())
    {
        about.push(sentence(description));
    }
    if let Some(code) = code {
        about.push(format!("Code: [{code}]({})", link_path(folder, code)));
    }
    if !about.is_empty() {
        entry.push_str(" - ");
        entry.push_str(&about.join(" "));
    }
    entry
}

/// The idea an index's line links to, when the line is one of lengua's entries: a list item at
/// the margin that starts with a link to an `idea.md` of this computer.
#[must_use]
pub fn entry_target(line: &str) -> Option<String> {
    let item = line
        .strip_prefix("* ")
        .or_else(|| line.strip_prefix("- "))
        .or_else(|| line.strip_prefix("+ "))?;
    let (start, _, target) = spans(item).into_iter().next()?;
    let file = target.split(['#', '?']).next().unwrap_or_default();
    let names_an_idea = file.rsplit('/').next() == Some(IDEA_FILE) && !file.contains("://");
    (start == 0 && names_an_idea).then_some(target)
}

/// How a log entry begins: as a program new to the bundle or as one it already lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A program the bundle's index didn't list before.
    Creation,
    /// A program the bundle's index listed already.
    Update,
}

impl Change {
    /// Its bold word in the log.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Creation => "Creation",
            Self::Update => "Update",
        }
    }
}

/// A log entry about the program in `folder`, from the bundle's root: `* **Update**:
/// [add](add/idea.md) recorded.`
#[must_use]
pub fn log_entry(change: Change, title: &str, folder: &str, what: &str) -> String {
    format!(
        "* **{}**: [{}]({}) {what}.",
        change.word(),
        link_text(title),
        link_path(folder, IDEA_FILE)
    )
}

/// `log` with `entry` at the top of `day`'s entries, newest first, and the day's heading made
/// when it has none. `None` when the newest entry about the same page already says the same.
#[must_use]
pub fn logged(log: Option<&str>, day: Day, entry: &str) -> Option<String> {
    let day = day.to_string();
    let Some(log) = log.filter(|log| !log.trim().is_empty()) else {
        return Some(format!("{LOG_HEADING}\n\n## {day}\n\n{entry}\n"));
    };
    let lines: Vec<&str> = log.split_inclusive('\n').collect();
    let about = entry_about(entry);
    let latest = lines
        .iter()
        .map(|line| line.trim_end())
        .find(|line| about.is_some() && entry_about(line) == about);
    if latest == Some(entry) {
        return None;
    }
    // The first day no later than this one: this day's own, or an older one to go above.
    let place = lines
        .iter()
        .position(|line| day_of(line).is_some_and(|heading| heading <= day.as_str()));
    let (split, chunk) = match place {
        Some(at) if day_of(lines[at]) == Some(day.as_str()) => {
            let mut below = at + 1;
            while below < lines.len() && lines[below].trim().is_empty() {
                below += 1;
            }
            let heading_next = lines.get(below).is_some_and(|line| line.starts_with('#'));
            let gap = if heading_next { "\n" } else { "" };
            (below, format!("{entry}\n{gap}"))
        }
        Some(at) => (at, format!("## {day}\n\n{entry}\n\n")),
        None => (lines.len(), format!("## {day}\n\n{entry}\n")),
    };
    let mut text: String = lines[..split].concat();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if chunk.starts_with("## ") && !text.is_empty() && !text.ends_with("\n\n") {
        text.push('\n');
    }
    text.push_str(&chunk);
    text.push_str(&lines[split..].concat());
    Some(text)
}

/// The page a log entry is about: the target of its first link.
fn entry_about(line: &str) -> Option<String> {
    if !line.starts_with("* ") {
        return None;
    }
    links(line).into_iter().next().map(|(_, target)| target)
}

/// The day a log's heading names, when the line is one: `## 2026-11-06`.
fn day_of(line: &str) -> Option<&str> {
    let day = line.trim_end().strip_prefix("## ")?;
    is_day(day).then_some(day)
}

/// Whether `text` is a day written `YYYY-MM-DD`.
fn is_day(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

/// A day, as the headings of a log name it: `YYYY-MM-DD`, in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    year: i64,
    month: u8,
    day: u8,
}

impl Day {
    /// Today, by this computer's clock, in UTC.
    #[must_use]
    pub fn today() -> Self {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        Self::from_unix_days(i64::try_from(seconds / 86_400).unwrap_or(0))
    }

    /// The day `days` after 1970-01-01, by Howard Hinnant's `civil_from_days`.
    #[must_use]
    pub fn from_unix_days(days: i64) -> Self {
        let shifted = days + 719_468;
        let era = shifted.div_euclid(146_097);
        let of_era = shifted.rem_euclid(146_097);
        let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
        let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let from_march = (5 * of_year + 2) / 153;
        let day = of_year - (153 * from_march + 2) / 5 + 1;
        let month = if from_march < 10 {
            from_march + 3
        } else {
            from_march - 9
        };
        Self {
            year: year_of_era + era * 400 + i64::from(month <= 2),
            month: u8::try_from(month).unwrap_or(1),
            day: u8::try_from(day).unwrap_or(1),
        }
    }

    /// The day `text` names, written `YYYY-MM-DD`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if !is_day(text) {
            return None;
        }
        let year = text[..4].parse().ok()?;
        let month: u8 = text[5..7].parse().ok()?;
        let day: u8 = text[8..].parse().ok()?;
        let real = (1..=12).contains(&month) && (1..=31).contains(&day);
        real.then_some(Self { year, month, day })
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// `text` on one line, its runs of white space each one space.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` ending as a sentence does.
fn sentence(text: String) -> String {
    if text.ends_with(['.', '!', '?']) {
        text
    } else {
        format!("{text}.")
    }
}

/// `text` as a link's text: on one line, with its brackets escaped.
fn link_text(text: &str) -> String {
    one_line(text).replace('[', "\\[").replace(']', "\\]")
}

/// The link to `file` in `folder`, in angle brackets when it holds a space or a parenthesis.
fn link_path(folder: &str, file: &str) -> String {
    let path = if folder.is_empty() {
        file.to_owned()
    } else {
        format!("{folder}/{file}")
    };
    if path.contains([' ', '(', ')']) {
        format!("<{path}>")
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_keys_lengua_needs_from_frontmatter() {
        let page = "---\ntype: note\ntitle: Notes\nextra: [1, 2]\n---\n# Notes\n";
        let front = frontmatter(page).expect("frontmatter");
        assert_eq!(front.kind.as_deref(), Some("note"));
        assert_eq!(front.title.as_deref(), Some("Notes"));
        assert_eq!(front.description, None);
        // A version written as a number reads as one written as text.
        for version in ["\"0.2\"", "0.2"] {
            let text = format!("---\r\nokf_version: {version}\r\n---\r\n# Programs\r\n");
            assert_eq!(
                frontmatter(&text).and_then(|front| front.okf_version),
                Some("0.2".to_owned())
            );
        }
        assert_eq!(frontmatter("# Notes\n"), None);
        assert_eq!(frontmatter("---\ntype: note\n"), None, "never closed");
        assert_eq!(frontmatter("---\n---\n"), Some(Frontmatter::default()));
    }

    #[test]
    fn finds_links_outside_code() {
        assert_eq!(
            links("See [sort lines](../sort/idea.md) and [the site](https://example.com \"t\")."),
            [
                ("sort lines".to_owned(), "../sort/idea.md".to_owned()),
                ("the site".to_owned(), "https://example.com".to_owned()),
            ]
        );
        // Code, images, escapes and links with nothing to lead to are no links.
        assert!(links("`[x](y.md)` and ``a [b](c) d``").is_empty());
        assert!(links("![a picture](picture.png) \\[not](a.md) [empty]()").is_empty());
        // Angle brackets keep a space, and brackets nest.
        assert_eq!(
            links("[a [nested] name](<my notes/idea.md>) é [ü](ü.md)"),
            [
                ("a [nested] name".to_owned(), "my notes/idea.md".to_owned()),
                ("ü".to_owned(), "ü.md".to_owned()),
            ]
        );
        // Text that only looks like the start of a link, or ends early, is text.
        assert!(links("[open](never closed").is_empty());
        assert_eq!(links("`unclosed [x](y.md)").len(), 1);
    }

    #[test]
    fn reads_an_ideas_parts_apart_from_its_other_links() {
        let text = "---\ntype: lengua/program-idea\ntitle: t\ndescription: '[no](link.md)'\n---\n\
                    # Purpose\nLike [sort](../sort/idea.md).\n```\n[in code](code.md)\n```\n\
                    - Builds on: [not yet a part](../early/idea.md)\n# Constraints\n\
                    - Uses: nothing\n\
                    - Builds on: [sort](../sort/idea.md), [lines](../lines/idea.md)\n";
        let found = idea_links(text);
        assert_eq!(
            found.parts,
            [
                (14, "../sort/idea.md".to_owned()),
                (14, "../lines/idea.md".to_owned())
            ]
        );
        assert_eq!(
            found.links,
            [
                (7, "../sort/idea.md".to_owned()),
                (11, "../early/idea.md".to_owned())
            ]
        );
    }

    #[test]
    fn writes_and_recognizes_index_entries() {
        let entry = index_entry(
            "add",
            "add",
            Some("Adds two numbers,\n  given as arguments"),
            Some("main.wat"),
        );
        assert_eq!(
            entry,
            "* [add](add/idea.md) - Adds two numbers, given as arguments. \
             Code: [main.wat](add/main.wat)"
        );
        assert_eq!(entry_target(&entry).as_deref(), Some("add/idea.md"));
        assert_eq!(
            index_entry("[x]", "my tools/x", None, None),
            "* [\\[x\\]](<my tools/x/idea.md>)"
        );
        assert_eq!(
            index_entry("root", "", Some("Says hi!"), None),
            "* [root](idea.md) - Says hi!"
        );
        for line in [
            "* [add](add/idea.md)",
            "- [add](./add/idea.md#b1) - by hand",
        ] {
            assert!(entry_target(line).is_some(), "{line}");
        }
        for line in [
            "  * [add](add/idea.md)",
            "* see [add](add/idea.md)",
            "* [a page](notes.md)",
            "* [far](https://example.com/idea.md)",
            "[add](add/idea.md)",
        ] {
            assert_eq!(entry_target(line), None, "{line}");
        }
    }

    #[test]
    fn logs_each_day_newest_first() {
        let first = Day::parse("2026-11-06").expect("a day");
        let next = Day::parse("2026-11-07").expect("a day");
        let recorded = log_entry(Change::Creation, "add", "add", "recorded");
        let done = log_entry(Change::Update, "add", "add", "checked: done");
        let log = logged(None, first, &recorded).expect("a new log");
        assert_eq!(
            log,
            "# Update log\n\n## 2026-11-06\n\n* **Creation**: [add](add/idea.md) recorded.\n"
        );
        let log = logged(Some(&log), first, &done).expect("a second entry");
        assert_eq!(logged(Some(&log), first, &done), None, "said once");
        let again = log_entry(Change::Update, "add", "add", "recorded");
        let log = logged(Some(&log), next, &again).expect("a new day");
        assert_eq!(
            log,
            "# Update log\n\n## 2026-11-07\n\n* **Update**: [add](add/idea.md) recorded.\n\n\
             ## 2026-11-06\n\n* **Update**: [add](add/idea.md) checked: done.\n\
             * **Creation**: [add](add/idea.md) recorded.\n"
        );
        // A day older than every heading goes last; a human's lines stay as they are.
        let human = "# Our log\nby hand\n## 2026-11-07\n* [x](x/idea.md) made by hand";
        assert_eq!(
            logged(Some(human), first, &recorded).expect("an older day"),
            "# Our log\nby hand\n## 2026-11-07\n* [x](x/idea.md) made by hand\n\n\
             ## 2026-11-06\n\n* **Creation**: [add](add/idea.md) recorded.\n"
        );
    }

    #[test]
    fn days_follow_the_calendar() {
        assert_eq!(Day::from_unix_days(0).to_string(), "1970-01-01");
        assert_eq!(Day::from_unix_days(20_089).to_string(), "2025-01-01");
        assert_eq!(Day::from_unix_days(20_763).to_string(), "2026-11-06");
        assert_eq!(Day::from_unix_days(11_016).to_string(), "2000-02-29");
        assert_eq!(Day::parse("2026-11-06"), Some(Day::from_unix_days(20_763)));
        for text in ["2026-13-01", "2026-11-6", "2026/11/06", "20261106"] {
            assert_eq!(Day::parse(text), None, "{text}");
        }
        assert!(Day::from_unix_days(20_763) < Day::from_unix_days(20_764));
    }
}
