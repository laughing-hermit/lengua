use crate::Sink;
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{Line, backticked, relative_path, snake, split_key, tokenize, upper_camel};

const KEYS: &str = "`Reads`, `Writes`, `Connects to`, `Listens on`, `Serves`, `Memory`, `Time`, `Modules`, `Exports`, `Uses`, `Uses crate` and `Uses component`";
const MAX_MEMORY: u64 = 4 << 30;
const MAX_TIME_MS: u64 = 3_600_000;
const RESERVED: [&str; 4] = ["main", "ui", "index", "log"];
const TOP_ONLY: [&str; 10] = [
    "Reads",
    "Writes",
    "Connects to",
    "Listens on",
    "Serves",
    "Memory",
    "Time",
    "Modules",
    "Exports",
    "Uses component",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Grant {
    StandardInput,
    Arguments,
    Clock,
    Random,
    Files,
    Network,
    Listen,
    Http,
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
    pub libraries: Vec<(String, usize)>,
    pub grants: Vec<Grant>,
    pub folders: Vec<(String, bool)>,
    pub hosts: Vec<String>,
    pub listens: Vec<u16>,
    pub serves: Option<(u16, usize)>,
    pub memory: u64,
    pub time_ms: u64,
    pub exports: Vec<(String, usize)>,
    pub crates: Vec<CrateUse>,
    pub components: Vec<(String, String, usize)>,
}

pub(crate) fn module_name(name: &str) -> bool {
    let plain = name.strip_prefix("ui/").unwrap_or(name);
    snake(plain) && !RESERVED.contains(&plain)
}

pub(crate) fn library_path(path: &str) -> bool {
    let path = path.strip_suffix('/').unwrap_or(path);
    path.contains('/')
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path.split('/').all(|segment| !segment.is_empty())
        && path
            .rsplit('/')
            .next()
            .is_some_and(|last| last != "." && last != "..")
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
    folder(value).is_some()
}

fn folder(value: &str) -> Option<String> {
    let path = value
        .strip_prefix("files in ")
        .and_then(backticked)
        .filter(|p| relative_path(p))?;
    Some(path.strip_suffix('/').unwrap_or(path).to_string())
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

fn tcp_port(value: &str) -> Option<u16> {
    value
        .strip_prefix("TCP port ")
        .and_then(|p| p.trim().parse::<u16>().ok())
        .filter(|p| *p > 0)
}

fn component_line(
    value: &str,
    line: usize,
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Option<(String, String)> {
    let (file, words) = value.split_once(", built from ")?;
    let mut tokens = Vec::new();
    tokenize(&format!("built from {words}"), line, &mut tokens);
    dictionary.words(&tokens, sink);
    let file = backticked(file).filter(|f| {
        relative_path(f)
            && std::path::Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wasm"))
    })?;
    (!words.trim().is_empty()).then(|| (file.to_string(), words.trim().to_string()))
}

fn amount(value: &str, units: &[(&str, u64)], max: u64) -> Option<u64> {
    let (number, unit) = value.split_once(' ')?;
    let scale = units.iter().find(|(u, _)| *u == unit).map(|(_, s)| *s)?;
    number
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(scale))
        .filter(|total| *total > 0 && *total <= max)
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
    (semver(version) && !reason.trim().is_empty()).then(|| (name.to_string(), version.to_string()))
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
        let library_use = key == "Uses" && value.contains('/');
        if (TOP_ONLY.contains(&key) || library_use) && !self.top {
            sink.add(
                line,
                "constraint-not-here",
                format!("`{key}` is only in the top idea, `idea.md`"),
            );
        }
        let again = match key {
            "Memory" => std::mem::replace(&mut self.memory, true),
            "Time" => std::mem::replace(&mut self.time, true),
            "Serves" => self.out.serves.is_some(),
            _ => false,
        };
        if again {
            sink.add(
                line,
                "constraint-duplicate",
                format!("the idea has two `- {key}:` lines"),
            );
        }
        let found = self
            .value(key, value, line, sink)
            .or_else(|| self.reach(key, value, line));
        let Some((good, form)) = found else {
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

    fn reach(&mut self, key: &str, value: &str, line: usize) -> Option<(bool, &'static str)> {
        Some(match key {
            "Connects to" => {
                let good = host_port(value);
                if good {
                    self.out.grants.push(Grant::Network);
                    if let Some(host) = backticked(value) {
                        self.out.hosts.push(host.to_string());
                    }
                }
                (good, "`` `host:port` ``")
            }
            "Listens on" => {
                let found = tcp_port(value);
                if let Some(port) = found {
                    self.out.grants.push(Grant::Listen);
                    self.out.listens.push(port);
                }
                (found.is_some(), "`TCP port <number>`, from 1 to 65535")
            }
            "Serves" => {
                let found = value.strip_prefix("HTTP on ").and_then(tcp_port);
                if let Some(port) = found {
                    self.out.grants.push(Grant::Http);
                    self.out.serves = Some((port, line));
                }
                (found.is_some(), "`HTTP on TCP port <number>`")
            }
            "Exports" => {
                let list = names(value);
                let good = list
                    .as_ref()
                    .is_some_and(|l| l.iter().all(|n| snake(n) || upper_camel(n)));
                if good {
                    self.out
                        .exports
                        .extend(list.into_iter().flatten().map(|n| (n, line)));
                }
                (
                    good,
                    "function and type names in backticks, with commas between them",
                )
            }
            _ => return None,
        })
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
                let found = folder(value);
                let good = found.is_some();
                if let Some(path) = found {
                    self.out.grants.push(Grant::Files);
                    self.out.folders.push((path, true));
                }
                (good, "``files in `folder` ``")
            }
            "Memory" => {
                let found = amount(
                    value,
                    &[("KiB", 1 << 10), ("MiB", 1 << 20), ("GiB", 1 << 30)],
                    MAX_MEMORY,
                );
                if let Some(bytes) = found {
                    self.out.memory = bytes;
                }
                (
                    found.is_some(),
                    "a number and `KiB`, `MiB` or `GiB`, up to 4 GiB",
                )
            }
            "Time" => {
                let found = amount(value, &[("ms", 1), ("s", 1000)], MAX_TIME_MS);
                if let Some(ms) = found {
                    self.out.time_ms = ms;
                }
                (found.is_some(), "a number and `ms` or `s`, up to 3600 s")
            }
            "Modules" | "Uses" => self.listed(key, value, line),
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
            "Uses component" => {
                let found = component_line(value, line, self.dictionary, sink);
                let good = found.is_some();
                if let Some((file, built)) = found {
                    self.out.components.push((file, built, line));
                }
                (
                    good,
                    "`` `<file>.wasm`, built from <what built it> ``, with the file inside the folder",
                )
            }
            _ => return None,
        })
    }

    fn listed(&mut self, key: &str, value: &str, line: usize) -> (bool, &'static str) {
        let list = names(value).unwrap_or_default();
        let mut good = !list.is_empty();
        for name in list {
            if key == "Uses" && name.contains('/') {
                good &= library_path(&name);
                self.out.libraries.push((name, line));
            } else {
                good &= module_name(&name);
                let target = if key == "Modules" {
                    &mut self.out.modules
                } else {
                    &mut self.out.uses
                };
                target.push((name, line));
            }
        }
        (
            good,
            "module names, or library folders such as `../libraries/json`, in backticks, with commas between them",
        )
    }

    fn grant(&mut self, value: &str) {
        self.out.grants.push(match value {
            "standard input" => Grant::StandardInput,
            "arguments" => Grant::Arguments,
            "the clock" => Grant::Clock,
            "random numbers" => Grant::Random,
            _ => {
                if let Some(path) = folder(value) {
                    self.out.folders.push((path, false));
                }
                Grant::Files
            }
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
            [
                Grant::StandardInput,
                Grant::Files,
                Grant::Files,
                Grant::Network
            ]
        );
        assert_eq!(out.exports, [("count".to_string(), 9)]);
        assert_eq!(
            out.folders,
            [("data".to_string(), false), ("out".to_string(), true)]
        );
        assert_eq!(out.hosts, ["example.com:443"]);
        assert_eq!(out.memory, 16 << 20);
        assert_eq!(out.time_ms, 2000);
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

    #[test]
    fn network_server_library_and_component_lines_are_read() {
        let text = "- Connects to: `127.0.0.1:7878`\n- Listens on: TCP port 7879\n- Serves: HTTP on TCP port 8080\n- Memory: 16 MiB\n- Time: 2 s\n- Uses: `../../libraries/json`, `cart`\n- Exports: `parse`, `Value`\n- Uses component: `components/tax.wasm`, built from C by `wasi-sdk`\n";
        let (codes, out) = run(IdeaKind::Program, text);
        assert_eq!(codes, Vec::<&str>::new());
        assert_eq!(out.hosts, ["127.0.0.1:7878"]);
        assert_eq!(out.listens, [7879]);
        assert_eq!(out.serves, Some((8080, 4)));
        assert_eq!(out.libraries, [("../../libraries/json".to_string(), 7)]);
        assert_eq!(out.uses, [("cart".to_string(), 7)]);
        assert_eq!(
            out.exports,
            [("parse".to_string(), 8), ("Value".to_string(), 8)]
        );
        assert_eq!(
            out.components,
            [(
                "components/tax.wasm".to_string(),
                "C by `wasi-sdk`".to_string(),
                9
            )]
        );
        assert_eq!(out.grants, [Grant::Network, Grant::Listen, Grant::Http]);
    }

    #[test]
    fn bad_network_and_library_lines_are_findings() {
        let text = "- Listens on: port 80\n- Serves: HTTPS on 443\n- Uses: `/etc/json`\n- Uses component: `tax.c`, built from C\n- Memory: 16 MiB\n- Time: 2 s\n";
        let (codes, _) = run(IdeaKind::Program, text);
        assert_eq!(codes.len(), 4, "{codes:?}");
        assert!(codes.iter().all(|c| *c == "constraint-value"), "{codes:?}");
        let (module, _) = run(
            IdeaKind::Module,
            "- Uses: `../json`\n- Uses component: `a.wasm`, built from C\n",
        );
        assert_eq!(module, ["constraint-not-here", "constraint-not-here"]);
        assert!(super::library_path("../libraries/json"));
        assert!(super::library_path("libraries/json/"));
        assert!(!super::library_path("json"));
        assert!(!super::library_path("../.."));
        assert!(!super::library_path("c:/json"));
    }
}
