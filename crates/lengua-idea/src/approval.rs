use std::fs;
use std::path::Path;

use crate::frontmatter::{self, Entry, Front};
use crate::text::is_fence;
use crate::{Sink, lock, okf, sha256, wiki};

pub(crate) const KEY: &str = "approved";
pub const VISION: &str = "vision.md";
const APPROVED_SECTIONS: [&str; 2] = ["# Behaviours", "# Constraints"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub sha256: String,
    pub by: String,
    pub at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Approval {
    pub ideas: Option<String>,
    pub ideas_record: Option<Record>,
    pub vision: Option<String>,
    pub vision_record: Option<Record>,
}

impl Approval {
    #[must_use]
    pub fn ideas_approved(&self) -> bool {
        matches!((&self.ideas, &self.ideas_record), (Some(now), Some(r)) if *now == r.sha256)
    }

    #[must_use]
    pub fn vision_approved(&self) -> bool {
        match (&self.vision, &self.vision_record) {
            (None, _) => true,
            (Some(now), Some(r)) => *now == r.sha256,
            (Some(_), None) => false,
        }
    }

    #[must_use]
    pub fn approved(&self) -> bool {
        self.ideas_approved() && self.vision_approved()
    }

    pub(crate) fn current_ideas(&self) -> Option<&Record> {
        self.ideas_record.as_ref().filter(|_| self.ideas_approved())
    }

    pub(crate) fn current_vision(&self) -> Option<&Record> {
        self.vision_record
            .as_ref()
            .filter(|_| self.vision.is_some() && self.vision_approved())
    }

    #[must_use]
    pub fn lock_lines(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(r) = self.current_ideas() {
            out.push((r.sha256.clone(), "idea.md".to_string()));
        }
        if let Some(r) = self.current_vision() {
            out.push((r.sha256.clone(), VISION.to_string()));
        }
        out
    }
}

fn read(folder: &Path, file: &str) -> Option<String> {
    fs::read_to_string(folder.join(file))
        .ok()
        .map(|t| t.replace("\r\n", "\n"))
}

fn split(text: &str, file: &str) -> Front {
    frontmatter::split(text, &mut Sink::new(file))
}

pub(crate) fn sections(body: &str) -> String {
    let mut out = String::new();
    let mut keep = false;
    let mut fence = false;
    for line in body.split_inclusive('\n') {
        if is_fence(line) {
            fence = !fence;
        } else if !fence && line.starts_with("# ") {
            if keep {
                out = format!("{}\n", out.trim_end_matches('\n'));
            }
            keep = APPROVED_SECTIONS.contains(&line.trim_end());
        }
        if keep {
            out.push_str(line);
        }
    }
    if keep {
        out = format!("{}\n", out.trim_end_matches('\n'));
    }
    out
}

fn ideas_hash(folder: &Path) -> Option<String> {
    let files = lock::ideas(folder);
    if !files.iter().any(|f| f == "idea.md") {
        return None;
    }
    let mut all = String::new();
    for file in files {
        let text = read(folder, &file).unwrap_or_default();
        let front = split(&text, &file);
        all.push_str(&file);
        all.push('\n');
        all.push_str(&sections(&wiki::without_links(&front.body)));
    }
    Some(sha256::hex(all.as_bytes()))
}

fn vision_hash(folder: &Path) -> Option<String> {
    let text = read(folder, VISION)?;
    let front = split(&text, VISION);
    let body = wiki::without_links(&front.body);
    Some(sha256::hex(body.trim_start_matches('\n').as_bytes()))
}

pub(crate) fn record(front: &Front) -> Option<Record> {
    let maps = okf::mappings(front.entry(KEY)?)?;
    let map = maps.first()?;
    Some(Record {
        sha256: okf::get(map, "sha256")?.to_string(),
        by: okf::get(map, "by")?.to_string(),
        at: okf::get(map, "at")?.to_string(),
    })
}

fn recorded(folder: &Path, file: &str) -> Option<Record> {
    let text = read(folder, file)?;
    record(&split(&text, file))
}

#[must_use]
pub fn approval(folder: &Path) -> Approval {
    Approval {
        ideas: ideas_hash(folder),
        ideas_record: recorded(folder, "idea.md"),
        vision: vision_hash(folder),
        vision_record: recorded(folder, VISION),
    }
}

pub(crate) fn check_entry(entry: &Entry, sink: &mut Sink) {
    let Some(map) = okf::mappings(entry).and_then(|m| m.into_iter().next()) else {
        sink.add(
            entry.line,
            "approval-form",
            "lengua could not read `approved`: the tool `approve` writes it as `{ sha256: <hex>, by: human:<id>, at: <time> }`".to_string(),
        );
        return;
    };
    let hash = okf::get(&map, "sha256").unwrap_or_default();
    let hex = hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    let by = okf::get(&map, "by").unwrap_or_default();
    let human = by.starts_with("human:") && okf::actor(by);
    let at = okf::get(&map, "at").is_some_and(okf::timestamp);
    if !(hex && human && at) {
        sink.add(
            entry.line,
            "approval-form",
            "`approved` has a `sha256` of 64 hexadecimal digits, a `by` that is `human:<id>` and an `at` that is an ISO 8601 time; the tool `approve` writes it".to_string(),
        );
    }
}

pub(crate) fn with_entry(text: &str, file: &str, key: &str, value: &str) -> String {
    let front = split(text, file);
    let line = format!("{key}: {value}\n");
    if front.entries.is_empty() && !text.starts_with("---\n") {
        return format!("---\n{line}---\n\n{}", text.trim_start_matches('\n'));
    }
    let mut out = String::from("---\n");
    let mut placed = false;
    for entry in &front.entries {
        if entry.key == key {
            out.push_str(&line);
            placed = true;
        } else {
            out.push_str(&entry.text);
        }
    }
    if !placed {
        out.push_str(&line);
    }
    out.push_str("---\n");
    out.push_str(&front.body);
    out
}

fn person(by: &str) -> Result<String, String> {
    let id: String = by
        .trim()
        .trim_start_matches("human:")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("-");
    let actor = format!("human:{id}");
    if id.is_empty() || !okf::actor(&actor) {
        return Err(
            "`person` names the person who approved, for example `ana` or `human:ana`".to_string(),
        );
    }
    Ok(actor)
}

fn idea_problems(folder: &Path) -> Vec<String> {
    crate::idea_findings(folder)
        .into_iter()
        .map(|f| format!("{}:{}: {}", f.file, f.line, f.code))
        .collect()
}

pub struct Approved {
    pub approval: Approval,
    pub written: Vec<String>,
}

pub fn approve(folder: &Path, by: &str, words: &str, now: u64) -> Result<Approved, String> {
    let by = person(by)?;
    let words = words.trim();
    if words.is_empty() {
        return Err(
            "`words` holds the person's own words of approval, as they said them".to_string(),
        );
    }
    let before = approval(folder);
    if before.ideas.is_none() && before.vision.is_none() {
        return Err(format!(
            "the folder has no `idea.md` and no `{VISION}`, so there is nothing to approve"
        ));
    }
    if before.ideas.is_some() {
        let problems = idea_problems(folder);
        if !problems.is_empty() {
            return Err(format!(
                "the ideas have problems ({}); call `check`, fix them, and show the person the ideas again before they approve",
                problems.join(", ")
            ));
        }
    }
    let at = wiki::iso(now);
    let mut written = Vec::new();
    let mut parts = Vec::new();
    let pairs = [
        (
            "idea.md",
            before.ideas.clone(),
            before.ideas_approved(),
            "the ideas",
        ),
        (
            VISION,
            before.vision.clone(),
            before.vision.is_some() && before.vision_approved(),
            "the vision",
        ),
    ];
    for (file, hash, done, what) in pairs {
        let Some(hash) = hash.filter(|_| !done) else {
            continue;
        };
        let text = read(folder, file).unwrap_or_default();
        let value = format!("{{ sha256: {hash}, by: {by}, at: {at} }}");
        fs::write(folder.join(file), with_entry(&text, file, KEY, &value))
            .map_err(|e| format!("lengua could not write `{file}`: {e}"))?;
        written.push(file.to_string());
        parts.push(format!("{what} `{hash}`"));
    }
    if !parts.is_empty() {
        let entry = format!(
            "* **Approval** at {} by {by}: {}. The person's words: {}.\n",
            wiki::time_text(now),
            parts.join(" and "),
            wiki::quoted(words)
        );
        let previous = read(folder, "log.md").unwrap_or_default();
        fs::write(
            folder.join("log.md"),
            wiki::log_text(&previous, &entry, now),
        )
        .map_err(|e| format!("lengua could not write `log.md`: {e}"))?;
        written.push("log.md".to_string());
    }
    Ok(Approved {
        approval: approval(folder),
        written,
    })
}

#[cfg(test)]
mod tests {
    use super::{sections, with_entry};

    #[test]
    fn only_the_behaviours_and_the_constraints_are_approved() {
        let body = "# Purpose\n\nText.\n\n# Behaviours\n\n## One\n\n```\n# Structure\n```\n\n\n# Constraints\n\n- Memory: 1 MiB\n\n# Structure\n\n- `main`: runs.\n";
        assert_eq!(
            sections(body),
            "# Behaviours\n\n## One\n\n```\n# Structure\n```\n# Constraints\n\n- Memory: 1 MiB\n"
        );
        assert_eq!(sections("# Purpose\n\nText.\n"), "");
    }

    #[test]
    fn an_entry_is_added_or_replaced() {
        assert_eq!(
            with_entry("# Purpose\n", "idea.md", "approved", "{ a: b }"),
            "---\napproved: { a: b }\n---\n\n# Purpose\n"
        );
        assert_eq!(
            with_entry(
                "---\ntitle: x\napproved: old\n---\n\n# Purpose\n",
                "idea.md",
                "approved",
                "new"
            ),
            "---\ntitle: x\napproved: new\n---\n\n# Purpose\n"
        );
        assert_eq!(
            with_entry(
                "---\ntitle: x\n---\n\n# Purpose\n",
                "idea.md",
                "approved",
                "new"
            ),
            "---\ntitle: x\napproved: new\n---\n\n# Purpose\n"
        );
    }
}
