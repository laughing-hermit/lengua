//! `spec.md`: a program's source of truth, written in Open Knowledge Format (OKF) markdown.
//!
//! The file starts with YAML frontmatter (`type: lengua/program-spec`, `title`, an optional
//! `description`), followed by markdown sections: `# Purpose`, `# Behaviors` with one
//! `## B<n>: <title>` heading per behavior written as Given/When/Then steps, and
//! `# Constraints`, which lists the system calls the program may use.

use std::fmt;

use serde::Deserialize;
use serde::de::IgnoredAny;

/// The OKF `type` that every lengua spec declares.
pub const SPEC_TYPE: &str = "lengua/program-spec";

/// The exit code reserved for failed assertions.
pub const ASSERTION_EXIT_CODE: u8 = 134;

/// At most this many problems are reported at once.
const MAX_PROBLEMS: usize = 20;

/// A parsed, valid spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// Short program name from the frontmatter.
    pub title: String,
    /// One-sentence summary from the frontmatter, if any.
    pub description: Option<String>,
    /// The behaviors, in file order.
    pub behaviors: Vec<Behavior>,
    /// System calls the program may use, in the order the spec lists them.
    pub syscalls: Vec<Syscall>,
    /// The backtick values in `# Purpose`, in order of first appearance: the ranges of the
    /// program's inputs, which generated tests draw from.
    pub purpose_values: Vec<String>,
}

impl Spec {
    /// Looks up a behavior by ID.
    #[must_use]
    pub fn behavior(&self, id: BehaviorId) -> Option<&Behavior> {
        self.behaviors.iter().find(|behavior| behavior.id == id)
    }
}

/// One Given/When/Then behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behavior {
    /// The behavior ID, such as `B1`.
    pub id: BehaviorId,
    /// The short title after the ID.
    pub title: String,
    /// Line of the behavior heading.
    pub line: usize,
    /// The steps, in order.
    pub steps: Vec<Step>,
    /// The exit code the behavior states.
    pub exit_code: u8,
}

impl Behavior {
    /// Backtick values in this behavior's steps, in order.
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.steps
            .iter()
            .flat_map(|step| step.values.iter().map(String::as_str))
    }
}

/// One step of a behavior, such as ``Given the arguments `2` and `3` ``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The Gherkin keyword that starts the step.
    pub keyword: Keyword,
    /// The step text after the keyword.
    pub text: String,
    /// Values written in backticks, in order.
    pub values: Vec<String>,
    /// Line of the step.
    pub line: usize,
}

/// A Gherkin step keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    /// The starting situation.
    Given,
    /// The action.
    When,
    /// An expected outcome.
    Then,
    /// Continues the previous kind of step.
    And,
    /// Continues the previous kind of step, as a contrast.
    But,
}

impl Keyword {
    fn parse(word: &str) -> Option<Self> {
        let word = word.trim_matches(|c: char| matches!(c, '*' | '_' | ':' | ','));
        match word.to_ascii_lowercase().as_str() {
            "given" => Some(Self::Given),
            "when" => Some(Self::When),
            "then" => Some(Self::Then),
            "and" => Some(Self::And),
            "but" => Some(Self::But),
            _ => None,
        }
    }
}

/// A behavior ID such as `B1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BehaviorId(u32);

impl BehaviorId {
    /// Parses `B<n>`, with n from 1 to 9999 written without leading zeros.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('B')?;
        let valid = (1..=4).contains(&digits.len())
            && !digits.starts_with('0')
            && digits.bytes().all(|byte| byte.is_ascii_digit());
        if valid {
            digits.parse().ok().map(Self)
        } else {
            None
        }
    }
}

impl fmt::Display for BehaviorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B{}", self.0)
    }
}

/// A system call a spec may allow. Programs can't use any other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Syscall {
    /// Read from a file descriptor.
    Read,
    /// Write to a file descriptor.
    Write,
    /// Open a file relative to a directory.
    Openat,
    /// Close a file descriptor.
    Close,
    /// Move a file offset.
    Lseek,
    /// End the program.
    Exit,
    /// End the program (all threads; lengua programs have one).
    ExitGroup,
}

impl Syscall {
    /// Every system call a spec may allow.
    pub const ALL: [Self; 7] = [
        Self::Read,
        Self::Write,
        Self::Openat,
        Self::Close,
        Self::Lseek,
        Self::Exit,
        Self::ExitGroup,
    ];

    /// The Linux name, as written in specs.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Openat => "openat",
            Self::Close => "close",
            Self::Lseek => "lseek",
            Self::Exit => "exit",
            Self::ExitGroup => "exit_group",
        }
    }

    /// Looks up a system call by its Linux name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|syscall| syscall.name() == name)
    }
}

/// Real system calls that lengua programs may never use, named so the message can say why.
const FORBIDDEN_SYSCALLS: &[&str] = &[
    "accept",
    "accept4",
    "bind",
    "brk",
    "chmod",
    "chown",
    "clone",
    "clone3",
    "connect",
    "creat",
    "dup",
    "dup2",
    "dup3",
    "execve",
    "execveat",
    "fcntl",
    "fork",
    "futex",
    "ioctl",
    "kill",
    "listen",
    "madvise",
    "mkdir",
    "mkdirat",
    "mmap",
    "mprotect",
    "mremap",
    "munmap",
    "open",
    "pipe",
    "pipe2",
    "prctl",
    "ptrace",
    "recvfrom",
    "recvmsg",
    "rename",
    "renameat",
    "rmdir",
    "rt_sigaction",
    "rt_sigprocmask",
    "sendmsg",
    "sendto",
    "shutdown",
    "sigaltstack",
    "socket",
    "tgkill",
    "tkill",
    "unlink",
    "unlinkat",
    "vfork",
    "wait4",
    "waitid",
];

/// A problem found in spec.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The line it's on, when it belongs to one.
    pub line: Option<usize>,
    /// What's wrong and how to fix it.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Parses and validates spec.md, reporting every problem found (up to 20).
///
/// # Errors
///
/// Returns the problems when the spec isn't valid.
pub fn parse(text: &str) -> Result<Spec, Vec<Problem>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut parser = Parser::default();
    let Some(end) = frontmatter_end(&lines) else {
        parser.problem(
            Some(1),
            "spec.md must start with YAML frontmatter: a line with ---, then \
             `type: lengua/program-spec` and `title: <name>`, then a line with ---",
        );
        return Err(parser.into_problems());
    };
    let header = parser.frontmatter(&lines[1..end].join("\n"));
    let mut fence = None;
    for (index, line) in lines.iter().enumerate().skip(end + 1) {
        let number = index + 1;
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fence = match fence {
                Some(_) => None,
                None => Some(number),
            };
            continue;
        }
        if fence.is_none() {
            parser.line(number, line);
        }
    }
    parser.finish_behavior();
    if let Some(line) = fence {
        parser.problem(Some(line), "this ``` code block is never closed");
    }
    parser.finish(header)
}

fn frontmatter_end(lines: &[&str]) -> Option<usize> {
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        return None;
    }
    lines
        .iter()
        .skip(1)
        .position(|line| line.trim_end() == "---")
        .map(|position| position + 1)
}

#[derive(Debug, Deserialize)]
struct Frontmatter {
    #[serde(rename = "type")]
    kind: Option<String>,
    title: Option<String>,
    description: Option<String>,
    status: Option<IgnoredAny>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Preamble,
    Purpose,
    Behaviors,
    Constraints,
    Other,
}

#[derive(Debug)]
struct Draft {
    id: Option<BehaviorId>,
    title: String,
    line: usize,
    steps: Vec<Step>,
}

#[derive(Debug)]
struct Parser {
    problems: Vec<Problem>,
    hidden_problems: usize,
    section: Section,
    sections: Vec<(Section, usize)>,
    current: Option<Draft>,
    started: Vec<(BehaviorId, usize)>,
    behaviors: Vec<Behavior>,
    syscalls: Option<Vec<Syscall>>,
    purpose_values: Vec<String>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            problems: Vec::new(),
            hidden_problems: 0,
            section: Section::Preamble,
            sections: Vec::new(),
            current: None,
            started: Vec::new(),
            behaviors: Vec::new(),
            syscalls: None,
            purpose_values: Vec::new(),
        }
    }
}

impl Parser {
    fn problem(&mut self, line: Option<usize>, message: impl Into<String>) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(Problem {
                line,
                message: message.into(),
            });
        } else {
            self.hidden_problems += 1;
        }
    }

    fn into_problems(mut self) -> Vec<Problem> {
        if self.hidden_problems > 0 {
            self.problems.push(Problem {
                line: None,
                message: format!("…and {} more problems", self.hidden_problems),
            });
        }
        self.problems
    }

    /// Returns the title and description when the frontmatter is valid.
    fn frontmatter(&mut self, yaml: &str) -> Option<(String, Option<String>)> {
        let parsed = if yaml.trim().is_empty() {
            Ok(Frontmatter {
                kind: None,
                title: None,
                description: None,
                status: None,
            })
        } else {
            serde_saphyr::from_str::<Frontmatter>(yaml)
        };
        let frontmatter = match parsed {
            Ok(frontmatter) => frontmatter,
            Err(error) => {
                let error = error.to_string();
                let first_line = error.lines().next().unwrap_or("unreadable YAML");
                self.problem(
                    Some(1),
                    format!("the frontmatter isn't valid YAML: {first_line}"),
                );
                return None;
            }
        };
        match frontmatter.kind.as_deref() {
            Some(SPEC_TYPE) => {}
            Some(other) => self.problem(
                Some(1),
                format!(
                    "the frontmatter says `type: {other}`; lengua specs use `type: {SPEC_TYPE}`"
                ),
            ),
            None => self.problem(
                Some(1),
                format!("the frontmatter has no `type`; add `type: {SPEC_TYPE}`"),
            ),
        }
        if frontmatter.status.is_some() {
            self.problem(
                Some(1),
                "remove `status` from the frontmatter: approval is recorded in lengua.lock, and \
                 any edit to spec.md, even to a status field, needs a new approval",
            );
        }
        let title = frontmatter
            .title
            .map(|title| title.trim().to_owned())
            .filter(|title| !title.is_empty());
        if title.is_none() {
            self.problem(
                Some(1),
                "the frontmatter has no `title`; add a short program name, such as `title: add`",
            );
        }
        let description = frontmatter
            .description
            .map(|description| description.trim().to_owned())
            .filter(|description| !description.is_empty());
        title.map(|title| (title, description))
    }

    fn line(&mut self, number: usize, line: &str) {
        if let Some(name) = heading(line, 1) {
            self.finish_behavior();
            self.enter_section(number, name);
            return;
        }
        match self.section {
            Section::Purpose => self.purpose_line(line),
            Section::Behaviors => {
                if let Some(text) = heading(line, 2) {
                    self.finish_behavior();
                    self.start_behavior(number, text);
                } else if !is_heading(line) {
                    self.behavior_line(number, line);
                }
            }
            Section::Constraints => self.constraint_line(number, line),
            Section::Preamble | Section::Other => {}
        }
    }

    /// Keeps Purpose's backtick values: the ranges of the program's inputs.
    fn purpose_line(&mut self, line: &str) {
        let (values, _) = backtick_values(line);
        for value in values {
            if !self.purpose_values.contains(&value) {
                self.purpose_values.push(value);
            }
        }
    }

    fn enter_section(&mut self, number: usize, name: &str) {
        let section = match name.to_ascii_lowercase().as_str() {
            "purpose" => Section::Purpose,
            "behaviors" | "behaviours" => Section::Behaviors,
            "constraints" => Section::Constraints,
            _ => Section::Other,
        };
        if section != Section::Other {
            if let Some(&(_, first)) = self.sections.iter().find(|(seen, _)| *seen == section) {
                self.problem(
                    Some(number),
                    format!("`# {name}` appears twice (also on line {first}); merge the two"),
                );
            }
            self.sections.push((section, number));
        }
        self.section = section;
    }

    fn start_behavior(&mut self, number: usize, text: &str) {
        let parsed = text.split_once(':').and_then(|(id, title)| {
            BehaviorId::parse(id.trim()).map(|id| (id, title.trim().to_owned()))
        });
        let Some((id, title)) = parsed else {
            self.problem(
                Some(number),
                "behavior headings look like `## B1: Adds two numbers` (B, a number, a colon, a \
                 short title)",
            );
            self.current = Some(Draft {
                id: None,
                title: String::new(),
                line: number,
                steps: Vec::new(),
            });
            return;
        };
        if let Some(&(_, first)) = self.started.iter().find(|(seen, _)| *seen == id) {
            self.problem(
                Some(number),
                format!("{id} is used twice (also on line {first}); give each behavior its own ID"),
            );
        }
        if title.is_empty() {
            self.problem(
                Some(number),
                format!("{id} needs a short title after the colon"),
            );
        }
        self.started.push((id, number));
        self.current = Some(Draft {
            id: Some(id),
            title,
            line: number,
            steps: Vec::new(),
        });
    }

    fn behavior_line(&mut self, number: usize, line: &str) {
        let item = list_item(line);
        let Some(draft) = self.current.as_mut() else {
            let starts_like_a_step = item
                .and_then(|item| item.split_whitespace().next())
                .is_some_and(|word| Keyword::parse(word).is_some());
            if starts_like_a_step {
                self.problem(
                    Some(number),
                    "this step isn't under a behavior heading such as `## B1: Adds two numbers`",
                );
            }
            return;
        };
        let label = draft
            .id
            .map_or_else(|| "this behavior".to_owned(), |id| id.to_string());
        if let Some(item) = item {
            let (word, rest) = item
                .trim()
                .split_once(char::is_whitespace)
                .unwrap_or((item.trim(), ""));
            let (values, closed) = backtick_values(rest);
            match Keyword::parse(word) {
                Some(keyword) => draft.steps.push(Step {
                    keyword,
                    text: rest.trim().to_owned(),
                    values,
                    line: number,
                }),
                None => self.problem(
                    Some(number),
                    format!("{label}: each step starts with Given, When, Then, And or But"),
                ),
            }
            if !closed {
                self.problem(Some(number), format!("{label}: a backtick isn't closed"));
            }
        } else if line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            // An indented line continues the previous step.
            let (values, closed) = backtick_values(line);
            if let Some(step) = draft.steps.last_mut() {
                step.text.push(' ');
                step.text.push_str(line.trim());
                step.values.extend(values);
            }
            if !closed {
                self.problem(Some(number), format!("{label}: a backtick isn't closed"));
            }
        }
    }

    fn finish_behavior(&mut self) {
        let Some(draft) = self.current.take() else {
            return;
        };
        let Some(id) = draft.id else {
            return;
        };
        if let Some(exit_code) = self.check_steps(id, draft.line, &draft.steps) {
            self.behaviors.push(Behavior {
                id,
                title: draft.title,
                line: draft.line,
                steps: draft.steps,
                exit_code,
            });
        }
    }

    /// Checks the Given → When → Then order and returns the stated exit code.
    fn check_steps(&mut self, id: BehaviorId, line: usize, steps: &[Step]) -> Option<u8> {
        if steps.is_empty() {
            self.problem(
                Some(line),
                format!("{id} has no steps; write them as `- Given …`, `- When …`, `- Then …`"),
            );
            return None;
        }
        let mut phase: Option<Keyword> = None;
        let mut exit_steps = Vec::new();
        for step in steps {
            match step.keyword {
                Keyword::Given => {
                    if phase.is_some_and(|phase| phase != Keyword::Given) {
                        self.problem(
                            Some(step.line),
                            format!("{id}: Given steps come before When and Then"),
                        );
                    }
                    phase = Some(Keyword::Given);
                }
                Keyword::When => {
                    if phase.is_some_and(|phase| phase != Keyword::Given) {
                        self.problem(
                            Some(step.line),
                            format!(
                                "{id}: a behavior has one When, after its Given steps; split \
                                 this into two behaviors"
                            ),
                        );
                    }
                    phase = Some(Keyword::When);
                }
                Keyword::Then => {
                    if phase.is_some_and(|phase| phase == Keyword::Given) {
                        self.problem(
                            Some(step.line),
                            format!("{id}: Then comes after a When step"),
                        );
                    }
                    phase = Some(Keyword::Then);
                }
                Keyword::And | Keyword::But => {
                    if phase.is_none() {
                        self.problem(
                            Some(step.line),
                            format!("{id}: the first step must be Given, not And or But"),
                        );
                    }
                }
            }
            if phase == Some(Keyword::Then) && states_exit_code(&step.text) {
                exit_steps.push(step);
            }
        }
        let missing: Vec<&str> = [Keyword::Given, Keyword::When, Keyword::Then]
            .into_iter()
            .filter(|keyword| !steps.iter().any(|step| step.keyword == *keyword))
            .map(|keyword| match keyword {
                Keyword::Given => "Given",
                Keyword::When => "When",
                _ => "Then",
            })
            .collect();
        if !missing.is_empty() {
            self.problem(
                Some(line),
                format!(
                    "{id} needs Given, When and Then steps; it has no {}",
                    missing.join(" and no ")
                ),
            );
        }
        match exit_steps.as_slice() {
            [] => {
                self.problem(
                    Some(line),
                    format!(
                        "{id} doesn't say how the program exits; add a step such as \
                         `- And it exits with code `0``"
                    ),
                );
                None
            }
            [step] => self.exit_code(id, step),
            [first, second, ..] => {
                self.problem(
                    Some(second.line),
                    format!(
                        "{id} states its exit code twice (also on line {}); keep one",
                        first.line
                    ),
                );
                None
            }
        }
    }

    fn exit_code(&mut self, id: BehaviorId, step: &Step) -> Option<u8> {
        let numbers: Vec<i64> = step
            .values
            .iter()
            .filter_map(|value| value.parse().ok())
            .collect();
        let [code] = numbers.as_slice() else {
            self.problem(
                Some(step.line),
                format!("{id}: write exactly one exit code as a number in backticks, like `0`"),
            );
            return None;
        };
        match u8::try_from(*code) {
            Ok(ASSERTION_EXIT_CODE) => {
                self.problem(
                    Some(step.line),
                    format!(
                        "{id}: exit code {ASSERTION_EXIT_CODE} is reserved for failed \
                         assertions; choose another code"
                    ),
                );
                None
            }
            Ok(code) => Some(code),
            Err(_) => {
                self.problem(
                    Some(step.line),
                    format!("{id}: exit codes go from 0 to 255, not {code}"),
                );
                None
            }
        }
    }

    fn constraint_line(&mut self, number: usize, line: &str) {
        let Some(item) = list_item(line) else {
            return;
        };
        let item = item.trim();
        let Some(rest) = strip_prefix_ignore_case(item, "allowed system calls:") else {
            return;
        };
        if self.syscalls.is_some() {
            self.problem(
                Some(number),
                "the allowed system calls are listed twice; keep one line",
            );
            return;
        }
        let allowed = Syscall::ALL.map(Syscall::name).join(", ");
        let mut syscalls = Vec::new();
        for name in rest.split(',').flat_map(|part| part.split(" and ")) {
            let name = name.trim().trim_end_matches('.').trim_matches('`').trim();
            if name.is_empty() {
                continue;
            }
            match Syscall::from_name(name) {
                Some(syscall) if !syscalls.contains(&syscall) => syscalls.push(syscall),
                Some(_) => {}
                None if FORBIDDEN_SYSCALLS.contains(&name) => self.problem(
                    Some(number),
                    format!(
                        "`{name}` isn't allowed: lengua programs can't allocate memory, start \
                         processes or threads, handle signals, use the network or change the \
                         file system. Allowed: {allowed}"
                    ),
                ),
                None => self.problem(
                    Some(number),
                    format!("`{name}` isn't a system call lengua knows. Allowed: {allowed}"),
                ),
            }
        }
        if !syscalls.contains(&Syscall::Exit) {
            self.problem(
                Some(number),
                "allow `exit`: every program ends with it, including after a failed assertion",
            );
        }
        self.syscalls = Some(syscalls);
    }

    fn finish(mut self, header: Option<(String, Option<String>)>) -> Result<Spec, Vec<Problem>> {
        if !self
            .sections
            .iter()
            .any(|(section, _)| *section == Section::Behaviors)
        {
            self.problem(
                None,
                "spec.md needs a `# Behaviors` section with at least one behavior, such as \
                 `## B1: Adds two numbers`",
            );
        } else if self.started.is_empty() {
            self.problem(
                None,
                "`# Behaviors` has no behaviors; add one, such as `## B1: Adds two numbers`",
            );
        }
        if self.syscalls.is_none() {
            self.problem(
                None,
                "add `- Allowed system calls: …` under `# Constraints`, listing only what the \
                 program uses, for example `- Allowed system calls: read, write, exit`",
            );
        }
        match (header, self.syscalls.take()) {
            (Some((title, description)), Some(syscalls)) if self.problems.is_empty() => Ok(Spec {
                title,
                description,
                behaviors: self.behaviors,
                syscalls,
                purpose_values: self.purpose_values,
            }),
            _ => Err(self.into_problems()),
        }
    }
}

/// The heading text when `line` is a heading of exactly `level`.
fn heading(line: &str, level: usize) -> Option<&str> {
    let hashes = line.bytes().take_while(|&byte| byte == b'#').count();
    if hashes != level {
        return None;
    }
    let rest = &line[level..];
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix([' ', '\t']).map(str::trim)
}

fn is_heading(line: &str) -> bool {
    (1..=6).any(|level| heading(line, level).is_some())
}

/// The item text when `line` is a top-level list item (`- `, `* `, `+ ` or `1. `).
fn list_item(line: &str) -> Option<&str> {
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return Some(rest);
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    line[digits..]
        .strip_prefix(". ")
        .or_else(|| line[digits..].strip_prefix(") "))
}

/// Values written in backticks, and whether every backtick span is closed.
fn backtick_values(text: &str) -> (Vec<String>, bool) {
    let mut values = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after = &rest[start..];
        let run = after.bytes().take_while(|&byte| byte == b'`').count();
        let fence = &after[..run];
        let body = &after[run..];
        let Some(end) = find_exact_run(body, run) else {
            return (values, false);
        };
        let value = &body[..end];
        if !value.is_empty() {
            values.push(value.to_owned());
        }
        rest = &body[end + fence.len()..];
    }
    (values, true)
}

/// Finds a run of exactly `run` backticks.
fn find_exact_run(text: &str, run: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let length = bytes[index..]
                .iter()
                .take_while(|&&byte| byte == b'`')
                .count();
            if length == run {
                return Some(index);
            }
            index += length;
        } else {
            index += 1;
        }
    }
    None
}

/// Whether a Then step states the exit code, judged by its words outside backticks.
fn states_exit_code(text: &str) -> bool {
    let mut words = String::new();
    let mut inside = false;
    for character in text.chars() {
        if character == '`' {
            inside = !inside;
        } else if !inside {
            words.push(character.to_ascii_lowercase());
        }
    }
    words.contains("exit") && (words.contains("code") || words.contains("status"))
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADD: &str = "---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
";

    fn problems(text: &str) -> Vec<String> {
        match parse(text) {
            Ok(spec) => panic!("expected problems, got {spec:?}"),
            Err(problems) => problems.iter().map(ToString::to_string).collect(),
        }
    }

    fn assert_problem(text: &str, needle: &str) {
        let problems = problems(text);
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "no problem containing {needle:?} in {problems:#?}"
        );
    }

    #[test]
    fn parses_the_add_spec() {
        let spec = parse(ADD).expect("valid spec");
        assert_eq!(spec.title, "add");
        assert_eq!(
            spec.description.as_deref(),
            Some("Adds two whole numbers given as arguments and prints the sum.")
        );
        assert_eq!(spec.behaviors.len(), 2);
        let first = &spec.behaviors[0];
        assert_eq!(first.id.to_string(), "B1");
        assert_eq!(first.title, "Adds two numbers");
        assert_eq!(first.line, 10);
        assert_eq!(first.exit_code, 0);
        assert_eq!(first.values().collect::<Vec<_>>(), ["2", "3", "5", "0"]);
        assert_eq!(spec.behaviors[1].exit_code, 1);
        assert_eq!(
            spec.behaviors[1].values().collect::<Vec<_>>(),
            ["usage: add <a> <b>", "1"]
        );
        assert_eq!(
            spec.syscalls,
            [Syscall::Read, Syscall::Write, Syscall::Exit]
        );
        // Only Purpose states ranges; the behaviors' values (2, 3, 5, 0, 1) aren't among them.
        assert_eq!(spec.purpose_values, ["-1000000000", "1000000000"]);
        let id = BehaviorId::parse("B2").expect("valid ID");
        assert_eq!(
            spec.behavior(id).map(|b| b.title.as_str()),
            Some("Missing arguments")
        );
    }

    #[test]
    fn accepts_windows_line_endings() {
        let spec = parse(&ADD.replace('\n', "\r\n")).expect("valid spec");
        assert_eq!(spec.behaviors.len(), 2);
    }

    #[test]
    fn requires_frontmatter() {
        assert_problem("# Behaviors\n", "must start with YAML frontmatter");
        assert_problem("---\ntitle: add\n", "must start with YAML frontmatter");
    }

    #[test]
    fn requires_the_lengua_type() {
        assert_problem(
            &ADD.replace("type: lengua/program-spec\n", ""),
            "has no `type`",
        );
        assert_problem(
            &ADD.replace("lengua/program-spec", "concept"),
            "says `type: concept`",
        );
    }

    #[test]
    fn requires_a_title() {
        assert_problem(&ADD.replace("title: add\n", ""), "has no `title`");
        assert_problem(
            &ADD.replace("title: add", "title: \"  \""),
            "has no `title`",
        );
    }

    #[test]
    fn rejects_a_status_field() {
        assert_problem(
            &ADD.replace("title: add\n", "title: add\nstatus: approved\n"),
            "remove `status`",
        );
    }

    #[test]
    fn reports_invalid_yaml() {
        assert_problem(
            &ADD.replace("title: add", "title: [unclosed"),
            "isn't valid YAML",
        );
    }

    #[test]
    fn rejects_duplicate_ids() {
        assert_problem(
            &ADD.replace("B2:", "B1:"),
            "B1 is used twice (also on line 10)",
        );
    }

    #[test]
    fn rejects_malformed_headings() {
        assert_problem(
            &ADD.replace("## B2: Missing arguments", "## Missing arguments"),
            "behavior headings look like",
        );
        assert_problem(
            &ADD.replace("## B2:", "## B02:"),
            "behavior headings look like",
        );
        assert_problem(
            &ADD.replace("## B2: Missing arguments", "## B2:"),
            "short title",
        );
    }

    #[test]
    fn requires_given_when_and_then() {
        let without_then = ADD.replace(
            "- Then it prints `5` followed by a newline\n- And it exits with code `0`\n",
            "",
        );
        assert_problem(
            &without_then,
            "B1 needs Given, When and Then steps; it has no Then",
        );
        assert_problem(
            &ADD.replace(
                "- When the program runs\n- Then it prints `5`",
                "- Then it prints `5`",
            ),
            "Then comes after a When step",
        );
    }

    #[test]
    fn keeps_steps_in_order() {
        assert_problem(
            &ADD.replace(
                "- And it exits with code `0`\n",
                "- And it exits with code `0`\n- When it runs again\n",
            ),
            "a behavior has one When",
        );
        assert_problem(
            &ADD.replace("- Given the arguments", "- And the arguments"),
            "the first step must be Given",
        );
        assert_problem(
            &ADD.replace(
                "- When the program runs\n- Then it prints `5`",
                "- When the program runs\n- Given more\n- Then it prints `5`",
            ),
            "Given steps come before When and Then",
        );
        assert_problem(
            &ADD.replace(
                "- When the program runs\n- Then it prints `5`",
                "- When the program runs\n- Afterwards it prints `5`",
            ),
            "each step starts with Given, When, Then, And or But",
        );
    }

    #[test]
    fn requires_one_valid_exit_code() {
        assert_problem(
            &ADD.replace("exits with code `1`", "exits with code `134`"),
            "134 is reserved for failed assertions",
        );
        assert_problem(
            &ADD.replace("exits with code `1`", "exits with code `256`"),
            "exit codes go from 0 to 255, not 256",
        );
        assert_problem(
            &ADD.replace("exits with code `1`", "exits with an error code"),
            "exactly one exit code",
        );
        assert_problem(
            &ADD.replace("- And it exits with code `0`\n", ""),
            "B1 doesn't say how the program exits",
        );
        assert_problem(
            &ADD.replace(
                "- And it exits with code `0`\n",
                "- And it exits with code `0`\n- And its exit status is `0`\n",
            ),
            "states its exit code twice",
        );
    }

    #[test]
    fn exit_words_inside_backticks_dont_count() {
        let text = ADD.replace(
            "- Then it prints `5` followed by a newline",
            "- Then it prints `exit code` followed by a newline",
        );
        let spec = parse(&text).expect("valid spec");
        assert_eq!(spec.behaviors[0].exit_code, 0);
    }

    #[test]
    fn checks_system_calls() {
        assert_problem(
            &ADD.replace("read, write, exit", "read, write, exit, mmap"),
            "`mmap` isn't allowed",
        );
        assert_problem(
            &ADD.replace("read, write, exit", "read, frobnicate, exit"),
            "`frobnicate` isn't a system call lengua knows",
        );
        assert_problem(
            &ADD.replace("read, write, exit", "read, write"),
            "allow `exit`",
        );
        assert_problem(
            &ADD.replace("- Allowed system calls: read, write, exit\n", ""),
            "add `- Allowed system calls: …`",
        );
        let spec = parse(&ADD.replace("read, write, exit", "`read`, `write` and `exit`."))
            .expect("valid spec");
        assert_eq!(
            spec.syscalls,
            [Syscall::Read, Syscall::Write, Syscall::Exit]
        );
    }

    #[test]
    fn requires_behaviors() {
        let end = ADD.find("# Behaviors").expect("behaviors section");
        let constraints = ADD.find("# Constraints").expect("constraints section");
        let without = format!("{}{}", &ADD[..end], &ADD[constraints..]);
        assert_problem(&without, "needs a `# Behaviors` section");
        let empty = format!("{}# Behaviors\n\n{}", &ADD[..end], &ADD[constraints..]);
        assert_problem(&empty, "has no behaviors");
    }

    #[test]
    fn ignores_code_blocks_and_flags_unclosed_ones() {
        let with_block = ADD.replace(
            "# Constraints",
            "```\n## B9: not a behavior\n- Then nothing\n```\n# Constraints",
        );
        assert_eq!(parse(&with_block).expect("valid spec").behaviors.len(), 2);
        assert_problem(
            &ADD.replace("# Constraints", "```\n# Constraints"),
            "code block is never closed",
        );
    }

    #[test]
    fn flags_unclosed_backticks_in_steps() {
        assert_problem(
            &ADD.replace("the arguments `2` and `3`", "the arguments `2 and `3`"),
            "B1: a backtick isn't closed",
        );
    }

    #[test]
    fn reads_double_backtick_values_and_continuations() {
        let text = ADD.replace(
            "- Then it prints `5` followed by a newline",
            "- Then it prints ``a`b``\n  and then `5`",
        );
        let spec = parse(&text).expect("valid spec");
        assert_eq!(
            spec.behaviors[0].values().collect::<Vec<_>>(),
            ["2", "3", "a`b", "5", "0"]
        );
    }

    #[test]
    fn reports_every_problem_at_once() {
        let text = ADD
            .replace("title: add\n", "")
            .replace("B2:", "B1:")
            .replace("read, write, exit", "read, write");
        assert_eq!(problems(&text).len(), 3, "{:#?}", problems(&text));
    }

    #[test]
    fn caps_the_number_of_problems() {
        let mut text = ADD.to_owned();
        for _ in 0..30 {
            text.push_str("# Behaviors\n");
        }
        let problems = problems(&text);
        assert_eq!(problems.len(), MAX_PROBLEMS + 1);
        assert!(
            problems
                .last()
                .is_some_and(|last| last.contains("more problems"))
        );
    }

    #[test]
    fn parses_behavior_ids() {
        assert_eq!(
            BehaviorId::parse("B1").map(|id| id.to_string()),
            Some("B1".into())
        );
        assert_eq!(
            BehaviorId::parse("B9999").map(|id| id.to_string()),
            Some("B9999".into())
        );
        for invalid in ["B0", "B01", "b1", "B", "B10000", "B1a", "1"] {
            assert_eq!(BehaviorId::parse(invalid), None, "{invalid}");
        }
    }
}
