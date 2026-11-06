//! A behavior's Given/When/Then steps as its tests. Its own steps are its example, with literal
//! values. The steps under its `###` heading are its generated tests: the same phrasings, whose
//! values are templates over the variables of the table below them, one row a generated test.
//! lengua reads steps in a fixed vocabulary, as Gherkin's step definitions do, so the idea's own
//! words are the tests and nothing is written twice.
//!
//! Given:
//! - ``the arguments `a` and `b` ``, ``the argument `a` ``, `no arguments`
//! - ``standard input holding TEXT``, `no standard input` or `empty standard input`
//! - ``a file `name` holding TEXT``, ``an empty file `name` ``, ``no file `name` ``
//! - ``the environment variable `NAME` set to TEXT``
//! - ``an empty folder `name` ``
//! - ``the clock at `seconds` ``, with ``then `seconds` `` for later readings
//! - ``the random bytes `fc` and `2a` ``
//! - ``a server on port `8080` that receives TEXT and replies TEXT``, with ``at `example.com` ``
//!   before `on port` for a computer the idea names, ``no server on port `8080` ``
//! - ``a client that sends TEXT``, and `the clients connect at once` for clients that overlap
//! - `standard output is closed`, a pipe nobody reads, so every write to it fails with `EPIPE`
//!
//! When: `the program runs`.
//!
//! Then:
//! - ``it prints TEXT``, with `to standard error` for standard error, and `it prints nothing`
//! - ``it prints text matching `rule` `` for output the program chooses, such as
//!   `` `rolled {any(1, 6)}` ``
//! - ``the file `name` holds TEXT``
//! - ``there is no file `name` ``, ``there is no folder `name` ``, ``there is a folder `name` ``
//! - ``the client gets back TEXT``
//! - ``it exits with code `0` ``, `it keeps serving`
//!
//! TEXT is a value in backticks, written with `\n`, `\r`, `\t`, `\e` and `\\` for the characters
//! they name; ``followed by a newline`` adds one, ``repeated `N` times`` repeats a value in an example,
//! and `followed by` or `then` joins parts. In a generated test, `{…}` holds a formula, whose own
//! strings keep their escapes.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ClientTable, ExampleTable, OutlineRow, OutlineSteps, Problem, Repeat, Rule, ServerTable, Text,
    TextPart, Written, unescape,
};
use crate::idea::{Behavior, Endpoint, Idea, Keyword, Outline, Step, Token};

/// The column of a table that gives its row's number of runs, not a variable.
const RUNS: &str = "runs";

/// The phrasings lengua reads, for a step it can't.
const GIVEN_PHRASES: &str = "`Given the arguments `…` and `…``, `Given no arguments`, `Given \
     standard input holding `…``, `Given a file `name` holding `…``, `Given no file `name``, \
     `Given the environment variable `NAME` set to `…``, `Given an empty folder `name``, `Given \
     the clock at `…``, `Given the \
     random bytes `…``, `Given a server on port \
     `…` that receives `…` and replies `…``, `Given a client that sends `…``, `And the clients \
     connect at once`, `And standard output is closed`";
const THEN_PHRASES: &str = "`Then it prints `…``, `Then it prints `…` to standard error`, \
     `Then it prints nothing`, `Then it prints text matching `rule``, `Then the file `name` \
     holds `…``, `Then there is no file `name``, `Then there is a folder `name``, `Then the \
     client gets back `…``, `And it exits with code `N``, `And it keeps serving`";

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

/// Reads `steps` in `mode`, reporting each step it can't read with its rewrite.
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
        reading.line = step.line;
        let tokens = step.tokens();
        let read = match phase {
            Keyword::When => when(&tokens),
            Keyword::Then => reading.output(&tokens),
            _ => reading.input(&tokens),
        };
        if read.is_none() {
            let rewrite = if mode == Mode::Generated && step.text.contains("repeated") {
                "a value drawn from the table: a generated test's text comes from its variables, \
                 and `repeated` is for examples"
                    .to_owned()
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
                "{whose}: each `a client that sends …` needs its `the client gets back …`, in the \
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
    answers: Vec<Text>,
    /// A generated test's exit code as written: a number or a formula.
    exit_code: Option<String>,
    serves: bool,
    /// The line of the step that first wrote each field, for problems in generated tests.
    lines: BTreeMap<String, usize>,
    /// The connections the idea names, which the test's servers stand in for.
    places: Vec<Endpoint>,
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
        if at.words(&["no", "server"]) || at.words(&["nothing", "listening"]) {
            // The connection to the place it names is handed over already ended.
            let host = at.is("at").then(|| at.value()).flatten();
            let port = (at.words(&["on", "port"]) || at.words(&["there", "on", "port"]))
                .then(|| at.value())
                .flatten();
            if port.is_some() || host.is_some() {
                self.connection(host.as_deref(), port.as_deref());
            }
            return Some(());
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
                at.word("set")?;
                at.word("to")?;
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
            return None;
        }
        if at.words(&["standard", "input", "holding"]) {
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
            at.word("holding")?;
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
                .any(|server| server.connection == connection && !self.places.is_empty())
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
        if at.words(&["it", "keeps", "serving"]) {
            self.serves = true;
            return at.end();
        }
        if at.words(&["it", "prints"]) {
            if at.is("nothing") {
                stream(&mut at)?;
                return at.end();
            }
            let printed = if at.words(&["text", "matching"]) {
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
        if at.words(&["the", "client", "gets", "back"]) {
            let answer = if at.words(&["text", "matching"]) {
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
        at.word("holds")?;
        let held = if at.words(&["text", "matching"]) {
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
        if self.places.is_empty() {
            return 0;
        }
        // A generated test's value can be a template, which the idea's names can't settle.
        if port.is_some_and(|port| port.contains('{'))
            || host.is_some_and(|host| host.contains('{'))
        {
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

    /// The idea's connections in words.
    fn named_places(&self) -> String {
        self.places
            .iter()
            .map(Endpoint::describe)
            .collect::<Vec<_>>()
            .join(", ")
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

/// TEXT: values joined by `followed by` or `then`, each optionally ``repeated `N` times`` in an
/// example, with `a newline` for a line break.
fn text(at: &mut Cursor<'_>, mode: Mode) -> Option<Text> {
    let mut parts = Vec::new();
    loop {
        if at.words(&["a", "newline"]) {
            push_plain(&mut parts, "\n");
        } else {
            let value = mode.value(&at.value()?);
            if at.is("repeated") {
                if mode == Mode::Generated {
                    return None;
                }
                let times = at.value()?.parse().ok()?;
                at.word("times")?;
                parts.push(TextPart::Repeat(Repeat {
                    repeat: value,
                    times,
                }));
            } else {
                push_plain(&mut parts, &value);
            }
        }
        let before = at.at;
        if at.words(&["followed", "by"]) || at.words(&["and", "then"]) || at.is("then") {
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

/// Adds plain text to `parts`, joined to plain text just before it.
fn push_plain(parts: &mut Vec<TextPart>, plain: &str) {
    if let Some(TextPart::Plain(last)) = parts.last_mut() {
        last.push_str(plain);
    } else {
        parts.push(TextPart::Plain(plain.to_owned()));
    }
}

/// A rule for output the program chooses: the test language in backticks, optionally
/// `followed by a newline`. In a generated test, `any(…)` in a template is a rule already.
fn rule(at: &mut Cursor<'_>, mode: Mode) -> Option<Text> {
    let mut matches = mode.value(&at.value()?);
    if at.words(&["followed", "by", "a", "newline"]) {
        matches.push('\n');
    }
    Some(Text::Rule(Rule { matches }))
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
        Keyword::Then if has("serv") || has("listen") => Some("it keeps serving".to_owned()),
        Keyword::Then if has("client") => Some(format!("the client gets back {first}")),
        Keyword::Then if has("hold") || has("file") || has("writ") => Some(format!(
            "the file {first} holds {}",
            values.get(1).cloned().unwrap_or_else(|| "`…`".into())
        )),
        Keyword::Then if has("error") => Some(format!("it prints {first} to standard error")),
        Keyword::Then if has("print") || has("output") || has("show") || has("say") => {
            Some(format!("it prints {first}"))
        }
        Keyword::Then => None,
        _ if has("argument") => Some(format!("the arguments {}", values.join(" and "))),
        _ if has("input") => Some(format!("standard input holding {first}")),
        _ if has("clock") || has("time") => Some(format!("the clock at {first}")),
        _ if has("random") || has("byte") => {
            Some(format!("the random bytes {}", values.join(" and ")))
        }
        _ if has("server") => {
            Some("a server on port `…` that receives `…` and replies `…`".to_owned())
        }
        _ if has("client") => Some(format!("a client that sends {first}")),
        _ if has("file") => Some(format!(
            "a file {first} holding {}",
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
             A test of `1` to `9`.\n\n\
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
             followed by a newline\n- And it exits with code `0`",
            "",
        );
        let args: Vec<String> = read.args.iter().map(plain).collect();
        assert_eq!(args, ["2", "3"]);
        assert_eq!(plain(&read.stdout), "5\n");

        let read = table(
            "- Given standard input holding `a\\t\\tb   \\rc`\n- When the program runs\n- Then it \
             prints `usage: x` followed by a newline to standard error\n- And it exits with code \
             `1`",
            "- Uses: standard input",
        );
        assert_eq!(plain(&read.stdin), "a\t\tb   \rc");
        assert_eq!(plain(&read.stderr), "usage: x\n");
    }

    #[test]
    fn reads_files_repeats_and_rules() {
        let mut read = table(
            "- Given a file named `big.txt` holding `a` repeated `5` times followed by `x`\n- And \
             the arguments `big.txt` and `out.txt`\n- When the program runs\n- Then the file \
             `out.txt` holds `A` followed by a newline\n- And it prints text matching `rolled \
             {any(1, 6)}` followed by a newline\n- And it exits with code `0`",
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
            "- Given the environment variable `NAME` set to `Ada`\n- And the environment variable \
             `HOME` set to `/home/ada`\n- When the program runs\n- Then it prints `Hello, Ada!` \
             followed by a newline\n- And it exits with code `0`",
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
             `todo.txt` holding `milk`\n- When the program runs\n- Then there is no file \
             `todo.txt`\n- And there is no folder `tmp`\n- And there is a folder `archive`\n- And \
             the file `archive/todo.txt` holds `milk`\n- And it exits with code `0`",
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
                .contains("`- Given standard input holding `abc``"),
            "{problems:?}"
        );
        assert!(
            problems[1].message.contains("`- Then it prints `ABC``"),
            "{problems:?}"
        );
    }

    const EXAMPLE: &str = "- Given the arguments `2` and `3`\n- When the program runs\n- Then it \
                           prints `5` followed by a newline\n- And it exits with code `0`\n\n";

    #[test]
    fn reads_generated_tests_as_templates_over_their_table() {
        let (steps, rows) = outline(
            &format!(
                "{EXAMPLE}### For any two numbers\nSome prose about them.\n\n- Given the \
                 arguments `{{a}}` and `{{b}}`\n- When the program runs\n- Then it prints \
                 `{{a + b}}` followed by a newline\n- And it exits with code `{{if(a < 0, 1, \
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
                "{EXAMPLE}### For any text\n- Given standard input holding `a` repeated `3` \
                 times\n- When the program runs\n- Then it prints `x`\n- And it exits with code \
                 `0`\n\n| a |\n|---|\n| `int(1, 9)` |\n"
            ),
            "- Uses: standard input",
        );
        let behavior = &idea.behaviors[0];
        let problems = generated(behavior, behavior.outline.as_ref().expect("tests"), &idea)
            .expect_err("a repeat");
        assert!(
            problems[0].message.contains("`repeated` is for examples"),
            "{problems:?}"
        );
    }
}
