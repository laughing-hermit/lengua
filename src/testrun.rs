//! Runs a program's tests: every example, then every generated block with fresh inputs from a
//! new seed, boundary values first. Failures say what was expected and what happened, down to
//! the line of a failed assertion.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt as _;

use serde::Serialize;

use crate::checker::source::{Item, Operand, Source};
use crate::platform::Allowed;
use crate::runner::{self, End, Job, Limits, Net, Run};
use crate::spec::ASSERTION_EXIT_CODE;
use crate::spec::Network;
use crate::testfile::{Exchange, PORT_TEXT, Reply};
use crate::testfile::{TestSuite, describe_inputs};

/// The longest a program's tests may take together.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(90);

/// At most this many failures are reported.
pub const MAX_FAILURES: usize = 10;

/// The smallest, middle and largest of a measurement over runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Spread {
    /// The smallest.
    pub min: u64,
    /// The median.
    pub median: u64,
    /// The largest.
    pub max: u64,
}

impl Spread {
    fn of(mut values: Vec<u64>) -> Option<Self> {
        values.sort_unstable();
        Some(Self {
            min: *values.first()?,
            median: values[values.len() / 2],
            max: *values.last()?,
        })
    }
}

/// What one test, or all runs of a generated block, cost. Each measure is missing when the
/// system doesn't allow the CPU's performance counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Cost {
    /// The test, such as `example B1` or `generated B1`.
    pub test: String,
    /// Runs measured.
    pub runs: usize,
    /// Instructions executed in the program's own code: the steadiest measure of its work.
    pub instructions: Option<Spread>,
    /// CPU cycles in the program's own code: its time, with slow instructions such as division
    /// weighing more.
    pub cycles: Option<Spread>,
    /// CPU time in microseconds over all threads, including what the kernel does for its system
    /// calls.
    pub cpu_time_us: Option<Spread>,
    /// Time on the clock in microseconds, from start to end: threads shorten it while CPU time
    /// stays.
    pub wall_time_us: Option<Spread>,
}

/// Measurements collected over runs.
#[derive(Debug, Default)]
struct Samples {
    runs: usize,
    instructions: Vec<u64>,
    cycles: Vec<u64>,
    cpu_time_us: Vec<u64>,
    wall_time_us: Vec<u64>,
}

impl Samples {
    fn add(&mut self, run: &Run) {
        self.runs += 1;
        self.instructions.extend(run.instructions);
        self.cycles.extend(run.cycles);
        self.cpu_time_us
            .extend(run.cpu_time_ns.map(|nanoseconds| nanoseconds / 1000));
        self.wall_time_us.push(run.wall_time_ns / 1000);
    }

    fn cost(self, test: String) -> Cost {
        Cost {
            test,
            runs: self.runs,
            instructions: Spread::of(self.instructions),
            cycles: Spread::of(self.cycles),
            cpu_time_us: Spread::of(self.cpu_time_us),
            wall_time_us: Spread::of(self.wall_time_us),
        }
    }
}

/// What running the tests found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Runs made.
    pub runs: usize,
    /// Failures, at most [`MAX_FAILURES`].
    pub failures: Vec<String>,
    /// Instruction counts per test, when the CPU's counters are available.
    pub costs: Vec<Cost>,
}

impl Outcome {
    /// Whether every test passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// The source line of each assertion, by number.
#[must_use]
pub fn assertion_lines(source: &Source) -> BTreeMap<i64, usize> {
    let mut lines = BTreeMap::new();
    for function in &source.functions {
        let instructions: Vec<_> = function
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Instruction(instruction) => Some(instruction),
                Item::Label(_) => None,
            })
            .collect();
        for pair in instructions.windows(2) {
            let calls_assert = pair[1].mnemonic == "call"
                && pair[1].operands == [Operand::Target("assert_fail".into())];
            if let (
                true,
                [
                    Operand::Register(_),
                    Operand::Immediate { value, name: None },
                ],
            ) = (calls_assert, pair[0].operands.as_slice())
            {
                lines.insert(*value, pair[0].line);
            }
        }
    }
    lines
}

/// Shows expected text as a quoted string, with its rules written as `{any(lo, hi)}`.
fn show_expected(text: &str) -> String {
    show(crate::expect::show(text).as_bytes())
}

/// Shows bytes as a quoted string, shortened when long.
fn show(bytes: &[u8]) -> String {
    const LIMIT: usize = 200;
    let text = String::from_utf8_lossy(bytes);
    if text.chars().count() > LIMIT {
        let short: String = text.chars().take(LIMIT).collect();
        format!("{short:?}… ({} bytes)", bytes.len())
    } else {
        format!("{text:?}")
    }
}

/// How a run ended, naming the line of a failed assertion.
#[must_use]
pub fn explain_end(run: &Run, assertions: &BTreeMap<i64, usize>, limits: Limits) -> String {
    match run.end {
        End::Exited(code) if code == i32::from(ASSERTION_EXIT_CODE) => {
            let none = BTreeMap::new();
            let expected = Expected {
                stdout: "",
                stderr: "",
                exit_code: 0,
                files: &none,
                files_after: &none,
                now: 0,
                random_supplied: None,
                serves: false,
            };
            compare(run, &expected, assertions).unwrap_or_else(|| run.end.describe(limits))
        }
        other => other.describe(limits),
    }
}

/// What a run should produce.
struct Expected<'a> {
    stdout: &'a str,
    stderr: &'a str,
    exit_code: u8,
    /// The files given before the run: each must stay as it was unless `files_after` names it.
    files: &'a BTreeMap<String, String>,
    /// The files expected after the run, with their exact contents or rules.
    files_after: &'a BTreeMap<String, String>,
    /// When the run started, in Unix seconds, which a rule's `now` stands for.
    now: i128,
    /// How many random bytes the test supplies, when it supplies them.
    random_supplied: Option<usize>,
    /// Whether the program must still be serving at the end, instead of exiting.
    serves: bool,
}

/// Why a run's files don't match, if they don't.
fn compare_files(run: &Run, expected: &Expected<'_>, problems: &mut Vec<String>) {
    let found = match &run.files {
        Ok(found) => found,
        Err(problem) => {
            problems.push(format!("the program's files can't be checked: {problem}"));
            return;
        }
    };
    for (name, contents) in expected.files_after {
        match found.get(name) {
            None => problems.push(format!(
                "{name} should hold {}, but it wasn't written",
                show_expected(contents)
            )),
            Some(actual) if !crate::expect::matches(contents, actual, expected.now) => problems
                .push(format!(
                    "{name} should hold {}, but held {}",
                    show_expected(contents),
                    show(actual)
                )),
            Some(_) => {}
        }
    }
    for (name, contents) in expected.files {
        if expected.files_after.contains_key(name) {
            continue;
        }
        match found.get(name) {
            None => problems.push(format!("{name} should stay as it was, but it's gone")),
            Some(actual) if actual != contents.as_bytes() => problems.push(format!(
                "{name} should stay as it was, {}, but it now holds {}",
                show(contents.as_bytes()),
                show(actual)
            )),
            Some(_) => {}
        }
    }
    let unexpected: Vec<&str> = found
        .keys()
        .filter(|name| {
            !expected.files.contains_key(*name) && !expected.files_after.contains_key(*name)
        })
        .map(String::as_str)
        .collect();
    if !unexpected.is_empty() {
        problems.push(format!(
            "the program wrote files the test doesn't expect: {}",
            unexpected.join(", ")
        ));
    }
}

/// Why a run didn't match, or `None` when it did.
fn compare(
    run: &Run,
    expected: &Expected<'_>,
    assertions: &BTreeMap<i64, usize>,
) -> Option<String> {
    let Expected {
        stdout,
        stderr,
        exit_code,
        now,
        serves,
        ..
    } = *expected;
    // Running out of supplied random bytes stops the program; that's the failure to report.
    if let Some(problem) = &run.supplied {
        return Some(problem.clone());
    }
    let code = match run.end {
        // A server still serving at the end of its test: nothing to exit with.
        End::Stopped if serves => i32::from(exit_code),
        ending if serves => {
            return Some(format!(
                "the program was {} while the test expects it to keep serving",
                ending.describe(Limits::default())
            ));
        }
        End::Exited(code) => code,
        other => {
            return Some(format!(
                "the program was {}",
                other.describe(Limits::default())
            ));
        }
    };
    if code == i32::from(ASSERTION_EXIT_CODE) {
        let text = String::from_utf8_lossy(&run.stderr);
        let number = text
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("lengua: assertion "))
            .and_then(|rest| rest.strip_suffix(" failed"))
            .and_then(|digits| digits.parse::<i64>().ok());
        return Some(match number {
            Some(number) => match assertions.get(&number) {
                Some(line) => format!("assertion {number} failed at main.s:{line}"),
                None => format!("assertion {number} failed"),
            },
            None => "it exited with 134, the failed-assertion code".to_owned(),
        });
    }
    let mut problems = Vec::new();
    if !crate::expect::matches(stdout, &run.stdout, now) {
        problems.push(format!(
            "stdout should be {}, but was {}",
            show_expected(stdout),
            show(&run.stdout)
        ));
    }
    if !crate::expect::matches(stderr, &run.stderr, now) {
        problems.push(format!(
            "stderr should be {}, but was {}",
            show_expected(stderr),
            show(&run.stderr)
        ));
    }
    compare_files(run, expected, &mut problems);
    if code != i32::from(exit_code) {
        problems.push(format!(
            "the exit code should be {exit_code}, but was {code}"
        ));
    }
    if let (false, Some(asked), Some(supplied)) = (
        problems.is_empty(),
        run.random_asked,
        expected.random_supplied,
    ) && asked > supplied
    {
        problems.push(format!(
            "the program asked for {asked} random bytes and the test supplies {supplied}, so \
             the rest were zeros"
        ));
    }
    (!problems.is_empty()).then(|| problems.join("; "))
}

/// Runs `program` on one generated case and says what's wrong, if anything.
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn check_case(
    launcher: &Path,
    program: &Path,
    source: &Source,
    case: &crate::testfile::Case,
    syscalls: &[Allowed],
    network: &Network,
) -> io::Result<Option<String>> {
    let (_, problem) = check_one(
        launcher,
        program,
        &Test::of_case(case),
        syscalls,
        network,
        false,
        &assertion_lines(source),
    )
    .await?;
    Ok(problem)
}

/// A test's inputs and what it expects, from an example or a generated run.
struct Test<'a> {
    args: &'a [String],
    stdin: &'a str,
    files: &'a BTreeMap<String, String>,
    stdout: &'a str,
    stderr: &'a str,
    files_after: &'a BTreeMap<String, String>,
    exit_code: u8,
    random: Option<&'a str>,
    clock: &'a [String],
    server: &'a [Reply],
    client: &'a [Exchange],
    serves: bool,
}

impl<'a> Test<'a> {
    fn of_example(example: &'a crate::testfile::Example) -> Self {
        Self {
            args: &example.args,
            stdin: &example.stdin,
            files: &example.files,
            stdout: &example.stdout,
            stderr: &example.stderr,
            files_after: &example.files_after,
            exit_code: example.exit_code,
            random: example.random.as_deref(),
            clock: &example.clock,
            server: &example.server,
            client: &example.client,
            serves: example.serves,
        }
    }

    fn of_case(case: &'a crate::testfile::Case) -> Self {
        Self {
            args: &case.args,
            stdin: &case.stdin,
            files: &case.files,
            stdout: &case.stdout,
            stderr: &case.stderr,
            files_after: &case.files_after,
            exit_code: case.exit_code,
            random: case.random.as_deref(),
            clock: &case.clock,
            server: &case.server,
            client: &case.client,
            serves: case.serves,
        }
    }
}

/// How long the test's own server or client waits on the program at each step.
const PEER_TIMEOUT: Duration = Duration::from_secs(2);

/// The most the test's own server or client reads back on one connection.
const MAX_ANSWER: u64 = 8 * 1024 * 1024;

/// Runs one test, with the test's own server or client when it has one, and says what went
/// wrong, if anything. `pieces` makes the test's server and client send in small pieces, as a
/// network may deliver them.
async fn check_one(
    launcher: &Path,
    program: &Path,
    test: &Test<'_>,
    syscalls: &[Allowed],
    network: &Network,
    pieces: bool,
    assertions: &BTreeMap<i64, usize>,
) -> io::Result<(Run, Option<String>)> {
    let supply = crate::supply::Supply::parse(test.random, test.clock).unwrap_or_default();
    // The test's own server listens before the program starts, on a free port of its own.
    let server = if test.server.is_empty() {
        None
    } else {
        Some(tokio::net::TcpListener::bind("127.0.0.1:0").await?)
    };
    let port = server
        .as_ref()
        .map(tokio::net::TcpListener::local_addr)
        .transpose()?
        .map(|address| address.port());
    let fill = |text: &str| match port {
        Some(port) => text.replace(PORT_TEXT, &port.to_string()),
        None => text.to_owned(),
    };
    let args: Vec<String> = test.args.iter().map(|arg| fill(arg)).collect();
    let stdin = fill(test.stdin);
    let fill_files = |files: &BTreeMap<String, String>| -> BTreeMap<String, String> {
        files
            .iter()
            .map(|(name, contents)| (name.clone(), fill(contents)))
            .collect()
    };
    let (files, files_after) = (fill_files(test.files), fill_files(test.files_after));
    let net = Net {
        connect: port.into_iter().collect(),
        listen: network.listens.map(|_| 0),
    };
    let job = Job {
        files: &files,
        supply: Some(&supply),
        net: Some(&net),
        ..Job::new(program, &args, stdin.as_bytes(), syscalls)
    };
    let now = crate::expect::now();
    let started = runner::start(launcher, job, Limits::default()).await?;
    let mut problem = None;
    let run = if let Some(listener) = server {
        let replies: Vec<Reply> = test
            .server
            .iter()
            .map(|reply| Reply {
                receives: fill(&reply.receives),
                replies: fill(&reply.replies),
            })
            .collect();
        let peer = tokio::spawn(serve(listener, replies, pieces));
        let run = started.wait().await?;
        problem = match tokio::time::timeout(PEER_TIMEOUT, peer).await {
            Ok(Ok(result)) => result.err(),
            Ok(Err(error)) => Some(format!("the test's server failed: {error}")),
            Err(_) => Some("the program ended before it finished with the test's server".into()),
        };
        run
    } else if let Some(port) = started
        .port()
        .filter(|_| !test.client.is_empty() || test.serves)
    {
        problem = talk(port, test.client, pieces, now).await.err();
        if test.serves {
            if problem.is_none() && !waiting_for_connections(started.pid()).await {
                problem = Some(
                    "after its connections the program wasn't waiting for the next one, as a \
                     server that keeps serving does"
                        .into(),
                );
            }
            started.stop().await?
        } else {
            started.wait().await?
        }
    } else {
        started.wait().await?
    };
    let (stdout, stderr) = (fill(test.stdout), fill(test.stderr));
    let expected = Expected {
        stdout: &stdout,
        stderr: &stderr,
        exit_code: test.exit_code,
        files: &files,
        files_after: &files_after,
        now,
        random_supplied: supply.random.as_ref().map(Vec::len),
        serves: test.serves,
    };
    let problem = match (problem, compare(&run, &expected, assertions)) {
        (Some(network), Some(rest)) => Some(format!("{network}; {rest}")),
        (network, rest) => network.or(rest),
    };
    Ok((run, problem))
}

/// Sends `bytes`, in three pieces with short pauses when `pieces`, as a network may deliver
/// them: a program that expects one `read` to get everything fails.
async fn send(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    bytes: &[u8],
    pieces: bool,
) -> io::Result<()> {
    if !pieces || bytes.len() < 3 {
        return writer.write_all(bytes).await;
    }
    let middle = bytes.len() / 2;
    for piece in [&bytes[..1], &bytes[1..middle], &bytes[middle..]] {
        writer.write_all(piece).await?;
        writer.flush().await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    Ok(())
}

/// The test's own server: for each connection, reads exactly what the program must send,
/// replies, closes its side, and expects nothing more.
async fn serve(
    listener: tokio::net::TcpListener,
    replies: Vec<Reply>,
    pieces: bool,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt as _;
    let many = replies.len() > 1;
    for (index, reply) in replies.iter().enumerate() {
        let on = if many {
            format!(" on connection {}", index + 1)
        } else {
            String::new()
        };
        let (mut stream, _) = tokio::time::timeout(PEER_TIMEOUT, listener.accept())
            .await
            .map_err(|_| format!("the program didn't connect to the test's server{on}"))?
            .map_err(|error| format!("the test's server couldn't take a connection: {error}"))?;
        let expected = reply.receives.as_bytes();
        let mut got = Vec::new();
        let mut chunk = vec![0_u8; 64 * 1024];
        while got.len() < expected.len() {
            let want = (expected.len() - got.len()).min(chunk.len());
            let read = tokio::time::timeout(PEER_TIMEOUT, stream.read(&mut chunk[..want]))
                .await
                .map_err(|_| {
                    format!(
                        "the program sent {} and then waited{on}, but the test's server expects \
                         {}",
                        show(&got),
                        show(expected)
                    )
                })?
                .map_err(|error| format!("reading from the program{on} failed: {error}"))?;
            if read == 0 {
                return Err(format!(
                    "the program sent {} and closed the connection{on}, but the test's server \
                     expects {}",
                    show(&got),
                    show(expected)
                ));
            }
            got.extend_from_slice(&chunk[..read]);
            if !expected.starts_with(&got) {
                return Err(format!(
                    "the program sent {}{on}, but the test's server expects {}",
                    show(&got),
                    show(expected)
                ));
            }
        }
        let (mut reader, mut writer) = stream.into_split();
        // The program may close as soon as it has its answer, so a failed send is its doing.
        let _ = send(&mut writer, reply.replies.as_bytes(), pieces).await;
        let _ = writer.shutdown().await;
        let mut extra = Vec::new();
        let _ = tokio::time::timeout(
            PEER_TIMEOUT,
            (&mut reader).take(MAX_ANSWER).read_to_end(&mut extra),
        )
        .await;
        if !extra.is_empty() {
            return Err(format!(
                "after the test's server replied{on}, the program sent {} more",
                show(&extra)
            ));
        }
    }
    Ok(())
}

/// The test's own client: for each connection, sends, closes its side, and reads everything
/// the program answers until it closes the connection.
async fn talk(port: u16, exchanges: &[Exchange], pieces: bool, now: i128) -> Result<(), String> {
    use tokio::io::AsyncReadExt as _;
    let many = exchanges.len() > 1;
    for (index, exchange) in exchanges.iter().enumerate() {
        let on = if many {
            format!(" on connection {}", index + 1)
        } else {
            String::new()
        };
        let stream = tokio::time::timeout(
            PEER_TIMEOUT,
            tokio::net::TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        .map_err(|_| format!("the test's client couldn't connect{on}"))?
        .map_err(|error| format!("the test's client couldn't connect{on}: {error}"))?;
        let (mut reader, mut writer) = stream.into_split();
        let sends = exchange.sends.clone().into_bytes();
        let sending = tokio::spawn(async move {
            let _ = send(&mut writer, &sends, pieces).await;
            let _ = writer.shutdown().await;
        });
        let mut answer = Vec::new();
        let read = tokio::time::timeout(
            PEER_TIMEOUT,
            (&mut reader).take(MAX_ANSWER).read_to_end(&mut answer),
        )
        .await;
        sending.abort();
        if read.is_err() {
            return Err(format!(
                "the program answered {}{on} and then kept the connection open; close it after \
                 answering",
                show(&answer)
            ));
        }
        if !crate::expect::matches(&exchange.receives, &answer, now) {
            return Err(format!(
                "the program answered {}{on}, but the test expects {}",
                show(&answer),
                show_expected(&exchange.receives)
            ));
        }
    }
    Ok(())
}

/// Whether the program is waiting in `accept4` on file descriptor 3 for the next connection,
/// checked for up to half a second.
async fn waiting_for_connections(pid: i32) -> bool {
    let path = format!("/proc/{pid}/syscall");
    for _ in 0..100 {
        match tokio::fs::read_to_string(&path).await {
            Ok(call) if call.starts_with("288 0x3 ") => return true,
            Ok(_) => tokio::time::sleep(Duration::from_millis(5)).await,
            Err(_) => return false,
        }
    }
    false
}

fn record(outcome: &mut Outcome, message: String) {
    if outcome.failures.len() < MAX_FAILURES {
        outcome.failures.push(message);
    }
}

/// Runs every test of `suite` against `program`, drawing generated inputs from `seed`. The
/// kernel lets the program make only the system calls in `syscalls`.
///
/// # Errors
///
/// Fails when the program can't be started at all.
pub async fn run_tests(
    suite: &TestSuite,
    launcher: &Path,
    program: &Path,
    source: &Source,
    seed: u64,
    syscalls: &[Allowed],
    network: &Network,
) -> io::Result<Outcome> {
    let assertions = assertion_lines(source);
    let deadline = Instant::now() + TOTAL_TIMEOUT;
    let mut outcome = Outcome::default();
    for example in &suite.examples {
        if Instant::now() > deadline {
            record(
                &mut outcome,
                format!("the tests took longer than {TOTAL_TIMEOUT:?}"),
            );
            return Ok(outcome);
        }
        let (run, problem) = check_one(
            launcher,
            program,
            &Test::of_example(example),
            syscalls,
            network,
            false,
            &assertions,
        )
        .await?;
        outcome.runs += 1;
        let name = format!("example {}", example.id);
        match problem {
            Some(problem) => {
                let mut message = format!("{name} (tests.toml line {}): {problem}", example.line);
                if !example.args.is_empty() {
                    let _ = write!(message, " (args {:?})", example.args);
                }
                record(&mut outcome, message);
            }
            None => {
                let mut samples = Samples::default();
                samples.add(&run);
                outcome.costs.push(samples.cost(name));
            }
        }
    }
    for block in &suite.generated {
        let name = format!("generated {}", block.id);
        let total = block.run_count();
        let mut samples = Samples::default();
        let mut failed = false;
        for (number, case) in block.cases(seed).enumerate() {
            let case = match case {
                Ok(case) => case,
                Err((inputs, field, error)) => {
                    record(
                        &mut outcome,
                        format!(
                            "{name} (tests.toml line {}, seed {seed}): with {}, the expected \
                             {field} {error}",
                            block.line,
                            describe_inputs(&inputs)
                        ),
                    );
                    failed = true;
                    break;
                }
            };
            if Instant::now() > deadline {
                record(
                    &mut outcome,
                    format!("the tests took longer than {TOTAL_TIMEOUT:?}"),
                );
                return Ok(outcome);
            }
            // Every other run, the test's server and client send in pieces.
            let (run, problem) = check_one(
                launcher,
                program,
                &Test::of_case(&case),
                syscalls,
                network,
                number % 2 == 1,
                &assertions,
            )
            .await?;
            outcome.runs += 1;
            if let Some(problem) = problem {
                record(
                    &mut outcome,
                    format!(
                        "{name} (tests.toml line {}), run {} of {total} (seed {seed}): with {}: \
                         {problem}",
                        block.line,
                        number + 1,
                        describe_inputs(&case.inputs)
                    ),
                );
                failed = true;
                break;
            }
            samples.add(&run);
        }
        if !failed {
            outcome.costs.push(samples.cost(name));
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(stdout: &str, stderr: &str, code: i32) -> Run {
        Run {
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
            end: End::Exited(code),
            instructions: Some(100),
            cycles: Some(300),
            cpu_time_ns: Some(1000),
            wall_time_ns: 2000,
            files: Ok(BTreeMap::new()),
            supplied: None,
            random_asked: None,
        }
    }

    fn expect<'a>(
        stdout: &'a str,
        exit_code: u8,
        files: &'a BTreeMap<String, String>,
        files_after: &'a BTreeMap<String, String>,
    ) -> Expected<'a> {
        Expected {
            stdout,
            stderr: "",
            exit_code,
            files,
            files_after,
            now: 1_735_689_600,
            random_supplied: None,
            serves: false,
        }
    }

    #[test]
    fn rules_take_any_number_in_range_and_show_as_written() {
        let none = BTreeMap::new();
        let no_files = BTreeMap::new();
        let rolled = format!(
            "rolled {}\n",
            crate::expect::hole(
                crate::expect::Bound::Number(1),
                crate::expect::Bound::Number(6)
            )
        );
        let rule = expect(&rolled, 0, &no_files, &no_files);
        assert_eq!(compare(&finished("rolled 6\n", "", 0), &rule, &none), None);
        let wrong = compare(&finished("rolled 7\n", "", 0), &rule, &none).expect("differs");
        assert!(
            wrong.contains("stdout should be \"rolled {any(1, 6)}\\n\", but was \"rolled 7\\n\""),
            "{wrong}"
        );
        // now is when the run started.
        let clock =
            crate::expect::hole(crate::expect::Bound::Now(-2), crate::expect::Bound::Now(2));
        let rule = expect(&clock, 0, &no_files, &no_files);
        assert_eq!(compare(&finished("1735689601", "", 0), &rule, &none), None);
        assert!(compare(&finished("1735689603", "", 0), &rule, &none).is_some());
    }

    #[test]
    fn compares_every_stream_and_the_exit_code() {
        let none = BTreeMap::new();
        let no_files = BTreeMap::new();
        let five = expect("5\n", 0, &no_files, &no_files);
        assert_eq!(compare(&finished("5\n", "", 0), &five, &none), None);
        let wrong = compare(&finished("6\n", "oops\n", 1), &five, &none).expect("differs");
        assert!(
            wrong.contains("stdout should be \"5\\n\", but was \"6\\n\""),
            "{wrong}"
        );
        assert!(
            wrong.contains("stderr should be \"\", but was \"oops\\n\""),
            "{wrong}"
        );
        assert!(
            wrong.contains("the exit code should be 0, but was 1"),
            "{wrong}"
        );
    }

    #[test]
    fn names_the_line_of_a_failed_assertion() {
        let lines = BTreeMap::from([(7, 42)]);
        let no_files = BTreeMap::new();
        let nothing = expect("", 0, &no_files, &no_files);
        let run = finished("", "lengua: assertion 007 failed\n", 134);
        assert_eq!(
            compare(&run, &nothing, &lines),
            Some("assertion 7 failed at main.s:42".to_owned())
        );
        let killed = Run {
            end: End::Signaled(11),
            ..finished("", "", 0)
        };
        assert!(
            compare(&killed, &nothing, &lines).is_some_and(|problem| problem.contains("SIGSEGV"))
        );
    }

    #[test]
    fn compares_the_files_left_behind() {
        let none = BTreeMap::new();
        let file = |name: &str, contents: &str| (name.to_owned(), contents.to_owned());
        let given = BTreeMap::from([file("notes.txt", "hello\n")]);
        let wanted = BTreeMap::from([file("out.txt", "HELLO\n")]);
        let expected = expect("", 0, &given, &wanted);
        let left = |files: &[(&str, &str)]| Run {
            files: Ok(files
                .iter()
                .map(|(name, contents)| ((*name).to_owned(), contents.as_bytes().to_vec()))
                .collect()),
            ..finished("", "", 0)
        };
        let good = left(&[("notes.txt", "hello\n"), ("out.txt", "HELLO\n")]);
        assert_eq!(compare(&good, &expected, &none), None);
        let problem =
            |files: &[(&str, &str)]| compare(&left(files), &expected, &none).expect("differs");
        assert!(
            problem(&[("notes.txt", "hello\n")])
                .contains("out.txt should hold \"HELLO\\n\", but it wasn't written")
        );
        assert!(
            problem(&[("notes.txt", "hello\n"), ("out.txt", "hello\n")])
                .contains("out.txt should hold \"HELLO\\n\", but held \"hello\\n\"")
        );
        assert!(
            problem(&[("out.txt", "HELLO\n")])
                .contains("notes.txt should stay as it was, but it's gone")
        );
        assert!(
            problem(&[("notes.txt", ""), ("out.txt", "HELLO\n")])
                .contains("notes.txt should stay as it was")
        );
        assert!(
            problem(&[
                ("notes.txt", "hello\n"),
                ("out.txt", "HELLO\n"),
                ("tmp", "")
            ])
            .contains("the program wrote files the test doesn't expect: tmp")
        );
        let unreadable = Run {
            files: Err("it left more than 256 files".into()),
            ..finished("", "", 0)
        };
        assert!(
            compare(&unreadable, &expected, &none)
                .is_some_and(|problem| problem.contains("more than 256 files"))
        );
    }

    #[test]
    fn finds_assertion_lines_in_the_source() {
        let (source, _) = crate::checker::source::parse(crate::checker::TEMPLATE);
        let lines = assertion_lines(&source);
        assert_eq!(lines.len(), 11);
        let line = lines[&4];
        let text = crate::checker::TEMPLATE
            .lines()
            .nth(line - 1)
            .expect("the line");
        assert_eq!(text.trim(), "mov edi, 4");
    }

    #[test]
    fn summarizes_measurements() {
        assert_eq!(
            Spread::of(vec![30, 10, 20]),
            Some(Spread {
                min: 10,
                median: 20,
                max: 30
            })
        );
        assert_eq!(Spread::of(Vec::new()), None);
        let mut samples = Samples::default();
        samples.add(&finished("", "", 0));
        samples.add(&Run {
            instructions: None,
            ..finished("", "", 0)
        });
        let cost = samples.cost("generated B1".into());
        assert_eq!(cost.runs, 2);
        assert_eq!(cost.instructions.map(|spread| spread.median), Some(100));
        assert_eq!(cost.cpu_time_us.map(|spread| spread.max), Some(1));
    }
}
