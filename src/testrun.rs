//! Runs a program's tests, main.wat's module in lengua's pool of workers: every example, then
//! every generated block with fresh inputs from a new seed, boundary values first, as many at
//! once as the machine has cores. Failures say what was expected and what happened, down to the
//! line of a failed assertion. Once everything passes, each test's largest input runs again
//! alone, for times the other runs can't disturb.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt as _;

use serde::Serialize;

use crate::connect::Reach;
use crate::idea::ASSERTION_EXIT_CODE;
use crate::idea::{Network, Use};
use crate::runner::{End, Limits, Run};
use crate::testfile::{Case, Example, Exchange, Reply};
use crate::testfile::{TestSuite, describe_inputs};

/// The longest a program's tests may take together.
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(90);

/// At most this many failures are reported.
pub const MAX_FAILURES: usize = 10;

/// How many times each test's largest input runs alone for its times.
pub const TIMED_RUNS: usize = 5;

/// How many runs go at once: one for each core, so every core is busy, and at least one.
#[must_use]
pub fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

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

/// What one test, or all runs of a generated block, cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Cost {
    /// The test, such as `example B1` or `generated B1`.
    pub test: String,
    /// Runs made, with other tests running at the same time.
    pub runs: usize,
    /// The WebAssembly operations the program executed over those runs, as the engine counts
    /// them: the steadiest measure of its work, which other runs don't change.
    pub instructions: Option<Spread>,
    /// What the times below measured, such as "its largest input, 1048576 bytes, run alone 5
    /// times".
    pub timed: String,
    /// Time on the clock in microseconds, from start to end.
    pub wall_time_us: Option<Spread>,
}

/// Measurements collected over runs.
#[derive(Debug, Default)]
struct Samples {
    runs: usize,
    instructions: Vec<u64>,
    wall_time_us: Vec<u64>,
}

impl Samples {
    fn add(&mut self, run: &Run) {
        self.runs += 1;
        self.instructions.extend(run.instructions);
        self.wall_time_us.push(run.wall_time_ns / 1000);
    }

    /// The cost of a test: operations from every run, times from the runs made alone.
    fn cost(self, test: String, timed: Self, what: &str) -> Cost {
        Cost {
            test,
            runs: self.runs,
            instructions: Spread::of(self.instructions),
            timed: format!("{what}, run alone {} times", timed.runs),
            wall_time_us: Spread::of(timed.wall_time_us),
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
    /// What each test cost, operations and times.
    pub costs: Vec<Cost>,
    /// The first example as it ran, in one line, for a report to show.
    pub shown: Option<String>,
}

impl Outcome {
    /// Whether every test passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
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

/// How a run ended, naming the line of a failed assertion in main.wat.
#[must_use]
pub fn explain_end(run: &Run, assertions: &BTreeMap<i64, usize>, limits: Limits) -> String {
    match run.end {
        End::Exited(status) if status == i32::from(ASSERTION_EXIT_CODE) => {
            let none = BTreeMap::new();
            let no_folders = BTreeSet::new();
            let expected = Expected {
                stdout: "",
                stderr: "",
                exit_code: 0,
                files: &none,
                files_after: &none,
                folders: &no_folders,
                gone: &no_folders,
                folders_after: &no_folders,
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
    /// The folders, empty, given before the run: each stays unless `gone` names it.
    folders: &'a BTreeSet<String>,
    /// Files and folders that must be gone after the run.
    gone: &'a BTreeSet<String>,
    /// Folders that must be there after the run.
    folders_after: &'a BTreeSet<String>,
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
        if expected.files_after.contains_key(name) || expected.gone.contains(name) {
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
            !(expected.files.contains_key(*name) && !expected.gone.contains(*name))
                && !expected.files_after.contains_key(*name)
        })
        .map(String::as_str)
        .collect();
    if !unexpected.is_empty() {
        problems.push(format!(
            "the program wrote files the test doesn't expect: {}",
            unexpected.join(", ")
        ));
    }
    for name in expected.gone {
        if found.contains_key(name) || run.folders.contains(name) {
            problems.push(format!("{name} should be gone, but it's still there"));
        }
    }
    for name in expected.folders_after {
        if !run.folders.contains(name) {
            problems.push(format!("the folder {name} should be there, but it isn't"));
        }
    }
    for name in expected.folders {
        if !expected.gone.contains(name) && !run.folders.contains(name) {
            problems.push(format!("the folder {name} should stay, but it's gone"));
        }
    }
    // A folder the test doesn't name is one the program made, unless it holds a file the test
    // names.
    let named = |folder: &String| {
        let inside = format!("{folder}/");
        expected.folders.contains(folder)
            || expected.folders_after.contains(folder)
            || expected
                .files
                .keys()
                .chain(expected.files_after.keys())
                .chain(expected.folders)
                .chain(expected.folders_after)
                .any(|name| name.starts_with(&inside))
    };
    let made: Vec<&str> = run
        .folders
        .iter()
        .filter(|folder| !named(folder))
        .map(String::as_str)
        .collect();
    if !made.is_empty() {
        problems.push(format!(
            "the program made folders the test doesn't expect: {}",
            made.join(", ")
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
    let code_file = crate::wasm::CODE_FILE;
    // A host that couldn't run the program as asked is the failure to report; running out of the
    // test's random bytes stops nothing (the rest are zeros), and a failing test says so below.
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
                Some(line) => format!("assertion {number} failed at {code_file}:{line}"),
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

/// Runs the WebAssembly program `module` on one generated case, in `pool`'s workers, and says
/// what's wrong, if anything: its idea's `uses` and `network` say what its host gives it.
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn check_wasm_case(
    pool: &crate::wasm::Pool,
    module: &crate::wasm::Program,
    case: &crate::testfile::Case,
    uses: &[Use],
    network: &Network,
    delivery: Delivery,
) -> io::Result<Option<String>> {
    let target = Target {
        pool,
        module,
        policy: crate::wasm::Policy::of(uses, network),
    };
    let (_, problem) = check_one(
        target,
        &Test::of_case(case),
        network,
        delivery,
        &module.built().assertions,
    )
    .await?;
    Ok(problem)
}

/// What a test runs: a WebAssembly module in lengua's pool of workers.
#[derive(Clone, Copy)]
struct Target<'a> {
    /// The workers it runs in.
    pool: &'a crate::wasm::Pool,
    /// The module.
    module: &'a crate::wasm::Program,
    /// What its idea lets it reach.
    policy: crate::wasm::Policy,
}

/// Starts the test's own servers on the program's connections, each from the start: `peers` are
/// lengua's ends of them, by the connection's number in the idea.
fn spawn_servers<S>(
    peers: Vec<(usize, S)>,
    test: &Test<'_>,
    network: &Network,
    delivery: Delivery,
) -> Vec<tokio::task::JoinHandle<Result<(), String>>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    peers
        .into_iter()
        .map(|(number, stream)| {
            let reply = test
                .server
                .iter()
                .find(|reply| reply.connection == number)
                .cloned();
            let on = match network.connections.get(number) {
                Some(place) if network.connections.len() > 1 => {
                    format!(" on its connection to {}", place.describe())
                }
                _ => String::new(),
            };
            tokio::spawn(serve(stream, reply, delivery, on))
        })
        .collect()
}

/// How a test's server or client sends what it sends, as a network may deliver it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// All at once.
    Whole,
    /// In three pieces: the first byte, up to the middle, and the rest.
    Pieces,
    /// The first two of those pieces together, then the third.
    FirstTwo,
    /// The first piece, then the other two together.
    LastTwo,
    /// In two pieces, cut after this many bytes: where a proof found a read can stop.
    At(usize),
    /// In pieces cut after each byte whose bit is set, the lowest bit for the first byte: where
    /// a proof found reads of standard input can stop.
    Cuts(u16),
}

impl Delivery {
    /// Every delivery a proof covers.
    pub const ALL: [Self; 4] = [Self::Whole, Self::Pieces, Self::FirstTwo, Self::LastTwo];

    /// The lengths of the pieces `length` bytes arrive in. Fewer than three bytes arrive whole,
    /// except where `At` or `Cuts` cut them.
    #[must_use]
    pub fn cuts(self, length: usize) -> Vec<usize> {
        if let Self::Cuts(mask) = self {
            let mut pieces = Vec::new();
            let mut start = 0;
            for after in 1..length.min(17) {
                if mask >> (after - 1) & 1 == 1 {
                    pieces.push(after - start);
                    start = after;
                }
            }
            pieces.push(length - start);
            return pieces.into_iter().filter(|piece| *piece > 0).collect();
        }
        if let Self::At(first) = self {
            return if first == 0 || first >= length {
                vec![length]
            } else {
                vec![first, length - first]
            };
        }
        if length < 3 {
            return vec![length];
        }
        let middle = length / 2;
        let cuts = match self {
            Self::Whole => vec![length],
            Self::Pieces => vec![1, middle - 1, length - middle],
            Self::FirstTwo => vec![middle, length - middle],
            Self::LastTwo | Self::At(_) | Self::Cuts(_) => vec![1, length - 1],
        };
        // A piece of nothing sends nothing: three bytes arrive as one and two.
        cuts.into_iter().filter(|cut| *cut > 0).collect()
    }

    /// The delivery in words, for a report.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::Whole => "all at once".into(),
            Self::Pieces => {
                "in three pieces: the first byte, up to the middle, and the rest".into()
            }
            Self::FirstTwo => "in two pieces, cut at the middle".into(),
            Self::LastTwo => "in two pieces: the first byte, and the rest".into(),
            Self::At(first) => format!("in two pieces, cut after byte {first}"),
            Self::Cuts(mask) => {
                let after: Vec<String> = (1..=16)
                    .filter(|after| mask >> (after - 1) & 1 == 1)
                    .map(|after: u32| after.to_string())
                    .collect();
                match after.split_last() {
                    Some((last, [])) => format!("in pieces, cut after byte {last}"),
                    Some((last, rest)) => {
                        format!("in pieces, cut after bytes {} and {last}", rest.join(", "))
                    }
                    None => "all at once".into(),
                }
            }
        }
    }

    /// The delivery `describe` gave these words for.
    #[must_use]
    pub fn described(words: &str) -> Option<Self> {
        if let Some(first) = words.strip_prefix("in two pieces, cut after byte ") {
            return first.parse().ok().map(Self::At);
        }
        let after = words
            .strip_prefix("in pieces, cut after bytes ")
            .or_else(|| words.strip_prefix("in pieces, cut after byte "));
        if let Some(after) = after {
            let mut mask = 0_u16;
            for place in after
                .split([',', ' '])
                .filter(|word| !word.is_empty() && *word != "and")
            {
                let place: u32 = place.parse().ok()?;
                mask |= 1 << place.checked_sub(1).filter(|bit| *bit < 16)?;
            }
            return Some(Self::Cuts(mask));
        }
        Self::ALL
            .into_iter()
            .find(|delivery| delivery.describe() == words)
    }
}

/// A test's inputs and what it expects, from an example or a generated run.
struct Test<'a> {
    args: &'a [String],
    stdin: &'a str,
    files: &'a BTreeMap<String, String>,
    folders: &'a BTreeSet<String>,
    env: &'a BTreeMap<String, String>,
    gone: &'a BTreeSet<String>,
    folders_after: &'a BTreeSet<String>,
    stdout: &'a str,
    stderr: &'a str,
    files_after: &'a BTreeMap<String, String>,
    exit_code: u8,
    random: Option<&'a str>,
    clock: &'a [String],
    server: &'a [Reply],
    client: &'a [Exchange],
    clients_at_once: bool,
    stdout_closed: bool,
    serves: bool,
}

impl<'a> Test<'a> {
    fn of_example(example: &'a crate::testfile::Example) -> Self {
        Self {
            args: &example.args,
            stdin: &example.stdin,
            files: &example.files,
            folders: &example.folders,
            env: &example.env,
            gone: &example.gone,
            folders_after: &example.folders_after,
            stdout: &example.stdout,
            stderr: &example.stderr,
            files_after: &example.files_after,
            exit_code: example.exit_code,
            random: example.random.as_deref(),
            clock: &example.clock,
            server: &example.server,
            client: &example.client,
            clients_at_once: example.clients_at_once,
            stdout_closed: example.stdout_closed,
            serves: example.serves,
        }
    }

    fn of_case(case: &'a crate::testfile::Case) -> Self {
        Self {
            args: &case.args,
            stdin: &case.stdin,
            files: &case.files,
            folders: &case.folders,
            env: &case.env,
            gone: &case.gone,
            folders_after: &case.folders_after,
            stdout: &case.stdout,
            stderr: &case.stderr,
            files_after: &case.files_after,
            exit_code: case.exit_code,
            random: case.random.as_deref(),
            clock: &case.clock,
            server: &case.server,
            client: &case.client,
            clients_at_once: case.clients_at_once,
            stdout_closed: case.stdout_closed,
            serves: case.serves,
        }
    }
}

/// How long the test's own server or client waits on the program at each step.
const PEER_TIMEOUT: Duration = Duration::from_secs(2);

/// The most the test's own server or client reads back on one connection.
const MAX_ANSWER: u64 = 8 * 1024 * 1024;

/// Runs one test of `target`, with the test's own server or client when it has one, and says
/// what went wrong, if anything. `delivery` cuts what the test's server and client send into
/// pieces, as a network may deliver them.
///
/// A run that listed a folder runs again with each listing turned round: no system promises an
/// order, each file system has its own, so a result that changes with the order is wrong on
/// some system, and the test fails.
async fn check_one(
    target: Target<'_>,
    test: &Test<'_>,
    network: &Network,
    delivery: Delivery,
    assertions: &BTreeMap<i64, usize>,
) -> io::Result<(Run, Option<String>)> {
    let (run, mut problem) =
        check_one_way(target, test, network, delivery, assertions, false).await?;
    if problem.is_none() && run.listed {
        let (_, turned) = check_one_way(target, test, network, delivery, assertions, true).await?;
        if let Some(turned) = turned {
            problem = Some(format!(
                "with its folder's entries listed the other way round, {turned}: each file \
                 system lists a folder in an order of its own, so a result must not depend on \
                 it; sort the entries first"
            ));
        }
    }
    Ok((run, problem))
}

/// [`check_one`], once: with each folder listing turned round when `reversed`.
async fn check_one_way(
    target: Target<'_>,
    test: &Test<'_>,
    network: &Network,
    delivery: Delivery,
    assertions: &BTreeMap<i64, usize>,
    reversed: bool,
) -> io::Result<(Run, Option<String>)> {
    let supply = crate::supply::Supply::parse(test.random, test.clock).unwrap_or_default();
    let args = test.args.to_vec();
    let stdin = test.stdin;
    // A test's program reaches only the test's own servers: each connection its idea names is
    // answered by the test's server for it, or handed over ended when the test has none.
    let connections: Vec<Reach> = (0..network.connections.len())
        .map(|number| {
            if test.server.iter().any(|reply| reply.connection == number) {
                Reach::Peer
            } else {
                Reach::Nowhere
            }
        })
        .collect();
    // What the program opens by itself reaches the test's own servers too, and nothing else:
    // each computer the idea's tests stand in for that this test has a server for. Its servers
    // are numbered after the idea's own connections.
    let made = network.connections.len();
    let stood: Vec<usize> = (0..network.stands.len())
        .filter(|index| {
            test.server
                .iter()
                .any(|reply| reply.connection == made + index)
        })
        .collect();
    let stands: Vec<crate::reach::Stand> = stood
        .iter()
        .filter_map(|index| network.stands.get(*index).cloned())
        .collect();
    // A server listens on a port of its own, on the loopback, whoever the idea serves.
    let listen = network.listens.map(|_| (0, false));
    // Standard input arrives as the run's delivery cuts it: whole, in the pieces every other
    // generated run sends, or where a proof found a cut that matters. So a program that reads it
    // as if it came whole fails, even where its proof doesn't finish.
    let stdin_pieces = match delivery {
        Delivery::Whole => Vec::new(),
        _ => delivery.cuts(stdin.len()),
    };
    let pieced = stdin_pieces.len() > 1
        && matches!(
            delivery,
            Delivery::Pieces | Delivery::FirstTwo | Delivery::LastTwo
        );
    let now = crate::expect::now();
    let spec = crate::wasm::Spec {
        stdin_pieces: &stdin_pieces,
        files: test.files,
        folders: test.folders,
        env: test.env,
        supply: Some(&supply),
        connections: &connections,
        listen,
        stands: Some(&stands),
        names: &network.names,
        reversed,
        stdout_closed: test.stdout_closed,
        ..crate::wasm::Spec::new(&args, stdin.as_bytes(), target.policy)
    };
    let mut started = target
        .pool
        .start(target.module, spec, Limits::default())
        .await?;
    let mut problem = None;
    // The test's servers speak on the program's own connections, each from the start.
    let mut servers = spawn_servers(started.take_peers(), test, network, delivery);
    // And for each computer the program reaches by itself, on every connection it opens there
    // and every datagram it sends there, in the order of the test's steps.
    for (number, peer) in started.take_stands() {
        let Some((index, stand)) = stood
            .get(number)
            .and_then(|index| Some((*index, network.stands.get(*index)?)))
        else {
            continue;
        };
        let replies: Vec<Reply> = test
            .server
            .iter()
            .filter(|reply| reply.connection == made + index)
            .cloned()
            .collect();
        let on = format!(" at {}:{}", stand.host, stand.port);
        servers.push(tokio::spawn(stand_in(peer, replies, delivery, on)));
    }
    let run = if let Some(port) = started
        .port()
        .filter(|_| !test.client.is_empty() || test.serves)
    {
        problem = if network.reach.receives.is_some() {
            talk_datagrams(port, test.client, now).await.err()
        } else if test.clients_at_once {
            talk_at_once(port, test.client, now).await.err()
        } else {
            talk(port, test.client, delivery, now).await.err()
        };
        if test.serves {
            let waiting = waiting_for_connections(&started).await;
            if problem.is_none() && !waiting {
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
    for server in servers {
        let found = match tokio::time::timeout(PEER_TIMEOUT, server).await {
            Ok(Ok(result)) => result.err(),
            Ok(Err(error)) => Some(format!("the test's server failed: {error}")),
            Err(_) => Some("the program ended before it finished with the test's server".into()),
        };
        problem = problem.or(found);
    }
    let expected = Expected {
        stdout: test.stdout,
        stderr: test.stderr,
        exit_code: test.exit_code,
        files: test.files,
        files_after: test.files_after,
        folders: test.folders,
        gone: test.gone,
        folders_after: test.folders_after,
        now,
        random_supplied: supply.random.as_ref().map(Vec::len),
        serves: test.serves,
    };
    let problem = match (problem, compare(&run, &expected, assertions)) {
        (Some(network), Some(rest)) => Some(format!("{network}; {rest}")),
        (network, rest) => network.or(rest),
    };
    // A run whose standard input came in pieces says so: a program that reads it as if it came
    // whole fails only then.
    let problem = problem.map(|problem| {
        if pieced {
            format!("standard input arriving {}: {problem}", delivery.describe())
        } else {
            problem
        }
    });
    Ok((run, problem))
}

/// Sends `bytes` in the pieces `delivery` cuts, with short pauses between, as a network may
/// deliver them: a program that expects one `read` to get everything fails.
async fn send(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    bytes: &[u8],
    delivery: Delivery,
) -> io::Result<()> {
    let cuts = delivery.cuts(bytes.len());
    if cuts.len() == 1 {
        return writer.write_all(bytes).await;
    }
    let mut start = 0;
    for length in cuts {
        writer.write_all(&bytes[start..start + length]).await?;
        writer.flush().await?;
        start += length;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    Ok(())
}

/// The test's own server on one of the program's connections, which lengua made before the
/// program started: reads exactly what the program must send, replies, closes its side, and
/// expects nothing more. `on` names the connection when the program has more than one.
async fn serve<S>(
    stream: S,
    reply: Option<Reply>,
    delivery: Delivery,
    on: String,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    use tokio::io::AsyncReadExt as _;
    let Some(reply) = reply else {
        return Ok(());
    };
    let (mut reader, mut writer) = tokio::io::split(stream);
    let expected = reply.receives.as_bytes();
    let mut got = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    while got.len() < expected.len() {
        let want = (expected.len() - got.len()).min(chunk.len());
        let read = tokio::time::timeout(PEER_TIMEOUT, reader.read(&mut chunk[..want]))
            .await
            .map_err(|_| {
                format!(
                    "the program sent {} and then waited{on}, but the test's server expects {}",
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
    // The program may close as soon as it has its answer, so a failed send is its doing.
    let _ = send(&mut writer, reply.replies.as_bytes(), delivery).await;
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
    Ok(())
}

/// The test's own server for a computer the program reaches by itself, which stands in for the
/// real one: on each connection the program opens there, in order, it does as [`serve`] does
/// with the next of `replies`; on each datagram, it reads exactly what the next of them
/// receives and answers with one datagram. After the last, nothing listens there. `on` names
/// the computer.
async fn stand_in(
    peer: crate::wasm::Peer,
    replies: Vec<Reply>,
    delivery: Delivery,
    on: String,
) -> Result<(), String> {
    match peer {
        crate::wasm::Peer::Connections(listener) => {
            // Each connection has its own server, so a program may hold one while it opens the
            // next.
            let mut serving = Vec::new();
            for reply in replies {
                let accepted = tokio::time::timeout(PEER_TIMEOUT, listener.accept()).await;
                let Ok(Ok((stream, _))) = accepted else {
                    return Err(format!(
                        "the program opened no connection{on} where the test's server waited \
                         for one"
                    ));
                };
                serving.push(tokio::spawn(serve(
                    stream,
                    Some(reply),
                    delivery,
                    on.clone(),
                )));
            }
            drop(listener);
            for server in serving {
                server
                    .await
                    .map_err(|error| format!("the test's server failed: {error}"))??;
            }
            Ok(())
        }
        crate::wasm::Peer::Datagrams(socket) => {
            let mut chunk = vec![0_u8; 65_536];
            for reply in replies {
                let received =
                    tokio::time::timeout(PEER_TIMEOUT, socket.recv_from(&mut chunk)).await;
                let Ok(Ok((count, from))) = received else {
                    return Err(format!(
                        "the program sent no datagram{on} where the test's computer waited for \
                         one"
                    ));
                };
                let expected = reply.receives.as_bytes();
                if &chunk[..count] != expected {
                    return Err(format!(
                        "the program sent the datagram {}{on}, but the test's computer expects {}",
                        show(&chunk[..count]),
                        show(expected)
                    ));
                }
                if !reply.replies.is_empty() {
                    let _ = socket.send_to(reply.replies.as_bytes(), from).await;
                }
            }
            Ok(())
        }
    }
}

/// The test's own client of a program that receives datagrams: for each of its steps it sends
/// one datagram to the program's port and, where the test expects an answer, reads one datagram
/// back, which must be that answer.
async fn talk_datagrams(port: u16, exchanges: &[Exchange], now: i128) -> Result<(), String> {
    let here = std::net::Ipv4Addr::LOCALHOST;
    let socket = tokio::net::UdpSocket::bind((here, 0))
        .await
        .map_err(|error| format!("the test's client couldn't open its socket: {error}"))?;
    let many = exchanges.len() > 1;
    let mut chunk = vec![0_u8; 65_536];
    for (index, exchange) in exchanges.iter().enumerate() {
        let on = if many {
            format!(" to datagram {}", index + 1)
        } else {
            String::new()
        };
        socket
            .send_to(exchange.sends.as_bytes(), (here, port))
            .await
            .map_err(|error| format!("the test's client couldn't send its datagram: {error}"))?;
        if exchange.receives.is_empty() {
            continue;
        }
        let received = tokio::time::timeout(PEER_TIMEOUT, socket.recv_from(&mut chunk)).await;
        let Ok(Ok((count, _))) = received else {
            return Err(format!(
                "the program answered nothing{on} while the test's client waited for a datagram"
            ));
        };
        if !crate::expect::matches(&exchange.receives, &chunk[..count], now) {
            return Err(format!(
                "the program answered the datagram {}{on}, but the test expects {}",
                show(&chunk[..count]),
                show_expected(&exchange.receives)
            ));
        }
    }
    Ok(())
}

/// The test's own client, as curl and browsers behave: for each connection, it sends and keeps
/// its side open while it waits for the answer. Once the program has started answering (or at
/// once, when the test expects no answer) it closes its side, then reads everything the program
/// answers until it closes the connection.
async fn talk(
    port: u16,
    exchanges: &[Exchange],
    delivery: Delivery,
    now: i128,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
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
            let _ = send(&mut writer, &sends, delivery).await;
            writer
        });
        let mut answer = Vec::new();
        let mut closed = false;
        if !exchange.receives.is_empty() {
            let mut chunk = vec![0_u8; 65536];
            let first = tokio::time::timeout(PEER_TIMEOUT, reader.read(&mut chunk)).await;
            match first {
                Ok(Ok(0)) => closed = true,
                Ok(Ok(read)) => answer.extend_from_slice(&chunk[..read]),
                Ok(Err(_)) => closed = true,
                Err(_) => {
                    sending.abort();
                    return Err(format!(
                        "the program answered nothing{on} while the test's client waited with \
                         its side of the connection open. curl and browsers keep it open until \
                         they have the whole answer, so answer once the request is complete \
                         (its newline or blank line), without waiting for the client to close \
                         its side"
                    ));
                }
            }
        }
        // The answer has begun: the client has nothing more to send, and says so.
        if let Ok(mut writer) = sending.await {
            let _ = writer.shutdown().await;
        }
        if !closed {
            let left = MAX_ANSWER.saturating_sub(answer.len() as u64);
            let read = tokio::time::timeout(
                PEER_TIMEOUT,
                (&mut reader).take(left).read_to_end(&mut answer),
            )
            .await;
            if read.is_err() {
                return Err(format!(
                    "the program answered {}{on} and then kept the connection open; close it \
                     after answering",
                    show(&answer)
                ));
            }
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

/// The test's own clients at once: every one connects, and all but the newest send all but
/// their last byte and hold it; the newest sends everything and must be answered while the
/// others still hold, then each in turn, from the newest, sends its last byte and must be
/// answered. A server that serves one client at a time is caught waiting on the first.
async fn talk_at_once(port: u16, exchanges: &[Exchange], now: i128) -> Result<(), String> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let last = exchanges.len().saturating_sub(1);
    let mut open = Vec::new();
    for (index, exchange) in exchanges.iter().enumerate() {
        let on = format!(" on connection {}", index + 1);
        let stream = tokio::time::timeout(
            PEER_TIMEOUT,
            tokio::net::TcpStream::connect(("127.0.0.1", port)),
        )
        .await
        .map_err(|_| format!("the test's client couldn't connect{on}"))?
        .map_err(|error| format!("the test's client couldn't connect{on}: {error}"))?;
        let (reader, mut writer) = stream.into_split();
        let sends = exchange.sends.as_bytes();
        let held = if index < last {
            sends.len().saturating_sub(1)
        } else {
            sends.len()
        };
        writer
            .write_all(&sends[..held])
            .await
            .map_err(|error| format!("the test's client couldn't send{on}: {error}"))?;
        open.push((reader, writer, held));
    }
    for (index, (exchange, (mut reader, mut writer, held))) in
        exchanges.iter().zip(open).enumerate().rev()
    {
        let on = format!(" on connection {}", index + 1);
        let sends = exchange.sends.as_bytes();
        writer
            .write_all(&sends[held..])
            .await
            .map_err(|error| format!("the test's client couldn't send{on}: {error}"))?;
        // As in any test, the client keeps its side open until the answer begins.
        let mut answer = Vec::new();
        let mut chunk = vec![0_u8; 65536];
        match tokio::time::timeout(PEER_TIMEOUT, reader.read(&mut chunk)).await {
            Ok(Ok(read)) => answer.extend_from_slice(&chunk[..read]),
            Ok(Err(_)) => {}
            Err(_) if index == last && last > 0 => {
                return Err(format!(
                    "the program answered nothing{on}, whose request was whole, while {} still \
                     sending: its idea serves clients at once, so it must answer each as its \
                     request completes, whatever the others are doing, with poll over the \
                     listening socket and every connection",
                    if last == 1 {
                        "connection 1 was".to_owned()
                    } else {
                        format!("connections 1 to {last} were")
                    }
                ));
            }
            Err(_) => {
                return Err(format!(
                    "the program answered nothing{on} once its request was whole"
                ));
            }
        }
        let _ = writer.shutdown().await;
        let left = MAX_ANSWER.saturating_sub(answer.len() as u64);
        let read = tokio::time::timeout(
            PEER_TIMEOUT,
            (&mut reader).take(left).read_to_end(&mut answer),
        )
        .await;
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

/// Whether the program is waiting for the next connection, checked for up to half a second: its
/// host says so while it waits in `sock_accept` or `poll_oneoff`.
async fn waiting_for_connections(running: &crate::wasm::Running) -> bool {
    for _ in 0..100 {
        if running.waiting() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    false
}

/// How long a shown text may be.
const SHOWN_CHARS: usize = 500;

/// `text` as a quoted string, cut to [`SHOWN_CHARS`].
fn quoted(text: &str) -> String {
    let mut shown: String = text.chars().take(SHOWN_CHARS).collect();
    if shown.len() < text.len() {
        shown.push('…');
    }
    format!("{shown:?}")
}

/// An example's run in one line: what it was given, and what it did.
fn shown(example: &Example, run: &Run) -> String {
    let mut given = Vec::new();
    if !example.args.is_empty() {
        given.push(format!("arguments {:?}", example.args));
    }
    if !example.stdin.is_empty() {
        given.push(format!("standard input {}", quoted(&example.stdin)));
    }
    for (name, contents) in &example.files {
        given.push(format!("a file {name} holding {}", quoted(contents)));
    }
    for reply in &example.server {
        given.push(format!(
            "a server that received {} and replied {}",
            quoted(&reply.receives),
            quoted(&reply.replies)
        ));
    }
    for exchange in &example.client {
        given.push(format!(
            "a client that sent {} and received {}",
            quoted(&exchange.sends),
            quoted(&exchange.receives)
        ));
    }
    let mut did = Vec::new();
    if !run.stdout.is_empty() {
        did.push(format!(
            "printed {}",
            quoted(&String::from_utf8_lossy(&run.stdout))
        ));
    }
    if !run.stderr.is_empty() {
        did.push(format!(
            "printed {} to standard error",
            quoted(&String::from_utf8_lossy(&run.stderr))
        ));
    }
    for (name, contents) in &example.files_after {
        if !example
            .files
            .get(name)
            .is_some_and(|before| before == contents)
        {
            did.push(format!("left {name} holding {}", quoted(contents)));
        }
    }
    did.push(match run.end {
        End::Exited(code) => format!("exited with code {code}"),
        _ if example.serves => "kept serving".to_owned(),
        _ => "ended".to_owned(),
    });
    let given = if given.is_empty() {
        "nothing".to_owned()
    } else {
        given.join(", ")
    };
    format!(
        "example {}: given {given}, the program {}",
        example.id,
        did.join(", ")
    )
}

fn record(outcome: &mut Outcome, message: String) {
    if outcome.failures.len() < MAX_FAILURES {
        outcome.failures.push(message);
    }
}

/// What every run of a check shares: the module, in lengua's pool of workers, what its idea lets
/// it reach, and its assertions' lines in main.wat.
struct Shared {
    /// The workers.
    pool: crate::wasm::Pool,
    /// The module.
    module: Arc<crate::wasm::Program>,
    /// What its idea lets it reach.
    policy: crate::wasm::Policy,
    network: Network,
    assertions: BTreeMap<i64, usize>,
}

/// A test a run owns, so runs can go at once.
enum Owned {
    Example(Example),
    Case(Case),
}

impl Owned {
    fn test(&self) -> Test<'_> {
        match self {
            Self::Example(example) => Test::of_example(example),
            Self::Case(case) => Test::of_case(case),
        }
    }
}

async fn run_one(
    shared: &Shared,
    test: &Owned,
    delivery: Delivery,
) -> io::Result<(Run, Option<String>)> {
    let target = Target {
        pool: &shared.pool,
        module: &shared.module,
        policy: shared.policy,
    };
    check_one(
        target,
        &test.test(),
        &shared.network,
        delivery,
        &shared.assertions,
    )
    .await
}

/// The bytes a case gives the program: arguments, standard input, files and what the test's
/// own server or client sends.
fn input_bytes(case: &Case) -> usize {
    case.args.iter().map(String::len).sum::<usize>()
        + case.stdin.len()
        + case.files.values().map(String::len).sum::<usize>()
        + case
            .server
            .iter()
            .map(|reply| reply.replies.len())
            .sum::<usize>()
        + case
            .client
            .iter()
            .map(|exchange| exchange.sends.len())
            .sum::<usize>()
}

/// What a generated run came to: inputs whose results can't be computed, or a run.
enum Drawn {
    Unfit((crate::expr::Env, &'static str, crate::expr::EvalError)),
    Ran(Box<(Owned, Run, Option<String>)>),
}

/// A test whose times are taken once everything has passed.
struct Timing {
    name: String,
    test: Owned,
    what: String,
    samples: Samples,
}

fn timed_out() -> String {
    format!("the tests took longer than {TOTAL_TIMEOUT:?}")
}

/// Runs every test of `suite` against the WebAssembly program `module`, in `pool`'s workers,
/// drawing generated inputs from `seed`, up to `workers` runs at once: its idea's `uses` and
/// `network` say what its host gives it. The failures reported are the ones running the tests one
/// at a time would give: every failing example, and each block's lowest-numbered failing run.
///
/// # Errors
///
/// Fails when the program can't be started at all.
pub async fn run_wasm_tests(
    suite: &TestSuite,
    pool: &crate::wasm::Pool,
    module: Arc<crate::wasm::Program>,
    seed: u64,
    uses: &[Use],
    network: &Network,
    workers: usize,
) -> io::Result<Outcome> {
    let shared = Arc::new(Shared {
        assertions: module.built().assertions.clone(),
        pool: pool.clone(),
        module,
        policy: crate::wasm::Policy::of(uses, network),
        network: network.clone(),
    });
    run_all(suite, shared, seed, workers).await
}

/// [`run_wasm_tests`]' work.
async fn run_all(
    suite: &TestSuite,
    shared: Arc<Shared>,
    seed: u64,
    workers: usize,
) -> io::Result<Outcome> {
    let workers = workers.max(1);
    let deadline = Instant::now() + TOTAL_TIMEOUT;
    let mut outcome = Outcome::default();
    let mut timings = Vec::new();

    // Every example, at once.
    let mut running = tokio::task::JoinSet::new();
    let mut results = BTreeMap::new();
    let mut next = 0;
    while next < suite.examples.len() || !running.is_empty() {
        while running.len() < workers && next < suite.examples.len() {
            if Instant::now() > deadline {
                record(&mut outcome, timed_out());
                return Ok(outcome);
            }
            let example = &suite.examples[next];
            let test = Owned::Example(example.clone());
            let shared = Arc::clone(&shared);
            let index = next;
            running.spawn(async move {
                let result = run_one(&shared, &test, Delivery::Whole).await;
                (index, test, result)
            });
            next += 1;
        }
        if let Some(joined) = running.join_next().await {
            let (index, test, result) = joined.map_err(io::Error::other)?;
            results.insert(index, (test, result?));
        }
    }
    for (index, example) in suite.examples.iter().enumerate() {
        let Some((test, (run, problem))) = results.remove(&index) else {
            continue;
        };
        outcome.runs += 1;
        let name = format!("example {}", example.id);
        if index == 0 && problem.is_none() {
            outcome.shown = Some(shown(example, &run));
        }
        match problem {
            Some(problem) => {
                let mut message = format!("{name} (idea.md line {}): {problem}", example.line);
                if !example.args.is_empty() {
                    let _ = write!(message, " (args {:?})", example.args);
                }
                record(&mut outcome, message);
            }
            None => {
                let mut samples = Samples::default();
                samples.add(&run);
                timings.push(Timing {
                    name,
                    test,
                    what: "its input".to_owned(),
                    samples,
                });
            }
        }
    }

    // Each generated block's runs, at once: each run draws its own inputs from the seed and
    // computes what it should produce as it starts, so `now` is fresh and that work is spread
    // over the cores too. Once run k fails, no later run starts, and the earlier ones finish, so
    // the failure reported is the block's lowest-numbered one.
    for block in &suite.generated {
        let name = format!("generated {}", block.id);
        let total = block.run_count();
        let shared_block = Arc::new(block.clone());
        let mut samples = Samples::default();
        let mut failure: Option<(usize, String)> = None;
        let mut largest: Option<(usize, Case)> = None;
        let mut running = tokio::task::JoinSet::new();
        let mut number = 0;
        loop {
            while running.len() < workers && number < total && failure.is_none() {
                if Instant::now() > deadline {
                    record(&mut outcome, timed_out());
                    return Ok(outcome);
                }
                let shared = Arc::clone(&shared);
                let block = Arc::clone(&shared_block);
                let this = number;
                running.spawn(async move {
                    let drawn = tokio::task::spawn_blocking(move || {
                        let inputs = block.inputs(seed).at(this);
                        block.case(inputs, crate::expect::now())
                    })
                    .await
                    .map_err(io::Error::other)?;
                    let case = match drawn {
                        Ok(case) => case,
                        Err(unfit) => return Ok((this, Drawn::Unfit(unfit))),
                    };
                    let test = Owned::Case(case);
                    // Every other run, standard input and the test's server and client send in
                    // pieces.
                    let delivery = if this % 2 == 1 {
                        Delivery::Pieces
                    } else {
                        Delivery::Whole
                    };
                    let (run, problem) = run_one(&shared, &test, delivery).await?;
                    Ok::<_, io::Error>((this, Drawn::Ran(Box::new((test, run, problem)))))
                });
                number += 1;
            }
            let Some(joined) = running.join_next().await else {
                break;
            };
            let (this, drawn) = joined.map_err(io::Error::other)??;
            let message = match drawn {
                Drawn::Unfit((inputs, field, error)) => Some(format!(
                    "{name} (idea.md line {}, seed {seed}): with {}, the expected {field} \
                     {error}",
                    block.line,
                    describe_inputs(&inputs)
                )),
                Drawn::Ran(ran) => {
                    let (Owned::Case(case), run, problem) = *ran else {
                        continue;
                    };
                    outcome.runs += 1;
                    match problem {
                        Some(problem) => Some(format!(
                            "{name} (idea.md line {}), run {} of {total} (seed {seed}): with \
                             {}: {problem}",
                            block.line,
                            this + 1,
                            describe_inputs(&case.inputs)
                        )),
                        None => {
                            samples.add(&run);
                            let size = input_bytes(&case);
                            if largest.as_ref().is_none_or(|(most, _)| size > *most) {
                                largest = Some((size, case));
                            }
                            None
                        }
                    }
                }
            };
            if let Some(message) = message
                && failure.as_ref().is_none_or(|(first, _)| this < *first)
            {
                failure = Some((this, message));
            }
        }
        match (failure, largest) {
            (Some((_, message)), _) => record(&mut outcome, message),
            (None, Some((size, case))) => timings.push(Timing {
                name,
                test: Owned::Case(case),
                what: format!("its largest input, {size} bytes"),
                samples,
            }),
            (None, None) => {}
        }
    }
    if !outcome.passed() {
        return Ok(outcome);
    }

    // Times from runs made alone, which nothing else disturbs.
    for timing in timings {
        let mut timed = Samples::default();
        for _ in 0..TIMED_RUNS {
            let (run, problem) = run_one(&shared, &timing.test, Delivery::Whole).await?;
            if let Some(problem) = problem {
                record(
                    &mut outcome,
                    format!(
                        "{} passed with the other tests running, but failed run again alone on \
                         {}: {problem}",
                        timing.name, timing.what
                    ),
                );
                outcome.costs.clear();
                return Ok(outcome);
            }
            timed.add(&run);
        }
        outcome
            .costs
            .push(timing.samples.cost(timing.name, timed, &timing.what));
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
            wall_time_ns: 2000,
            files: Ok(BTreeMap::new()),
            folders: BTreeSet::new(),
            supplied: None,
            random_asked: None,
            connections: Vec::new(),
            listed: false,
        }
    }

    fn expect<'a>(
        stdout: &'a str,
        exit_code: u8,
        files: &'a BTreeMap<String, String>,
        files_after: &'a BTreeMap<String, String>,
    ) -> Expected<'a> {
        static NO_FOLDERS: BTreeSet<String> = BTreeSet::new();
        Expected {
            stdout,
            stderr: "",
            exit_code,
            files,
            files_after,
            folders: &NO_FOLDERS,
            gone: &NO_FOLDERS,
            folders_after: &NO_FOLDERS,
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
            Some("assertion 7 failed at main.wat:42".to_owned())
        );
        // A trap names its line of main.wat too.
        let trapped = Run {
            end: End::Trapped(crate::runner::Trap::OutOfBounds, Some(12)),
            ..finished("", "", 0)
        };
        assert_eq!(
            compare(&trapped, &nothing, &lines),
            Some(
                "the program was trapped at main.wat line 12: it reached memory outside its own"
                    .to_owned()
            )
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
        // Folders: a file moved into one the program made, the file it came from gone.
        let folders = |names: &[&str]| -> BTreeSet<String> {
            names.iter().map(|name| (*name).to_owned()).collect()
        };
        let (given_folders, gone, made) = (
            folders(&["old"]),
            folders(&["notes.txt"]),
            folders(&["box"]),
        );
        let moved = BTreeMap::from([file("box/notes.txt", "hello\n")]);
        let expected = Expected {
            folders: &given_folders,
            gone: &gone,
            folders_after: &made,
            ..expect("", 0, &given, &moved)
        };
        let left_in = |files: &[(&str, &str)], names: &[&str]| Run {
            folders: folders(names),
            ..left(files)
        };
        let ok = left_in(&[("box/notes.txt", "hello\n")], &["box", "old"]);
        assert_eq!(compare(&ok, &expected, &none), None);
        let problem = |files: &[(&str, &str)], names: &[&str]| {
            compare(&left_in(files, names), &expected, &none).expect("differs")
        };
        assert!(
            problem(
                &[("box/notes.txt", "hello\n"), ("notes.txt", "hello\n")],
                &["box", "old"]
            )
            .contains("notes.txt should be gone, but it's still there")
        );
        assert!(
            problem(&[("box/notes.txt", "hello\n")], &["box"])
                .contains("the folder old should stay, but it's gone")
        );
        assert!(
            problem(&[("box/notes.txt", "hello\n")], &["box", "old", "tmp"])
                .contains("the program made folders the test doesn't expect: tmp")
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
        let mut timed = Samples::default();
        timed.add(&Run {
            wall_time_ns: 250_000,
            ..finished("", "", 0)
        });
        let cost = samples.cost("generated B1".into(), timed, "its largest input, 3 bytes");
        assert_eq!(cost.runs, 2);
        assert_eq!(cost.instructions.map(|spread| spread.median), Some(100));
        // Times come from the runs made alone.
        assert_eq!(cost.wall_time_us.map(|spread| spread.max), Some(250));
        assert_eq!(cost.timed, "its largest input, 3 bytes, run alone 1 times");
    }
}
