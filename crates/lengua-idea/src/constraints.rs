use crate::Sink;
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{Line, backticked, relative_path, snake, split_key, tokenize};

const KEYS: &str = "`Reads`, `Writes`, `Connects to`, `Memory`, `Time`, `Modules`, `Exports`, `Uses` and `Uses crate`";
const MAX_MEMORY: u64 = 4 << 30;
const MAX_TIME_MS: u64 = 3_600_000;
const RESERVED: [&str; 4] = ["main", "ui", "index", "log"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Grant {
    StandardInput,
    Arguments,
    Clock,
    Random,
    Files,
    Network,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CrateUse {
    pub name: String,
    pub version: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Constraints {
    pub modules: Vec<(String, usize)>,
    pub uses: Vec<(String, usize)>,
    pub grants: Vec<Grant>,
    pub exports: bool,
    pub crates: Vec<CrateUse>,
}

pub(crate) fn module_name(name: &str) -> bool {
    let plain = name.strip_prefix("ui/").unwrap_or(name);
    snake(plain) && !RESERVED.contains(&plain)
}

fn names(value: &str) -> Option<Vec<String>> {
    value
        .split(',')
        .map(|part| backticked(part).map(str::to_string))
        .collect()
}

fn reads(value: &str) -> bool {
    matches!(
        value,
        "standard input" | "arguments" | "the clock" | "random numbers"
    ) || files_in(value)
}

fn files_in(value: &str) -> bool {
    value
        .strip_prefix("files in ")
        .and_then(backticked)
        .is_some_and(relative_path)
}

fn host_port(value: &str) -> bool {
    let Some((host, port)) = backticked(value).and_then(|v| v.rsplit_once(':')) else {
        return false;
    };
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    host_ok && port.parse::<u16>().is_ok_and(|p| p > 0)
}

fn amount(value: &str, units: &[(&str, u64)], max: u64) -> bool {
    let Some((number, unit)) = value.split_once(' ') else {
        return false;
    };
    let Some(scale) = units.iter().find(|(u, _)| *u == unit).map(|(_, s)| *s) else {
        return false;
    };
    number
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(scale))
        .is_some_and(|total| total > 0 && total <= max)
}

fn semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn crate_line(
    value: &str,
    line: usize,
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Option<(String, String)> {
    let (head, reason) = value.split_once(", for ")?;
    let (name, version) = head.trim().rsplit_once(' ')?;
    let mut tokens = Vec::new();
    tokenize(reason, line, &mut tokens);
    dictionary.words(&tokens, sink);
    let name = backticked(name).filter(|n| {
        !n.is_empty()
            && n.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    })?;
    (semver(version) && !reason.trim().is_empty())
        .then(|| (name.to_string(), version.to_string()))
}

struct Reader<'d> {
    top: bool,
    dictionary: &'d Dictionary,
    memory: bool,
    time: bool,
    out: Constraints,
}

pub(crate) fn parse(
    kind: IdeaKind,
    heading: usize,
    lines: &[Line],
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Constraints {
    let mut reader = Reader {
        top: kind == IdeaKind::Program,
        dictionary,
        memory: false,
        time: false,
        out: Constraints::default(),
    };
    let mut items: Vec<String> = Vec::new();
    for line in lines {
        let text = line.text.trim_end();
        if text.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = text.strip_prefix("- ").and_then(split_key) else {
            sink.add(
                line.number,
                "constraint-text",
                "each constraint is one line, `- Key: value`".to_string(),
            );
            continue;
        };
        if items.iter().any(|i| i == text) {
            sink.add(
                line.number,
                "constraint-duplicate",
                "this constraint is in the idea two times".to_string(),
            );
        }
        items.push(text.to_string());
        reader.item(key, value, line.number, sink);
    }
    if reader.top && !reader.memory {
        sink.add(
            heading,
            "constraint-memory-missing",
            "the idea has no `- Memory:` line, for example `- Memory: 16 MiB`".to_string(),
        );
    }
    if reader.top && !reader.time {
        sink.add(
            heading,
            "constraint-time-missing",
            "the idea has no `- Time:` line, for example `- Time: 2 s`".to_string(),
        );
    }
    reader.out
}

impl Reader<'_> {
    fn item(&mut self, key: &str, value: &str, line: usize, sink: &mut Sink) {
        let grant = matches!(
            key,
            "Reads" | "Writes" | "Connects to" | "Memory" | "Time" | "Modules" | "Exports"
        );
        if grant && !self.top {
            sink.add(
                line,
                "constraint-not-here",
                format!("`{key}` is only in the top idea, `idea.md`"),
            );
        }
        let again = match key {
            "Memory" => std::mem::replace(&mut self.memory, true),
            "Time" => std::mem::replace(&mut self.time, true),
            _ => false,
        };
        if again {
            sink.add(
                line,
                "constraint-duplicate",
                format!("the idea has two `- {key}:` lines"),
            );
        }
        let Some((good, form)) = self.value(key, value, line, sink) else {
            sink.add(
                line,
                "constraint-unknown",
                format!("`{key}` is not a constraint; the constraints are {KEYS}"),
            );
            return;
        };
        if !good {
            sink.add(
                line,
                "constraint-value",
                format!("the value of `{key}` is {form}"),
            );
        }
    }

    fn value(
        &mut self,
        key: &str,
        value: &str,
        line: usize,
        sink: &mut Sink,
    ) -> Option<(bool, &'static str)> {
        Some(match key {
            "Reads" => {
                let good = reads(value);
                if good {
                    self.grant(value);
                }
                (
                    good,
                    "`standard input`, `arguments`, `the clock`, `random numbers` or ``files in `folder` ``",
                )
            }
            "Writes" => {
                let good = files_in(value);
                if good {
                    self.out.grants.push(Grant::Files);
                }
                (good, "``files in `folder` ``")
            }
            "Connects to" => {
                let good = host_port(value);
                if good {
                    self.out.grants.push(Grant::Network);
                }
                (good, "`` `host:port` ``")
            }
            "Memory" => (
                amount(
                    value,
                    &[("KiB", 1 << 10), ("MiB", 1 << 20), ("GiB", 1 << 30)],
                    MAX_MEMORY,
                ),
                "a number and `KiB`, `MiB` or `GiB`, up to 4 GiB",
            ),
            "Time" => (
                amount(value, &[("ms", 1), ("s", 1000)], MAX_TIME_MS),
                "a number and `ms` or `s`, up to 3600 s",
            ),
            "Modules" | "Uses" => {
                let list = names(value);
                let good = list
                    .as_ref()
                    .is_some_and(|l| l.iter().all(|n| module_name(n)));
                let target = if key == "Modules" {
                    &mut self.out.modules
                } else {
                    &mut self.out.uses
                };
                target.extend(list.into_iter().flatten().map(|n| (n, line)));
                (good, "module names in backticks, with commas between them")
            }
            "Exports" => {
                let good = names(value).is_some_and(|l| l.iter().all(|n| snake(n)));
                self.out.exports |= good;
                (good, "function names in backticks, with commas between them")
            }
            "Uses crate" => {
                let found = crate_line(value, line, self.dictionary, sink);
                let good = found.is_some();
                if let Some((name, version)) = found {
                    self.out.crates.push(CrateUse {
                        name,
                        version,
                        line,
                    });
                }
                (good, "`` `name` 1.2.3, for <reason> ``")
            }
            _ => return None,
        })
    }

    fn grant(&mut self, value: &str) {
        self.out.grants.push(match value {
            "standard input" => Grant::StandardInput,
            "arguments" => Grant::Arguments,
            "the clock" => Grant::Clock,
            "random numbers" => Grant::Random,
            _ => Grant::Files,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{CrateUse, Grant, module_name, parse};
    use crate::Sink;
    use crate::idea::IdeaKind;
    use crate::ste::Dictionary;
    use crate::text::lines;

    fn run(kind: IdeaKind, text: &str) -> (Vec<&'static str>, super::Constraints) {
        let mut sink = Sink::new("idea.md");
        let out = parse(kind, 1, &lines(text, 2), &Dictionary::load(), &mut sink);
        (sink.findings.iter().map(|f| f.code).collect(), out)
    }

    #[test]
    fn good_grants_have_no_findings() {
        let text = "- Reads: standard input\n- Reads: files in `data/`\n- Writes: files in `out`\n- Connects to: `example.com:443`\n- Memory: 16 MiB\n- Time: 2 s\n- Modules: `words`, `ui/summary`\n- Exports: `count`\n- Uses crate: `gpui` 0.2.0, for the screen\n";
        let (codes, out) = run(IdeaKind::Program, text);
        assert_eq!(codes, Vec::<&str>::new());
        let modules: Vec<&str> = out.modules.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(modules, ["words", "ui/summary"]);
        assert_eq!(
            out.grants,
            [Grant::StandardInput, Grant::Files, Grant::Files, Grant::Network]
        );
        assert!(out.exports);
        assert_eq!(
            out.crates,
            [CrateUse {
                name: "gpui".to_string(),
                version: "0.2.0".to_string(),
                line: 10,
            }]
        );
    }

    #[test]
    fn a_program_needs_memory_and_time() {
        let (codes, _) = run(IdeaKind::Program, "- Reads: arguments\n");
        assert_eq!(
            codes,
            ["constraint-memory-missing", "constraint-time-missing"]
        );
    }

    #[test]
    fn bad_values_are_findings() {
        let text = "- Reads: the disk\n- Memory: 8 GiB\n- Memory: 1 MiB\n- Time: soon\n- Connects to: `host`\n- Colour: red\nloose\n- Modules: words\n";
        let (codes, _) = run(IdeaKind::Program, text);
        for code in [
            "constraint-value",
            "constraint-duplicate",
            "constraint-unknown",
            "constraint-text",
        ] {
            assert!(codes.contains(&code), "{code} in {codes:?}");
        }
    }

    #[test]
    fn grants_are_only_in_the_top_idea() {
        let (codes, out) = run(IdeaKind::Module, "- Reads: arguments\n- Uses: `pages`\n");
        assert_eq!(codes, ["constraint-not-here"]);
        assert_eq!(out.uses, [("pages".to_string(), 3)]);
    }

    #[test]
    fn module_names_are_snake_case_and_not_reserved() {
        assert!(module_name("cart"));
        assert!(module_name("ui/basket"));
        assert!(!module_name("Cart"));
        assert!(!module_name("main"));
        assert!(!module_name("ui/ui"));
    }
}
