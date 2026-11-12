//! `idea.md`: a program's source of truth, written in Open Knowledge Format (OKF) markdown.
//!
//! The file starts with YAML frontmatter (`type`, `title` and `description`), followed by
//! markdown sections: `# Purpose`, `# Behaviors` with one `## B<n>: <title>` heading per behavior
//! written as Given/When/Then steps, each followed by its generated tests under a `###` heading,
//! and `# Constraints`, which names what the program uses, `- Uses:` and `- Network:` lines, and
//! the inputs whose values must not decide how it runs, `- Secret:`. Nothing in it is particular
//! to one computer. A `- Threads:` line still parses, so that check can say why it refuses one: a
//! program runs as one thread.
//!
//! An idea that lengua compiles to WebAssembly is an OKF Attested Computation (OKF v0.2 §10):
//! `type: Attested Computation`, with `runtime: webassembly`, `computation: main.wat`, the
//! `parameters` a run fills (each with lengua's key `from`, where its value comes from), lengua's
//! `run` as its `executor` and lengua's `check` as its `attester`. An idea of the native lengua,
//! `type: lengua/program-idea`, which it compiled to main.s, still parses and has none of those
//! keys, so that `check` can say lengua compiles to main.wat since 0.2.0 and that the tag
//! `native-final` keeps the native lengua. OKF's other keys, and any key lengua doesn't know, are
//! kept and never refused.

use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::reach::{Grants, Stand};
use crate::trust::Block;

/// The OKF `type` of an idea of the native lengua, which compiled it to main.s: still read, so
/// that `check` can say what became of it.
pub const IDEA_TYPE: &str = "lengua/program-idea";

/// The `type` lengua's ideas declared when the file was called `spec.md`: still read, so a renamed
/// file works as it was.
pub const LEGACY_IDEA_TYPE: &str = "lengua/program-spec";

/// The OKF `type` of an idea that lengua compiles to WebAssembly (OKF v0.2 §10), its name
/// exactly as OKF writes it.
pub const ATTESTED_TYPE: &str = "Attested Computation";

/// An Attested Computation's `runtime`: lengua runs the computation as WebAssembly.
pub const RUNTIME: &str = "webassembly";

/// The `resource` of an Attested Computation's `executor`: the page that says how lengua's `run`
/// runs it, a URL, as OKF §6.2 asks of `executor.resource`.
pub const EXECUTOR: &str = "https://github.com/laughing-hermit/lengua/blob/main/docs/run.md";

/// The `resource` of an Attested Computation's `attester`: the page that says how lengua's
/// `check` attests a run, a URL, as OKF §6.2 asks of `attester.resource`.
pub const ATTESTER: &str = "https://github.com/laughing-hermit/lengua/blob/main/docs/check.md";

/// The keys only an Attested Computation has, which an idea of the native lengua is refused for.
const ATTESTED_KEYS: [&str; 4] = ["runtime", "computation", "executor", "attester"];

/// A parameter as an Attested Computation writes it, for the problems to show.
const PARAMETER: &str = "{ name: a, type: integer, required: true, from: argument 1 }";

/// The exit code reserved for failed assertions.
pub const ASSERTION_EXIT_CODE: u8 = 134;

/// At most this many problems are reported at once.
const MAX_PROBLEMS: usize = 20;

/// A parsed, valid idea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idea {
    /// Short program name from the frontmatter.
    pub title: String,
    /// One-sentence summary from the frontmatter, which the bundle's index.md lists the program
    /// with.
    pub description: String,
    /// The behaviors, in file order.
    pub behaviors: Vec<Behavior>,
    /// What the program may use beyond its output, as the idea names it.
    pub uses: Vec<Use>,
    /// The system calls those words allow, worked out from `uses`, the network and the output
    /// every program has: the vocabulary a test's random bytes and clock readings are checked in.
    pub syscalls: Vec<Syscall>,
    /// How many threads the idea asks for, when it has a `- Threads:` line, which `check` refuses:
    /// a WebAssembly program runs as one thread.
    pub threads: Option<u32>,
    /// The backtick values in `# Purpose`, in order of first appearance: the ranges of the
    /// program's inputs, which generated tests draw from.
    pub purpose_values: Vec<String>,
    /// What it may do on the network.
    pub network: Network,
    /// The inputs whose values must not decide how the program runs, as its `- Secret:` line
    /// names them, in the line's order: lengua's checker refuses main.wat when a value computed
    /// from one decides a branch, an address, a length, a loop's count, a division or a float
    /// operation.
    pub secrets: Vec<Source>,
    /// The programs it builds on, each a folder with its own idea and code: the nodes it links to
    /// in the graph of programs.
    pub parts: Vec<Part>,
    /// The plain markdown links in its text, outside code and its `- Builds on:` line: with its
    /// parts, the graph's edges where they lead to other ideas.
    pub links: Vec<Link>,
    /// What its frontmatter says as an Attested Computation, the form of an idea that lengua
    /// compiles to WebAssembly; `None` for an idea of the native lengua, which `check` refuses.
    pub attested: Option<Attested>,
    /// What its `- Module:` line says, when it has one: its program is `main.wasm`, an outside
    /// module that a compiler made from another language, which lengua runs under the idea's
    /// grants and marks "trusted, not proven" ([`crate::wasm::outside`]).
    pub outside: Option<Outside>,
}

/// What an idea's `- Module:` line says of its outside module: ``- Module: main.wasm, built from
/// Rust with rustc for wasm32-wasip1``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outside {
    /// What the idea says the module was built from, in its own words: the language, the
    /// compiler, the source. lengua records the words and can't check them.
    pub built_from: String,
}

/// The most characters an idea's `- Module:` line gives to what its module was built from.
pub const MAX_BUILT_FROM: usize = 200;

/// How an idea's `- Module:` line reads, as its problems say it.
const MODULE_FORM: &str = "an outside module's line reads `- Module: main.wasm, built from …`, \
                           with the language and the compiler in up to 200 characters, as in \
                           `- Module: main.wasm, built from Rust with rustc for wasm32-wasip1`";

/// What an Attested Computation's frontmatter says beyond the native lengua's ideas (OKF v0.2
/// §10): how lengua runs the computation, `main.wat`, and what a run returns for its check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attested {
    /// `runtime`: `webassembly`, which says what the parameters mean.
    pub runtime: String,
    /// `computation`: `main.wat`, the code file beside idea.md.
    pub computation: String,
    /// `parameters`: the typed, named holes a run fills, in order.
    pub parameters: Vec<Parameter>,
    /// `executor.receipt`: what each run returns for the attester to check, in order.
    pub receipt: Vec<Receipt>,
}

/// One of an Attested Computation's parameters: `{ name: a, type: integer, required: true, from:
/// argument 1 }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its `name`: a lowercase letter, then lowercase letters, digits and `_`.
    pub name: String,
    /// Its `type`.
    pub kind: ParameterType,
    /// Whether each run must give it: its `required`, true when the idea leaves it out.
    pub required: bool,
    /// Where its value comes from: lengua's key `from`.
    pub from: Source,
    /// The line it starts on.
    pub line: usize,
}

/// The type of a parameter's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterType {
    /// A whole number.
    Integer,
    /// A number written in decimal, which may have a fraction.
    Decimal,
    /// Text.
    Text,
    /// Bytes of any value.
    Bytes,
}

impl ParameterType {
    /// Every type, in the order the problems list them.
    pub const ALL: [Self; 4] = [Self::Integer, Self::Decimal, Self::Text, Self::Bytes];

    /// The name an idea gives it: `integer`, `decimal`, `text` or `bytes`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Text => "text",
            Self::Bytes => "bytes",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// Where a value comes from: a parameter's, as its `from` says, or a secret's, as the
/// `- Secret:` line names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The program's argument at this place, from 1: `argument 1`.
    Argument(usize),
    /// Standard input: `standard input`.
    StandardInput,
    /// The environment variable of this name: `environment HOME`.
    Environment(String),
    /// The file of this name, by its path in the working folder: `file input.txt`.
    File(String),
}

impl Source {
    /// The words of `from` that name it: `argument 1`, `standard input`, `environment HOME` or
    /// `file input.txt`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Argument(place) => format!("argument {place}"),
            Self::StandardInput => "standard input".to_owned(),
            Self::Environment(name) => format!("environment {name}"),
            Self::File(name) => format!("file {name}"),
        }
    }

    /// What the idea's `- Uses:` line must name for the program to read it, when anything: an
    /// argument needs nothing.
    #[must_use]
    pub fn uses(&self) -> Option<Use> {
        match self {
            Self::Argument(_) => None,
            Self::StandardInput => Some(Use::StandardInput),
            Self::Environment(_) => Some(Use::Environment),
            Self::File(_) => Some(Use::Files),
        }
    }

    /// The words of a `- Secret:` line that name it: `standard input`, ``a file `key.txt` ``,
    /// ``the environment variable `TOKEN` `` or ``argument `2` ``.
    #[must_use]
    pub fn secret_words(&self) -> String {
        match self {
            Self::StandardInput => "standard input".to_owned(),
            Self::File(name) => format!("a file `{name}`"),
            Self::Environment(name) => format!("the environment variable `{name}`"),
            Self::Argument(place) => format!("argument `{place}`"),
        }
    }
}

/// A part of what lengua's `run` returns, which an executor's `receipt` lists for the attester.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receipt {
    /// What the program wrote to standard output.
    Stdout,
    /// What it wrote to standard error.
    Stderr,
    /// Its exit code.
    Exit,
    /// The files it left in its working folder.
    Files,
}

impl Receipt {
    /// Every part, in the order the problems list them.
    pub const ALL: [Self; 4] = [Self::Stdout, Self::Stderr, Self::Exit, Self::Files];

    /// The name a receipt gives it: `stdout`, `stderr`, `exit` or `files`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::Exit => "exit",
            Self::Files => "files",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|part| part.name() == name)
    }
}

/// A plain markdown link in an idea's text: `[sort lines](../sort/idea.md)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The line it's on.
    pub line: usize,
    /// What the link says.
    pub text: String,
    /// Where it leads, as written.
    pub target: String,
}

/// A program another builds on, as its idea links to it: `[sort lines](../sort/idea.md)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// What the link calls it.
    pub name: String,
    /// Its idea.md, relative to the folder of the idea that links to it.
    pub idea: String,
}

impl Part {
    /// Its folder, relative to the folder of the idea that links to it.
    #[must_use]
    pub fn folder(&self) -> String {
        folder_of_link(&self.idea)
    }
}

/// The folder a link to a part names, without a trailing `idea.md`, code file or slash: `../sort`
/// for `../sort/idea.md`.
#[must_use]
pub fn folder_of_link(link: &str) -> String {
    let trimmed = link.trim().trim_end_matches('/');
    let trimmed = [crate::state::IDEA_FILE, crate::state::CODE_FILE]
        .into_iter()
        .find_map(|file| trimmed.strip_suffix(file)?.strip_suffix('/'))
        .unwrap_or(trimmed);
    trimmed.trim_end_matches('/').to_owned()
}

/// The most clients a server may take at once: a table of this many connections is a few
/// pages of linear memory, and `poll_oneoff` goes through all of them on every wake.
pub const MAX_AT_ONCE: u32 = 4096;

/// What an idea lets a program do on the network: plain TCP to the places it names, and a port
/// it serves on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Network {
    /// The connections it gets, in order. lengua makes each before the program starts and
    /// hands it over as file descriptors 5, 6 and on, so the program never connects by itself
    /// and holds no network of its own, on every kind of computer.
    pub connections: Vec<Endpoint>,
    /// The port it serves on. lengua binds it and hands the program the listening socket as
    /// file descriptor 4, so the program never binds or listens itself.
    pub listens: Option<u16>,
    /// Whether other computers may connect to the port it serves on, which its idea says with
    /// `for any computer`: lengua then listens on every address of this computer, not only the
    /// loopback. Tests always use the loopback.
    pub serves_anywhere: bool,
    /// How many clients it serves at once, which its idea says with `up to N clients at once`:
    /// it then waits on all of them, and on its listening socket, with `poll`. One at a time
    /// when `None`.
    pub at_once: Option<u32>,
    /// What it may reach at run time, which it opens itself: TCP connections, the addresses of
    /// names, and UDP datagrams, each to the computers and ports its lines name
    /// ([`crate::reach`]).
    pub reach: Grants,
    /// The computers its tests stand in for, of those it reaches at run time, in the order its
    /// steps first name them: ``a server at `example.org` on port `80` …``. A test's server for
    /// one of them is numbered after the idea's own connections.
    pub stands: Vec<Stand>,
    /// The names its tests give addresses, each with its addresses as written: ``the addresses
    /// `192.0.2.7` for the name `example.org` ``. Every test of the idea sees them all.
    pub names: Vec<(String, Vec<String>)>,
    /// What its tests say each command it starts gives, in place of starting a program:
    /// ``the program `hostname` prints `box` and exits with code `0` ``. Every test of the
    /// idea sees them all, and no test starts anything.
    pub programs: Vec<crate::starts::Stood>,
}

impl Network {
    /// Whether the program uses the network at all.
    #[must_use]
    pub fn is_used(&self) -> bool {
        !self.connections.is_empty() || self.listens.is_some() || !self.reach.no_network()
    }
}

/// The values of `tokens` when they start as `shape` does: a word for each `Some`, and a value
/// in backticks for each `None`.
fn shaped<'t>(tokens: &'t [Token], shape: &[Option<&str>]) -> Option<Vec<&'t str>> {
    let mut values = Vec::new();
    for (index, wanted) in shape.iter().enumerate() {
        match (tokens.get(index)?, wanted) {
            (Token::Word(word), Some(wanted)) if word == wanted => {}
            (Token::Value(value), None) => values.push(value.as_str()),
            _ => return None,
        }
    }
    Some(values)
}

/// The computer a step's tokens name for a test to stand in for, of those a program reaches at
/// run time: ``a server at `example.org` on port `80` …`` for TCP, and ``a computer at
/// `192.0.2.53` on UDP port `53` …`` for datagrams.
fn stand_named(tokens: &[Token]) -> Option<Stand> {
    let tcp = [
        Some("a"),
        Some("server"),
        Some("at"),
        None,
        Some("on"),
        Some("port"),
        None,
    ];
    let udp = [
        Some("a"),
        Some("computer"),
        Some("at"),
        None,
        Some("on"),
        Some("udp"),
        Some("port"),
        None,
    ];
    let (datagrams, values) = match shaped(tokens, &tcp) {
        Some(values) => (false, values),
        None => (true, shaped(tokens, &udp)?),
    };
    let [host, port] = values.as_slice() else {
        return None;
    };
    Some(Stand {
        datagrams,
        host: crate::reach::spelled(host).ok()?.text,
        port: port.trim().parse::<u16>().ok().filter(|port| *port > 0)?,
    })
}

/// The name a step's tokens give addresses, with them: ``the addresses `192.0.2.7` and
/// `2001:db8::1` for the name `example.org` ``.
fn name_given(tokens: &[Token]) -> Option<(String, Vec<String>)> {
    let mut rest = tokens
        .strip_prefix(&[Token::Word("the".into()), Token::Word("addresses".into())])
        .or_else(|| {
            tokens.strip_prefix(&[Token::Word("the".into()), Token::Word("address".into())])
        })?;
    let mut addresses = Vec::new();
    loop {
        match rest {
            [Token::Value(address), after @ ..] => {
                addresses.push(address.parse::<std::net::IpAddr>().ok()?.to_string());
                rest = after;
            }
            [Token::Word(and), after @ ..] if and == "and" => rest = after,
            _ => break,
        }
    }
    let named = shaped(rest, &[Some("for"), Some("the"), Some("name"), None])?;
    let name = crate::reach::spelled(named.first()?).ok()?;
    (name.address.is_none() && !addresses.is_empty() && rest.len() == 4)
        .then_some((name.text, addresses))
}

/// What a `- Uses:` line naming `uses` and a `- Network:` line saying `network` allow, read as an
/// idea's `# Constraints` reads them, for a program with no idea of its own, such as a test of
/// the WebAssembly host. An empty `network` names no network.
///
/// # Errors
///
/// The problems either line has, as an idea would report them.
pub fn constraints(uses: &[&str], network: &str) -> Result<(Vec<Use>, Network), Vec<Problem>> {
    let mut parser = Parser::default();
    if !uses.is_empty() {
        parser.constraint_line(1, &format!("- Uses: {}", uses.join(", ")));
    }
    if !network.trim().is_empty() {
        parser.constraint_line(2, &format!("- Network: {network}"));
    }
    let uses = parser.uses.take().unwrap_or_default();
    if parser.problems.is_empty() {
        Ok((uses, parser.network))
    } else {
        Err(parser.into_problems())
    }
}

/// What the lines of `# Constraints` in `lines`, each written whole as an idea writes it
/// (`- Files: reads the file its first argument names`), allow, for a program with no idea of
/// its own, such as a test of the checker or of the host.
///
/// # Errors
///
/// The problems the lines have, as an idea would report them.
pub fn constraints_of(lines: &[&str]) -> Result<(Vec<Use>, Network), Vec<Problem>> {
    let mut parser = Parser::default();
    for (number, line) in lines.iter().enumerate() {
        parser.constraint_line(number + 1, line);
    }
    let uses = parser.uses.take().unwrap_or_default();
    if parser.problems.is_empty() {
        Ok((uses, parser.network))
    } else {
        Err(parser.into_problems())
    }
}

/// What a step's tokens say a command gives in a test, in place of starting it: ``the
/// program `hostname` prints `box` and exits with code `0` ``, with ``with the arguments `-s`
/// and `x` `` after the name for a command with arguments, and `prints nothing` for one that
/// prints nothing. The text takes the escapes a step's text takes.
pub(crate) fn program_stood(tokens: &[Token]) -> Option<crate::starts::Stood> {
    let word = |token: Option<&Token>, wanted: &str| matches!(token, Some(Token::Word(word)) if word == wanted);
    let mut rest =
        tokens.strip_prefix(&[Token::Word("the".into()), Token::Word("program".into())])?;
    let [Token::Value(program), after @ ..] = rest else {
        return None;
    };
    rest = after;
    let mut arguments = Vec::new();
    if word(rest.first(), "with") {
        // `with the arguments `a` and `b``, or `with the argument `a``.
        if !word(rest.get(1), "the")
            || !(word(rest.get(2), "arguments") || word(rest.get(2), "argument"))
        {
            return None;
        }
        rest = &rest[3..];
        loop {
            match rest {
                [Token::Value(argument), after @ ..] => {
                    arguments.push(argument.clone());
                    rest = after;
                }
                [Token::Word(and), after @ ..] if and == "and" => rest = after,
                _ => break,
            }
        }
        if arguments.is_empty() {
            return None;
        }
    }
    if !word(rest.first(), "prints") {
        return None;
    }
    let (output, rest) = match &rest[1..] {
        [Token::Word(nothing), after @ ..] if nothing == "nothing" => (String::new(), after),
        [Token::Value(text), after @ ..] => (crate::testfile::unescape(text), after),
        _ => return None,
    };
    let code = shaped(
        rest,
        &[Some("and"), Some("exits"), Some("with"), Some("code"), None],
    )?;
    let code = code.first()?.trim().parse::<u8>().ok()?;
    (rest.len() == 5).then(|| crate::starts::Stood {
        program: program.clone(),
        arguments,
        output,
        code,
    })
}

/// The most connections an idea may name.
pub const MAX_CONNECTIONS: usize = crate::connect::MAX_CONNECTIONS;

/// One connection an idea names: a TCP port of this computer, of another computer, or of the
/// computer one of the program's arguments names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint {
    /// The computer, by name or address, in lower case; `None` for this computer, or for the
    /// one an argument names.
    pub host: Option<String>,
    /// Which of the program's arguments, from 1, names the computer, when the idea says so:
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
pub(crate) const ORDINALS: [&str; 9] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];

/// Which argument, from 1, `place` says names the computer: `the computer its first argument
/// names`; `None` for anything else.
pub(crate) fn argument_named(place: &str) -> Option<usize> {
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
        return if crate::connect::this_computer(address) {
            HostKind::ThisComputer
        } else {
            HostKind::Other
        };
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return HostKind::ThisComputer;
    }
    // A name's last label is never a number: a system's older parsing reads `2130706433`,
    // `127.1`, `0x7f.1` and `0177.0.0.1` as addresses, the loopback's here, where DNS has no
    // such names.
    let number = |label: &str| {
        let hex = label.strip_prefix("0x");
        (!label.is_empty() && label.bytes().all(|byte| byte.is_ascii_digit()))
            || hex.is_some_and(|digits| digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
    };
    let last = host.strip_suffix('.').unwrap_or(host).rsplit('.').next();
    if last.is_some_and(number) {
        return HostKind::NotAName;
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

impl Idea {
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
    /// The keyword as an idea writes it.
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

/// Something a program may use beyond its output, as an idea names it: the same on every target,
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
    /// Every capability, in the order ideas list them.
    pub const ALL: [Self; 6] = [
        Self::StandardInput,
        Self::Files,
        Self::Clock,
        Self::RandomBytes,
        Self::Environment,
        Self::Terminal,
    ];

    /// The words an idea names it with.
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

    /// The capability a system call belongs to, for saying what an idea must name to allow it;
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

/// The most threads an idea may allow: each needs its own stack.
pub const MAX_THREADS: u32 = 64;

/// A system call an idea may allow. Programs can't use any other.
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
    /// Every system call an idea may allow.
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

    /// The Linux name, as written in ideas.
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

/// A problem found in idea.md.
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

/// Parses and validates idea.md, reporting every problem found (up to 20).
///
/// # Errors
///
/// Returns the problems when the idea isn't valid.
pub fn parse(text: &str) -> Result<Idea, Vec<Problem>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut parser = Parser::default();
    let Some(end) = frontmatter_end(&lines) else {
        parser.problem(Some(1), no_frontmatter());
        return Err(parser.into_problems());
    };
    let blocks = crate::trust::blocks(&lines, 1, end);
    let keys = Keys {
        lines: &lines,
        blocks: &blocks,
    };
    let header = parser.frontmatter(&keys, end);
    // An idea travels to every computer: its words name none, unless one is its subject.
    let subject = header
        .as_ref()
        .map(|(title, _)| title.to_lowercase())
        .unwrap_or_default();
    if let Some(block) = keys.block("description") {
        let number = block.start + 1;
        parser.portable(number, lines[block.start], &subject);
        // The description is prose, in STE as the rest of the idea is.
        if let Some((_, Some(description))) = &header {
            parser.prose.text(number, description);
        }
    }
    let unclosed = walk(&lines, end, |met| match met {
        // A code block ends the prose before it, and its own lines aren't prose.
        Met::Opens => parser.prose.end(),
        Met::Closes(start, language, body) => {
            parser.prose.end();
            parser.block(start, &language, &body);
        }
        Met::Line(number, line) => {
            parser.portable(number, line, &subject);
            parser.ste(number, line);
            parser.line(number, line);
            parser.links_line(number, line);
        }
    });
    parser.finish_behavior();
    if let Some(line) = unclosed {
        parser.problem(Some(line), UNCLOSED);
    }
    parser.finish(header)
}

/// What a code block that is never closed is told.
const UNCLOSED: &str = "this ``` code block is never closed";

/// What an idea with no frontmatter is told.
fn no_frontmatter() -> String {
    format!(
        "idea.md must start with YAML frontmatter: a line with ---, then `type: {ATTESTED_TYPE}`, \
         `title: <name>` and `description: <one sentence>`, then a line with ---"
    )
}

/// What the walk over an idea's lines after its frontmatter meets, one line at a time.
enum Met<'t> {
    /// A fence that opens a code block.
    Opens,
    /// A fence that closes the code block that opened on line `.0`, in the language `.1`, with
    /// `.2` its lines.
    Closes(usize, String, Vec<&'t str>),
    /// A line outside code blocks, by its number from 1.
    Line(usize, &'t str),
}

/// The walk [`parse`] and [`grants`] make over `lines` after the frontmatter that closes on line
/// `end` (from 0): `met` hears of each line, as a fence that opens or closes a code block or as
/// a line outside code blocks, so both readers take the very same lines. Gives the line of a code
/// block that is never closed.
fn walk<'t>(lines: &[&'t str], end: usize, mut met: impl FnMut(Met<'t>)) -> Option<usize> {
    // An open code block: the line it starts on, its language, and its lines so far.
    let mut fence: Option<(usize, String, Vec<&'t str>)> = None;
    for (index, &line) in lines.iter().enumerate().skip(end + 1) {
        let number = index + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            match fence.take() {
                Some((start, language, body)) => met(Met::Closes(start, language, body)),
                None => {
                    let language = trimmed.trim_start_matches(['`', '~']).trim();
                    let language = language.split_whitespace().next().unwrap_or("");
                    fence = Some((number, language.to_ascii_lowercase(), Vec::new()));
                    met(Met::Opens);
                }
            }
            continue;
        }
        match fence.as_mut() {
            Some((_, _, body)) => body.push(line),
            None => met(Met::Line(number, line)),
        }
    }
    fence.map(|(start, ..)| start)
}

/// What an idea grants a run: its `- Uses:` line's capabilities and its `- Network:` line's
/// network, read from the same lines [`parse`] reads, by the same code, and nothing else of the
/// idea: no YAML, no plain-English rules, no behaviors or tests. A run takes them from the idea
/// whose hash its lock records, which check has parsed in full. What an idea's tests stand in
/// for ([`Network::stands`], [`Network::names`]) is read from its behaviors, which a run doesn't
/// read: a run reaches the real computers its idea grants.
///
/// # Errors
///
/// Any problem of the frontmatter's end, of a code block that is never closed, or of a line of
/// `# Constraints`, as [`parse`] reports it.
pub fn grants(text: &str) -> Result<(Vec<Use>, Network), Vec<Problem>> {
    let mut parser = constraints_read(text)?;
    let uses = parser.uses.take().unwrap_or_default();
    Ok((uses, parser.network))
}

/// The inputs an idea's `- Secret:` line names, read as [`grants`] reads its lines, by the same
/// code; none for an idea without the line. A run takes them from the idea whose hash its lock
/// records, to check the machine code it is about to run.
///
/// # Errors
///
/// As [`grants`].
pub fn secrets(text: &str) -> Result<Vec<Source>, Vec<Problem>> {
    let mut parser = constraints_read(text)?;
    Ok(parser
        .secrets
        .take()
        .map(|(_, secrets)| secrets)
        .unwrap_or_default())
}

/// What an idea's `- Module:` line says, read as [`grants`] reads its lines, by the same code;
/// `None` for an idea without the line, whose program is main.wat. A run takes it from the idea
/// whose hash its lock records, to know which file is the program and to say that an outside
/// module is trusted, not proven, before it starts.
///
/// # Errors
///
/// As [`grants`].
pub fn outside(text: &str) -> Result<Option<Outside>, Vec<Problem>> {
    let mut parser = constraints_read(text)?;
    Ok(parser.outside.take().map(|(_, outside)| outside))
}

/// The lines of `# Constraints` of an idea's `text`, read as [`parse`] reads them, and nothing
/// else of the idea; or the problems [`grants`] reports.
fn constraints_read(text: &str) -> Result<Parser, Vec<Problem>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut parser = Parser::default();
    let Some(end) = frontmatter_end(&lines) else {
        parser.problem(Some(1), no_frontmatter());
        return Err(parser.into_problems());
    };
    let unclosed = walk(&lines, end, |met| {
        if let Met::Line(number, line) = met {
            parser.grants_line(number, line);
        }
    });
    if let Some(line) = unclosed {
        parser.problem(Some(line), UNCLOSED);
    }
    if parser.problems.is_empty() {
        Ok(parser)
    } else {
        Err(parser.into_problems())
    }
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

/// The frontmatter's keys that lengua reads. An Attested Computation's are read as any YAML, so
/// that a problem can say what each holds instead of only that the YAML doesn't fit.
#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    #[serde(rename = "type")]
    kind: Option<String>,
    title: Option<String>,
    description: Option<String>,
    runtime: Option<Value>,
    computation: Option<Value>,
    parameters: Option<Value>,
    executor: Option<Value>,
    attester: Option<Value>,
    status: Option<Value>,
}

/// The idea's lines and its frontmatter's top-level keys, which put each problem with the
/// frontmatter on its line.
struct Keys<'a> {
    /// The idea's lines.
    lines: &'a [&'a str],
    /// The frontmatter's top-level keys, each with its lines.
    blocks: &'a [Block],
}

impl Keys<'_> {
    /// The top-level key `key`, when the frontmatter has it.
    fn block(&self, key: &str) -> Option<&Block> {
        self.blocks.iter().find(|block| block.key == key)
    }

    /// The line the top-level key `key` is on.
    fn line(&self, key: &str) -> Option<usize> {
        self.block(key).map(|block| block.start + 1)
    }

    /// The line `key` starts on inside the value of the top-level key `parent`, or else
    /// `parent`'s own line.
    fn nested(&self, parent: &str, key: &str) -> Option<usize> {
        let block = self.block(parent)?;
        let at = crate::trust::key_line(self.lines, block.start + 1, block.end, key)
            .unwrap_or(block.start);
        Some(at + 1)
    }

    /// The lines of each entry of the list that the top-level key `key` holds, when it's written
    /// an entry a line or more: each one's first line, and one past its last.
    fn entries(&self, key: &str) -> Vec<(usize, usize)> {
        self.block(key)
            .map(|block| crate::trust::entries(self.lines, block.start + 1, block.end))
            .unwrap_or_default()
    }

    /// The line `key` starts on in the lines `span` of an entry, or else `line`, the entry's.
    fn within(&self, span: Option<(usize, usize)>, key: &str, line: usize) -> usize {
        span.and_then(|(start, end)| crate::trust::key_line(self.lines, start, end, key))
            .map_or(line, |at| at + 1)
    }
}

/// A YAML scalar's text: a string, a number or a boolean. `None` for no value, a list or a
/// mapping.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// A YAML value as a problem shows it: a scalar's text, or a list or a mapping as JSON writes it.
fn shown(value: &Value) -> String {
    scalar_text(value).unwrap_or_else(|| value.to_string())
}

/// Whether `name` can be the environment variable a parameter's value comes from: capitals,
/// digits and `_`.
fn variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Why `name` can't be the file a parameter's value comes from, when it can't: it must name a
/// file in the program's working folder, by a path there that every system takes.
fn file_problem(name: &str) -> Option<String> {
    if name.split('/').all(|part| part.is_empty() || part == ".") {
        return Some("it names no file".to_owned());
    }
    if name.starts_with('/') {
        return Some("a path from the root leaves the working folder".to_owned());
    }
    if name.split('/').any(|part| part == "..") {
        return Some("`..` leaves the working folder".to_owned());
    }
    crate::names::broken(name.as_bytes()).map(|broken| broken.why)
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
    links: Vec<Link>,
    /// Whether `# Purpose` holds any text.
    purpose_written: bool,
    purpose_values: Vec<String>,
    /// What STE's checks find in the idea's prose, reported after every other problem.
    prose: crate::ste::Prose,
    /// The frontmatter of an Attested Computation, as far as it reads.
    attested: Option<Attested>,
    /// Each parameter's source that reads, with the parameter as a problem names it and its line,
    /// whether or not the rest of the parameter reads.
    sources: Vec<(String, Source, usize)>,
    /// Each parameter's name that reads, with the line it's on, whether or not the rest of the
    /// parameter reads.
    names: Vec<(String, usize)>,
    /// The `- Secret:` line's line and the sources on it that read, once it's read.
    secrets: Option<(usize, Vec<Source>)>,
    /// Whether the idea is of the native lengua's type, which has no secrets to follow.
    native: bool,
    /// The `- Module:` line's line and what it says, once it's read.
    outside: Option<(usize, Outside)>,
    /// Whether an Attested Computation's frontmatter says `computation: main.wasm`, an outside
    /// module's, with the key's line; `None` for an idea whose frontmatter has no `computation`
    /// that reads, or no frontmatter read at all, as when only its grants are read.
    computation: Option<(Option<usize>, bool)>,
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
            links: Vec::new(),
            purpose_written: false,
            purpose_values: Vec::new(),
            prose: crate::ste::Prose::default(),
            attested: None,
            sources: Vec::new(),
            names: Vec::new(),
            secrets: None,
            native: false,
            outside: None,
            computation: None,
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
                    "`{word}` names one kind of computer, not what the program does: an idea is \
                     the same on every computer, so say what the program does and leave how to \
                     the compile skill. Name it only when it's the program's subject, in its title"
                ),
            );
        }
    }

    /// Gives STE's checks the line when it's prose they read: a paragraph of `# Purpose`, a
    /// behavior's title or a step. Any other line ends the paragraph or step before it.
    fn ste(&mut self, number: usize, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // A blank line ends a paragraph; a step goes on until a line that isn't its own.
            if self.section != Section::Behaviors {
                self.prose.end();
            }
            return;
        }
        if trimmed.starts_with('|') || heading(line, 1).is_some() {
            self.prose.end();
            return;
        }
        match self.section {
            Section::Purpose if is_heading(line) => self.prose.end(),
            Section::Purpose => match list_item(line) {
                // Each item of a list is a paragraph of its own.
                Some(item) => {
                    self.prose.end();
                    self.prose.paragraph(number, item.trim());
                }
                None => self.prose.paragraph(number, trimmed),
            },
            Section::Behaviors => self.ste_behaviors(number, line),
            Section::Preamble | Section::Constraints | Section::Other => self.prose.end(),
        }
    }

    /// A line of `# Behaviors` for STE's checks: a behavior's title, a step, or a step's next line.
    fn ste_behaviors(&mut self, number: usize, line: &str) {
        if let Some(text) = heading(line, 2) {
            self.prose.end();
            // The title after `B1:`; a heading without one has a problem of its own.
            let title = text
                .split_once(':')
                .filter(|(id, _)| BehaviorId::parse(id.trim()).is_some());
            if let Some((_, title)) = title {
                self.prose.title(number, title.trim());
            }
        } else if is_heading(line) {
            self.prose.end();
        } else if let Some(item) = list_item(line) {
            let first = item.split_whitespace().next().unwrap_or_default();
            if Keyword::parse(first).is_some() {
                self.prose.step(number, item.trim());
            } else {
                self.prose.end();
            }
        } else if line.starts_with([' ', '\t']) {
            self.prose.continuation(number, line.trim());
        } else {
            self.prose.end();
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

    /// Returns the title and description when the frontmatter is valid. `end` is the line of its
    /// closing `---`, from 0.
    fn frontmatter(&mut self, keys: &Keys, end: usize) -> Option<(String, Option<String>)> {
        // What the idea's hash leaves out, lengua's own record of trust, is no part of what
        // lengua reads: left out here too, an alias of an anchor there names nothing, and the
        // YAML is refused, so nothing read can change with the hash staying as it is.
        let unhashed = |line: &usize| {
            keys.blocks.iter().any(|block| {
                crate::trust::TRUST_KEYS.contains(&block.key.as_str()) && block.holds(*line)
            })
        };
        let yaml = (1..end)
            .filter(|line| !unhashed(line))
            .map(|line| keys.lines[line])
            .collect::<Vec<_>>()
            .join("\n");
        // Only `true` and `false` are booleans, so a parameter may be called `n` or `y`.
        let parsed = if yaml.trim().is_empty() {
            Ok(Frontmatter::default())
        } else {
            serde_saphyr::from_str_with_options::<Frontmatter>(
                &yaml,
                serde_saphyr::options! { strict_booleans: true },
            )
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
        let kind = frontmatter.kind.as_deref();
        match kind {
            Some(ATTESTED_TYPE | IDEA_TYPE | LEGACY_IDEA_TYPE) => {}
            Some(other) => self.problem(
                keys.line("type").or(Some(1)),
                format!(
                    "the frontmatter says `type: {other}`; a lengua idea is `type: \
                     {ATTESTED_TYPE}`, which lengua compiles to main.wat"
                ),
            ),
            None => self.problem(
                Some(1),
                format!(
                    "the frontmatter has no `type`; add `type: {ATTESTED_TYPE}`, the type of an \
                     idea that lengua compiles to main.wat"
                ),
            ),
        }
        let title = frontmatter
            .title
            .as_deref()
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
            .as_deref()
            .map(|description| description.trim().to_owned())
            .filter(|description| !description.is_empty());
        if description.is_none() {
            self.problem(
                Some(1),
                "the frontmatter has no `description`; add one sentence that says what the \
                 program does, such as `description: Adds the two whole numbers in its \
                 arguments and prints the sum.`, which the bundle's index.md lists the program \
                 with",
            );
        }
        match kind {
            Some(ATTESTED_TYPE) => self.attested_keys(keys, &frontmatter),
            Some(kind @ (IDEA_TYPE | LEGACY_IDEA_TYPE)) => self.native_keys(keys, kind),
            _ => {}
        }
        title.map(|title| (title, description))
    }

    /// An idea of the native lengua has none of an Attested Computation's own keys.
    fn native_keys(&mut self, keys: &Keys, kind: &str) {
        self.native = true;
        for key in ATTESTED_KEYS {
            if let Some(line) = keys.line(key) {
                self.problem(
                    Some(line),
                    format!(
                        "`{key}` is a key of an Attested Computation, and this idea is `type: \
                         {kind}`, the native lengua's: write `type: {ATTESTED_TYPE}`, the type \
                         of an idea that lengua compiles to main.wat"
                    ),
                );
            }
        }
    }

    /// An Attested Computation's own keys: `runtime`, `computation`, `executor` and `attester`,
    /// which it must have, and `parameters` and `status`, which it may.
    fn attested_keys(&mut self, keys: &Keys, front: &Frontmatter) {
        self.exact(
            keys,
            ("runtime", RUNTIME),
            front.runtime.as_ref(),
            "lengua runs an Attested Computation as WebAssembly",
        );
        // `main.wasm` is an outside module's, which the `- Module:` line must then name too
        // ([`Self::outside_agrees`]); anything else is held to main.wat.
        let outside = front.computation.as_ref().and_then(scalar_text).as_deref()
            == Some(crate::wasm::outside::FILE);
        if outside {
            self.computation = Some((keys.line("computation"), true));
        } else {
            let before = self.problems.len() + self.hidden_problems;
            self.exact(
                keys,
                ("computation", crate::state::CODE_FILE),
                front.computation.as_ref(),
                "the computation is main.wat, the code file beside idea.md",
            );
            if self.problems.len() + self.hidden_problems == before {
                self.computation = Some((keys.line("computation"), false));
            }
        }
        let receipt = self.executor(keys, front.executor.as_ref());
        self.attester(keys, front.attester.as_ref());
        let parameters = self.parameters(keys, front.parameters.as_ref());
        self.status(keys, front.status.as_ref());
        let computation = if outside {
            crate::wasm::outside::FILE
        } else {
            crate::state::CODE_FILE
        };
        self.attested = Some(Attested {
            runtime: RUNTIME.to_owned(),
            computation: computation.to_owned(),
            parameters,
            receipt,
        });
    }

    /// A key, `(key, expected)`, that an Attested Computation must give just one value, for the
    /// reason `why`.
    fn exact(
        &mut self,
        keys: &Keys,
        (key, expected): (&str, &str),
        value: Option<&Value>,
        why: &str,
    ) {
        let fix = format!("{key}: {expected}");
        match value {
            None => self.missing(keys, key, &fix, why),
            Some(value) if scalar_text(value).as_deref() == Some(expected) => {}
            Some(value) => self.problem(
                keys.line(key),
                format!(
                    "the frontmatter says `{key}: {}`; {why}, so write `{fix}`",
                    shown(value)
                ),
            ),
        }
    }

    /// A key that an Attested Computation must have, with no value: on the frontmatter's first
    /// line when it's missing, or on its own when it's empty.
    fn missing(&mut self, keys: &Keys, key: &str, fix: &str, why: &str) {
        match keys.line(key) {
            Some(line) => self.problem(
                Some(line),
                format!("`{key}` is empty; {why}, so write `{fix}`"),
            ),
            None => self.problem(
                Some(1),
                format!("the frontmatter has no `{key}`; {why}, so add `{fix}`"),
            ),
        }
    }

    /// `executor`: lengua's `run`, and the receipt each run returns, which it returns.
    fn executor(&mut self, keys: &Keys, value: Option<&Value>) -> Vec<Receipt> {
        const FIX: &str = "executor: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md, receipt: [stdout, stderr, exit] }";
        const WHY: &str = "lengua's `run` runs the computation and returns a receipt";
        let Some(value) = value else {
            self.missing(keys, "executor", FIX, WHY);
            return Vec::new();
        };
        let Some(map) = value.as_object() else {
            self.problem(
                keys.line("executor"),
                format!(
                    "the frontmatter says `executor: {}`; {WHY}, so write `{FIX}`",
                    shown(value)
                ),
            );
            return Vec::new();
        };
        self.resource(
            keys,
            ("executor", EXECUTOR),
            map.get("resource"),
            "lengua's `run` runs the computation",
        );
        self.receipt(keys, map.get("receipt"))
    }

    /// `executor.receipt`: what each run returns for the attester, from stdout, stderr, exit and
    /// files, each once.
    fn receipt(&mut self, keys: &Keys, value: Option<&Value>) -> Vec<Receipt> {
        let line = keys.nested("executor", "receipt");
        let names = "stdout, stderr, exit and files";
        let usage = format!(
            "it lists what each run returns for the attester to check, from {names}: write \
             `receipt: [stdout, stderr, exit]`"
        );
        let items = match value.filter(|value| !value.is_null()) {
            None => {
                self.problem(line, format!("`executor` has no `receipt`; {usage}"));
                return Vec::new();
            }
            Some(value) => match value.as_array() {
                Some(items) if !items.is_empty() => items,
                _ => {
                    self.problem(
                        line,
                        format!("`executor` says `receipt: {}`; {usage}", shown(value)),
                    );
                    return Vec::new();
                }
            },
        };
        let mut receipt = Vec::new();
        for item in items {
            match scalar_text(item).as_deref().and_then(Receipt::from_name) {
                Some(part) if receipt.contains(&part) => self.problem(
                    line,
                    format!("`receipt` names `{}` twice; keep it once", part.name()),
                ),
                Some(part) => receipt.push(part),
                None => self.problem(
                    line,
                    format!(
                        "`receipt` names `{}`, which lengua's `run` doesn't return: use {names}",
                        shown(item)
                    ),
                ),
            }
        }
        receipt
    }

    /// `attester`: lengua's `check`.
    fn attester(&mut self, keys: &Keys, value: Option<&Value>) {
        const FIX: &str = "attester: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md }";
        const WHY: &str = "lengua's `check` checks each run";
        let Some(value) = value else {
            self.missing(keys, "attester", FIX, WHY);
            return;
        };
        match value.as_object() {
            Some(map) => self.resource(keys, ("attester", ATTESTER), map.get("resource"), WHY),
            None => self.problem(
                keys.line("attester"),
                format!(
                    "the frontmatter says `attester: {}`; {WHY}, so write `{FIX}`",
                    shown(value)
                ),
            ),
        }
    }

    /// The `resource` of `(parent, expected)`, an executor or an attester, which must be
    /// lengua's, for the reason `why`.
    fn resource(
        &mut self,
        keys: &Keys,
        (parent, expected): (&str, &str),
        value: Option<&Value>,
        why: &str,
    ) {
        let line = keys.nested(parent, "resource");
        match value.filter(|value| !value.is_null()) {
            Some(value) if scalar_text(value).as_deref() == Some(expected) => {}
            Some(value) => self.problem(
                line,
                format!(
                    "`{parent}` says `resource: {}`; {why}, so write `resource: {expected}`",
                    shown(value)
                ),
            ),
            None => self.problem(
                line,
                format!("`{parent}` has no `resource`; {why}, so add `resource: {expected}`"),
            ),
        }
    }

    /// `status`, when it's given: one of OKF's three (§5.4).
    fn status(&mut self, keys: &Keys, value: Option<&Value>) {
        let Some(value) = value else {
            return;
        };
        if !matches!(
            scalar_text(value).as_deref(),
            Some("draft" | "stable" | "deprecated")
        ) {
            self.problem(
                keys.line("status"),
                format!(
                    "the frontmatter says `status: {}`; OKF's statuses are draft, stable and \
                     deprecated: write one of them, or leave `status` out, which means stable",
                    shown(value)
                ),
            );
        }
    }

    /// `parameters`, when it's given: a list of `{ name, type, required, from }`, each name its
    /// own and each source with one parameter, the arguments numbered from 1 without gaps.
    /// Returns the parameters that are right.
    fn parameters(&mut self, keys: &Keys, value: Option<&Value>) -> Vec<Parameter> {
        let Some(value) = value else {
            return Vec::new();
        };
        let at = keys.line("parameters").unwrap_or(1);
        let Some(items) = value.as_array() else {
            self.problem(
                Some(at),
                format!(
                    "the frontmatter says `parameters: {}`; `parameters` is a list, a mapping for \
                     each parameter, such as `- {PARAMETER}`",
                    shown(value)
                ),
            );
            return Vec::new();
        };
        let spans = keys.entries("parameters");
        let mut parameters: Vec<Parameter> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let span = spans.get(index).copied();
            let line = span.map_or(at, |(start, _)| start + 1);
            if let Some(parameter) = self.parameter(keys, item, (line, span)) {
                parameters.push(parameter);
            }
        }
        self.check_sources();
        parameters
    }

    /// One parameter, on the line `line` and the lines `span` when it has lines of its own.
    fn parameter(
        &mut self,
        keys: &Keys,
        item: &Value,
        (line, span): (usize, Option<(usize, usize)>),
    ) -> Option<Parameter> {
        let Some(map) = item.as_object() else {
            self.problem(
                Some(line),
                format!(
                    "each parameter is a mapping, such as `{PARAMETER}`, not `{}`",
                    shown(item)
                ),
            );
            return None;
        };
        let name = self.parameter_name(map.get("name"), keys.within(span, "name", line));
        let called = name.as_deref().map_or_else(
            || "a parameter".to_owned(),
            |name| format!("the parameter `{name}`"),
        );
        let kind = self.parameter_type(&called, map.get("type"), keys.within(span, "type", line));
        let required = self.parameter_required(
            &called,
            map.get("required"),
            keys.within(span, "required", line),
        );
        let from = self.parameter_source(&called, map.get("from"), keys.within(span, "from", line));
        if let Some(source) = &from {
            self.sources.push((called, source.clone(), line));
        }
        Some(Parameter {
            name: name?,
            kind: kind?,
            required: required?,
            from: from?,
            line,
        })
    }

    /// A parameter's `name`, on the line `line`: a lowercase letter, then lowercase letters,
    /// digits and `_`, which no parameter before it has, whatever else is wrong with that one.
    fn parameter_name(&mut self, value: Option<&Value>, line: usize) -> Option<String> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            self.problem(
                Some(line),
                "a parameter has no `name`; give it one in lowercase letters, digits and `_`, \
                 such as `name: count`",
            );
            return None;
        };
        let name = shown(value);
        let valid = name.starts_with(|first: char| first.is_ascii_lowercase())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if !valid {
            self.problem(
                Some(line),
                format!(
                    "the parameter name `{name}` isn't a lowercase letter and then lowercase \
                     letters, digits and `_`: write it like `count` or `max_lines`"
                ),
            );
            return None;
        }
        let repeated = self.names.iter().find(|(known, _)| *known == name);
        if let Some(&(_, first)) = repeated {
            self.problem(
                Some(line),
                format!(
                    "the parameter `{name}` is named twice (also on line {first}); give each \
                     parameter its own name"
                ),
            );
            return None;
        }
        self.names.push((name.clone(), line));
        Some(name)
    }

    /// A parameter's `type`, on the line `line`.
    fn parameter_type(
        &mut self,
        called: &str,
        value: Option<&Value>,
        line: usize,
    ) -> Option<ParameterType> {
        let types = ParameterType::ALL
            .map(|kind| format!("`type: {}`", kind.name()))
            .join(", ");
        let Some(value) = value.filter(|value| !value.is_null()) else {
            self.problem(
                Some(line),
                format!("{called} has no `type`; add one of {types}"),
            );
            return None;
        };
        let kind = scalar_text(value)
            .as_deref()
            .and_then(ParameterType::from_name);
        if kind.is_none() {
            self.problem(
                Some(line),
                format!(
                    "{called} says `type: {}`; lengua's types are integer, decimal, text and \
                     bytes: write one of {types}",
                    shown(value)
                ),
            );
        }
        kind
    }

    /// A parameter's `required`, on the line `line`: a boolean, true when it's left out.
    fn parameter_required(
        &mut self,
        called: &str,
        value: Option<&Value>,
        line: usize,
    ) -> Option<bool> {
        match value {
            None | Some(Value::Null) => Some(true),
            Some(Value::Bool(required)) => Some(*required),
            Some(other) => {
                // A boolean in quotes is text, so the problem shows the quotes to take away.
                let quoted = matches!(other.as_str(), Some("true" | "false"));
                let (written, how) = if quoted {
                    (other.to_string(), " without quotes")
                } else {
                    (shown(other), "")
                };
                self.problem(
                    Some(line),
                    format!(
                        "{called} says `required: {written}`; write `required: true` or \
                         `required: false`{how}, or leave it out, which means true"
                    ),
                );
                None
            }
        }
    }

    /// Where a parameter's value comes from, its `from` on the line `line`: `argument N`,
    /// `standard input`, `environment NAME` or `file NAME`.
    fn parameter_source(
        &mut self,
        called: &str,
        value: Option<&Value>,
        line: usize,
    ) -> Option<Source> {
        let forms = "`from: argument 1`, `from: standard input`, `from: environment NAME` or \
                     `from: file NAME`";
        let arguments = "arguments are numbered from 1, as in `from: argument 1`";
        let variables = "an environment variable's name is capitals, digits and `_`, as in \
                         `from: environment HOME`";
        let files = "name a file in the working folder by its path there, as in `from: file \
                     input.txt`";
        let Some(value) = value.filter(|value| !value.is_null()) else {
            self.problem(
                Some(line),
                format!("{called} has no `from`; say where its value comes from: {forms}"),
            );
            return None;
        };
        let text = shown(value);
        let words = text.trim();
        let says = format!("{called} says `from: {text}`");
        let found = if words == "standard input" {
            Ok(Source::StandardInput)
        } else if let Some(place) = words.strip_prefix("argument ") {
            match place.trim().parse::<usize>() {
                Ok(place) if place >= 1 => Ok(Source::Argument(place)),
                _ => Err(arguments.to_owned()),
            }
        } else if let Some(name) = words.strip_prefix("environment ") {
            let name = name.trim();
            if variable_name(name) {
                Ok(Source::Environment(name.to_owned()))
            } else {
                Err(variables.to_owned())
            }
        } else if let Some(name) = words.strip_prefix("file ") {
            let name = name.trim();
            match file_problem(name) {
                None => Ok(Source::File(name.to_owned())),
                Some(why) => Err(format!("{why}: {files}")),
            }
        } else {
            Err(format!("write {forms}"))
        };
        match found {
            Ok(source) => Some(source),
            Err(fix) => {
                self.problem(Some(line), format!("{says}; {fix}"));
                None
            }
        }
    }

    /// Each source gives one parameter, and the arguments are numbered from 1 without gaps.
    fn check_sources(&mut self) {
        let mut found = Vec::new();
        for (index, (called, source, line)) in self.sources.iter().enumerate() {
            let Some((other, _, at)) = self.sources[..index]
                .iter()
                .find(|(_, earlier, _)| earlier == source)
            else {
                continue;
            };
            let message = if *source == Source::StandardInput {
                format!(
                    "{called} comes from standard input, as {other} does (line {at}); a program \
                     has one standard input, so give it to one parameter"
                )
            } else {
                let source = source.describe();
                format!(
                    "{called} comes from `{source}`, as {other} does (line {at}); give each \
                     parameter a source of its own"
                )
            };
            found.push((*line, message));
        }
        let mut places: Vec<(usize, &str, usize)> = self
            .sources
            .iter()
            .filter_map(|(called, source, line)| match source {
                Source::Argument(place) => Some((*place, called.as_str(), *line)),
                _ => None,
            })
            .collect();
        places.sort_by_key(|(place, ..)| *place);
        places.dedup_by_key(|(place, ..)| *place);
        let gap = (1..)
            .zip(&places)
            .find(|(expected, (place, ..))| place != expected);
        if let Some((expected, (place, called, line))) = gap {
            found.push((
                *line,
                format!(
                    "{called} comes from argument {place}, but no parameter comes from argument \
                     {expected}: number the arguments from 1 without gaps"
                ),
            ));
        }
        for (line, message) in found {
            self.problem(Some(line), message);
        }
    }

    /// Each parameter's source is one that the idea's `- Uses:` line names, but an argument,
    /// which needs nothing named.
    fn sources_used(&mut self, uses: &[Use]) {
        let mut unnamed = Vec::new();
        for (called, source, line) in &self.sources {
            let Some(needed) = source.uses() else {
                continue;
            };
            if uses.contains(&needed) {
                continue;
            }
            let source = source.describe();
            let word = needed.name();
            unnamed.push((
                *line,
                format!(
                    "{called} comes from `{source}`, which the idea doesn't allow: add `{word}` \
                     to the `- Uses:` line of `# Constraints`, as in `- Uses: {word}`"
                ),
            ));
        }
        for (line, message) in unnamed {
            self.problem(Some(line), message);
        }
    }

    /// Keeps the markdown links on the line, but a `- Builds on:` line's, which are its parts.
    fn links_line(&mut self, number: usize, line: &str) {
        if self.section == Section::Constraints && builds_on_item(line).is_some() {
            return;
        }
        for (text, target) in crate::graph::okf::links(line) {
            self.links.push(Link {
                line: number,
                text,
                target,
            });
        }
    }

    /// A code block: under a behavior, a ```toml block is its tests for every input; anywhere
    /// else a block is the idea's own text.
    /// A fenced block is the idea's own prose, unless it holds tests in TOML, which an idea
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
                 write-idea skill's references/test-format.md shows them",
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

    /// A line outside code blocks as [`grants`] reads it: a `#` heading enters its section, as
    /// [`Self::line`] does, and a line of `# Constraints` is read as there; every other line is
    /// left as it is.
    fn grants_line(&mut self, number: usize, line: &str) {
        if let Some(name) = heading(line, 1) {
            self.enter_section(number, name);
        } else if self.section == Section::Constraints {
            self.constraint_line(number, line);
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
    /// ends with the program still serving. The order of Given, When and Then is the write-idea
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
                         `- And it exits with code `0``, or for a server that does not stop, \
                         `- And it does not stop`"
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

    /// `- Builds on: [sort lines](../sort/idea.md), [numbers](../numbers/idea.md)`: the parts,
    /// each a link to its idea by a path relative to this one, so the graph travels with the
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
                && target.ends_with("idea.md");
            if !portable {
                self.problem(
                    Some(number),
                    format!(
                        "the part [{name}]({target}) links to its idea by a path relative to this \
                         one, ending in idea.md, so the graph travels with the folders: such as \
                         `[{name}](../{}/idea.md)`",
                        name.to_lowercase().replace(' ', "-")
                    ),
                );
                continue;
            }
            if self.parts.iter().any(|part| part.idea == target) {
                self.problem(Some(number), format!("the part {target} is listed twice"));
                continue;
            }
            self.parts.push(Part { name, idea: target });
        }
        if found == 0 {
            self.problem(
                Some(number),
                "`- Builds on:` links to each part's idea: `- Builds on: [sort lines](../sort/idea.md)`",
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
        if let Some(rest) = strip_prefix_ignore_case(item, "files:") {
            self.files_line(number, rest);
            return;
        }
        if let Some(rest) = strip_prefix_ignore_case(item, "runs:") {
            self.runs_line(number, rest);
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
        if let Some(rest) = strip_prefix_ignore_case(item, "secret:") {
            self.secret_line(number, rest);
            return;
        }
        if let Some(rest) = strip_prefix_ignore_case(item, "module:") {
            self.module_line(number, rest);
            return;
        }
        let Some(rest) = strip_prefix_ignore_case(item, "uses:") else {
            self.problem(
                Some(number),
                "`# Constraints` lists only what the program uses, which means the same on every \
                 computer: `- Uses: …`, `- Network: …`, `- Files: …`, `- Runs: …`, `- Secret: …` \
                 and `- Builds on: …`, and `- Module: …` for an outside module; say \
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
                    format!(
                        "a program runs as one thread, so an idea names no threads: remove \
                         `{name}`"
                    ),
                ),
                None => self.problem(
                    Some(number),
                    format!(
                        "`{name}` isn't something lengua lets a program use; it may use {known} \
                         and the network, besides writing its output. The files its arguments \
                         name and the programs it starts have lines of their own: `- Files: reads \
                         the file its first argument names` and ``- Runs: `hostname` with no \
                         arguments``. lengua programs can't allocate memory or handle signals"
                    ),
                ),
            }
        }
        uses.sort_unstable();
        self.uses = Some(uses);
    }

    /// `- Files: reads the file its first argument names`: a file or a folder the user names
    /// when the program runs, which the program may open ([`crate::named`]).
    fn files_line(&mut self, number: usize, rest: &str) {
        let lower = rest.trim().replace('`', "").to_ascii_lowercase();
        let added = crate::named::Grant::read(&lower)
            .and_then(|grant| crate::named::add(&mut self.network.reach.files, grant));
        if let Err(problem) = added {
            self.problem(Some(number), problem);
        }
    }

    /// ``- Runs: `hostname` with no arguments``: a program the program may start, with the
    /// arguments the line bounds ([`crate::starts`]). The line is read as it is written: a
    /// program's name and its arguments keep their case.
    fn runs_line(&mut self, number: usize, rest: &str) {
        let added = crate::starts::Rule::read(rest)
            .and_then(|rule| crate::starts::add(&mut self.network.reach.runs, rule));
        if let Err(problem) = added {
            self.problem(Some(number), problem);
        }
    }

    /// `- Network: connects to TCP ports 80 and 8080`, `- Network: connects to TCP port 443 on
    /// example.com` or `- Network: listens on TCP port 8080`.
    fn network_line(&mut self, number: usize, rest: &str) {
        let usage = "write `- Network: connects to TCP port 8080` (or `ports 80 and 8080`) for a \
                     program that talks to a server on this computer, `- Network: connects to \
                     TCP port 443 on example.com` for one on another computer, `- Network: \
                     listens on TCP port 8080` for one that serves, or `- Network: opens TCP \
                     connections to port 443 on any subdomain of example.com` for one that \
                     chooses where it connects while it runs";
        let rest = rest.trim().trim_end_matches('.').replace('`', "");
        let mut lower = rest.to_ascii_lowercase();
        // What the program opens itself at run time: `opens TCP connections to …`, `looks up the
        // addresses of …`, `sends UDP datagrams to …`, `receives UDP datagrams on …`.
        match self.network.reach.line(&lower) {
            Some(Ok(())) => return,
            Some(Err(problem)) => {
                self.problem(Some(number), problem);
                return;
            }
            None => {}
        }
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
                        "lengua makes each of these connections before the program starts, so the \
                         idea says where each goes: name the computer, as in `- Network: connects \
                         to TCP port 443 on example.com`, or leave it out for this computer. A \
                         program that chooses the computer while it runs opens its connections \
                         itself: `- Network: opens TCP connections to port 443 on any computer`",
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
                                 its idea names, so keep it once",
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

    /// `- Threads: up to 16`: how many threads the program may run at once, which check refuses,
    /// as a program runs as one thread. A line that doesn't read so, or a second one, is refused
    /// here with the same words.
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
                "a program runs as one thread, so an idea names no threads: remove the \
                 `- Threads:` lines",
            ),
            (Some(count), None) => self.threads = Some(count),
            (None, None) => self.problem(
                Some(number),
                "a program runs as one thread, so an idea names no threads: remove the \
                 `- Threads:` line",
            ),
        }
    }

    /// `- Module: main.wasm, built from Rust with rustc for wasm32-wasip1`: the idea's program
    /// is an outside module, and what the idea says it was built from.
    fn module_line(&mut self, number: usize, rest: &str) {
        if let Some(first) = self.outside.as_ref().map(|(line, _)| *line) {
            self.problem(
                Some(number),
                format!(
                    "the module is named twice (also on line {first}). Keep one `- Module:` line"
                ),
            );
            return;
        }
        let file = crate::wasm::outside::FILE;
        let built_from = rest
            .trim()
            .strip_prefix(file)
            .map(str::trim_start)
            .and_then(|after| after.strip_prefix(','))
            .map(str::trim_start)
            .and_then(|after| strip_prefix_ignore_case(after, "built from "))
            .map(|words| words.trim().trim_end_matches('.').trim())
            .filter(|words| {
                !words.is_empty()
                    && words.chars().count() <= MAX_BUILT_FROM
                    && !words.chars().any(char::is_control)
            });
        match built_from {
            Some(words) => {
                self.outside = Some((
                    number,
                    Outside {
                        built_from: words.to_owned(),
                    },
                ));
            }
            None => self.problem(Some(number), MODULE_FORM),
        }
    }

    /// An outside module's idea says so twice, and the two agree: its frontmatter's
    /// `computation: main.wasm` and its `- Module:` line. It names no secret, since the secret
    /// rule reads main.wat, and builds on no part, since a part gives functions of its main.wat.
    /// `secret` is the `- Secret:` line's line, when the idea has one.
    fn outside_agrees(&mut self, secret: Option<usize>) {
        let file = crate::wasm::outside::FILE;
        let code = crate::state::CODE_FILE;
        let line = self.outside.as_ref().map(|(line, _)| *line);
        match (line, self.computation) {
            (Some(line), Some((_, false))) => self.problem(
                Some(line),
                format!(
                    "the idea has a `- Module:` line, so its program is {file}, an outside \
                     module, and its frontmatter says so too: write `computation: {file}`; or \
                     remove the line, for a program that lengua checks and proves as {code}"
                ),
            ),
            (None, Some((key, true))) => self.problem(
                key,
                format!(
                    "the frontmatter says `computation: {file}`, an outside module, so `# \
                     Constraints` says what it was built from: add `- Module: {file}, built \
                     from …`, with the language and the compiler; or write `computation: \
                     {code}`, for a program that lengua checks and proves"
                ),
            ),
            _ => {}
        }
        let Some(line) = line else {
            return;
        };
        if self.native {
            self.problem(
                Some(line),
                "`- Module:` is for an idea of `type: Attested Computation`: change the type",
            );
        }
        if let Some(secret) = secret {
            self.problem(
                Some(secret),
                format!(
                    "an outside module keeps no secret that lengua can check: the secret rule \
                     reads {code}, and {file} has none. Remove the `- Secret:` line, or compile \
                     the idea to {code} with the compile skill"
                ),
            );
        }
        if !self.parts.is_empty() {
            self.problem(
                Some(line),
                format!(
                    "an outside module builds on no part: a part gives functions of its {code}, \
                     which {file} can't take. Remove the `- Builds on:` line"
                ),
            );
        }
    }

    /// ``- Secret: standard input, a file `key.txt` and argument `2` ``: the inputs whose values
    /// must not decide how the program runs, several on one line.
    fn secret_line(&mut self, number: usize, rest: &str) {
        if let Some(first) = self.secrets.as_ref().map(|(line, _)| *line) {
            self.problem(
                Some(number),
                format!(
                    "the secrets are on two lines (also on line {first}). Keep one `- Secret:` \
                     line"
                ),
            );
            return;
        }
        let items = secret_items(rest);
        if items.is_empty() {
            self.problem(
                Some(number),
                format!("`- Secret:` names no input. {SECRET_FORMS}"),
            );
        }
        let mut found: Vec<Source> = Vec::new();
        for item in &items {
            match secret_source(item) {
                Ok(source) if found.contains(&source) => self.problem(
                    Some(number),
                    format!(
                        "{} is secret twice on this line. Name each secret once",
                        source.secret_words()
                    ),
                ),
                Ok(source) => found.push(source),
                Err(problem) => self.problem(Some(number), problem),
            }
        }
        self.secrets = Some((number, found));
    }

    /// The `- Secret:` line's sources, each one the idea lets the program read: with the
    /// `- Uses:` word it needs, but an argument, which needs none, and standard input, which the
    /// terminal reads too. An idea of the native lengua has none.
    fn secrets_used(&mut self, uses: &[Use]) -> Vec<Source> {
        let Some((line, secrets)) = self.secrets.take() else {
            return Vec::new();
        };
        if self.native {
            self.problem(
                Some(line),
                "`- Secret:` is for an idea that lengua compiles to main.wat, `type: Attested \
                 Computation`: change the type",
            );
        }
        for source in &secrets {
            let needed = match source {
                Source::StandardInput if uses.contains(&Use::Terminal) => None,
                other => other.uses(),
            };
            if let Some(needed) = needed
                && !uses.contains(&needed)
            {
                let word = needed.name();
                self.problem(
                    Some(line),
                    format!(
                        "{} is secret, but the `- Uses:` line does not name {word}. Add `{word}` \
                         to the `- Uses:` line, as in `- Uses: {word}`",
                        source.secret_words()
                    ),
                );
            }
        }
        secrets
    }

    fn finish(mut self, header: Option<(String, Option<String>)>) -> Result<Idea, Vec<Problem>> {
        if !self.purpose_written {
            self.problem(
                None,
                "idea.md needs a `# Purpose` section that says in plain words what the program \
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
                "idea.md needs a `# Behaviors` section with at least one behavior, such as \
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
        self.check_network();
        self.sources_used(&uses);
        let secret_line = self.secrets.as_ref().map(|(line, _)| *line);
        let secrets = self.secrets_used(&uses);
        self.outside_agrees(secret_line);
        // STE's problems come last: what the idea says matters before how it says it.
        for (line, message) in std::mem::take(&mut self.prose).finish() {
            self.problem(Some(line), message);
        }
        match header {
            Some((title, Some(description))) if self.problems.is_empty() => Ok(Idea {
                title,
                description,
                behaviors: self.behaviors,
                uses,
                syscalls,
                threads: self.threads,
                purpose_values: self.purpose_values,
                network: self.network,
                secrets,
                parts: self.parts,
                links: self.links,
                attested: self.attested,
                outside: self.outside.map(|(_, outside)| outside),
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

    /// A behavior keeps serving only in a server, which takes TCP connections or UDP datagrams,
    /// one of the two. The computers the idea's tests stand in for, and the names they give
    /// addresses, are read from its steps here.
    fn check_network(&mut self) {
        if self.network.listens.is_some() && self.network.reach.receives.is_some() {
            self.problem(
                None,
                "a program listens on a TCP port or receives UDP datagrams on a port, one of the \
                 two: keep `- Network: listens on TCP port N` or `- Network: receives UDP \
                 datagrams on port N`",
            );
        }
        let steps = self.behaviors.iter().flat_map(|behavior| {
            let outline = behavior.outline.iter().flat_map(|outline| &outline.steps);
            behavior.steps.iter().chain(outline)
        });
        let mut stands: Vec<Stand> = Vec::new();
        let mut names: Vec<(String, Vec<String>)> = Vec::new();
        let mut programs: Vec<crate::starts::Stood> = Vec::new();
        for step in steps {
            let tokens = step.tokens();
            if let Some(stand) = stand_named(&tokens) {
                // A place the idea names for lengua to connect to before the start is that
                // connection's; any other is one the program reaches itself, when a rule of the
                // idea covers it.
                let made = !stand.datagrams
                    && self.network.connections.iter().any(|place| {
                        place.port == stand.port
                            && place.host.as_deref() == Some(stand.host.as_str())
                    });
                let rules = if stand.datagrams {
                    &self.network.reach.datagrams
                } else {
                    &self.network.reach.opens
                };
                let covered = crate::reach::spelled(&stand.host).is_ok_and(|computer| {
                    Grants::granted(rules, &computer, stand.port, None, None).is_some()
                });
                if !made && covered && !stands.contains(&stand) {
                    stands.push(stand);
                }
            } else if let Some((name, addresses)) = name_given(&tokens)
                && !names.iter().any(|(known, _)| *known == name)
            {
                names.push((name, addresses));
            } else if let Some(stood) = program_stood(&tokens)
                && !programs.iter().any(|known| {
                    known.program == stood.program && known.arguments == stood.arguments
                })
            {
                programs.push(stood);
            }
        }
        self.network.stands = stands;
        self.network.names = names;
        self.network.programs = programs;
        let listens = self.network.listens.is_some() || self.network.reach.receives.is_some();
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

/// What a `- Secret:` line may name, as its problems say it.
const SECRET_FORMS: &str = "Name standard input, a file `NAME`, the environment variable `NAME` \
                            or argument `N`, as in `- Secret: standard input`";

/// The items of a `- Secret:` line: its text cut at each comma and each ` and ` outside
/// backticks, each without the spaces and the full stop around it.
fn secret_items(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut item = String::new();
    let mut inside = false;
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        if !inside && let Some(after) = strip_prefix_ignore_case(rest, " and ") {
            items.push(std::mem::take(&mut item));
            rest = after;
            continue;
        }
        if character == '`' {
            inside = !inside;
        }
        if character == ',' && !inside {
            items.push(std::mem::take(&mut item));
        } else {
            item.push(character);
        }
        rest = &rest[character.len_utf8()..];
    }
    items.push(item);
    items
        .iter()
        .map(|item| item.trim().trim_end_matches('.').trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// The input an item of a `- Secret:` line names: `standard input`, ``a file `NAME` ``, ``the
/// environment variable `NAME` `` or ``argument `N` ``; or what's wrong with it, in STE.
fn secret_source(item: &str) -> Result<Source, String> {
    let unknown = || format!("`{item}` is not an input that can be secret. {SECRET_FORMS}");
    if item.eq_ignore_ascii_case("standard input") {
        return Ok(Source::StandardInput);
    }
    // The words before the value, and the value: in backticks, or for a number, bare.
    let (words, value) = match item.split_once('`') {
        Some((words, after)) => match after.split_once('`') {
            Some((value, "")) => (words, value),
            _ => return Err(unknown()),
        },
        None => item.rsplit_once(' ').ok_or_else(unknown)?,
    };
    let words = words.trim().to_ascii_lowercase();
    let kind = ["the ", "a ", "an "]
        .iter()
        .find_map(|article| words.strip_prefix(*article))
        .unwrap_or(words.as_str());
    let value = value.trim();
    match kind {
        "file" => match file_problem(value) {
            None => Ok(Source::File(value.to_owned())),
            Some(why) => Err(format!(
                "the secret file `{value}` is not a file in the working folder: {why}. Name it by \
                 its path there, as in a file `key.txt`"
            )),
        },
        "environment variable" | "variable" if variable_name(value) => {
            Ok(Source::Environment(value.to_owned()))
        }
        "environment variable" | "variable" => Err(format!(
            "`{value}` is not the name of an environment variable. Write it in capitals, digits \
             and `_`, as in the environment variable `TOKEN`"
        )),
        "argument" => match value.parse::<usize>() {
            Ok(place) if place >= 1 => Ok(Source::Argument(place)),
            _ => Err(format!(
                "`{value}` is not the place of an argument. Number the arguments from 1, as in \
                 argument `1`"
            )),
        },
        _ => Err(unknown()),
    }
}

/// The `- Uses:` line that says what an older idea's `- Allowed system calls:` line allowed: a
/// idea names what a program uses, the same on every target, not one kernel's system calls.
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
            " ({} can't be used: lengua programs can't allocate memory or handle signals, and \
             they start a program only through a `- Runs:` line)",
            refused.join(", ")
        )
    };
    format!(
        "idea.md names what the program uses, the same on every computer, not Linux system \
         calls: {line}{refused}; the network keeps its own line"
    )
}

/// The heading text when `line` is a heading of exactly `level`.
/// A line of a behavior's steps: a list item starts a step, an indented line continues the one
/// before it, and any other line is the idea's own prose. Returns its problems.
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

/// The text of a heading of `level` (`#` is 1), when `line` is one.
pub(crate) fn heading(line: &str, level: usize) -> Option<&str> {
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

/// What follows `- Builds on:` when `line` is that list item, in any case.
pub(crate) fn builds_on_item(line: &str) -> Option<&str> {
    strip_prefix_ignore_case(list_item(line)?.trim(), "builds on:")
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
pub(crate) fn find_exact_run(text: &str, run: usize) -> Option<usize> {
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

/// Whether a Then step says the program is still serving at the end, as a server does:
/// `it does not stop`. lengua's old phrasings still count, so STE's problem is an old one's only
/// problem.
fn keeps_serving(text: &str) -> bool {
    let words = words(text);
    [
        "does not stop",
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
type: lengua/program-idea
title: add
description: Adds the two whole numbers in its arguments and prints the sum.
---
# Purpose
The program adds two whole numbers between `-1000000000` and `1000000000`.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b |
|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |

## B2: Shows the usage line without arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
"#;

    fn problems(text: &str) -> Vec<String> {
        match parse(text) {
            Ok(idea) => panic!("expected problems, got {idea:?}"),
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

    /// An idea links to the parts it builds on, each by a path relative to it, so the graph of
    /// programs travels with their folders.
    #[test]
    fn an_idea_builds_on_parts() {
        let built = ADD.replace(
            "- Uses: nothing",
            "- Uses: nothing\n- Builds on: [sort lines](../sort/idea.md), [numbers](parts/numbers/idea.md)",
        );
        let idea = parse(&built).expect("valid idea");
        assert_eq!(
            idea.parts,
            vec![
                Part {
                    name: "sort lines".into(),
                    idea: "../sort/idea.md".into(),
                },
                Part {
                    name: "numbers".into(),
                    idea: "parts/numbers/idea.md".into(),
                },
            ]
        );
        assert_eq!(idea.parts[0].folder(), "../sort");
        assert_eq!(folder_of_link("../sort/"), "../sort");
        assert_eq!(folder_of_link("../sort/main.wat"), "../sort");
        for link in [
            "/home/me/sort/idea.md",
            "https://example.com/idea.md",
            "../sort",
        ] {
            let bad = ADD.replace(
                "- Uses: nothing",
                &format!("- Uses: nothing\n- Builds on: [sort]({link})"),
            );
            assert_problem(&bad, "links to its idea by a path relative to this one");
        }
        let bare = ADD.replace("- Uses: nothing", "- Uses: nothing\n- Builds on: sort");
        assert_problem(&bare, "`- Builds on:` links to each part's idea");
    }

    /// An idea names no kind of computer in its prose, unless one is its subject; values,
    /// which the program reads or prints, may be anything.
    #[test]
    fn an_idea_names_no_kind_of_computer() {
        let fast = ADD.replace(
            "adds two whole numbers between",
            "adds, with AVX-512 and one system call, two whole numbers between",
        );
        assert_problem(&fast, "line 7: `AVX-512` names one kind of computer");
        assert_problem(&fast, "`system call` names one kind of computer");
        let described = ADD.replace("prints the sum.", "prints the sum on Linux.");
        assert_problem(&described, "line 4: `Linux` names one kind of computer");
        let registers = ADD.replace(
            "adds two whole numbers between",
            "adds rax and xmm0, between",
        );
        assert_problem(&registers, "`rax`");
        assert_problem(&registers, "`xmm0`");
        // Its subject, values, and everyday words pass.
        let subject = ADD.replace("title: add", "title: x86 add").replace(
            "adds two whole numbers between",
            "adds two x86 registers' whole numbers between",
        );
        assert!(parse(&subject).is_ok(), "{:?}", parse(&subject));
        let values = ADD.replace("prints `5`", "prints `rax`");
        assert!(parse(&values).is_ok(), "{:?}", parse(&values));
        let everyday = ADD.replace(
            "adds two whole numbers between",
            "adds, under a neon sign with a clang, two whole numbers between",
        );
        assert!(parse(&everyday).is_ok(), "{:?}", parse(&everyday));
    }

    #[test]
    fn parses_the_add_idea() {
        let idea = parse(ADD).expect("valid idea");
        assert_eq!(idea.title, "add");
        assert_eq!(
            idea.description,
            "Adds the two whole numbers in its arguments and prints the sum."
        );
        assert_eq!(idea.behaviors.len(), 2);
        let first = &idea.behaviors[0];
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
        assert_eq!(idea.behaviors[1].exit_code, 1);
        assert_eq!(
            idea.behaviors[1].values().collect::<Vec<_>>(),
            ["usage: add <a> <b>", "1"]
        );
        assert!(idea.behaviors[1].outline.is_none());
        // Writing its output and exiting need nothing named.
        assert!(idea.uses.is_empty());
        assert_eq!(
            idea.syscalls,
            [Syscall::Write, Syscall::Exit, Syscall::ExitGroup]
        );
        // Only Purpose states ranges; the behaviors' values (2, 3, 5, 0, 1) aren't among them.
        assert_eq!(idea.purpose_values, ["-1000000000", "1000000000"]);
        let id = BehaviorId::parse("B2").expect("valid ID");
        assert_eq!(
            idea.behavior(id).map(|b| b.title.as_str()),
            Some("Shows the usage line without arguments")
        );
    }

    #[test]
    fn threads_are_allowed_in_plain_words() {
        assert_eq!(parse(ADD).expect("valid idea").threads, None);
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
        // check refuses every `- Threads:` line: one it can't read, or a second, at once.
        let refused = "a program runs as one thread, so an idea names no threads: remove the \
                       `- Threads:` line";
        assert_problem(&with("- Threads: up to 1"), refused);
        assert_problem(&with("- Threads: up to 65"), refused);
        assert_problem(&with("- Threads: many"), refused);
        assert_problem(&with("- Threads: up to 4\n- Threads: up to 8"), refused);
        // A use of threads offers no `- Threads:` line, which check refuses on every idea.
        assert_problem(
            &ADD.replace("- Uses: nothing", "- Uses: threads"),
            "a program runs as one thread, so an idea names no threads: remove `threads`",
        );
    }

    #[test]
    fn each_behavior_names_its_own_rule_and_purpose_says_why() {
        assert_problem(
            &ADD.replace(
                "## B2: Shows the usage line without arguments",
                "## B2: Adds two NUMBERS",
            ),
            "B2 has the same title as B1",
        );
        let start = ADD.find("# Purpose").expect("purpose");
        let end = ADD.find("# Behaviors").expect("behaviors");
        let without_purpose = format!("{}{}", &ADD[..start], &ADD[end..]);
        assert_problem(&without_purpose, "needs a `# Purpose` section");
    }

    #[test]
    fn accepts_windows_line_endings() {
        let idea = parse(&ADD.replace('\n', "\r\n")).expect("valid idea");
        assert_eq!(idea.behaviors.len(), 2);
        let outline = idea.behaviors[0].outline.as_ref().expect("generated tests");
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
            &ADD.replace("type: lengua/program-idea\n", ""),
            "has no `type`",
        );
        assert_problem(
            &ADD.replace("lengua/program-idea", "concept"),
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

    /// The bundle's index.md lists each program with its description, so every idea has one.
    #[test]
    fn requires_a_description() {
        let line = "description: Adds the two whole numbers in its arguments and prints the sum.\n";
        for without in [
            ADD.replace(line, ""),
            ADD.replace(line, "description: \" \"\n"),
        ] {
            assert_eq!(
                problems(&without),
                [
                    "line 1: the frontmatter has no `description`; add one sentence that says what \
                  the program does, such as `description: Adds the two whole numbers in its \
                  arguments and prints the sum.`, which the bundle's index.md lists the program \
                  with"
                ]
            );
        }
    }

    /// An idea links to others in its text with plain markdown links: with its parts, the edges
    /// of the graph of programs.
    #[test]
    fn reads_the_links_to_other_ideas() {
        let linked = ADD
            .replace(
                "# Purpose\n",
                "# Purpose\nLike [subtract](../subtract/idea.md), but `[not](this.md)`.\n",
            )
            .replace(
                "- Uses: nothing",
                "- Uses: nothing\n- Builds on: [numbers](../numbers/idea.md)",
            );
        let idea = parse(&linked).expect("valid idea");
        assert_eq!(
            idea.links,
            [Link {
                line: 7,
                text: "subtract".into(),
                target: "../subtract/idea.md".into(),
            }]
        );
        assert_eq!(idea.parts[0].idea, "../numbers/idea.md");
        assert_eq!(folder_of_link("../numbers/main.wat"), "../numbers");
    }

    /// This computer is known by its address however the address is spelled: the loopback and
    /// the unspecified address written as IPv4 inside IPv6 are this computer's, and a spelling
    /// only a system's older parsing reads as an address (one number, fewer than four parts, hex
    /// or octal) is no computer's name, since such a system reads `2130706433` as 127.0.0.1.
    #[test]
    fn this_computer_is_known_by_its_address_not_its_spelling() {
        for this in ["::ffff:127.0.0.1", "::ffff:127.9.9.9", "::ffff:0.0.0.0"] {
            assert_eq!(host_kind(this), HostKind::ThisComputer, "{this}");
        }
        for nothing in [
            "2130706433",
            "127.1",
            "0x7f.1",
            "0177.0.0.1",
            "0x7f000001",
            "example.0x10",
            "10.1.",
        ] {
            assert_eq!(host_kind(nothing), HostKind::NotAName, "{nothing}");
        }
        for other in [
            "192.0.2.7",
            "::ffff:192.0.2.7",
            "example.com",
            "a1.example",
            "0x.org",
        ] {
            assert_eq!(host_kind(other), HostKind::Other, "{other}");
        }
    }

    /// The frontmatter's `verified` and `generated` are left out of the idea's hash, so nothing
    /// lengua reads may come from them: a key whose value is an alias of an anchor there is
    /// refused, since its value could change with the hash staying as it is.
    #[test]
    fn nothing_read_from_the_frontmatter_comes_from_what_the_hash_leaves_out() {
        let aliased = ADD.replace("title: add\n", "verified: &name add\ntitle: *name\n");
        assert_ne!(aliased, ADD);
        assert_problem(&aliased, "the frontmatter isn't valid YAML");
    }

    /// What a program opens by itself while it runs is granted on `- Network:` lines of their
    /// own, beside the connections lengua makes before the start. The computers the idea's tests
    /// stand in for, and the names they give addresses, are read from its steps: only those a
    /// rule of the idea covers, and never a connection lengua makes.
    #[test]
    fn network_lines_grant_what_a_program_opens_by_itself() {
        use crate::reach::Computers;
        let lines = "- Network: opens TCP connections to ports 80 and 443 on any subdomain of \
                     Example.org.\n- Network: looks up the addresses of any subdomain of \
                     example.org\n- Network: sends UDP datagrams to port `53` on 192.0.2.53\n- \
                     Network: connects to TCP port 8080 on example.com";
        let steps = "- Given the arguments `2` and `3`\n- And a server at `WWW.example.org` on \
                     port `80` that receives `a` and replies `b`\n- And a server at \
                     `example.com` on port `8080` that receives `a` and replies `b`\n- And a \
                     server at `elsewhere.test` on port `80` that receives `a` and replies `b`\n- \
                     And a computer at `192.0.2.53` on UDP port `53` that receives `a` and \
                     replies `b`\n- And the addresses `192.0.2.7` and `2001:DB8::1` for the name \
                     `www.example.org`";
        let granted = ADD.replace("- Uses: nothing", lines).replacen(
            "- Given the arguments `2` and `3`",
            steps,
            1,
        );
        let idea = parse(&granted).expect("valid idea");
        let reach = &idea.network.reach;
        assert_eq!(reach.opens.len(), 1);
        assert_eq!(
            reach.opens[0].computers,
            Computers::Beneath("example.org".into())
        );
        assert_eq!(reach.opens[0].ports, [80, 443]);
        assert_eq!(reach.names.len(), 1);
        assert_eq!(reach.datagrams[0].ports, [53]);
        assert_eq!(reach.receives, None);
        assert_eq!(idea.network.connections.len(), 1);
        assert!(idea.network.is_used());
        let stand = |datagrams, host: &str, port| Stand {
            datagrams,
            host: host.to_owned(),
            port,
        };
        assert_eq!(
            idea.network.stands,
            [
                stand(false, "www.example.org", 80),
                stand(true, "192.0.2.53", 53)
            ],
            "the idea's own connection and the computer no rule covers aren't among them"
        );
        assert_eq!(
            idea.network.names,
            [(
                "www.example.org".to_owned(),
                vec!["192.0.2.7".to_owned(), "2001:db8::1".to_owned()]
            )]
        );
        let with = |line: &str| ADD.replace("- Uses: nothing", line);
        assert_problem(
            &with("- Network: opens TCP connections to port 80"),
            "name the computers",
        );
        assert_problem(
            &with("- Network: opens TCP connections to port 80 on localhost"),
            "is this computer",
        );
        assert_problem(
            &with("- Network: looks up the addresses of any subdomain of org"),
            "isn't a domain",
        );
        assert_problem(
            &with(
                "- Network: listens on TCP port 7007\n- Network: receives UDP datagrams on port \
                 7007",
            ),
            "one of the two",
        );
        let receives = parse(
            &with("- Network: receives UDP datagrams on port 5353 for any computer")
                .replace("- And it exits with code `0`", "- And it does not stop"),
        )
        .expect("valid idea");
        assert_eq!(receives.network.reach.receives, Some((5353, true)));
        assert!(receives.behaviors[0].serves);
    }

    /// `- Files:` and `- Runs:` lines grant the files and folders a program's arguments name
    /// and the programs it starts, each on a line of its own. What its tests say a command gives
    /// is read from its steps, and a run reads the grants and none of that. Neither line makes
    /// the program one that uses the network.
    #[test]
    fn files_and_runs_lines_grant_what_a_program_reaches_by_itself() {
        use crate::named::Which;
        use crate::starts::Slot;
        let lines = "- Files: reads the file its first argument names\n- Files: writes the file \
                     its `first` argument names.\n- Files: Reads and writes the folder its \
                     second argument names\n- Runs: `Git` with the arguments `rev-parse`, a \
                     value and up to 2 values, within 30 seconds";
        let steps = "- Given the arguments `2` and `3`\n- And the program `Git` with the \
                     arguments `rev-parse` and `HEAD` prints `abc\\n` and exits with code `0`\n- \
                     And the program `Git` with the arguments `rev-parse` and `x` prints nothing \
                     and exits with code `128`";
        let granted = ADD.replace("- Uses: nothing", lines).replacen(
            "- Given the arguments `2` and `3`",
            steps,
            1,
        );
        let idea = parse(&granted).expect("valid idea");
        let reach = &idea.network.reach;
        assert_eq!(reach.files.len(), 2);
        assert!(reach.files[0].reads && reach.files[0].writes && !reach.files[0].folder);
        assert_eq!(reach.files[1].which, Which::Argument(2));
        assert!(reach.files[1].folder && reach.files[1].writes);
        assert_eq!(reach.runs.len(), 1);
        assert_eq!(reach.runs[0].program, "Git");
        assert_eq!(
            reach.runs[0].arguments,
            [Slot::Text("rev-parse".to_owned()), Slot::Value]
        );
        assert_eq!((reach.runs[0].more, reach.runs[0].seconds), (2, 30));
        assert!(!idea.network.is_used() && !reach.is_empty() && reach.no_network());
        let stood = &idea.network.programs;
        assert_eq!(stood.len(), 2);
        assert_eq!(stood[0].arguments, ["rev-parse", "HEAD"]);
        assert_eq!((stood[0].output.as_str(), stood[0].code), ("abc\n", 0));
        assert_eq!((stood[1].output.as_str(), stood[1].code), ("", 128));
        // A run reads the grants, and nothing of the tests.
        let (_, network) = grants(&granted).expect("its grants");
        assert_eq!(network.reach, idea.network.reach);
        assert!(network.programs.is_empty());
        let with = |line: &str| ADD.replace("- Uses: nothing", line);
        assert_problem(
            &with("- Files: reads /etc/passwd"),
            "write `- Files: reads the file its first argument names`",
        );
        assert_problem(
            &with("- Files: writes the folder its first argument names"),
            "reads it too",
        );
        assert_problem(
            &with(
                "- Files: reads the file its first argument names\n- Files: reads the folder \
                 its first argument names",
            ),
            "one of the two",
        );
        assert_problem(
            &with("- Runs: `sh` with the arguments `-c` and a value"),
            "is a shell",
        );
        assert_problem(
            &with("- Runs: git status"),
            "the program's name in backticks",
        );
        assert_problem(
            &with("- Uses: files and programs"),
            "have lines of their own",
        );
    }

    #[test]
    fn network_lines_name_the_ports() {
        let client = ADD.replace(
            "- Uses: nothing",
            "- Network: connects to TCP ports `80` and 8080.",
        );
        let idea = parse(&client).expect("valid idea");
        let here = |port| Endpoint {
            host: None,
            argument: None,
            port,
            tls: false,
        };
        assert_eq!(
            idea.network.connections,
            [here(80), here(8080)],
            "this computer, unless the idea says otherwise"
        );
        assert_eq!(idea.network.listens, None);
        // lengua makes the connections: the program never opens a socket or connects.
        assert!(!idea.syscalls.contains(&Syscall::Accept4));
        for call in [
            Syscall::Read,
            Syscall::Sendto,
            Syscall::Shutdown,
            Syscall::Close,
        ] {
            assert!(idea.syscalls.contains(&call), "{call:?}");
        }
        let elsewhere =
            parse(&client.replace("8080.", "8080 on Example.com.")).expect("valid idea");
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
                .expect("valid idea");
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
        .expect("valid idea");
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
        let address = parse(&client.replace("8080.", "8080 on 192.0.2.7.")).expect("valid idea");
        assert_eq!(
            address.network.connections[0].host.as_deref(),
            Some("192.0.2.7")
        );
        let here_too =
            parse(&client.replace("8080.", "8080 on this computer.")).expect("valid idea");
        assert_eq!(here_too.network.connections, idea.network.connections);
        // The program no longer picks where it connects: the idea names the computer.
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
            .replace("- And it exits with code `0`", "- And it does not stop");
        let idea = parse(&server).expect("valid idea");
        assert_eq!(idea.network.listens, Some(7007));
        assert!(
            !idea.network.serves_anywhere,
            "the loopback, unless the idea says otherwise"
        );
        // Clients at once are a server's, from 2 to a limit.
        let at_once = parse(&server.replace("port 7007", "port 7007, up to 64 clients at once"))
            .expect("valid idea");
        assert_eq!(at_once.network.at_once, Some(64));
        assert_eq!(at_once.network.listens, Some(7007));
        assert!(
            at_once.syscalls.contains(&Syscall::Poll) && !idea.syscalls.contains(&Syscall::Poll)
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
                .expect("valid idea");
            assert_eq!(open.network.listens, Some(7007));
            assert!(
                open.network.serves_anywhere && open.network.connections.is_empty(),
                "{any}"
            );
        }
        assert!(idea.behaviors[0].serves);
        assert!(!idea.behaviors[1].serves);
        assert!(
            idea.syscalls.contains(&Syscall::Accept4) && idea.syscalls.contains(&Syscall::Sendto)
        );
        assert_problem(
            &ADD.replace("- And it exits with code `0`", "- And it does not stop"),
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
            &ADD.replace(
                "## B2: Shows the usage line without arguments",
                "## Shows the usage line without arguments",
            ),
            "behavior headings look like",
        );
        assert_problem(
            &ADD.replace("## B2:", "## B02:"),
            "behavior headings look like",
        );
        assert_problem(
            &ADD.replace("## B2: Shows the usage line without arguments", "## B2:"),
            "short title",
        );
    }

    #[test]
    fn requires_given_when_and_then() {
        let without_then = ADD.replace(
            "- Then it prints `5` and a newline\n- And it exits with code `0`\n",
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
        // The order of the other steps is style, the write-idea skill's to keep.
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
            "- Then it prints `5` and a newline",
            "- Then it prints `exit code` and a newline",
        );
        let idea = parse(&text).expect("valid idea");
        assert_eq!(idea.behaviors[0].exit_code, 0);
    }

    #[test]
    fn names_what_the_program_uses() {
        let uses = |line: &str| ADD.replace("- Uses: nothing", line);
        let idea =
            parse(&uses("- Uses: standard input, files and the clock.")).expect("valid idea");
        assert_eq!(idea.uses, [Use::StandardInput, Use::Files, Use::Clock]);
        for call in [Syscall::Read, Syscall::Openat, Syscall::ClockGettime] {
            assert!(idea.syscalls.contains(&call), "{call:?}");
        }
        assert!(!idea.syscalls.contains(&Syscall::Getrandom));
        let idea = parse(&uses("- Uses: `random bytes`")).expect("valid idea");
        assert_eq!(idea.uses, [Use::RandomBytes]);
        // The terminal: its keys, its mode and its size, with a wait for the next key.
        let idea = parse(&uses("- Uses: the terminal")).expect("valid idea");
        assert_eq!(idea.uses, [Use::Terminal]);
        for call in [Syscall::Read, Syscall::Ioctl, Syscall::Poll] {
            assert!(idea.syscalls.contains(&call), "{call:?}");
        }
        // No Constraints at all is a program that needs nothing.
        let end = ADD.find("# Constraints").expect("constraints");
        assert!(parse(&ADD[..end]).expect("valid idea").uses.is_empty());
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
    fn an_older_ideas_system_calls_come_with_their_uses_line() {
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
        // Prose under the heading, and code blocks that hold no tests, are the idea's own.
        let prose = ADD.replace(
            "### For any `a` and `b`\n",
            "### For any `a` and `b`\nAny two numbers in range.\n\n```text\n| x |\n```\n",
        );
        let idea = parse(&prose).expect("valid idea");
        assert_eq!(
            idea.behaviors[0]
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
        assert_eq!(parse(&with_block).expect("valid idea").behaviors.len(), 2);
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
            "- Then it prints `5` and a newline",
            "- Then it prints ``a`b``\n  and then `5`",
        );
        let idea = parse(&text).expect("valid idea");
        assert_eq!(
            idea.behaviors[0].values().collect::<Vec<_>>(),
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

    /// The add idea as an Attested Computation, which lengua compiles to main.wat.
    const ATTESTED: &str = include_str!("../tests/fixtures/add/idea.md");

    /// The parameters of [`ATTESTED`], on its lines 8 and 9.
    const A: &str = "{ name: a, type: integer, required: true, from: argument 1 }";
    const B: &str = "{ name: b, type: integer, required: true, from: argument 2 }";

    /// [`ATTESTED`] with the first `from` in it made `to`.
    fn attested(from: &str, to: &str) -> String {
        assert!(ATTESTED.contains(from), "{from:?}");
        ATTESTED.replacen(from, to, 1)
    }

    /// [`ATTESTED`] with its parameters written as `a` and `b`, and `uses` its `- Uses:` line.
    fn with_parameters(a: &str, b: &str, uses: &str) -> String {
        attested(A, a)
            .replacen(B, b, 1)
            .replace("- Uses: nothing", uses)
    }

    #[test]
    fn parses_an_attested_computation() {
        let idea = parse(ATTESTED).unwrap_or_else(|problems| panic!("{problems:#?}"));
        let argument = |name: &str, place, line| Parameter {
            name: name.to_owned(),
            kind: ParameterType::Integer,
            required: true,
            from: Source::Argument(place),
            line,
        };
        assert_eq!(
            idea.attested,
            Some(Attested {
                runtime: "webassembly".to_owned(),
                computation: "main.wat".to_owned(),
                parameters: vec![argument("a", 1, 8), argument("b", 2, 9)],
                receipt: vec![Receipt::Stdout, Receipt::Stderr, Receipt::Exit],
            })
        );
        // The rest is the add idea's, whose native form has no computation's keys.
        assert_eq!(idea.title, "add");
        assert_eq!(idea.behaviors.len(), 2);
        assert!(idea.uses.is_empty());
        assert_eq!(parse(ADD).expect("valid idea").attested, None);
    }

    /// [`ATTESTED`] as an outside module's idea: its frontmatter names main.wasm, and `line`
    /// follows its `- Uses:` line.
    fn outside_idea(line: &str) -> String {
        attested("computation: main.wat", "computation: main.wasm")
            .replace("- Uses: nothing", &format!("- Uses: nothing\n{line}"))
    }

    /// An idea says its program is an outside module twice, in its frontmatter and in a
    /// `- Module:` line that says what it was built from; a run reads the line alone.
    #[test]
    fn an_idea_names_an_outside_module() {
        let line = "- Module: main.wasm, built from Rust with rustc 1.99 for wasm32-wasip1.";
        let text = outside_idea(line);
        let idea = parse(&text).unwrap_or_else(|problems| panic!("{problems:#?}"));
        let said = Outside {
            built_from: "Rust with rustc 1.99 for wasm32-wasip1".to_owned(),
        };
        assert_eq!(idea.outside, Some(said.clone()));
        assert_eq!(
            idea.attested.map(|attested| attested.computation),
            Some("main.wasm".to_owned())
        );
        assert_eq!(outside(&text), Ok(Some(said)));
        // An idea lengua compiles to main.wat names none.
        assert_eq!(parse(ATTESTED).expect("valid idea").outside, None);
        assert_eq!(outside(ATTESTED), Ok(None));
    }

    /// The two places agree, the line reads one way, and an outside module has no secret and
    /// builds on no part.
    #[test]
    fn an_outside_module_s_idea_is_held_to_its_form() {
        let line = "- Module: main.wasm, built from C with clang";
        // The line without the frontmatter's word, and the word without the line.
        assert_problem(
            &ATTESTED.replace("- Uses: nothing", &format!("- Uses: nothing\n{line}")),
            "write `computation: main.wasm`",
        );
        assert_problem(
            &attested("computation: main.wat", "computation: main.wasm"),
            "add `- Module: main.wasm, built from …`",
        );
        for wrong in [
            "- Module: main.wasm",
            "- Module: main.wasm, built from",
            "- Module: other.wasm, built from C with clang",
            "- Module: built from C with clang",
            "- Module: main.wat, built from C",
        ] {
            assert_problem(&outside_idea(wrong), "an outside module's line reads");
        }
        let long = format!("- Module: main.wasm, built from {}", "c".repeat(201));
        assert_problem(&outside_idea(&long), "an outside module's line reads");
        assert_problem(
            &outside_idea(&format!("{line}\n{line}")),
            "the module is named twice",
        );
        assert_problem(
            &outside_idea(&format!("{line}\n- Secret: argument `1`")),
            "an outside module keeps no secret that lengua can check",
        );
        assert_problem(
            &outside_idea(&format!("{line}\n- Builds on: [sort](../sort/idea.md)")),
            "an outside module builds on no part",
        );
    }

    /// `parameters` may be left out: a computation that reads none has none.
    #[test]
    fn an_attested_computation_may_have_no_parameters() {
        let none = attested(&format!("parameters:\n  - {A}\n  - {B}\n"), "");
        let idea = parse(&none).unwrap_or_else(|problems| panic!("{problems:#?}"));
        let computation = idea.attested.expect("an Attested Computation");
        assert!(computation.parameters.is_empty());
        assert_eq!(
            computation.receipt,
            [Receipt::Stdout, Receipt::Stderr, Receipt::Exit]
        );
    }

    /// Each key an Attested Computation must have: missing, on the frontmatter's first line, or
    /// wrong, on its own line, each with what to write.
    #[test]
    fn an_attested_computation_needs_its_keys() {
        let missing = [
            (
                "runtime: webassembly\n",
                "line 1: the frontmatter has no `runtime`; lengua runs an Attested Computation as \
                 WebAssembly, so add `runtime: webassembly`",
            ),
            (
                "computation: main.wat\n",
                "line 1: the frontmatter has no `computation`; the computation is main.wat, the \
                 code file beside idea.md, so add `computation: main.wat`",
            ),
            (
                "executor:\n  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md\n  receipt: [stdout, stderr, exit]\n",
                "line 1: the frontmatter has no `executor`; lengua's `run` runs the computation \
                 and returns a receipt, so add `executor: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md, receipt: \
                 [stdout, stderr, exit] }`",
            ),
            (
                "attester:\n  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md\n",
                "line 1: the frontmatter has no `attester`; lengua's `check` checks each run, so \
                 add `attester: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md }`",
            ),
        ];
        for (lines, problem) in missing {
            assert_eq!(problems(&attested(lines, "")), [problem]);
        }
        let wrong = [
            (
                "runtime: webassembly",
                "runtime: python",
                "line 5: the frontmatter says `runtime: python`; lengua runs an Attested \
                 Computation as WebAssembly, so write `runtime: webassembly`",
            ),
            (
                "runtime: webassembly",
                "runtime:",
                "line 5: `runtime` is empty; lengua runs an Attested Computation as WebAssembly, \
                 so write `runtime: webassembly`",
            ),
            (
                "computation: main.wat",
                "computation: add.wat",
                "line 6: the frontmatter says `computation: add.wat`; the computation is \
                 main.wat, the code file beside idea.md, so write `computation: main.wat`",
            ),
            (
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md",
                "  resource: python run.py",
                "line 11: `executor` says `resource: python run.py`; lengua's `run` runs the \
                 computation, so write `resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md`",
            ),
            (
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md\n",
                "",
                "line 10: `executor` has no `resource`; lengua's `run` runs the computation, so \
                 add `resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md`",
            ),
            (
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md",
                "  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md",
                "line 14: `attester` says `resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md`; lengua's `check` checks each \
                 run, so write `resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md`",
            ),
            (
                "executor:\n  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md\n  receipt: [stdout, stderr, exit]",
                "executor: lengua run",
                "line 10: the frontmatter says `executor: lengua run`; lengua's `run` runs the \
                 computation and returns a receipt, so write `executor: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md, \
                 receipt: [stdout, stderr, exit] }`",
            ),
            (
                "attester:\n  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md",
                "attester: lengua check",
                "line 13: the frontmatter says `attester: lengua check`; lengua's `check` checks \
                 each run, so write `attester: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md }`",
            ),
        ];
        for (from, to, problem) in wrong {
            assert_eq!(problems(&attested(from, to)), [problem], "{to}");
        }
    }

    /// An executor's receipt lists what lengua's `run` returns, each once.
    #[test]
    fn a_receipt_lists_what_a_run_returns() {
        let receipt = |list: &str| attested("[stdout, stderr, exit]", list);
        let usage = "it lists what each run returns for the attester to check, from stdout, \
                     stderr, exit and files: write `receipt: [stdout, stderr, exit]`";
        assert_eq!(
            problems(&attested("  receipt: [stdout, stderr, exit]\n", "")),
            [format!("line 10: `executor` has no `receipt`; {usage}")]
        );
        for written in ["[]", "stdout"] {
            assert_eq!(
                problems(&receipt(written)),
                [format!(
                    "line 12: `executor` says `receipt: {written}`; {usage}"
                )]
            );
        }
        assert_eq!(
            problems(&receipt("[stdout, job_id]")),
            [
                "line 12: `receipt` names `job_id`, which lengua's `run` doesn't return: use \
                 stdout, stderr, exit and files"
            ]
        );
        assert_eq!(
            problems(&receipt("[stdout, exit, stdout]")),
            ["line 12: `receipt` names `stdout` twice; keep it once"]
        );
        let every = parse(&receipt("[files, exit, stderr, stdout]")).expect("valid idea");
        assert_eq!(
            every.attested.map(|attested| attested.receipt),
            Some(vec![
                Receipt::Files,
                Receipt::Exit,
                Receipt::Stderr,
                Receipt::Stdout
            ])
        );
    }

    /// Each rule of `parameters`, on the line of the parameter it's about, or of its key.
    #[test]
    fn parameters_follow_their_rules() {
        let types = "`type: integer`, `type: decimal`, `type: text`, `type: bytes`";
        let nothing = "- Uses: nothing";
        let one = |a: &str| problems(&with_parameters(a, B, nothing));
        assert_eq!(
            one(&A.replace("name: a", "name: A1")),
            [
                "line 8: the parameter name `A1` isn't a lowercase letter and then lowercase \
                 letters, digits and `_`: write it like `count` or `max_lines`"
            ]
        );
        assert_eq!(
            one(&A.replace("name: a, ", "")),
            [
                "line 8: a parameter has no `name`; give it one in lowercase letters, digits and \
                 `_`, such as `name: count`"
            ]
        );
        let twice = with_parameters(A, &B.replace("name: b", "name: a"), nothing);
        assert_eq!(
            problems(&twice),
            [
                "line 9: the parameter `a` is named twice (also on line 8); give each parameter \
                 its own name"
            ]
        );
        let float = format!(
            "line 8: the parameter `a` says `type: float`; lengua's types are integer, decimal, \
             text and bytes: write one of {types}"
        );
        assert_eq!(
            one(&A.replace("type: integer", "type: float")),
            [float.as_str()]
        );
        // A name is its own whatever else is wrong with the parameter that has it first.
        let both = with_parameters(
            &A.replace("type: integer", "type: float"),
            &B.replace("name: b", "name: a"),
            nothing,
        );
        assert_eq!(
            problems(&both),
            [
                float.as_str(),
                "line 9: the parameter `a` is named twice (also on line 8); give each parameter \
                 its own name"
            ]
        );
        assert_eq!(
            one(&A.replace("type: integer, ", "")),
            [format!(
                "line 8: the parameter `a` has no `type`; add one of {types}"
            )]
        );
        // Only `true` and `false` are booleans, so YAML's older words for them are refused.
        for required in ["maybe", "yes"] {
            assert_eq!(
                one(&A.replace("required: true", &format!("required: {required}"))),
                [format!(
                    "line 8: the parameter `a` says `required: {required}`; write `required: \
                     true` or `required: false`, or leave it out, which means true"
                )]
            );
        }
        // A boolean in quotes is text, and the problem shows the quotes.
        assert_eq!(
            one(&A.replace("required: true", "required: \"true\"")),
            [
                "line 8: the parameter `a` says `required: \"true\"`; write `required: true` or \
                 `required: false` without quotes, or leave it out, which means true"
            ]
        );
        let optional = parse(&with_parameters(
            &A.replace("required: true", "required: false"),
            &B.replace("required: true, ", ""),
            nothing,
        ))
        .expect("valid idea");
        let required: Vec<bool> = optional
            .attested
            .iter()
            .flat_map(|attested| &attested.parameters)
            .map(|parameter| parameter.required)
            .collect();
        assert_eq!(required, [false, true], "true when it's left out");
        // Each type, and names YAML could read as something else.
        for (kind, name) in [("decimal", "n"), ("text", "y"), ("bytes", "no_2")] {
            let a = A
                .replace("integer", kind)
                .replace("name: a", &format!("name: {name}"));
            let idea = parse(&with_parameters(&a, B, nothing)).expect("valid idea");
            let first = idea
                .attested
                .and_then(|attested| attested.parameters.into_iter().next())
                .expect("a parameter");
            assert_eq!((first.name.as_str(), first.kind.name()), (name, kind));
        }
    }

    /// Where a parameter's value comes from: each argument and each other source gives one
    /// parameter, the arguments numbered from 1 without gaps, and each source but an argument is
    /// one the idea's `- Uses:` line names.
    #[test]
    fn each_parameter_comes_from_one_source() {
        let nothing = "- Uses: nothing";
        let from = |text: &str| A.replace("argument 1", text);
        // `a` from the source `text`, with what it uses, and `b` from the first argument.
        let sources = |text: &str, uses: &str| {
            let b = B.replace("argument 2", "argument 1");
            let idea = parse(&with_parameters(&from(text), &b, uses))
                .unwrap_or_else(|problems| panic!("{text}: {problems:#?}"));
            idea.attested
                .map(|attested| attested.parameters[0].from.clone())
                .expect("a parameter")
        };
        assert_eq!(
            sources("standard input", "- Uses: standard input"),
            Source::StandardInput
        );
        assert_eq!(
            sources("environment HOME_2", "- Uses: the environment"),
            Source::Environment("HOME_2".to_owned())
        );
        assert_eq!(
            sources("file data/input.txt", "- Uses: files"),
            Source::File("data/input.txt".to_owned())
        );
        let first = |text: &str| problems(&with_parameters(&from(text), B, nothing));
        assert_problem(
            &with_parameters(&A.replace(", from: argument 1", ""), B, nothing),
            "line 8: the parameter `a` has no `from`; say where its value comes from: `from: \
             argument 1`, `from: standard input`, `from: environment NAME` or `from: file NAME`",
        );
        let refused = [
            (
                "stdin",
                "write `from: argument 1`, `from: standard input`, `from: environment NAME` or \
                 `from: file NAME`",
            ),
            (
                "argument 0",
                "arguments are numbered from 1, as in `from: argument 1`",
            ),
            (
                "environment home",
                "an environment variable's name is capitals, digits and `_`, as in `from: \
                 environment HOME`",
            ),
            ("file ../input.txt", "`..` leaves the working folder"),
            (
                "file /tmp/input.txt",
                "a path from the root leaves the working folder",
            ),
            (
                "file aux.txt",
                "`aux.txt`: Windows reserves AUX, with any extension, in every folder",
            ),
        ];
        for (text, fix) in refused {
            assert_problem(
                &with_parameters(&from(text), B, nothing),
                &format!("line 8: the parameter `a` says `from: {text}`; {fix}"),
            );
        }
        // One parameter a source, the arguments from 1 without gaps.
        let second = B.replace("argument 2", "argument 1");
        assert_eq!(
            problems(&with_parameters(A, &second, nothing)),
            [
                "line 9: the parameter `b` comes from `argument 1`, as the parameter `a` does \
                 (line 8); give each parameter a source of its own"
            ]
        );
        let third = B.replace("argument 2", "argument 3");
        assert_eq!(
            problems(&with_parameters(A, &third, nothing)),
            [
                "line 9: the parameter `b` comes from argument 3, but no parameter comes from \
                 argument 2: number the arguments from 1 without gaps"
            ]
        );
        let input = from("standard input");
        let more = B.replace("argument 2", "standard input");
        assert_eq!(
            problems(&with_parameters(&input, &more, "- Uses: standard input")),
            [
                "line 9: the parameter `b` comes from standard input, as the parameter `a` does \
                 (line 8); a program has one standard input, so give it to one parameter"
            ]
        );
        // A source other than an argument is one the idea's `- Uses:` line names.
        assert_eq!(
            first("standard input"),
            [
                "line 9: the parameter `b` comes from argument 2, but no parameter comes from \
                 argument 1: number the arguments from 1 without gaps",
                "line 8: the parameter `a` comes from `standard input`, which the idea doesn't \
                 allow: add `standard input` to the `- Uses:` line of `# Constraints`, as in `- \
                 Uses: standard input`"
            ]
        );
        for (text, uses) in [
            ("environment HOME", "the environment"),
            ("file input.txt", "files"),
        ] {
            assert_problem(
                &with_parameters(&from(text), &second, nothing),
                &format!("which the idea doesn't allow: add `{uses}` to the `- Uses:` line"),
            );
        }
    }

    /// A parameter written as a block mapping has its keys on lines of their own, and each
    /// problem goes on the line of the key it's about.
    #[test]
    fn a_block_parameter_s_problems_are_on_their_lines() {
        let block = attested(
            &format!("  - {A}\n  - {B}\n"),
            "  - name: a\n    type: integer\n    from: argument 1\n  - name: b\n    type: \
             float\n    from: argument 2\n",
        );
        assert_eq!(
            problems(&block),
            [
                "line 12: the parameter `b` says `type: float`; lengua's types are integer, \
                 decimal, text and bytes: write one of `type: integer`, `type: decimal`, `type: \
                 text`, `type: bytes`"
            ]
        );
        let idea = parse(&block.replace("type: float", "type: decimal")).expect("valid idea");
        let lines: Vec<usize> = idea
            .attested
            .iter()
            .flat_map(|attested| &attested.parameters)
            .map(|parameter| parameter.line)
            .collect();
        assert_eq!(lines, [8, 11]);
        // Not a list, or an entry that isn't a mapping.
        let listed = attested(
            &format!("parameters:\n  - {A}\n  - {B}\n"),
            "parameters: a\n",
        );
        assert_eq!(
            problems(&listed),
            [format!(
                "line 7: the frontmatter says `parameters: a`; `parameters` is a list, a mapping \
                 for each parameter, such as `- {A}`"
            )]
        );
        assert_problem(
            &with_parameters("a", B, "- Uses: nothing"),
            &format!("line 8: each parameter is a mapping, such as `{A}`, not `a`"),
        );
    }

    /// OKF's other keys, and any key lengua doesn't know, are kept and never refused; a `status`
    /// is one of OKF's three.
    #[test]
    fn okf_s_other_keys_are_kept() {
        let others = "status: stable\ntags: [math, numbers]\nsources:\n  - { id: sum, resource: \
                      https://example.com/sum }\ngenerated: { by: agent/1.0, at: \
                      2026-11-06T22:00:00Z }\nverified:\n  - { by: human:ana, at: \
                      2026-11-06T23:00:00Z }\nstale_after: 2027-01-01T00:00:00Z\nowner: the \
                      numbers team\n";
        let with_others = attested("attester:\n", &format!("{others}attester:\n"));
        let idea = parse(&with_others).unwrap_or_else(|problems| panic!("{problems:#?}"));
        assert!(idea.attested.is_some());
        for status in ["draft", "deprecated"] {
            let other = with_others.replace("status: stable", &format!("status: {status}"));
            assert!(parse(&other).is_ok(), "{status}");
        }
        assert_eq!(
            problems(&with_others.replace("status: stable", "status: approved")),
            [
                "line 13: the frontmatter says `status: approved`; OKF's statuses are draft, \
                 stable and deprecated: write one of them, or leave `status` out, which means \
                 stable"
            ]
        );
        // The native form keeps them too, and reads none of them.
        let native = ADD.replace("title: add\n", &format!("title: add\n{others}"));
        assert!(parse(&native).is_ok());
    }

    /// An idea of the native lengua has none of an Attested Computation's own keys, and each is
    /// refused with the type that has it.
    #[test]
    fn a_native_idea_has_no_computation_keys() {
        let native = ADD.replace("title: add\n", "title: add\nruntime: webassembly\n");
        assert_eq!(
            problems(&native),
            [
                "line 4: `runtime` is a key of an Attested Computation, and this idea is `type: \
                 lengua/program-idea`, the native lengua's: write `type: Attested Computation`, \
                 the type of an idea that lengua compiles to main.wat"
            ]
        );
        for key in [
            "computation: main.wat",
            "executor: { resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md }",
            "attester:",
        ] {
            let text = ADD.replace("title: add\n", &format!("title: add\n{key}\n"));
            assert_problem(&text, "write `type: Attested Computation`");
        }
        assert_problem(
            &native.replace(IDEA_TYPE, LEGACY_IDEA_TYPE),
            "this idea is `type: lengua/program-spec`",
        );
        // A type lengua doesn't know is refused on its own line.
        assert_problem(
            &ATTESTED.replace(ATTESTED_TYPE, "Metric"),
            "line 2: the frontmatter says `type: Metric`; a lengua idea is `type: Attested \
             Computation`",
        );
    }

    /// A `- Secret:` line names the inputs whose values must not decide how the program runs,
    /// several on one line; each needs the `- Uses:` word that lets the program read it, but an
    /// argument; and each problem is on the line, with what to write.
    #[test]
    fn a_secret_line_names_the_inputs_that_decide_nothing() {
        let with = |uses: &str, secret: &str| {
            ATTESTED.replace("- Uses: nothing", &format!("{uses}\n{secret}"))
        };
        let all = with(
            "- Uses: standard input, files and the environment",
            "- Secret: standard input, a file `keys/key.txt`, the environment variable `TOKEN` \
             and argument `2`.",
        );
        let idea = parse(&all).unwrap_or_else(|problems| panic!("{problems:#?}"));
        assert_eq!(
            idea.secrets,
            [
                Source::StandardInput,
                Source::File("keys/key.txt".to_owned()),
                Source::Environment("TOKEN".to_owned()),
                Source::Argument(2),
            ]
        );
        assert_eq!(idea.secrets[1].secret_words(), "a file `keys/key.txt`");
        assert!(parse(ATTESTED).expect("the add idea").secrets.is_empty());
        // An argument needs no `- Uses:` word, and the terminal reads standard input too.
        for (uses, secret) in [
            ("- Uses: nothing", "- Secret: argument `1`"),
            ("- Uses: the terminal", "- Secret: standard input"),
        ] {
            let idea = parse(&with(uses, secret))
                .unwrap_or_else(|problems| panic!("{secret}: {problems:#?}"));
            assert_eq!(idea.secrets.len(), 1, "{secret}");
        }
        let refused = [
            (
                "- Uses: nothing",
                "- Secret: standard input",
                "standard input is secret, but the `- Uses:` line does not name standard input. \
                 Add `standard input` to the `- Uses:` line",
            ),
            (
                "- Uses: nothing",
                "- Secret: a file `key.txt`",
                "a file `key.txt` is secret, but the `- Uses:` line does not name files",
            ),
            (
                "- Uses: nothing",
                "- Secret: the environment variable `KEY`",
                "the environment variable `KEY` is secret, but the `- Uses:` line does not name \
                 the environment",
            ),
            (
                "- Uses: nothing",
                "- Secret: the clock",
                "`the clock` is not an input that can be secret. Name standard input",
            ),
            (
                "- Uses: files",
                "- Secret: a file `../key.txt`",
                "the secret file `../key.txt` is not a file in the working folder: `..` leaves \
                 the working folder",
            ),
            (
                "- Uses: the environment",
                "- Secret: the environment variable `key`",
                "`key` is not the name of an environment variable",
            ),
            (
                "- Uses: nothing",
                "- Secret: argument `0`",
                "`0` is not the place of an argument",
            ),
            (
                "- Uses: nothing",
                "- Secret: argument `1` and argument `1`",
                "argument `1` is secret twice on this line",
            ),
            ("- Uses: nothing", "- Secret:", "`- Secret:` names no input"),
        ];
        for (uses, secret, needle) in refused {
            let text = with(uses, secret);
            let line = text.lines().count();
            assert_problem(&text, &format!("line {line}: {needle}"));
        }
        // One line holds them all.
        let twice = with(
            "- Uses: nothing",
            "- Secret: argument `1`\n- Secret: argument `2`",
        );
        let line = twice.lines().count();
        assert_problem(
            &twice,
            &format!(
                "line {line}: the secrets are on two lines (also on line {})",
                line - 1
            ),
        );
        // An idea of the native lengua has no secret: only main.wat's checker follows one.
        let native = ADD.replace("- Uses: nothing", "- Uses: nothing\n- Secret: argument `1`");
        assert_problem(
            &native,
            "`- Secret:` is for an idea that lengua compiles to main.wat",
        );
    }

    /// The ideas of the programs the repository ships, in examples/ and tests/fixtures, each by
    /// its path.
    fn shipped_ideas() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut ideas = Vec::new();
        for place in ["examples", "tests/fixtures"] {
            for entry in std::fs::read_dir(root.join(place)).expect("a folder of programs") {
                let path = entry.expect("an entry").path().join("idea.md");
                if let Ok(text) = std::fs::read_to_string(&path) {
                    ideas.push((path.display().to_string(), text));
                }
            }
        }
        ideas.sort();
        ideas
    }

    /// A run's grants are what check's parse finds: for every idea the repository ships that
    /// parses, grants gives exactly its uses and its network, and secrets gives its secrets,
    /// which one of them, secret-equal, has. Only what the idea's tests stand in for, which
    /// parse reads from its behaviors and one of them, follow, has, is no part of a run's
    /// grants: every line of `# Constraints` is, each of follow's among them.
    #[test]
    fn grants_are_the_uses_and_the_network_parse_finds() {
        let mut compared = 0;
        let mut secret = 0;
        let mut stood = 0;
        for (name, text) in shipped_ideas() {
            let Ok(idea) = parse(&text) else {
                continue;
            };
            stood += usize::from(!idea.network.stands.is_empty());
            let granted = Network {
                stands: Vec::new(),
                names: Vec::new(),
                programs: Vec::new(),
                ..idea.network
            };
            assert_eq!(grants(&text), Ok((idea.uses, granted)), "{name}");
            secret += usize::from(!idea.secrets.is_empty());
            assert_eq!(secrets(&text), Ok(idea.secrets), "{name}");
            compared += 1;
        }
        assert!(compared >= 43, "only {compared} ideas parsed");
        assert!(secret >= 1, "no shipped idea names a secret");
        assert!(
            stood >= 1,
            "no shipped idea's tests stand in for a computer"
        );
    }

    /// grants takes a `- Uses:` line only where parse does, in `# Constraints` outside code
    /// blocks: the same line in a code block of `# Constraints`, in `# Purpose`, under a `###`
    /// heading of `# Behaviors` or inside `verified:` in the frontmatter grants nothing.
    #[test]
    fn grants_take_only_the_lines_parse_takes() {
        let files = |text: &str| parse(text).is_ok_and(|idea| idea.uses.contains(&Use::Files));
        let granted = |text: &str| grants(text).map(|(uses, _)| uses);
        let constraints = ATTESTED.replace("- Uses: nothing", "- Uses: files");
        assert_eq!(granted(&constraints), Ok(vec![Use::Files]));
        assert!(files(&constraints));
        let places = [
            (
                "- Uses: nothing",
                "- Uses: nothing\n\n```text\n- Uses: files\n```",
            ),
            ("# Behaviors\n", "- Uses: files\n\n# Behaviors\n"),
            (
                "### For any `a` and `b`\n",
                "### For any `a` and `b`\n- Uses: files\n",
            ),
            (
                "type: Attested Computation\n",
                "type: Attested Computation\nverified:\n- Uses: files\n",
            ),
        ];
        for (from, to) in places {
            let text = ATTESTED.replace(from, to);
            assert_ne!(text, ATTESTED, "{from:?}");
            assert_eq!(granted(&text), Ok(Vec::new()), "{to:?}");
            assert!(!files(&text), "{to:?}");
        }
    }

    /// grants refuses what parse refuses in `# Constraints`, with parse's own problems: two
    /// `- Uses:` lines, and a use lengua doesn't know.
    #[test]
    fn grants_refuse_what_parse_refuses_in_constraints() {
        for to in ["- Uses: nothing\n- Uses: files", "- Uses: telepathy"] {
            let text = ATTESTED.replace("- Uses: nothing", to);
            let refused = grants(&text).expect_err("grants refuses it");
            let parsed = parse(&text).expect_err("parse refuses it");
            assert!(!refused.is_empty(), "{to:?}");
            for problem in &refused {
                assert!(
                    parsed.contains(problem),
                    "{problem}, not parse's: {parsed:?}"
                );
            }
        }
    }
}
