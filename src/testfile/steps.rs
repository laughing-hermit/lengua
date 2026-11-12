//! A behavior's Given/When/Then steps as its tests. Its own steps are its example, with literal
//! values. The steps under its `###` heading are its generated tests: the same phrasings, whose
//! values are templates over the variables of the table below them, one row a generated test.
//! lengua reads steps in a fixed vocabulary, as Gherkin's step definitions do, so the idea's own
//! words are the tests and nothing is written twice. The vocabulary is in ASD-STE100 Simplified
//! Technical English, as the rest of an idea is: a step in one of lengua's old phrasings is
//! refused, with the STE phrasing that replaces it (`crate::ste`).
//!
//! Given:
//! - ``the arguments `a` and `b` ``, ``the argument `a` ``, `no arguments`
//! - ``standard input that contains TEXT``, `no standard input` or `empty standard input`
//! - ``a file `name` that contains TEXT``, ``an empty file `name` ``, ``no file `name` ``
//! - ``the environment variable `NAME` with the value TEXT``
//! - ``an empty folder `name` ``
//! - ``the clock at `seconds` ``, with ``then `seconds` `` for later readings
//! - ``the random bytes `fc` and `2a` ``
//! - ``a server on port `8080` that receives TEXT and replies TEXT``, with ``at `example.com` ``
//!   before `on port` for a computer the idea names, ``no server on port `8080` ``
//! - for a program that opens connections by itself, the same server step with its computer
//!   and port, one step a connection it opens there, in order; ``a computer at `192.0.2.53` on
//!   UDP port `53` that receives TEXT and replies TEXT`` for a datagram each way; and ``the
//!   addresses `192.0.2.7` and `2001:db8::1` for the name `example.org` `` for what a look at a
//!   name's addresses gives
//! - ``a client that sends TEXT``, and `the clients connect at once` for clients that overlap
//! - `standard output is closed`, a pipe nobody reads, so every write to it fails with `EPIPE`
//! - `the program is a piece`, for an idea with a `- Splits:` line: the run is a piece of the
//!   program's split work, with standard input as its bytes and what it prints as its answer
//!
//! When: `the program runs`.
//!
//! Then:
//! - ``it prints TEXT``, with `to standard error` for standard error, and `it prints nothing`
//! - ``it prints text that matches `rule` `` for output the program chooses, such as
//!   `` `rolled {any(1, 6)}` ``
//! - ``the file `name` contains TEXT``
//! - ``there is no file `name` ``, ``there is no folder `name` ``, ``there is a folder `name` ``
//! - ``the client receives TEXT``
//! - ``it exits with code `0` ``, `it does not stop`
//!
//! TEXT is a value in backticks, written with `\n`, `\r`, `\t`, `\e` and `\\` for the characters
//! they name; `and a newline` adds one, `` `N` times`` after a value repeats it, and `then` joins
//! parts. In a generated test, `{…}` holds a formula, whose own strings keep their escapes, and a
//! repeat is of fixed text, never of a placeholder.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ClientTable, ExampleTable, OutlineRow, OutlineSteps, Problem, Repeat, Rule, ServerTable, Text,
    TextPart, Written, unescape,
};
use crate::idea::{Behavior, Endpoint, Idea, Keyword, Outline, Step, Token};
use crate::reach::{Grants, Stand};

/// The column of a table that gives its row's number of runs, not a variable.
const RUNS: &str = "runs";

/// The phrasings lengua reads, for a step it can't.
const GIVEN_PHRASES: &str = "`Given the arguments `…` and `…``, `Given no arguments`, `Given \
     standard input that contains `…``, `Given a file `name` that contains `…``, `Given no file \
     `name``, `Given the environment variable `NAME` with the value `…``, `Given an empty folder \
     `name``, `Given the clock at `…``, `Given the random bytes `…``, `Given a server on port \
     `…` that receives `…` and replies `…``, `Given a server at `example.org` on port `…` that \
     receives `…` and replies `…``, `Given a computer at `192.0.2.53` on UDP port `…` that \
     receives `…` and replies `…``, `Given the addresses `192.0.2.7` for the name \
     `example.org``, `Given the program `hostname` prints `…` and exits with code `0``, `Given \
     a client that sends `…``, `And the clients connect at once`, `And \
     standard output is closed`, `And the program is a piece`";
const THEN_PHRASES: &str = "`Then it prints `…``, `Then it prints `…` to standard error`, \
     `Then it prints nothing`, `Then it prints text that matches `rule``, `Then the file `name` \
     contains `…``, `Then there is no file `name``, `Then there is a folder `name``, `Then the \
     client receives `…``, `And it exits with code `N``, `And it does not stop`";

/// How a step's values read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    /// An example's: literal text.
    #[default]
    Example,
    /// A generated test's: templates over its table's variables.
    Generated,
}

/// The example `behavior`'s steps describe.
///
/// # Errors
///
/// Each step lengua can't read, with the phrasing to write instead.
pub(super) fn example(behavior: &Behavior, idea: &Idea) -> Result<ExampleTable, Vec<Problem>> {
    read(&behavior.steps, behavior, Mode::Example, idea).map(Reading::example)
}

/// The generated tests `outline` describes: its steps as templates, and its table's rows.
///
/// # Errors
///
/// Each step lengua can't read, with the phrasing to write instead, and each column named twice.
pub(super) fn generated(
    behavior: &Behavior,
    outline: &Outline,
    idea: &Idea,
) -> Result<(OutlineSteps, Vec<OutlineRow>), Vec<Problem>> {
    let mut problems = Vec::new();
    let reading = read(&outline.steps, behavior, Mode::Generated, idea)
        .map_err(|found| problems.extend(found))
        .ok();
    let mut seen = BTreeSet::new();
    for column in &outline.columns {
        if !seen.insert(column) {
            problems.push(Problem {
                line: Some(outline.columns_line),
                message: format!(
                    "{}'s table names `{column}` twice; give each variable one column",
                    behavior.id
                ),
            });
        }
    }
    match reading {
        Some(reading) if problems.is_empty() => Ok((reading.outline(outline.line), rows(outline))),
        _ => Err(problems),
    }
}

/// Each row of `outline`'s table: its variables' generators, and its runs when it gives them.
fn rows(outline: &Outline) -> Vec<OutlineRow> {
    outline
        .rows
        .iter()
        .map(|row| {
            let mut runs = None;
            let mut vars = Vec::new();
            for (column, cell) in outline.columns.iter().zip(&row.cells) {
                if column == RUNS {
                    runs = (!cell.is_empty()).then(|| cell.clone());
                } else {
                    vars.push((column.clone(), cell.clone()));
                }
            }
            OutlineRow {
                line: row.line,
                runs,
                vars,
            }
        })
        .collect()
}

/// Reads `steps` in `mode`, reporting each step it can't read with its rewrite, and each of
/// lengua's old phrasings with the STE phrasing that replaces it.
fn read(
    steps: &[Step],
    behavior: &Behavior,
    mode: Mode,
    idea: &Idea,
) -> Result<Reading, Vec<Problem>> {
    let whose = match mode {
        Mode::Example => behavior.id.to_string(),
        Mode::Generated => format!("{}'s generated tests", behavior.id),
    };
    let mut reading = Reading {
        mode,
        places: idea.network.connections.clone(),
        stands: idea.network.stands.clone(),
        reach: idea.network.reach.clone(),
        ..Reading::default()
    };
    let mut problems = Vec::new();
    let mut phase = Keyword::Given;
    for step in steps {
        phase = match step.keyword {
            Keyword::And | Keyword::But => phase,
            keyword => keyword,
        };
        if step
            .values
            .iter()
            .any(|value| crate::expect::has_holes(value))
        {
            problems.push(Problem {
                line: Some(step.line),
                message: format!(
                    "{whose}: this step holds one of the characters U+FDD0, U+FDD1 and U+FDD2, \
                     which lengua reserves for marking rules such as {{any(1, 6)}}; remove them"
                ),
            });
            continue;
        }
        let tokens = step.tokens();
        let old = crate::ste::old_forms(&tokens);
        if !old.is_empty() {
            // STE refuses the old phrasing, and says the one lengua reads instead.
            for form in old {
                problems.push(Problem {
                    line: Some(step.line),
                    message: format!("STE: {whose}: {}", form.fix()),
                });
            }
            continue;
        }
        reading.line = step.line;
        let read = match phase {
            Keyword::When => when(&tokens),
            Keyword::Then => reading.output(&tokens),
            _ => reading.input(&tokens),
        };
        if read.is_none() {
            let rewrite = if mode == Mode::Generated && repeats(&tokens) {
                format!(
                    "a value drawn from the table: a generated test's text comes from its \
                     variables, and a value written `` `N` times`` repeats fixed text of at most \
                     {} bytes, never a placeholder",
                    super::MAX_FILE_BYTES
                )
            } else {
                rewrite(step, phase)
            };
            problems.push(Problem {
                line: Some(step.line),
                message: format!(
                    "{whose}: lengua reads each step as one of its phrasings; write this one as \
                     {rewrite}"
                ),
            });
        }
    }
    for (line, problem) in std::mem::take(&mut reading.found) {
        problems.push(Problem {
            line: Some(line),
            message: format!("{whose}: {problem}"),
        });
    }
    if reading.client.len() != reading.answers.len() {
        problems.push(Problem {
            line: steps.first().map(|step| step.line),
            message: format!(
                "{whose}: each `a client that sends …` needs its `the client receives …`, in the \
                 same order"
            ),
        });
    }
    if problems.is_empty() {
        Ok(reading)
    } else {
        Err(problems)
    }
}

/// What a behavior's steps have said so far.
#[derive(Default)]
struct Reading {
    mode: Mode,
    /// The step being read.
    line: usize,
    args: Vec<Text>,
    stdin: Option<Text>,
    files: BTreeMap<String, Text>,
    folders: BTreeSet<String>,
    env: BTreeMap<String, Text>,
    stdout: Vec<Text>,
    stderr: Vec<Text>,
    files_after: BTreeMap<String, Text>,
    gone: BTreeSet<String>,
    folders_after: BTreeSet<String>,
    random: Option<String>,
    clock: Vec<String>,
    server: Vec<ServerTable<Text>>,
    client: Vec<Text>,
    clients_at_once: bool,
    /// Whether its standard output is closed, so every write to it fails.
    stdout_closed: bool,
    /// Whether the run is a piece of the program's split work.
    piece: bool,
    answers: Vec<Text>,
    /// A generated test's exit code as written: a number or a formula.
    exit_code: Option<String>,
    serves: bool,
    /// The line of the step that first wrote each field, for problems in generated tests.
    lines: BTreeMap<String, usize>,
    /// The connections the idea names, which the test's servers stand in for.
    places: Vec<Endpoint>,
    /// The computers the idea's tests stand in for, of those the program reaches by itself: a
    /// server for one of them is numbered after the idea's own connections.
    stands: Vec<Stand>,
    /// What the idea grants the program at run time.
    reach: Grants,
    /// What's wrong with steps that read well, with each one's line.
    found: Vec<(usize, String)>,
}

impl Reading {
    /// Notes that the step being read wrote `field`.
    fn wrote(&mut self, field: String) {
        self.lines.entry(field).or_insert(self.line);
    }

    /// A Given step: what the program is given.
    fn input(&mut self, tokens: &[Token]) -> Option<()> {
        let mode = self.mode;
        let mut at = Cursor::new(tokens);
        if at.words(&["the", "clients", "connect", "at", "once"]) {
            self.wrote("clients_at_once".into());
            self.clients_at_once = true;
            return at.end();
        }
        // `standard output is closed`: a pipe nobody reads, as after `prog | head -1` has read
        // its line, so every write to it fails with `EPIPE`, never a signal.
        if at.words(&["standard", "output", "is", "closed"]) {
            self.wrote("stdout_closed".into());
            self.stdout_closed = true;
            return at.end();
        }
        // `the program is a piece`: the run is a piece of the program's split work, as a call
        // that splits runs one, with its bytes as its standard input.
        if at.words(&["the", "program", "is", "a", "piece"]) {
            self.wrote("piece".into());
            self.piece = true;
            if self.reach.split.is_none() {
                self.found.push((
                    self.line,
                    "idea.md grants no split of the program's work: a test runs a piece only \
                     for an idea with a `- Splits:` line"
                        .to_owned(),
                ));
            }
            return at.end();
        }
        if at.words(&["no", "arguments"])
            || at.words(&["no", "standard", "input"])
            || at.words(&["empty", "standard", "input"])
        {
            return at.end();
        }
        if at.words(&["no", "file"]) {
            at.word("named");
            at.value()?;
            return at.end();
        }
        if at.words(&["no", "server"]) {
            // The connection to the place it names is handed over already ended; a computer the
            // program reaches by itself has nothing there unless a step gives it a server.
            let host = at.is("at").then(|| at.value()).flatten();
            let port = at.words(&["on", "port"]).then(|| at.value()).flatten();
            let opened = host
                .as_deref()
                .zip(port.as_deref())
                .is_some_and(|(host, port)| self.opens(host, port));
            if !opened && (port.is_some() || host.is_some()) {
                self.connection(host.as_deref(), port.as_deref());
            }
            return Some(());
        }
        // ``a computer at `192.0.2.53` on UDP port `53` that receives TEXT and replies TEXT``:
        // the test's own computer for the datagrams the program sends there, one step a
        // datagram each way.
        if at.words(&["a", "computer", "at"]) {
            let host = at.value()?;
            at.words(&["on", "udp", "port"]).then_some(())?;
            let port = at.value()?;
            at.word("that")?;
            at.word("receives")?;
            let receives = text(&mut at, mode)?;
            at.word("and")?;
            at.word("replies")?;
            let replies = text(&mut at, mode)?;
            let Some(index) = self.stand(true, &host, &port) else {
                self.found.push((
                    self.line,
                    format!(
                        "idea.md grants no UDP datagrams to port {port} on {host}: the test's \
                         computer stands in for one that `- Network: sends UDP datagrams to …` \
                         names"
                    ),
                ));
                return at.end();
            };
            self.wrote(format!("server.{}", self.server.len()));
            self.server.push(ServerTable {
                connection: self.places.len() + index,
                receives,
                replies,
            });
            return at.end();
        }
        if at.is("the") {
            at.is("single");
            if at.is("arguments") || at.is("argument") {
                self.wrote("args".into());
                loop {
                    self.args.push(Text::Plain(mode.value(&at.value()?)));
                    at.word("and");
                    if at.is_end() {
                        return Some(());
                    }
                }
            }
            if at.words(&["environment", "variable"]) {
                let name = at.value()?;
                at.word("with")?;
                at.word("the")?;
                at.word("value")?;
                self.wrote(format!("env.{name}"));
                self.env.insert(name, text(&mut at, mode)?);
                return at.end();
            }
            if at.words(&["clock", "at"]) {
                self.wrote("clock".into());
                self.clock.push(at.value()?);
                while !at.is_end() {
                    at.word("and");
                    at.word("then");
                    at.word("at");
                    self.clock.push(at.value()?);
                }
                return Some(());
            }
            if at.words(&["random", "bytes"]) {
                let mut hex = String::new();
                while !at.is_end() {
                    at.word("and");
                    hex.push_str(&at.value()?);
                }
                (!hex.is_empty()).then_some(())?;
                self.wrote("random".into());
                self.random = Some(hex);
                return Some(());
            }
            // ``the program `hostname` prints `box` and exits with code `0` ``: what a command
            // the program starts gives, in every test of the idea. A test starts nothing.
            if at.is("program") {
                let stood = crate::idea::program_stood(tokens)?;
                let arguments: Vec<&str> = stood.arguments.iter().map(String::as_str).collect();
                let granted = self
                    .reach
                    .runs
                    .iter()
                    .any(|rule| rule.covers(&stood.program, &arguments));
                if !granted {
                    self.found.push((
                        self.line,
                        format!(
                            "idea.md grants no start of `{}` with those arguments: the test says \
                             what a command gives that a `- Runs:` line covers",
                            stood.program
                        ),
                    ));
                }
                return Some(());
            }
            // ``the addresses `192.0.2.7` and `2001:db8::1` for the name `example.org` ``: what
            // a look at the name's addresses gives, in every test of the idea.
            if at.is("addresses") || at.is("address") {
                let mut addresses = 0;
                while !at.words(&["for", "the", "name"]) {
                    at.word("and");
                    at.value()?.parse::<std::net::IpAddr>().ok()?;
                    addresses += 1;
                }
                let name = at.value()?;
                if addresses == 0 {
                    return None;
                }
                let granted = crate::reach::spelled(&name).is_ok_and(|computer| {
                    computer.address.is_none()
                        && Grants::granted(&self.reach.names, &computer, 0, None, None).is_some()
                });
                if !granted {
                    self.found.push((
                        self.line,
                        format!(
                            "idea.md grants no look at the addresses of `{name}`: the test gives \
                             addresses to a name that `- Network: looks up the addresses of …` \
                             names"
                        ),
                    ));
                }
                return at.end();
            }
            return None;
        }
        if at.words(&["standard", "input", "that", "contains"]) {
            self.wrote("stdin".into());
            self.stdin = Some(text(&mut at, mode)?);
            return at.end();
        }
        if at.words(&["an", "empty", "folder"]) {
            at.is("named");
            self.folders.insert(at.value()?);
            return at.end();
        }
        if at.words(&["an", "empty", "file"]) {
            at.is("named");
            let name = at.value()?;
            self.wrote(format!("files.{name}"));
            self.files.insert(name, Text::default());
            return at.end();
        }
        if at.words(&["a", "file"]) {
            at.word("named");
            let name = at.value()?;
            at.word("that")?;
            at.word("contains")?;
            self.wrote(format!("files.{name}"));
            self.files.insert(name, text(&mut at, mode)?);
            return at.end();
        }
        if at.words(&["a", "server"]) {
            let host = if at.is("at") { Some(at.value()?) } else { None };
            let port = if at.words(&["on", "port"]) {
                Some(at.value()?)
            } else {
                None
            };
            at.word("that")?;
            at.word("receives")?;
            let receives = text(&mut at, mode)?;
            at.word("and")?;
            at.word("replies")?;
            let replies = text(&mut at, mode)?;
            let connection = self.connection(host.as_deref(), port.as_deref());
            if self
                .server
                .iter()
                .any(|server| server.connection == connection && connection < self.places.len())
            {
                let place = self.places[connection].describe();
                self.found.push((
                    self.line,
                    format!(
                        "it has two servers for its connection to {place}; a program gets one \
                         connection to each place its idea names, so keep one"
                    ),
                ));
            }
            self.wrote(format!("server.{}", self.server.len()));
            self.server.push(ServerTable {
                connection,
                receives,
                replies,
            });
            return at.end();
        }
        if at.words(&["a", "client", "that", "sends"]) {
            self.wrote(format!("client.{}", self.client.len()));
            self.client.push(text(&mut at, mode)?);
            return at.end();
        }
        None
    }

    /// A Then step: what the program must do.
    fn output(&mut self, tokens: &[Token]) -> Option<()> {
        let mode = self.mode;
        let mut at = Cursor::new(tokens);
        if at.words(&["it", "exits", "with", "code"]) {
            // An example's code is the behavior's own, which the idea reads; a generated test's
            // may be a formula.
            let code = at.value()?;
            if mode == Mode::Example || !code.contains('{') {
                code.parse::<i64>().ok()?;
            }
            self.wrote("exit_code".into());
            self.exit_code = Some(code);
            return at.end();
        }
        if at.words(&["it", "does", "not", "stop"]) {
            self.serves = true;
            return at.end();
        }
        if at.words(&["it", "prints"]) {
            if at.is("nothing") {
                stream(&mut at)?;
                return at.end();
            }
            let printed = if at.words(&["text", "that", "matches"]) {
                rule(&mut at, mode)?
            } else {
                text(&mut at, mode)?
            };
            match stream(&mut at)? {
                Stream::Out if self.stdout_closed => {
                    self.found.push((
                        self.line,
                        "its standard output is closed, so it prints nothing there: say what it \
                         prints to standard error, and its exit code"
                            .into(),
                    ));
                }
                Stream::Out => {
                    self.wrote("stdout".into());
                    self.stdout.push(printed);
                }
                Stream::Error => {
                    self.wrote("stderr".into());
                    self.stderr.push(printed);
                }
            }
            return at.end();
        }
        if at.words(&["the", "client", "receives"]) {
            let answer = if at.words(&["text", "that", "matches"]) {
                rule(&mut at, mode)?
            } else {
                text(&mut at, mode)?
            };
            self.answers.push(answer);
            return at.end();
        }
        if at.words(&["there", "is", "no"]) {
            (at.is("file") || at.is("folder")).then_some(())?;
            self.gone.insert(at.value()?);
            return at.end();
        }
        if at.words(&["there", "is", "a", "folder"]) {
            self.folders_after.insert(at.value()?);
            return at.end();
        }
        at.word("the");
        at.word("file");
        let name = at.value()?;
        at.word("contains")?;
        let held = if at.words(&["text", "that", "matches"]) {
            rule(&mut at, mode)?
        } else {
            text(&mut at, mode)?
        };
        self.wrote(format!("files_after.{name}"));
        self.files_after.insert(name, held);
        at.end()
    }

    /// Which of the idea's connections a server step, or a step with no server, names: by its
    /// port, and its computer where two places share the port; the only connection when it
    /// names neither. A name the idea doesn't have is noted as a problem. An idea with no
    /// connections at all is told so once, by the test's own check.
    fn connection(&mut self, host: Option<&str>, port: Option<&str>) -> usize {
        if self.places.is_empty() && self.stands.is_empty() {
            return 0;
        }
        // A generated test's value can be a template, which the idea's names can't settle.
        if port.is_some_and(|port| port.contains('{'))
            || host.is_some_and(|host| host.contains('{'))
        {
            return 0;
        }
        // A computer the program opens a connection to by itself, which the step names by its
        // computer and its port: its servers are numbered after the idea's own connections.
        if let Some((host, port)) = host.zip(port)
            && let Some(index) = self.stand(false, host, port)
        {
            return self.places.len() + index;
        }
        // The one computer the idea's tests stand in for, when a step names none and the idea
        // names no connection of its own.
        if self.places.is_empty() && host.is_none() && port.is_none() && self.stands.len() == 1 {
            return 0;
        }
        let host = host.map(str::to_ascii_lowercase);
        let wanted = port.map(|port| port.trim().parse::<u16>());
        let matching: Vec<usize> = (0..self.places.len())
            .filter(|index| {
                let place = &self.places[*index];
                let port_matches = match &wanted {
                    Some(Ok(port)) => place.port == *port,
                    Some(Err(_)) => false,
                    None => true,
                };
                let host_matches = host.as_ref().is_none_or(|host| {
                    place.host.as_deref() == Some(host.as_str())
                        || (host == "this computer"
                            && place.host.is_none()
                            && place.argument.is_none())
                });
                port_matches && host_matches
            })
            .collect();
        match matching.as_slice() {
            [index] => *index,
            [] => {
                let named = match (&host, port) {
                    (Some(host), Some(port)) => format!("TCP port {port} on {host}"),
                    (None, Some(port)) => format!("TCP port {port}"),
                    (Some(host), None) => host.clone(),
                    (None, None) => "a place".into(),
                };
                self.found.push((
                    self.line,
                    format!(
                        "idea.md connects to no {named}: its server stands in for one of the \
                         connections `- Network: connects to …` names ({})",
                        self.named_places()
                    ),
                ));
                0
            }
            _ => {
                self.found.push((
                    self.line,
                    format!(
                        "idea.md connects to more than one place on that port ({}): say which, \
                         as in `a server at `example.com` on port `443` that receives …`",
                        self.named_places()
                    ),
                ));
                0
            }
        }
    }

    /// The idea's connections in words, and what it grants the program to open by itself.
    fn named_places(&self) -> String {
        self.places
            .iter()
            .map(Endpoint::describe)
            .chain(
                self.reach
                    .opens
                    .iter()
                    .map(|rule| format!("TCP connections it opens to {}", rule.describe())),
            )
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Which of the computers the idea's tests stand in for a step names by `host` and `port`,
    /// for datagrams or for connections.
    fn stand(&self, datagrams: bool, host: &str, port: &str) -> Option<usize> {
        let host = crate::reach::spelled(host).ok()?.text;
        let port: u16 = port.trim().parse().ok()?;
        self.stands.iter().position(|stand| {
            stand.datagrams == datagrams && stand.host == host && stand.port == port
        })
    }

    /// Whether `port` of `host` is one the program opens a connection to by itself: a rule of
    /// the idea grants it, and it isn't a connection lengua makes before the start.
    fn opens(&self, host: &str, port: &str) -> bool {
        let Ok(port) = port.trim().parse::<u16>() else {
            return false;
        };
        crate::reach::spelled(host).is_ok_and(|computer| {
            let made = self.places.iter().any(|place| {
                place.port == port && place.host.as_deref() == Some(computer.text.as_str())
            });
            !made && Grants::granted(&self.reach.opens, &computer, port, None, None).is_some()
        })
    }

    /// The example the steps describe.
    fn example(mut self) -> ExampleTable {
        let args = std::mem::take(&mut self.args);
        let client = self
            .client
            .into_iter()
            .zip(self.answers)
            .map(|(sends, receives)| ClientTable { sends, receives })
            .collect();
        ExampleTable {
            args,
            stdin: self.stdin.unwrap_or_default(),
            files: self.files,
            folders: self.folders,
            env: self.env,
            stdout: joined(self.stdout),
            stderr: joined(self.stderr),
            files_after: self.files_after,
            gone: self.gone,
            folders_after: self.folders_after,
            random: self.random,
            clock: self.clock,
            server: self.server,
            client,
            clients_at_once: self.clients_at_once,
            stdout_closed: self.stdout_closed,
            piece: self.piece,
        }
    }

    /// The generated tests the steps under the heading on `line` describe, each template with
    /// the line of its step.
    fn outline(mut self, line: usize) -> OutlineSteps {
        let args = std::mem::take(&mut self.args);
        let lines = std::mem::take(&mut self.lines);
        let at = |field: &str| lines.get(field).copied().unwrap_or(line);
        let written = |text: Text, field: &str| Written {
            text: template_of(text),
            line: at(field),
        };
        let texts = |texts: BTreeMap<String, Text>, field: &str| -> BTreeMap<String, Written> {
            texts
                .into_iter()
                .map(|(name, text)| {
                    let written = written(text, &format!("{field}.{name}"));
                    (name, written)
                })
                .collect()
        };
        OutlineSteps {
            line,
            args: args.into_iter().map(|arg| written(arg, "args")).collect(),
            stdin: written(self.stdin.unwrap_or_default(), "stdin"),
            files: texts(self.files, "files"),
            folders: self.folders,
            env: texts(self.env, "env"),
            stdout: written(joined(self.stdout), "stdout"),
            stderr: written(joined(self.stderr), "stderr"),
            files_after: texts(self.files_after, "files_after"),
            gone: self.gone,
            folders_after: self.folders_after,
            random: self.random.map(|random| Written {
                text: random,
                line: at("random"),
            }),
            clock: self
                .clock
                .into_iter()
                .map(|reading| Written {
                    text: reading,
                    line: at("clock"),
                })
                .collect(),
            server: self
                .server
                .into_iter()
                .enumerate()
                .map(|(index, reply)| {
                    let field = format!("server.{index}");
                    (
                        reply.connection,
                        written(reply.receives, &field),
                        written(reply.replies, &field),
                    )
                })
                .collect(),
            client: self
                .client
                .into_iter()
                .zip(self.answers)
                .enumerate()
                .map(|(index, (sends, receives))| {
                    let field = format!("client.{index}");
                    (written(sends, &field), written(receives, &field))
                })
                .collect(),
            clients_at_once: self.clients_at_once,
            stdout_closed: self.stdout_closed,
            piece: self.piece,
            exit_code: self.exit_code.map(|code| Written {
                text: code,
                line: at("exit_code"),
            }),
            serves: self.serves,
        }
    }
}

impl Mode {
    /// A backtick value as this mode reads it: an example's with its escapes read, a template's
    /// with them read outside its placeholders only.
    fn value(self, written: &str) -> String {
        match self {
            Self::Example => unescape(written),
            Self::Generated => unescape_template(written),
        }
    }
}

/// A template written in a step: `\n`, `\r`, `\t` and `\\` read as the characters they name in
/// its text, while each `{…}` placeholder stays as written, so a formula's own strings keep their
/// escapes.
fn unescape_template(written: &str) -> String {
    let mut out = String::with_capacity(written.len());
    let mut literal = String::new();
    let mut characters = written.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '{' | '}' if characters.peek() == Some(&character) => {
                characters.next();
                literal.push(character);
                literal.push(character);
            }
            '{' => {
                out.push_str(&unescape(&literal));
                literal.clear();
                out.push('{');
                let mut depth = 1;
                while depth > 0 {
                    let Some(inner) = characters.next() else {
                        break;
                    };
                    out.push(inner);
                    match inner {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        '"' => {
                            // A string inside the formula, whose escaped quotes don't end it.
                            while let Some(quoted) = characters.next() {
                                out.push(quoted);
                                if quoted == '\\' {
                                    if let Some(escaped) = characters.next() {
                                        out.push(escaped);
                                    }
                                } else if quoted == '"' {
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            other => literal.push(other),
        }
    }
    out.push_str(&unescape(&literal));
    out
}

/// A generated test's text as one template.
fn template_of(text: Text) -> String {
    match text {
        Text::Plain(plain) => plain,
        Text::Rule(Rule { matches }) => matches,
        Text::Parts(parts) => parts
            .into_iter()
            .map(|part| match part {
                TextPart::Plain(plain) => plain,
                TextPart::Repeat(Repeat { repeat, .. }) => repeat,
            })
            .collect(),
    }
}

/// A When step: one run of the program.
fn when(tokens: &[Token]) -> Option<()> {
    let mut at = Cursor::new(tokens);
    at.words(&["the", "program", "runs"]).then_some(())?;
    at.end()
}

/// Which stream a printing step names: standard output unless it says otherwise.
enum Stream {
    Out,
    Error,
}

fn stream(at: &mut Cursor<'_>) -> Option<Stream> {
    if at.words(&["to", "standard", "error"]) {
        return Some(Stream::Error);
    }
    at.words(&["to", "standard", "output"]);
    Some(Stream::Out)
}

/// TEXT: values joined by `then`, each optionally with `` `N` times`` after it, and `a newline`
/// for a line break, which `and` joins too: ``- Then it prints `5` and a newline``. A generated
/// test's repeat is of fixed text, written out in its template, as an input past a limit that no
/// generator reaches needs; a placeholder's value comes from the table, never from a repeat.
fn text(at: &mut Cursor<'_>, mode: Mode) -> Option<Text> {
    let mut parts = Vec::new();
    loop {
        if at.words(&["a", "newline"]) {
            push_plain(&mut parts, "\n");
        } else {
            let value = mode.value(&at.value()?);
            if let Some(count) = at.times() {
                let times: u64 = count.parse().ok()?;
                if mode == Mode::Generated {
                    push_plain(&mut parts, &written_out(&value, times)?);
                } else {
                    parts.push(TextPart::Repeat(Repeat {
                        repeat: value,
                        times,
                    }));
                }
            } else {
                push_plain(&mut parts, &value);
            }
        }
        while at.words(&["and", "a", "newline"]) {
            push_plain(&mut parts, "\n");
        }
        let before = at.at;
        if at.words(&["and", "then"]) || at.is("then") {
            continue;
        }
        at.at = before;
        break;
    }
    Some(match parts.as_slice() {
        [TextPart::Plain(plain)] => Text::Plain(plain.clone()),
        _ => Text::Parts(parts),
    })
}

/// A generated test's repeat, written out: `value`, fixed text, `times` times, within what an
/// example's repeated value may hold. `None` for a value with a placeholder, which the table
/// draws, for an empty one, or for more than a value holds.
fn written_out(value: &str, times: u64) -> Option<String> {
    let times = usize::try_from(times).ok()?;
    let bytes = value.len().checked_mul(times)?;
    if value.is_empty() || value.contains('{') || bytes > super::MAX_FILE_BYTES {
        return None;
    }
    Some(value.repeat(times))
}

/// Adds plain text to `parts`, joined to plain text just before it.
fn push_plain(parts: &mut Vec<TextPart>, plain: &str) {
    if let Some(TextPart::Plain(last)) = parts.last_mut() {
        last.push_str(plain);
    } else {
        parts.push(TextPart::Plain(plain.to_owned()));
    }
}

/// A rule for output the program chooses: the test language in backticks, optionally with
/// `and a newline`. In a generated test, `any(…)` in a template is a rule already.
fn rule(at: &mut Cursor<'_>, mode: Mode) -> Option<Text> {
    let mut matches = mode.value(&at.value()?);
    if at.words(&["and", "a", "newline"]) {
        matches.push('\n');
    }
    Some(Text::Rule(Rule { matches }))
}

/// Whether a step repeats a value, as `` `a` `4097` times`` does.
fn repeats(tokens: &[Token]) -> bool {
    tokens.windows(3).any(|three| match three {
        [Token::Value(_), Token::Value(_), Token::Word(word)] => word == "times",
        _ => false,
    })
}

/// Outputs printed by several steps, joined in order.
fn joined(printed: Vec<Text>) -> Text {
    let mut parts = Vec::new();
    for text in printed {
        match text {
            Text::Plain(plain) => push_plain(&mut parts, &plain),
            Text::Parts(more) => {
                for part in more {
                    match part {
                        TextPart::Plain(plain) => push_plain(&mut parts, &plain),
                        repeat @ TextPart::Repeat(_) => parts.push(repeat),
                    }
                }
            }
            // A rule is the whole output; a second printing step around it is reported where
            // the example is checked, as text and a rule can't join.
            rule @ Text::Rule(_) if parts.is_empty() => return rule,
            Text::Rule(_) => return Text::Parts(Vec::new()),
        }
    }
    match parts.as_slice() {
        [] => Text::default(),
        [TextPart::Plain(plain)] => Text::Plain(plain.clone()),
        _ => Text::Parts(parts),
    }
}

/// The phrasing to write a step lengua can't read: its own values in the phrasing its words
/// come closest to, or the list of phrasings.
fn rewrite(step: &Step, phase: Keyword) -> String {
    let words: Vec<String> = step
        .tokens()
        .into_iter()
        .filter_map(|token| match token {
            Token::Word(word) => Some(word),
            Token::Value(_) => None,
        })
        .collect();
    let has = |word: &str| words.iter().any(|found| found.starts_with(word));
    let values: Vec<String> = step
        .values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect();
    let first = values.first().cloned().unwrap_or_else(|| "`…`".into());
    let keyword = step.keyword.name();
    let suggestion = match phase {
        Keyword::When => Some("the program runs".to_owned()),
        Keyword::Then if has("exit") => Some(format!("it exits with code {first}")),
        Keyword::Then if has("serv") || has("listen") => Some("it does not stop".to_owned()),
        Keyword::Then if has("client") => Some(format!("the client receives {first}")),
        Keyword::Then if has("hold") || has("file") || has("writ") => Some(format!(
            "the file {first} contains {}",
            values.get(1).cloned().unwrap_or_else(|| "`…`".into())
        )),
        Keyword::Then if has("error") => Some(format!("it prints {first} to standard error")),
        Keyword::Then if has("print") || has("output") || has("show") || has("say") => {
            Some(format!("it prints {first}"))
        }
        Keyword::Then => None,
        _ if has("argument") => Some(format!("the arguments {}", values.join(" and "))),
        _ if has("input") => Some(format!("standard input that contains {first}")),
        _ if has("clock") || has("time") => Some(format!("the clock at {first}")),
        _ if has("random") || has("byte") => {
            Some(format!("the random bytes {}", values.join(" and ")))
        }
        _ if has("server") => {
            Some("a server on port `…` that receives `…` and replies `…`".to_owned())
        }
        _ if has("client") => Some(format!("a client that sends {first}")),
        _ if has("file") => Some(format!(
            "a file {first} that contains {}",
            values.get(1).cloned().unwrap_or_else(|| "`…`".into())
        )),
        _ => None,
    };
    match suggestion {
        Some(phrase) => format!("`- {keyword} {phrase}`"),
        None if phase == Keyword::Then => format!("one of {THEN_PHRASES}"),
        None => format!("one of {GIVEN_PHRASES}"),
    }
}

/// A place in a step's tokens.
struct Cursor<'a> {
    tokens: &'a [Token],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, at: 0 }
    }

    /// Takes `word` when it's next.
    fn word(&mut self, word: &str) -> Option<()> {
        match self.tokens.get(self.at) {
            Some(Token::Word(found)) if found == word => {
                self.at += 1;
                Some(())
            }
            _ => None,
        }
    }

    /// Takes `word` when it's next, saying whether it was.
    fn is(&mut self, word: &str) -> bool {
        self.word(word).is_some()
    }

    /// Takes all of `words` when they come next, or none of them.
    fn words(&mut self, words: &[&str]) -> bool {
        let start = self.at;
        for word in words {
            if self.word(word).is_none() {
                self.at = start;
                return false;
            }
        }
        true
    }

    /// Takes the next value.
    fn value(&mut self) -> Option<String> {
        match self.tokens.get(self.at) {
            Some(Token::Value(value)) => {
                self.at += 1;
                Some(value.clone())
            }
            _ => None,
        }
    }

    /// Takes `` `N` times`` when it comes next, giving `N`.
    fn times(&mut self) -> Option<String> {
        match (self.tokens.get(self.at), self.tokens.get(self.at + 1)) {
            (Some(Token::Value(count)), Some(Token::Word(word))) if word == "times" => {
                self.at += 2;
                Some(count.clone())
            }
            _ => None,
        }
    }

    fn is_end(&self) -> bool {
        self.at >= self.tokens.len()
    }

    /// Done, when nothing is left.
    fn end(&self) -> Option<()> {
        self.is_end().then_some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idea_with(steps: &str, constraints: &str) -> Idea {
        let text = format!(
            "---\ntype: lengua/program-idea\ntitle: t\ndescription: A test.\n---\n# Purpose\n\
             It tests `1` to `9`.\n\n\
             # Behaviors\n## B1: Does it\n{steps}\n\n# Constraints\n{constraints}\n"
        );
        crate::idea::parse(&text).unwrap_or_else(|problems| panic!("{problems:?}"))
    }

    fn table(steps: &str, constraints: &str) -> ExampleTable {
        let idea = idea_with(steps, constraints);
        example(&idea.behaviors[0], &idea).unwrap_or_else(|problems| panic!("{problems:?}"))
    }

    fn outline(steps: &str, constraints: &str) -> (OutlineSteps, Vec<OutlineRow>) {
        let idea = idea_with(steps, constraints);
        let behavior = &idea.behaviors[0];
        let outline = behavior.outline.as_ref().expect("generated tests");
        generated(behavior, outline, &idea).unwrap_or_else(|problems| panic!("{problems:?}"))
    }

    fn plain(text: &Text) -> String {
        match text {
            Text::Plain(plain) => plain.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// `standard output is closed` makes every write to it fail, so a behavior with it says what
    /// the program prints to standard error and its exit code, never what it prints to standard
    /// output.
    #[test]
    fn a_closed_standard_output_takes_no_output() {
        let read = table(
            "- Given the arguments `2` and `3`\n- And standard output is closed\n- When the \
             program runs\n- Then it exits with code `3`",
            "",
        );
        assert!(read.stdout_closed);
        let idea = idea_with(
            "- Given the arguments `2` and `3`\n- And standard output is closed\n- When the \
             program runs\n- Then it prints `5`\n- And it exits with code `3`",
            "",
        );
        let problems = example(&idea.behaviors[0], &idea).expect_err("output it can't print");
        assert!(
            format!("{problems:?}").contains("its standard output is closed"),
            "{problems:?}"
        );
    }

    #[test]
    fn reads_arguments_input_and_output() {
        let read = table(
            "- Given the arguments `2` and `3`\n- When the program runs\n- Then it prints `5` \
             and a newline\n- And it exits with code `0`",
            "",
        );
        let args: Vec<String> = read.args.iter().map(plain).collect();
        assert_eq!(args, ["2", "3"]);
        assert_eq!(plain(&read.stdout), "5\n");

        let read = table(
            "- Given standard input that contains `a\\t\\tb   \\rc`\n- When the program runs\n- \
             Then it prints `usage: x` and a newline to standard error\n- And it exits with code \
             `1`",
            "- Uses: standard input",
        );
        assert_eq!(plain(&read.stdin), "a\t\tb   \rc");
        assert_eq!(plain(&read.stderr), "usage: x\n");
    }

    #[test]
    fn reads_files_repeats_and_rules() {
        let mut read = table(
            "- Given a file `big.txt` that contains `a` `5` times then `x`\n- And the arguments \
             `big.txt` and `out.txt`\n- When the program runs\n- Then the file `out.txt` \
             contains `A` and a newline\n- And it prints text that matches `rolled {any(1, \
             6)}` and a newline\n- And it exits with code `0`",
            "- Uses: files",
        );
        let big = read.files.remove("big.txt").expect("the file");
        assert_eq!(big.expand(), Ok("aaaaax".into()));
        assert_eq!(plain(&read.files_after["out.txt"]), "A\n");
        assert!(
            matches!(&read.stdout, Text::Rule(Rule { matches }) if matches == "rolled {any(1, 6)}\n")
        );
    }

    #[test]
    fn reads_environment_variables() {
        let read = table(
            "- Given the environment variable `NAME` with the value `Ada`\n- And the environment \
             variable `HOME` with the value `/home/ada`\n- When the program runs\n- Then it prints \
             `Hello, Ada!` and a newline\n- And it exits with code `0`",
            "- Uses: the environment",
        );
        let env: Vec<(&str, String)> = read
            .env
            .iter()
            .map(|(name, value)| (name.as_str(), plain(value)))
            .collect();
        assert_eq!(env, [("HOME", "/home/ada".into()), ("NAME", "Ada".into())]);
    }

    #[test]
    fn reads_folders_and_files_that_go() {
        let read = table(
            "- Given the argument `todo.txt`\n- And an empty folder `archive`\n- And a file \
             `todo.txt` that contains `milk`\n- When the program runs\n- Then there is no file \
             `todo.txt`\n- And there is no folder `tmp`\n- And there is a folder `archive`\n- And \
             the file `archive/todo.txt` contains `milk`\n- And it exits with code `0`",
            "- Uses: files",
        );
        assert_eq!(read.folders.iter().collect::<Vec<_>>(), ["archive"]);
        assert_eq!(read.gone.iter().collect::<Vec<_>>(), ["tmp", "todo.txt"]);
        assert_eq!(read.folders_after.iter().collect::<Vec<_>>(), ["archive"]);
        assert_eq!(plain(&read.files_after["archive/todo.txt"]), "milk");
    }

    #[test]
    fn reads_the_network_and_the_supply() {
        let read = table(
            "- Given the argument `/hello`\n- And a server on port `8080` that receives `GET \
             /hello HTTP/1.0\\r\\n\\r\\n` and replies `HTTP/1.0 200 OK\\r\\n\\r\\nhi`\n- When \
             the program runs\n- Then it prints `hi`\n- And it exits with code `0`",
            "- Network: connects to TCP port 8080",
        );
        let args: Vec<String> = read.args.iter().map(plain).collect();
        assert_eq!(
            args,
            ["/hello"],
            "the program needs no port: lengua connects it"
        );
        assert_eq!(
            plain(&read.server[0].receives),
            "GET /hello HTTP/1.0\r\n\r\n"
        );
        assert_eq!(read.server[0].connection, 0);
        // A server stands in for the connection its port, and its computer, name; with one
        // connection it needs to name neither.
        let read = table(
            "- Given a server on port `8080` that receives `a` and replies `b`\n- And a server \
             at `example.com` on port `80` that receives `c` and replies `d`\n- When the program \
             runs\n- Then it prints `bd`\n- And it exits with code `0`",
            "- Network: connects to TCP ports 80 and 8080\n- Network: connects to TCP port 80 on \
             example.com",
        );
        let connections: Vec<usize> = read.server.iter().map(|server| server.connection).collect();
        assert_eq!(connections, [1, 2]);
        let read = table(
            "- Given a server that receives `a` and replies `b`\n- When the program runs\n- Then \
             it prints `b`\n- And it exits with code `0`",
            "- Network: connects to TCP port 443 on example.com",
        );
        assert_eq!(read.server[0].connection, 0);
        let problems = |steps: &str, constraints: &str| {
            let idea = idea_with(steps, constraints);
            example(&idea.behaviors[0], &idea)
                .expect_err("a problem")
                .into_iter()
                .map(|problem| problem.message)
                .collect::<Vec<_>>()
                .join("; ")
        };
        let ends = "\n- When the program runs\n- Then it prints `b`\n- And it exits with code `0`";
        let found = problems(
            &format!("- Given a server on port `9` that receives `a` and replies `b`{ends}"),
            "- Network: connects to TCP port 8080",
        );
        assert!(found.contains("connects to no TCP port 9"), "{found}");
        let found = problems(
            &format!("- Given no server on port `9`{ends}"),
            "- Network: connects to TCP port 8080",
        );
        assert!(found.contains("connects to no TCP port 9"), "{found}");
        let found = problems(
            &format!(
                "- Given a server on port `80` that receives `a` and replies `b`\n- And a server \
                 on port `80` that receives `a` and replies `b`{ends}"
            ),
            "- Network: connects to TCP port 80",
        );
        assert!(found.contains("two servers for its connection"), "{found}");
        let found = problems(
            &format!("- Given a server on port `80` that receives `a` and replies `b`{ends}"),
            "- Network: connects to TCP port 80\n- Network: connects to TCP port 80 on example.com",
        );
        assert!(found.contains("say which"), "{found}");

        let read = table(
            "- Given the random bytes `fc` and `2a`\n- And the clock at `1735689600`\n- \
             When the program runs\n- Then it prints `1`\n- And it exits with code `0`",
            "- Uses: random bytes, the clock",
        );
        assert_eq!(read.random.as_deref(), Some("fc2a"));
        assert_eq!(read.clock, ["1735689600"]);
    }

    /// A program that opens connections by itself gets a test's server for each computer its
    /// steps name, numbered after the connections lengua makes: one step a connection it opens
    /// there, in order. A computer no rule of the idea covers has no server, and neither has a
    /// name the idea grants no look at.
    #[test]
    fn reads_the_servers_of_what_a_program_opens_by_itself() {
        let constraints = "- Network: connects to TCP port 8080\n- Network: opens TCP connections \
                           to port 80 on any subdomain of example.org\n- Network: sends UDP \
                           datagrams to port 53 on 192.0.2.53\n- Network: looks up the addresses \
                           of any subdomain of example.org";
        let ends = "\n- When the program runs\n- Then it prints `x`\n- And it exits with code `0`";
        let read = table(
            &format!(
                "- Given a server on port `8080` that receives `a` and replies `b`\n- And a \
                 server at `www.example.org` on port `80` that receives `c` and replies `d`\n- \
                 And a server at `files.example.org` on port `80` that receives `e` and replies \
                 `f`\n- And a server at `WWW.example.org` on port `80` that receives `g` and \
                 replies `h`\n- And a computer at `192.0.2.53` on UDP port `53` that receives \
                 `i` and replies `j`\n- And the addresses `192.0.2.7` for the name \
                 `www.example.org`\n- And no server at `api.example.org` on port `80`{ends}"
            ),
            constraints,
        );
        // The idea's own connection is 0. Then the computers its tests stand in for, in the
        // order its steps first name them: a second step for one is its second connection.
        let connections: Vec<usize> = read.server.iter().map(|server| server.connection).collect();
        assert_eq!(connections, [0, 1, 2, 1, 3]);
        assert_eq!(plain(&read.server[3].receives), "g");
        let problems = |steps: &str| {
            let idea = idea_with(&format!("{steps}{ends}"), constraints);
            example(&idea.behaviors[0], &idea)
                .expect_err("a problem")
                .into_iter()
                .map(|problem| problem.message)
                .collect::<Vec<_>>()
                .join("; ")
        };
        let found = problems(
            "- Given a server at `elsewhere.test` on port `80` that receives `a` and replies `b`",
        );
        assert!(
            found.contains("connects to no TCP port 80 on elsewhere.test"),
            "{found}"
        );
        assert!(
            found.contains("TCP connections it opens to port 80 on any subdomain of example.org"),
            "{found}"
        );
        let found = problems(
            "- Given a computer at `192.0.2.54` on UDP port `53` that receives `a` and replies \
             `b`",
        );
        assert!(
            found.contains("grants no UDP datagrams to port 53"),
            "{found}"
        );
        let found = problems("- Given the addresses `192.0.2.7` for the name `example.com`");
        assert!(
            found.contains("grants no look at the addresses of `example.com`"),
            "{found}"
        );
    }

    #[test]
    fn a_step_it_cant_read_comes_with_its_rewrite() {
        let idea = idea_with(
            "- Given the input `abc`\n- When the program runs\n- Then the output is `ABC`\n- And it \
             exits with code `0`",
            "- Uses: standard input",
        );
        let problems = example(&idea.behaviors[0], &idea).expect_err("two steps it can't read");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0]
                .message
                .contains("`- Given standard input that contains `abc``"),
            "{problems:?}"
        );
        assert!(
            problems[1].message.contains("`- Then it prints `ABC``"),
            "{problems:?}"
        );
    }

    const EXAMPLE: &str = "- Given the arguments `2` and `3`\n- When the program runs\n- Then it \
                           prints `5` and a newline\n- And it exits with code `0`\n\n";

    #[test]
    fn reads_generated_tests_as_templates_over_their_table() {
        let (steps, rows) = outline(
            &format!(
                "{EXAMPLE}### For any two numbers\nSome prose about them.\n\n- Given the \
                 arguments `{{a}}` and `{{b}}`\n- When the program runs\n- Then it prints \
                 `{{a + b}}` and a newline\n- And it exits with code `{{if(a < 0, 1, \
                 0)}}`\n\n| a | b | runs |\n|---|---|---|\n| `int(1, 9)` | `int(1, 9)` | 50 |\n| \
                 `choice(1)` | `int(1, 9)` | |\n"
            ),
            "",
        );
        let args: Vec<&str> = steps.args.iter().map(|arg| arg.text.as_str()).collect();
        assert_eq!(args, ["{a}", "{b}"]);
        assert_eq!(steps.stdout.text, "{a + b}\n");
        assert_eq!(
            steps.exit_code.as_ref().map(|code| code.text.as_str()),
            Some("{if(a < 0, 1, 0)}")
        );
        // Each template keeps the line of its step.
        assert_eq!(steps.stdout.line, steps.args[0].line + 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].runs.as_deref(), Some("50"));
        assert_eq!(rows[1].runs, None);
        assert_eq!(
            rows[1].vars,
            [
                ("a".to_owned(), "choice(1)".to_owned()),
                ("b".to_owned(), "int(1, 9)".to_owned())
            ]
        );
    }

    #[test]
    fn escapes_are_read_outside_placeholders_only() {
        assert_eq!(
            unescape_template("{count(text, \"\\n\")}\\n"),
            "{count(text, \"\\n\")}\n"
        );
        assert_eq!(unescape_template("{{ {a} }}\\t"), "{{ {a} }}\t");
        assert_eq!(
            unescape_template("{\"}\\\"\" + s}\\r\\n"),
            "{\"}\\\"\" + s}\r\n"
        );
    }

    #[test]
    fn generated_tests_draw_text_instead_of_repeating_it() {
        let idea = idea_with(
            &format!(
                "{EXAMPLE}### For any text\n- Given standard input that contains `{{a}}` `3` \
                 times\n- When the program runs\n- Then it prints `x`\n- And it exits with code \
                 `0`\n\n| a |\n|---|\n| `int(1, 9)` |\n"
            ),
            "- Uses: standard input",
        );
        let behavior = &idea.behaviors[0];
        let problems = generated(behavior, behavior.outline.as_ref().expect("tests"), &idea)
            .expect_err("a repeated placeholder");
        assert!(
            problems[0].message.contains("never a placeholder"),
            "{problems:?}"
        );
    }

    /// A generated test may repeat fixed text, which its template holds written out, as input
    /// past a limit that no generator reaches needs: here 4 newlines, then a drawn line.
    #[test]
    fn generated_tests_repeat_fixed_text_written_out() {
        let (steps, rows) = outline(
            "- Given standard input that contains `\\n` `5` times\n- When the program runs\n- \
             Then it prints `x`\n- And it exits with code `0`\n\n### For any line\n- Given \
             standard input that contains `\\n` `4` times then `{line}`\n- When the program \
             runs\n- Then it prints `x`\n- And it exits with code `0`\n\n| line |\n|---|\n| \
             `string(1, 9, \"a\\n\")` |",
            "- Uses: standard input",
        );
        assert_eq!(steps.stdin.text, "\n\n\n\n{line}");
        assert_eq!(rows.len(), 1);
    }

    /// A behavior's steps in lengua's STE phrasings, for the tests below to change.
    const STE_STEPS: &str = "- Given standard input that contains `x`\n- When the program runs\n- \
                             Then it prints `x` and a newline\n- And it exits with code `0`";

    /// The problems lengua finds once the step at `at` in [`STE_STEPS`] says `text` instead: an
    /// idea that holds such a step needn't come from idea.md's own reading. The step's values
    /// are `text`'s own, as reading it would give them.
    fn changed(at: usize, text: &str) -> Vec<Problem> {
        let mut idea = idea_with(STE_STEPS, "- Uses: standard input");
        let step = &mut idea.behaviors[0].steps[at];
        step.text = text.to_owned();
        step.values = step
            .tokens()
            .into_iter()
            .filter_map(|token| match token {
                Token::Value(value) => Some(value),
                Token::Word(_) => None,
            })
            .collect();
        example(&idea.behaviors[0], &idea).expect_err("a step it doesn't read")
    }

    /// Each STE phrasing reads as the old one it replaces meant.
    #[test]
    fn reads_the_ste_phrasings() {
        let mut read = table(
            "- Given standard input that contains `a` and a newline\n- And a file `in.txt` that \
             contains `b` `3` times then `c`\n- And a file named `x.txt` that contains `d`\n- And \
             the environment variable `NAME` with the value `Ada`\n- When the program runs\n- Then \
             it prints `e` and a newline then `f`\n- And the file `out.txt` contains `g` and a \
             newline\n- And it exits with code `0`",
            "- Uses: standard input, files, the environment",
        );
        assert_eq!(plain(&read.stdin), "a\n");
        let repeated = read.files.remove("in.txt").expect("the file");
        assert_eq!(repeated.expand(), Ok("bbbc".into()));
        assert_eq!(plain(&read.files["x.txt"]), "d");
        assert_eq!(plain(&read.env["NAME"]), "Ada");
        assert_eq!(plain(&read.stdout), "e\nf");
        assert_eq!(plain(&read.files_after["out.txt"]), "g\n");
    }

    /// Rules, the test's client, and a server that does not stop, in their STE phrasings.
    #[test]
    fn reads_the_ste_phrasings_of_rules_and_servers() {
        let read = table(
            "- Given the argument `x`\n- When the program runs\n- Then it prints text that matches \
             `rolled {any(1, 6)}` and a newline\n- And the file `out.txt` contains text that \
             matches `{any(1, 9)}`\n- And it exits with code `0`",
            "- Uses: files",
        );
        let Text::Rule(Rule { matches }) = &read.stdout else {
            panic!("{:?}", read.stdout);
        };
        assert_eq!(matches, "rolled {any(1, 6)}\n");
        let Text::Rule(Rule { matches }) = &read.files_after["out.txt"] else {
            panic!("{:?}", read.files_after);
        };
        assert_eq!(matches, "{any(1, 9)}");
        let (steps, rows) = outline(
            "- Given a client that sends `hi` and a newline\n- When the program runs\n- Then the \
             client receives `hello` and a newline\n- And it does not stop\n\n### For any text\n\
             - Given a client that sends `{text}` and a newline\n- When the program runs\n- Then \
             the client receives text that matches `{any(1, 9)}`\n- And it does not stop\n\n| \
             text |\n|---|\n| `string(1, 9, \"ab\")` |",
            "- Network: listens on TCP port 8080",
        );
        assert!(steps.serves);
        assert_eq!(rows.len(), 1);
        assert_eq!(steps.client.len(), 1);
        assert_eq!(steps.client[0].0.text, "{text}\n");
        assert_eq!(steps.client[0].1.text, "{any(1, 9)}");
    }

    /// A step in one of lengua's old phrasings is refused with the STE phrasing that replaces it,
    /// and nothing else is said of it.
    #[test]
    fn refuses_the_old_phrasings_with_their_ste_phrasings() {
        for old in [
            "standard input holding `x`",
            "a file `in.txt` holding `x`",
            "a file named `in.txt` holding `x`",
            "the environment variable `NAME` set to `x`",
            "it prints text matching `{any(1, 6)}`",
            "the file `out.txt` holds `x`",
            "the client gets back `x`",
            "it keeps serving",
            "nothing listening on port `8080`",
            "it prints `x` followed by a newline",
            "it prints `x` followed by `y`",
            "standard input that contains `a` repeated `3` times",
        ] {
            let problems = changed(0, old);
            assert!(
                problems
                    .iter()
                    .all(|problem| problem.message.starts_with("STE: B1: write ")),
                "{old}: {problems:?}"
            );
        }
        let problems = changed(0, "standard input holding `x` followed by a newline");
        let messages: Vec<&str> = problems
            .iter()
            .map(|problem| problem.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "STE: B1: write `standard input that contains `…``, not `standard input holding \
                 `…``",
                "STE: B1: write ``…` and a newline`, not ``…` followed by a newline`",
            ]
        );
    }

    /// What lengua suggests for a step it can't read is in its STE phrasings.
    #[test]
    fn suggestions_say_the_ste_phrasings() {
        for phrases in [GIVEN_PHRASES, THEN_PHRASES] {
            for old in [
                "holding",
                "holds",
                "matching",
                "set to",
                "gets back",
                "keeps serving",
                "followed by",
                "repeated",
            ] {
                assert!(!phrases.contains(old), "{old}");
            }
        }
        let suggested = |at: usize, text: &str| changed(at, text)[0].message.clone();
        let input = suggested(0, "the input `abc`");
        assert!(input.ends_with("as `- Given standard input that contains `abc``"));
        let file = suggested(2, "the file `out.txt` has `y`");
        assert!(file.ends_with("as `- Then the file `out.txt` contains `y``"));
        let client = suggested(2, "the client gets `y`");
        assert!(client.ends_with("as `- Then the client receives `y``"));
        let serves = suggested(3, "the server stays up");
        assert!(serves.ends_with("as `- And it does not stop`"));
    }

    /// A generated test's text comes from its table: a placeholder is never repeated.
    #[test]
    fn generated_tests_draw_text_instead_of_repeating_it_in_ste() {
        let idea = idea_with(
            &format!(
                "{STE_STEPS}\n\n### For any text\n- Given standard input that contains `{{a}}` \
                 `3` times\n- When the program runs\n- Then it prints `x`\n- And it exits with \
                 code `0`\n\n| a |\n|---|\n| `int(1, 9)` |\n"
            ),
            "- Uses: standard input",
        );
        let behavior = &idea.behaviors[0];
        let problems = generated(behavior, behavior.outline.as_ref().expect("tests"), &idea)
            .expect_err("a repeated placeholder");
        let message = &problems[0].message;
        assert!(
            message.contains("`` `N` times`` repeats fixed text"),
            "{problems:?}"
        );
        assert!(message.contains("never a placeholder"), "{problems:?}");
    }
}
