//! The lengua MCP server: tool definitions and server instructions. A program's code is main.wat,
//! the code of an idea that is an Attested Computation, checked and run as WebAssembly the same
//! way on every system ([`wat`]).

mod wat;

pub use self::wat::{Attestation, attest};

// How wide the CPU's vectors go, which `lengua-mcp doctor` says as the machine line does.
pub(crate) use self::wat::simd_level;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::checker::wasm::Obligation;
use crate::data_dir::DataDir;
use crate::expr;
use crate::graph;
use crate::hash::Sha256Hex;
use crate::idea::{self, Idea};
use crate::lock::{self, CodeStamp, IdeaStamp, Key, Lock, ProofStamp};
use crate::platform::Machine;
use crate::prove;
use crate::runner::{self, End, Limits};
use crate::state::{
    CODE_FILE, Files, IDEA_FILE, LEGACY_IDEA_FILE, LEGACY_TESTS_FILE, ProgramFolder, Snapshot,
    State, stamped_copy, tested_module,
};
use crate::testdiff;
use crate::testfile::{self, TestSuite};
use crate::testrun;
use crate::trust;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua builds the programs the user asks for, from commands and \
filters to servers and clients, unless they name another language: you write idea.md, an \
Attested Computation with behaviors and tests, with the write-idea skill, then compile it into \
main.wat with the compile skill. After each edit, call check with the folder's absolute path, \
the first time with the user's words as request, and follow its next_step until it says Done. \
Never edit lengua.lock. Run programs only with run.";

/// The longest request the lock records: the user's words, not a whole conversation.
const MAX_REQUEST_CHARS: usize = 2000;

/// The longest a `check` call waits for the tests and the proof before it replies. They go on
/// in the background, and the next call picks them up, so no call outlasts an agent harness's
/// patience.
const CHECK_WAIT: Duration = Duration::from_secs(40);

/// Once the tests pass, how long `check` still waits for the proofs before it says done: quick
/// proofs come back in the same reply, and the rest go on in the background, listed with how far
/// each has got, so a long proof never holds up the news.
const GREEN_GRACE: Duration = Duration::from_secs(20);

/// The longest the proof of one version of a program may take, over every call.
const PROOF_TIME: Duration = Duration::from_secs(300);

/// The longest `run` lets a program run, within one call.
const RUN_SECONDS: u64 = 40;

/// How long a `check` call waits: [`CHECK_WAIT`], or `LENGUA_CHECK_WAIT_MS` for tests.
fn check_wait() -> Duration {
    std::env::var("LENGUA_CHECK_WAIT_MS")
        .ok()
        .and_then(|milliseconds| milliseconds.parse().ok())
        .map_or(CHECK_WAIT, Duration::from_millis)
}

/// How long `check` waits for the proofs once the tests pass: [`GREEN_GRACE`], or
/// `LENGUA_GREEN_GRACE_MS` for tests.
fn green_grace() -> Duration {
    std::env::var("LENGUA_GREEN_GRACE_MS")
        .ok()
        .and_then(|milliseconds| milliseconds.parse().ok())
        .map_or(GREEN_GRACE, Duration::from_millis)
}

/// What a proof that stopped short of the idea's lengths says it can do.
const FURTHER: &str = "; check with prove_further to prove on from there";

/// How the line of a test whose proof neither proved it nor found it wrong begins, before why,
/// which lengua.lock keeps.
const UNPROVEN: &str = "tested, not proven: ";

/// What `check`'s done reply for main.wat says of lengua's entry in idea.md's `verified`, which
/// check writes as it replies done; a reply leaves it out when the entry didn't go in.
const VERIFIED_NOTE: &str = " idea.md's verified list now holds lengua's entry for this idea and \
                             this main.wat, until a proof finds main.wat wrong.";

/// How long the proof of one version may take: [`PROOF_TIME`], or `LENGUA_PROOF_SECONDS` for
/// tests that aren't about proofs.
fn proof_time() -> Duration {
    std::env::var("LENGUA_PROOF_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .map_or(PROOF_TIME, Duration::from_secs)
}

/// How long each write of the proofs to lengua.lock waits first: nothing, or
/// `LENGUA_STAMP_DELAY_MS` for tests, which make the write slow to look at what a caller sees
/// until it is done.
fn stamp_delay() -> Duration {
    std::env::var("LENGUA_STAMP_DELAY_MS")
        .ok()
        .and_then(|milliseconds| milliseconds.parse().ok())
        .map_or(Duration::ZERO, Duration::from_millis)
}

/// The lengua MCP server.
#[derive(Debug, Clone)]
pub struct LenguaServer {
    tool_router: ToolRouter<Self>,
    data_dir: Arc<DataDir>,
    /// Serializes every read-modify-write of a lock file.
    lock_writes: Arc<tokio::sync::Mutex<()>>,
    /// This server's own binary, which the workers that run main.wat start from, and which the
    /// shell command a done reply gives names.
    launcher: Arc<PathBuf>,
    /// Servers that `run` started, by program folder, until `run` stops them, ten minutes
    /// pass, or the session ends and drops them.
    background: Arc<tokio::sync::Mutex<BTreeMap<PathBuf, Background>>>,
    /// The tests and proof of each program folder's latest files, running or ended.
    checks: Arc<tokio::sync::Mutex<BTreeMap<PathBuf, Arc<Checking>>>>,
    /// The user's words given with an idea that still had problems, by program folder: recorded
    /// once the idea is clean, so they're asked for once.
    held: Arc<tokio::sync::Mutex<BTreeMap<PathBuf, String>>>,
    /// The workers that run main.wat's tests, made when a program first needs them.
    workers: Arc<Workers>,
}

/// A server's pool of WebAssembly workers, made at its first use: its launcher is this binary,
/// and it keeps compiled modules in the data folder's cache.
#[derive(Default)]
struct Workers(std::sync::OnceLock<crate::wasm::Pool>);

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers")
            .field("made", &self.0.get().is_some())
            .finish()
    }
}

/// A server `run` started in the background.
struct Background {
    /// The program, still serving.
    server: Serving,
    /// Which start this is, so an old timer never stops a newer one.
    id: u64,
    /// Its assertions' lines in main.wat, by number.
    assertions: BTreeMap<i64, usize>,
}

/// A program serving in the background, in a worker of its own, and its module.
struct Serving {
    /// The worker's run of it.
    running: Box<crate::wasm::Running>,
    /// Its module, kept until it stops.
    program: Arc<crate::wasm::Program>,
}

impl Serving {
    /// The loopback port it serves on.
    fn port(&self) -> Option<u16> {
        self.running.port()
    }

    /// Stops it, and says how it went.
    async fn stop(self) -> std::io::Result<runner::Run> {
        let Self { running, program } = self;
        let stopped = running.stop().await;
        drop(program);
        stopped
    }
}

impl std::fmt::Debug for Background {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Background")
            .field("id", &self.id)
            .field("port", &self.server.port())
            .finish_non_exhaustive()
    }
}

/// How long a server `run` started keeps serving at most.
const BACKGROUND_LIMIT: Duration = Duration::from_secs(600);

/// The tests and then the proof of one version of a program, in the background.
struct Checking {
    /// idea.md and main.wat, by lengua's hashes.
    files: [Sha256Hex; 2],
    /// The module built from main.wat, by hash.
    module: Sha256Hex,
    /// Set once the files change, which stops the proof.
    cancel: Arc<AtomicBool>,
    /// What it has found so far.
    found: std::sync::Mutex<Found>,
    /// How far it has got, for whoever waits on it.
    stage: watch::Sender<Stage>,
    /// Whether a reply has said the tests pass yet.
    green_told: AtomicBool,
    /// Whether to prove on from where lengua.lock says this build's proofs stopped short.
    further: bool,
    /// What main.wat's rules leave to its proofs, which its memory line names.
    obligations: Vec<Obligation>,
}

impl std::fmt::Debug for Checking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checking")
            .field("stage", &*self.stage.borrow())
            .finish_non_exhaustive()
    }
}

/// How far a checking job has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The tests are running.
    Testing,
    /// Every test passed at this time, and the proof is running.
    Proving(Instant),
    /// Nothing more will happen.
    Ended,
}

/// What a checking job has found.
#[derive(Debug, Default)]
struct Found {
    /// Why the tests failed, one line each, when they did.
    failures: Vec<String>,
    /// What the tests showed, once every one passed.
    passed: Option<Passed>,
    /// Each generated test's proof, in order.
    proofs: Vec<BlockProof>,
    /// Whether the proof found inputs where main.wat is wrong.
    wrong: bool,
}

/// What passing tests showed.
#[derive(Debug)]
struct Passed {
    /// Runs made.
    runs: usize,
    /// The generated tests that tried every value in their ranges.
    every_value: Vec<String>,
    /// What each test cost, one line each.
    performance: Vec<String>,
    /// The first example as it ran.
    shown: Option<String>,
}

/// One generated test's proof.
#[derive(Debug)]
struct BlockProof {
    /// The test, such as `generated B1`.
    test: String,
    /// How far it has got, while it runs.
    progress: Option<Arc<prove::Progress>>,
    /// What was proven, or why not, once it ended.
    detail: Option<String>,
    /// Whether it found inputs where the code is wrong.
    wrong: bool,
}

impl BlockProof {
    /// One line for a reply: what was proven, or how far the proof has got.
    fn line(&self) -> String {
        let test = &self.test;
        if let Some(detail) = &self.detail {
            return format!("{test}: {detail}");
        }
        let Some(progress) = &self.progress else {
            return format!("{test}: waiting for its turn");
        };
        let done = progress.done.load(Ordering::Relaxed);
        let total = progress.total.load(Ordering::Relaxed);
        match progress.proven_below.load(Ordering::Relaxed) {
            0 => format!("{test}: proving, {done} of {total} input combinations so far"),
            below => format!(
                "{test}: proving, every input with text up to {} bytes proven so far",
                below - 1
            ),
        }
    }
}

impl LenguaServer {
    /// Creates a server that keeps its private data in `data_dir`.
    #[must_use]
    pub fn new(data_dir: DataDir) -> Self {
        Self {
            tool_router: Self::tool_router(),
            data_dir: Arc::new(data_dir),
            lock_writes: Arc::default(),
            launcher: Arc::new(std::env::current_exe().unwrap_or_default()),
            background: Arc::default(),
            checks: Arc::default(),
            held: Arc::default(),
            workers: Arc::default(),
        }
    }

    /// The server's pool of WebAssembly workers, one for the session, made at its first use.
    fn pool(&self) -> crate::wasm::Pool {
        self.workers
            .0
            .get_or_init(|| {
                crate::wasm::Pool::new(
                    PathBuf::clone(&self.launcher),
                    Some(crate::wasm::engine::cache_folder(&self.data_dir)),
                )
            })
            .clone()
    }
}

/// Input for `check`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckParams {
    /// Absolute path of the program folder, the folder that holds idea.md.
    pub path: String,
    /// The user's own words that asked for this program or this change, quoted exactly, such as
    /// "Make me a program that adds two numbers". Needed the first time, and whenever the user
    /// asks for something new; without it, an edited idea is recorded under the words already
    /// recorded.
    #[serde(default)]
    pub request: Option<String>,
    /// Proves on from where the last proof of this very build stopped short of the lengths the
    /// idea gives its texts, for each proof whose line says so: another stretch of proof time,
    /// in the background.
    #[serde(default)]
    pub prove_further: bool,
}

/// What `check` reports: where the program stands, and the one thing to do next.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
pub struct CheckReport {
    /// Where the program stands: write_idea, idea_problems, needs_request, write_code,
    /// code_problems, testing, tests_failed, proving or done.
    pub state: String,
    /// Every problem to fix before going on, each saying where it is and how to fix it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The machine the code runs on, while it isn't written yet: the engine that compiles
    /// main.wat for this CPU, and the WebAssembly features main.wat may use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// What the tests found, once they pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<String>,
    /// Each generated test's proof: what's proven for every input, how far, or why not; while
    /// proving, how far it has got.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proofs: Vec<String>,
    /// What's known of memory safety, once the tests pass: what the rules settle, and the runs
    /// the proofs hold to the rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// The idea's first example as the tests ran it: what to show the user it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// What each test cost, once done: the WebAssembly operations a run executes over all its
    /// runs, and from its largest input run alone, the time on the clock (lower is faster).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performance: Vec<String>,
    /// What changed after the user's request, once done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<ChangesReport>,
    /// The command that runs the program from the user's own terminal, scripts and services,
    /// once done, followed by its arguments: `run`'s checks and confinement, with no time limit,
    /// the shell's own input and output, and files read and written in the folder it starts in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    /// How the graph of program folders that this one belongs to stands: its bundle, what
    /// index.md and log.md now say of the program, and what needs care, each line starting with
    /// how much: `blocks` (fix it first), `warning` or `note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph: Vec<String>,
    /// The single next thing to do.
    pub next_step: String,
}

impl CheckReport {
    fn new(state: &str, next_step: String) -> Self {
        Self {
            state: state.to_owned(),
            next_step,
            ..Self::default()
        }
    }

    fn problems(state: &str, problems: Vec<String>, next_step: String) -> Self {
        Self {
            problems,
            ..Self::new(state, next_step)
        }
    }
}

/// What changed after the user's request.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
pub struct ChangesReport {
    /// The user's requests in this build, as recorded, oldest first.
    pub requests: Vec<String>,
    /// How idea.md, tests included, changed since the build's first request, as `-`/`+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea_changed: Option<String>,
    /// lengua's own list of the test changes that test less than before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests_less: Vec<String>,
}

/// `count` things, in the plural unless there's one.
fn counted(count: usize, thing: &str) -> String {
    if count == 1 {
        format!("1 {thing}")
    } else {
        format!("{count} {thing}s")
    }
}

/// Input for `run`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunParams {
    /// Absolute path of the program folder.
    pub path: String,
    /// The program's command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Text for the program's standard input, which ends after it.
    #[serde(default)]
    pub stdin: String,
    /// Absolute path of a folder to run the program in, so it can read and write the user's
    /// files there, named relative to it. Without it, the program runs in an empty temporary
    /// folder.
    #[serde(default)]
    pub folder: Option<String>,
    /// Stops the server that `run` started for this program and reports what it printed.
    #[serde(default)]
    pub stop: bool,
    /// The name of a file in `folder` to give the program as its standard input instead of
    /// `stdin`: the program gets it whole, at most 16 MiB.
    #[serde(default)]
    pub stdin_file: Option<String>,
    /// How many seconds the program may run, 1 to 40; 10 when not given.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Environment variables for a program whose idea uses the environment, by name: only
    /// these reach it.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// What `run` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RunReport {
    /// Standard output (invalid UTF-8 replaced).
    pub stdout: String,
    /// Standard error (invalid UTF-8 replaced).
    pub stderr: String,
    /// The exit code, unless the program was killed.
    pub exit_code: Option<i32>,
    /// How the program ended.
    pub ended: String,
    /// Time on the clock in microseconds, from start to end.
    pub wall_time_us: u64,
    /// Where a server is taking connections, while it runs in the background.
    pub serving: Option<String>,
    /// The connections lengua made for the program before it started, when its idea names any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<ConnectionReport>,
}

/// One connection lengua made for a program, or couldn't.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ConnectionReport {
    /// Where it goes, in words.
    pub place: String,
    /// Why lengua couldn't make it, when it couldn't: the program got it already ended, so its
    /// reads end at once and its writes fail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

impl ConnectionReport {
    fn of(made: &crate::connect::Made) -> Self {
        Self {
            place: made.place.clone(),
            problem: made.problem.clone(),
        }
    }
}

/// What `run` reports about a program that has ended: its failed assertion on its line of
/// main.wat.
fn ended_report(
    result: &runner::Run,
    assertions: &BTreeMap<i64, usize>,
    limits: Limits,
) -> RunReport {
    let mut ended = testrun::explain_end(result, assertions, limits);
    // A host that couldn't run the program as asked, such as a module it couldn't start: the
    // report says why.
    if let Some(problem) = &result.supplied
        && !ended.contains(problem.as_str())
    {
        ended = format!("{ended}; {problem}");
    }
    RunReport {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        exit_code: match result.end {
            End::Exited(code) => Some(code),
            _ => None,
        },
        ended,
        wall_time_us: result.wall_time_ns / 1000,
        serving: None,
        connections: result
            .connections
            .iter()
            .map(ConnectionReport::of)
            .collect(),
    }
}

impl LenguaServer {
    /// Keeps `server`, which `run` started for `folder` and which serves now, among the
    /// background ones until `run` stops it, ten minutes pass, or the session ends; `run`'s
    /// reply says where it serves and what became of its connections. `assertions` are its
    /// assertions' lines in main.wat.
    fn keep_serving(
        &self,
        background: &mut BTreeMap<PathBuf, Background>,
        folder: &ProgramFolder,
        server: Serving,
        assertions: BTreeMap<i64, usize>,
        connections: Vec<ConnectionReport>,
    ) -> RunReport {
        static STARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let address = format!("127.0.0.1:{}", server.port().unwrap_or(0));
        let id = STARTS.fetch_add(1, Ordering::Relaxed);
        background.insert(
            folder.path().to_path_buf(),
            Background {
                server,
                id,
                assertions,
            },
        );
        let (servers, key) = (Arc::clone(&self.background), folder.path().to_path_buf());
        tokio::spawn(async move {
            tokio::time::sleep(BACKGROUND_LIMIT).await;
            let mut servers = servers.lock().await;
            if servers.get(&key).is_some_and(|entry| entry.id == id)
                && let Some(entry) = servers.remove(&key)
            {
                drop(servers);
                let _ = entry.server.stop().await;
            }
        });
        RunReport {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            ended: format!(
                "serving on {address}. Try it with your own tools, such as curl or nc, then call \
                 run with stop: true to stop it and see what it printed. It stops by itself \
                 after ten minutes or when this session ends."
            ),
            wall_time_us: 0,
            serving: Some(address),
            connections,
        }
    }
}

/// The limits of a server `run` leaves serving: ten minutes, and no file's size held.
fn background_limits() -> Limits {
    Limits {
        timeout: BACKGROUND_LIMIT,
        max_file_size: None,
        ..Limits::default()
    }
}

/// Why `run` can't start a server for `folder`, when one is serving for it already.
fn already_serving(
    background: &BTreeMap<PathBuf, Background>,
    folder: &ProgramFolder,
) -> Result<(), String> {
    match background.get(folder.path()) {
        Some(entry) => Err(format!(
            "{folder} is already serving on 127.0.0.1:{}; call run with stop: true first",
            entry.server.port().unwrap_or(0)
        )),
        None => Ok(()),
    }
}

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "check",
        title = "Check",
        description = "Checks the lengua program in <path> stage by stage, records each stage in \
            lengua.lock, and stops at the first that needs work: it checks idea.md, the source \
            of truth, and the tests in it, and records it with the user's words that asked for \
            it; checks the code, main.wat, against lengua's rules; runs every test with fresh \
            random inputs; and proves the program for \
            every input it can. It keeps the index.md and log.md of \
            the folder's bundle, the graph of program folders, and says how the graph stands. \
            Call it after every edit to idea.md \
            or the code. It replies within about 40 seconds; the tests and the proof go on in the \
            background, so while it says testing or proving, call it again to wait. path: \
            absolute path of the program folder; request: the user's words that \
            asked for this program or this change, quoted exactly and never made up, needed the \
            first time and whenever they ask for something new.",
        annotations(
            title = "Check",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn check(
        &self,
        Parameters(params): Parameters<CheckParams>,
    ) -> Result<Json<CheckReport>, String> {
        let folder = folder_of(&params.path)?;
        let mut events = Vec::new();
        let mut report = self.check_stages(&folder, &params, &mut events).await?;
        report.graph = self.graph_lines(&folder, events).await;
        Ok(Json(report))
    }

    #[tool(
        name = "run",
        title = "Run program",
        description = "Runs the checked program in <path> with args and stdin. It runs only \
            when lengua.lock shows that idea.md and its code, main.wat, are exactly the versions \
            that were recorded and checked, and a fresh build of the code is byte for byte the \
            module check tested. The program gets an empty environment, \
            10 seconds and \
            8 MiB of output, and lengua lets it use only what its idea names, reach only the \
            places it names, through connections lengua makes for it before it starts, and touch \
            files only inside its working folder: \
            an empty temporary folder, or `folder`. A server starts in the background on its \
            idea's port and run returns its address; call run with stop: true to stop it and get \
            what it printed (it also stops after ten minutes or when the session ends). path: \
            absolute path of the program folder; args: the program's arguments; stdin: text for \
            its standard input; folder: absolute path of a folder of the user's files to run in, \
            naming the files in args relative to it (\"notes.txt\", not \"/home/me/notes.txt\"); \
            stdin_file: a file in folder to read as standard input instead of stdin, up to 16 \
            MiB; timeout_seconds: up to 40 (10 by default); env: environment variables, for an \
            idea that uses the environment; stop: true to stop the server run started.",
        annotations(title = "Run program", read_only_hint = false, open_world_hint = false)
    )]
    async fn run(
        &self,
        Parameters(params): Parameters<RunParams>,
    ) -> Result<Json<RunReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        if params.stop {
            let entry = self.background.lock().await.remove(folder.path());
            let Some(entry) = entry else {
                return Err(format!(
                    "no server that run started is running for {folder}"
                ));
            };
            let result = entry
                .server
                .stop()
                .await
                .map_err(|error| format!("couldn't stop the program: {error}"))?;
            let limits = Limits {
                timeout: BACKGROUND_LIMIT,
                ..Limits::default()
            };
            return Ok(Json(ended_report(&result, &entry.assertions, limits)));
        }
        self.run_wasm(&folder, &params).await.map(Json)
    }
}

/// What `check`'s idea stage came to.
enum IdeaStage {
    /// The idea parses, its tests check, its parts are there, and the lock records it.
    Recorded(Box<(Idea, TestSuite)>),
    /// The reply that stops there: the idea's problems, or the words it needs first.
    Stops(Box<CheckReport>),
}

/// What recording idea.md came to.
enum Recording {
    /// The lock already recorded the idea as it is.
    Unchanged,
    /// The lock records it now.
    Recorded,
    /// It can't be recorded without the user's words: this next step says so.
    Needs(String),
}

/// Waits until `checking` ends or `until`, whichever comes first; once its tests pass, at most
/// the grace more, unless a reply has already said so.
async fn wait_for(checking: &Checking, until: Instant) {
    let mut stage = checking.stage.subscribe();
    loop {
        let deadline = match *stage.borrow_and_update() {
            Stage::Ended => return,
            Stage::Proving(since) if !checking.green_told.load(Ordering::Relaxed) => {
                until.min(since + green_grace())
            }
            Stage::Testing | Stage::Proving(_) => until,
        };
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || tokio::time::timeout(left, stage.changed()).await.is_err() {
            return;
        }
    }
}

impl LenguaServer {
    /// `check`'s stages, up to the first that needs work, adding to `events` what the bundle's
    /// log records of them: the idea recorded, the program done, a proof that found it wrong.
    async fn check_stages(
        &self,
        folder: &ProgramFolder,
        params: &CheckParams,
        events: &mut Vec<graph::Event>,
    ) -> Result<CheckReport, String> {
        let began = Instant::now();
        let machine = Machine::detect();
        // lengua's entry in idea.md's `verified` stays only while it names the idea and main.wat
        // as they are, whatever stage this check stops at.
        let idea_text = match read_optional(folder, IDEA_FILE)? {
            Some(text) => Some(self.keep_verified(folder, text).await),
            None => None,
        };
        let Some(idea_text) = idea_text else {
            let next_step = if read_optional(folder, LEGACY_IDEA_FILE)?.is_some() {
                format!(
                    "Next: rename {folder}/{LEGACY_IDEA_FILE} to {IDEA_FILE}, the idea file's \
                     name now (what it says stays as it is), then call check with path {folder}."
                )
            } else if let Some(report) = wat::native_code(folder) {
                // main.s with no idea is the native lengua's code, which this lengua doesn't
                // read. Beside an idea, main.s waits until the idea stage has recorded the idea
                // with the user's words (check_wasm).
                return Ok(report);
            } else {
                State::NoIdea.next_step(folder)
            };
            return Ok(CheckReport {
                machine: Some(wat::machine_line(&machine)),
                ..CheckReport::new("write_idea", next_step)
            });
        };
        self.check_wasm(folder, params, events, (began, &machine), &idea_text)
            .await
    }

    /// `check`'s idea stage: idea.md, tests included, recorded with the user's words once it
    /// parses as an Attested Computation with no threads, its tests check and its parts are
    /// there, each a program with its own idea.md and main.wat.
    async fn idea_stage(
        &self,
        folder: &ProgramFolder,
        params: &CheckParams,
        events: &mut Vec<graph::Event>,
        idea_text: &str,
    ) -> Result<IdeaStage, String> {
        let request = match params.request.as_deref().map(str::trim) {
            Some(words) if !words.is_empty() => Some(words.to_owned()),
            _ => self.held.lock().await.get(folder.path()).cloned(),
        };
        let fix_idea = || {
            format!(
                "Next: fix {folder}/{IDEA_FILE} with the write-idea skill, then call check with \
                 path {folder}."
            )
        };
        let problems = if read_optional(folder, LEGACY_TESTS_FILE)?.is_some() {
            Err(vec![format!(
                "{LEGACY_TESTS_FILE}: the tests live in {IDEA_FILE} now. Make each behavior's \
                 steps say what its [[example]] said, write its [[generated]] tables as its \
                 generated tests, a `###` section of the same steps with a table of their vars \
                 and runs, as the write-idea skill shows, then delete {LEGACY_TESTS_FILE}."
            )])
        } else {
            match idea::parse(idea_text) {
                Err(problems) => Err(problems.iter().map(ToString::to_string).collect()),
                Ok(idea) => match testfile::check(&idea) {
                    Err(problems) => Err(problems.iter().map(ToString::to_string).collect()),
                    Ok(suite) => {
                        // An idea of the native lengua's type, then one with threads.
                        let problem = wat::type_problem(idea_text, &idea)
                            .or_else(|| wat::threads_problem(idea_text, &idea));
                        let missing = missing_parts(folder, &idea);
                        // A cycle of `- Builds on:` links blocks the programs in it, and only
                        // them.
                        if let Some(problem) = problem {
                            Err(vec![problem])
                        } else if !missing.is_empty() {
                            Err(missing)
                        } else if let Some(cycle) = graph::builds_on_cycle(folder.path()) {
                            Err(vec![cycle])
                        } else {
                            Ok((idea, suite))
                        }
                    }
                },
            }
        };
        let (idea, suite) = match problems {
            Ok(parsed) => parsed,
            Err(problems) => {
                if let Some(words) = &request {
                    self.held
                        .lock()
                        .await
                        .insert(folder.path().to_path_buf(), words.clone());
                }
                let report = CheckReport::problems("idea_problems", problems, fix_idea());
                return Ok(IdeaStage::Stops(Box::new(report)));
            }
        };
        match self.record(folder, idea_text, request.as_deref()).await? {
            Recording::Needs(next_step) => {
                let report = CheckReport::new("needs_request", next_step);
                return Ok(IdeaStage::Stops(Box::new(report)));
            }
            Recording::Recorded => events.push(graph::Event::Recorded),
            Recording::Unchanged => {}
        }
        self.held.lock().await.remove(folder.path());
        Ok(IdeaStage::Recorded(Box::new((idea, suite))))
    }

    /// The reply's lines about the graph of program folders that `folder` belongs to, once the
    /// bundle's index.md and log.md say what `events` did, when there are any.
    async fn graph_lines(&self, folder: &ProgramFolder, events: Vec<graph::Event>) -> Vec<String> {
        let data_dir = Arc::clone(&self.data_dir);
        let folder = folder.path().to_path_buf();
        let today = graph::Day::today();
        tokio::task::spawn_blocking(move || graph::pass(&data_dir, &folder, &events, today))
            .await
            .unwrap_or_else(|error| vec![format!("lengua couldn't read the graph: {error}")])
    }

    /// What the proof found wrong in `module`, the module a run of `folder` is about to run, when
    /// this server's job for the folder checked that very module and confirmed a counterexample:
    /// the tests passed, but these inputs, run for real, show main.wat is wrong. The module is the
    /// one the run holds, by the hash lengua.lock records, and no file is read again: a main.wat
    /// replaced or removed after the state's read can't hide what the job found.
    async fn proven_wrong(&self, folder: &ProgramFolder, module: &Sha256Hex) -> Option<String> {
        let checking = self.checks.lock().await.get(folder.path()).cloned()?;
        if checking.module != *module {
            return None;
        }
        let found = checking.found.lock().ok()?;
        found.wrong.then(|| {
            found
                .proofs
                .iter()
                .filter(|proof| proof.wrong)
                .map(BlockProof::line)
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    /// Records idea.md, unless the lock already records it as it is, with the user's words when
    /// given. Returns the next step instead when it can't be recorded without their words.
    async fn record(
        &self,
        folder: &ProgramFolder,
        text: &str,
        request: Option<&str>,
    ) -> Result<Recording, String> {
        let words = request.map(str::trim).filter(|words| !words.is_empty());
        if words.is_some_and(|words| words.chars().count() > MAX_REQUEST_CHARS) {
            return Err(format!(
                "request holds more than {MAX_REQUEST_CHARS} characters; quote only the user's \
                 words that ask for the program or the change."
            ));
        }
        let current = trust::idea_hash(text.as_bytes());
        let _writing = self.lock_writes.lock().await;
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        let previous = match lock::read(folder.path(), Some(&key)) {
            Ok(previous) => previous,
            Err(error) if words.is_none() => {
                return Ok(Recording::Needs(
                    State::LockInvalid(error).next_step(folder),
                ));
            }
            Err(_) => None,
        };
        // Words the build already has, or none, mean the same request.
        let new_words = words.filter(|words| {
            previous
                .as_ref()
                .is_none_or(|lock| !lock.request.words.iter().any(|known| known == words))
        });
        let lock = match (previous, new_words) {
            (None, None) => {
                return Ok(Recording::Needs(State::IdeaUnrecorded.next_step(folder)));
            }
            (Some(lock), None) => {
                if lock.idea.sha256 == current {
                    return Ok(Recording::Unchanged);
                }
                // The idea was revised under a request it already has: it's checked again, and
                // the change is shown.
                Lock {
                    idea: IdeaStamp {
                        sha256: current.clone(),
                    },
                    code: None,
                    proofs: None,
                    ..lock
                }
            }
            // New words in the middle of a build join it, so nothing changed before them drops
            // out of what is shown.
            (Some(lock), Some(words)) if lock.code.is_none() => {
                let mut request = lock.request;
                if request.words.len() >= lock::MAX_REQUESTS {
                    request.words.remove(1);
                }
                request.words.push(words.to_owned());
                Lock {
                    request,
                    idea: IdeaStamp {
                        sha256: current.clone(),
                    },
                    code: None,
                    proofs: None,
                }
            }
            // A new request after a finished build starts a new build. With the idea unchanged,
            // its checked code stays.
            (Some(lock), Some(words)) if lock.idea.sha256 == current => {
                let request = Lock::requested(current.clone(), words.to_owned()).request;
                Lock { request, ..lock }
            }
            (_, Some(words)) => Lock::requested(current.clone(), words.to_owned()),
        };
        // Every recorded version is kept by its hash, so later changes can be shown.
        write_stamped_copy(&self.data_dir, &current, text)?;
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        Ok(Recording::Recorded)
    }

    /// The checking job for these files, running or ended, unless it ended with failed tests,
    /// which run again, or passed them but lengua.lock no longer records the code it checked,
    /// or ended with proofs that stopped short and `further` asks to prove on.
    async fn current_checking(
        &self,
        folder: &ProgramFolder,
        files: &[Sha256Hex; 2],
        further: bool,
    ) -> Option<Arc<Checking>> {
        let checks = self.checks.lock().await;
        let checking = checks.get(folder.path())?;
        if checking.files != *files {
            return None;
        }
        let (failed, passed) = checking.found.lock().map_or((true, false), |found| {
            (!found.failures.is_empty(), found.passed.is_some())
        });
        let ended = *checking.stage.borrow() == Stage::Ended;
        if ended && failed {
            return None;
        }
        if passed {
            let lock = Key::load(&self.data_dir)
                .ok()
                .flatten()
                .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten());
            let recorded = lock
                .as_ref()
                .and_then(|lock| lock.code.as_ref())
                .is_some_and(|code| {
                    code.sha256 == files[1] && code.module_sha256 == checking.module
                });
            if !recorded {
                return None;
            }
            let short = lock
                .as_ref()
                .and_then(|lock| lock.proofs.as_ref())
                .is_some_and(|proofs| {
                    proofs.module_sha256 == checking.module
                        && proofs.bounded.iter().any(|bounded| bounded.more)
                });
            if further && ended && short {
                return None;
            }
        }
        Some(Arc::clone(checking))
    }

    /// Takes the proofs lengua.lock records about this very module, after a restart lost the job
    /// that made them, or when the job is to prove on, with why each other test is tested, not
    /// proven. Returns `None` when there are none, and otherwise the blocks to prove on, each
    /// with the length to start at.
    fn recorded_proofs(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        suite: &TestSuite,
    ) -> Option<Vec<(usize, usize)>> {
        let stamp = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.proofs)
            .filter(|proofs| proofs.module_sha256 == checking.module)?;
        let mut found = checking.found.lock().ok()?;
        let mut any_wrong = false;
        let mut further = Vec::new();
        for (index, (block, proof)) in suite.generated.iter().zip(&mut found.proofs).enumerate() {
            let id = block.id.to_string();
            if let Some(bounded) = stamp
                .bounded
                .iter()
                .find(|bounded| bounded.id == id && bounded.more && checking.further)
            {
                further.push((index, bounded.bytes + 1));
                continue;
            }
            if let Some((_, what)) = stamp.wrong.iter().find(|(known, _)| *known == id) {
                proof.detail = Some(what.clone());
                proof.wrong = true;
                any_wrong = true;
                continue;
            }
            proof.detail = Some(if stamp.proven.contains(&id) {
                "proven for every input in range, as lengua.lock records".to_owned()
            } else if let Some(bounded) = stamp.bounded.iter().find(|bounded| bounded.id == id) {
                format!(
                    "proven for every input with text up to {} of {} bytes, as lengua.lock \
                     records; longer texts are tested, not proven{}",
                    bounded.bytes,
                    bounded.of,
                    if bounded.more { FURTHER } else { "" }
                )
            } else if let Some((_, why)) = stamp.unproven.iter().find(|(known, _)| *known == id) {
                format!("{UNPROVEN}{why}")
            } else {
                "tested, not proven".to_owned()
            });
        }
        found.wrong |= any_wrong;
        Some(further)
    }

    /// Proves each generated test in turn, each with its share of [`PROOF_TIME`], and records
    /// what was proven, or why not: main.wat's module, as `code` holds it, whose tests just passed
    /// every run drawn from `seed`. With `further`, proves only those blocks, each from its length
    /// on, and keeps what lengua.lock records of the others.
    async fn prove_all(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        code: &wat::WasmCode,
        seed: u64,
        further: &[(usize, usize)],
    ) {
        let set = |index: usize, detail: String, wrong: bool| {
            if let Ok(mut found) = checking.found.lock() {
                found.wrong |= wrong;
                if let Some(proof) = found.proofs.get_mut(index) {
                    proof.progress = None;
                    proof.detail = Some(detail);
                    proof.wrong = wrong;
                }
            }
        };
        let suite = &code.suite;
        let idea = &code.idea;
        let model = Model {
            program: Arc::clone(&code.program),
            policy: crate::wasm::Policy::of(&idea.uses, &idea.network),
        };
        let deadline = Instant::now() + proof_time();
        // The ids proven, those found wrong, those tested, not proven with why, how many the
        // solvers proved, and why any rested on one solver.
        let mut proven_ids = Vec::new();
        let mut bounded_ids: Vec<(usize, lock::Bounded)> = Vec::new();
        let mut wrong_ids: Vec<(String, String)> = Vec::new();
        let mut unproven_ids: Vec<(usize, (String, String))> = Vec::new();
        let mut solved = 0;
        let mut alone_reason: Option<String> = None;
        // Proving on keeps what the lock records of every block it doesn't prove.
        let mut recorded_solvers = None;
        if !further.is_empty()
            && let Some(stamp) = Key::load(&self.data_dir)
                .ok()
                .flatten()
                .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
                .and_then(|lock| lock.proofs)
                .filter(|proofs| proofs.module_sha256 == checking.module)
        {
            for (index, block) in suite.generated.iter().enumerate() {
                if further.iter().any(|(again, _)| *again == index) {
                    continue;
                }
                let id = block.id.to_string();
                if stamp.proven.contains(&id) {
                    proven_ids.push((index, id.clone()));
                }
                if let Some(bounded) = stamp.bounded.iter().find(|bounded| bounded.id == id) {
                    bounded_ids.push((index, bounded.clone()));
                }
                if let Some(wrong) = stamp.wrong.iter().find(|(known, _)| *known == id) {
                    wrong_ids.push(wrong.clone());
                }
                if let Some(unproven) = stamp.unproven.iter().find(|(known, _)| *known == id) {
                    unproven_ids.push((index, unproven.clone()));
                }
            }
            recorded_solvers = Some(stamp.solvers.clone());
        }
        let listens = idea.network.listens.is_some();
        let blocks = suite.generated.len();
        // Small blocks first: they finish in their share and pass the rest on to the big ones.
        let mut order: Vec<usize> = if further.is_empty() {
            (0..blocks).collect()
        } else {
            further.iter().map(|(index, _)| *index).collect()
        };
        let blocks = order.len();
        order.sort_by_key(|index| prove::size(&suite.generated[*index]));
        // The last block's line goes final only once the lock records the proofs, so a caller that
        // sees every proof settled, or a server restarted at that moment, finds them there.
        let mut last_line: Option<(usize, String, bool)> = None;
        let release = |line: Option<(usize, String, bool)>| {
            if let Some((index, detail, wrong)) = line {
                set(index, detail, wrong);
            }
        };
        for (turn, &index) in order.iter().enumerate() {
            let block = &suite.generated[index];
            let last = turn + 1 == blocks;
            if checking.cancel.load(Ordering::Relaxed) {
                return;
            }
            // Each block gets its share of the time left, so one big block can't starve the
            // rest; time a block doesn't use passes on to the next.
            let now = Instant::now();
            let share = deadline.saturating_duration_since(now)
                / u32::try_from(blocks - turn).unwrap_or(u32::MAX).max(1);
            // Every value of a block's variables is every input only when the program reads
            // nothing else: no random bytes or clock readings the test doesn't supply, and no
            // `now` in the expected results.
            let host_values = wat::host_values(idea, block);
            if let Some(every) = block
                .every_value_text()
                .filter(|_| !host_values && !block.uses_now())
            {
                proven_ids.push((index, block.id.to_string()));
                let detail = format!("proven: the tests tried {every}");
                if last {
                    last_line = Some((index, detail, false));
                } else {
                    set(index, detail, false);
                }
                continue;
            }
            let budget = prove::Budget {
                cancel: Arc::clone(&checking.cancel),
                from_length: further
                    .iter()
                    .find(|(again, _)| *again == index)
                    .map_or(0, |(_, from)| *from),
                ..prove::Budget::until(now + share, listens)
            };
            if let Ok(mut found) = checking.found.lock()
                && let Some(proof) = found.proofs.get_mut(index)
            {
                proof.progress = Some(Arc::clone(&budget.progress));
            }
            // The model must agree with the real program on inputs it passed before it's
            // trusted to cover the rest: the block's first runs, drawn from the tests' own seed.
            let samples: Vec<testfile::Case> =
                block.cases(seed).take(24).filter_map(Result::ok).collect();
            let verdict = model.verdict(block, samples, listens, budget).await;
            if checking.cancel.load(Ordering::Relaxed) {
                return;
            }
            let proved = matches!(verdict, Ok(prove::Verdict::Proven { .. }));
            let (detail, wrong) = match verdict {
                Err(reason) | Ok(prove::Verdict::NotProven(reason)) => {
                    (format!("tested, not proven: {reason}"), false)
                }
                Ok(prove::Verdict::Proven {
                    shapes,
                    paths,
                    questions,
                    alone,
                    bound,
                    cut_any_way,
                    unreached,
                    induction,
                    lookback,
                }) => {
                    match &bound {
                        Some(bound) => bounded_ids.push((
                            index,
                            lock::Bounded {
                                id: block.id.to_string(),
                                bytes: bound.bytes,
                                of: bound.of,
                                more: bound.more,
                            },
                        )),
                        None => proven_ids.push((index, block.id.to_string())),
                    }
                    if questions > 0 {
                        solved += 1;
                        if alone_reason.is_none() {
                            alone_reason.clone_from(&alone);
                        }
                    }
                    (
                        proven_detail(
                            block,
                            shapes,
                            paths,
                            questions,
                            alone,
                            bound,
                            cut_any_way,
                            &unreached,
                            induction.map(|chunk| (chunk, lookback)),
                        ),
                        false,
                    )
                }
                Ok(prove::Verdict::Counterexample(values)) => {
                    let pool = self.pool();
                    wat::replay(&pool, &code.program, idea, block, values, host_values).await
                }
            };
            // Why a test is tested, not proven outlasts this job too, so a check after a restart
            // says why again.
            if !proved && !wrong {
                let why = detail.strip_prefix(UNPROVEN).unwrap_or(&detail);
                unproven_ids.push((index, (block.id.to_string(), why.to_owned())));
            }
            if wrong {
                // At once, so the finding outlasts this job: run refuses the build from now on.
                wrong_ids.push((block.id.to_string(), detail.clone()));
                self.stamp_proofs(
                    folder,
                    ProofStamp {
                        module_sha256: checking.module.clone(),
                        proven: proven_ids.iter().map(|(_, id)| id.clone()).collect(),
                        bounded: bounded_ids.iter().map(|(_, bound)| bound.clone()).collect(),
                        solvers: "z3 and cvc5".to_owned(),
                        wrong: wrong_ids.clone(),
                        unproven: unproven_ids.iter().map(|(_, test)| test.clone()).collect(),
                    },
                )
                .await;
                // lengua's entry in idea.md's `verified` goes at once too, before any reply says
                // the build is wrong; the lock now keeps another from going in.
                self.unverify(folder, checking).await;
                // Then its line goes final, the last block's too: the lock already records the
                // finding, so no reply need wait for its last write to say the build is wrong.
                set(index, detail, true);
            } else if last {
                last_line = Some((index, detail, false));
            } else {
                set(index, detail, false);
            }
        }
        if proven_ids.is_empty() && bounded_ids.is_empty() && wrong_ids.is_empty() {
            release(last_line);
            return;
        }
        let solvers = match (solved, alone_reason, recorded_solvers) {
            // Proving on with no question keeps what the earlier proof needed.
            (0, _, Some(recorded)) => recorded,
            (0, ..) => {
                "none: the tests tried every value, or the terms settled every step".to_owned()
            }
            (_, None, _) => "z3 and cvc5".to_owned(),
            (_, Some(reason), _) => format!("one solver alone: {reason}"),
        };
        // In the tests' own order, however they were proven.
        proven_ids.sort_unstable();
        bounded_ids.sort_unstable_by_key(|(index, _)| *index);
        unproven_ids.sort_unstable_by_key(|(index, _)| *index);
        self.stamp_proofs(
            folder,
            ProofStamp {
                module_sha256: checking.module.clone(),
                proven: proven_ids.into_iter().map(|(_, id)| id).collect(),
                bounded: bounded_ids.into_iter().map(|(_, bound)| bound).collect(),
                solvers,
                wrong: wrong_ids,
                unproven: unproven_ids.into_iter().map(|(_, test)| test).collect(),
            },
        )
        .await;
        release(last_line);
    }

    /// What `check` replies about a checking job.
    fn checking_report(&self, folder: &ProgramFolder, checking: &Checking) -> CheckReport {
        let stage = *checking.stage.borrow();
        let code = CODE_FILE;
        let Ok(found) = checking.found.lock() else {
            return CheckReport::new("testing", again(folder));
        };
        let proofs: Vec<String> = found.proofs.iter().map(BlockProof::line).collect();
        if !found.failures.is_empty() {
            return CheckReport::problems(
                "tests_failed",
                found.failures.clone(),
                format!(
                    "Next: decide from {IDEA_FILE} whether {code} or the test is wrong, fix \
                     {code} with the compile skill or the test in {IDEA_FILE} with the \
                     write-idea skill, then call check with path {folder}. Boundary values run \
                     first every time; the other generated inputs are new on every check."
                ),
            );
        }
        let Some(passed) = &found.passed else {
            return CheckReport::new(
                "testing",
                format!("The tests are running. {}", again(folder)),
            );
        };
        checking.green_told.store(true, Ordering::Relaxed);
        let mut tests = vec![format!(
            "{code} follows every rule and passed all {} test runs",
            passed.runs
        )];
        tests.extend(passed.every_value.iter().cloned());
        if found.wrong {
            return CheckReport {
                tests,
                proofs,
                ..CheckReport::problems(
                    "tests_failed",
                    found
                        .proofs
                        .iter()
                        .filter(|proof| proof.wrong)
                        .map(BlockProof::line)
                        .collect(),
                    format!(
                        "Next: fix {folder}/{code} with the compile skill so it handles those \
                         inputs, then call check with path {folder}."
                    ),
                )
            };
        }
        // Done once the tests pass and the grace is over: proofs still running go on, and a
        // later check shows them.
        let still_proving = match stage {
            Stage::Proving(since) if since.elapsed() < green_grace() => {
                return CheckReport {
                    tests,
                    proofs,
                    memory: Some(wat::memory_line(&checking.obligations, &found.proofs)),
                    ..CheckReport::new(
                        "proving",
                        format!(
                            "Every test passes, and the proof is running in the background. {}",
                            again(folder)
                        ),
                    )
                };
            }
            Stage::Proving(_) => true,
            Stage::Testing | Stage::Ended => false,
        };
        let changes = self.current_changes(folder);
        let changes = (changes.requests.len() > 1
            || changes.idea.is_some()
            || !changes.tests_less.is_empty())
        .then_some(ChangesReport {
            requests: changes.requests,
            idea_changed: changes.idea,
            tests_less: changes.tests_less,
        });
        let shown = if changes.is_some() {
            " changes lists what changed after the user's request: tell them each change, and why."
        } else {
            ""
        };
        let running = if still_proving {
            " The proofs that say proving go on in the background for up to five minutes: tell \
             the user they're still running, and call check again before they rely on the \
             program: a proof that finds inputs where it's wrong shows there, and run refuses \
             that build from then on."
        } else {
            ""
        };
        // check writes lengua's entry in the idea as it replies done, as VERIFIED_NOTE says.
        CheckReport {
            tests,
            proofs,
            memory: Some(wat::memory_line(&checking.obligations, &found.proofs)),
            example: passed.shown.clone(),
            performance: passed.performance.clone(),
            changes,
            shell: Some(self.shell_command(folder)),
            ..CheckReport::new(
                "done",
                format!(
                    "Done: tell the user where the program is, what its idea promises, that it \
                     passed every test, and what's proven, memory \
                     included.{VERIFIED_NOTE}{running}{shown} run runs it on their own inputs \
                     here; \
                     give them the shell command, followed by its arguments, for their own \
                     terminal, scripts and services."
                ),
            )
        }
    }

    /// The command that runs `folder`'s program from a shell: then its arguments. The data folder
    /// is named only where a shell wouldn't find this one by itself.
    fn shell_command(&self, folder: &ProgramFolder) -> String {
        let quote = |text: &str| {
            if !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "/._-+=:@%,".contains(c))
            {
                text.to_owned()
            } else {
                format!("'{}'", text.replace('\'', "'\\''"))
            }
        };
        let mut command = quote(&self.launcher.to_string_lossy());
        if DataDir::from_env().as_ref().map(DataDir::path) != Some(self.data_dir.path()) {
            command.push_str(" --data-dir ");
            command.push_str(&quote(&self.data_dir.path().to_string_lossy()));
        }
        format!(
            "{command} run {} --",
            quote(&folder.path().to_string_lossy())
        )
    }

    /// What changed after the user's request, by the program's current lock; nothing when it
    /// has no lock that can be trusted.
    fn current_changes(&self, folder: &ProgramFolder) -> Changes {
        let lock = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten());
        lock.map_or_else(Changes::default, |lock| self.changes(folder, &lock))
    }

    /// What changed after the user's request: idea.md, tests included, since the build's first
    /// request, against the copy lengua kept.
    fn changes(&self, folder: &ProgramFolder, lock: &Lock) -> Changes {
        // lengua's own entry in `verified` is no change of the user's, in the copy lengua kept as
        // in idea.md now.
        let theirs = |text: String| trust::without_lengua_verified(&text).unwrap_or(text);
        let kept = |hash: &Sha256Hex| {
            std::fs::read_to_string(stamped_copy(self.data_dir.path(), hash))
                .ok()
                .filter(|text| trust::idea_hash(text.as_bytes()) == *hash)
                .map(theirs)
        };
        let unseen = "the first version isn't on this machine to compare";
        let idea = match std::fs::read_to_string(folder.file(IDEA_FILE)) {
            Ok(now) if trust::idea_hash(now.as_bytes()) != lock.request.idea => {
                let now = theirs(now);
                Some(kept(&lock.request.idea).map_or_else(
                    || format!("{IDEA_FILE} changed; {unseen}"),
                    |before| testdiff::line_diff(&before, &now),
                ))
            }
            _ => None,
        };
        // Tests that test less than the ones first recorded for the request.
        let tests_less = match std::fs::read_to_string(folder.file(IDEA_FILE)) {
            Ok(now) => kept(&lock.request.idea)
                .map(|before| testdiff::tests_less(&before, &now))
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        Changes {
            requests: lock.request.words.clone(),
            idea: idea.filter(|diff| !diff.is_empty()),
            tests_less,
        }
    }

    /// Records checked code in the lock, provided neither the idea nor the code changed while
    /// being checked. The module kept for the stamp it replaces goes, when that was another.
    async fn stamp_code(
        &self,
        folder: &ProgramFolder,
        idea: &Sha256Hex,
        code: CodeStamp,
    ) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        let file = CODE_FILE;
        let changed = || {
            format!(
                "{IDEA_FILE} or {file} changed while the code was being checked, so nothing was \
                 recorded; call check again."
            )
        };
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or_else(changed)?;
        let lock = lock::read(folder.path(), Some(&key))
            .ok()
            .flatten()
            .ok_or_else(changed)?;
        let current = read_optional(folder, file)?.ok_or_else(changed)?;
        let unchanged = lock.idea.sha256 == *idea
            && trust::idea_hash(read_idea(folder)?.as_bytes()) == *idea
            && Sha256Hex::of(current.as_bytes()) == code.sha256;
        if !unchanged {
            return Err(changed());
        }
        // Proofs about this very build still hold; of any other, only what a proof found wrong
        // about the same main.wat goes on ([`kept_proofs`]).
        let proofs = kept_proofs(lock.code.as_ref(), lock.proofs, &code);
        // The module the stamp replaces, when it is another: its kept file goes once the new
        // stamp is written, and a folder whose lock still names it builds it at its next run.
        let replaced = lock
            .code
            .as_ref()
            .map(|checked| checked.module_sha256.clone())
            .filter(|module| *module != code.module_sha256);
        let lock = Lock {
            code: Some(code),
            proofs,
            ..lock
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        if let Some(module) = replaced {
            let builder = crate::wasm::text::BUILDER;
            let _ = std::fs::remove_file(tested_module(self.data_dir.path(), builder, &module));
        }
        Ok(())
    }

    /// Records in the lock what the proofs found, `proofs`, about the build they name, provided
    /// the lock still records that build as the checked code. Returns whether it did.
    async fn stamp_proofs(&self, folder: &ProgramFolder, proofs: ProofStamp) -> bool {
        let delay = stamp_delay();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let _writing = self.lock_writes.lock().await;
        let Ok(Some(key)) = Key::load(&self.data_dir) else {
            return false;
        };
        let Ok(Some(lock)) = lock::read(folder.path(), Some(&key)) else {
            return false;
        };
        if lock
            .code
            .as_ref()
            .is_none_or(|code| code.module_sha256 != proofs.module_sha256)
        {
            return false;
        }
        let lock = Lock {
            proofs: Some(Box::new(proofs)),
            ..lock
        };
        lock::write(folder.path(), &lock, &key).is_ok()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LenguaServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lengua", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// What changed after the user's request.
#[derive(Debug, Default)]
struct Changes {
    /// The user's requests in this build, oldest first.
    requests: Vec<String>,
    /// idea.md since the build's first request.
    idea: Option<String>,
    /// The test changes that test less, by lengua's own comparison.
    tests_less: Vec<String>,
}

/// The next step while a job runs: call again.
fn again(folder: &ProgramFolder) -> String {
    // Calls a harness runs one after another each wait their own window: one at a time keeps
    // every call short.
    format!(
        "Call check again with path {folder}, one call at a time, to wait for it; nothing needs \
         to change."
    )
}

/// What a proof runs on: main.wat's module, with what its idea lets it reach.
#[derive(Clone)]
struct Model {
    /// main.wat's module.
    program: Arc<crate::wasm::Program>,
    /// What its idea lets it reach.
    policy: crate::wasm::Policy,
}

impl Model {
    /// The verdict on `block` within `budget`, in a thread of its own: the model must agree with
    /// the real program on `samples`, inputs it passed, before it's trusted to cover the rest.
    /// They must be runs the tests made, drawn from their seed: a sample the program never ran
    /// may be one it is wrong on, which a disagreement would hide.
    async fn verdict(
        &self,
        block: &testfile::Generated,
        samples: Vec<testfile::Case>,
        listens: bool,
        budget: prove::Budget,
    ) -> Result<prove::Verdict, String> {
        let (model, block) = (self.clone(), block.clone());
        tokio::task::spawn_blocking(move || {
            let (built, policy) = (model.program.built(), model.policy);
            let found = prove::agrees_wasm(built, policy, &samples, listens, budget.deadline);
            found.map(|found| {
                found.unwrap_or_else(|| prove::prove_wasm(built, policy, &block, &budget))
            })
        })
        .await
        .unwrap_or_else(|error| Err(format!("the proof stopped: {error}")))
    }
}

/// Inputs a proof found, as a real run is given them: the generated test's values, how what the
/// test's peer or standard input sends arrives, what the model said a run broke, and all of it
/// in words.
struct Witness {
    /// The test's values.
    inputs: expr::Env,
    /// How what the test sends arrives: the delivery the proof found them with.
    delivery: testrun::Delivery,
    /// What the model said a run broke: a trap of main.wat's.
    broken: Option<String>,
    /// All of it, in words.
    described: String,
}

impl Witness {
    /// The witness in `values`, a counterexample of `block`'s: its inputs, beside the delivery it
    /// was found with and what broke, under the names [`prove::DELIVERY`] and [`prove::UNSAFE`].
    fn of(block: &testfile::Generated, mut values: BTreeMap<String, expr::Value>) -> Self {
        // The delivery it was found with isn't the test's input: a real run is given it.
        let delivery = match values.remove(prove::DELIVERY) {
            Some(expr::Value::Str(found)) => {
                testrun::Delivery::described(&found).unwrap_or(testrun::Delivery::Whole)
            }
            _ => testrun::Delivery::Whole,
        };
        let networked = !block.server.is_empty() || !block.client.is_empty();
        let broken = match values.remove(prove::UNSAFE) {
            Some(expr::Value::Str(problem)) => Some(problem),
            _ => None,
        };
        let inputs: expr::Env = values.into_iter().collect();
        let described = if networked {
            format!(
                "{}, what the test's peer sends arriving {}",
                testfile::describe_inputs(&inputs),
                delivery.describe()
            )
        } else if delivery == testrun::Delivery::Whole {
            testfile::describe_inputs(&inputs)
        } else {
            format!(
                "{}, standard input arriving {}",
                testfile::describe_inputs(&inputs),
                delivery.describe()
            )
        };
        Self {
            inputs,
            delivery,
            broken,
            described,
        }
    }
}

/// A proof's line for what a real run of the inputs it found came to, `ran`: whether main.wat is
/// wrong on them, `described`, and whether it is. Where the run passed, the model was wrong about
/// them, and about what it said the run broke, `broken`, unless the run reads random bytes or the
/// clock, `host_values`, which a real run can't be given: then the line says what the model
/// found, a trap and its line too.
fn settled(
    described: &str,
    broken: Option<&str>,
    ran: std::io::Result<Option<String>>,
    host_values: bool,
) -> (String, bool) {
    let file = CODE_FILE;
    match ran {
        Err(error) => (
            format!("tested, not proven: couldn't run the program: {error}"),
            false,
        ),
        Ok(Some(problem)) => (
            format!(
                "the proof found inputs the tests didn't try where {file} is wrong: with \
                 {described}: {problem}"
            ),
            true,
        ),
        // A real run draws its own random bytes and reads its own clock, so it can't replay
        // what the proof found: a trap there, the model's, is named with its line.
        Ok(None) if host_values => match broken {
            Some(problem) => (
                format!(
                    "tested, not proven: with {described}, the prover found random bytes or a \
                     clock reading for which {file} traps ({problem}), which a real run can't \
                     replay; check how {file} uses them there"
                ),
                false,
            ),
            None => (
                format!(
                    "tested, not proven: with {described}, the prover found random bytes or a \
                     clock reading for which {file}'s output breaks the test, which a real run \
                     can't replay; check how {file} turns them into output"
                ),
                false,
            ),
        },
        Ok(None) => match broken {
            Some(problem) => (
                format!(
                    "tested, not proven: the prover's model trapped with {described} \
                     ({problem}), and a real run didn't"
                ),
                false,
            ),
            None => (
                format!(
                    "tested, not proven: the prover's model disagreed with a real run on \
                     {described}"
                ),
                false,
            ),
        },
    }
}

/// The proofs a new stamp of main.wat, `code`, keeps of the lock's, `proofs`, which are about the
/// code the lock stamped before, `before`. About the very module stamped, all of them: code
/// edited without changing its module, as by a comment, keeps them, though the lines their
/// records name may have moved since, which those records then say. About another module of the
/// same main.wat, as when lengua's own build changed, what they found wrong and nothing else,
/// now about the new module: a wrong verdict refuses more, never less, while what the others say
/// held of another module. None after an edit that changed the module, or of proofs about
/// another module than the one stamped before: the new module is proven afresh.
fn kept_proofs(
    before: Option<&CodeStamp>,
    proofs: Option<Box<ProofStamp>>,
    code: &CodeStamp,
) -> Option<Box<ProofStamp>> {
    let mut proofs = proofs?;
    let edited = before.is_some_and(|checked| checked.sha256 != code.sha256);
    if proofs.module_sha256 == code.module_sha256 {
        if edited {
            lines_may_have_moved(&mut proofs);
        }
        return Some(proofs);
    }
    let before = before?;
    if edited || proofs.module_sha256 != before.module_sha256 || proofs.wrong.is_empty() {
        return None;
    }
    let ProofStamp { solvers, wrong, .. } = *proofs;
    Some(Box::new(ProofStamp {
        module_sha256: code.module_sha256.clone(),
        proven: Vec::new(),
        bounded: Vec::new(),
        solvers,
        wrong,
        unproven: Vec::new(),
    }))
}

/// Notes on what `proofs` found wrong, or why they left a test unproven, once main.wat is edited
/// without changing its module, as by a comment: a record that names one of its lines says the
/// lines may have moved since, once, and the others stay as they were.
fn lines_may_have_moved(proofs: &mut ProofStamp) {
    let file = CODE_FILE;
    let note = format!(
        " ({file}'s lines as they were when the proof ran: an edit since left the build the \
         same, but may have moved them)"
    );
    let names = [format!("{file} line "), format!("{file}:")];
    for (_, found) in proofs.wrong.iter_mut().chain(proofs.unproven.iter_mut()) {
        if names.iter().any(|name| found.contains(name.as_str())) && !found.ends_with(&note) {
            found.push_str(&note);
        }
    }
}

/// What a proven block's proof covered, in a line, whose paths it followed through main.wat's
/// WebAssembly.
#[allow(clippy::too_many_arguments)]
fn proven_detail(
    block: &testfile::Generated,
    shapes: usize,
    paths: usize,
    questions: usize,
    alone: Option<String>,
    bound: Option<prove::Bound>,
    cut_any_way: bool,
    unreached: &[String],
    induction: Option<(usize, usize)>,
) -> String {
    let mut detail = format!(
        "proven for every input {} ({}, {} through the WebAssembly), {}",
        bound.as_ref().map_or_else(
            || "in range".to_owned(),
            |bound| {
                let unit = if bound.characters {
                    "characters"
                } else {
                    "bytes"
                };
                format!("with text up to {} of {} {unit}", bound.bytes, bound.of)
            }
        ),
        counted(shapes, "input combination"),
        counted(paths, "path"),
        match (questions, alone) {
            (0, _) =>
                "every step settled by the terms themselves, so no solver was needed".to_owned(),
            (_, None) => {
                "with each solver question confirmed by two independent solvers, z3 and cvc5"
                    .to_owned()
            }
            (_, Some(reason)) => format!(
                "by one solver alone ({reason}; with both z3 and cvc5 installed, each step is \
                 confirmed twice)"
            ),
        }
    );
    match induction {
        Some((chunk, 0)) => {
            let _ = write!(
                detail,
                "; every length by induction over its read loop: from any state the loop can be \
                 in at its head, one more buffer-full of {chunk} bytes keeps what it writes and \
                 counts in step with the formula, and every input of up to {chunk} bytes is \
                 proven from there"
            );
        }
        Some((chunk, back)) => {
            let bytes = if back == 1 { "byte" } else { "bytes" };
            let _ = write!(
                detail,
                "; every length by induction over its read loop: from any state the loop can be \
                 in at its head, two more buffer-fulls of {chunk} bytes keep its counts in step \
                 with the formula, the second's counted after the first's last {back} {bytes} \
                 however the program keeps them; every final piece of up to {chunk} bytes after \
                 a buffer-full is proven from there, and every input of up to {chunk} bytes from \
                 the start"
            );
        }
        None => {}
    }
    if let Some(bound) = &bound {
        let _ = write!(
            detail,
            "; longer texts are tested, not proven: {}{}",
            bound.why,
            if bound.more { FURTHER } else { "" }
        );
    }
    let networked = !block.server.is_empty() || !block.client.is_empty();
    if block.clients_at_once {
        detail.push_str(
            "; the clients taking their steps in every order between the server's polls, and as \
             the test does, each holding its last byte until the next has its answer",
        );
    }
    if networked && cut_any_way {
        detail.push_str("; what the test's peer sends arriving cut into pieces any way");
    } else if networked {
        detail.push_str(
            "; what the test's peer sends arriving whole or cut after its first byte, at its \
             middle, or both",
        );
    } else if cut_any_way && !block.stdin.is_empty() {
        detail.push_str("; standard input arriving cut into pieces any way");
    }
    // A fast path for inputs past the proof's bound is code no run reached: proven says
    // nothing of it.
    if let Some((last, rest)) = unreached.split_last() {
        let names = if rest.is_empty() {
            last.clone()
        } else {
            format!("{} or {last}", rest.join(", "))
        };
        let pronoun = if rest.is_empty() {
            "it does"
        } else {
            "they do"
        };
        let _ = write!(
            detail,
            "; the proof never ran {names}, so what {pronoun} isn't proven"
        );
    }
    detail
}

/// A test's cost in one line, medians only: the WebAssembly operations a run executes, and its
/// time on the clock.
fn cost_line(cost: &testrun::Cost) -> String {
    let mut line = format!("{}: {}", cost.test, counted(cost.runs, "run"));
    if let Some(operations) = cost.instructions {
        let _ = write!(line, ", {} WebAssembly operations each", operations.median);
    }
    if let Some(wall) = cost.wall_time_us {
        let _ = write!(
            line,
            "; {}: median {} µs on the clock",
            cost.timed, wall.median
        );
    }
    line
}

/// The program folder `path` names: the folder itself or its idea.md.
fn folder_of(path: &str) -> Result<ProgramFolder, String> {
    let path = path.trim();
    match Path::new(path).file_name() {
        Some(name) if name == IDEA_FILE => ProgramFolder::new(
            Path::new(path)
                .parent()
                .and_then(Path::to_str)
                .unwrap_or_default(),
        ),
        _ => ProgramFolder::new(path),
    }
}

/// A program file, or `None` when it doesn't exist yet.
fn read_optional(folder: &ProgramFolder, name: &str) -> Result<Option<String>, String> {
    match std::fs::read_to_string(folder.file(name)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("couldn't read {folder}/{name}: {error}")),
    }
}

fn read_idea(folder: &ProgramFolder) -> Result<String, String> {
    read_optional(folder, IDEA_FILE)?.ok_or_else(|| {
        format!("There's no {IDEA_FILE} in {folder}. Write it with the write-idea skill first.")
    })
}

/// The parts `idea` builds on that aren't lengua ideas in their folders, each a problem saying
/// so: a part is a folder with its own idea.md and main.wat.
fn missing_parts(folder: &ProgramFolder, idea: &idea::Idea) -> Vec<String> {
    idea.parts
        .iter()
        .filter_map(|part| {
            let path = folder.path().join(&part.idea);
            match std::fs::read_to_string(&path) {
                Err(_) => Some(format!(
                    "`- Builds on:` links to [{}]({}), but {} isn't there: make that part first, \
                     a folder with its own idea.md and {CODE_FILE}, or fix the link",
                    part.name,
                    part.idea,
                    path.display()
                )),
                Ok(text) if idea::parse(&text).is_err() => Some(format!(
                    "the part [{}]({}) has idea problems of its own: call check with path {} \
                     first",
                    part.name,
                    part.idea,
                    path.parent()
                        .map_or(path.clone(), Path::to_path_buf)
                        .display()
                )),
                Ok(_) => None,
            }
        })
        .collect()
}

fn state_of(data_dir: &DataDir, folder: &ProgramFolder) -> Result<State, String> {
    let key = Key::load(data_dir).map_err(|error| error.to_string())?;
    let snapshot = Snapshot::read(folder, key.as_ref())
        .map_err(|error| format!("can't read {folder}: {error}"))?;
    Ok(State::of(&snapshot))
}

/// [`state_of`] for a run: the folder's state, with the snapshot it comes from, whose lock is the
/// one lengua's key verified, and the bytes of idea.md and main.wat that snapshot hashed, which
/// the run takes in place of reading the lock or either file again.
fn state_for_run(
    data_dir: &DataDir,
    folder: &ProgramFolder,
) -> Result<(State, Snapshot, Files), String> {
    let key = Key::load(data_dir).map_err(|error| error.to_string())?;
    let (snapshot, files) = Snapshot::read_with_files(folder, key.as_ref())
        .map_err(|error| format!("can't read {folder}: {error}"))?;
    Ok((State::of(&snapshot), snapshot, files))
}

/// The connections `network` names, in order, as lengua makes them for a program: a port of this
/// computer, of a named computer, over TLS, or of the computer an argument names.
fn reaches(network: &idea::Network) -> Vec<crate::connect::Reach> {
    network
        .connections
        .iter()
        .map(|place| match (&place.host, place.argument) {
            (_, Some(index)) => crate::connect::Reach::Argument {
                index,
                port: place.port,
                tls: place.tls,
            },
            (None, None) => crate::connect::Reach::ThisComputer(place.port),
            (Some(host), None) if place.tls => crate::connect::Reach::Tls(host.clone(), place.port),
            (Some(host), None) => crate::connect::Reach::Host(host.clone(), place.port),
        })
        .collect()
}

/// What `lengua-mcp run` was asked to run from a shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellRun {
    /// The program folder, absolute or relative to the shell's current folder.
    pub path: String,
    /// The folder to run in, which the program may read and write: the shell's current folder
    /// when `None`.
    pub folder: Option<String>,
    /// Environment variables for a program whose idea uses the environment: only these reach
    /// it.
    pub env: BTreeMap<String, String>,
    /// The program's command-line arguments.
    pub args: Vec<String>,
}

/// Runs a checked program from a shell, script or service: `run`'s checks and confinement, with
/// the shell's own standard input, output and error and no time limit, main.wat's module in this
/// process. Returns the program's exit code, or 134 when it trapped, which it says on standard
/// error. It is a plain function, and needs no async runtime: nothing on its way waits on one,
/// and the connections lengua makes for a program have a runtime of their own.
///
/// # Errors
///
/// Fails, saying why, when lengua won't run the program or can't start it.
pub fn run_from_shell(data_dir: &DataDir, request: &ShellRun) -> Result<i32, String> {
    let here = std::env::current_dir()
        .map_err(|error| format!("can't read the current folder: {error}"))?;
    let absolute = |path: &str| here.join(path).to_string_lossy().into_owned();
    let folder = ProgramFolder::new(&absolute(&request.path))?;
    let workspace = working_folder(
        &request
            .folder
            .as_deref()
            .map_or_else(|| here.to_string_lossy().into_owned(), absolute),
    )?;
    wat::run_from_shell(data_dir, &folder, request, &workspace)
}

/// The folder a `run` happens in: an existing absolute folder other than `/`, where confining
/// the program would mean nothing.
fn working_folder(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(format!(
            "folder must be an absolute path, such as /home/you/notes, not {}",
            path.display()
        ));
    }
    let folder = path
        .canonicalize()
        .map_err(|error| format!("folder {}: {error}", path.display()))?;
    if !folder.is_dir() {
        return Err(format!("folder {} isn't a folder", folder.display()));
    }
    if folder.parent().is_none() {
        return Err("folder can't be /; name the folder that holds the user's files".into());
    }
    Ok(folder)
}

fn random_seed() -> Result<u64, String> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(|error| format!("couldn't draw a seed: {error}"))?;
    Ok(u64::from_le_bytes(bytes))
}

/// Saves a recorded idea.md under its hash, written atomically.
fn write_stamped_copy(data_dir: &DataDir, file: &Sha256Hex, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let path = stamped_copy(data_dir.path(), file);
    if std::fs::read(&path).is_ok_and(|kept| trust::idea_hash(&kept) == *file) {
        return Ok(());
    }
    let failed = |error: std::io::Error| {
        format!(
            "couldn't keep a copy of {} at {}: {error}",
            file.label(),
            path.display()
        )
    };
    let folder = path.parent().unwrap_or(data_dir.path());
    std::fs::create_dir_all(folder).map_err(failed)?;
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(failed)?;
    file.write_all(text.as_bytes()).map_err(failed)?;
    file.persist(&path).map_err(|error| failed(error.error))?;
    Ok(())
}

/// Keeps `wasm`, a module whose tests passed, whose SHA-256 is `module`, in this build's folder
/// of the data folder ([`tested_module`]), written atomically, for a run from a shell to run:
/// unless it is there already.
fn keep_tested(data_dir: &DataDir, module: &Sha256Hex, wasm: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let path = tested_module(data_dir.path(), crate::wasm::text::BUILDER, module);
    if std::fs::read(&path).is_ok_and(|kept| Sha256Hex::of(&kept) == *module) {
        return Ok(());
    }
    let failed = |error: std::io::Error| {
        format!(
            "couldn't keep the module check tested at {}: {error}",
            path.display()
        )
    };
    let folder = path.parent().unwrap_or(data_dir.path());
    std::fs::create_dir_all(folder).map_err(failed)?;
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(failed)?;
    file.write_all(wasm).map_err(failed)?;
    file.persist(&path).map_err(|error| failed(error.error))?;
    Ok(())
}

fn idea_problems(folder: &ProgramFolder, problems: &[idea::Problem]) -> String {
    let mut text = format!("{folder}/{IDEA_FILE} has problems; call check:\n");
    for problem in problems {
        let _ = writeln!(text, "- {problem}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_fit_in_512_characters() {
        assert!(
            INSTRUCTIONS.chars().count() <= 512,
            "instructions are {} characters",
            INSTRUCTIONS.chars().count()
        );
    }

    #[test]
    fn server_reports_its_own_name_and_version() {
        let server = LenguaServer::new(DataDir::new("/tmp/lengua-test".into()));
        let info = server.get_info();
        assert_eq!(info.server_info.name, "lengua");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.instructions.as_deref(), Some(INSTRUCTIONS));
    }

    /// lengua's entry in `verified` is no change to the idea: recording the idea again finds the
    /// lock as it was, and any other change is recorded.
    #[tokio::test]
    async fn the_trust_record_is_no_change_to_record() {
        let data = tempfile::tempdir().expect("a data folder");
        let program = tempfile::tempdir().expect("a program folder");
        let server = LenguaServer::new(DataDir::new(data.path().to_path_buf()));
        let path = program.path().to_str().expect("UTF-8 path");
        let folder = ProgramFolder::new(path).expect("absolute path");
        let idea = include_str!("../tests/fixtures/add/idea.md");
        let first = server.record(&folder, idea, Some("an adder")).await;
        assert!(matches!(first, Ok(Recording::Recorded)));
        let hash = trust::idea_hash(idea.as_bytes());
        let entry = trust::Verified::new("2026-11-06T23:00:00Z", hash, Sha256Hex::of(b"code"));
        let verified = trust::with_lengua_verified(idea, &entry);
        assert_ne!(verified, idea);
        let again = server.record(&folder, &verified, None).await;
        assert!(matches!(again, Ok(Recording::Unchanged)));
        let changed = verified.replace("title: add", "title: sum");
        let edited = server.record(&folder, &changed, None).await;
        assert!(matches!(edited, Ok(Recording::Recorded)));
    }

    #[test]
    fn a_check_may_name_the_folder_or_its_idea() {
        // An absolute path on this system.
        let root = if cfg!(windows) { "C:\\" } else { "/" };
        let add = Path::new(root).join("home").join("you").join("add");
        let named = add.to_string_lossy().into_owned();
        let idea = add.join("idea.md");
        let folder = folder_of(&idea.to_string_lossy()).expect("an idea path");
        assert_eq!(folder.to_string(), named);
        let folder = folder_of(&format!(" {named} ")).expect("a folder path");
        assert_eq!(folder.to_string(), named);
        assert!(folder_of("add/idea.md").is_err());
    }

    /// A real run of what a proof found settles it: a run that fails confirms it; one that
    /// passes, though it can't be given the random bytes or clock reading the proof found, leaves
    /// the line saying what the model found there, a trap with its line or output that breaks the
    /// test; and one that passes on the very inputs says the model was wrong.
    #[test]
    fn a_real_run_settles_what_a_proof_found() {
        let trap = "main.wat line 12: it reached memory outside its own";
        let failed = Ok(Some(
            "the program was trapped at main.wat line 12".to_owned(),
        ));
        let (line, wrong) = settled("n = 3", Some(trap), failed, true);
        assert!(wrong, "{line}");
        let (line, wrong) = settled("n = 3", Some(trap), Ok(None), true);
        assert!(!wrong, "{line}");
        assert!(
            line.contains(&format!("main.wat traps ({trap}), which a real run")),
            "{line}"
        );
        let (line, wrong) = settled("n = 3", None, Ok(None), true);
        assert!(!wrong, "{line}");
        assert!(line.contains("main.wat's output breaks"), "{line}");
        let (line, wrong) = settled("n = 3", Some(trap), Ok(None), false);
        assert!(!wrong, "{line}");
        assert!(line.ends_with("and a real run didn't"), "{line}");
    }

    /// What a proof found, kept for main.wat edited without changing its module, says once that
    /// the lines it names may have moved; a record that names no line stays as it was.
    #[test]
    fn a_record_kept_across_an_edit_says_its_lines_may_have_moved() {
        let mut proofs = ProofStamp {
            module_sha256: Sha256Hex::of(b"module"),
            proven: Vec::new(),
            bounded: Vec::new(),
            solvers: "z3 and cvc5".to_owned(),
            wrong: vec![
                ("B1".into(), "trapped at main.wat line 30".into()),
                ("B2".into(), "it printed 3".into()),
            ],
            unproven: vec![("B3".into(), "main.wat line 94: a float".into())],
        };
        lines_may_have_moved(&mut proofs);
        let once = proofs.clone();
        lines_may_have_moved(&mut proofs);
        assert_eq!(proofs, once);
        let moved = " (main.wat's lines as they were when the proof ran: an edit since left the \
                     build the same, but may have moved them)";
        assert_eq!(
            proofs.wrong[0].1,
            format!("trapped at main.wat line 30{moved}")
        );
        assert_eq!(proofs.wrong[1].1, "it printed 3");
        assert_eq!(
            proofs.unproven[0].1,
            format!("main.wat line 94: a float{moved}")
        );
    }

    /// A check that stamps another module of the same main.wat, as after lengua's own build
    /// changed, carries what a proof found wrong over to the new module, and nothing else of the
    /// old proofs; an edited main.wat, old proofs about another module than the old stamp's, or
    /// old proofs that found nothing wrong carry nothing; and proofs about the very module
    /// stamped stay, all of them.
    #[test]
    fn a_wrong_verdict_outlasts_a_change_of_lengua_s_build() {
        let stamp = |code: &[u8], module: &[u8]| CodeStamp {
            sha256: Sha256Hex::of(code),
            module_sha256: Sha256Hex::of(module),
            engine: crate::wasm::engine::ENGINE.to_owned(),
            max_stack_bytes: 16,
            unsafe_blocks: 0,
            features: Vec::new(),
        };
        let proofs = |module: &[u8]| {
            Box::new(ProofStamp {
                module_sha256: Sha256Hex::of(module),
                proven: vec!["B2".into()],
                bounded: vec![lock::Bounded {
                    id: "B3".into(),
                    bytes: 8,
                    of: 16,
                    more: true,
                }],
                solvers: "z3 and cvc5".to_owned(),
                wrong: vec![("B1".into(), "it printed -0".into())],
                unproven: vec![("B4".into(), "a float".into())],
            })
        };
        let before = stamp(b"main.wat", b"old module");
        let rebuilt = stamp(b"main.wat", b"new module");
        let carried = kept_proofs(Some(&before), Some(proofs(b"old module")), &rebuilt)
            .expect("the wrong verdict goes on");
        assert_eq!(carried.module_sha256, rebuilt.module_sha256);
        assert_eq!(carried.wrong, proofs(b"old module").wrong);
        assert_eq!(carried.solvers, "z3 and cvc5");
        assert!(carried.proven.is_empty() && carried.bounded.is_empty());
        assert!(carried.unproven.is_empty());
        let edited = stamp(b"edited main.wat", b"new module");
        assert_eq!(
            kept_proofs(Some(&before), Some(proofs(b"old module")), &edited),
            None
        );
        let other = proofs(b"another module");
        assert_eq!(kept_proofs(Some(&before), Some(other), &rebuilt), None);
        let mut right = proofs(b"old module");
        right.wrong.clear();
        assert_eq!(kept_proofs(Some(&before), Some(right), &rebuilt), None);
        let unstamped = proofs(b"old module");
        assert_eq!(kept_proofs(None, Some(unstamped), &rebuilt), None);
        let same = proofs(b"old module");
        let kept = kept_proofs(Some(&before), Some(same.clone()), &before);
        assert_eq!(kept, Some(same));
    }

    /// What this server's job found wrong refuses a run of the module the job checked, by the
    /// hash the run holds, whatever main.wat is in the folder by then: none, as once it is
    /// removed after the state's read, or another; it refuses no run of another module.
    #[tokio::test]
    async fn a_job_s_finding_refuses_its_module_whatever_main_wat_is_now() {
        let data = tempfile::tempdir().expect("a data folder");
        let program = tempfile::tempdir().expect("a program folder");
        let server = LenguaServer::new(DataDir::new(data.path().to_path_buf()));
        let path = program.path().to_str().expect("UTF-8 path");
        let folder = ProgramFolder::new(path).expect("absolute path");
        let module = Sha256Hex::of(b"the module the job checked");
        let detail = "the proof found inputs the tests didn't try where main.wat is wrong: with \
                      a = 1 and b = -1: it printed \"-0\"";
        let checking = Arc::new(Checking {
            files: [Sha256Hex::of(b"idea.md"), Sha256Hex::of(b"main.wat")],
            module: module.clone(),
            cancel: Arc::default(),
            found: std::sync::Mutex::new(Found {
                proofs: vec![BlockProof {
                    test: "generated B1".to_owned(),
                    progress: None,
                    detail: Some(detail.to_owned()),
                    wrong: true,
                }],
                wrong: true,
                ..Found::default()
            }),
            stage: watch::Sender::new(Stage::Ended),
            green_told: AtomicBool::new(false),
            further: false,
            obligations: Vec::new(),
        });
        server
            .checks
            .lock()
            .await
            .insert(folder.path().to_path_buf(), checking);
        let refused = Some(format!("generated B1: {detail}"));
        assert_eq!(server.proven_wrong(&folder, &module).await, refused);
        std::fs::write(folder.file(CODE_FILE), "(module)\n").expect("write another main.wat");
        assert_eq!(server.proven_wrong(&folder, &module).await, refused);
        let other = Sha256Hex::of(b"another module");
        assert_eq!(server.proven_wrong(&folder, &other).await, None);
    }
}
