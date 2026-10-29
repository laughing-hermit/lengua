//! `tests.toml`: one example per behavior with the spec's own values, plus generated tests whose
//! formulas compute the expected results for fresh random inputs on every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;
use toml::Spanned;

use crate::expr::{
    self, Env, EvalError, Function, Generator, HexPart, Limit, PatternPart, Template, Value,
};
use crate::hash::Sha256Hex;
use crate::spec::{ASSERTION_EXIT_CODE, Behavior, BehaviorId, Spec, Syscall};
use crate::supply::Supply;

/// The most runs a generated block may ask for.
pub const MAX_RUNS: u32 = 1000;

/// Runs of a generated block that doesn't say.
pub const DEFAULT_RUNS: u32 = 100;

/// At most this many problems are reported at once.
const MAX_PROBLEMS: usize = 20;

/// The built-in variable for the time a run starts, in Unix seconds, when a block declares no
/// variable of that name.
const NOW: &str = "now";

/// The built-in variable for the port of the test's own server, when a block declares no
/// variable of that name. It renders as [`PORT_TEXT`], which the runner fills in.
const PORT: &str = "port";

/// Where a test names its own server's port: the runner writes the real one in its place.
pub const PORT_TEXT: &str = "{port}";

/// A checked tests.toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSuite {
    /// One example per behavior, in file order.
    pub examples: Vec<Example>,
    /// The generated blocks, in file order.
    pub generated: Vec<Generated>,
}

/// A test with fixed values taken from the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// The behavior it checks.
    pub id: BehaviorId,
    /// Line of its `[[example]]` table.
    pub line: usize,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Standard input.
    pub stdin: String,
    /// Files in the working folder before the run, by name.
    pub files: BTreeMap<String, String>,
    /// Expected standard output.
    pub stdout: String,
    /// Expected standard error.
    pub stderr: String,
    /// Files expected in the working folder after the run, by name, with their exact contents.
    pub files_after: BTreeMap<String, String>,
    /// Expected exit code.
    pub exit_code: u8,
    /// The bytes its `getrandom` calls receive, in hex, or `None` when the kernel draws them.
    pub random: Option<String>,
    /// What its `clock_gettime` calls read, in seconds; empty when the kernel's clock answers.
    pub clock: Vec<String>,
    /// The test's own server, one entry a connection, for a program that connects.
    pub server: Vec<Reply>,
    /// The test's own client, one entry a connection, for a program that serves.
    pub client: Vec<Exchange>,
    /// Whether the program must still be serving at the end, instead of exiting.
    pub serves: bool,
}

/// What the test's own server does with one connection from the program: it reads exactly
/// `receives`, sends `replies`, closes its side, and expects nothing more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// What the program must send.
    pub receives: String,
    /// What the server sends back.
    pub replies: String,
}

/// What the test's own client does on one connection to the program: it sends `sends`, closes
/// its side, and reads everything the program answers, which must be `receives`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    /// What the client sends.
    pub sends: String,
    /// What the program must answer, with any rules.
    pub receives: String,
}

impl Example {
    /// What the test supplies in place of the kernel.
    #[must_use]
    pub fn supply(&self) -> Supply {
        Supply::parse(self.random.as_deref(), &self.clock).unwrap_or_default()
    }
}

/// A test that draws new inputs on every run and computes the expected results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The behavior it checks.
    pub id: BehaviorId,
    /// Line of its `[[generated]]` table.
    pub line: usize,
    /// How many runs: boundary values first, then random ones.
    pub runs: u32,
    /// The variables and their generators, in name order.
    pub vars: Vec<(String, Generator)>,
    /// Argument templates.
    pub args: Vec<Template>,
    /// Standard input template.
    pub stdin: Template,
    /// Templates of the files in the working folder before the run, by name.
    pub files: BTreeMap<String, Template>,
    /// Expected standard output template.
    pub stdout: Template,
    /// Expected standard error template.
    pub stderr: Template,
    /// Templates of the files expected after the run, by name.
    pub files_after: BTreeMap<String, Template>,
    /// Expected exit code.
    pub exit_code: ExitCode,
    /// Whether every combination of the variables' values runs, instead of a sample.
    pub every_value: bool,
    /// The random bytes template: hex digits and `hex(x, n)`.
    pub random: Option<Template>,
    /// The clock reading templates.
    pub clock: Vec<Template>,
    /// The test server's templates, one pair a connection: what it receives, what it replies.
    pub server: Vec<(Template, Template)>,
    /// The test client's templates, one pair a connection: what it sends, what it receives.
    pub client: Vec<(Template, Template)>,
    /// Whether the program must still be serving at the end, instead of exiting.
    pub serves: bool,
}

/// A generated block runs every combination of its values when there are at most this many…
const EVERY_VALUE_MAX: u128 = 5000;

/// …and the whole suite stays within this many runs, about 30 seconds of tests.
const SUITE_RUNS_MAX: u128 = 20_000;

/// A generated block's expected exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitCode {
    /// The same code on every run.
    Fixed(u8),
    /// A formula such as `"{if(a < 0, 1, 0)}"`.
    Formula(Template),
}

/// One run of a generated block: concrete inputs and the results they should produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// The variables' values.
    pub inputs: Env,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Standard input.
    pub stdin: String,
    /// Files in the working folder before the run.
    pub files: BTreeMap<String, String>,
    /// Expected standard output.
    pub stdout: String,
    /// Expected standard error.
    pub stderr: String,
    /// Files expected after the run.
    pub files_after: BTreeMap<String, String>,
    /// Expected exit code.
    pub exit_code: u8,
    /// The bytes its `getrandom` calls receive, in hex, or `None` when the kernel draws them.
    pub random: Option<String>,
    /// What its `clock_gettime` calls read, in seconds; empty when the kernel's clock answers.
    pub clock: Vec<String>,
    /// The test's own server, one entry a connection.
    pub server: Vec<Reply>,
    /// The test's own client, one entry a connection.
    pub client: Vec<Exchange>,
    /// Whether the program must still be serving at the end, instead of exiting.
    pub serves: bool,
}

impl Case {
    /// What the test supplies in place of the kernel.
    #[must_use]
    pub fn supply(&self) -> Supply {
        Supply::parse(self.random.as_deref(), &self.clock).unwrap_or_default()
    }
}

impl Generated {
    /// Whether the block uses `now`, the time a run starts, which it may when it declares no
    /// variable of that name.
    #[must_use]
    pub fn uses_now(&self) -> bool {
        !self.vars.iter().any(|(name, _)| name == NOW)
            && self
                .outputs()
                .any(|template| template.variables().contains(NOW))
    }

    /// The variables that reach the program only as random bytes, with the byte counts
    /// `hex(x, n)` gives them.
    #[must_use]
    pub fn random_only(&self) -> BTreeMap<String, BTreeSet<usize>> {
        let others = self.reached_otherwise(true);
        let mut random: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for part in self
            .random
            .as_ref()
            .and_then(Template::hex_parts)
            .unwrap_or_default()
        {
            if let HexPart::Var(var, count) = part
                && !others.contains(&var)
            {
                random.entry(var).or_default().insert(count);
            }
        }
        random
    }

    /// The variables that reach the program only as clock readings.
    #[must_use]
    pub fn clock_only(&self) -> BTreeSet<String> {
        let others = self.reached_otherwise(false);
        let random = self
            .random
            .as_ref()
            .map(Template::variables)
            .unwrap_or_default();
        self.clock
            .iter()
            .flat_map(Template::variables)
            .filter(|var| !others.contains(var) && !random.contains(var))
            .collect()
    }

    /// The variables in args, stdin, files and what the test's server and client send, and in
    /// clock readings unless `clock` is false.
    fn reached_otherwise(&self, clock: bool) -> BTreeSet<String> {
        self.args
            .iter()
            .chain([&self.stdin])
            .chain(self.files.values())
            .chain(self.clock.iter().filter(|_| clock))
            .chain(self.server.iter().map(|(_, replies)| replies))
            .chain(self.client.iter().map(|(sends, _)| sends))
            .flat_map(Template::variables)
            .collect()
    }

    /// The templates of the expected results: what the program prints, writes, exits with and
    /// sends over the network.
    fn outputs(&self) -> impl Iterator<Item = &Template> {
        let formula = match &self.exit_code {
            ExitCode::Formula(template) => Some(template),
            ExitCode::Fixed(_) => None,
        };
        [&self.stdout, &self.stderr]
            .into_iter()
            .chain(self.files_after.values())
            .chain(formula)
            .chain(self.server.iter().map(|(receives, _)| receives))
            .chain(self.client.iter().map(|(_, receives)| receives))
    }

    /// The templates of what the test gives the program: arguments, input, files, clock
    /// readings, and what its own server replies and its own client sends.
    fn given(&self) -> impl Iterator<Item = &Template> {
        self.args
            .iter()
            .chain([&self.stdin])
            .chain(self.files.values())
            .chain(self.clock.iter())
            .chain(self.server.iter().map(|(_, replies)| replies))
            .chain(self.client.iter().map(|(sends, _)| sends))
    }

    /// The expected results for `inputs`, for a run that starts at `now` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Fails when a formula can't be evaluated or gives an impossible exit code.
    pub fn case(&self, inputs: Env, now: i128) -> Result<Case, (Env, &'static str, EvalError)> {
        match self.results(&inputs, now) {
            Ok(case) => Ok(Case { inputs, ..case }),
            Err((field, error)) => Err((inputs, field, error)),
        }
    }

    /// The rendered args, stdin, files, expected outputs and exit code for `inputs`, with no
    /// inputs recorded.
    fn results(&self, inputs: &Env, now: i128) -> Result<Case, (&'static str, EvalError)> {
        let mut with_built_ins = inputs.clone();
        if !inputs.contains_key(NOW) {
            with_built_ins.insert(NOW.to_owned(), Value::Int(now));
        }
        if !inputs.contains_key(PORT) {
            with_built_ins.insert(PORT.to_owned(), Value::Str(PORT_TEXT.to_owned()));
        }
        let inputs = &with_built_ins;
        let render_files = |files: &BTreeMap<String, Template>, field| {
            files
                .iter()
                .map(|(name, template)| Ok((name.clone(), template.render(inputs)?)))
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map_err(|error| (field, error))
        };
        let args = self
            .args
            .iter()
            .map(|arg| arg.render(inputs))
            .collect::<Result<_, _>>()
            .map_err(|error| ("args", error))?;
        let stdin = self
            .stdin
            .render(inputs)
            .map_err(|error| ("stdin", error))?;
        let files = render_files(&self.files, "files")?;
        let stdout = self
            .stdout
            .render(inputs)
            .map_err(|error| ("stdout", error))?;
        let stderr = self
            .stderr
            .render(inputs)
            .map_err(|error| ("stderr", error))?;
        let files_after = render_files(&self.files_after, "files_after")?;
        let crowded = [("stdout", &stdout), ("stderr", &stderr)]
            .into_iter()
            .chain(files_after.values().map(|text| ("files_after", text)))
            .find(|(_, text)| crate::expect::crowded(text));
        if let Some((field, _)) = crowded {
            return Err((field, EvalError::Type(expr::CROWDED.into())));
        }
        let exit_code = match &self.exit_code {
            ExitCode::Fixed(code) => *code,
            ExitCode::Formula(template) => {
                exit_code_value(template, inputs).map_err(|error| ("exit_code", error))?
            }
        };
        let random = self
            .random
            .as_ref()
            .map(|template| template.render(inputs))
            .transpose()
            .map_err(|error| ("random", error))?;
        let clock = self
            .clock
            .iter()
            .map(|template| template.render(inputs))
            .collect::<Result<_, _>>()
            .map_err(|error| ("clock", error))?;
        let server = self
            .server
            .iter()
            .map(|(receives, replies)| {
                Ok(Reply {
                    receives: receives.render(inputs)?,
                    replies: replies.render(inputs)?,
                })
            })
            .collect::<Result<_, EvalError>>()
            .map_err(|error| ("server", error))?;
        let client = self
            .client
            .iter()
            .map(|(sends, receives)| {
                Ok(Exchange {
                    sends: sends.render(inputs)?,
                    receives: receives.render(inputs)?,
                })
            })
            .collect::<Result<_, EvalError>>()
            .map_err(|error| ("client", error))?;
        Ok(Case {
            inputs: Env::new(),
            args,
            stdin,
            files,
            stdout,
            stderr,
            files_after,
            exit_code,
            random,
            clock,
            server,
            client,
            serves: self.serves,
        })
    }

    /// Every run for `seed`: boundary values first, then random ones.
    ///
    /// # Errors
    ///
    /// Returns the first inputs whose results can't be computed.
    pub fn cases(
        &self,
        seed: u64,
    ) -> impl ExactSizeIterator<Item = Result<Case, (Env, &'static str, EvalError)>> + '_ {
        let inputs = if self.every_value {
            expr::every_case(&self.vars)
        } else {
            expr::cases(&self.vars, self.run_count(), seed)
        };
        inputs.map(|inputs| self.case(inputs, crate::expect::now()))
    }

    /// How many runs the block makes: every combination when [`Self::every_value`], otherwise
    /// `runs`.
    #[must_use]
    pub fn run_count(&self) -> usize {
        if self.every_value {
            expr::combinations(&self.vars)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0)
        } else {
            usize::try_from(self.runs).unwrap_or(usize::MAX)
        }
    }

    /// When every combination runs, says so in plain words: "every value of n in its range (all
    /// 201)".
    #[must_use]
    pub fn every_value_text(&self) -> Option<String> {
        if !self.every_value {
            return None;
        }
        let names: Vec<&str> = self.vars.iter().map(|(name, _)| name.as_str()).collect();
        let count = self.run_count();
        Some(match names.as_slice() {
            [name] => format!("every value of {name} in its range (all {count})"),
            [rest @ .., last] => format!(
                "every combination of {} and {last} in their ranges (all {count})",
                rest.join(", ")
            ),
            [] => format!("all {count} runs"),
        })
    }
}

fn exit_code_value(template: &Template, inputs: &Env) -> Result<u8, EvalError> {
    match template.value(inputs)? {
        Value::Int(code) => match u8::try_from(code) {
            Ok(ASSERTION_EXIT_CODE) => Err(EvalError::Type(format!(
                "gives exit code {ASSERTION_EXIT_CODE}, which is reserved for failed assertions"
            ))),
            Ok(code) => Ok(code),
            Err(_) => Err(EvalError::Type(format!(
                "gives exit code {code}; exit codes go from 0 to 255"
            ))),
        },
        other => Err(EvalError::Type(format!(
            "must give a number for the exit code, not {other}"
        ))),
    }
}

/// Marks the generated blocks that run every combination of their values: those with at most
/// [`EVERY_VALUE_MAX`] combinations, smallest first, while the suite stays within
/// [`SUITE_RUNS_MAX`] runs. A block with `choice(…)` never is: its values are hand-picked, not a
/// range.
fn mark_every_value(blocks: &mut [Generated]) {
    let mut candidates: Vec<(u128, usize)> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            !block
                .vars
                .iter()
                .any(|(_, generator)| matches!(generator, Generator::Choice(_)))
        })
        .filter_map(|(index, block)| {
            let count = expr::combinations(&block.vars)?;
            (count <= EVERY_VALUE_MAX).then_some((count, index))
        })
        .collect();
    candidates.sort_unstable();
    let mut total: u128 = blocks.iter().map(|block| u128::from(block.runs)).sum();
    for (count, index) in candidates {
        let next = total - u128::from(blocks[index].runs) + count;
        if next <= SUITE_RUNS_MAX {
            blocks[index].every_value = true;
            total = next;
        }
    }
}

/// The most files a test may give the program or expect from it.
pub const MAX_FILES: usize = 16;

/// The most bytes a test's file may hold.
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

/// Why `name` can't name a test's file, if it can't: files are named relative to the program's
/// working folder and stay inside it.
fn file_name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("a file name can't be empty".into());
    }
    if name.starts_with('/') {
        return Some(format!(
            "`{name}` is an absolute path; name files relative to the working folder, such as \
             `notes.txt`"
        ));
    }
    if name.chars().any(char::is_control) || name.len() > 255 {
        return Some(format!(
            "{name:?} isn't a usable file name: keep it under 256 bytes, with no control \
             characters"
        ));
    }
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Some(format!(
            "`{name}` must stay inside the working folder: no `..`, `.` or empty parts"
        ));
    }
    None
}

/// Problems with a test's file names: bad names, too many files, or a name used both as a file
/// and as a folder.
fn file_set_problems<'n>(names: impl Iterator<Item = &'n String>) -> Vec<String> {
    let names: BTreeSet<&String> = names.collect();
    let mut problems: Vec<String> = names
        .iter()
        .filter_map(|name| file_name_problem(name))
        .collect();
    if names.len() > MAX_FILES {
        problems.push(format!(
            "it names {} files; a test gives and expects at most {MAX_FILES}",
            names.len()
        ));
    }
    for name in &names {
        if let Some(inside) = names
            .iter()
            .find(|other| other.starts_with(&format!("{name}/")))
        {
            problems.push(format!(
                "`{name}` is a file, so `{inside}` can't be inside it"
            ));
        }
    }
    problems
}

/// Linux refuses to start a program with an argument longer than this.
pub const MAX_ARG_BYTES: usize = 128 * 1024;

/// lengua keeps a program's arguments together under this, well inside Linux's limit.
pub const MAX_ARGS_BYTES: usize = 1024 * 1024;

/// Why Linux couldn't start a run with these arguments, if it couldn't.
fn oversized_args(case: &Case) -> Option<String> {
    let total: usize = case.args.iter().map(String::len).sum();
    if let Some((index, arg)) = case
        .args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.len() > MAX_ARG_BYTES)
    {
        return Some(format!(
            "with {}, argument {} is {} bytes, but Linux limits each argument to {MAX_ARG_BYTES} \
             bytes; narrow the range, or pass long text on standard input",
            describe_inputs(&case.inputs),
            index + 1,
            arg.len()
        ));
    }
    (total > MAX_ARGS_BYTES).then(|| {
        format!(
            "with {}, the arguments total {total} bytes, over lengua's limit of {MAX_ARGS_BYTES}; \
             pass long text on standard input",
            describe_inputs(&case.inputs)
        )
    })
}

/// Shows variable values as `a = 2, s = "x"`.
#[must_use]
pub fn describe_inputs(inputs: &Env) -> String {
    inputs
        .iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A problem found in tests.toml.
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestsFile {
    meta: Spanned<Meta>,
    #[serde(default)]
    example: Vec<Spanned<ExampleTable>>,
    #[serde(default)]
    generated: Vec<Spanned<GeneratedTable>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    spec: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleTable {
    id: String,
    #[serde(default)]
    args: Vec<Text>,
    #[serde(default)]
    stdin: Text,
    #[serde(default)]
    files: BTreeMap<String, Text>,
    #[serde(default)]
    stdout: Text,
    #[serde(default)]
    stderr: Text,
    #[serde(default)]
    files_after: BTreeMap<String, Text>,
    exit_code: Option<i64>,
    random: Option<String>,
    #[serde(default)]
    clock: Vec<String>,
    server: Option<ServerField<Text>>,
    #[serde(default)]
    client: Vec<ClientTable<Text>>,
    #[serde(default)]
    serves: bool,
}

/// The test's own server: one connection's script, or a list of them.
#[derive(Debug, Deserialize)]
#[serde(untagged, bound(deserialize = "T: Deserialize<'de> + Default"))]
enum ServerField<T> {
    One(ServerTable<T>),
    Many(Vec<ServerTable<T>>),
}

impl<T> ServerField<T> {
    fn into_vec(self) -> Vec<ServerTable<T>> {
        match self {
            Self::One(table) => vec![table],
            Self::Many(tables) => tables,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de> + Default")
)]
struct ServerTable<T> {
    #[serde(default)]
    receives: T,
    #[serde(default)]
    replies: T,
}

#[derive(Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de> + Default")
)]
struct ClientTable<T> {
    #[serde(default)]
    sends: T,
    #[serde(default)]
    receives: T,
}

/// An example's literal text: a string, or a list of strings and repeated runs, so a long input
/// such as a file one byte over a limit stays short to write: `[{ repeat = "a", times = 1048576
/// }, "x"]`. An expected output may instead be a rule, `{ matches = "rolled {any(1, 6)}\n" }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Text {
    Plain(String),
    Parts(Vec<TextPart>),
    Rule(Rule),
}

/// Expected output that allows a range of numbers where the program chooses one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    matches: String,
}

impl Default for Text {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TextPart {
    Plain(String),
    Repeat(Repeat),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Repeat {
    repeat: String,
    times: u64,
}

impl Text {
    /// Expected output: the text written out, or a rule's text with its holes.
    fn expected(self) -> Result<String, String> {
        match self {
            Self::Rule(Rule { matches }) => Template::parse(&matches)
                .map_err(|error| format!("matches = {matches:?}: {error}"))?
                .rule(),
            text => text.expand(),
        }
    }

    /// The text written out, up to [`MAX_FILE_BYTES`].
    fn expand(self) -> Result<String, String> {
        let parts = match self {
            Self::Plain(text) => return Ok(text),
            Self::Parts(parts) => parts,
            Self::Rule(_) => {
                return Err(
                    "a rule such as { matches = \"{any(1, 6)}\" } goes only in stdout, stderr \
                     and files_after; the test's inputs are exact"
                        .into(),
                );
            }
        };
        let mut text = String::new();
        for part in parts {
            match part {
                TextPart::Plain(plain) => text.push_str(&plain),
                TextPart::Repeat(Repeat { repeat, times }) => {
                    let bytes = usize::try_from(times)
                        .ok()
                        .and_then(|times| repeat.len().checked_mul(times))
                        .filter(|bytes| text.len() + bytes <= MAX_FILE_BYTES);
                    if repeat.is_empty() || bytes.is_none() {
                        return Err(format!(
                            "{{ repeat = {repeat:?}, times = {times} }} must repeat some text, \
                             and the whole value must stay within {MAX_FILE_BYTES} bytes"
                        ));
                    }
                    text.push_str(&repeat.repeat(usize::try_from(times).unwrap_or(0)));
                }
            }
            if text.len() > MAX_FILE_BYTES {
                return Err(format!("it holds more than {MAX_FILE_BYTES} bytes"));
            }
        }
        Ok(text)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedTable {
    id: String,
    runs: Option<i64>,
    vars: BTreeMap<String, String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    stdin: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    files_after: BTreeMap<String, String>,
    exit_code: Option<ExitCodeField>,
    random: Option<String>,
    #[serde(default)]
    clock: Vec<String>,
    server: Option<ServerField<String>>,
    #[serde(default)]
    client: Vec<ClientTable<String>>,
    #[serde(default)]
    serves: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ExitCodeField {
    Number(i64),
    Formula(String),
}

/// Checks tests.toml against the recorded spec, reporting every problem found (up to 20).
///
/// `spec_label` is `sha256:<hex>` of the recorded spec.md.
///
/// # Errors
///
/// Returns the problems when the tests aren't valid.
pub fn check(text: &str, spec: &Spec, spec_label: &str) -> Result<TestSuite, Vec<Problem>> {
    let mut checker = Checker {
        text,
        spec,
        problems: Vec::new(),
        hidden: 0,
    };
    let file: TestsFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(error) => {
            checker.problem(
                None,
                format!("tests.toml isn't valid: {}", error.to_string().trim()),
            );
            return Err(checker.problems);
        }
    };
    if let Ok(table) = toml::from_str::<toml::Table>(text)
        && holds_markers(&toml::Value::Table(table))
    {
        checker.problem(
            None,
            "tests.toml holds one of the characters U+FDD0, U+FDD1 and U+FDD2, which lengua \
             reserves for marking rules such as {any(1, 6)}; remove them",
        );
        return Err(checker.problems);
    }
    let meta_line = checker.line(file.meta.span().start);
    if file.meta.get_ref().spec != spec_label {
        checker.problem(
            Some(meta_line),
            format!(
                "[meta] says spec = {:?}, but the recorded spec is {spec_label:?}. Write the \
                 tests from the current spec.md, then set spec = {spec_label:?}",
                file.meta.get_ref().spec
            ),
        );
    }
    let examples = checker.examples(file.example);
    let seed = dry_run_seed(text);
    let generated = checker.generated_blocks(file.generated, seed);
    checker.coverage(&examples, &generated);
    checker.formulas(&examples, &generated);
    if checker.problems.is_empty() {
        Ok(TestSuite {
            examples,
            generated,
        })
    } else {
        if checker.hidden > 0 {
            checker.problems.push(Problem {
                line: None,
                message: format!("…and {} more problems", checker.hidden),
            });
        }
        Err(checker.problems)
    }
}

/// A spec's backtick value with `\r`, `\n`, `\t` and `\\` read as the characters they name, as
/// people write line breaks and tabs in a protocol.
fn unescape(written: &str) -> String {
    let mut text = String::with_capacity(written.len());
    let mut characters = written.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            text.push(character);
            continue;
        }
        match characters.next() {
            Some('r') => text.push('\r'),
            Some('n') => text.push('\n'),
            Some('t') => text.push('\t'),
            Some('\\') => text.push('\\'),
            Some(other) => {
                text.push('\\');
                text.push(other);
            }
            None => text.push('\\'),
        }
    }
    text
}

/// Whether any string in `value`, or any key, holds a character lengua uses to mark holes.
fn holds_markers(value: &toml::Value) -> bool {
    match value {
        toml::Value::String(text) => crate::expect::has_holes(text),
        toml::Value::Array(values) => values.iter().any(holds_markers),
        toml::Value::Table(table) => table
            .iter()
            .any(|(key, value)| crate::expect::has_holes(key) || holds_markers(value)),
        _ => false,
    }
}

/// The seed of the dry run, fixed by the file's content so `check` always gives the same verdict.
fn dry_run_seed(text: &str) -> u64 {
    let digest = Sha256Hex::of(text.as_bytes());
    u64::from_str_radix(&digest.as_str()[..16], 16).unwrap_or(0)
}

struct Checker<'a> {
    text: &'a str,
    spec: &'a Spec,
    problems: Vec<Problem>,
    hidden: usize,
}

impl<'a> Checker<'a> {
    fn problem(&mut self, line: Option<usize>, message: impl Into<String>) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(Problem {
                line,
                message: message.into(),
            });
        } else {
            self.hidden += 1;
        }
    }

    fn line(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .bytes()
            .filter(|&byte| byte == b'\n')
            .count()
            + 1
    }

    /// The behavior `id` names, reporting unknown IDs.
    fn behavior(&mut self, id: &str, line: usize, table: &str) -> Option<&'a Behavior> {
        let spec = self.spec;
        let found = BehaviorId::parse(id).and_then(|id| spec.behavior(id));
        if found.is_none() {
            let known: Vec<String> = spec
                .behaviors
                .iter()
                .map(|behavior| behavior.id.to_string())
                .collect();
            self.problem(
                Some(line),
                format!(
                    "{table} has id = {id:?}, which isn't a behavior in spec.md (they are {})",
                    known.join(", ")
                ),
            );
        }
        found
    }

    fn examples(&mut self, tables: Vec<Spanned<ExampleTable>>) -> Vec<Example> {
        let mut examples: Vec<Example> = Vec::new();
        for table in tables {
            let line = self.line(table.span().start);
            let table = table.into_inner();
            let Some(behavior) = self.behavior(&table.id, line, "[[example]]") else {
                continue;
            };
            let id = behavior.id;
            let expected_exit = behavior.exit_code;
            let values: Vec<String> = behavior.values().map(str::to_owned).collect();
            if let Some(first) = examples.iter().find(|example| example.id == id) {
                let first = first.line;
                self.problem(
                    Some(line),
                    format!("{id} has two examples (also on line {first}); keep one per behavior"),
                );
                continue;
            }
            let mut expand = |field: &str, text: Text, output: bool| {
                let text = if output {
                    text.expected()
                } else {
                    text.expand()
                };
                text.unwrap_or_else(|problem| {
                    self.problem(Some(line), format!("example {id}: {field}: {problem}"));
                    String::new()
                })
            };
            let args: Vec<String> = table
                .args
                .into_iter()
                .map(|arg| expand("args", arg, false))
                .collect();
            let stdin = expand("stdin", table.stdin, false);
            let stdout = expand("stdout", table.stdout, true);
            let stderr = expand("stderr", table.stderr, true);
            let files: BTreeMap<String, String> = table
                .files
                .into_iter()
                .map(|(name, contents)| {
                    let contents = expand(&format!("files.{name}"), contents, false);
                    (name, contents)
                })
                .collect();
            let files_after: BTreeMap<String, String> = table
                .files_after
                .into_iter()
                .map(|(name, contents)| {
                    let contents = expand(&format!("files_after.{name}"), contents, true);
                    (name, contents)
                })
                .collect();
            // What the test's server and client send is exact; what the program answers its client
            // may hold rules. Long ones may be written with repeats, like any example text.
            let server: Vec<Reply> = table
                .server
                .map(ServerField::into_vec)
                .unwrap_or_default()
                .into_iter()
                .map(|table| Reply {
                    receives: expand("server.receives", table.receives, false),
                    replies: expand("server.replies", table.replies, false),
                })
                .collect();
            let client: Vec<Exchange> = table
                .client
                .into_iter()
                .map(|table| Exchange {
                    sends: expand("client.sends", table.sends, false),
                    receives: expand("client.receives", table.receives, true),
                })
                .collect();
            let name = format!("example {id}");
            let Some(written_exit) = self.check_ending(
                line,
                &name,
                behavior,
                table.exit_code.is_some(),
                table.serves,
            ) else {
                continue;
            };
            let written_exit = table.exit_code.unwrap_or(written_exit);
            let exit_code = match u8::try_from(written_exit) {
                Ok(ASSERTION_EXIT_CODE) => {
                    self.problem(
                        Some(line),
                        format!(
                            "example {id} expects exit code {ASSERTION_EXIT_CODE}, which is \
                             reserved for failed assertions"
                        ),
                    );
                    continue;
                }
                Ok(code) => code,
                Err(_) => {
                    self.problem(
                        Some(line),
                        format!("example {id}: exit codes go from 0 to 255, not {written_exit}"),
                    );
                    continue;
                }
            };
            if exit_code != expected_exit {
                self.problem(
                    Some(line),
                    format!(
                        "example {id} expects exit code {exit_code}, but the spec says {id} \
                         exits with code {expected_exit}"
                    ),
                );
            }
            for problem in file_set_problems(files.keys().chain(files_after.keys())) {
                self.problem(Some(line), format!("example {id}: {problem}"));
            }
            self.check_supply(line, &name, table.random.is_some(), !table.clock.is_empty());
            if let Err(problem) = Supply::parse(table.random.as_deref(), &table.clock) {
                self.problem(Some(line), format!("{name}: {problem}"));
            }
            self.check_peers(
                line,
                &name,
                !server.is_empty(),
                !client.is_empty() || table.serves,
            );
            let uses_port = args
                .iter()
                .chain([&stdin])
                .chain(files.values())
                .any(|text| text.contains(PORT_TEXT));
            if uses_port && server.is_empty() {
                self.problem(
                    Some(line),
                    format!(
                        "{name} names {PORT_TEXT}, the port of the test's own server, but has no \
                         server; add server = {{ receives = \"…\", replies = \"…\" }}"
                    ),
                );
            }
            let example = Example {
                id,
                line,
                args,
                stdin,
                files,
                stdout,
                stderr,
                files_after,
                exit_code,
                random: table.random,
                clock: table.clock,
                server,
                client,
                serves: table.serves,
            };
            let network_test =
                !example.server.is_empty() || !example.client.is_empty() || example.serves;
            let network_ports: Vec<String> = self
                .spec
                .network
                .connects
                .iter()
                .chain(&self.spec.network.listens)
                .map(u16::to_string)
                .collect();
            for written in values {
                // `\r\n` in a spec's backticks means the line break a protocol needs.
                let value = unescape(&written);
                let in_files = |files: &BTreeMap<String, String>| {
                    files
                        .iter()
                        .any(|(name, contents)| name.contains(&value) || contents.contains(&value))
                };
                let present = example.args.contains(&value)
                    || example.stdin.contains(&value)
                    || example.stdout.contains(&value)
                    || example.stderr.contains(&value)
                    || in_files(&example.files)
                    || in_files(&example.files_after)
                    || example.exit_code.to_string() == value
                    || example
                        .random
                        .as_deref()
                        .is_some_and(|random| random.contains(&value))
                    || example.clock.iter().any(|reading| reading.contains(&value))
                    || example.server.iter().any(|reply| {
                        reply.receives.contains(&value) || reply.replies.contains(&value)
                    })
                    || example.client.iter().any(|exchange| {
                        exchange.sends.contains(&value) || exchange.receives.contains(&value)
                    })
                    // The spec's own ports: a test talks over a free port of its own instead.
                    || (network_test && network_ports.contains(&value));
                if !present {
                    self.problem(
                        Some(line),
                        format!(
                            "example {id} doesn't contain the spec's value `{written}`; copy every \
                             backtick value of {id} into its args, stdin, files, stdout, stderr, \
                             files_after, random, clock or exit_code"
                        ),
                    );
                }
            }
            examples.push(example);
        }
        let missing: Vec<String> = self
            .spec
            .behaviors
            .iter()
            .filter(|behavior| !examples.iter().any(|example| example.id == behavior.id))
            .map(|behavior| behavior.id.to_string())
            .collect();
        for id in missing {
            self.problem(
                None,
                format!("{id} has no [[example]]; add one with the values from its spec steps"),
            );
        }
        examples
    }

    fn generated_blocks(
        &mut self,
        tables: Vec<Spanned<GeneratedTable>>,
        seed: u64,
    ) -> Vec<Generated> {
        let mut blocks = Vec::new();
        for table in tables {
            let line = self.line(table.span().start);
            if let Some(block) = self.generated(line, table.into_inner()) {
                blocks.push(block);
            }
        }
        mark_every_value(&mut blocks);
        for block in &blocks {
            self.dry_run(block, seed);
        }
        blocks
    }

    fn generated(&mut self, line: usize, table: GeneratedTable) -> Option<Generated> {
        let behavior = self.behavior(&table.id, line, "[[generated]]")?;
        let id = behavior.id;
        let behavior_exit = behavior.exit_code;
        let name = format!("generated {id}");
        let before = self.problems.len() + self.hidden;
        let runs = match table.runs.map(u32::try_from) {
            None => DEFAULT_RUNS,
            Some(Ok(runs)) if (1..=MAX_RUNS).contains(&runs) => runs,
            Some(_) => {
                self.problem(
                    Some(line),
                    format!("{name}: runs must be from 1 to {MAX_RUNS}"),
                );
                DEFAULT_RUNS
            }
        };
        if table.vars.is_empty() {
            self.problem(
                Some(line),
                format!("{name} has no vars; declare the inputs it draws, such as vars = {{ a = \"int(-10, 10)\" }}"),
            );
        }
        let mut vars = Vec::new();
        for (var, source) in &table.vars {
            if !expr::is_variable_name(var) {
                self.problem(
                    Some(line),
                    format!(
                        "{name}: `{var}` can't be a variable name; use lowercase letters, digits \
                         and _, not a function name"
                    ),
                );
                continue;
            }
            match Generator::parse(source) {
                Ok(generator) => vars.push((var.clone(), generator)),
                Err(error) => self.problem(Some(line), format!("{name}: vars.{var}: {error}")),
            }
        }
        let mut template = |field: &str, source: &str| match Template::parse(source) {
            Ok(template) => Some(template),
            Err(error) => {
                self.problem(Some(line), format!("{name}: {field}: {error}"));
                None
            }
        };
        let args: Vec<Option<Template>> =
            table.args.iter().map(|arg| template("args", arg)).collect();
        let stdin = template("stdin", &table.stdin);
        let files: Option<BTreeMap<String, Template>> = table
            .files
            .iter()
            .map(|(file, source)| Some((file.clone(), template("files", source)?)))
            .collect();
        let stdout = template("stdout", &table.stdout);
        let stderr = template("stderr", &table.stderr);
        let files_after: Option<BTreeMap<String, Template>> = table
            .files_after
            .iter()
            .map(|(file, source)| Some((file.clone(), template("files_after", source)?)))
            .collect();
        let random = match &table.random {
            None => Some(None),
            Some(source) => template("random", source).map(Some),
        };
        let clock: Option<Vec<Template>> = table
            .clock
            .iter()
            .map(|source| template("clock", source))
            .collect();
        let server: Option<Vec<(Template, Template)>> = table
            .server
            .map(ServerField::into_vec)
            .unwrap_or_default()
            .iter()
            .map(|reply| {
                Some((
                    template("server.receives", &reply.receives)?,
                    template("server.replies", &reply.replies)?,
                ))
            })
            .collect();
        let client: Option<Vec<(Template, Template)>> = table
            .client
            .iter()
            .map(|exchange| {
                Some((
                    template("client.sends", &exchange.sends)?,
                    template("client.receives", &exchange.receives)?,
                ))
            })
            .collect();
        for problem in file_set_problems(table.files.keys().chain(table.files_after.keys())) {
            self.problem(Some(line), format!("{name}: {problem}"));
        }
        self.check_supply(line, &name, table.random.is_some(), !table.clock.is_empty());
        self.check_peers(
            line,
            &name,
            server.as_ref().is_some_and(|server| !server.is_empty()),
            client.as_ref().is_some_and(|client| !client.is_empty()) || table.serves,
        );
        self.check_ending(
            line,
            &name,
            behavior,
            table.exit_code.is_some(),
            table.serves,
        )?;
        let exit_field = table
            .exit_code
            .unwrap_or(ExitCodeField::Number(i64::from(behavior_exit)));
        let exit_code = match &exit_field {
            ExitCodeField::Number(code) => match u8::try_from(*code) {
                Ok(code) if code == behavior_exit => Some(ExitCode::Fixed(code)),
                Ok(code) => {
                    self.problem(
                        Some(line),
                        format!(
                            "{name} expects exit code {code}, but the spec says {id} exits with \
                             code {behavior_exit}"
                        ),
                    );
                    None
                }
                Err(_) => {
                    self.problem(
                        Some(line),
                        format!("{name}: exit codes go from 0 to 255, not {code}"),
                    );
                    None
                }
            },
            ExitCodeField::Formula(source) => match Template::parse(source) {
                Ok(template) => Some(ExitCode::Formula(template)),
                Err(error) => {
                    self.problem(Some(line), format!("{name}: exit_code: {error}"));
                    None
                }
            },
        };
        let (
            Some(args),
            Some(stdin),
            Some(files),
            Some(stdout),
            Some(stderr),
            Some(files_after),
            Some(exit_code),
            Some(random),
            Some(clock),
            Some(server),
            Some(client),
        ) = (
            args.into_iter().collect::<Option<Vec<_>>>(),
            stdin,
            files,
            stdout,
            stderr,
            files_after,
            exit_code,
            random,
            clock,
            server,
            client,
        )
        else {
            return None;
        };
        let block = Generated {
            id,
            line,
            runs,
            vars,
            args,
            stdin,
            files,
            stdout,
            stderr,
            files_after,
            exit_code,
            every_value: false,
            random,
            clock,
            server,
            client,
            serves: table.serves,
        };
        self.check_variables(&block, &name);
        self.check_ranges(&block, &name);
        (self.problems.len() + self.hidden == before).then_some(block)
    }

    /// A test has its own server only for a program that connects, and its own client, or
    /// expects the program to keep serving, only for one that serves.
    fn check_peers(&mut self, line: usize, name: &str, server: bool, client: bool) {
        if server && self.spec.network.connects.is_empty() {
            self.problem(
                Some(line),
                format!(
                    "{name} has a server for the program to connect to, but spec.md doesn't say \
                     `- Network: connects to TCP port N`"
                ),
            );
        }
        if client && self.spec.network.listens.is_none() {
            self.problem(
                Some(line),
                format!(
                    "{name} has a client or serves = true, but spec.md doesn't say `- Network: \
                     listens on TCP port N`"
                ),
            );
        }
    }

    /// A test ends as its behavior does: with the exit code the spec states, or, for a behavior
    /// that keeps serving, with `serves = true` and no exit code. Returns the exit code the test
    /// needs when it gives none, or `None` after a problem.
    fn check_ending(
        &mut self,
        line: usize,
        name: &str,
        behavior: &Behavior,
        has_exit_code: bool,
        serves: bool,
    ) -> Option<i64> {
        let id = behavior.id;
        let problem = match (behavior.serves, serves, has_exit_code) {
            (true, true, false) => return Some(0),
            (false, false, true) => return Some(i64::from(behavior.exit_code)),
            (true, false, _) => format!(
                "{name}: the spec says {id} keeps serving, so write serves = true and no \
                 exit_code"
            ),
            (false, true, _) => format!(
                "{name} has serves = true, but the spec says {id} exits with code {}",
                behavior.exit_code
            ),
            (true, true, true) => {
                format!("{name} keeps serving, so it has no exit_code; remove it")
            }
            (false, false, false) => format!("{name} needs exit_code, the code the spec states"),
        };
        self.problem(Some(line), problem);
        None
    }

    /// A test may supply random bytes only when the spec allows `getrandom`, and clock readings
    /// only when it allows `clock_gettime`.
    fn check_supply(&mut self, line: usize, name: &str, random: bool, clock: bool) {
        let allows = |call: Syscall| self.spec.syscalls.contains(&call);
        if random && !allows(Syscall::Getrandom) {
            self.problem(
                Some(line),
                format!(
                    "{name} supplies random bytes, but spec.md doesn't allow getrandom; drop \
                     random, or ask the user to allow getrandom"
                ),
            );
        }
        if clock && !allows(Syscall::ClockGettime) {
            self.problem(
                Some(line),
                format!(
                    "{name} supplies clock readings, but spec.md doesn't allow clock_gettime; \
                     drop clock, or ask the user to allow clock_gettime"
                ),
            );
        }
    }

    /// Generators take their ranges from Purpose, except for a variable that reaches the program
    /// only as random bytes or clock readings. Random bytes draw every value they can hold, as
    /// the kernel's would; a clock reading draws times from 1970 on.
    fn check_ranges(&mut self, block: &Generated, name: &str) {
        let line = block.line;
        let random = block.random_only();
        let clock = block.clock_only();
        for (var, generator) in &block.vars {
            if !random.contains_key(var) && !clock.contains(var) {
                self.check_limits(line, name, var, generator);
            } else if let Some(counts) = random.get(var) {
                let [count] = counts.iter().copied().collect::<Vec<_>>()[..] else {
                    self.problem(
                        Some(line),
                        format!("{name}: {var} fills random bytes with different byte counts"),
                    );
                    continue;
                };
                let every = if count == 8 {
                    Generator::Int {
                        lo: i64::MIN.into(),
                        hi: i64::MAX.into(),
                    }
                } else {
                    Generator::Int {
                        lo: 0,
                        hi: (1 << (8 * count)) - 1,
                    }
                };
                if *generator != every {
                    self.problem(
                        Some(line),
                        format!(
                            "{name}: vars.{var} = {generator} fills {count} random byte(s), which \
                             the kernel could give any value, so it draws them all: \
                             {var} = \"{every}\""
                        ),
                    );
                }
            } else {
                let from_1970 = match generator {
                    Generator::Int { lo, .. } => *lo >= 0,
                    Generator::Dec { lo, places, .. } => *lo >= 0 && *places <= 9,
                    _ => false,
                };
                if !from_1970 {
                    self.problem(
                        Some(line),
                        format!(
                            "{name}: vars.{var} = {generator} is a clock reading; draw seconds \
                             since 1970 with int(lo, hi), or decimal(lo, hi) with up to 9 \
                             places, from 0 up"
                        ),
                    );
                }
            }
        }
    }

    /// Generators may only use numbers that Purpose states, so their ranges come from the spec.
    /// Numbers from behaviors don't count: an example's own inputs would make `int(2, 3)` pass.
    fn check_limits(&mut self, line: usize, name: &str, var: &str, generator: &Generator) {
        // Each value Purpose states, with the most places it's written with.
        let mut stated: BTreeMap<Limit, u8> = BTreeMap::new();
        for (limit, places) in self
            .spec
            .purpose_values
            .iter()
            .filter_map(|value| Limit::parse(value))
        {
            let most = stated.entry(limit).or_insert(places);
            *most = (*most).max(places);
        }
        let unreadable: Vec<&str> = self
            .spec
            .purpose_values
            .iter()
            .map(String::as_str)
            .filter(|value| {
                Limit::parse(value).is_none() && value.bytes().any(|byte| byte.is_ascii_digit())
            })
            .collect();
        let hint = if unreadable.is_empty() {
            String::new()
        } else {
            format!(
                " (Purpose writes {}: write numbers plainly, like `12.50`, with no `$`, comma or \
                 exponent)",
                unreadable
                    .iter()
                    .map(|value| format!("`{value}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        // "At most N bytes" implies the empty text, so a text length may start at 0 unstated.
        let empty_text = matches!(generator, Generator::Str { min: 0, .. });
        for limit in generator.limits() {
            if empty_text && limit == Limit::whole(0) {
                continue;
            }
            if !stated.contains_key(&limit) {
                self.problem(
                    Some(line),
                    format!(
                        "{name}: vars.{var} = {generator} uses {limit}, which spec.md's Purpose \
                         doesn't state. Generators take their ranges from Purpose: use the \
                         numbers it gives in backticks, or add the range to Purpose in spec.md \
                         and call record_spec again{hint}"
                    ),
                );
            }
        }
        if let Some(places) = generator.places()
            && let Some(written) = generator
                .limits()
                .iter()
                .filter_map(|limit| stated.get(limit))
                .max()
            && *written != places
        {
            self.problem(
                Some(line),
                format!(
                    "{name}: vars.{var} = {generator} draws numbers with {places} decimal \
                     places, but Purpose writes them with {written}; write the bounds the way \
                     Purpose does"
                ),
            );
        }
    }

    fn check_variables(&mut self, block: &Generated, name: &str) {
        let declared: BTreeSet<String> = block.vars.iter().map(|(var, _)| var.clone()).collect();
        let mut inputs = BTreeSet::new();
        for (field, template) in block
            .args
            .iter()
            .map(|arg| ("args", arg))
            .chain([("stdin", &block.stdin)])
            .chain(block.files.values().map(|file| ("files", file)))
            .chain(block.clock.iter().map(|reading| ("clock", reading)))
            .chain(
                block
                    .server
                    .iter()
                    .map(|(_, replies)| ("server.replies", replies)),
            )
            .chain(
                block
                    .client
                    .iter()
                    .map(|(sends, _)| ("client.sends", sends)),
            )
        {
            if template.pattern().is_none() {
                self.problem(
                    Some(block.line),
                    format!(
                        "{name}: {field} {:?} computes a value; what a test gives the program \
                         (args, stdin, files, clock, server.replies, client.sends) may only hold \
                         bare variables like {{a}}, so move the computation to what it expects",
                        template.source()
                    ),
                );
            }
            inputs.extend(template.variables());
        }
        // `port` is the test server's, filled in when the test runs.
        let port_built_in = !declared.contains(PORT);
        let uses_port = block
            .given()
            .chain(block.outputs())
            .any(|template| template.variables().contains(PORT));
        if port_built_in && uses_port && block.server.is_empty() {
            self.problem(
                Some(block.line),
                format!(
                    "{name} uses port, the port of the test's own server, but has no server; \
                     add server = {{ receives = \"…\", replies = \"…\" }}"
                ),
            );
        }
        if port_built_in {
            inputs.remove(PORT);
        }
        if let Some(random) = &block.random {
            if random.hex_parts().is_none() {
                self.problem(
                    Some(block.line),
                    format!(
                        "{name}: random {:?} may only hold hex digits and hex(x, n), a variable \
                         and 1 to 8 bytes, such as \"{{hex(roll, 1)}}\"",
                        random.source()
                    ),
                );
            }
            inputs.extend(random.variables());
        }
        let others = block
            .args
            .iter()
            .chain([&block.stdin])
            .chain(block.files.values())
            .chain(block.clock.iter())
            .chain(block.outputs());
        if others
            .into_iter()
            .any(|template| template.calls(Function::Hex))
        {
            self.problem(
                Some(block.line),
                format!("{name}: hex(x, n) only fills random bytes, in random"),
            );
        }
        if !block.clock.is_empty() && block.uses_now() {
            self.problem(
                Some(block.line),
                format!(
                    "{name} supplies the clock, so the time is known exactly: compute the output \
                     from the clock's variables instead of now"
                ),
            );
        }
        let mut used = inputs.clone();
        used.extend(block.stdout.variables());
        used.extend(block.stderr.variables());
        for template in block.files_after.values() {
            used.extend(template.variables());
        }
        if let ExitCode::Formula(template) = &block.exit_code {
            used.extend(template.variables());
        }
        let now_built_in = !declared.contains(NOW);
        if now_built_in && inputs.contains(NOW) {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: args, stdin and files can't hold now, the time the run starts; the \
                     program reads the time itself"
                ),
            );
        }
        if now_built_in
            && block
                .outputs()
                .any(|template| template.uses_outside_any(NOW))
        {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: now is the second the run starts, which no test can know exactly, \
                     so it may only bound a rule such as {{any(now - 2, now + 2)}}"
                ),
            );
        }
        for var in used.difference(&declared) {
            if (now_built_in && var == NOW) || (port_built_in && var == PORT) {
                continue;
            }
            self.problem(
                Some(block.line),
                format!("{name} uses `{var}`, which isn't declared in vars"),
            );
        }
        for var in declared.difference(&inputs) {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: `{var}` never reaches the program; use it in args, stdin, files, \
                     random or clock, or remove it"
                ),
            );
        }
    }

    /// Evaluates every run's expected results now, so a formula that overflows, divides by zero
    /// or prints the same thing for every input is caught before any code exists.
    fn dry_run(&mut self, block: &Generated, seed: u64) {
        let name = format!("generated {}", block.id);
        let varies = block.stdout.has_placeholders()
            || block.stderr.has_placeholders()
            || block.files_after.values().any(Template::has_placeholders)
            || block
                .server
                .iter()
                .any(|(receives, _)| receives.has_placeholders())
            || block
                .client
                .iter()
                .any(|(_, receives)| receives.has_placeholders());
        // Two different outputs are enough to know they vary; keep no more, since text is large.
        let mut outputs: Vec<Vec<String>> = Vec::new();
        let mut runs = 0;
        for result in block.cases(seed) {
            let case = match result {
                Ok(case) => case,
                Err((inputs, field, error)) => {
                    self.problem(
                        Some(block.line),
                        format!(
                            "{name}: with {}, {field} {error}; narrow the generators or fix the \
                             formula",
                            describe_inputs(&inputs)
                        ),
                    );
                    return;
                }
            };
            if let Some(problem) = oversized_args(&case) {
                self.problem(Some(block.line), format!("{name}: {problem}"));
                return;
            }
            if let Err(problem) = Supply::parse(case.random.as_deref(), &case.clock) {
                self.problem(
                    Some(block.line),
                    format!("{name}: with {}, {problem}", describe_inputs(&case.inputs)),
                );
                return;
            }
            if let Some((file, contents)) = case
                .files
                .iter()
                .chain(&case.files_after)
                .find(|(_, contents)| contents.len() > MAX_FILE_BYTES)
            {
                self.problem(
                    Some(block.line),
                    format!(
                        "{name}: with {}, {file} holds {} bytes; a test's files hold at most \
                         {MAX_FILE_BYTES}",
                        describe_inputs(&case.inputs),
                        contents.len()
                    ),
                );
                return;
            }
            // The test's server reads exactly what it expects, so that can't be a rule.
            if case
                .server
                .iter()
                .any(|reply| crate::expect::has_holes(&reply.receives))
            {
                self.problem(
                    Some(block.line),
                    format!(
                        "{name}: server.receives must be exact, since the test's server reads \
                         exactly those bytes; a rule such as any(…) goes in what the program \
                         prints or answers"
                    ),
                );
                return;
            }
            runs += 1;
            let expected: Vec<String> = [case.stdout, case.stderr]
                .into_iter()
                .chain(case.files_after.into_values())
                .chain(case.server.into_iter().map(|reply| reply.receives))
                .chain(case.client.into_iter().map(|exchange| exchange.receives))
                .collect();
            if outputs.len() < 2 && !outputs.contains(&expected) {
                outputs.push(expected);
            }
        }
        // A rule such as {any(1, 6)} stands for what the program chooses, not for the inputs.
        let rule = outputs
            .iter()
            .flatten()
            .any(|text| crate::expect::has_holes(text));
        if varies && outputs.len() < 2 && !rule {
            self.problem(
                Some(block.line),
                format!(
                    "{name}: all {runs} runs expect the same output, so a program that always \
                     prints it would pass. Make the output depend on the inputs, widen the \
                     generators, raise runs, or drop the placeholders"
                ),
            );
        }
    }

    /// Every behavior whose example has inputs needs a generated block, and all blocks of a
    /// behavior share their templates.
    fn coverage(&mut self, examples: &[Example], generated: &[Generated]) {
        for example in examples {
            let blocks: Vec<&Generated> = generated
                .iter()
                .filter(|block| block.id == example.id)
                .collect();
            let has_inputs = !example.args.is_empty()
                || !example.stdin.is_empty()
                || !example.files.is_empty()
                || example.random.is_some()
                || !example.clock.is_empty()
                || !example.server.is_empty()
                || !example.client.is_empty();
            for block in &blocks {
                if block.server.len() != example.server.len()
                    || block.client.len() != example.client.len()
                {
                    self.problem(
                        Some(block.line),
                        format!(
                            "generated {} and its example must talk over the same number of \
                             connections: {} with the test's server and {} with its client in \
                             the example",
                            block.id,
                            example.server.len(),
                            example.client.len()
                        ),
                    );
                }
                if block.random.is_some() != example.random.is_some()
                    || block.clock.is_empty() != example.clock.is_empty()
                {
                    self.problem(
                        Some(block.line),
                        format!(
                            "generated {} and its example must supply the same things: random \
                             bytes in both or neither, and clock readings in both or neither",
                            block.id
                        ),
                    );
                }
            }
            if has_inputs && blocks.is_empty() {
                self.problem(
                    Some(example.line),
                    format!(
                        "{} has inputs, so it needs a [[generated]] block that tries fresh \
                         inputs on every run",
                        example.id
                    ),
                );
            }
            if let [first, rest @ ..] = blocks.as_slice() {
                for block in rest {
                    if templates(block) != templates(first) {
                        self.problem(
                            Some(block.line),
                            format!(
                                "the [[generated]] blocks of {} must share args, stdin, files, \
                                 random, clock, stdout, stderr, files_after and exit_code (as on \
                                 line {}); only vars and runs may differ",
                                block.id, first.line
                            ),
                        );
                    }
                }
            }
        }
    }

    /// Each behavior's formulas must reproduce its example, whose values come from the
    /// recorded spec.
    fn formulas(&mut self, examples: &[Example], generated: &[Generated]) {
        for example in examples {
            let blocks: Vec<&Generated> = generated
                .iter()
                .filter(|block| block.id == example.id)
                .collect();
            let Some(first) = blocks.first() else {
                continue;
            };
            let id = example.id;
            let inputs = match bind(first, example) {
                Ok(inputs) => inputs,
                Err(reason) => {
                    self.problem(
                        Some(first.line),
                        format!("generated {id} can't be matched to example {id}: {reason}"),
                    );
                    continue;
                }
            };
            let Some(block) = blocks.iter().find(|block| {
                block.vars.iter().all(|(name, generator)| {
                    inputs
                        .get(name)
                        .is_some_and(|value| generator.contains(value))
                })
            }) else {
                let outside: Vec<String> = first
                    .vars
                    .iter()
                    .filter_map(|(name, generator)| {
                        let value = inputs.get(name)?;
                        let why = generator.why_outside(value)?;
                        Some(format!("{name} = {value} is outside {generator}: {why}"))
                    })
                    .collect();
                self.problem(
                    Some(first.line),
                    format!(
                        "example {id}'s inputs aren't in the generated ranges ({}); the ranges \
                         must include the spec's example",
                        outside.join("; ")
                    ),
                );
                continue;
            };
            let now = crate::expect::now();
            let case = match block.case(inputs, now) {
                Ok(case) => case,
                Err((inputs, field, error)) => {
                    self.problem(
                        Some(block.line),
                        format!(
                            "generated {id}: with example {id}'s inputs ({}), {field} {error}",
                            describe_inputs(&inputs)
                        ),
                    );
                    continue;
                }
            };
            let agree =
                |computed: &str, example: &str| crate::expect::agrees(computed, example, now);
            let shown = |files: &BTreeMap<String, String>| {
                let files: BTreeMap<&str, String> = files
                    .iter()
                    .map(|(name, contents)| (name.as_str(), crate::expect::show(contents)))
                    .collect();
                format!("{files:?}")
            };
            let mismatch = if !agree(&case.stdout, &example.stdout) {
                Some((
                    "stdout",
                    format!("{:?}", crate::expect::show(&case.stdout)),
                    format!("{:?}", crate::expect::show(&example.stdout)),
                ))
            } else if !agree(&case.stderr, &example.stderr) {
                Some((
                    "stderr",
                    format!("{:?}", crate::expect::show(&case.stderr)),
                    format!("{:?}", crate::expect::show(&example.stderr)),
                ))
            } else if !case.files_after.keys().eq(example.files_after.keys())
                || case
                    .files_after
                    .values()
                    .zip(example.files_after.values())
                    .any(|(computed, example)| !agree(computed, example))
            {
                Some((
                    "files_after",
                    shown(&case.files_after),
                    shown(&example.files_after),
                ))
            } else if let Some((computed, written)) = case
                .server
                .iter()
                .zip(&example.server)
                .find(|(computed, written)| computed.receives != written.receives)
            {
                Some((
                    "server.receives",
                    format!("{:?}", computed.receives),
                    format!("{:?}", written.receives),
                ))
            } else if let Some((computed, written)) = case
                .client
                .iter()
                .zip(&example.client)
                .find(|(computed, written)| !agree(&computed.receives, &written.receives))
            {
                Some((
                    "client.receives",
                    format!("{:?}", crate::expect::show(&computed.receives)),
                    format!("{:?}", crate::expect::show(&written.receives)),
                ))
            } else if case.exit_code != example.exit_code {
                Some((
                    "exit code",
                    case.exit_code.to_string(),
                    example.exit_code.to_string(),
                ))
            } else {
                None
            };
            if let Some((field, formula, spec)) = mismatch {
                self.problem(
                    Some(block.line),
                    format!(
                        "generated {id}'s formula disagrees with the spec: with example {id}'s \
                         inputs ({}) it expects {field} {formula}, but example {id} expects \
                         {spec}. Fix the formula",
                        describe_inputs(&case.inputs)
                    ),
                );
            }
        }
    }
}

/// A generated block's templates as written, to tell whether two blocks share them.
#[derive(PartialEq, Eq)]
struct TemplateSources<'a> {
    args: Vec<&'a str>,
    stdin: &'a str,
    files: BTreeMap<&'a str, &'a str>,
    stdout: &'a str,
    stderr: &'a str,
    files_after: BTreeMap<&'a str, &'a str>,
    formula: Option<&'a str>,
    fixed: Option<u8>,
    random: Option<&'a str>,
    clock: Vec<&'a str>,
    server: Vec<(&'a str, &'a str)>,
    client: Vec<(&'a str, &'a str)>,
    serves: bool,
}

fn templates(block: &Generated) -> TemplateSources<'_> {
    let (formula, fixed) = match &block.exit_code {
        ExitCode::Fixed(code) => (None, Some(*code)),
        ExitCode::Formula(template) => (Some(template.source()), None),
    };
    fn sources(files: &BTreeMap<String, Template>) -> BTreeMap<&str, &str> {
        files
            .iter()
            .map(|(name, template)| (name.as_str(), template.source()))
            .collect()
    }
    TemplateSources {
        args: block.args.iter().map(Template::source).collect(),
        stdin: block.stdin.source(),
        files: sources(&block.files),
        stdout: block.stdout.source(),
        stderr: block.stderr.source(),
        files_after: sources(&block.files_after),
        formula,
        fixed,
        random: block.random.as_ref().map(Template::source),
        clock: block.clock.iter().map(Template::source).collect(),
        server: block
            .server
            .iter()
            .map(|(receives, replies)| (receives.source(), replies.source()))
            .collect(),
        client: block
            .client
            .iter()
            .map(|(sends, receives)| (sends.source(), receives.source()))
            .collect(),
        serves: block.serves,
    }
}

/// Reads the variables' values back out of the example's args, stdin and files.
fn bind(block: &Generated, example: &Example) -> Result<Env, String> {
    if block.args.len() != example.args.len() {
        return Err(format!(
            "it has {} args and the example has {}",
            block.args.len(),
            example.args.len()
        ));
    }
    if !block.files.keys().eq(example.files.keys()) {
        return Err(format!(
            "it gives the files {:?} and the example gives {:?}",
            block.files.keys().collect::<Vec<_>>(),
            example.files.keys().collect::<Vec<_>>()
        ));
    }
    if block.clock.len() != example.clock.len() {
        return Err(format!(
            "it gives {} clock readings and the example gives {}",
            block.clock.len(),
            example.clock.len()
        ));
    }
    // The random bytes say their variables' values outright.
    let mut env = Env::new();
    if let (Some(template), Some(bytes)) = (&block.random, &example.random) {
        read_hex(template, bytes, &mut env)?;
    }
    let generators: BTreeMap<&str, &Generator> = block
        .vars
        .iter()
        .map(|(name, generator)| (name.as_str(), generator))
        .collect();
    let mut pairs = Vec::new();
    for (template, text) in block
        .args
        .iter()
        .zip(&example.args)
        .chain([(&block.stdin, &example.stdin)])
        .chain(block.files.values().zip(example.files.values()))
        .chain(block.clock.iter().zip(&example.clock))
        .chain(
            block
                .server
                .iter()
                .map(|(_, replies)| replies)
                .zip(example.server.iter().map(|reply| &reply.replies)),
        )
        .chain(
            block
                .client
                .iter()
                .map(|(sends, _)| sends)
                .zip(example.client.iter().map(|exchange| &exchange.sends)),
        )
    {
        let pattern = template.pattern().ok_or_else(|| {
            "what it gives the program must be bare variables, as in args".to_owned()
        })?;
        pairs.push((pattern, text.as_str()));
    }
    // The test server's port reads as its stand-in, just as the example writes it.
    let uses_port = block
        .given()
        .chain(block.outputs())
        .any(|template| template.variables().contains(PORT));
    if uses_port && !generators.contains_key(PORT) {
        env.insert(PORT.to_owned(), Value::Str(PORT_TEXT.to_owned()));
    }
    let mut solutions = Vec::new();
    solve(&pairs, 0, &generators, &mut env, &mut solutions);
    match solutions.len() {
        0 => {
            // Each input on its own, to name the ones that can't fit.
            let misfits: Vec<String> = pairs
                .iter()
                .filter(|pair| {
                    let mut alone = Vec::new();
                    solve(
                        std::slice::from_ref(*pair),
                        0,
                        &generators,
                        &mut env.clone(),
                        &mut alone,
                    );
                    alone.is_empty()
                })
                .map(|(pattern, text)| {
                    let written: String = pattern
                        .iter()
                        .map(|part| match part {
                            PatternPart::Text(text) => text.replace('{', "{{"),
                            PatternPart::Var(name) => format!("{{{name}}}"),
                        })
                        .collect();
                    format!(
                        "{written:?} can't become {}",
                        Value::Str((*text).to_owned())
                    )
                })
                .collect();
            Err(if misfits.is_empty() {
                "no values of its vars give all of what the example gives the program at once; \
                 check that a variable used twice holds the same text in both places"
                    .into()
            } else {
                format!(
                    "no values of its vars turn what it gives the program into what the example \
                     gives it: {}",
                    misfits.join("; ")
                )
            })
        }
        1 => Ok(solutions.remove(0)),
        _ => Err(
            "the example's inputs fit its placeholders in more than one way; separate \
                  neighboring placeholders with fixed text"
                .into(),
        ),
    }
}

/// Reads the variables of a block's `random` template out of an example's random bytes.
fn read_hex(template: &Template, bytes: &str, env: &mut Env) -> Result<(), String> {
    let misfit = || {
        format!(
            "the example's random bytes {bytes:?} don't fit its random {:?}",
            template.source()
        )
    };
    let parts = template
        .hex_parts()
        .ok_or("its random bytes must be hex digits and hex(x, n)")?;
    let mut rest = bytes.to_ascii_lowercase();
    for part in parts {
        match part {
            HexPart::Digits(digits) => {
                rest = rest
                    .strip_prefix(&digits.to_ascii_lowercase())
                    .ok_or_else(misfit)?
                    .to_owned();
            }
            HexPart::Var(name, count) => {
                let digits = rest.get(..2 * count).ok_or_else(misfit)?;
                let value = crate::supply::hex(digits).map_err(|_| misfit())?;
                let mut word = [0_u8; 8];
                word[..count].copy_from_slice(&value);
                let value = if count == 8 {
                    i128::from(i64::from_le_bytes(word))
                } else {
                    i128::from(u64::from_le_bytes(word))
                };
                if env
                    .insert(name, Value::Int(value))
                    .is_some_and(|before| before != Value::Int(value))
                {
                    return Err(misfit());
                }
                rest = rest[2 * count..].to_owned();
            }
        }
    }
    if rest.is_empty() {
        Ok(())
    } else {
        Err(misfit())
    }
}

/// Backtracking search for variable values that render each pattern into its text. Stops after
/// two solutions, which is enough to know the match is ambiguous.
fn solve(
    pairs: &[(Vec<PatternPart>, &str)],
    index: usize,
    generators: &BTreeMap<&str, &Generator>,
    env: &mut Env,
    solutions: &mut Vec<Env>,
) {
    fn within(
        pairs: &[(Vec<PatternPart>, &str)],
        index: usize,
        parts: &[PatternPart],
        text: &str,
        generators: &BTreeMap<&str, &Generator>,
        env: &mut Env,
        solutions: &mut Vec<Env>,
    ) {
        if solutions.len() > 1 {
            return;
        }
        match parts.split_first() {
            None => {
                if text.is_empty() {
                    solve(pairs, index + 1, generators, env, solutions);
                }
            }
            Some((PatternPart::Text(literal), tail)) => {
                if let Some(after) = text.strip_prefix(literal.as_str()) {
                    within(pairs, index, tail, after, generators, env, solutions);
                }
            }
            Some((PatternPart::Var(name), tail)) => {
                if let Some(bound) = env.get(name) {
                    let rendered = bound.render().unwrap_or_default();
                    if let Some(after) = text.strip_prefix(rendered.as_str()) {
                        within(pairs, index, tail, after, generators, env, solutions);
                    }
                    return;
                }
                let Some(generator) = generators.get(name.as_str()) else {
                    return;
                };
                // Only the splits where the rest can still match are worth reading: the value
                // runs to the end, or up to where the next fixed text starts. On long inputs this
                // keeps matching linear.
                let viable = |split: usize| match tail.first() {
                    None => split == text.len(),
                    Some(PatternPart::Text(literal)) => text[split..].starts_with(literal.as_str()),
                    Some(PatternPart::Var(_)) => true,
                };
                let splits: Vec<usize> = text
                    .char_indices()
                    .map(|(position, _)| position)
                    .chain([text.len()])
                    .filter(|split| viable(*split))
                    .collect();
                for split in splits {
                    if let Some(value) = generator.read(&text[..split]) {
                        env.insert(name.clone(), value);
                        within(
                            pairs,
                            index,
                            tail,
                            &text[split..],
                            generators,
                            env,
                            solutions,
                        );
                        env.remove(name);
                    }
                }
            }
        }
    }
    match pairs.get(index) {
        None => solutions.push(env.clone()),
        Some((parts, text)) => within(pairs, index, parts, text, generators, env, solutions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec;

    const SPEC: &str = "---
type: lengua/program-spec
title: add
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

    const LABEL: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn tests(body: &str) -> String {
        format!("[meta]\nspec = \"{LABEL}\"\n{body}")
    }

    const EXAMPLES: &str = r#"
[[example]]
id = "B1"
args = ["2", "3"]
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1
"#;

    const GENERATED: &str = r#"
[[generated]]
id = "B1"
runs = 200
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
args = ["{a}", "{b}"]
stdout = "{a + b}\n"
exit_code = 0
"#;

    fn spec() -> Spec {
        spec::parse(SPEC).expect("valid spec")
    }

    fn valid() -> String {
        tests(&format!("{EXAMPLES}{GENERATED}"))
    }

    fn problems(text: &str) -> Vec<String> {
        match check(text, &spec(), LABEL) {
            Ok(suite) => panic!("expected problems, got {suite:#?}"),
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
    fn accepts_valid_tests() {
        let suite = check(&valid(), &spec(), LABEL).expect("valid tests");
        assert_eq!(suite.examples.len(), 2);
        assert_eq!(suite.examples[0].args, ["2", "3"]);
        assert_eq!(suite.examples[1].line, 10);
        let block = &suite.generated[0];
        assert_eq!(block.runs, 200);
        assert_eq!(block.line, 15);
        let cases: Vec<Case> = block
            .cases(42)
            .collect::<Result<_, _>>()
            .expect("computable");
        assert_eq!(cases.len(), 200);
        let first = &cases[0];
        assert_eq!(first.args, ["-1000000000", "-1000000000"]);
        assert_eq!(first.stdout, "-2000000000\n");
        assert_eq!(first.exit_code, 0);
    }

    const DICE: &str = "---
type: lengua/program-spec
title: roll
---
# Purpose
Rolls a die for a player numbered from `1` to `9`.

# Behaviors
## B1: Rolls a die
- Given the argument `1`
- When the program runs
- Then it prints `rolled ` and a number from `1` to `6`, followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, getrandom, exit
";

    const ROLLS: &str = r#"
[[example]]
id = "B1"
args = ["1"]
stdout = { matches = "rolled {any(1, 6)}\n" }
exit_code = 0

[[generated]]
id = "B1"
vars = { player = "int(1, 9)" }
args = ["{player}"]
stdout = "rolled {any(1, 6)}\n"
exit_code = 0
"#;

    fn dice(body: &str) -> Result<TestSuite, Vec<String>> {
        let spec = spec::parse(DICE).expect("valid spec");
        check(&tests(body), &spec, LABEL)
            .map_err(|problems| problems.iter().map(ToString::to_string).collect())
    }

    fn assert_dice_problem(body: &str, needle: &str) {
        let problems = dice(body).expect_err("problems");
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "no problem containing {needle:?} in {problems:#?}"
        );
    }

    #[test]
    fn rules_allow_any_number_in_a_range() {
        let suite = dice(ROLLS).expect("valid tests");
        let example = &suite.examples[0];
        assert!(crate::expect::matches(&example.stdout, b"rolled 6\n", 0));
        assert!(!crate::expect::matches(&example.stdout, b"rolled 7\n", 0));
        assert_eq!(crate::expect::show(&example.stdout), "rolled {any(1, 6)}\n");
        let case = suite.generated[0]
            .cases(1)
            .next()
            .expect("a run")
            .expect("computable");
        assert_eq!(case.stdout, example.stdout);
    }

    #[test]
    fn rules_bound_only_expected_output() {
        assert_dice_problem(
            &ROLLS.replace(r#"args = ["1"]"#, r#"args = [{ matches = "{any(1, 9)}" }]"#),
            "goes only in stdout, stderr and files_after",
        );
        assert_dice_problem(
            &ROLLS.replace("rolled {any(1, 6)}\\n\" }", "rolled {player}\\n\" }"),
            "a rule's placeholders are any(lo, hi) only",
        );
        assert_dice_problem(
            &ROLLS.replace("{any(1, 6)}\\n\" }", "{any(6, 1)}\\n\" }"),
            "smallest number after its largest",
        );
        assert_dice_problem(
            &ROLLS.replace(
                "rolled {any(1, 6)}\\n\"\nexit",
                "rolled {any(1, 7)}\\n\"\nexit",
            ),
            "generated B1's formula disagrees with the spec",
        );
        assert_dice_problem(
            &ROLLS.replace(
                "rolled {any(1, 6)}\\n\"\nexit",
                "rolled {len(any(1, 6))}\\n\"\nexit",
            ),
            "len can't look inside any(…)",
        );
        assert_dice_problem(
            &ROLLS.replace(r#"args = ["1"]"#, r#"args = ["1\uFDD1"]"#),
            "U+FDD0, U+FDD1 and U+FDD2",
        );
        // Where the number ends must be clear: "30" isn't a 3 and a 0.
        assert_dice_problem(
            &ROLLS.replace("rolled {any(1, 6)}\\n\" }", "rolled {any(1, 6)}0\\n\" }"),
            "followed directly by a digit",
        );
        assert_dice_problem(
            &ROLLS.replace(
                "rolled {any(1, 6)}\\n\"\nexit",
                "rolled {any(1, 6)}{player}\\n\"\nexit",
            ),
            "followed directly by a digit",
        );
    }

    #[test]
    fn now_only_bounds_a_rule() {
        const CLOCK: &str = "---
type: lengua/program-spec
title: clock
---
# Purpose
Prints the time in Unix seconds for a player numbered from `1` to `9`.

# Behaviors
## B1: Prints the time
- Given the argument `1`
- When the program runs
- Then it prints the current Unix time followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, clock_gettime, exit
";
        let body = r#"
[[example]]
id = "B1"
args = ["1"]
stdout = { matches = "{any(now - 2, now + 2)}\n" }
exit_code = 0

[[generated]]
id = "B1"
vars = { player = "int(1, 9)" }
args = ["{player}"]
stdout = "{any(now - 2, now + 2)}\n"
exit_code = 0
"#;
        let spec = spec::parse(CLOCK).expect("valid spec");
        let suite = check(&tests(body), &spec, LABEL).expect("valid tests");
        assert!(suite.generated[0].uses_now());
        let now = crate::expect::now();
        let case = suite.generated[0]
            .case(BTreeMap::from([("player".to_owned(), Value::Int(1))]), now)
            .expect("computable");
        assert!(crate::expect::matches(
            &case.stdout,
            format!("{}\n", now - 2).as_bytes(),
            0
        ));
        let problems = |body: &str| {
            check(&tests(body), &spec, LABEL)
                .expect_err("problems")
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        };
        let exact =
            problems(&body.replace("\"{any(now - 2, now + 2)}\\n\"\nexit", "\"{now}\\n\"\nexit"));
        assert!(exact.contains("may only bound a rule"), "{exact}");
        let input =
            problems(&body.replace(r#"args = ["{player}"]"#, r#"args = ["{player}", "{now}"]"#));
        assert!(input.contains("can't hold now"), "{input}");
        // A block that declares now draws it like any variable.
        let declared = problems(&body.replace(
            r#"vars = { player = "int(1, 9)" }"#,
            r#"vars = { player = "int(1, 9)", now = "int(1, 9)" }"#,
        ));
        assert!(
            declared.contains("`now` never reaches the program"),
            "{declared}"
        );
    }

    fn spec_text(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples")
                .join(name)
                .join("spec.md"),
        )
        .expect("an example spec")
    }

    const SUPPLIED: &str = r#"
[[example]]
id = "B1"
random = "fc2a"
stdout = "rolled 1\n"
exit_code = 0

[[generated]]
id = "B1"
vars = { first = "int(0, 255)", second = "int(0, 255)" }
random = "{hex(first, 1)}{hex(second, 1)}"
stdout = "rolled {if(first < 252, first % 6, if(second < 252, second % 6, 0)) + 1}\n"
exit_code = 0
"#;

    fn supplied(body: &str) -> Result<TestSuite, Vec<String>> {
        let spec = spec::parse(&spec_text("dice")).expect("valid spec");
        check(&tests(body), &spec, LABEL)
            .map_err(|problems| problems.iter().map(ToString::to_string).collect())
    }

    fn assert_supplied_problem(body: &str, needle: &str) {
        let problems = supplied(body).expect_err("problems");
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "no problem containing {needle:?} in {problems:#?}"
        );
    }

    #[test]
    fn tests_supply_random_bytes_from_variables() {
        let suite = supplied(SUPPLIED).expect("valid tests");
        assert_eq!(suite.examples[0].supply().random, Some(vec![0xfc, 0x2a]));
        let block = &suite.generated[0];
        let inputs = BTreeMap::from([
            ("first".to_owned(), Value::Int(252)),
            ("second".to_owned(), Value::Int(42)),
        ]);
        let case = block.case(inputs, 0).expect("computable");
        assert_eq!(case.random.as_deref(), Some("fc2a"));
        assert_eq!(case.stdout, "rolled 1\n");
        assert_eq!(block.random_only().len(), 2);
    }

    #[test]
    fn supplied_values_follow_their_rules() {
        // Random bytes take every value the kernel could give.
        assert_supplied_problem(
            &SUPPLIED.replace(r#"first = "int(0, 255)""#, r#"first = "int(0, 9)""#),
            "first = \"int(0, 255)\"",
        );
        assert_supplied_problem(
            &SUPPLIED.replace("rolled {if(", "rolled {hex(first, 1)}{if("),
            "hex(x, n) only fills random bytes",
        );
        assert_supplied_problem(
            &SUPPLIED.replace("{hex(first, 1)}{hex(second, 1)}", "{first}{second}"),
            "may only hold hex digits and hex(x, n)",
        );
        assert_supplied_problem(
            &SUPPLIED.replace(r#"random = "fc2a""#, r#"random = "fc2g""#),
            "pairs of hex digits",
        );
        // The spec's bytes must be in the example, and the example and block supply alike.
        assert_supplied_problem(
            &SUPPLIED.replace(r#"random = "fc2a""#, r#"random = "fd2a""#),
            "doesn't contain the spec's value `fc`",
        );
        assert_supplied_problem(
            &SUPPLIED.replace("random = \"fc2a\"\n", ""),
            "must supply the same things",
        );
        // A spec that doesn't allow getrandom gets no random bytes.
        assert_problem(
            &valid().replace(
                "args = [\"2\", \"3\"]",
                "args = [\"2\", \"3\"]\nrandom = \"00\"",
            ),
            "spec.md doesn't allow getrandom",
        );
    }

    #[test]
    fn tests_supply_clock_readings() {
        const HOUR: &str = "---
type: lengua/program-spec
title: hour
---
# Purpose
Prints the hour of the day in UTC, from `0` to `23`, for a reader numbered from `1` to `9`.

# Behaviors
## B1: Prints the hour
- Given the argument `1` and the clock at `1735693200.5`
- When the program runs
- Then it prints `1` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, clock_gettime, exit
";
        let body = r#"
[[example]]
id = "B1"
args = ["1"]
clock = ["1735693200.5"]
stdout = "1\n"
exit_code = 0

[[generated]]
id = "B1"
vars = { reader = "int(1, 9)", t = "decimal(0.0, 4102444800.0)" }
args = ["{reader}"]
clock = ["{t}"]
stdout = "{floor(t) % 86400 / 3600}\n"
exit_code = 0
"#;
        let spec = spec::parse(HOUR).expect("valid spec");
        let problems = |body: &str| {
            check(&tests(body), &spec, LABEL)
                .err()
                .unwrap_or_default()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        };
        let suite = check(&tests(body), &spec, LABEL).expect("valid tests");
        let reading = suite.examples[0].supply().clock[0];
        assert_eq!(
            (reading.seconds, reading.nanoseconds),
            (1_735_693_200, 500_000_000)
        );
        assert!(suite.generated[0].clock_only().contains("t"));
        let early = problems(&body.replace("decimal(0.0,", "decimal(-1.0,"));
        assert!(early.contains("is a clock reading"), "{early}");
        let now = problems(&body.replace(
            "stdout = \"{floor(t) % 86400 / 3600}\\n\"",
            "stdout = \"{any(now - 1, now)}\\n\"",
        ));
        assert!(now.contains("supplies the clock"), "{now}");
    }

    #[test]
    fn network_tests_read_like_any_example() {
        // A spec writes a protocol's line breaks as `\r\n`; the example holds the real ones, and
        // long messages may be written with repeats.
        let spec = spec::parse(&spec_text("echo").replace(
            "Given a client that sends `hello` followed by a newline",
            "Given a client that sends `hello\\r\\n`",
        ))
        .expect("valid spec");
        let label = LABEL;
        let body = |example: &str| {
            format!(
                "[meta]\nspec = \"{label}\"\n\n[[example]]\nid = \"B1\"\n{example}\nserves = true\n\n\
                 [[generated]]\nid = \"B1\"\nvars = {{ line = 'string(0, 1023, \"ehlo\")' }}\n\
                 client = [{{ sends = \"{{line}}\\r\\n\", receives = \"{{line}}\\r\\n\" }}]\n\
                 serves = true\n"
            )
        };
        let crlf = body(r#"client = [{ sends = "hello\r\n", receives = "hello\r\n" }]"#);
        check(&crlf, &spec, label).expect("`hello\\r\\n` is in the example");
        let repeated = body(
            r#"client = [{ sends = [{ repeat = "hel", times = 1 }, "lo\r\n"], receives = "hello\r\n" }]"#,
        );
        check(&repeated, &spec, label).expect("repeats work in network fields");
        // When the block can't produce the example, the message names what doesn't fit.
        let misfit = crlf.replace("sends = \"{line}\\r\\n\"", "sends = \"{line}!\\r\\n\"");
        let problems: Vec<String> = check(&misfit, &spec, label)
            .expect_err("a misfit")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains(r#""{line}!\r\n" can't become "hello\r\n""#)),
            "{problems:#?}"
        );
    }

    #[test]
    fn requires_the_current_spec_label() {
        let text = valid().replace(LABEL, "sha256:1111");
        assert_problem(&text, "[meta] says spec = \"sha256:1111\"");
    }

    #[test]
    fn reports_toml_errors() {
        assert_problem("[meta]\nspec = ", "tests.toml isn't valid");
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 1\ncolour = \"red\""),
            "unknown field `colour`",
        );
        assert_problem(
            &valid().replace("exit_code = 1\n", ""),
            "needs exit_code, the code the spec states",
        );
    }

    #[test]
    fn requires_one_example_per_behavior() {
        let without_b2 = tests(&format!(
            "{}{GENERATED}",
            EXAMPLES
                .split("\n[[example]]\nid = \"B2\"")
                .next()
                .expect("B1 part")
        ));
        assert_problem(&without_b2, "B2 has no [[example]]");
        let twice = valid().replace("id = \"B2\"", "id = \"B1\"");
        assert_problem(&twice, "B1 has two examples (also on line 4)");
        let unknown = valid().replace("id = \"B2\"", "id = \"B7\"");
        assert_problem(
            &unknown,
            "id = \"B7\", which isn't a behavior in spec.md (they are B1, B2)",
        );
    }

    #[test]
    fn requires_generated_tests_for_behaviors_with_inputs() {
        assert_problem(
            &tests(EXAMPLES),
            "B1 has inputs, so it needs a [[generated]] block",
        );
    }

    #[test]
    fn requires_every_spec_value_in_the_example() {
        let text = valid().replace("stdout = \"5\\n\"", "stdout = \"6\\n\"");
        assert_problem(&text, "example B1 doesn't contain the spec's value `5`");
    }

    #[test]
    fn checks_exit_codes() {
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 134"),
            "expects exit code 134, which is reserved",
        );
        assert_problem(
            &valid().replace("exit_code = 1", "exit_code = 2"),
            "example B2 expects exit code 2, but the spec says B2 exits with code 1",
        );
        assert_problem(
            &valid().replace(
                "stdout = \"{a + b}\\n\"\nexit_code = 0",
                "stdout = \"{a + b}\\n\"\nexit_code = 3",
            ),
            "generated B1 expects exit code 3",
        );
        assert_problem(
            &valid().replace(
                "stdout = \"{a + b}\\n\"\nexit_code = 0",
                "stdout = \"{a + b}\\n\"\nexit_code = \"{if(a > 0, 0, 134)}\"",
            ),
            "reserved for failed assertions",
        );
    }

    #[test]
    fn reports_bad_expressions() {
        assert_problem(
            &valid().replace("{a + b}", "{a +* b}"),
            "generated B1: stdout: in {a +* b}",
        );
        assert_problem(
            &valid().replace("{a + b}", "{a + c}"),
            "uses `c`, which isn't declared in vars",
        );
        assert_problem(
            &valid().replace("\"int(-1000000000, 1000000000)\" }", "\"float(0, 1)\" }"),
            "vars.b: unknown generator `float`",
        );
    }

    #[test]
    fn rejects_computed_inputs_and_unused_variables() {
        assert_problem(
            &valid().replace("args = [\"{a}\", \"{b}\"]", "args = [\"{a}\", \"{b + 1}\"]"),
            "args \"{b + 1}\" computes a value",
        );
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\" }",
                "b = \"int(-1000000000, 1000000000)\", c = \"int(-1000000000, 1000000000)\" }",
            ),
            "`c` never reaches the program",
        );
    }

    #[test]
    fn catches_formulas_that_fail_on_some_inputs() {
        assert_problem(
            &valid().replace("{a + b}", "{a / b}"),
            "stdout divides by zero",
        );
        assert_problem(
            &valid().replace("{a + b}", "{a * b * a * b * a * b * a * b}"),
            "overflows",
        );
    }

    #[test]
    fn rejects_constant_output_despite_placeholders() {
        assert_problem(
            &valid().replace("{a + b}", "{min(5, 5)}"),
            "all 200 runs expect the same output",
        );
    }

    #[test]
    fn formulas_must_reproduce_the_example() {
        assert_problem(
            &valid().replace("{a + b}", "{a - b}"),
            "with example B1's inputs (a = 2, b = 3) it expects stdout \"-1\\n\", but example \
             B1 expects \"5\\n\"",
        );
        assert_problem(
            &valid().replace(
                "int(-1000000000, 1000000000)\" }",
                "int(-1000000000, -1000000000)\" }",
            ),
            "b = 3 is outside int(-1000000000, -1000000000)",
        );
        assert_problem(
            &valid().replace(
                "args = [\"{a}\", \"{b}\"]",
                "args = [\"{a}\", \"{b}\", \"x\"]",
            ),
            "it has 3 args and the example has 2",
        );
        assert_problem(
            &valid()
                .replace("args = [\"{a}\", \"{b}\"]", "args = [\"{a}{b}\"]")
                .replace("args = [\"2\", \"3\"]", "args = [\"123\"]"),
            "more than one way",
        );
    }

    /// A spec for negating whole numbers from `lo` to `hi`.
    fn negate_spec(lo: i64, hi: i64) -> Spec {
        spec::parse(&format!(
            "---
type: lengua/program-spec
title: negate
---
# Purpose
Negates a whole number between `{lo}` and `{hi}`.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
"
        ))
        .expect("valid spec")
    }

    fn negate_tests(blocks: &[&str]) -> String {
        let mut text =
            tests("[[example]]\nid = \"B1\"\nargs = [\"7\"]\nstdout = \"-7\\n\"\nexit_code = 0\n");
        for vars in blocks {
            text.push_str(&format!(
                "\n[[generated]]\nid = \"B1\"\nvars = {{ {vars} }}\nargs = [\"{{n}}\"]\n\
                 stdout = \"{{-n}}\\n\"\nexit_code = 0\n"
            ));
        }
        text
    }

    #[test]
    fn text_may_start_empty_without_purpose_saying_so() {
        let spec = spec::parse(
            "---
type: lengua/program-spec
title: lines
---
# Purpose
Counts the lines of at most `1048576` bytes of text.

# Behaviors
## B1: Counts lines
- Given the input `a` followed by a newline
- When the program runs
- Then it prints `1` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
",
        )
        .expect("valid spec");
        let tests = tests(
            "[[example]]\nid = \"B1\"\nstdin = \"a\\n\"\nstdout = \"1\\n\"\nexit_code = 0\n\n\
             [[generated]]\nid = \"B1\"\nvars = { text = 'string(0, 1048576, \"a\\n\")' }\n\
             stdin = \"{text}\"\nstdout = \"{count(text, \\\"\\\\n\\\")}\\n\"\nexit_code = 0\n",
        );
        let result = check(&tests, &spec, LABEL);
        assert!(result.is_ok(), "{result:#?}");
    }

    #[test]
    fn decimal_ranges_come_from_purpose_as_written() {
        let spec = |purpose: &str| {
            spec::parse(&format!(
                "---
type: lengua/program-spec
title: half
---
# Purpose
Halves a price between {purpose}.

# Behaviors
## B1: Halves a price
- Given the argument `3.00`
- When the program runs
- Then it prints `1.50` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
"
            ))
            .expect("valid spec")
        };
        let tests = |generator: &str| {
            tests(&format!(
                "[[example]]\nid = \"B1\"\nargs = [\"3.00\"]\nstdout = \"1.50\\n\"\nexit_code = 0\n\n\
                 [[generated]]\nid = \"B1\"\nvars = {{ x = \"{generator}\" }}\nargs = [\"{{x}}\"]\n\
                 stdout = \"{{fixed(x / 2.0, 2)}}\\n\"\nexit_code = 0\n"
            ))
        };
        let plain = spec("`0.00` and `100.00`");
        assert!(check(&tests("decimal(0.00, 100.00)"), &plain, LABEL).is_ok());
        let problems = |text: &str, spec: &Spec| match check(text, spec, LABEL) {
            Ok(_) => panic!("accepted {text}"),
            Err(problems) => problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        };
        let found = problems(&tests("decimal(0.000, 100.000)"), &plain);
        assert!(
            found.iter().any(|problem| problem
                .contains("draws numbers with 3 decimal places, but Purpose writes them with 2")),
            "{found:#?}"
        );
        let found = problems(
            &tests("decimal(0.00, 100.00)"),
            &spec("`0.00` and `$100.00`"),
        );
        assert!(
            found.iter().any(
                |problem| problem.contains("uses 100, which spec.md's Purpose")
                    && problem.contains("Purpose writes `$100.00`")
            ),
            "{found:#?}"
        );
    }

    #[test]
    fn small_ranges_run_every_value() {
        let suite = check(
            &negate_tests(&[r#"n = "int(-100, 100)""#]),
            &negate_spec(-100, 100),
            LABEL,
        )
        .expect("valid tests");
        let block = &suite.generated[0];
        assert!(block.every_value);
        assert_eq!(block.run_count(), 201);
        assert_eq!(
            block.every_value_text().as_deref(),
            Some("every value of n in its range (all 201)")
        );
        let values: BTreeSet<String> = block
            .cases(1)
            .map(|case| case.expect("computable").args[0].clone())
            .collect();
        assert_eq!(values.len(), 201);

        // A large range is sampled, as are hand-picked values.
        let suite = check(&valid(), &spec(), LABEL).expect("valid tests");
        assert!(!suite.generated[0].every_value);
        assert_eq!(suite.generated[0].run_count(), 200);
        let suite = check(
            &negate_tests(&[r#"n = "choice(-100, 7, 100)""#]),
            &negate_spec(-100, 100),
            LABEL,
        )
        .expect("valid tests");
        assert!(
            !suite.generated[0].every_value,
            "choice values aren't a range"
        );
    }

    #[test]
    fn every_value_stays_within_the_suite_budget() {
        let block = r#"n = "int(-2500, 2499)""#;
        let suite = check(&negate_tests(&[block; 5]), &negate_spec(-2500, 2499), LABEL)
            .expect("valid tests");
        let full = suite
            .generated
            .iter()
            .filter(|block| block.every_value)
            .count();
        assert_eq!(
            full, 3,
            "5000 runs each, while the suite stays within 20000"
        );
        let runs: usize = suite.generated.iter().map(Generated::run_count).sum();
        assert_eq!(runs, 3 * 5000 + 2 * 100);
    }

    #[test]
    fn arguments_stay_within_linux_limits() {
        let spec = spec::parse(
            "---
type: lengua/program-spec
title: len
---
# Purpose
Prints the length of a word of `1` to `200000` letters.

# Behaviors
## B1: Prints the length
- Given the argument `abc`
- When the program runs
- Then it prints `3` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, exit
",
        )
        .expect("valid spec");
        let tests = |generator: &str| {
            tests(&format!(
                "[[example]]\nid = \"B1\"\nargs = [\"abc\"]\nstdout = \"3\\n\"\nexit_code = 0\n\n\
                 [[generated]]\nid = \"B1\"\nvars = {{ w = '{generator}' }}\nargs = [\"{{w}}\"]\n\
                 stdout = \"{{len(w)}}\\n\"\nexit_code = 0\n"
            ))
        };
        let problems = match check(&tests(r#"string(1, 200000, "abc")"#), &spec, LABEL) {
            Ok(_) => panic!("a 200000-byte argument was accepted"),
            Err(problems) => problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        };
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("Linux limits each argument to 131072 bytes")),
            "{problems:#?}"
        );
        assert!(
            problems.iter().all(|problem| problem.len() < 2000),
            "messages shorten long values: {problems:#?}"
        );
    }

    #[test]
    fn generator_limits_come_from_purpose() {
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\"",
                "b = \"int(-1000, 1000000000)\"",
            ),
            "uses -1000, which spec.md's Purpose doesn't state",
        );
        // 2 and 3 are B1's example inputs, not a stated range: int(2, 3) would test almost nothing.
        assert_problem(
            &valid().replace(
                "vars = { a = \"int(-1000000000, 1000000000)\", b = \"int(-1000000000, \
                 1000000000)\" }",
                "vars = { a = \"int(2, 3)\", b = \"int(2, 3)\" }",
            ),
            "uses 2, which spec.md's Purpose doesn't state",
        );
        // String lengths too: the spec has 1 and 3 only as an exit code and an argument.
        assert_problem(
            &valid().replace(
                "b = \"int(-1000000000, 1000000000)\"",
                "b = 'string(1, 3, \"123\")'",
            ),
            "uses 1, which spec.md's Purpose doesn't state",
        );
    }

    #[test]
    fn blocks_of_one_behavior_share_their_formulas() {
        let second = GENERATED.replace("{a + b}", "{b + a}");
        let text = tests(&format!("{EXAMPLES}{GENERATED}{second}"));
        assert_problem(
            &text,
            "must share args, stdin, files, random, clock, stdout, stderr, files_after and exit_code",
        );
        let narrower = GENERATED.replace("runs = 200", "runs = 50");
        let ok = tests(&format!("{EXAMPLES}{GENERATED}{narrower}"));
        assert!(check(&ok, &spec(), LABEL).is_ok());
    }

    #[test]
    fn long_example_text_can_be_written_as_repeats() {
        let parse = |toml_text: &str| {
            let mut table: BTreeMap<String, Text> = toml::from_str(toml_text).expect("valid TOML");
            table.remove("v").expect("the value").expand()
        };
        assert_eq!(parse(r#"v = "plain""#), Ok("plain".into()));
        assert_eq!(
            parse(r#"v = [{ repeat = "ab", times = 3 }, "x"]"#),
            Ok("abababx".into())
        );
        assert!(parse(r#"v = [{ repeat = "", times = 3 }]"#).is_err());
        assert!(parse(r#"v = [{ repeat = "a", times = 9000000 }]"#).is_err());
        assert!(
            toml::from_str::<BTreeMap<String, Text>>(r#"v = [{ repeat = "a", count = 3 }]"#)
                .is_err()
        );
    }

    #[test]
    fn files_go_in_before_the_run_and_are_checked_after() {
        let spec_text = "---
type: lengua/program-spec
title: shout-file
---
# Purpose
Uppercases a text file of at most `1000` bytes, `notes.txt`, into `loud.txt`.

# Behaviors
## B1: Uppercases the file
- Given a file named `notes.txt` holding `hello`
- When the program runs
- Then `loud.txt` holds `HELLO`
- And it exits with code `0`

# Constraints
- Allowed system calls: openat, read, write, close, exit
";
        let spec = spec::parse(spec_text).expect("valid spec");
        let valid = tests(
            r#"
[[example]]
id = "B1"
files = { "notes.txt" = "hello" }
files_after = { "loud.txt" = "HELLO" }
exit_code = 0

[[generated]]
id = "B1"
vars = { s = 'string(0, 1000, "helo XYZ")' }
files = { "notes.txt" = "{s}" }
files_after = { "loud.txt" = "{upper(s)}" }
exit_code = 0
"#,
        );
        let suite = check(&valid, &spec, LABEL).expect("valid tests");
        let case = suite.generated[0]
            .cases(7)
            .next()
            .expect("a run")
            .expect("computable");
        assert_eq!(
            case.files_after["loud.txt"],
            case.files["notes.txt"].to_ascii_uppercase()
        );
        let problems = |text: &str| match check(text, &spec, LABEL) {
            Ok(_) => panic!("expected problems in {text}"),
            Err(problems) => problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        };
        for (from, to, needle) in [
            (
                r#"files = { "notes.txt" = "hello" }"#,
                r#"files = { "/tmp/notes.txt" = "hello" }"#,
                "is an absolute path",
            ),
            (
                r#"files = { "notes.txt" = "{s}" }"#,
                r#"files = { "../notes.txt" = "{s}" }"#,
                "must stay inside the working folder",
            ),
            (
                r#"files = { "notes.txt" = "{s}" }"#,
                r#"files = { "notes.txt" = "{upper(s)}" }"#,
                "may only hold bare variables",
            ),
            (
                "{upper(s)}",
                "{lower(s)}",
                "formula disagrees with the spec",
            ),
            (
                r#"files = { "notes.txt" = "hello" }"#,
                "",
                "the example gives []",
            ),
        ] {
            let found = problems(&valid.replacen(from, to, 1));
            assert!(
                found.iter().any(|problem| problem.contains(needle)),
                "no problem containing {needle:?} in {found:#?}"
            );
        }
    }

    #[test]
    fn reads_inputs_from_stdin_and_text_around_placeholders() {
        let spec_text = SPEC.replace("Given the arguments `2` and `3`", "Given the input `2 3`");
        let spec = spec::parse(&spec_text).expect("valid spec");
        let text = tests(
            r#"
[[example]]
id = "B1"
stdin = "2 3\n"
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1

[[generated]]
id = "B1"
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
stdin = "{a} {b}\n"
stdout = "{a + b}\n"
exit_code = 0
"#,
        );
        let suite = check(&text, &spec, LABEL).expect("valid tests");
        assert_eq!(suite.generated[0].runs, DEFAULT_RUNS);
        let wrong = text.replace("{a + b}", "{a * b}");
        assert!(check(&wrong, &spec, LABEL).is_err());
    }

    #[test]
    fn dry_runs_are_deterministic() {
        let text = valid();
        let first = check(&text, &spec(), LABEL);
        assert_eq!(first, check(&text, &spec(), LABEL));
        assert_eq!(dry_run_seed(&text), dry_run_seed(&text));
    }
}
