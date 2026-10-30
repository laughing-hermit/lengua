//! A spec's tests: each behavior's steps, read as its example, plus the generated tests under its
//! `###` heading, the same steps with formulas that compute the expected results for fresh inputs
//! drawn on every run from the table below them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::expr::{
    self, Env, EvalError, Function, Generator, HexPart, Limit, PatternPart, Template, Value,
};
use crate::hash::Sha256Hex;
use crate::spec::{ASSERTION_EXIT_CODE, Behavior, BehaviorId, Keyword, Spec, Step, Syscall};
use crate::supply::Supply;

mod steps;

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

/// A spec's checked tests.
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
    /// Line of its behavior's heading in spec.md.
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
        self.inputs(seed)
            .map(|inputs| self.case(inputs, crate::expect::now()))
    }

    /// Every run's inputs for `seed`, in the order [`Self::cases`] makes them, without their
    /// expected results, so each can be computed when its run starts.
    #[must_use]
    pub fn inputs(&self, seed: u64) -> expr::Cases<'_> {
        if self.every_value {
            expr::every_case(&self.vars)
        } else {
            expr::cases(&self.vars, self.run_count(), seed)
        }
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
            "with {}, argument {} is {} bytes, but an argument holds at most {MAX_ARG_BYTES} \
             bytes on this computer; narrow the range, or pass long text on standard input or in \
             a file",
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

/// A problem found in a spec's tests.
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

/// What a behavior's steps give the program and expect it to print. How it ends is the
/// behavior's own: its exit code, or that it keeps serving.
#[derive(Debug)]
struct ExampleTable {
    args: Vec<Text>,
    stdin: Text,
    files: BTreeMap<String, Text>,
    stdout: Text,
    stderr: Text,
    files_after: BTreeMap<String, Text>,
    random: Option<String>,
    clock: Vec<String>,
    server: Vec<ServerTable<Text>>,
    client: Vec<ClientTable<Text>>,
}

/// What the test's own server does on one connection: it receives exactly this, then replies.
#[derive(Debug)]
struct ServerTable<T> {
    receives: T,
    replies: T,
}

/// What the test's own client does on one connection: it sends this, then reads the answer.
#[derive(Debug)]
struct ClientTable<T> {
    sends: T,
    receives: T,
}

/// A step's text: plain, or parts with repeated runs, so a long input such as a file one byte
/// over a limit stays short to write: ``- Given a file `big.txt` holding `a` repeated `1048576`
/// times followed by `x` ``. An expected output may instead be a rule, ``it prints text matching
/// `rolled {any(1, 6)}` ``.
#[derive(Debug)]
enum Text {
    Plain(String),
    Parts(Vec<TextPart>),
    Rule(Rule),
}

/// Expected output that allows a range of numbers where the program chooses one.
#[derive(Debug)]
struct Rule {
    matches: String,
}

impl Default for Text {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}

#[derive(Debug)]
enum TextPart {
    Plain(String),
    Repeat(Repeat),
}

#[derive(Debug)]
struct Repeat {
    repeat: String,
    times: u64,
}

impl Text {
    /// Expected output: the text written out, or a rule's text with its holes.
    fn expected(self) -> Result<String, String> {
        match self {
            Self::Rule(Rule { matches }) => Template::parse(&matches)
                .map_err(|error| format!("text matching {matches:?}: {error}"))?
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
                    "a rule goes only in what the program prints; its inputs are exact".into(),
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
                            "repeating {repeat:?} {times} times must repeat some text, and the \
                             whole value must stay within {MAX_FILE_BYTES} bytes"
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

/// Text a step wrote, with the line of that step.
#[derive(Debug, Clone, Default)]
struct Written {
    text: String,
    line: usize,
}

/// A behavior's generated tests as the steps under its `###` heading write them: templates over
/// the variables of its table, each with the line of its step.
#[derive(Debug)]
struct OutlineSteps {
    /// Line of the heading.
    line: usize,
    args: Vec<Written>,
    stdin: Written,
    files: BTreeMap<String, Written>,
    stdout: Written,
    stderr: Written,
    files_after: BTreeMap<String, Written>,
    random: Option<Written>,
    clock: Vec<Written>,
    server: Vec<(Written, Written)>,
    client: Vec<(Written, Written)>,
    /// The exit code as written: a number or a formula.
    exit_code: Option<Written>,
    serves: bool,
}

/// One row of a behavior's table: one generated test's variables, and its runs when it gives
/// them.
#[derive(Debug)]
struct OutlineRow {
    line: usize,
    runs: Option<String>,
    vars: Vec<(String, String)>,
}

/// The spec's tests: each behavior's steps, read as its example, and the generated tests under
/// its `###` heading, checked against the spec. Every problem is reported (up to 20).
///
/// # Errors
///
/// Returns the problems when the tests aren't valid.
pub fn check(spec: &Spec) -> Result<TestSuite, Vec<Problem>> {
    let mut checker = Checker {
        spec,
        problems: Vec::new(),
        hidden: 0,
    };
    let mut tables = Vec::new();
    for behavior in &spec.behaviors {
        match steps::example(behavior, spec) {
            Ok(table) => tables.push((behavior, table)),
            Err(problems) => {
                for problem in problems {
                    checker.problem(problem.line, problem.message);
                }
            }
        }
    }
    let examples = checker.examples(tables);
    let mut generated = Vec::new();
    let mut written = String::new();
    for behavior in &spec.behaviors {
        let Some(outline) = &behavior.outline else {
            continue;
        };
        written.push_str(&outline.text);
        match steps::generated(behavior, outline, spec) {
            Ok((steps, rows)) => generated.extend(checker.outline(behavior, &steps, rows)),
            Err(problems) => {
                for problem in problems {
                    checker.problem(problem.line, problem.message);
                }
            }
        }
    }
    let generated = checker.generated_blocks(generated, dry_run_seed(&written));
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

/// Each behavior's example outputs as TOML (`stdout`, `stderr`, `files_after`), each exact text
/// or `{ matches = … }`, for comparing two versions of a spec's tests.
#[must_use]
pub fn example_outputs(spec: &Spec) -> Vec<(String, toml::Table)> {
    let value = |text: Text| match text {
        Text::Rule(Rule { matches }) => {
            let mut rule = toml::Table::new();
            rule.insert("matches".into(), toml::Value::String(matches));
            toml::Value::Table(rule)
        }
        other => toml::Value::String(other.expand().unwrap_or_default()),
    };
    spec.behaviors
        .iter()
        .filter_map(|behavior| {
            let table = steps::example(behavior, spec).ok()?;
            let mut outputs = toml::Table::new();
            outputs.insert("stdout".into(), value(table.stdout));
            outputs.insert("stderr".into(), value(table.stderr));
            let files: toml::Table = table
                .files_after
                .into_iter()
                .map(|(name, text)| (name, value(text)))
                .collect();
            outputs.insert("files_after".into(), toml::Value::Table(files));
            Some((behavior.id.to_string(), outputs))
        })
        .collect()
}

/// Each behavior's generated tests as TOML, one table a row with its behavior's `id`: `runs`,
/// `vars`, `stdout`, `stderr` and `files_after`, for comparing two versions of a spec's tests.
#[must_use]
pub fn generated_tables(spec: &Spec) -> Vec<toml::Table> {
    let mut tables = Vec::new();
    for behavior in &spec.behaviors {
        let Some(outline) = &behavior.outline else {
            continue;
        };
        let Ok((steps, rows)) = steps::generated(behavior, outline, spec) else {
            continue;
        };
        let text = |written: &Written| toml::Value::String(written.text.clone());
        for row in rows {
            let mut table = toml::Table::new();
            table.insert("id".into(), toml::Value::String(behavior.id.to_string()));
            if let Some(runs) = row
                .runs
                .as_deref()
                .and_then(|runs| runs.parse::<i64>().ok())
            {
                table.insert("runs".into(), toml::Value::Integer(runs));
            }
            let vars: toml::Table = row
                .vars
                .into_iter()
                .map(|(name, source)| (name, toml::Value::String(source)))
                .collect();
            table.insert("vars".into(), toml::Value::Table(vars));
            table.insert("stdout".into(), text(&steps.stdout));
            table.insert("stderr".into(), text(&steps.stderr));
            let files: toml::Table = steps
                .files_after
                .iter()
                .map(|(name, written)| (name.clone(), text(written)))
                .collect();
            table.insert("files_after".into(), toml::Value::Table(files));
            tables.push(table);
        }
    }
    tables
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

/// Who a test talks to over the network, and the port its server step names.
struct Peers<'p> {
    server: bool,
    client: bool,
    port: Option<&'p str>,
}

/// The port a server step names, such as the `8080` of ``a server on port `8080` that …``.
fn server_port(steps: &[Step]) -> Option<&str> {
    steps
        .iter()
        .find(|step| step.text.contains("server on port"))
        .and_then(|step| step.values.first())
        .map(String::as_str)
}

/// Generated tests to start from for `behavior`: its own steps again, with each value it's given
/// a variable, each value it gives back but its exit code a formula to write, and a table of the
/// variables.
fn skeleton(behavior: &Behavior) -> String {
    let mut names = Vec::new();
    let mut lines = vec!["### For any …".to_owned()];
    let mut phase = Keyword::Given;
    for step in &behavior.steps {
        phase = match step.keyword {
            Keyword::And | Keyword::But => phase,
            keyword => keyword,
        };
        let ending = step.text.contains("exits with code");
        let mut text = step.text.clone();
        for value in &step.values {
            let written = match phase {
                Keyword::Given => {
                    let name = format!("{}", char::from(b'a' + (names.len() % 26) as u8));
                    let placeholder = format!("{{{name}}}");
                    names.push(name);
                    placeholder
                }
                Keyword::Then if !ending => "{…}".to_owned(),
                _ => continue,
            };
            text = text.replacen(&format!("`{value}`"), &format!("`{written}`"), 1);
        }
        lines.push(format!("- {} {text}", step.keyword.name()));
    }
    if names.is_empty() {
        names.push("a".to_owned());
    }
    lines.push(String::new());
    lines.push(format!("| {} |", names.join(" | ")));
    lines.push(format!("|{}", "---|".repeat(names.len())));
    lines.push(format!("|{}", " `…` |".repeat(names.len())));
    lines.join("\n")
}

/// A generated test's field, in the words its steps write it with.
fn words(field: &str) -> String {
    match field {
        "args" => "the arguments".into(),
        "stdin" => "standard input".into(),
        "files" => "its files".into(),
        "stdout" => "what it prints".into(),
        "stderr" => "what it prints to standard error".into(),
        "files_after" => "the files it leaves".into(),
        "random" => "the random bytes".into(),
        "clock" => "the clock".into(),
        "exit_code" | "exit code" => "its exit code".into(),
        "server" | "server.receives" => "what the server receives".into(),
        "server.replies" => "what the server replies".into(),
        "client" | "client.sends" => "what the client sends".into(),
        "client.receives" => "what the client gets back".into(),
        other => match (
            other.strip_prefix("files."),
            other.strip_prefix("files_after."),
        ) {
            (Some(file), _) => format!("the file `{file}`"),
            (_, Some(file)) => format!("what `{file}` holds after"),
            _ => other.to_owned(),
        },
    }
}

/// The seed of the dry run, fixed by the tests as written so `check` always gives the same
/// verdict.
fn dry_run_seed(text: &str) -> u64 {
    let digest = Sha256Hex::of(text.as_bytes());
    u64::from_str_radix(&digest.as_str()[..16], 16).unwrap_or(0)
}

struct Checker<'a> {
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

    /// Each behavior's example, as its steps describe it. The spec already holds each step to a
    /// phrasing and each behavior to one ending, so what's left is what the steps' values make:
    /// texts that fit, file names, and what the spec lets a test supply.
    fn examples(&mut self, tables: Vec<(&'a Behavior, ExampleTable)>) -> Vec<Example> {
        let mut examples = Vec::new();
        for (behavior, table) in tables {
            let id = behavior.id;
            let line = behavior.line;
            let name = format!("{id}'s example");
            let mut expand = |what: &str, text: Text, output: bool| {
                let text = if output {
                    text.expected()
                } else {
                    text.expand()
                };
                text.unwrap_or_else(|problem| {
                    self.problem(Some(line), format!("{name}: {what}: {problem}"));
                    String::new()
                })
            };
            let args: Vec<String> = table
                .args
                .into_iter()
                .map(|arg| expand("the arguments", arg, false))
                .collect();
            let stdin = expand("standard input", table.stdin, false);
            let stdout = expand("what it prints", table.stdout, true);
            let stderr = expand("what it prints to standard error", table.stderr, true);
            let files: BTreeMap<String, String> = table
                .files
                .into_iter()
                .map(|(file, contents)| {
                    let contents = expand(&format!("the file `{file}`"), contents, false);
                    (file, contents)
                })
                .collect();
            let files_after: BTreeMap<String, String> = table
                .files_after
                .into_iter()
                .map(|(file, contents)| {
                    let contents = expand(&format!("what `{file}` holds after"), contents, true);
                    (file, contents)
                })
                .collect();
            // What the test's server and client send is exact; what the program answers its client
            // may hold rules.
            let server: Vec<Reply> = table
                .server
                .into_iter()
                .map(|table| Reply {
                    receives: expand("what the server receives", table.receives, false),
                    replies: expand("what the server replies", table.replies, false),
                })
                .collect();
            let client: Vec<Exchange> = table
                .client
                .into_iter()
                .map(|table| Exchange {
                    sends: expand("what the client sends", table.sends, false),
                    receives: expand("what the client gets back", table.receives, true),
                })
                .collect();
            for problem in file_set_problems(files.keys().chain(files_after.keys())) {
                self.problem(Some(line), format!("{name}: {problem}"));
            }
            self.check_supply(line, &name, table.random.is_some(), !table.clock.is_empty());
            if let Err(problem) = Supply::parse(table.random.as_deref(), &table.clock) {
                self.problem(Some(line), format!("{name}: {problem}"));
            }
            self.check_peers(
                line,
                &name,
                Peers {
                    server: !server.is_empty(),
                    client: !client.is_empty() || behavior.serves,
                    port: server_port(&behavior.steps),
                },
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
                         server; add `- And a server that receives `…` and replies `…``"
                    ),
                );
            }
            examples.push(Example {
                id,
                line,
                args,
                stdin,
                files,
                stdout,
                stderr,
                files_after,
                exit_code: behavior.exit_code,
                random: table.random,
                clock: table.clock,
                server,
                client,
                serves: behavior.serves,
            });
        }
        examples
    }

    fn generated_blocks(&mut self, mut blocks: Vec<Generated>, seed: u64) -> Vec<Generated> {
        mark_every_value(&mut blocks);
        for block in &blocks {
            self.dry_run(block, seed);
        }
        blocks
    }

    /// A behavior's generated tests: its outline's templates, read once, with each row of its
    /// table's variables. Nothing when any of it has a problem.
    fn outline(
        &mut self,
        behavior: &'a Behavior,
        steps: &OutlineSteps,
        rows: Vec<OutlineRow>,
    ) -> Vec<Generated> {
        let id = behavior.id;
        let name = format!("generated {id}");
        let before = self.problems.len() + self.hidden;
        let Some(shared) = self.templates(behavior, steps, &name) else {
            return Vec::new();
        };
        let mut blocks = Vec::new();
        for row in rows {
            let line = row.line;
            if row.vars.is_empty() {
                self.problem(
                    Some(line),
                    format!(
                        "{name}'s table has no variables: give each one a column, such as \
                         `| a |`, and its generator in each row, such as `` `int(-10, 10)` ``"
                    ),
                );
            }
            let runs = match row.runs.as_deref().map(str::parse::<u32>) {
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
            let mut vars = Vec::new();
            for (var, source) in &row.vars {
                if !expr::is_variable_name(var) {
                    self.problem(
                        Some(line),
                        format!(
                            "{name}: `{var}` can't be a variable name; use lowercase letters, \
                             digits and _, not a function name"
                        ),
                    );
                    continue;
                }
                if crate::expect::has_holes(source) {
                    self.problem(
                        Some(line),
                        format!(
                            "{name}: {var}'s generator holds one of the characters U+FDD0, \
                             U+FDD1 and U+FDD2, which lengua reserves for marking rules; remove \
                             them"
                        ),
                    );
                    continue;
                }
                match Generator::parse(source) {
                    Ok(generator) => vars.push((var.clone(), generator)),
                    Err(error) => self.problem(Some(line), format!("{name}: {var}: {error}")),
                }
            }
            let block = Generated {
                line,
                runs,
                vars,
                ..shared.clone()
            };
            self.check_ranges(&block, &name);
            blocks.push(block);
        }
        // The rows share their steps and their columns, so the variables check once.
        if let Some(first) = blocks.first() {
            self.check_variables(first, &name, steps.line);
        }
        if self.problems.len() + self.hidden == before {
            blocks
        } else {
            Vec::new()
        }
    }

    /// The templates an outline's steps write, each problem on the line of its step, as one
    /// generated test with no variables yet.
    fn templates(
        &mut self,
        behavior: &Behavior,
        steps: &OutlineSteps,
        name: &str,
    ) -> Option<Generated> {
        let id = behavior.id;
        let behavior_exit = behavior.exit_code;
        let mut template = |field: &str, written: &Written| match Template::parse(&written.text) {
            Ok(template) => Some(template),
            Err(error) => {
                self.problem(
                    Some(written.line),
                    format!("{name}: {}: {error}", words(field)),
                );
                None
            }
        };
        let args: Option<Vec<Template>> =
            steps.args.iter().map(|arg| template("args", arg)).collect();
        let stdin = template("stdin", &steps.stdin);
        let files: Option<BTreeMap<String, Template>> = steps
            .files
            .iter()
            .map(|(file, written)| {
                Some((file.clone(), template(&format!("files.{file}"), written)?))
            })
            .collect();
        let stdout = template("stdout", &steps.stdout);
        let stderr = template("stderr", &steps.stderr);
        let files_after: Option<BTreeMap<String, Template>> = steps
            .files_after
            .iter()
            .map(|(file, written)| {
                Some((
                    file.clone(),
                    template(&format!("files_after.{file}"), written)?,
                ))
            })
            .collect();
        let random = match &steps.random {
            None => Some(None),
            Some(written) => template("random", written).map(Some),
        };
        let clock: Option<Vec<Template>> = steps
            .clock
            .iter()
            .map(|reading| template("clock", reading))
            .collect();
        let server: Option<Vec<(Template, Template)>> = steps
            .server
            .iter()
            .map(|(receives, replies)| {
                Some((
                    template("server.receives", receives)?,
                    template("server.replies", replies)?,
                ))
            })
            .collect();
        let client: Option<Vec<(Template, Template)>> = steps
            .client
            .iter()
            .map(|(sends, receives)| {
                Some((
                    template("client.sends", sends)?,
                    template("client.receives", receives)?,
                ))
            })
            .collect();
        let exit_code = match &steps.exit_code {
            None => None,
            Some(written) if written.text.contains('{') => match Template::parse(&written.text) {
                Ok(template) => Some(ExitCode::Formula(template)),
                Err(error) => {
                    self.problem(
                        Some(written.line),
                        format!("{name}: its exit code: {error}"),
                    );
                    return None;
                }
            },
            Some(written) => match written.text.parse::<u8>() {
                Ok(code) if code == behavior_exit => Some(ExitCode::Fixed(code)),
                Ok(code) => {
                    self.problem(
                        Some(written.line),
                        format!(
                            "{name} expects exit code {code}, but the spec says {id} exits with \
                             code {behavior_exit}"
                        ),
                    );
                    return None;
                }
                Err(_) => {
                    self.problem(
                        Some(written.line),
                        format!("{name}: exit codes go from 0 to 255, not {}", written.text),
                    );
                    return None;
                }
            },
        };
        let line = steps.line;
        for problem in file_set_problems(steps.files.keys().chain(steps.files_after.keys())) {
            self.problem(Some(line), format!("{name}: {problem}"));
        }
        self.check_supply(line, name, steps.random.is_some(), !steps.clock.is_empty());
        self.check_peers(
            line,
            name,
            Peers {
                server: !steps.server.is_empty(),
                client: !steps.client.is_empty() || steps.serves,
                port: behavior
                    .outline
                    .as_ref()
                    .and_then(|outline| server_port(&outline.steps)),
            },
        );
        self.check_ending(line, name, behavior, exit_code.is_some(), steps.serves)?;
        let exit_code = exit_code.unwrap_or(ExitCode::Fixed(behavior_exit));
        Some(Generated {
            id,
            line,
            runs: DEFAULT_RUNS,
            vars: Vec::new(),
            args: args?,
            stdin: stdin?,
            files: files?,
            stdout: stdout?,
            stderr: stderr?,
            files_after: files_after?,
            exit_code,
            every_value: false,
            random: random?,
            clock: clock?,
            server: server?,
            client: client?,
            serves: steps.serves,
        })
    }

    /// A test has its own server only for a program that connects, and its own client, or
    /// expects the program to keep serving, only for one that serves.
    fn check_peers(&mut self, line: usize, name: &str, peers: Peers<'_>) {
        if peers.server && self.spec.network.connects.is_empty() {
            self.problem(
                Some(line),
                format!(
                    "{name} has a server for the program to connect to, but spec.md doesn't say \
                     where it may connect: add `- Network: connects to TCP port {}` under \
                     `# Constraints`",
                    peers.port.unwrap_or("N")
                ),
            );
        }
        if peers.client && self.spec.network.listens.is_none() {
            self.problem(
                Some(line),
                format!(
                    "{name} has a client or keeps serving, but spec.md doesn't say where it \
                     serves: add `- Network: listens on TCP port N` under `# Constraints`, with \
                     the port it serves on"
                ),
            );
        }
    }

    /// A test ends as its behavior does: with the exit code the spec states, or, for a behavior
    /// that keeps serving, still serving. Returns the exit code the test needs when it gives
    /// none, or `None` after a problem.
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
                "{name}: {id} keeps serving, so its generated tests end with `- And it keeps \
                 serving` too"
            ),
            (false, true, _) => format!(
                "{name} keeps serving, but {id} exits with code {code}: end its generated tests \
                 with `- And it exits with code `{code}``",
                code = behavior.exit_code
            ),
            (true, true, true) => {
                format!("{name} both exits and keeps serving; end it one way, as {id} does")
            }
            (false, false, false) => format!(
                "{name} needs its exit code, the one {id} states: end its generated tests with \
                 `- And it exits with code `{code}``",
                code = behavior.exit_code
            ),
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
                    "{name} supplies random bytes, but spec.md's `- Uses:` line doesn't name \
                     `random bytes`; add it there, or drop the random bytes"
                ),
            );
        }
        if clock && !allows(Syscall::ClockGettime) {
            self.problem(
                Some(line),
                format!(
                    "{name} supplies clock readings, but spec.md's `- Uses:` line doesn't name \
                     `the clock`; add it there, or drop the readings"
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
                            "{name}: {var} = {generator} fills {count} random byte(s), which the \
                             computer could give any value, so it draws them all: `{every}`"
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
                            "{name}: {var} = {generator} is a clock reading; draw seconds since \
                             1970 with int(lo, hi), or decimal(lo, hi) with up to 9 places, from \
                             0 up"
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
                        "{name}: {var} = {generator} uses {limit}, which spec.md's Purpose \
                         doesn't state. Generators take their ranges from Purpose: use the \
                         numbers it gives in backticks, or add the range to Purpose in spec.md \
                         and call check again{hint}"
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
                    "{name}: {var} = {generator} draws numbers with {places} decimal \
                     places, but Purpose writes them with {written}; write the bounds the way \
                     Purpose does"
                ),
            );
        }
    }

    fn check_variables(&mut self, block: &Generated, name: &str, line: usize) {
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
                    Some(line),
                    format!(
                        "{name}: {} {:?} computes a value; what a test gives the program (its \
                         arguments, standard input, files and clock, the server's replies and the \
                         client's messages) may only hold bare variables like {{a}}, so move the \
                         computation to what it expects",
                        words(field),
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
                Some(line),
                format!(
                    "{name} uses port, the port of the test's own server, but has no server; \
                     add `- And a server on port `…` that receives `…` and replies `…``"
                ),
            );
        }
        if port_built_in {
            inputs.remove(PORT);
        }
        if let Some(random) = &block.random {
            if random.hex_parts().is_none() {
                self.problem(
                    Some(line),
                    format!(
                        "{name}: the random bytes {:?} may only hold hex digits and hex(x, n), a \
                         variable and 1 to 8 bytes, such as `{{hex(roll, 1)}}`",
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
                Some(line),
                format!("{name}: hex(x, n) only fills the random bytes"),
            );
        }
        if !block.clock.is_empty() && block.uses_now() {
            self.problem(
                Some(line),
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
                Some(line),
                format!(
                    "{name}: what a test gives the program can't hold now, the time the run \
                     starts; the program reads the time itself"
                ),
            );
        }
        if now_built_in
            && block
                .outputs()
                .any(|template| template.uses_outside_any(NOW))
        {
            self.problem(
                Some(line),
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
                Some(line),
                format!("{name} uses `{var}`, which isn't a column of its table"),
            );
        }
        for var in declared.difference(&inputs) {
            self.problem(
                Some(line),
                format!(
                    "{name}: `{var}` never reaches the program; use it in what the steps give \
                     the program, or remove its column"
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
                            "{name}: with {}, {} {error}; narrow the generators or fix the \
                             formula",
                            describe_inputs(&inputs),
                            words(field)
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
                        "{name}: what the server receives must be exact, since the test's \
                         server reads exactly those bytes; a rule such as any(…) goes in what \
                         the program prints or answers"
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

    /// Every behavior whose example has inputs needs generated tests, which talk and supply as
    /// its example does.
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
                            "generated {id} must talk over as many connections as {id}'s \
                             steps: {} with the test's server and {} with its client",
                            example.server.len(),
                            example.client.len(),
                            id = block.id,
                        ),
                    );
                }
                if block.random.is_some() != example.random.is_some()
                    || block.clock.is_empty() != example.clock.is_empty()
                {
                    self.problem(
                        Some(block.line),
                        format!(
                            "generated {id} must supply what {id}'s steps do: random bytes in \
                             both or neither, and clock readings in both or neither",
                            id = block.id
                        ),
                    );
                }
            }
            if has_inputs && blocks.is_empty() {
                self.problem(
                    Some(example.line),
                    format!(
                        "{id} has inputs, so it needs generated tests that try fresh inputs on \
                         every run. Write them under its steps, starting from this, with a \
                         formula where `{{…}}` stands and a generator in each cell:\n\n{}",
                        self.spec
                            .behavior(example.id)
                            .map_or_else(String::new, skeleton),
                        id = example.id
                    ),
                );
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
                        format!("generated {id} can't be matched to {id}'s steps: {reason}"),
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
                        "the inputs of {id}'s steps aren't in the generated ranges ({}); the \
                         ranges must include the spec's own example",
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
                            "generated {id}: with the inputs of {id}'s steps ({}), {} {error}",
                            describe_inputs(&inputs),
                            words(field)
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
                        "generated {id}'s formula disagrees with the spec: with the inputs of \
                         {id}'s steps ({}) it expects {} {formula}, but the steps expect \
                         {spec}. Fix the formula",
                        describe_inputs(&case.inputs),
                        words(field)
                    ),
                );
            }
        }
    }
}

/// Reads the variables' values back out of what the behavior's steps give the program.
fn bind(block: &Generated, example: &Example) -> Result<Env, String> {
    if block.args.len() != example.args.len() {
        return Err(format!(
            "it gives {} arguments and the steps give {}",
            block.args.len(),
            example.args.len()
        ));
    }
    if !block.files.keys().eq(example.files.keys()) {
        return Err(format!(
            "it gives the files {:?} and the steps give {:?}",
            block.files.keys().collect::<Vec<_>>(),
            example.files.keys().collect::<Vec<_>>()
        ));
    }
    if block.clock.len() != example.clock.len() {
        return Err(format!(
            "it gives {} clock readings and the steps give {}",
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
    // The test server's port reads as its stand-in, as the steps' port argument does.
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
                "no values of its vars give all of what the steps give the program at once; \
                 check that a variable used twice holds the same text in both places"
                    .into()
            } else {
                format!(
                    "no values of its vars turn what it gives the program into what the steps \
                     give it: {}",
                    misfits.join("; ")
                )
            })
        }
        1 => Ok(solutions.remove(0)),
        _ => Err(
            "the steps' inputs fit its placeholders in more than one way; separate \
             neighboring placeholders with fixed text"
                .into(),
        ),
    }
}

/// Reads the variables of a block's `random` template out of the steps' random bytes.
fn read_hex(template: &Template, bytes: &str, env: &mut Env) -> Result<(), String> {
    let misfit = || {
        format!(
            "the steps' random bytes {bytes:?} don't fit its random {:?}",
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

    /// The add spec, with `outline` as B1's generated tests.
    fn add(outline: &str) -> String {
        format!(
            "---
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

{outline}
## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`
"
        )
    }

    const GENERATED: &str = "### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |
";

    fn valid() -> String {
        add(GENERATED)
    }

    /// The tests of `spec_text`, a spec lengua accepts, or their problems with their lines.
    fn tests_of(spec_text: &str) -> Result<TestSuite, Vec<String>> {
        let spec = spec::parse(spec_text).unwrap_or_else(|problems| panic!("{problems:#?}"));
        check(&spec).map_err(|problems| problems.iter().map(ToString::to_string).collect())
    }

    fn assert_problem(spec_text: &str, needle: &str) {
        let problems = tests_of(spec_text).expect_err("problems");
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "no problem containing {needle:?} in {problems:#?}"
        );
    }

    /// `text` with each `from` replaced by `to`; `from` must be there.
    fn edit(text: &str, from: &str, to: &str) -> String {
        assert!(text.contains(from), "{from:?} isn't in {text}");
        text.replace(from, to)
    }

    #[test]
    fn accepts_valid_tests() {
        let suite = tests_of(&valid()).expect("valid tests");
        assert_eq!(suite.examples.len(), 2);
        assert_eq!(suite.examples[0].args, ["2", "3"]);
        assert_eq!(suite.examples[0].stdout, "5\n");
        assert_eq!(suite.examples[1].line, 25);
        assert_eq!(suite.examples[1].stderr, "usage: add <a> <b>\n");
        assert_eq!(suite.examples[1].exit_code, 1);
        let block = &suite.generated[0];
        assert_eq!(block.runs, 200);
        assert_eq!(block.line, 23, "a generated test's line is its row's");
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

    #[test]
    fn variants_are_rows_of_one_table() {
        let two = edit(
            &valid(),
            "| 200 |\n",
            "| 200 |\n| `int(-1000000000, 1000000000)` | `choice(0)` | |\n",
        );
        let suite = tests_of(&two).expect("valid tests");
        assert_eq!(suite.generated.len(), 2);
        assert_eq!(suite.generated[1].runs, DEFAULT_RUNS);
        assert_eq!(suite.generated[1].line, 24);
        assert_eq!(suite.generated[1].stdout, suite.generated[0].stdout);
    }

    #[test]
    fn problems_point_at_their_step_or_their_row() {
        let problems = tests_of(&edit(&valid(), "{a + b}", "{a +* b}")).expect_err("problems");
        assert!(
            problems
                .iter()
                .any(|problem| problem.starts_with("line 18:")
                    && problem.contains("generated B1: what it prints: in {a +* b}")),
            "{problems:#?}"
        );
        let problems = tests_of(&edit(&valid(), "| 200 |", "| lots |")).expect_err("problems");
        assert!(
            problems
                .iter()
                .any(|problem| problem.starts_with("line 23:")
                    && problem.contains("runs must be from 1 to 1000")),
            "{problems:#?}"
        );
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
- Then it prints text matching `rolled {any(1, 6)}` followed by a newline
- And it exits with code `0`

### For any `player`
- Given the argument `{player}`
- When the program runs
- Then it prints `rolled {any(1, 6)}` followed by a newline
- And it exits with code `0`

| player |
|---|
| `int(1, 9)` |

# Constraints
- Uses: random bytes
";

    #[test]
    fn rules_allow_any_number_in_a_range() {
        let suite = tests_of(DICE).expect("valid tests");
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
        let rule = "matching `rolled {any(1, 6)}`";
        let formula = "prints `rolled {any(1, 6)}`";
        for (from, to, needle) in [
            (
                rule,
                "matching `rolled {player}`",
                "a rule's placeholders are any(lo, hi) only",
            ),
            (
                rule,
                "matching `rolled {any(6, 1)}`",
                "smallest number after its largest",
            ),
            (
                formula,
                "prints `rolled {any(1, 7)}`",
                "generated B1's formula disagrees with the spec",
            ),
            (
                formula,
                "prints `rolled {len(any(1, 6))}`",
                "len can't look inside any(…)",
            ),
            (
                "the argument `1`",
                "the argument `1\u{FDD1}`",
                "U+FDD0, U+FDD1 and U+FDD2",
            ),
            (
                "`int(1, 9)`",
                "`choice(\"1\u{FDD1}\")`",
                "U+FDD0, U+FDD1 and U+FDD2",
            ),
            // Where the number ends must be clear: "30" isn't a 3 and a 0.
            (
                rule,
                "matching `rolled {any(1, 6)}0`",
                "followed directly by a digit",
            ),
            (
                formula,
                "prints `rolled {any(1, 6)}{player}`",
                "followed directly by a digit",
            ),
        ] {
            assert_problem(&edit(DICE, from, to), needle);
        }
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
- Then it prints text matching `{any(now - 2, now + 2)}` followed by a newline
- And it exits with code `0`

### For any `player`
- Given the argument `{player}`
- When the program runs
- Then it prints `{any(now - 2, now + 2)}` followed by a newline
- And it exits with code `0`

| player |
|---|
| `int(1, 9)` |

# Constraints
- Uses: the clock
";
        let suite = tests_of(CLOCK).expect("valid tests");
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
        let problems = |from: &str, to: &str| {
            tests_of(&edit(CLOCK, from, to))
                .expect_err("problems")
                .join("\n")
        };
        let exact = problems("prints `{any(now - 2, now + 2)}`", "prints `{now}`");
        assert!(exact.contains("may only bound a rule"), "{exact}");
        let input = problems("argument `{player}`", "arguments `{player}` and `{now}`");
        assert!(input.contains("can't hold now"), "{input}");
        // A table that names now draws it like any variable.
        let declared = problems(
            "| player |\n|---|\n| `int(1, 9)` |",
            "| player | now |\n|---|---|\n| `int(1, 9)` | `int(1, 9)` |",
        );
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

    #[test]
    fn tests_supply_random_bytes_from_variables() {
        let suite = tests_of(&spec_text("dice")).expect("valid tests");
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
        let dice = spec_text("dice");
        for (from, to, needle) in [
            // Random bytes take every value the computer could give.
            (
                "| `int(0, 255)` | `int(0, 255)` |",
                "| `int(0, 9)` | `int(0, 255)` |",
                "draws them all: `int(0, 255)`",
            ),
            (
                "`rolled {if(",
                "`rolled {hex(first, 1)}{if(",
                "hex(x, n) only fills the random bytes",
            ),
            (
                "`{hex(first, 1)}` and `{hex(second, 1)}`",
                "`{first}` and `{second}`",
                "may only hold hex digits and hex(x, n)",
            ),
            ("`fc` and `2a`", "`fc` and `2g`", "pairs of hex digits"),
            // The steps and their generated tests supply alike.
            (
                "Given the random bytes `fc` and `2a`",
                "Given no arguments",
                "must supply what B1's steps do",
            ),
        ] {
            assert_problem(&edit(&dice, from, to), needle);
        }
        // A program whose spec doesn't name random bytes gets none.
        assert_problem(
            &edit(
                &valid(),
                "`3`\n- When",
                "`3`\n- And the random bytes `00`\n- When",
            ),
            "`- Uses:` line doesn't name `random bytes`",
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
- Given the argument `1`
- And the clock at `1735693200.5`
- When the program runs
- Then it prints `1` followed by a newline
- And it exits with code `0`

### For any `reader` and `t`
- Given the argument `{reader}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{floor(t) % 86400 / 3600}` followed by a newline
- And it exits with code `0`

| reader | t |
|---|---|
| `int(1, 9)` | `decimal(0.0, 4102444800.0)` |

# Constraints
- Uses: the clock
";
        let suite = tests_of(HOUR).expect("valid tests");
        let reading = suite.examples[0].supply().clock[0];
        assert_eq!(
            (reading.seconds, reading.nanoseconds),
            (1_735_693_200, 500_000_000)
        );
        assert!(suite.generated[0].clock_only().contains("t"));
        let problems = |from: &str, to: &str| {
            tests_of(&edit(HOUR, from, to))
                .expect_err("problems")
                .join("\n")
        };
        let early = problems("decimal(0.0,", "decimal(-1.0,");
        assert!(early.contains("is a clock reading"), "{early}");
        let now = problems("{floor(t) % 86400 / 3600}", "{any(now - 1, now)}");
        assert!(now.contains("supplies the clock"), "{now}");
    }

    #[test]
    fn network_tests_read_like_any_example() {
        // A spec writes a protocol's line breaks as `\r\n`, and long messages may be written with
        // repeats.
        let crlf = edit(
            &edit(
                &spec_text("echo"),
                "`hello` followed by a newline",
                "`hello\\r\\n`",
            ),
            "`{line}` followed by a newline",
            "`{line}\\r\\n`",
        );
        let suite = tests_of(&crlf).expect("`hello\\r\\n` is in the example");
        assert_eq!(suite.examples[0].client[0].sends, "hello\r\n");
        let repeated = edit(
            &crlf,
            "sends `hello\\r\\n`",
            "sends `hel` repeated `1` times followed by `lo\\r\\n`",
        );
        tests_of(&repeated).expect("repeats work in network steps");
        // When the generated tests can't produce the example, the message names what doesn't fit.
        let misfit = edit(&crlf, "sends `{line}\\r\\n`", "sends `{line}!\\r\\n`");
        let problems = tests_of(&misfit).expect_err("a misfit");
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains(r#""{line}!\r\n" can't become "hello\r\n""#)),
            "{problems:#?}"
        );
    }

    #[test]
    fn reports_problems_in_generated_tests() {
        let problem = |from: &str, to: &str, needle: &str| {
            assert_problem(&edit(&valid(), from, to), needle);
        };
        problem(
            "- Then it prints `{a + b}`",
            "- Then the output is `{a + b}`",
            "B1's generated tests: lengua reads each step as one of its phrasings",
        );
        problem(
            "`{a + b}` followed by a newline\n- And it exits with code `0`\n",
            "`{a + b}` followed by a newline\n",
            "needs its exit code, the one B1 states",
        );
        problem(
            "| a | b | runs |",
            "| a | a | runs |",
            "B1's table names `a` twice",
        );
    }

    #[test]
    fn requires_generated_tests_for_behaviors_with_inputs() {
        // The problem holds the generated tests to start from: the behavior's own steps.
        assert_problem(
            &add(""),
            "B1 has inputs, so it needs generated tests that try fresh inputs on every run. \
             Write them under its steps, starting from this, with a formula where `{…}` stands \
             and a generator in each cell:\n\n### For any …\n- Given the arguments `{a}` and \
             `{b}`\n- When the program runs\n- Then it prints `{…}` followed by a newline\n- And \
             it exits with code `0`\n\n| a | b |\n|---|---|\n| `…` | `…` |",
        );
    }

    #[test]
    fn a_missing_network_line_comes_with_its_port() {
        let fetch = spec_text("fetch");
        let line = fetch
            .lines()
            .find(|line| line.starts_with("- Network:"))
            .expect("the network line");
        let without = edit(&fetch, line, "- Uses: nothing");
        let spec = spec::parse(&without).expect("valid spec");
        let problems: Vec<String> = check(&spec)
            .expect_err("problems")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("add `- Network: connects to TCP port 8080`")),
            "{problems:#?}"
        );
    }

    #[test]
    fn checks_exit_codes() {
        let ending = "`{a + b}` followed by a newline\n- And it exits with code `0`";
        assert_problem(
            &edit(
                &valid(),
                ending,
                "`{a + b}` followed by a newline\n- And it exits with code `3`",
            ),
            "generated B1 expects exit code 3",
        );
        assert_problem(
            &edit(
                &valid(),
                ending,
                "`{a + b}` followed by a newline\n- And it exits with code `{if(a > 0, 0, 134)}`",
            ),
            "reserved for failed assertions",
        );
    }

    #[test]
    fn reports_bad_expressions() {
        assert_problem(
            &edit(&valid(), "{a + b}", "{a +* b}"),
            "generated B1: what it prints: in {a +* b}",
        );
        assert_problem(
            &edit(&valid(), "{a + b}", "{a + c}"),
            "uses `c`, which isn't a column of its table",
        );
        assert_problem(
            &edit(
                &valid(),
                "| `int(-1000000000, 1000000000)` | 200 |",
                "| `float(0, 1)` | 200 |",
            ),
            "generated B1: b: unknown generator `float`",
        );
    }

    #[test]
    fn rejects_computed_inputs_and_unused_variables() {
        assert_problem(
            &edit(&valid(), "`{a}` and `{b}`", "`{a}` and `{b + 1}`"),
            "the arguments \"{b + 1}\" computes a value",
        );
        assert_problem(
            &edit(
                &edit(
                    &valid(),
                    "| a | b | runs |\n|---|---|---|",
                    "| a | b | c | runs |\n|---|---|---|---|",
                ),
                "| 200 |",
                "| `int(-1000000000, 1000000000)` | 200 |",
            ),
            "`c` never reaches the program",
        );
    }

    #[test]
    fn catches_formulas_that_fail_on_some_inputs() {
        assert_problem(
            &edit(&valid(), "{a + b}", "{a / b}"),
            "what it prints divides by zero",
        );
        assert_problem(
            &edit(&valid(), "{a + b}", "{a * b * a * b * a * b * a * b}"),
            "overflows",
        );
    }

    #[test]
    fn rejects_constant_output_despite_placeholders() {
        assert_problem(
            &edit(&valid(), "{a + b}", "{min(5, 5)}"),
            "all 200 runs expect the same output",
        );
    }

    #[test]
    fn formulas_must_reproduce_the_example() {
        assert_problem(
            &edit(&valid(), "{a + b}", "{a - b}"),
            "with the inputs of B1's steps (a = 2, b = 3) it expects what it prints \"-1\\n\", \
             but the steps expect \"5\\n\"",
        );
        assert_problem(
            &edit(
                &valid(),
                "| `int(-1000000000, 1000000000)` | 200 |",
                "| `int(-1000000000, -1000000000)` | 200 |",
            ),
            "b = 3 is outside int(-1000000000, -1000000000)",
        );
        assert_problem(
            &edit(&valid(), "`{a}` and `{b}`", "`{a}` and `{b}` and `x`"),
            "it gives 3 arguments and the steps give 2",
        );
        assert_problem(
            &edit(
                &edit(&valid(), "arguments `{a}` and `{b}`", "argument `{a}{b}`"),
                "the arguments `2` and `3`",
                "the argument `123`",
            ),
            "more than one way",
        );
    }

    /// A spec for negating whole numbers from `lo` to `hi`, with a row of its table for each of
    /// `rows`, the generators of `n`.
    fn negate(lo: i64, hi: i64, rows: &[&str]) -> String {
        let rows: String = rows.iter().map(|n| format!("| `{n}` |\n")).collect();
        format!(
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

### For any `n`
- Given the argument `{{n}}`
- When the program runs
- Then it prints `{{-n}}` followed by a newline
- And it exits with code `0`

| n |
|---|
{rows}"
        )
    }

    #[test]
    fn text_may_start_empty_without_purpose_saying_so() {
        const LINES: &str = "---
type: lengua/program-spec
title: lines
---
# Purpose
Counts the lines of at most `1048576` bytes of text.

# Behaviors
## B1: Counts lines
- Given standard input holding `a` followed by a newline
- When the program runs
- Then it prints `1` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, \"\\n\")}` followed by a newline
- And it exits with code `0`

| text |
|---|
| `string(0, 1048576, \"a\\n\")` |

# Constraints
- Uses: standard input
";
        let suite = tests_of(LINES).expect("valid tests");
        assert_eq!(suite.examples[0].stdin, "a\n");
        // Long example text is written as repeats, up to the most a test may hold.
        let long = edit(
            LINES,
            "holding `a` followed by a newline",
            "holding `a` repeated `3` times followed by a newline",
        );
        let suite = tests_of(&long).expect("valid tests");
        assert_eq!(suite.examples[0].stdin, "aaa\n");
        assert_problem(
            &edit(LINES, "holding `a`", "holding `a` repeated `9000000` times"),
            "the whole value must stay within",
        );
    }

    #[test]
    fn decimal_ranges_come_from_purpose_as_written() {
        let half = |purpose: &str, generator: &str| {
            format!(
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

### For any `x`
- Given the argument `{{x}}`
- When the program runs
- Then it prints `{{fixed(x / 2.0, 2)}}` followed by a newline
- And it exits with code `0`

| x |
|---|
| `{generator}` |
"
            )
        };
        assert!(tests_of(&half("`0.00` and `100.00`", "decimal(0.00, 100.00)")).is_ok());
        assert_problem(
            &half("`0.00` and `100.00`", "decimal(0.000, 100.000)"),
            "draws numbers with 3 decimal places, but Purpose writes them with 2",
        );
        let found =
            tests_of(&half("`0.00` and `$100.00`", "decimal(0.00, 100.00)")).expect_err("problems");
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
        let suite = tests_of(&negate(-100, 100, &["int(-100, 100)"])).expect("valid tests");
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
        let suite = tests_of(&valid()).expect("valid tests");
        assert!(!suite.generated[0].every_value);
        assert_eq!(suite.generated[0].run_count(), 200);
        let suite = tests_of(&negate(-100, 100, &["choice(-100, 7, 100)"])).expect("valid tests");
        assert!(
            !suite.generated[0].every_value,
            "choice values aren't a range"
        );
    }

    #[test]
    fn every_value_stays_within_the_suite_budget() {
        let suite = tests_of(&negate(-2500, 2499, &["int(-2500, 2499)"; 5])).expect("valid tests");
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
    fn arguments_stay_within_this_computers_limits() {
        let spec = |generator: &str| {
            format!(
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

### For any `w`
- Given the argument `{{w}}`
- When the program runs
- Then it prints `{{len(w)}}` followed by a newline
- And it exits with code `0`

| w |
|---|
| `{generator}` |
"
            )
        };
        let problems = tests_of(&spec(r#"string(1, 200000, "abc")"#))
            .expect_err("a 200000-byte argument was accepted");
        assert!(
            problems.iter().any(|problem| problem
                .contains("an argument holds at most 131072 bytes on this computer")),
            "{problems:#?}"
        );
        assert!(
            problems.iter().all(|problem| problem.len() < 2000),
            "messages shorten long values: {problems:#?}"
        );
    }

    #[test]
    fn generator_limits_come_from_purpose() {
        let row = "| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |";
        assert_problem(
            &edit(
                &valid(),
                row,
                "| `int(-1000000000, 1000000000)` | `int(-1000, 1000000000)` | 200 |",
            ),
            "uses -1000, which spec.md's Purpose doesn't state",
        );
        // 2 and 3 are B1's example inputs, not a stated range: int(2, 3) would test almost nothing.
        assert_problem(
            &edit(&valid(), row, "| `int(2, 3)` | `int(2, 3)` | 200 |"),
            "uses 2, which spec.md's Purpose doesn't state",
        );
        // String lengths too: the spec has 1 and 3 only as an exit code and an argument.
        assert_problem(
            &edit(
                &valid(),
                row,
                "| `int(-1000000000, 1000000000)` | `string(1, 3, \"123\")` | 200 |",
            ),
            "uses 1, which spec.md's Purpose doesn't state",
        );
    }

    #[test]
    fn files_go_in_before_the_run_and_are_checked_after() {
        const SHOUT: &str = "---
type: lengua/program-spec
title: shout-file
---
# Purpose
Uppercases a text file of at most `1000` bytes, `notes.txt`, into `loud.txt`.

# Behaviors
## B1: Uppercases the file
- Given a file named `notes.txt` holding `hello`
- When the program runs
- Then the file `loud.txt` holds `HELLO`
- And it exits with code `0`

### For any `s`
- Given a file `notes.txt` holding `{s}`
- When the program runs
- Then the file `loud.txt` holds `{upper(s)}`
- And it exits with code `0`

| s |
|---|
| `string(0, 1000, \"helo XYZ\")` |

# Constraints
- Uses: files
";
        let suite = tests_of(SHOUT).expect("valid tests");
        let case = suite.generated[0]
            .cases(7)
            .next()
            .expect("a run")
            .expect("computable");
        assert_eq!(
            case.files_after["loud.txt"],
            case.files["notes.txt"].to_ascii_uppercase()
        );
        for (from, to, needle) in [
            (
                "file named `notes.txt`",
                "file named `/tmp/notes.txt`",
                "is an absolute path",
            ),
            (
                "a file `notes.txt` holding `{s}`",
                "a file `../notes.txt` holding `{s}`",
                "must stay inside the working folder",
            ),
            (
                "holding `{s}`",
                "holding `{upper(s)}`",
                "may only hold bare variables",
            ),
            (
                "holds `{upper(s)}`",
                "holds `{lower(s)}`",
                "formula disagrees with the spec",
            ),
            (
                "Given a file named `notes.txt` holding `hello`",
                "Given no arguments",
                "the steps give []",
            ),
        ] {
            assert_problem(&edit(SHOUT, from, to), needle);
        }
    }

    #[test]
    fn reads_inputs_from_stdin_and_text_around_placeholders() {
        let spec = edit(
            &edit(
                &add("### For any `a` and `b`
- Given standard input holding `{a} {b}` followed by a newline
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b |
|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` |
"),
                "Given the arguments `2` and `3`",
                "Given standard input holding `2 3` followed by a newline",
            ),
            "- And it exits with code `1`\n",
            "- And it exits with code `1`\n\n# Constraints\n- Uses: standard input\n",
        );
        let suite = tests_of(&spec).expect("valid tests");
        assert_eq!(suite.examples[0].stdin, "2 3\n");
        assert_eq!(suite.generated[0].runs, DEFAULT_RUNS);
        assert!(tests_of(&edit(&spec, "{a + b}", "{a * b}")).is_err());
    }

    #[test]
    fn dry_runs_are_deterministic() {
        let spec = spec::parse(&valid()).expect("valid spec");
        assert_eq!(check(&spec), check(&spec));
        assert_eq!(dry_run_seed(GENERATED), dry_run_seed(GENERATED));
    }
}
