use std::fs;
use std::path::Path;

use crate::frontmatter::Front;
use crate::idea::IdeaKind;
use crate::{Finding, Library, Note, Shape, Trust};

pub(crate) struct Parts<'a> {
    pub shape: Shape,
    pub libraries: &'a [Library],
    pub components: &'a [(String, String, usize)],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub key: &'static str,
    pub file: &'static str,
    pub hash: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Page {
    pub file: String,
    pub kind: IdeaKind,
    pub name: String,
    pub title: String,
    pub description: String,
    pub uses: Vec<String>,
    pub front: Front,
    pub idea_hash: String,
    pub source: String,
    pub source_hash: Option<String>,
    pub generated: Vec<Stamp>,
}

impl Page {
    pub(crate) fn id(&self) -> &str {
        self.file.strip_suffix(".md").unwrap_or(&self.file)
    }

    fn previous_idea(&self) -> Option<&str> {
        self.front.value("idea_sha256")
    }

    fn previous_source(&self) -> Option<&str> {
        self.front.value("source_sha256").filter(|v| *v != "none")
    }
}

pub(crate) fn date(now: u64) -> (u64, u64, u64) {
    let z = now / 86_400 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

fn day_text(now: u64) -> String {
    let (year, month, day) = date(now);
    format!("{year:04}-{month:02}-{day:02}")
}

fn time_text(now: u64) -> String {
    let seconds = now % 86_400;
    format!(
        "{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn flow(list: &[String]) -> String {
    format!("[{}]", list.join(", "))
}

fn type_name(kind: IdeaKind, shape: Shape) -> &'static str {
    match kind {
        IdeaKind::Program => match shape {
            Shape::Command => "lengua Program",
            Shape::Server => "lengua Server",
            Shape::Library => "lengua Library",
        },
        IdeaKind::Module => "lengua Module",
        IdeaKind::Screen => "lengua Screen",
    }
}

pub(crate) fn page_text(page: &Page, used_by: &[String], label: &str, shape: Shape) -> String {
    let source_hash = page
        .source_hash
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let mut keys = vec![
        ("type", type_name(page.kind, shape).to_string()),
        ("description", quoted(&page.description)),
        ("module", page.name.clone()),
        ("source", page.source.clone()),
        ("uses", flow(&page.uses)),
        ("used_by", flow(used_by)),
        ("label", label.to_string()),
        ("idea_sha256", page.idea_hash.clone()),
        ("source_sha256", source_hash),
    ];
    keys.extend(page.generated.iter().map(|s| (s.key, s.hash.clone())));
    page.front.render(&keys)
}

pub(crate) fn index_text(pages: &[Page], label: &str, parts: &Parts) -> String {
    let mut out = String::from("---\nokf_version: \"0.2\"\n---\n");
    let groups = [
        ("Program", IdeaKind::Program),
        ("Modules", IdeaKind::Module),
        ("Screens", IdeaKind::Screen),
    ];
    for (heading, kind) in groups {
        let lines: Vec<String> = pages
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| {
                format!(
                    "* [{}]({}) - {} Label: `{label}`.\n",
                    p.title, p.file, p.description
                )
            })
            .collect();
        section(&mut out, heading, &lines);
    }
    let libraries: Vec<String> = parts
        .libraries
        .iter()
        .map(|l| {
            format!(
                "* [{}]({}/idea.md) - {} Label: `{}`.\n",
                l.name, l.path, l.description, l.label
            )
        })
        .collect();
    section(&mut out, "Libraries", &libraries);
    let components: Vec<String> = parts
        .components
        .iter()
        .map(|(file, built, _)| {
            format!("* [{file}]({file}) - A component built from {built}. Label: `trusted`.\n")
        })
        .collect();
    section(&mut out, "Components", &components);
    out
}

fn section(out: &mut String, heading: &str, lines: &[String]) {
    if !lines.is_empty() {
        out.push_str("\n# ");
        out.push_str(heading);
        out.push_str("\n\n");
        out.push_str(&lines.concat());
    }
}

fn change(previous: Option<&str>, now: Option<&str>) -> Option<&'static str> {
    match (previous, now) {
        (None, Some(_)) => Some("new"),
        (Some(_), None) => Some("removed"),
        (Some(a), Some(b)) if a != b => Some("changed"),
        _ => None,
    }
}

pub(crate) fn log_entry(pages: &[Page], now: u64, trust: &Trust) -> (String, Vec<Note>) {
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    for page in pages {
        let idea = change(page.previous_idea(), Some(page.idea_hash.as_str()));
        let code = change(page.previous_source(), page.source_hash.as_deref());
        if let Some(what) = idea {
            lines.push(format!(
                "  * Idea [{}]({}), {what}: `{}`.\n",
                page.title, page.file, page.idea_hash
            ));
        }
        if let Some(what) = code {
            let hash = page.source_hash.as_deref().unwrap_or("none");
            lines.push(format!("  * Code `{}`, {what}: `{hash}`.\n", page.source));
        }
        let drift = match (idea, code) {
            (Some("changed"), None) if page.source_hash.is_some() => Some(format!(
                "the idea `{}` changed and its code `{}` did not",
                page.file, page.source
            )),
            (None, Some("changed")) => Some(format!(
                "the code `{}` changed and its idea `{}` did not",
                page.source, page.file
            )),
            _ => None,
        };
        if let Some(message) = drift {
            lines.push(format!("  * Note: {message}.\n"));
            notes.push(Note {
                file: page.file.clone(),
                message,
            });
        }
        for stamp in &page.generated {
            if let Some(what) = change(page.front.value(stamp.key), Some(stamp.hash.as_str())) {
                lines.push(format!(
                    "  * Generated `{}`, {what}: `{}`.\n",
                    stamp.file, stamp.hash
                ));
            }
        }
    }
    let nothing = if lines.is_empty() {
        "; nothing changed"
    } else {
        ""
    };
    let head = format!(
        "* **Check** at {}: no problems; {}{nothing}.\n",
        time_text(now),
        trust.text()
    );
    (format!("{head}{}", lines.concat()), notes)
}

pub(crate) fn log_text(previous: &str, entry: &str, now: u64) -> String {
    let day = format!("## {}", day_text(now));
    if previous.trim().is_empty() {
        return format!("# Log\n\n{day}\n\n{entry}");
    }
    let lines: Vec<&str> = previous.split_inclusive('\n').collect();
    let first = lines.iter().position(|l| l.starts_with("## "));
    match first {
        Some(i) if lines[i].trim_end() == day => {
            let mut at = i + 1;
            while lines.get(at).is_some_and(|l| l.trim().is_empty()) {
                at += 1;
            }
            format!("{}\n{entry}{}", lines[..=i].concat(), lines[at..].concat())
        }
        Some(i) => format!(
            "{}{day}\n\n{entry}\n{}",
            lines[..i].concat(),
            lines[i..].concat()
        ),
        None => {
            let base = previous.trim_end_matches('\n');
            format!("{base}\n\n{day}\n\n{entry}")
        }
    }
}

fn write_if_changed(
    folder: &Path,
    file: &str,
    text: &str,
    written: &mut Vec<String>,
) -> Result<(), Finding> {
    let path = folder.join(file);
    if fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    let parent = path.parent().map_or(Ok(()), fs::create_dir_all);
    parent
        .and_then(|()| fs::write(&path, text))
        .map_err(|e| Finding {
            file: file.to_string(),
            line: 1,
            code: "wiki-write",
            message: format!("lengua could not write the file: {e}"),
        })?;
    written.push(file.to_string());
    Ok(())
}

pub(crate) struct Written {
    pub files: Vec<String>,
    pub notes: Vec<Note>,
    pub findings: Vec<Finding>,
}

pub(crate) fn write(
    folder: &Path,
    pages: &[Page],
    generated: &[(String, String)],
    now: u64,
    trust: &Trust,
    parts: &Parts,
) -> Written {
    let mut out = Written {
        files: Vec::new(),
        notes: Vec::new(),
        findings: Vec::new(),
    };
    let label = trust.label.name();
    let (entry, notes) = log_entry(pages, now, trust);
    out.notes = notes;
    let mut texts: Vec<(String, String)> = Vec::new();
    for page in pages {
        let used_by: Vec<String> = pages
            .iter()
            .filter(|other| other.uses.iter().any(|u| u == page.id()))
            .map(|other| other.id().to_string())
            .collect();
        texts.push((
            page.file.clone(),
            page_text(page, &used_by, label, parts.shape),
        ));
    }
    texts.push(("index.md".to_string(), index_text(pages, label, parts)));
    let previous = fs::read_to_string(folder.join("log.md"))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    texts.push(("log.md".to_string(), log_text(&previous, &entry, now)));
    texts.extend(generated.iter().cloned());
    for (file, text) in texts {
        if let Err(finding) = write_if_changed(folder, &file, &text, &mut out.files) {
            out.findings.push(finding);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{date, log_text, quoted, time_text};

    const NOV_13_2026: u64 = 1_794_528_000;

    #[test]
    fn dates_come_from_the_seconds() {
        assert_eq!(date(0), (1970, 1, 1));
        assert_eq!(date(951_782_400), (2000, 2, 29));
        assert_eq!(date(NOV_13_2026), (2026, 11, 13));
        assert_eq!(time_text(NOV_13_2026 + 3661), "01:01:01Z");
    }

    #[test]
    fn a_new_log_has_a_heading_and_the_entry() {
        assert_eq!(
            log_text("", "* one\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* one\n"
        );
    }

    #[test]
    fn a_later_entry_on_the_same_day_goes_first() {
        let old = "# Log\n\n## 2026-11-13\n\n* one\n";
        assert_eq!(
            log_text(old, "* two\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* two\n* one\n"
        );
    }

    #[test]
    fn a_new_day_goes_above_the_old_days() {
        let old = "# Log\n\n## 2026-11-12\n\n* one\n";
        assert_eq!(
            log_text(old, "* two\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* two\n\n## 2026-11-12\n\n* one\n"
        );
    }

    #[test]
    fn descriptions_are_quoted_for_yaml() {
        assert_eq!(quoted("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
    }
}
