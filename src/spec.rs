//! `spec.md`: a program's source of truth, written in Open Knowledge Format (OKF) markdown.
//!
//! The file starts with YAML frontmatter (`type: lengua/program-spec`, `title`, an optional
//! `description`), followed by markdown sections: `# Purpose`, `# Behaviors` with one
//! `## B<n>: <title>` heading per behavior written as Given/When/Then steps, each followed by its
//! generated tests under a `###` heading, and `# Constraints`, which names what the program uses:
//! `- Uses:`, `- Network:` and `- Threads:` lines. Nothing in it is particular to one computer.

use std::fmt;

use serde::Deserialize;

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
    /// What the program may use beyond its output, as the spec names it.
    pub uses: Vec<Use>,
    /// The system calls that allows on this target, worked out from `uses`, the network and the
    /// output every program has.
    pub syscalls: Vec<Syscall>,
    /// The edge's operations the same words allow: what a program in the edge form may call.
    pub ops: Vec<crate::edge::Op>,
    /// How many threads the program may run at once, when the spec allows more than one.
    pub threads: Option<u32>,
    /// The backtick values in `# Purpose`, in order of first appearance: the ranges of the
    /// program's inputs, which generated tests draw from.
    pub purpose_values: Vec<String>,
    /// What it may do on the network.
    pub network: Network,
    /// The programs it builds on, each a folder with its own spec and code: the nodes it links to
    /// in the graph of programs.
    pub parts: Vec<Part>,
}

/// A program another builds on, as its spec links to it: `[sort lines](../sort/spec.md)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// What the link calls it.
    pub name: String,
    /// Its spec.md, relative to the folder of the spec that links to it.
    pub spec: String,
}

impl Part {
    /// Its folder, relative to the folder of the spec that links to it.
    #[must_use]
    pub fn folder(&self) -> String {
        folder_of_link(&self.spec)
    }
}

/// The folder a link to a part names, without a trailing `spec.md`, `main.s` or slash: `../sort`
/// for `../sort/spec.md`.
#[must_use]
pub fn folder_of_link(link: &str) -> String {
    let trimmed = link.trim().trim_end_matches('/');
    let trimmed = trimmed
        .strip_suffix("/spec.md")
        .or_else(|| trimmed.strip_suffix("/main.s"))
        .unwrap_or(trimmed);
    trimmed.trim_end_matches('/').to_owned()
}

/// The most clients a server may take at once: a table of this many connections is a few
/// pages of `.bss`, and `poll` goes through all of them on every wake.
pub const MAX_AT_ONCE: u32 = 4096;

/// What a spec lets a program do on the network: plain TCP to the places it names, and a port
/// it serves on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Network {
    /// The connections it gets, in order. lengua makes each before the program starts and
    /// hands it over as file descriptors 4, 5 and on, so the program never connects by itself
    /// and holds no network of its own, on every kind of computer.
    pub connections: Vec<Endpoint>,
    /// The port it serves on. lengua binds it and hands the program the listening socket as
    /// file descriptor 3, so the program never binds or listens itself.
    pub listens: Option<u16>,
    /// Whether other computers may connect to the port it serves on, which its spec says with
    /// `for any computer`: lengua then listens on every address of this computer, not only the
    /// loopback. Tests always use the loopback.
    pub serves_anywhere: bool,
    /// How many clients it serves at once, which its spec says with `up to N clients at once`:
    /// it then waits on all of them, and on its listening socket, with `poll`. One at a time
    /// when `None`.
    pub at_once: Option<u32>,
}

impl Network {
    /// Whether the program uses the network at all.
    #[must_use]
    pub fn is_used(&self) -> bool {
        !self.connections.is_empty() || self.listens.is_some()
    }
}

/// The most connections a spec may name.
pub const MAX_CONNECTIONS: usize = crate::connect::MAX_CONNECTIONS;

/// One connection a spec names: a TCP port of this computer, of another computer, or of the
/// computer one of the program's arguments names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint {
    /// The computer, by name or address, in lower case; `None` for this computer, or for the
    /// one an argument names.
    pub host: Option<String>,
    /// Which of the program's arguments, from 1, names the computer, when the spec says so:
    /// `on the computer its first argument names`. lengua reads that argument when the program
    /// starts and connects there, as a curl-like tool's user types the computer; its tests' server
    /// stands in for any value the argument takes.
    pub argument: Option<usize>,
    /// Its TCP port.
    pub port: u16,
    /// Whether lengua does TLS on it, checking the computer's certificate, so the program reads
    /// and writes plain bytes: `over TLS`, which needs a named computer.
    pub tls: bool,
}

impl Endpoint {
    /// In words: `TCP port 8080 of this computer`, `TCP port 443 on example.com over TLS`.
    #[must_use]
    pub fn describe(&self) -> String {
        let over = if self.tls { " over TLS" } else { "" };
        match (&self.host, self.argument) {
            (_, Some(index)) => format!(
                "TCP port {} on the computer its {} argument names{over}",
                self.port,
                ORDINALS[index - 1]
            ),
            (None, None) => format!("TCP port {} of this computer{over}", self.port),
            (Some(host), None) => format!("TCP port {} on {host}{over}", self.port),
        }
    }
}

/// The arguments a connection's computer can come from, by their place: `first` to `ninth`.
const ORDINALS: [&str; 9] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];

/// Which argument, from 1, `place` says names the computer: `the computer its first argument
/// names`; `None` for anything else.
fn argument_named(place: &str) -> Option<usize> {
    let ordinal = place
        .strip_prefix("the computer its ")?
        .strip_suffix(" argument names")?;
    ORDINALS
        .iter()
        .position(|word| *word == ordinal)
        .map(|at| at + 1)
}

/// What a computer's name or address names: another computer, this one, or nothing a
/// connection can go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// Another computer, by a name as DNS writes it or by its address.
    Other,
    /// This computer: its loopback, `localhost`, or every address at once.
    ThisComputer,
    /// Neither a computer's name nor an address.
    NotAName,
}

/// What `host`, in lower case, names.
#[must_use]
pub fn host_kind(host: &str) -> HostKind {
    if let Ok(address) = host.parse::<std::net::IpAddr>() {
        return if address.is_loopback() || address.is_unspecified() {
            HostKind::ThisComputer
        } else {
            HostKind::Other
        };
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return HostKind::ThisComputer;
    }
    let label_ok = |label: &str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let name = host.strip_suffix('.').unwrap_or(host);
    if name.len() > 253 || !name.split('.').all(label_ok) {
        HostKind::NotAName
    } else {
        HostKind::Other
    }
}

/// Why `host` can't be a connection's computer, if it can't: it must be a computer's name, as
/// DNS writes it, or an address of another computer.
fn host_problem(host: &str) -> Option<String> {
    match host_kind(host) {
        HostKind::Other => None,
        HostKind::ThisComputer => Some(format!(
            "`{host}` is this computer; a port alone is this computer's: leave the computer out, \
             as in `- Network: connects to TCP port 8080`"
        )),
        HostKind::NotAName => Some(format!(
            "`{host}` isn't a computer's name or address: write a name as DNS does, such as \
             `example.com`, or an address such as `192.0.2.7`"
        )),
    }
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
    /// The exit code the behavior states; 0 for one that keeps serving.
    pub exit_code: u8,
    /// Whether the behavior ends with the program still serving, instead of an exit code.
    pub serves: bool,
    /// Its generated tests, under its `###` heading.
    pub outline: Option<Outline>,
}

/// A behavior's generated tests: the steps under its `###` heading, in its example's phrasings
/// with `{placeholders}`, and the table of where their variables come from, one row a generated
/// test, as a Gherkin Scenario Outline has its Examples table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    /// Line of the `###` heading.
    pub line: usize,
    /// The steps.
    pub steps: Vec<Step>,
    /// The table's column names: the variables, and `runs` where rows give their own count.
    pub columns: Vec<String>,
    /// Line of the table's header row.
    pub columns_line: usize,
    /// The rows, one generated test each.
    pub rows: Vec<Row>,
    /// The section as written, from its heading.
    pub text: String,
}

/// A row of an outline's table: each column's cell, without its backticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Line of the row.
    pub line: usize,
    /// The cells, one a column.
    pub cells: Vec<String>,
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

/// A word of a step, or a value it gives in backticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A word outside backticks, in lowercase and without the punctuation around it.
    Word(String),
    /// A value written in backticks, as written.
    Value(String),
}

impl Step {
    /// The step's words and values, in order.
    #[must_use]
    pub fn tokens(&self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut rest = self.text.as_str();
        let words = |text: &str, tokens: &mut Vec<Token>| {
            for word in text.split_whitespace() {
                let word = word
                    .trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '(' | ')' | '!'))
                    .to_lowercase();
                if !word.is_empty() {
                    tokens.push(Token::Word(word));
                }
            }
        };
        while let Some(start) = rest.find('`') {
            words(&rest[..start], &mut tokens);
            let after = &rest[start..];
            let run = after.bytes().take_while(|&byte| byte == b'`').count();
            let body = &after[run..];
            let Some(end) = find_exact_run(body, run) else {
                rest = "";
                break;
            };
            let value = &body[..end];
            // A value in double backticks may start or end with one; markdown trims a space.
            let value = if run > 1 {
                value.strip_prefix(' ').unwrap_or(value)
            } else {
                value
            };
            let value = if run > 1 {
                value.strip_suffix(' ').unwrap_or(value)
            } else {
                value
            };
            tokens.push(Token::Value(value.to_owned()));
            rest = &body[end + run..];
        }
        words(rest, &mut tokens);
        tokens
    }
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
    /// The keyword as a spec writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Given => "Given",
            Self::When => "When",
            Self::Then => "Then",
            Self::And => "And",
            Self::But => "But",
        }
    }

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

/// Something a program may use beyond its output, as a spec names it: the same on every target,
/// which maps it to its own system calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Use {
    /// Standard input.
    StandardInput,
    /// Files and folders in its working folder: open, read, write, seek and close files, read
    /// their sizes, list folders, make and remove folders, and delete and rename files.
    Files,
    /// The clock.
    Clock,
    /// Random bytes from the kernel.
    RandomBytes,
    /// Environment variables: only those a test, or the user through `run`, gives it.
    Environment,
    /// The terminal it runs in: keys as they're pressed, with no echo or line editing, a wait
    /// for the next key of a set time, and the window's size.
    Terminal,
}

impl Use {
    /// Every capability, in the order specs list them.
    pub const ALL: [Self; 6] = [
        Self::StandardInput,
        Self::Files,
        Self::Clock,
        Self::RandomBytes,
        Self::Environment,
        Self::Terminal,
    ];

    /// The words a spec names it with.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::StandardInput => "standard input",
            Self::Files => "files",
            Self::Clock => "the clock",
            Self::RandomBytes => "random bytes",
            Self::Environment => "the environment",
            Self::Terminal => "the terminal",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().trim_start_matches("the ").trim();
        match name {
            "standard input" | "stdin" => Some(Self::StandardInput),
            "files" | "file" => Some(Self::Files),
            "clock" => Some(Self::Clock),
            "random bytes" | "random" => Some(Self::RandomBytes),
            "environment" | "environment variables" => Some(Self::Environment),
            "terminal" => Some(Self::Terminal),
            _ => None,
        }
    }

    /// The Linux system calls it allows.
    #[must_use]
    pub fn syscalls(self) -> &'static [Syscall] {
        match self {
            Self::StandardInput => &[Syscall::Read],
            Self::Files => &[
                Syscall::Openat,
                Syscall::Read,
                Syscall::Write,
                Syscall::Close,
                Syscall::Lseek,
                Syscall::Fstat,
                Syscall::Getdents64,
                Syscall::Mkdirat,
                Syscall::Unlinkat,
                Syscall::Renameat,
            ],
            Self::Clock => &[Syscall::ClockGettime, Syscall::ClockNanosleep],
            Self::RandomBytes => &[Syscall::Getrandom],
            // The kernel lays them out above the arguments: no call reads them.
            Self::Environment => &[],
            Self::Terminal => &[Syscall::Read, Syscall::Ioctl, Syscall::Poll],
        }
    }

    /// The capability a system call belongs to, for saying what a spec must name to allow it;
    /// `None` for output, exit and the network's calls.
    #[must_use]
    pub fn of(syscall: Syscall) -> Option<Self> {
        match syscall {
            Syscall::Read => Some(Self::StandardInput),
            Syscall::Openat
            | Syscall::Close
            | Syscall::Lseek
            | Syscall::Fstat
            | Syscall::Getdents64
            | Syscall::Mkdirat
            | Syscall::Unlinkat
            | Syscall::Renameat => Some(Self::Files),
            Syscall::ClockGettime | Syscall::ClockNanosleep => Some(Self::Clock),
            Syscall::Getrandom => Some(Self::RandomBytes),
            Syscall::Ioctl => Some(Self::Terminal),
            _ => None,
        }
    }
}

/// The system calls every program has: its output and its exit.
const ALWAYS: [Syscall; 3] = [Syscall::Write, Syscall::Exit, Syscall::ExitGroup];

/// The most threads a spec may allow: each needs its own stack.
pub const MAX_THREADS: u32 = 64;

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
    /// Read an open file's size and kind.
    Fstat,
    /// Read the entries of an open folder.
    Getdents64,
    /// Make a folder.
    Mkdirat,
    /// Delete a file, or an empty folder.
    Unlinkat,
    /// Rename or move a file or folder.
    Renameat,
    /// End the program.
    Exit,
    /// End the program (all threads; lengua programs have one).
    ExitGroup,
    /// Read a clock: the time of day, or a clock that only moves forward.
    ClockGettime,
    /// Sleep for a time, or until a time, on either clock.
    ClockNanosleep,
    /// Fill a buffer with random bytes from the kernel.
    Getrandom,
    /// Take the next connection on file descriptor 3, for a program that serves.
    Accept4,
    /// End one direction of a connection.
    Shutdown,
    /// Send on a connection, with no address, possibly with `MSG_NOSIGNAL`.
    Sendto,
    /// Wait until the listening socket or one of the connections has something to read, for a
    /// server of several clients at once, or for the next key, a set time at most.
    Poll,
    /// Read or set the terminal's mode, or read its window's size: on standard input, output or
    /// error, and nothing else.
    Ioctl,
}

impl Syscall {
    /// Every system call a spec may allow.
    pub const ALL: [Self; 20] = [
        Self::Read,
        Self::Write,
        Self::Openat,
        Self::Close,
        Self::Lseek,
        Self::Fstat,
        Self::Getdents64,
        Self::Mkdirat,
        Self::Unlinkat,
        Self::Renameat,
        Self::Exit,
        Self::ExitGroup,
        Self::ClockGettime,
        Self::ClockNanosleep,
        Self::Getrandom,
        Self::Accept4,
        Self::Shutdown,
        Self::Sendto,
        Self::Poll,
        Self::Ioctl,
    ];

    /// The calls only a program on the network may make.
    pub const NETWORK: [Self; 4] = [Self::Accept4, Self::Shutdown, Self::Sendto, Self::Poll];

    /// The Linux name, as written in specs.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Openat => "openat",
            Self::Close => "close",
            Self::Lseek => "lseek",
            Self::Fstat => "fstat",
            Self::Getdents64 => "getdents64",
            Self::Mkdirat => "mkdirat",
            Self::Unlinkat => "unlinkat",
            Self::Renameat => "renameat",
            Self::Exit => "exit",
            Self::ExitGroup => "exit_group",
            Self::ClockGettime => "clock_gettime",
            Self::ClockNanosleep => "clock_nanosleep",
            Self::Getrandom => "getrandom",
            Self::Accept4 => "accept4",
            Self::Shutdown => "shutdown",
            Self::Sendto => "sendto",
            Self::Poll => "poll",
            Self::Ioctl => "ioctl",
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
    "bind",
    "brk",
    "chmod",
    "chown",
    "clone",
    "clone3",
    "creat",
    "dup",
    "dup2",
    "dup3",
    "execve",
    "execveat",
    "fcntl",
    "fork",
    "futex",
    "kill",
    "listen",
    "madvise",
    "mkdir",
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
    "rmdir",
    "rt_sigaction",
    "rt_sigprocmask",
    "sendmsg",
    "sigaltstack",
    "tgkill",
    "tkill",
    "unlink",
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
    // A spec travels to every computer: its words name none, unless one is its subject.
    let subject = header
        .as_ref()
        .map(|(title, _)| title.to_lowercase())
        .unwrap_or_default();
    if let Some((index, line)) = lines[1..end]
        .iter()
        .enumerate()
        .find(|(_, line)| line.trim_start().starts_with("description:"))
    {
        parser.portable(index + 2, line, &subject);
    }
    // An open code block: the line it starts on, its language, and its lines so far.
    let mut fence: Option<(usize, String, Vec<&str>)> = None;
    for (index, line) in lines.iter().enumerate().skip(end + 1) {
        let number = index + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            match fence.take() {
                Some((start, language, body)) => parser.block(start, &language, &body),
                None => {
                    let language = trimmed.trim_start_matches(['`', '~']).trim();
                    let language = language.split_whitespace().next().unwrap_or("");
                    fence = Some((number, language.to_ascii_lowercase(), Vec::new()));
                }
            }
            continue;
        }
        match fence.as_mut() {
            Some((_, _, body)) => body.push(line),
            None => {
                parser.portable(number, line, &subject);
                parser.line(number, line);
            }
        }
    }
    parser.finish_behavior();
    if let Some((line, ..)) = fence {
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
    outline: Option<Outline>,
    /// Where the outline's table is: not begun, its header read, or its rows.
    table: Table,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Table {
    #[default]
    Before,
    Header,
    Rows,
}

#[derive(Debug)]
struct Parser {
    problems: Vec<Problem>,
    hidden_problems: usize,
    section: Section,
    sections: Vec<(Section, usize)>,
    current: Option<Draft>,
    started: Vec<(BehaviorId, usize)>,
    /// Each behavior's title in lowercase, so no two behaviors name the same rule.
    titles: Vec<(String, BehaviorId)>,
    behaviors: Vec<Behavior>,
    uses: Option<Vec<Use>>,
    threads: Option<u32>,
    network: Network,
    parts: Vec<Part>,
    /// Whether `# Purpose` holds any text.
    purpose_written: bool,
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
            titles: Vec::new(),
            behaviors: Vec::new(),
            uses: None,
            threads: None,
            network: Network::default(),
            parts: Vec::new(),
            purpose_written: false,
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

    /// A problem for each word on the line, outside its values, that names one kind of computer
    /// rather than what the program does, unless the title names it too.
    fn portable(&mut self, number: usize, line: &str, subject: &str) {
        for word in machine_words(line) {
            if subject.contains(&word.to_lowercase()) {
                continue;
            }
            self.problem(
                Some(number),
                format!(
                    "`{word}` names one kind of computer, not what the program does: a spec is \
                     the same on every computer, so say what the program does and leave how to \
                     the compile skill. Name it only when it's the program's subject, in its title"
                ),
            );
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

    /// A code block: under a behavior, a ```toml block is its tests for every input; anywhere
    /// else a block is the spec's own text.
    /// A fenced block is the spec's own prose, unless it holds tests in TOML, which a spec
    /// writes as steps instead.
    fn block(&mut self, start: usize, language: &str, body: &[&str]) {
        let holds_tests = body.iter().any(|line| {
            let line = line.trim_start();
            line.starts_with("[[generated]]") || line.starts_with("[[example]]")
        });
        if language == "toml" && holds_tests {
            self.problem(
                Some(start),
                "tests aren't written in TOML: a behavior's steps are its example, and its \
                 generated tests go under a `###` heading after them, as the same steps with \
                 `{placeholders}` and a table of their variables, one row per variant; the \
                 write-spec skill's references/test-format.md shows them",
            );
        }
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
                } else if heading(line, 3).is_some() {
                    self.start_outline(number, line);
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
        self.purpose_written |= !line.trim().is_empty();
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
                outline: None,
                table: Table::Before,
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
        let key = title.to_lowercase();
        if let Some((_, other)) = self
            .titles
            .iter()
            .find(|(seen, _)| !key.is_empty() && *seen == key)
        {
            let other = *other;
            self.problem(
                Some(number),
                format!(
                    "{id} has the same title as {other}; each behavior's title names its own \
                     rule, such as `Adds two numbers` or `Rejects a missing argument`"
                ),
            );
        }
        self.titles.push((key, id));
        self.started.push((id, number));
        self.current = Some(Draft {
            id: Some(id),
            title,
            line: number,
            steps: Vec::new(),
            outline: None,
            table: Table::Before,
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
        let is_table = line.trim_start().starts_with('|');
        let problems = match draft.outline.as_mut() {
            Some(outline) => {
                outline.text.push_str(line);
                outline.text.push('\n');
                if is_table {
                    let problem = table_line(outline, draft.table, number, line)
                        .map(|problem| format!("{label}'s table: {problem}"));
                    draft.table = match draft.table {
                        Table::Before => Table::Header,
                        Table::Header | Table::Rows => Table::Rows,
                    };
                    problem.into_iter().collect()
                } else if item.is_some() && draft.table != Table::Before {
                    vec![format!(
                        "{label}'s generated tests: their steps go before their table of \
                         variables"
                    )]
                } else {
                    step_line(&label, line, item, number, &mut outline.steps)
                }
            }
            None if is_table => vec![format!(
                "{label}: a table of variables goes under a `###` heading after the behavior's \
                 steps, below the generated tests' own steps"
            )],
            None => step_line(&label, line, item, number, &mut draft.steps),
        };
        for problem in problems {
            self.problem(Some(number), problem);
        }
    }

    /// A `###` heading under a behavior: its generated tests start.
    fn start_outline(&mut self, number: usize, line: &str) {
        let Some(draft) = self.current.as_mut() else {
            self.problem(
                Some(number),
                "a `###` section of generated tests goes under the behavior it tests, after its \
                 steps",
            );
            return;
        };
        if draft.outline.is_some() {
            let label = draft
                .id
                .map_or_else(|| "this behavior".to_owned(), |id| id.to_string());
            self.problem(
                Some(number),
                format!(
                    "{label} has one `###` section of generated tests, since they share their \
                     steps: give each variant a row of its table instead"
                ),
            );
            return;
        }
        draft.outline = Some(Outline {
            line: number,
            steps: Vec::new(),
            columns: Vec::new(),
            columns_line: number,
            rows: Vec::new(),
            text: format!("{line}\n"),
        });
        draft.table = Table::Before;
    }

    fn finish_behavior(&mut self) {
        let Some(draft) = self.current.take() else {
            return;
        };
        let Some(id) = draft.id else {
            return;
        };
        if let Some(outline) = &draft.outline {
            self.check_outline(id, outline);
        }
        if let Some(ending) = self.check_steps(id, draft.line, &draft.steps) {
            self.behaviors.push(Behavior {
                id,
                title: draft.title,
                line: draft.line,
                steps: draft.steps,
                exit_code: ending.unwrap_or(0),
                serves: ending.is_none(),
                outline: draft.outline,
            });
        }
    }

    /// A behavior's generated tests need steps in its example's shape, and a table with rows.
    fn check_outline(&mut self, id: BehaviorId, outline: &Outline) {
        let missing: Vec<&str> = [
            (Keyword::Given, "Given"),
            (Keyword::When, "When"),
            (Keyword::Then, "Then"),
        ]
        .into_iter()
        .filter(|(keyword, _)| !outline.steps.iter().any(|step| step.keyword == *keyword))
        .map(|(_, name)| name)
        .collect();
        if outline.steps.is_empty() {
            self.problem(
                Some(outline.line),
                format!(
                    "{id}'s generated tests have no steps: write {id}'s steps under the heading, \
                     with `{{placeholders}}` where their table's variables go"
                ),
            );
        } else if !missing.is_empty() {
            self.problem(
                Some(outline.line),
                format!(
                    "{id}'s generated tests need Given, When and Then steps, as {id} has; they \
                     have no {}",
                    missing.join(" and no ")
                ),
            );
        }
        if let Some(second) = outline
            .steps
            .iter()
            .filter(|step| step.keyword == Keyword::When)
            .nth(1)
        {
            self.problem(
                Some(second.line),
                format!("{id}'s generated tests have one When, as {id} does"),
            );
        }
        if outline.columns.is_empty() {
            self.problem(
                Some(outline.line),
                format!(
                    "{id}'s generated tests need a table of their variables below their steps: a \
                     header naming each variable, such as `| a | b |`, then `|---|---|`, then a \
                     row of generators for each variant, such as ``| `int(1, 9)` | `int(1, 9)` |``"
                ),
            );
        } else if outline.rows.is_empty() {
            self.problem(
                Some(outline.columns_line),
                format!(
                    "{id}'s table has no rows: give each variant a row of generators, such as \
                     ``| `int(1, 9)` |``"
                ),
            );
        }
    }

    /// Finds the stated exit code, in a step after the When, or `Some(None)` for a behavior that
    /// ends with the program still serving. The order of Given, When and Then is the write-spec
    /// skill's to keep; lengua only needs one When, since an example is one run.
    fn check_steps(&mut self, id: BehaviorId, line: usize, steps: &[Step]) -> Option<Option<u8>> {
        if steps.is_empty() {
            self.problem(
                Some(line),
                format!("{id} has no steps; write them as `- Given …`, `- When …`, `- Then …`"),
            );
            return None;
        }
        let mut phase: Option<Keyword> = None;
        let mut exit_steps = Vec::new();
        let mut serving = Vec::new();
        for step in steps {
            match step.keyword {
                Keyword::Given => phase = Some(Keyword::Given),
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
                Keyword::Then => phase = Some(Keyword::Then),
                Keyword::And | Keyword::But => {}
            }
            if phase == Some(Keyword::Then) && states_exit_code(&step.text) {
                exit_steps.push(step);
            }
            if phase == Some(Keyword::Then) && keeps_serving(&step.text) {
                serving.push(step);
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
        match (exit_steps.as_slice(), serving.as_slice()) {
            ([], [_]) => Some(None),
            ([], []) => {
                self.problem(
                    Some(line),
                    format!(
                        "{id} doesn't say how the program exits; add a step such as \
                         `- And it exits with code `0``, or for a server that keeps running, \
                         `- And it keeps serving`"
                    ),
                );
                None
            }
            ([], [_, second, ..]) => {
                self.problem(
                    Some(second.line),
                    format!("{id} says twice that it keeps serving; keep one"),
                );
                None
            }
            ([_, ..], [serving, ..]) => {
                self.problem(
                    Some(serving.line),
                    format!(
                        "{id} both exits and keeps serving; a behavior ends one way, so keep one"
                    ),
                );
                None
            }
            ([step], []) => self.exit_code(id, step).map(Some),
            ([first, second, ..], []) => {
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

    /// `- Builds on: [sort lines](../sort/spec.md), [numbers](../numbers/spec.md)`: the parts,
    /// each a link to its spec by a path relative to this one, so the graph travels with the
    /// folders.
    fn parts_line(&mut self, number: usize, rest: &str) {
        let mut rest = rest;
        let mut found = 0;
        while let Some(open) = rest.find('[') {
            let Some(close) = rest[open..].find("](").map(|at| open + at) else {
                break;
            };
            let Some(end) = rest[close + 2..].find(')').map(|at| close + 2 + at) else {
                break;
            };
            let name = rest[open + 1..close].trim().to_owned();
            let target = rest[close + 2..end].trim().to_owned();
            rest = &rest[end + 1..];
            found += 1;
            let portable = !target.starts_with('/')
                && !target.contains("://")
                && !target.contains('\\')
                && target.ends_with("spec.md");
            if !portable {
                self.problem(
                    Some(number),
                    format!(
                        "the part [{name}]({target}) links to its spec by a path relative to this \
                         one, ending in spec.md, so the graph travels with the folders: such as \
                         `[{name}](../{}/spec.md)`",
                        name.to_lowercase().replace(' ', "-")
                    ),
                );
                continue;
            }
            if self.parts.iter().any(|part| part.spec == target) {
                self.problem(Some(number), format!("the part {target} is listed twice"));
                continue;
            }
            self.parts.push(Part { name, spec: target });
        }
        if found == 0 {
            self.problem(
                Some(number),
                "`- Builds on:` links to each part's spec: `- Builds on: [sort lines](../sort/spec.md)`",
            );
        }
    }

    fn constraint_line(&mut self, number: usize, line: &str) {
        let Some(item) = list_item(line) else {
            return;
        };
        let item = item.trim();
        if let Some(rest) = strip_prefix_ignore_case(item, "threads:") {
            self.threads_line(number, rest);
            return;
        }
        if let Some(rest) = strip_prefix_ignore_case(item, "network:") {
            self.network_line(number, rest);
            return;
        }
        if let Some(rest) = strip_prefix_ignore_case(item, "allowed system calls:") {
            self.problem(Some(number), allowed_calls_rewrite(rest));
            return;
        }
        if let Some(rest) = strip_prefix_ignore_case(item, "builds on:") {
            self.parts_line(number, rest);
            return;
        }
        let Some(rest) = strip_prefix_ignore_case(item, "uses:") else {
            self.problem(
                Some(number),
                "`# Constraints` lists only what the program uses, which means the same on every \
                 computer: `- Uses: …`, `- Network: …`, `- Threads: …` and `- Builds on: …`; say \
                 anything else in `# Purpose`",
            );
            return;
        };
        if self.uses.is_some() {
            self.problem(
                Some(number),
                "what the program uses is listed twice; keep one `- Uses:` line",
            );
            return;
        }
        let known = Use::ALL.map(Use::name).join(", ");
        let mut uses = Vec::new();
        for name in rest.split(',').flat_map(|part| part.split(" and ")) {
            let name = name.trim().trim_end_matches('.').trim_matches('`').trim();
            let lower = name.to_ascii_lowercase();
            if lower.is_empty() || lower == "nothing" || lower == "nothing else" {
                continue;
            }
            match Use::from_name(&lower) {
                Some(found) if !uses.contains(&found) => uses.push(found),
                Some(_) => {}
                None if lower.contains("network") || lower.contains("port") => self.problem(
                    Some(number),
                    "the network has its own line: `- Network: connects to TCP port 8080` or \
                     `- Network: listens on TCP port 8080`",
                ),
                None if lower.contains("thread") => self.problem(
                    Some(number),
                    "threads have their own line: `- Threads: up to N`",
                ),
                None => self.problem(
                    Some(number),
                    format!(
                        "`{name}` isn't something lengua lets a program use; it may use {known}, \
                         the network and threads, besides writing its output. lengua programs \
                         can't allocate memory, start processes, handle signals or change the \
                         file system"
                    ),
                ),
            }
        }
        uses.sort_unstable();
        self.uses = Some(uses);
    }

    /// `- Network: connects to TCP ports 80 and 8080`, `- Network: connects to TCP port 443 on
    /// example.com` or `- Network: listens on TCP port 8080`.
    fn network_line(&mut self, number: usize, rest: &str) {
        let usage = "write `- Network: connects to TCP port 8080` (or `ports 80 and 8080`) for a \
                     program that talks to a server on this computer, `- Network: connects to \
                     TCP port 443 on example.com` for one on another computer, or `- Network: \
                     listens on TCP port 8080` for one that serves";
        let rest = rest.trim().trim_end_matches('.').replace('`', "");
        let mut lower = rest.to_ascii_lowercase();
        // `, up to 64 clients at once`: a server of several clients at once.
        if let Some(start) = lower.find("up to ")
            && let Some(length) = lower[start..].find(" clients at once")
        {
            let count = lower[start + "up to ".len()..start + length]
                .trim()
                .to_owned();
            lower.replace_range(start..start + length + " clients at once".len(), "");
            lower = lower.trim().trim_end_matches(',').trim().to_owned();
            match count.parse::<u32>() {
                Ok(count) if (2..=MAX_AT_ONCE).contains(&count) && lower.starts_with("listens") => {
                    self.network.at_once = Some(count);
                }
                Ok(_) if !lower.starts_with("listens") => {
                    self.problem(
                        Some(number),
                        "clients at once are a server's: write `up to N clients at once` on its \
                         `- Network: listens on TCP port N` line",
                    );
                    return;
                }
                _ => {
                    self.problem(
                        Some(number),
                        format!(
                            "a server takes from 2 to {MAX_AT_ONCE} clients at once: write `up to \
                             64 clients at once`, say, or leave it out to serve one at a time"
                        ),
                    );
                    return;
                }
            }
        }
        let (listens, numbers) = if let Some(numbers) = lower
            .strip_prefix("connects to tcp ports")
            .or_else(|| lower.strip_prefix("connects to tcp port"))
        {
            (false, numbers.trim())
        } else if let Some(numbers) = lower.strip_prefix("listens on tcp port") {
            (true, numbers.trim())
        } else {
            self.problem(Some(number), usage);
            return;
        };
        // A server is reached from this computer, unless the line says any computer:
        // `listens on TCP port 8080 for any computer`. A client's connections go where the line
        // says, to this computer when it names none: `connects to TCP port 443 on example.com`,
        // or `on the computer its first argument names`.
        let mut over_tls = false;
        let mut argument = None;
        let (numbers, host) = if listens {
            match numbers
                .strip_suffix("for any computer")
                .or_else(|| numbers.strip_suffix("on any computer"))
            {
                Some(ports) => {
                    self.network.serves_anywhere = true;
                    (ports.trim(), None)
                }
                None => (
                    numbers
                        .strip_suffix("for this computer")
                        .or_else(|| numbers.strip_suffix("on this computer"))
                        .unwrap_or(numbers)
                        .trim(),
                    None,
                ),
            }
        } else {
            // `over TLS`: lengua does TLS on the connection, which needs the computer's name.
            let (numbers, tls) = match numbers.strip_suffix("over tls") {
                Some(rest) => (rest.trim_end().trim_end_matches(',').trim_end(), true),
                None => (numbers, false),
            };
            if tls && !numbers.contains(" on ") {
                self.problem(
                    Some(number),
                    "TLS checks a computer's certificate by its name: name the computer, as in \
                     `- Network: connects to TCP port 443 on example.com over TLS`",
                );
                return;
            }
            over_tls = tls;
            match numbers.split_once(" on ") {
                Some((_, "any computer" | "any computers")) => {
                    self.problem(
                        Some(number),
                        "a program connects only where its spec says, and lengua makes each \
                         connection for it: name the computer, as in `- Network: connects to TCP \
                         port 443 on example.com`, or leave it out for this computer",
                    );
                    return;
                }
                Some((_, "this computer")) if tls => {
                    self.problem(
                        Some(number),
                        "TLS checks a computer's certificate by its name: name the computer, as \
                         in `- Network: connects to TCP port 443 on example.com over TLS`",
                    );
                    return;
                }
                Some((ports, "this computer")) => (ports.trim(), None),
                Some((ports, place)) if place.starts_with("the computer its ") => {
                    let Some(index) = argument_named(place.trim()) else {
                        self.problem(
                            Some(number),
                            "name the argument by its place, from first to ninth: `- Network: \
                             connects to TCP port 443 on the computer its first argument names`",
                        );
                        return;
                    };
                    argument = Some(index);
                    (ports.trim(), None)
                }
                Some((ports, host)) => {
                    let host = host.trim().to_owned();
                    if let Some(problem) = host_problem(&host) {
                        self.problem(Some(number), problem);
                        return;
                    }
                    (ports.trim(), Some(host))
                }
                None => (numbers, None),
            }
        };
        let ports: Option<Vec<u16>> = numbers
            .split(',')
            .flat_map(|part| part.split(" and "))
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|port| port.parse::<u16>().ok().filter(|port| *port > 0))
            .collect();
        match (listens, ports.as_deref()) {
            (_, None | Some([])) => {
                self.problem(Some(number), format!("ports go from 1 to 65535; {usage}"))
            }
            (true, Some([port])) if self.network.listens.is_none() => {
                self.network.listens = Some(*port);
            }
            (true, Some([_])) => self.problem(
                Some(number),
                "the port it listens on is given twice; keep one line",
            ),
            (true, Some(_)) => self.problem(
                Some(number),
                "a program listens on one port; keep one in `- Network: listens on TCP port N`",
            ),
            (false, Some(ports)) => {
                for port in ports {
                    let endpoint = Endpoint {
                        host: host.clone(),
                        argument,
                        port: *port,
                        tls: over_tls,
                    };
                    if self.network.connections.contains(&endpoint) {
                        self.problem(
                            Some(number),
                            format!(
                                "{} is named twice; a program gets one connection to each place \
                                 its spec names, so keep it once",
                                endpoint.describe()
                            ),
                        );
                        return;
                    }
                    self.network.connections.push(endpoint);
                }
                if self.network.connections.len() > MAX_CONNECTIONS {
                    self.problem(
                        Some(number),
                        format!(
                            "a program gets at most {MAX_CONNECTIONS} connections; name the \
                             places it talks to"
                        ),
                    );
                }
            }
        }
    }

    /// `- Threads: up to 16`: how many threads the program may run at once.
    fn threads_line(&mut self, number: usize, rest: &str) {
        let rest = rest.trim().trim_end_matches('.');
        let count = strip_prefix_ignore_case(rest, "up to")
            .unwrap_or(rest)
            .trim()
            .trim_matches('`');
        let count = count
            .parse::<u32>()
            .ok()
            .filter(|count| (2..=MAX_THREADS).contains(count));
        match (count, self.threads) {
            (_, Some(_)) => self.problem(
                Some(number),
                "the threads are given twice; keep one `- Threads: up to N` line",
            ),
            (Some(count), None) => self.threads = Some(count),
            (None, None) => self.problem(
                Some(number),
                format!(
                    "write the threads as `- Threads: up to N`, with N from 2 to {MAX_THREADS}, \
                     such as `- Threads: up to 16`"
                ),
            ),
        }
    }

    fn finish(mut self, header: Option<(String, Option<String>)>) -> Result<Spec, Vec<Problem>> {
        if !self.purpose_written {
            self.problem(
                None,
                "spec.md needs a `# Purpose` section that says in plain words what the program \
                 does, for whom and why, and the ranges of its inputs",
            );
        }
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
        let uses = self.uses.take().unwrap_or_default();
        let syscalls = self.syscalls_for(&uses);
        let ops = crate::edge::ops_for(&uses, &self.network, self.threads);
        self.check_network();
        match header {
            Some((title, description)) if self.problems.is_empty() => Ok(Spec {
                title,
                description,
                behaviors: self.behaviors,
                uses,
                syscalls,
                ops,
                threads: self.threads,
                purpose_values: self.purpose_values,
                network: self.network,
                parts: self.parts,
            }),
            _ => Err(self.into_problems()),
        }
    }

    /// The system calls `uses` and the network allow on this target, with the output and exit
    /// every program has, each once in the order of [`Syscall::ALL`].
    fn syscalls_for(&self, uses: &[Use]) -> Vec<Syscall> {
        let mut allowed: Vec<Syscall> = ALWAYS.to_vec();
        for found in uses {
            allowed.extend(found.syscalls());
        }
        if self.network.listens.is_some() {
            allowed.push(Syscall::Accept4);
        }
        if self.network.at_once.is_some() {
            allowed.push(Syscall::Poll);
        }
        if self.network.is_used() {
            allowed.extend([
                Syscall::Read,
                Syscall::Close,
                Syscall::Shutdown,
                Syscall::Sendto,
            ]);
        }
        Syscall::ALL
            .into_iter()
            .filter(|syscall| allowed.contains(syscall))
            .collect()
    }

    /// A behavior keeps serving only in a server.
    fn check_network(&mut self) {
        let listens = self.network.listens.is_some();
        let serving: Vec<String> = self
            .behaviors
            .iter()
            .filter(|behavior| behavior.serves)
            .map(|behavior| behavior.id.to_string())
            .collect();
        if !serving.is_empty() && !listens {
            self.problem(
                None,
                format!(
                    "{} keeps serving, which only a server does: add `- Network: listens on TCP \
                     port N`, or end it with an exit code",
                    serving.join(", ")
                ),
            );
        }
    }
}

/// The `- Uses:` line that says what an older spec's `- Allowed system calls:` line allowed: a
/// spec names what a program uses, the same on every target, not one kernel's system calls.
fn allowed_calls_rewrite(listed: &str) -> String {
    let mut uses: Vec<Use> = Vec::new();
    let mut refused = Vec::new();
    let names = listed.split(',').flat_map(|part| part.split(" and "));
    let calls: Vec<Syscall> = names
        .filter_map(|name| {
            let name = name.trim().trim_end_matches('.').trim_matches('`').trim();
            let call = Syscall::from_name(name);
            if call.is_none() && FORBIDDEN_SYSCALLS.contains(&name) {
                refused.push(name.to_owned());
            }
            call
        })
        .collect();
    // `socket` and `connect` were a client's own calls; lengua makes its connections now, but
    // the line is a network program's all the same.
    let connects = listed
        .split(',')
        .flat_map(|part| part.split(" and "))
        .any(|name| {
            matches!(
                name.trim().trim_end_matches('.').trim_matches('`').trim(),
                "socket" | "connect"
            )
        });
    let networked = connects || calls.iter().any(|call| Syscall::NETWORK.contains(call));
    let opens = calls.contains(&Syscall::Openat);
    for call in calls {
        match Use::of(call) {
            // A network program reads and closes its connections: standard input and files are
            // named only when the program opens files or reads input of its own.
            Some(Use::StandardInput) if networked || opens => {}
            Some(Use::Files) if networked && call == Syscall::Close => {}
            Some(found) if !uses.contains(&found) => uses.push(found),
            _ => {}
        }
    }
    uses.sort_unstable();
    let line = if uses.is_empty() {
        "delete the line: writing output and exiting need nothing named".to_owned()
    } else {
        format!(
            "write `- Uses: {}` instead",
            uses.iter()
                .map(|found| found.name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let refused = if refused.is_empty() {
        String::new()
    } else {
        format!(
            " ({} can't be used: lengua programs can't allocate memory, start processes, handle \
             signals or change the file system)",
            refused.join(", ")
        )
    };
    format!(
        "spec.md names what the program uses, the same on every computer, not Linux system \
         calls: {line}{refused}; the network and threads keep their own lines"
    )
}

/// The heading text when `line` is a heading of exactly `level`.
/// A line of a behavior's steps: a list item starts a step, an indented line continues the one
/// before it, and any other line is the spec's own prose. Returns its problems.
fn step_line(
    label: &str,
    line: &str,
    item: Option<&str>,
    number: usize,
    steps: &mut Vec<Step>,
) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(item) = item {
        let (word, rest) = item
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((item.trim(), ""));
        let (values, closed) = backtick_values(rest);
        match Keyword::parse(word) {
            Some(keyword) => steps.push(Step {
                keyword,
                text: rest.trim().to_owned(),
                values,
                line: number,
            }),
            None => {
                problems.push(format!(
                    "{label}: each step starts with Given, When, Then, And or But"
                ));
            }
        }
        if !closed {
            problems.push(format!("{label}: a backtick isn't closed"));
        }
    } else if line.starts_with([' ', '\t']) && !line.trim().is_empty() {
        // An indented line continues the previous step.
        let (values, closed) = backtick_values(line);
        if let Some(step) = steps.last_mut() {
            step.text.push(' ');
            step.text.push_str(line.trim());
            step.values.extend(values);
        }
        if !closed {
            problems.push(format!("{label}: a backtick isn't closed"));
        }
    }
    problems
}

/// A line of an outline's table: the first names the columns, the second separates them from
/// the rows, and each line after is a row. Returns its problem.
fn table_line(outline: &mut Outline, table: Table, number: usize, line: &str) -> Option<String> {
    let cells = table_cells(line);
    match table {
        Table::Before => {
            outline.columns = cells;
            outline.columns_line = number;
            None
        }
        Table::Header => {
            let separates = !cells.is_empty()
                && cells.iter().all(|cell| {
                    let dashes = cell.trim_matches(':');
                    !dashes.is_empty() && dashes.bytes().all(|byte| byte == b'-')
                });
            (!separates).then(|| {
                "its second line separates the column names from the rows, such as `|---|---|`"
                    .to_owned()
            })
        }
        Table::Rows => {
            if cells.len() != outline.columns.len() {
                return Some(format!(
                    "this row has {} cells, but the table has {} columns ({})",
                    cells.len(),
                    outline.columns.len(),
                    outline.columns.join(", ")
                ));
            }
            outline.rows.push(Row {
                line: number,
                cells,
            });
            None
        }
    }
}

/// The cells of a markdown table's line, each trimmed and without its backticks, with `\|` read
/// as `|`.
fn table_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let line = line.trim();
    let mut characters = line.strip_prefix('|').unwrap_or(line).chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' if characters.peek() == Some(&'|') => {
                characters.next();
                cell.push('|');
            }
            '|' => cells.push(cell_text(&std::mem::take(&mut cell))),
            other => cell.push(other),
        }
    }
    if !cell.trim().is_empty() {
        cells.push(cell_text(&cell));
    }
    cells
}

/// A table cell's text: a code span's content, as markdown reads it, or the text as written.
fn cell_text(cell: &str) -> String {
    let cell = cell.trim();
    let ticks = cell.len() - cell.trim_start_matches('`').len();
    let fence = "`".repeat(ticks);
    if ticks > 0 && cell.len() >= 2 * ticks && cell.ends_with(&fence) {
        let inner = &cell[ticks..cell.len() - ticks];
        // Markdown drops one space from each end when both have one.
        let spaced = inner.len() >= 2 && inner.starts_with(' ') && inner.ends_with(' ');
        return if spaced {
            inner[1..inner.len() - 1].to_owned()
        } else {
            inner.to_owned()
        };
    }
    cell.to_owned()
}

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

/// Words that name one kind of computer: CPUs, their instruction sets and registers, operating
/// systems and their interfaces, and toolchains. Only words with no everyday meaning are here,
/// so `neon` and `clang` aren't.
const MACHINE_WORDS: &[&str] = &[
    "x86", "x86-64", "x86_64", "amd64", "i386", "i686", "aarch64", "arm64", "armv7", "armv8",
    "risc-v", "riscv", "riscv64", "avx", "avx2", "avx-512", "avx512", "sse", "sse2", "sse3",
    "ssse3", "sse4", "sse4.1", "sse4.2", "simd", "mmx", "syscall", "syscalls", "seccomp",
    "landlock", "linux", "macos", "freebsd", "openbsd", "netbsd", "posix", "glibc", "libc", "musl",
    "llvm", "llvm-mc", "ld.lld", "gcc", "nasm", "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp",
    "rsp", "eax", "ebx", "ecx", "edx", "esi", "edi",
];

/// Phrases of that kind longer than a word.
const MACHINE_PHRASES: &[&str] = &["system call", "file descriptor", "/proc/", "/dev/"];

/// The words in `line`, outside its backtick values, that name one kind of computer.
fn machine_words(line: &str) -> Vec<String> {
    // Only the prose: values are what the program reads or prints, which may be anything.
    let mut prose = String::new();
    let mut inside = false;
    for character in line.chars() {
        if character == '`' {
            inside = !inside;
            prose.push(' ');
        } else if !inside {
            prose.push(character);
        }
    }
    let lower = prose.to_lowercase();
    let mut found: Vec<String> = MACHINE_PHRASES
        .iter()
        .filter(|phrase| lower.contains(*phrase))
        .map(|phrase| phrase.trim_matches('/').to_owned())
        .collect();
    let numbered = |word: &str, prefix: &str| {
        word.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
    };
    for word in prose.split(|character: char| {
        !(character.is_alphanumeric() || matches!(character, '-' | '_' | '.'))
    }) {
        let word = word.trim_matches(|character: char| matches!(character, '.' | '-' | '_'));
        let lower = word.to_lowercase();
        let register = ["xmm", "ymm", "zmm"]
            .iter()
            .any(|prefix| numbered(&lower, prefix))
            || (numbered(&lower, "r")
                && lower[1..]
                    .parse::<u8>()
                    .is_ok_and(|n| (8..=15).contains(&n)));
        if (MACHINE_WORDS.contains(&lower.as_str()) || register)
            && !found.contains(&word.to_owned())
        {
            found.push(word.to_owned());
        }
    }
    found
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

/// A step's words outside backticks, in lowercase.
fn words(text: &str) -> String {
    let mut words = String::new();
    let mut inside = false;
    for character in text.chars() {
        if character == '`' {
            inside = !inside;
        } else if !inside {
            words.push(character.to_ascii_lowercase());
        }
    }
    words
}

/// Whether a Then step states the exit code, judged by its words outside backticks.
fn states_exit_code(text: &str) -> bool {
    let words = words(text);
    words.contains("exit") && (words.contains("code") || words.contains("status"))
}

/// Whether a Then step says the program is still serving at the end, as a server does.
fn keeps_serving(text: &str) -> bool {
    let words = words(text);
    [
        "keeps serving",
        "keeps listening",
        "still serving",
        "still listening",
    ]
    .iter()
    .any(|phrase| words.contains(phrase))
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADD: &str = r#"---
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

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b |
|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
"#;

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

    /// A spec links to the parts it builds on, each by a path relative to it, so the graph of
    /// programs travels with their folders.
    #[test]
    fn a_spec_builds_on_parts() {
        let built = ADD.replace(
            "- Uses: nothing",
            "- Uses: nothing\n- Builds on: [sort lines](../sort/spec.md), [numbers](parts/numbers/spec.md)",
        );
        let spec = parse(&built).expect("valid spec");
        assert_eq!(
            spec.parts,
            vec![
                Part {
                    name: "sort lines".into(),
                    spec: "../sort/spec.md".into(),
                },
                Part {
                    name: "numbers".into(),
                    spec: "parts/numbers/spec.md".into(),
                },
            ]
        );
        assert_eq!(spec.parts[0].folder(), "../sort");
        assert_eq!(folder_of_link("../sort/"), "../sort");
        assert_eq!(folder_of_link("../sort/main.s"), "../sort");
        for link in [
            "/home/me/sort/spec.md",
            "https://example.com/spec.md",
            "../sort",
        ] {
            let bad = ADD.replace(
                "- Uses: nothing",
                &format!("- Uses: nothing\n- Builds on: [sort]({link})"),
            );
            assert_problem(&bad, "links to its spec by a path relative to this one");
        }
        let bare = ADD.replace("- Uses: nothing", "- Uses: nothing\n- Builds on: sort");
        assert_problem(&bare, "`- Builds on:` links to each part's spec");
    }

    /// A spec names no kind of computer in its prose, unless one is its subject; values,
    /// which the program reads or prints, may be anything.
    #[test]
    fn a_spec_names_no_kind_of_computer() {
        let fast = ADD.replace(
            "Adds two whole numbers between",
            "Adds, with AVX-512 and one system call, two whole numbers between",
        );
        assert_problem(&fast, "line 7: `AVX-512` names one kind of computer");
        assert_problem(&fast, "`system call` names one kind of computer");
        let described = ADD.replace("prints the sum.", "prints the sum on Linux.");
        assert_problem(&described, "line 4: `Linux` names one kind of computer");
        let registers = ADD.replace(
            "Adds two whole numbers between",
            "Adds rax and xmm0, between",
        );
        assert_problem(&registers, "`rax`");
        assert_problem(&registers, "`xmm0`");
        // Its subject, values, and everyday words pass.
        let subject = ADD.replace("title: add", "title: x86 add").replace(
            "Adds two whole numbers between",
            "Adds two x86 registers' whole numbers between",
        );
        assert!(parse(&subject).is_ok(), "{:?}", parse(&subject));
        let values = ADD.replace("prints `5`", "prints `rax`");
        assert!(parse(&values).is_ok(), "{:?}", parse(&values));
        let everyday = ADD.replace(
            "Adds two whole numbers between",
            "Adds, under a neon sign with a clang, two whole numbers between",
        );
        assert!(parse(&everyday).is_ok(), "{:?}", parse(&everyday));
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
        let outline = first.outline.as_ref().expect("generated tests");
        assert_eq!(outline.line, 16);
        assert_eq!(outline.steps.len(), 4);
        assert_eq!(outline.steps[0].values, ["{a}", "{b}"]);
        assert_eq!(outline.columns, ["a", "b"]);
        assert_eq!(outline.rows.len(), 1);
        assert_eq!(outline.rows[0].line, 24);
        assert_eq!(
            outline.rows[0].cells,
            [
                "int(-1000000000, 1000000000)",
                "int(-1000000000, 1000000000)"
            ]
        );
        assert_eq!(spec.behaviors[1].exit_code, 1);
        assert_eq!(
            spec.behaviors[1].values().collect::<Vec<_>>(),
            ["usage: add <a> <b>", "1"]
        );
        assert!(spec.behaviors[1].outline.is_none());
        // Writing its output and exiting need nothing named.
        assert!(spec.uses.is_empty());
        assert_eq!(
            spec.syscalls,
            [Syscall::Write, Syscall::Exit, Syscall::ExitGroup]
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
    fn threads_are_allowed_in_plain_words() {
        assert_eq!(parse(ADD).expect("valid spec").threads, None);
        let with =
            |line: &str| ADD.replace("- Uses: nothing\n", &format!("- Uses: nothing\n{line}\n"));
        for line in [
            "- Threads: up to 16",
            "- Threads: 16",
            "- threads: up to `16`.",
        ] {
            assert_eq!(
                parse(&with(line)).expect("valid").threads,
                Some(16),
                "{line}"
            );
        }
        assert_problem(&with("- Threads: up to 1"), "with N from 2 to 64");
        assert_problem(&with("- Threads: up to 65"), "with N from 2 to 64");
        assert_problem(&with("- Threads: many"), "with N from 2 to 64");
        assert_problem(
            &with("- Threads: up to 4\n- Threads: up to 8"),
            "the threads are given twice",
        );
        assert_problem(
            &ADD.replace("- Uses: nothing", "- Uses: threads"),
            "threads have their own line",
        );
    }

    #[test]
    fn each_behavior_names_its_own_rule_and_purpose_says_why() {
        assert_problem(
            &ADD.replace("## B2: Missing arguments", "## B2: Adds two NUMBERS"),
            "B2 has the same title as B1",
        );
        let start = ADD.find("# Purpose").expect("purpose");
        let end = ADD.find("# Behaviors").expect("behaviors");
        let without_purpose = format!("{}{}", &ADD[..start], &ADD[end..]);
        assert_problem(&without_purpose, "needs a `# Purpose` section");
    }

    #[test]
    fn accepts_windows_line_endings() {
        let spec = parse(&ADD.replace('\n', "\r\n")).expect("valid spec");
        assert_eq!(spec.behaviors.len(), 2);
        let outline = spec.behaviors[0].outline.as_ref().expect("generated tests");
        assert_eq!(outline.rows.len(), 1);
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
    fn network_lines_name_the_ports() {
        let client = ADD.replace(
            "- Uses: nothing",
            "- Network: connects to TCP ports `80` and 8080.",
        );
        let spec = parse(&client).expect("valid spec");
        let here = |port| Endpoint {
            host: None,
            argument: None,
            port,
            tls: false,
        };
        assert_eq!(
            spec.network.connections,
            [here(80), here(8080)],
            "this computer, unless the spec says otherwise"
        );
        assert_eq!(spec.network.listens, None);
        // lengua makes the connections: the program never opens a socket or connects.
        assert!(!spec.syscalls.contains(&Syscall::Accept4));
        for call in [
            Syscall::Read,
            Syscall::Sendto,
            Syscall::Shutdown,
            Syscall::Close,
        ] {
            assert!(spec.syscalls.contains(&call), "{call:?}");
        }
        let elsewhere =
            parse(&client.replace("8080.", "8080 on Example.com.")).expect("valid spec");
        let there = |port| Endpoint {
            host: Some("example.com".into()),
            argument: None,
            port,
            tls: false,
        };
        assert_eq!(elsewhere.network.connections, [there(80), there(8080)]);
        // Over TLS, lengua checks the computer's certificate by its name and does the TLS.
        let secure =
            parse(&client.replace("ports `80` and 8080.", "port 443 on example.com over TLS."))
                .expect("valid spec");
        assert_eq!(
            secure.network.connections,
            [Endpoint {
                host: Some("example.com".into()),
                argument: None,
                port: 443,
                tls: true,
            }]
        );
        assert_eq!(
            secure.network.connections[0].describe(),
            "TCP port 443 on example.com over TLS"
        );
        // The computer one of its arguments names, as a curl-like tool's user types it: lengua
        // reads the argument when the program starts.
        let typed = parse(&client.replace(
            "ports `80` and 8080.",
            "port 443 on the computer its second argument names over TLS.",
        ))
        .expect("valid spec");
        assert_eq!(
            typed.network.connections,
            [Endpoint {
                host: None,
                argument: Some(2),
                port: 443,
                tls: true,
            }]
        );
        assert_eq!(
            typed.network.connections[0].describe(),
            "TCP port 443 on the computer its second argument names over TLS"
        );
        for unknown in [
            "port 443 on the computer its tenth argument names.",
            "port 443 on the computer its argument names.",
        ] {
            assert_problem(
                &client.replace("ports `80` and 8080.", unknown),
                "name the argument by its place",
            );
        }
        for unnamed in ["port 443 over TLS.", "port 443 on this computer over TLS."] {
            assert_problem(
                &client.replace("ports `80` and 8080.", unnamed),
                "TLS checks a computer's certificate by its name",
            );
        }
        let address = parse(&client.replace("8080.", "8080 on 192.0.2.7.")).expect("valid spec");
        assert_eq!(
            address.network.connections[0].host.as_deref(),
            Some("192.0.2.7")
        );
        let here_too =
            parse(&client.replace("8080.", "8080 on this computer.")).expect("valid spec");
        assert_eq!(here_too.network.connections, spec.network.connections);
        // The program no longer picks where it connects: the spec names the computer.
        assert_problem(
            &client.replace("8080.", "8080 on any computer."),
            "name the computer",
        );
        for this in ["localhost", "127.0.0.1", "::1"] {
            assert_problem(
                &client.replace("8080.", &format!("8080 on {this}.")),
                "is this computer",
            );
        }
        assert_problem(
            &client.replace("8080.", "8080 on exa_mple.com."),
            "isn't a computer's name",
        );
        assert_problem(
            &client.replace("ports `80` and 8080", "ports 80 and 80"),
            "is named twice",
        );
        let many: Vec<String> = (1..=17).map(|port| port.to_string()).collect();
        assert_problem(
            &client.replace("ports `80` and 8080", &format!("ports {}", many.join(", "))),
            "at most 16 connections",
        );
        let server = ADD
            .replace("- Uses: nothing", "- Network: listens on TCP port 7007")
            .replace("- And it exits with code `0`", "- And it keeps serving");
        let spec = parse(&server).expect("valid spec");
        assert_eq!(spec.network.listens, Some(7007));
        assert!(
            !spec.network.serves_anywhere,
            "the loopback, unless the spec says otherwise"
        );
        // Clients at once are a server's, from 2 to a limit.
        let at_once = parse(&server.replace("port 7007", "port 7007, up to 64 clients at once"))
            .expect("valid spec");
        assert_eq!(at_once.network.at_once, Some(64));
        assert_eq!(at_once.network.listens, Some(7007));
        assert!(
            at_once.syscalls.contains(&Syscall::Poll) && !spec.syscalls.contains(&Syscall::Poll)
        );
        assert_problem(
            &server.replace("port 7007", "port 7007, up to 1 clients at once"),
            "a server takes from 2 to 4096 clients at once",
        );
        assert_problem(
            &client.replace("8080.", "8080, up to 4 clients at once."),
            "clients at once are a server's",
        );
        // A server other computers may reach says so; its own connections don't follow.
        for any in ["for any computer", "on any computer"] {
            let open = parse(&server.replace("port 7007", &format!("port 7007 {any}")))
                .expect("valid spec");
            assert_eq!(open.network.listens, Some(7007));
            assert!(
                open.network.serves_anywhere && open.network.connections.is_empty(),
                "{any}"
            );
        }
        assert!(spec.behaviors[0].serves);
        assert!(!spec.behaviors[1].serves);
        assert!(
            spec.syscalls.contains(&Syscall::Accept4) && spec.syscalls.contains(&Syscall::Sendto)
        );
        assert_problem(
            &ADD.replace("- And it exits with code `0`", "- And it keeps serving"),
            "B1 keeps serving, which only a server does",
        );
        assert_problem(
            &client.replace("ports `80` and 8080", "port 0"),
            "ports go from 1 to 65535",
        );
        // The network has its own line, with its ports.
        assert_problem(
            &ADD.replace("- Uses: nothing", "- Uses: the network"),
            "the network has its own line",
        );
    }

    #[test]
    fn ignores_other_frontmatter_fields() {
        // The record lives in lengua.lock; keeping it out of the frontmatter is the skill's job.
        assert!(parse(&ADD.replace("title: add\n", "title: add\nstatus: approved\n")).is_ok());
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
            "it has no When",
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
        // The order of the other steps is style, the write-spec skill's to keep.
        assert!(
            parse(&ADD.replace(
                "- When the program runs\n- Then it prints `5`",
                "- When the program runs\n- Given more\n- Then it prints `5`",
            ))
            .is_ok()
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
    fn names_what_the_program_uses() {
        let uses = |line: &str| ADD.replace("- Uses: nothing", line);
        let spec =
            parse(&uses("- Uses: standard input, files and the clock.")).expect("valid spec");
        assert_eq!(spec.uses, [Use::StandardInput, Use::Files, Use::Clock]);
        for call in [Syscall::Read, Syscall::Openat, Syscall::ClockGettime] {
            assert!(spec.syscalls.contains(&call), "{call:?}");
        }
        assert!(!spec.syscalls.contains(&Syscall::Getrandom));
        let spec = parse(&uses("- Uses: `random bytes`")).expect("valid spec");
        assert_eq!(spec.uses, [Use::RandomBytes]);
        // The terminal: its keys, its mode and its size, with a wait for the next key.
        let spec = parse(&uses("- Uses: the terminal")).expect("valid spec");
        assert_eq!(spec.uses, [Use::Terminal]);
        for call in [Syscall::Read, Syscall::Ioctl, Syscall::Poll] {
            assert!(spec.syscalls.contains(&call), "{call:?}");
        }
        // No Constraints at all is a program that needs nothing.
        let end = ADD.find("# Constraints").expect("constraints");
        assert!(parse(&ADD[..end]).expect("valid spec").uses.is_empty());
        assert_problem(
            &uses("- Uses: memory"),
            "`memory` isn't something lengua lets a program use",
        );
        assert_problem(
            &uses("- Uses: standard input\n- Uses: files"),
            "listed twice",
        );
    }

    #[test]
    fn an_older_specs_system_calls_come_with_their_uses_line() {
        let calls = |list: &str| {
            ADD.replace(
                "- Uses: nothing",
                &format!("- Allowed system calls: {list}"),
            )
        };
        for (list, needle) in [
            (
                "read, write, exit",
                "write `- Uses: standard input` instead",
            ),
            ("write, exit", "delete the line"),
            (
                "openat, read, write, close, exit",
                "write `- Uses: files` instead",
            ),
            (
                "write, getrandom, clock_gettime, exit",
                "write `- Uses: the clock, random bytes` instead",
            ),
            // A network program's reads and closes are its connections'.
            (
                "socket, connect, read, write, close, exit",
                "delete the line",
            ),
            ("read, write, mmap, exit", "mmap can't be used"),
        ] {
            assert_problem(&calls(list), needle);
        }
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
    fn reads_generated_tests_under_their_behaviors() {
        // Prose under the heading, and code blocks that hold no tests, are the spec's own.
        let prose = ADD.replace(
            "### For any `a` and `b`\n",
            "### For any `a` and `b`\nAny two numbers in range.\n\n```text\n| x |\n```\n",
        );
        let spec = parse(&prose).expect("valid spec");
        assert_eq!(
            spec.behaviors[0]
                .outline
                .as_ref()
                .map(|outline| outline.rows.len()),
            Some(1)
        );
        // A code span in a cell keeps its text whole, with `\|` for a pipe.
        assert_eq!(
            table_cells("| `a` | `` x`y `` | `a\\|b` | 50 |"),
            ["a", "x`y", "a|b", "50"]
        );
        let gives = |from: &str, to: &str, needle: &str| {
            assert!(ADD.contains(from), "{from:?}");
            assert_problem(&ADD.replacen(from, to, 1), needle);
        };
        gives(
            "# Constraints\n",
            "```toml\n[[generated]]\nvars = { a = \"int(1, 9)\" }\n```\n\n# Constraints\n",
            "tests aren't written in TOML",
        );
        gives(
            "- And it exits with code `1`\n",
            "- And it exits with code `1`\n\n| a |\n|---|\n| `int(1, 9)` |\n",
            "a table of variables goes under a `###` heading",
        );
        gives(
            "\n## B2:",
            "\n### For any more\n- Given no arguments\n\n## B2:",
            "has one `###` section of generated tests",
        );
        gives(
            "| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |\n",
            "| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |\n- And more\n",
            "their steps go before their table",
        );
        gives(
            "| a | b |\n|---|---|\n| `int(-1000000000, 1000000000)` | `int(-1000000000, \
             1000000000)` |\n",
            "",
            "need a table of their variables",
        );
        gives(
            "| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |",
            "| `int(-1000000000, 1000000000)` |",
            "this row has 1 cells, but the table has 2 columns (a, b)",
        );
        gives("|---|---|", "| x | y |", "its second line separates");
        gives(
            "# Behaviors\n",
            "# Behaviors\n### For any inputs\n",
            "goes under the behavior it tests",
        );
        gives(
            "### For any `a` and `b`\n- Given",
            "### For any `a` and `b`\n- Given more\n- When the program runs\n- Given",
            "have one When",
        );
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
            .replace("- Uses: nothing", "- Uses: memory");
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
