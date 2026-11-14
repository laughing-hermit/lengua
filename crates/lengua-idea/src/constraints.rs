use crate::Sink;
use crate::idea::IdeaKind;
use crate::ste::Dictionary;
use crate::text::{Line, backticked, relative_path, snake, split_key, tokenize, upper_camel};

const KEYS: &str = "`Reads`, `Writes`, `Connects to`, `Listens on`, `Serves`, `Asks`, `Calls`, `Native`, `Memory`, `Time`, `Modules`, `Exports`, `Uses`, `Uses crate`, `Uses component`, `Uses package`, `Uses shadcn` and `Route`";
const MAX_MEMORY: u64 = 4 << 30;
const MAX_TIME_MS: u64 = 3_600_000;
const RESERVED: [&str; 4] = ["main", "ui", "index", "log"];
const TOP_ONLY: [&str; 15] = [
    "Native",
    "Asks",
    "Calls",
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
    "Uses shadcn",
    "Route",
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
    Ai,
    Outgoing,
    Secret,
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CrateUse {
    pub name: String,
    pub version: String,
    pub line: usize,
    pub feature: Option<(String, Option<&'static str>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Models,
    Threads,
    Device,
    Desktop,
}

pub(crate) const REASONS: [(&str, Reason); 6] = [
    ("large AI models", Reason::Models),
    ("the fastest local inference", Reason::Models),
    ("training or fine-tuning", Reason::Models),
    ("heavy compute with shared-memory threads", Reason::Threads),
    ("a device with no host grant", Reason::Device),
    ("the Electron shell of a desktop app", Reason::Desktop),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Own(String),
    Folder(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeLine {
    pub part: Part,
    pub reason: Option<Reason>,
    pub because: String,
    pub line: usize,
}

const SYSTEMS: [(&str, &str); 3] = [
    ("Linux", "linux"),
    ("macOS", "macos"),
    ("Windows", "windows"),
];

fn native_line(value: &str, line: usize) -> Option<NativeLine> {
    let (part, because) = value.split_once(", for ")?;
    let part = match backticked(part) {
        Some(folder) => library_path(folder).then(|| Part::Folder(folder.to_string()))?,
        None if !part.trim().is_empty() && !part.contains('`') => {
            Part::Own(part.trim().to_string())
        }
        None => return None,
    };
    let because = because.trim().to_string();
    let reason = REASONS
        .iter()
        .find(|(phrase, _)| because.starts_with(phrase))
        .map(|(_, reason)| *reason);
    (!because.is_empty()).then_some(NativeLine {
        part,
        reason,
        because,
        line,
    })
}

fn crate_feature(head: &str) -> Option<(&str, Feature)> {
    let Some((left, right)) = head.split_once(" with ") else {
        return Some((head, None));
    };
    let (feature, system) = match right.split_once(" on ") {
        Some((feature, name)) => {
            let found = SYSTEMS.iter().find(|(shown, _)| *shown == name.trim())?;
            (feature, Some(found.1))
        }
        None => (right, None),
    };
    let feature = backticked(feature).filter(|f| {
        !f.is_empty()
            && f.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })?;
    Some((left, Some((feature.to_string(), system))))
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
    pub packages: Vec<CrateUse>,
    pub shadcn: Vec<(String, usize)>,
    pub routes: Vec<(String, String, usize)>,
    pub ai: Option<(u64, u64)>,
    pub calls: Vec<String>,
    pub secrets: Vec<String>,
    pub natives: Vec<NativeLine>,
}

pub(crate) const SHADCN: [&str; 5] = ["button", "card", "input", "label", "textarea"];

fn package_name(name: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && !p.starts_with('.')
            && p.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
            })
    };
    match name.strip_prefix('@').and_then(|rest| rest.split_once('/')) {
        Some((scope, rest)) => part(scope) && part(rest),
        None => !name.starts_with('@') && part(name),
    }
}

fn package_line(
    value: &str,
    line: usize,
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Option<(String, String)> {
    let (head, reason) = value.split_once(", for ")?;
    let (name, version) = head.trim().rsplit_once(' ')?;
    let name = backticked(name).filter(|n| package_name(n))?;
    if reason.trim().is_empty() {
        return None;
    }
    let mut tokens = Vec::new();
    tokenize(reason, line, &mut tokens);
    dictionary.words(&tokens, sink);
    if !semver(version) {
        sink.add(
            line,
            "package-version",
            format!(
                "the package `{name}` is pinned to one exact version, `major.minor.patch`, such as `1.2.3`; `{version}` is not one"
            ),
        );
    }
    Some((name.to_string(), version.to_string()))
}

fn route_line(value: &str) -> Option<(String, String)> {
    let inner = backticked(value)?;
    let (method, path) = inner.split_once(' ')?;
    let methods = ["GET", "POST", "PUT", "DELETE", "PATCH"];
    let parts_ok = path.strip_prefix('/')?.split('/').all(|part| {
        let name = part.strip_prefix('{').and_then(|p| p.strip_suffix('}'));
        match name {
            Some(n) => snake(n),
            None => part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
        }
    });
    (methods.contains(&method) && parts_ok).then(|| (method.to_string(), path.to_string()))
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
        || secret(value).is_some()
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

fn secret(value: &str) -> Option<String> {
    let name = value.strip_prefix("the secret ").and_then(backticked)?;
    let good = name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    good.then(|| name.to_string())
}

fn asks(value: &str, line: usize, dictionary: &Dictionary, sink: &mut Sink) -> Option<(u64, u64)> {
    let rest = value.strip_prefix("an AI model, for ")?;
    let (reason, limits) = rest.rsplit_once(", up to ")?;
    let mut tokens = Vec::new();
    tokenize(reason, line, &mut tokens);
    dictionary.words(&tokens, sink);
    let (calls, tokens_text) = limits.split_once(" calls and ")?;
    let tokens_count = tokens_text.strip_suffix(" tokens")?;
    let calls = calls.trim().parse::<u64>().ok().filter(|n| *n > 0)?;
    let most = tokens_count.trim().parse::<u64>().ok().filter(|n| *n > 0)?;
    (!reason.trim().is_empty()).then_some((calls, most))
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

type Feature = Option<(String, Option<&'static str>)>;

fn crate_line(
    value: &str,
    line: usize,
    dictionary: &Dictionary,
    sink: &mut Sink,
) -> Option<(String, String, Feature)> {
    let (head, reason) = value.split_once(", for ")?;
    let (head, feature) = crate_feature(head.trim())?;
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
        .then(|| (name.to_string(), version.to_string(), feature))
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
            .or_else(|| self.reach(key, value, line))
            .or_else(|| self.front(key, value, line, sink));
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
            "Native" => {
                let found = native_line(value, line);
                let good = found.is_some();
                if let Some(native) = found {
                    if matches!(native.part, Part::Folder(_)) {
                        self.out.grants.push(Grant::Native);
                    }
                    self.out.natives.push(native);
                }
                (
                    good,
                    "``<what the part is>, for <reason>`` or `` `<folder of a native part>`, for <reason> ``",
                )
            }
            "Calls" => {
                let found = value
                    .strip_prefix("HTTP at ")
                    .map(|v| (v, ""))
                    .or_else(|| value.strip_prefix("HTTPS at ").map(|v| (v, "https://")))
                    .filter(|(v, _)| host_port(v))
                    .and_then(|(v, scheme)| backticked(v).map(|h| format!("{scheme}{h}")));
                let good = found.is_some();
                if let Some(host) = found {
                    self.out.grants.push(Grant::Outgoing);
                    self.out.calls.push(host);
                }
                (
                    good,
                    "``HTTP at `host:port` `` or ``HTTPS at `host:port` ``",
                )
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
                    "`standard input`, `arguments`, `the clock`, `random numbers`, ``files in `folder` `` or ``the secret `NAME` ``",
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
            "Asks" => {
                let found = asks(value, line, self.dictionary, sink);
                let again = found.is_some() && self.out.ai.is_some();
                if again {
                    sink.add(
                        line,
                        "constraint-duplicate",
                        "the idea has two `- Asks:` lines".to_string(),
                    );
                }
                if let Some(limits) = found {
                    self.out.grants.push(Grant::Ai);
                    self.out.ai = Some(limits);
                }
                (
                    found.is_some(),
                    "`an AI model, for <what>, up to <N> calls and <M> tokens`",
                )
            }
            "Modules" | "Uses" => self.listed(key, value, line),
            "Uses crate" => {
                let found = crate_line(value, line, self.dictionary, sink);
                let good = found.is_some();
                if let Some((name, version, feature)) = found {
                    self.out.crates.push(CrateUse {
                        name,
                        version,
                        line,
                        feature,
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

    fn front(
        &mut self,
        key: &str,
        value: &str,
        line: usize,
        sink: &mut Sink,
    ) -> Option<(bool, &'static str)> {
        Some(match key {
            "Uses package" => {
                let found = package_line(value, line, self.dictionary, sink);
                let good = found.is_some();
                if let Some((name, version)) = found {
                    self.out.packages.push(CrateUse {
                        name,
                        version,
                        line,
                        feature: None,
                    });
                }
                (good, "`` `name` 1.2.3, for <reason> ``")
            }
            "Uses shadcn" => {
                let list = names(value).unwrap_or_default();
                let good = !list.is_empty() && list.iter().all(|n| SHADCN.contains(&n.as_str()));
                if good {
                    self.out.shadcn.extend(list.into_iter().map(|n| (n, line)));
                }
                (
                    good,
                    "shadcn/ui component names in backticks, with commas between them: `button`, `card`, `input`, `label`, `textarea`",
                )
            }
            "Route" => {
                let found = route_line(value);
                let good = found.is_some();
                if let Some((method, path)) = found {
                    self.out.routes.push((method, path, line));
                }
                (
                    good,
                    "`` `<METHOD> <path>` ``: `GET`, `POST`, `PUT`, `DELETE` or `PATCH`, and a path from `/` whose parts may be `{name}`",
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
            _ if secret(value).is_some() => {
                self.out.secrets.extend(secret(value));
                Grant::Secret
            }
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
        let text = "- Reads: standard input\n- Reads: files in `data/`\n- Writes: files in `out`\n- Connects to: `example.com:443`\n- Memory: 16 MiB\n- Time: 2 s\n- Modules: `words`, `ui/summary`\n- Exports: `count`\n- Uses crate: `widgets` 0.2.0, for the screen\n";
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
                name: "widgets".to_string(),
                version: "0.2.0".to_string(),
                line: 10,
                feature: None,
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

    #[test]
    fn model_outgoing_and_secret_lines_are_read() {
        let text = "- Asks: an AI model, for a short summary, up to 3 calls and 2000 tokens\n- Calls: HTTP at `localhost:7813`\n- Calls: HTTPS at `api.example.com:443`\n- Reads: the secret `SESSION_KEY`\n- Memory: 16 MiB\n- Time: 2 s\n";
        let (codes, out) = run(IdeaKind::Program, text);
        assert_eq!(codes, Vec::<&str>::new());
        assert_eq!(out.ai, Some((3, 2000)));
        assert_eq!(out.calls, ["localhost:7813", "https://api.example.com:443"]);
        assert_eq!(out.secrets, ["SESSION_KEY"]);
        assert_eq!(
            out.grants,
            [Grant::Ai, Grant::Outgoing, Grant::Outgoing, Grant::Secret]
        );
        let bad = "- Asks: an AI model, for a summary\n- Asks: a person, up to 1 calls and 2 tokens\n- Calls: `localhost`\n- Reads: the secret `key`\n- Memory: 16 MiB\n- Time: 2 s\n";
        let (codes, _) = run(IdeaKind::Program, bad);
        assert_eq!(codes.len(), 4, "{codes:?}");
        assert!(codes.iter().all(|c| *c == "constraint-value"), "{codes:?}");
        let (module, _) = run(IdeaKind::Module, "- Calls: HTTP at `a:1`\n");
        assert_eq!(module, ["constraint-not-here"]);
    }

    #[test]
    fn native_lines_and_crate_features_are_read() {
        let text = "- Native: a small model on the GPU, for the fastest local inference\n- Native: `../model`, for the fastest local inference of the notes\n- Native: a window, for a faster screen\n- Uses crate: `candle-core` 0.11.1 with `metal` on macOS, for the numbers of the model\n- Memory: 16 MiB\n- Time: 2 s\n";
        let (codes, out) = run(IdeaKind::Program, text);
        assert_eq!(codes, Vec::<&str>::new());
        assert_eq!(
            out.natives
                .iter()
                .map(|n| (n.part.clone(), n.reason))
                .collect::<Vec<_>>(),
            [
                (
                    super::Part::Own("a small model on the GPU".to_string()),
                    Some(super::Reason::Models)
                ),
                (
                    super::Part::Folder("../model".to_string()),
                    Some(super::Reason::Models)
                ),
                (super::Part::Own("a window".to_string()), None),
            ]
        );
        assert_eq!(out.grants, [Grant::Native]);
        assert_eq!(
            out.crates.first().and_then(|c| c.feature.clone()),
            Some(("metal".to_string(), Some("macos")))
        );
        let bad = "- Native: `/etc`, for large AI models\n- Native: for large AI models\n- Uses crate: `candle-core` 0.11.1 with `metal` on Plan 9, for numbers\n- Memory: 16 MiB\n- Time: 2 s\n";
        let (codes, _) = run(IdeaKind::Program, bad);
        assert_eq!(codes, ["constraint-value"; 3], "{codes:?}");
        let (module, _) = run(IdeaKind::Module, "- Native: a part, for large AI models\n");
        assert_eq!(module, ["constraint-not-here"]);
    }
}
