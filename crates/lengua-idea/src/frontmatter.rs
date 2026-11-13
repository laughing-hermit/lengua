use crate::Sink;

pub(crate) const LENGUA_KEYS: [&str; 9] = [
    "type",
    "description",
    "module",
    "source",
    "uses",
    "used_by",
    "label",
    "idea_sha256",
    "source_sha256",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub key: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Front {
    pub entries: Vec<Entry>,
    pub body: String,
    pub body_line: usize,
}

impl Front {
    pub(crate) fn value(&self, key: &str) -> Option<&str> {
        let entry = self.entries.iter().find(|e| e.key == key)?;
        let first = entry.text.lines().next()?;
        Some(first[key.len() + 1..].trim())
    }

    pub(crate) fn body_text(&self) -> &str {
        self.body.trim_start_matches('\n')
    }

    pub(crate) fn render(&self, lengua: &[(&str, String)]) -> String {
        let mut out = String::from("---\n");
        for (key, value) in lengua {
            out.push_str(key);
            out.push_str(": ");
            out.push_str(value);
            out.push('\n');
        }
        for entry in &self.entries {
            if !LENGUA_KEYS.contains(&entry.key.as_str()) {
                out.push_str(&entry.text);
            }
        }
        out.push_str("---\n\n");
        out.push_str(self.body_text());
        out
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub(crate) fn split(text: &str, sink: &mut Sink) -> Front {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Front {
            entries: Vec::new(),
            body: text.to_string(),
            body_line: 1,
        };
    }
    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|p| p + 1)
    else {
        sink.add(
            1,
            "frontmatter-unclosed",
            "the frontmatter opens with `---` on line 1 but has no closing `---` line".to_string(),
        );
        let start = lines
            .iter()
            .position(|l| l.starts_with("# "))
            .unwrap_or(lines.len());
        return Front {
            entries: Vec::new(),
            body: lines[start..].concat(),
            body_line: start + 1,
        };
    };
    let mut entries: Vec<Entry> = Vec::new();
    for (i, raw) in lines[1..close].iter().enumerate() {
        let number = i + 2;
        let line = raw.trim_end_matches('\n');
        if line.starts_with('\t') {
            sink.add(
                number,
                "frontmatter-line",
                "this frontmatter line starts with a tab; YAML uses spaces".to_string(),
            );
            continue;
        }
        let blank = line.trim().is_empty() || line.starts_with('#');
        if blank || line.starts_with(' ') || line.starts_with('-') {
            if let Some(last) = entries.last_mut() {
                last.text.push_str(line);
                last.text.push('\n');
            } else if !blank {
                sink.add(
                    number,
                    "frontmatter-line",
                    "this frontmatter line continues a key, but no key comes before it".to_string(),
                );
            }
            continue;
        }
        match line.split_once(':') {
            Some((key, _)) if valid_key(key) => {
                if entries.iter().any(|e| e.key == key) {
                    sink.add(
                        number,
                        "frontmatter-key-duplicate",
                        format!("the key `{key}` is in the frontmatter two times"),
                    );
                }
                entries.push(Entry {
                    key: key.to_string(),
                    text: format!("{line}\n"),
                });
            }
            _ => sink.add(
                number,
                "frontmatter-line",
                "this frontmatter line is not `key: value`".to_string(),
            ),
        }
    }
    Front {
        entries,
        body: lines[close + 1..].concat(),
        body_line: close + 2,
    }
}

#[cfg(test)]
mod tests {
    use super::split;
    use crate::Sink;

    #[test]
    fn no_frontmatter_keeps_the_whole_text() {
        let mut sink = Sink::new("a.md");
        let front = split("# Purpose\n", &mut sink);
        assert!(front.entries.is_empty());
        assert_eq!(front.body, "# Purpose\n");
        assert_eq!(front.body_line, 1);
        assert!(sink.findings.is_empty());
    }

    #[test]
    fn agent_keys_are_kept_and_lengua_keys_are_replaced() {
        let mut sink = Sink::new("a.md");
        let text = "---\ntitle: Words\ntags:\n  - text\nlabel: checked\n---\n# Purpose\n";
        let front = split(text, &mut sink);
        assert!(sink.findings.is_empty());
        assert_eq!(front.value("label"), Some("checked"));
        assert_eq!(front.body_line, 7);
        let out = front.render(&[("label", "unchecked".to_string())]);
        assert_eq!(
            out,
            "---\nlabel: unchecked\ntitle: Words\ntags:\n  - text\n---\n\n# Purpose\n"
        );
    }

    #[test]
    fn a_broken_frontmatter_gives_findings() {
        let mut sink = Sink::new("a.md");
        let front = split("---\ntitle: a\ntitle: b\nno colon\n---\n", &mut sink);
        let codes: Vec<&str> = sink.findings.iter().map(|f| f.code).collect();
        assert_eq!(codes, ["frontmatter-key-duplicate", "frontmatter-line"]);
        assert_eq!(front.entries.len(), 2);
    }

    #[test]
    fn an_unclosed_frontmatter_is_a_finding_and_the_body_is_still_read() {
        let mut sink = Sink::new("a.md");
        let front = split("---\ntitle: a\n# Purpose\nText.\n", &mut sink);
        assert_eq!(sink.findings.len(), 1);
        assert_eq!(sink.findings[0].code, "frontmatter-unclosed");
        assert_eq!(front.body, "# Purpose\nText.\n");
        assert_eq!(front.body_line, 3);
    }
}
