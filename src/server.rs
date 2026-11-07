//! The lengua MCP server: tool definitions and server instructions. The code file chooses the
//! path: main.s, checked, built and run natively here, or main.wat, the code of an idea that is
//! an Attested Computation, checked and run as WebAssembly the same way on every system
//! ([`wat`]).

mod wat;

pub use self::wat::{Attestation, attest};

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

use crate::checker::{self, source::Source};
use crate::data_dir::DataDir;
use crate::expr;
use crate::graph;
use crate::hash::Sha256Hex;
use crate::header::{self, Header};
use crate::idea::{self, Idea};
use crate::isa::Isa;
use crate::lock::{self, CodeStamp, IdeaStamp, Key, Lock, ProofStamp};
use crate::platform::{Emulator, Machine, Profile, Target};
use crate::prove;
use crate::runner::{self, End, Job, Limits, Net, Started};
use crate::state::{
    CODE_FILE, IDEA_FILE, LEGACY_IDEA_FILE, LEGACY_TESTS_FILE, ProgramFolder, Snapshot, State,
    WASM_CODE_FILE, stamped_copy,
};
use crate::testdiff;
use crate::testfile::{self, TestSuite};
use crate::testrun;
use crate::toolchain::Toolchain;
use crate::trust;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua builds the programs the user asks for, from commands and \
filters to servers and clients, unless they name another language: you write idea.md, behaviors \
and tests, with the write-idea skill, then compile it into main.s (main.wat for an Attested \
Computation) with the compile skill. After each edit, call check with the folder's absolute \
path, the first time with the user's words as request, and follow its next_step until it says \
Done. Never edit lengua.lock. Run programs only with run.";

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
    /// This server's own binary, which launches programs so their instructions can be counted.
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
    assertions: BTreeMap<i64, usize>,
    /// The code file its assertions' lines are in.
    code: &'static str,
}

/// A program serving in the background, native or WebAssembly.
enum Serving {
    /// A native program, and the build folder of the executable it started from.
    Native(Box<Started>, Arc<tempfile::TempDir>),
    /// A WebAssembly program, in a worker of its own, and its module.
    Wasm(Box<crate::wasm::Running>, Arc<crate::wasm::Program>),
}

impl Serving {
    /// The loopback port it serves on.
    fn port(&self) -> Option<u16> {
        match self {
            Self::Native(started, _) => started.port(),
            Self::Wasm(running, _) => running.port(),
        }
    }

    /// Stops it, and says how it went.
    async fn stop(self) -> std::io::Result<runner::Run> {
        match self {
            Self::Native(started, build) => {
                let stopped = started.stop().await;
                drop(build);
                stopped
            }
            Self::Wasm(running, module) => {
                let stopped = running.stop().await;
                drop(module);
                stopped
            }
        }
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
    /// idea.md and the code file, by lengua's hashes.
    files: [Sha256Hex; 2],
    /// The code file: main.s, or main.wat.
    code: &'static str,
    /// The executable built from main.s, or the module built from main.wat, by hash.
    executable: Sha256Hex,
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
    /// What main.wat's rules leave to its proofs, which its memory line names; main.s's memory
    /// line names none.
    obligations: Vec<checker::wasm::Obligation>,
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
    /// Whether the proof found inputs where the code, main.s or main.wat, is wrong.
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
    /// lengua's line for main.s, written once the job ends without the proof finding it wrong;
    /// main.wat has none.
    header: Option<Header>,
    /// QEMU, when it ran the tests for the target's CPU.
    under: Option<String>,
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

/// What a reply says of memory safety: the rules always; the runs the model's proofs covered,
/// on each of which every access stayed inside the object its pointer was made from.
fn memory_line(proofs: &[BlockProof]) -> String {
    let rules = "the rules (M1 to M8) hold every address they can follow to its owner";
    if let Some(detail) = proofs
        .iter()
        .find_map(|proof| proof.detail.as_deref())
        .filter(|detail| detail.starts_with(NOT_MODELLED))
    {
        return format!(
            "{rules}; the rest is tested, not proven: {}",
            &detail[NOT_MODELLED.len()..]
        );
    }
    let proven: Vec<&str> = proofs
        .iter()
        .filter(|proof| {
            proof
                .detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("proven for every input"))
        })
        .map(|proof| proof.test.trim_start_matches("generated "))
        .collect();
    if let Some((last, rest)) = proven.split_last() {
        let tests = if rest.is_empty() {
            (*last).to_owned()
        } else {
            format!("{} and {last}", rest.join(", "))
        };
        return format!(
            "{rules}; on every run the proofs of {tests} cover, every access also stays inside the \
             object its pointer was made from (or, where a run loses a pointer's origin, as through \
             a vector register, the object it points into), with no return address overwritten \
             and no stack read before it's written"
        );
    }
    if proofs.iter().any(|proof| proof.detail.is_none()) {
        return format!("{rules}; the proofs check the rest on every run they cover");
    }
    format!(
        "{rules}; accesses through pointers they can't follow, such as ones loaded from memory, \
         are tested, not proven"
    )
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
    /// Where the program stands: blocked, write_idea, idea_problems, needs_request, write_code,
    /// code_problems, testing, tests_failed, proving or done.
    pub state: String,
    /// Every problem to fix before going on, each saying where it is and how to fix it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The machine the code runs on, while it isn't written yet: for main.s, its cores, which an
    /// idea's threads may use, and the CPU extensions main.s may use; for main.wat, the engine
    /// that compiles it for this CPU, and the WebAssembly features main.wat may use.
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
    /// What each test cost, once done: instructions a run over all its runs, and from its
    /// largest input run alone, CPU cycles and times (lower is faster).
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
    /// `stdin`: main.s reads it itself however large it is; main.wat gets it whole, at most 16 MiB.
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
    /// Instructions executed in the program's own code, when the CPU's counters are available.
    pub instructions: Option<u64>,
    /// CPU cycles in the program's own code, when available.
    pub cycles: Option<u64>,
    /// CPU time in microseconds over all its threads, including system calls, when available.
    pub cpu_time_us: Option<u64>,
    /// Time on the clock in microseconds, from start to end.
    pub wall_time_us: u64,
    /// Where a server is taking connections, while it runs in the background.
    pub serving: Option<String>,
    /// What held the run, when QEMU's user mode ran it for the target's CPU.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emulated: Option<String>,
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
/// `code`, main.s or main.wat.
fn ended_report(
    result: &runner::Run,
    assertions: &BTreeMap<i64, usize>,
    limits: Limits,
    code: &str,
) -> RunReport {
    let mut ended = testrun::explain_end_in(result, assertions, limits, code);
    // A broker that stopped serving the program left it without names, clock or random bytes: the
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
        instructions: result.instructions,
        cycles: result.cycles,
        cpu_time_us: result.cpu_time_ns.map(|nanoseconds| nanoseconds / 1000),
        wall_time_us: result.wall_time_ns / 1000,
        serving: None,
        emulated: None,
        connections: result
            .connections
            .iter()
            .map(ConnectionReport::of)
            .collect(),
    }
}

impl LenguaServer {
    /// Starts a server and leaves it serving, at most one per program folder, until `run` stops
    /// it, ten minutes pass, or the session ends.
    async fn serve_in_background(
        &self,
        folder: &ProgramFolder,
        job: Job<'_>,
        build: Arc<tempfile::TempDir>,
        assertions: BTreeMap<i64, usize>,
    ) -> Result<RunReport, String> {
        let mut background = self.background.lock().await;
        already_serving(&background, folder)?;
        let started = runner::start(&self.launcher, job, background_limits())
            .await
            .map_err(|error| format!("couldn't start the server: {error}"))?;
        let connections = started
            .connections()
            .iter()
            .map(ConnectionReport::of)
            .collect();
        let server = Serving::Native(Box::new(started), build);
        Ok(self.keep_serving(
            &mut background,
            folder,
            server,
            (assertions, CODE_FILE),
            connections,
        ))
    }

    /// Keeps `server`, which `run` started for `folder` and which serves now, among the
    /// background ones until `run` stops it, ten minutes pass, or the session ends; `run`'s
    /// reply says where it serves and what became of its connections. `assertions` are its
    /// assertions' lines in its code file.
    fn keep_serving(
        &self,
        background: &mut BTreeMap<PathBuf, Background>,
        folder: &ProgramFolder,
        server: Serving,
        assertions: (BTreeMap<i64, usize>, &'static str),
        connections: Vec<ConnectionReport>,
    ) -> RunReport {
        static STARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let address = format!("127.0.0.1:{}", server.port().unwrap_or(0));
        let id = STARTS.fetch_add(1, Ordering::Relaxed);
        let (assertions, code) = assertions;
        background.insert(
            folder.path().to_path_buf(),
            Background {
                server,
                id,
                assertions,
                code,
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
            instructions: None,
            cycles: None,
            cpu_time_us: None,
            wall_time_us: 0,
            serving: Some(address),
            emulated: None,
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
            it; checks the code, main.s or for an Attested Computation main.wat, against \
            lengua's rules; runs every test with fresh random inputs; and proves the program for \
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
            when lengua.lock shows that idea.md and its code, main.s or main.wat, are exactly the \
            versions that were recorded and checked, and a fresh build of the code is byte for \
            byte the executable or module check tested. The program gets an empty environment, \
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
            stdin_file: a file in folder to read as standard input instead of stdin, however \
            large for main.s and up to 16 MiB for main.wat; timeout_seconds: up to 40 (10 by \
            default); env: environment variables, for a \
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
            return Ok(Json(ended_report(
                &result,
                &entry.assertions,
                limits,
                entry.code,
            )));
        }
        // An Attested Computation's code is main.wat, which runs as WebAssembly.
        if wat::attested_folder(&folder)? {
            return self.run_wasm(&folder, &params).await.map(Json);
        }
        let Runnable {
            built,
            source,
            idea,
            syscalls,
            net,
            channel,
            emulator,
        } = runnable(&self.data_dir, &folder, &params.env).await?;
        if let Some(wrong) = self.proven_wrong(&folder).await {
            return Err(format!(
                "The proof found inputs where {CODE_FILE} is wrong, so it won't run: {wrong}. \
                 Call check with path {folder} to see them, then fix {CODE_FILE}."
            ));
        }
        let workspace = match params.folder.as_deref() {
            None => None,
            Some(path) => Some(working_folder(path)?),
        };
        let seconds = params.timeout_seconds.unwrap_or(10);
        if !(1..=RUN_SECONDS).contains(&seconds) {
            return Err(format!("timeout_seconds goes from 1 to {RUN_SECONDS}"));
        }
        let limits = Limits {
            timeout: Duration::from_secs(seconds),
            max_file_size: None,
            ..Limits::default()
        };
        // Standard input from a file in the program's folder, which it reads itself.
        let stdin_file = match (params.stdin_file.as_deref(), workspace.as_deref()) {
            (None, _) => None,
            (Some(_), None) => {
                return Err("stdin_file names a file in folder; give folder too".into());
            }
            (Some(name), Some(folder)) => {
                let path = folder.join(name);
                let inside = path
                    .canonicalize()
                    .map_err(|error| format!("stdin_file {name}: {error}"))?;
                if !inside.starts_with(folder) || !inside.is_file() {
                    return Err(format!("stdin_file {name} must be a file inside folder"));
                }
                Some(inside)
            }
        };
        let job = Job {
            folder: workspace.as_deref(),
            net: Some(&net),
            stdin_file: stdin_file.as_deref(),
            env: &params.env,
            emulated: emulator.is_some(),
            channel,
            ..Job::new(
                &built.executable,
                &params.args,
                params.stdin.as_bytes(),
                &syscalls,
            )
        };
        let assertions = testrun::assertion_lines(&source);
        if idea.network.listens.is_some() {
            return self
                .serve_in_background(&folder, job, Arc::clone(&built.folder), assertions)
                .await
                .map(Json);
        }
        let result = runner::run(&self.launcher, job, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        let mut report = ended_report(&result, &assertions, limits, CODE_FILE);
        report.emulated = emulator.map(|emulator| {
            format!(
                "ran under {}: Landlock held it to its files and network as on any run, but the \
                 kernel couldn't hold its system calls to the idea, which only lengua's rules \
                 did, and nothing was measured",
                emulator.name
            )
        });
        Ok(Json(report))
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

/// Everything a checking job needs about the code it checks.
struct Code {
    idea: Idea,
    suite: TestSuite,
    source: Source,
    built: Build,
    report: checker::Report,
    profile: Profile,
    /// What lengua's broker does for the program, when it serves it: the program links the
    /// channel's code, and its idea says what it may ask.
    channel: Option<crate::broker::Permit>,
    /// QEMU, when this computer runs the target's programs only through it.
    emulator: Option<Emulator>,
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
        // as they are, whatever stage this check stops at and whichever path it takes.
        let idea_text = match read_optional(folder, IDEA_FILE)? {
            Some(text) => Some(self.keep_verified(folder, text).await),
            None => None,
        };
        // The idea's type chooses the path. An Attested Computation is compiled to main.wat,
        // which needs no LLVM, no Linux kernel and no CPU target: it is checked the same way on
        // every system.
        if let Some(text) = idea_text.as_deref().filter(|text| wat::attested(text)) {
            return self
                .check_wasm(folder, params, events, (began, &machine), text)
                .await;
        }
        let toolchain = Toolchain::discover().await;
        if let Some(blocked) = machine_blocker(&machine, toolchain.as_ref().err()) {
            // A new folder where lengua builds no main.s starts as a WebAssembly program.
            if idea_text.is_none() && read_optional(folder, LEGACY_IDEA_FILE)?.is_none() {
                return Ok(CheckReport {
                    machine: Some(wat::machine_line(&machine)),
                    ..CheckReport::new("write_idea", State::NoIdea.next_step(folder))
                });
            }
            return Ok(CheckReport::new("blocked", blocked));
        }
        let target = session_target(&machine)?;
        let profile = target.profile;
        let machine_line = || machine_line(&machine, &target);

        // idea.md, tests included: recorded with the user's words once it parses and its tests
        // check.
        let Some(idea_text) = idea_text else {
            if read_optional(folder, LEGACY_IDEA_FILE)?.is_some() {
                return Ok(CheckReport {
                    machine: Some(machine_line()),
                    ..CheckReport::new(
                        "write_idea",
                        format!(
                            "Next: rename {folder}/{LEGACY_IDEA_FILE} to {IDEA_FILE}, the idea \
                             file's name now (what it says stays as it is), then call check with \
                             path {folder}."
                        ),
                    )
                });
            }
            return Ok(CheckReport {
                machine: Some(machine_line()),
                ..CheckReport::new("write_idea", State::NoIdea.next_step(folder))
            });
        };
        let (idea, suite) = match self
            .idea_stage(folder, params, events, &idea_text, CODE_FILE)
            .await?
        {
            IdeaStage::Recorded(recorded) => *recorded,
            IdeaStage::Stops(report) => return Ok(*report),
        };
        let idea_hash = trust::idea_hash(idea_text.as_bytes());
        // A native idea's code is main.s: main.wat beside it is another path's.
        if let Some(report) = wat::code_file_problem(folder, false) {
            return Ok(report);
        }

        // main.s, checked against the rules, then tested and proven in the background.
        let Some(code) = read_optional(folder, CODE_FILE)? else {
            return Ok(CheckReport {
                machine: Some(machine_line()),
                ..CheckReport::new("write_code", State::CodeMissing.next_step(folder))
            });
        };
        // main.s's first line says which CPU's code it is: it never chooses the target.
        if let Some(other) = code_for_another_cpu(&code, &target) {
            return Ok(CheckReport::problems(
                "code_problems",
                vec![other],
                format!(
                    "Next: compile {folder}/{IDEA_FILE} into {folder}/{CODE_FILE} for {} with \
                     the compile skill, then call check with path {folder}.",
                    match target.emulator {
                        None => "this computer".to_owned(),
                        Some(_) => format!("{}, the target LENGUA_TARGET names", profile.name()),
                    }
                ),
            ));
        }
        // Code compiled for another computer is compiled again for this one: the target's CPU,
        // under QEMU the emulated one's.
        let features = target.features(&machine);
        if let Some(elsewhere) =
            Header::of(&code).and_then(|header| header.elsewhere(profile, &features))
        {
            return Ok(CheckReport::problems(
                "code_problems",
                vec![elsewhere],
                format!(
                    "Next: compile {folder}/{IDEA_FILE} into {folder}/{CODE_FILE} for this \
                     computer with the compile skill, then call check with path {folder}."
                ),
            ));
        }
        let files = [idea_hash, header::code_hash(code.as_bytes())];
        let checking = match self
            .current_checking(folder, &files, params.prove_further)
            .await
        {
            Some(checking) => checking,
            None => {
                let fix = || {
                    format!(
                        "Next: fix {folder}/{CODE_FILE} with the compile skill (the full rules \
                         are in its references/power-of-10.md), then call check with path \
                         {folder}."
                    )
                };
                let (source, layout) = checker::check_layout(&code);
                if !layout.passed() {
                    return Ok(CheckReport::problems(
                        "code_problems",
                        violation_lines(&layout),
                        fix(),
                    ));
                }
                let from_parts = parts_problems(&self.data_dir, folder, &idea, &source);
                if !from_parts.is_empty() {
                    return Ok(CheckReport::problems("code_problems", from_parts, fix()));
                }
                let built = match build(&code, profile).await {
                    Ok(built) => built,
                    Err(error) => {
                        return Ok(CheckReport::problems("code_problems", vec![error], fix()));
                    }
                };
                let lengua = checker::lengua_source(profile, &code);
                let report = checker::check_program(
                    &source,
                    &built.bytes,
                    checker::Context {
                        ops: &idea.ops,
                        threads: idea.threads,
                        lengua: lengua.as_ref(),
                        features: target.features(&machine),
                        profile,
                    },
                );
                if !report.passed() {
                    return Ok(CheckReport::problems(
                        "code_problems",
                        violation_lines(&report),
                        fix(),
                    ));
                }
                let channel = crate::edge::uses_channel(profile, &code)
                    .then(|| crate::broker::Permit::of(&idea.syscalls));
                self.start_checking(
                    folder,
                    files,
                    Code {
                        idea,
                        suite,
                        source,
                        built,
                        report,
                        profile,
                        channel,
                        emulator: target.emulator.clone(),
                    },
                    params.prove_further,
                )
                .await
            }
        };
        wait_for(&checking, began + check_wait()).await;
        if *checking.stage.borrow() == Stage::Ended {
            self.write_header(folder, &checking).await;
        }
        let report = self.checking_report(folder, &checking);
        let proven_wrong = checking.found.lock().is_ok_and(|found| found.wrong);
        if report.state == "done" {
            events.push(graph::Event::Done);
        } else if proven_wrong {
            events.push(graph::Event::ProvenWrong);
        }
        Ok(report)
    }

    /// `check`'s idea stage, the same for both code files: idea.md, tests included, recorded
    /// with the user's words once it parses, its tests check and its parts are there, each a
    /// program with its own idea.md and `code`, main.s or main.wat, of which only main.wat's
    /// idea has no threads.
    async fn idea_stage(
        &self,
        folder: &ProgramFolder,
        params: &CheckParams,
        events: &mut Vec<graph::Event>,
        idea_text: &str,
        code: &str,
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
                        let threads = (code == WASM_CODE_FILE)
                            .then(|| wat::threads_problem(idea_text, &idea))
                            .flatten();
                        let missing = missing_parts(folder, &idea, code);
                        // A cycle of `- Builds on:` links blocks the programs in it, and only
                        // them.
                        if let Some(threads) = threads {
                            Err(vec![threads])
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

    /// What the proof found wrong in the code now in `folder`, when its job confirmed a
    /// counterexample: the tests passed, but these inputs, run for real, show the code, main.s or
    /// main.wat, is wrong.
    async fn proven_wrong(&self, folder: &ProgramFolder) -> Option<String> {
        let checking = self.checks.lock().await.get(folder.path()).cloned()?;
        let code = std::fs::read(folder.file(checking.code)).ok()?;
        if crate::state::code_hash(checking.code, &code) != checking.files[1] {
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
                    code.sha256 == files[1] && code.executable_sha256 == checking.executable
                });
            if !recorded {
                return None;
            }
            let short = lock
                .as_ref()
                .and_then(|lock| lock.proofs.as_ref())
                .is_some_and(|proofs| {
                    proofs.executable_sha256 == checking.executable
                        && proofs.bounded.iter().any(|bounded| bounded.more)
                });
            if further && ended && short {
                return None;
            }
        }
        Some(Arc::clone(checking))
    }

    /// Starts testing and proving `code` in the background, stopping any job on older files;
    /// with `further`, the proof goes on from where lengua.lock says it stopped short.
    async fn start_checking(
        &self,
        folder: &ProgramFolder,
        files: [Sha256Hex; 2],
        code: Code,
        further: bool,
    ) -> Arc<Checking> {
        let proofs = code
            .suite
            .generated
            .iter()
            .map(|block| BlockProof {
                test: format!("generated {}", block.id),
                progress: None,
                detail: None,
                wrong: false,
            })
            .collect();
        let checking = Arc::new(Checking {
            files,
            code: CODE_FILE,
            executable: Sha256Hex::of(&code.built.bytes),
            cancel: Arc::default(),
            found: std::sync::Mutex::new(Found {
                proofs,
                ..Found::default()
            }),
            stage: watch::Sender::new(Stage::Testing),
            green_told: AtomicBool::new(false),
            further,
            obligations: Vec::new(),
        });
        if let Some(old) = self
            .checks
            .lock()
            .await
            .insert(folder.path().to_path_buf(), Arc::clone(&checking))
        {
            old.cancel.store(true, Ordering::Relaxed);
        }
        let (server, folder, job) = (self.clone(), folder.clone(), Arc::clone(&checking));
        tokio::spawn(async move {
            server.test_and_prove(&folder, &job, code).await;
            job.stage.send_replace(Stage::Ended);
        });
        checking
    }

    /// The job itself: every test, then, once they all pass, the proof.
    async fn test_and_prove(&self, folder: &ProgramFolder, checking: &Checking, code: Code) {
        let fail = |failures: Vec<String>| {
            if let Ok(mut found) = checking.found.lock() {
                found.failures = failures;
            }
        };
        let [idea_hash, code_hash] = checking.files.clone();
        let seed = match random_seed() {
            Ok(seed) => seed,
            Err(error) => return fail(vec![error]),
        };
        let syscalls = code
            .profile
            .allowed_syscalls(&code.idea.syscalls, code.idea.threads.is_some());
        let outcome = match testrun::run_tests(
            &code.suite,
            &self.launcher,
            &code.built.executable,
            &code.source,
            seed,
            &syscalls,
            &code.idea.network,
            testrun::workers(code.idea.threads),
            testrun::Setting {
                emulated: code.emulator.is_some(),
                channel: code.channel,
            },
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => return fail(vec![format!("couldn't run the program: {error}")]),
        };
        if !outcome.passed() {
            return fail(outcome.failures);
        }
        let facts = code.report.facts.clone().unwrap_or_default();
        let executable = checking.executable.clone();
        let stamp = CodeStamp {
            sha256: code_hash,
            executable_sha256: executable.clone(),
            toolchain: code.built.toolchain.clone(),
            max_stack_bytes: facts.max_stack_bytes,
            unsafe_blocks: u32::try_from(code.report.unsafe_blocks.len()).unwrap_or(u32::MAX),
            target: code.profile.name().to_owned(),
            features: facts
                .features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
            emulator: code.emulator.as_ref().map(|emulator| emulator.name.clone()),
        };
        let header = Header {
            idea: idea_hash.to_string(),
            target: stamp.target.clone(),
            features: stamp.features.clone(),
        };
        if let Err(error) = self.stamp_code(folder, &idea_hash, stamp).await {
            return fail(vec![error]);
        }
        if let Ok(mut found) = checking.found.lock() {
            found.passed = Some(Passed {
                runs: outcome.runs,
                every_value: code
                    .suite
                    .generated
                    .iter()
                    .filter_map(|block| {
                        block
                            .every_value_text()
                            .map(|text| format!("generated {} tried {text}", block.id))
                    })
                    .collect(),
                performance: outcome.costs.iter().map(cost_line).collect(),
                shown: outcome.shown,
                header: Some(header),
                under: code.emulator.as_ref().map(|emulator| emulator.name.clone()),
            });
        }
        checking.stage.send_replace(Stage::Proving(Instant::now()));
        if !modelled(code.profile) {
            if let Ok(mut found) = checking.found.lock() {
                for proof in &mut found.proofs {
                    proof.detail = Some(not_modelled(code.profile));
                }
            }
        } else {
            let prover = Prover::Native(&code, &syscalls);
            match self.recorded_proofs(folder, checking, &code.suite, &executable) {
                None => {
                    self.prove_all(folder, checking, prover, seed, &executable, &[])
                        .await;
                }
                Some(further) if !further.is_empty() => {
                    self.prove_all(folder, checking, prover, seed, &executable, &further)
                        .await;
                }
                Some(_) => {}
            }
        }
        if !checking.cancel.load(Ordering::Relaxed) {
            self.write_header(folder, checking).await;
        }
    }

    /// Writes lengua's line into main.s once `checking` has ended with its tests passed and no
    /// input the proof found wrong, provided main.s is still the code it checked and its line
    /// says something else. The code's hash leaves the line out, so the lock still holds. For
    /// main.wat, which has no such line, lengua's entry goes in idea.md's `verified` instead, and
    /// only for a job whose tests all passed, which stamped the code, and whose proofs found no
    /// input wrong.
    async fn write_header(&self, folder: &ProgramFolder, checking: &Checking) {
        if checking.code == WASM_CODE_FILE {
            let passed = checking.found.lock().is_ok_and(|found| {
                found.passed.is_some() && found.failures.is_empty() && !found.wrong
            });
            if passed {
                self.write_verified(folder, checking).await;
            }
            return;
        }
        let header = match checking.found.lock() {
            Ok(found) if !found.wrong => found
                .passed
                .as_ref()
                .and_then(|passed| passed.header.clone()),
            _ => None,
        };
        let Some(header) = header else {
            return;
        };
        let checked = &checking.files[1];
        let _writing = self.lock_writes.lock().await;
        let path = folder.file(CODE_FILE);
        let Ok(code) = std::fs::read_to_string(&path) else {
            return;
        };
        if header::code_hash(code.as_bytes()) != *checked
            || Header::of(&code).as_ref() == Some(&header)
        {
            return;
        }
        // Whole or not at all, with main.s's own permissions.
        let written = (|| -> std::io::Result<()> {
            let permissions = std::fs::metadata(&path)?.permissions();
            let mut file = tempfile::NamedTempFile::new_in(folder.path())?;
            std::io::Write::write_all(&mut file, header::with(&code, &header).as_bytes())?;
            file.as_file().set_permissions(permissions)?;
            file.as_file().sync_all()?;
            file.persist(&path).map_err(|error| error.error)?;
            Ok(())
        })();
        if let Err(error) = written {
            eprintln!(
                "lengua: couldn't write the header of {}: {error}",
                path.display()
            );
        }
    }

    /// Takes the proofs lengua.lock records about this very build, after a restart lost the job
    /// that made them, or when the job is to prove on. Returns `None` when there are none, and
    /// otherwise the blocks to prove on, each with the length to start at.
    fn recorded_proofs(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        suite: &TestSuite,
        executable: &Sha256Hex,
    ) -> Option<Vec<(usize, usize)>> {
        let stamp = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.proofs)
            .filter(|proofs| proofs.executable_sha256 == *executable)?;
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
            } else {
                "tested, not proven".to_owned()
            });
        }
        found.wrong |= any_wrong;
        Some(further)
    }

    /// Proves each generated test in turn, each with its share of [`PROOF_TIME`], and records
    /// what was proven: main.s's machine code, or main.wat's module, as `prover` holds it, whose
    /// tests just passed every run drawn from `seed`. With `further`, proves only those blocks,
    /// each from its length on, and keeps what lengua.lock records of the others.
    async fn prove_all(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        prover: Prover<'_>,
        seed: u64,
        executable: &Sha256Hex,
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
        let suite = prover.suite();
        let model = match prover.model() {
            Ok(model) => model,
            Err(error) => {
                for index in 0..suite.generated.len() {
                    set(index, format!("tested, not proven: {error}"), false);
                }
                return;
            }
        };
        let idea = prover.idea();
        let deadline = Instant::now() + proof_time();
        // The ids proven, those found wrong, how many the solvers proved, and why any rested on
        // one solver.
        let mut proven_ids = Vec::new();
        let mut bounded_ids: Vec<(usize, lock::Bounded)> = Vec::new();
        let mut wrong_ids: Vec<(String, String)> = Vec::new();
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
                .filter(|proofs| proofs.executable_sha256 == *executable)
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
            let kernel_values = prover.kernel_values(block);
            if let Some(every) = block
                .every_value_text()
                .filter(|_| !kernel_values && !block.uses_now())
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
                            checking.code,
                        ),
                        false,
                    )
                }
                Ok(prove::Verdict::Counterexample(values)) => match prover {
                    Prover::Native(code, syscalls) => {
                        self.replay(code, syscalls, block, values, kernel_values)
                            .await
                    }
                    Prover::Wasm(code) => {
                        let pool = self.pool();
                        wat::replay(&pool, &code.program, idea, block, values, kernel_values)
                            .await
                    }
                },
            };
            if wrong {
                // At once, so the finding outlasts this job: run refuses the build from now on.
                wrong_ids.push((block.id.to_string(), detail.clone()));
                let ids =
                    |list: &[(usize, String)]| list.iter().map(|(_, id)| id.clone()).collect();
                let bounds = bounded_ids.iter().map(|(_, bound)| bound.clone()).collect();
                self.stamp_proofs(
                    folder,
                    executable,
                    ids(&proven_ids),
                    bounds,
                    wrong_ids.clone(),
                    "z3 and cvc5".to_owned(),
                )
                .await;
                // main.wat's entry in idea.md's `verified` goes at once too, before any reply
                // says the build is wrong; the lock now keeps another from going in.
                if checking.code == WASM_CODE_FILE {
                    self.unverify(folder, checking).await;
                }
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
        let proven_ids = proven_ids.into_iter().map(|(_, id)| id).collect();
        let bounded_ids = bounded_ids.into_iter().map(|(_, bound)| bound).collect();
        self.stamp_proofs(
            folder,
            executable,
            proven_ids,
            bounded_ids,
            wrong_ids,
            solvers,
        )
        .await;
        release(last_line);
    }

    /// Runs inputs the proof found in main.s for real, so a counterexample is reported only when
    /// the program really is wrong on it. Returns the detail and whether main.s is wrong.
    async fn replay(
        &self,
        code: &Code,
        syscalls: &[crate::platform::Allowed],
        block: &testfile::Generated,
        values: BTreeMap<String, expr::Value>,
        kernel_values: bool,
    ) -> (String, bool) {
        let Witness {
            inputs,
            delivery,
            broken,
            described,
        } = Witness::of(block, values);
        // A real run can pass without showing it, so the proof's model is the witness.
        if let Some(problem) = broken {
            return (
                format!(
                    "the proof found inputs where main.s breaks the memory rules: with \
                     {described}, {problem}"
                ),
                true,
            );
        }
        let case = match block.case(inputs, crate::expect::now()) {
            Ok(case) => case,
            Err((_, field, error)) => {
                return (
                    format!("tested, not proven: with {described}, {field} {error}"),
                    false,
                );
            }
        };
        let ran = testrun::check_case(
            &self.launcher,
            &code.built.executable,
            &code.source,
            &case,
            syscalls,
            &code.idea.network,
            delivery,
            code.channel,
        )
        .await;
        settled(CODE_FILE, &described, None, ran, kernel_values)
    }

    /// What `check` replies about a checking job.
    fn checking_report(&self, folder: &ProgramFolder, checking: &Checking) -> CheckReport {
        let stage = *checking.stage.borrow();
        let code = checking.code;
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
        if let Some(emulator) = &passed.under {
            tests[0].push_str(&format!(" under {emulator}"));
            tests.push(format!(
                "under {emulator}, Landlock held each run to its files and network as on any \
                 run, but the kernel couldn't hold its system calls to the idea, which only \
                 lengua's rules did, and nothing was measured"
            ));
        }
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
                    memory: Some(memory_of(checking, &found.proofs)),
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
        // check writes lengua's entry in main.wat's idea as it replies done.
        let verified = if code == WASM_CODE_FILE {
            VERIFIED_NOTE
        } else {
            ""
        };
        CheckReport {
            tests,
            proofs,
            memory: Some(memory_of(checking, &found.proofs)),
            example: passed.shown.clone(),
            performance: passed.performance.clone(),
            changes,
            shell: Some(self.shell_command(folder)),
            ..CheckReport::new(
                "done",
                format!(
                    "Done: tell the user where the program is, what its idea promises, that it \
                     passed every test, and what's proven, memory \
                     included.{verified}{running}{shown} run runs it on their own inputs here; \
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
    /// being checked.
    async fn stamp_code(
        &self,
        folder: &ProgramFolder,
        idea: &Sha256Hex,
        code: CodeStamp,
    ) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        // The stamp is of main.s, or of main.wat for WebAssembly.
        let file = if code.is_wasm() {
            WASM_CODE_FILE
        } else {
            CODE_FILE
        };
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
            && crate::state::code_hash(file, current.as_bytes()) == code.sha256;
        if !unchanged {
            return Err(changed());
        }
        // Proofs about this very build still hold; about any other, they're gone.
        let proofs = lock
            .proofs
            .filter(|proofs| proofs.executable_sha256 == code.executable_sha256);
        let lock = Lock {
            code: Some(code),
            proofs,
            ..lock
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))
    }

    /// Records in the lock what the proof proved about the build `executable`, provided the lock
    /// still records that build as the checked code. Returns whether it did.
    async fn stamp_proofs(
        &self,
        folder: &ProgramFolder,
        executable: &Sha256Hex,
        proven: Vec<String>,
        bounded: Vec<lock::Bounded>,
        wrong: Vec<(String, String)>,
        solvers: String,
    ) -> bool {
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
            .is_none_or(|code| code.executable_sha256 != *executable)
        {
            return false;
        }
        let lock = Lock {
            proofs: Some(Box::new(ProofStamp {
                executable_sha256: executable.clone(),
                proven,
                bounded,
                solvers,
                wrong,
            })),
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

/// What a reply says of memory safety for `checking`'s code: main.s's line, or main.wat's, which
/// names what its rules leave to the proofs.
fn memory_of(checking: &Checking, proofs: &[BlockProof]) -> String {
    if checking.code == WASM_CODE_FILE {
        wat::memory_line(&checking.obligations, proofs)
    } else {
        memory_line(proofs)
    }
}

/// The build whose generated tests a job proves: main.s's, with the system calls its idea
/// allows, which a real run of what a proof finds is held to, or main.wat's.
#[derive(Clone, Copy)]
enum Prover<'a> {
    /// main.s's build, and the system calls its idea allows.
    Native(&'a Code, &'a [crate::platform::Allowed]),
    /// main.wat's build.
    Wasm(&'a wat::WasmCode),
}

impl<'a> Prover<'a> {
    /// The idea the code was checked against.
    fn idea(self) -> &'a Idea {
        match self {
            Self::Native(code, _) => &code.idea,
            Self::Wasm(code) => &code.idea,
        }
    }

    /// The idea's tests.
    fn suite(self) -> &'a TestSuite {
        match self {
            Self::Native(code, _) => &code.suite,
            Self::Wasm(code) => &code.suite,
        }
    }

    /// Whether a run of `block` reads values none of its variables fixes: random bytes or clock
    /// readings the test doesn't supply, which the kernel gives main.s and lengua's host gives
    /// main.wat. Then every value of the variables isn't every input, and a real run can't
    /// replay what a proof found.
    fn kernel_values(self, block: &testfile::Generated) -> bool {
        match self {
            Self::Native(code, _) => {
                let allows = |call: idea::Syscall| code.idea.syscalls.contains(&call);
                (allows(idea::Syscall::Getrandom) && block.random.is_none())
                    || (allows(idea::Syscall::ClockGettime) && block.clock.is_empty())
            }
            Self::Wasm(code) => wat::host_values(&code.idea, block),
        }
    }

    /// What the proofs run on: main.s's executable decoded, with what the rules found of it, or
    /// main.wat's module, with what its idea lets it reach.
    ///
    /// # Errors
    ///
    /// Why main.s's executable can't be decoded.
    fn model(self) -> Result<Model, String> {
        match self {
            Self::Native(code, _) => {
                let mut decoded = checker::decode::read(&code.built.bytes)?;
                // Which symbol each [rip + name] names, so proofs hold a pointer just past one
                // symbol's end to that symbol, not to the one starting there; and each checked
                // jump table's cases, so proofs follow every one.
                if let Some(facts) = &code.report.facts {
                    decoded.named.clone_from(&facts.named);
                    decoded.tables.clone_from(&facts.tables);
                    decoded.sve_bytes = facts.sve_bytes;
                }
                Ok(Model::Native(decoded))
            }
            Self::Wasm(code) => Ok(Model::Wasm(
                Arc::clone(&code.program),
                crate::wasm::Policy::of(&code.idea.uses, &code.idea.network),
            )),
        }
    }
}

/// What a proof runs on: main.s's executable, decoded, or main.wat's module, with what its idea
/// lets it reach.
#[derive(Clone)]
enum Model {
    /// main.s's executable, with what the rules found of its symbols and jump tables.
    Native(checker::decode::Machine),
    /// main.wat's module, and what its idea lets it reach.
    Wasm(Arc<crate::wasm::Program>, crate::wasm::Policy),
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
        tokio::task::spawn_blocking(move || match &model {
            Self::Native(machine) => {
                prove::agrees_until(machine, &samples, listens, budget.deadline)
                    .map(|found| found.unwrap_or_else(|| prove::prove(machine, &block, &budget)))
            }
            Self::Wasm(program, policy) => {
                let built = program.built();
                let found = prove::agrees_wasm(built, *policy, &samples, listens, budget.deadline);
                found.map(|found| {
                    found.unwrap_or_else(|| prove::prove_wasm(built, *policy, &block, &budget))
                })
            }
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
    /// What the model said a run broke: a memory rule of main.s's, or a trap of main.wat's.
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

/// A proof's line for what a real run of the inputs it found came to, `ran`, in the words of
/// `file`, the code: whether the code is wrong on them, `described`, and whether it is. Where the
/// run passed, the model was wrong about them, and about what it said the run broke, `broken`.
fn settled(
    file: &str,
    described: &str,
    broken: Option<&str>,
    ran: std::io::Result<Option<String>>,
    kernel_values: bool,
) -> (String, bool) {
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
        // what the proof found.
        Ok(None) if kernel_values => (
            format!(
                "tested, not proven: with {described}, the prover found random bytes or a clock \
                 reading for which {file}'s output breaks the test, which a real run can't \
                 replay; check how {file} turns them into output"
            ),
            false,
        ),
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

/// What a proven block's proof covered, in a line, for `code`, main.s or main.wat, whose paths
/// it followed through the machine code or the WebAssembly.
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
    code: &str,
) -> String {
    let through = if code == WASM_CODE_FILE {
        "the WebAssembly"
    } else {
        "the machine code"
    };
    let mut detail = format!(
        "proven for every input {} ({}, {} through {through}), {}",
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

/// A test's cost in one line: medians only.
fn cost_line(cost: &testrun::Cost) -> String {
    cost_line_in(cost, "instructions")
}

/// [`cost_line`], counting what a run executes in `unit`: a native program's instructions, or a
/// WebAssembly program's operations.
fn cost_line_in(cost: &testrun::Cost, unit: &str) -> String {
    let mut line = format!("{}: {}", cost.test, counted(cost.runs, "run"));
    if let Some(instructions) = cost.instructions {
        let _ = write!(line, ", {} {unit} each", instructions.median);
    }
    let mut times = Vec::new();
    if let Some(cycles) = cost.cycles {
        times.push(format!("{} cycles", cycles.median));
    }
    if let Some(cpu) = cost.cpu_time_us {
        times.push(format!("{} µs of CPU", cpu.median));
    }
    if let Some(wall) = cost.wall_time_us {
        times.push(format!("{} µs on the clock", wall.median));
    }
    if !times.is_empty() {
        let _ = write!(line, "; {}: median {}", cost.timed, times.join(", "));
    }
    line
}

/// The machine in one line: the target, its cores, and the CPU extensions main.s may use.
fn machine_line(machine: &Machine, target: &Target) -> String {
    let available = target.features(machine);
    let features = match serde_json::to_value(available) {
        Ok(serde_json::Value::Object(features)) if target.profile.isa() == Isa::X86_64 => features
            .into_iter()
            .filter(|(name, present)| {
                *present == serde_json::Value::Bool(true)
                    && !["crc", "pmull", "sve"].contains(&name.as_str())
            })
            .map(|(name, _)| name.replace("sse4_", "sse4."))
            .collect::<Vec<_>>(),
        _ => [
            ("crc", available.crc),
            ("pmull", available.pmull),
            ("aes", available.aes),
            ("sha", available.sha),
        ]
        .into_iter()
        .filter(|(_, present)| *present)
        .map(|(name, _)| name.to_owned())
        .chain((available.sve > 0).then(|| format!("sve (vectors of {} bytes)", available.sve)))
        .collect(),
    };
    format!(
        "{}, {}; main.s may use the base instructions{}",
        target.describe(machine),
        counted(machine.cores, "core"),
        if features.is_empty() {
            String::new()
        } else {
            format!(" and {}", features.join(", "))
        }
    )
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

fn read_code(folder: &ProgramFolder) -> Result<String, String> {
    read_optional(folder, CODE_FILE)?.ok_or_else(|| {
        format!("There's no {CODE_FILE} in {folder}. Write it with the compile skill.")
    })
}

/// The parts `idea` builds on that aren't lengua ideas in their folders, each a problem saying
/// so: a part is a folder with its own idea.md and `code`, main.s or main.wat.
fn missing_parts(folder: &ProgramFolder, idea: &idea::Idea, code: &str) -> Vec<String> {
    idea.parts
        .iter()
        .filter_map(|part| {
            let path = folder.path().join(&part.idea);
            match std::fs::read_to_string(&path) {
                Err(_) => Some(format!(
                    "`- Builds on:` links to [{}]({}), but {} isn't there: make that part first, \
                     a folder with its own idea.md and {code}, or fix the link",
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

/// What keeps main.s from building on its idea's parts: a part that isn't checked on this
/// computer or has no main.s, a function taken from one that isn't its current code word for
/// word, a function that names a part the idea doesn't build on, and a part the idea builds on
/// that main.s takes nothing from. Each part is a node checked on its own: what main.s takes from
/// it is its checked code, and changes to it show here.
fn parts_problems(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    idea: &idea::Idea,
    source: &checker::Source,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut used: Vec<&idea::Part> = Vec::new();
    let mut checked: Vec<String> = Vec::new();
    for function in &source.functions {
        let Some(from) = &function.from else {
            continue;
        };
        let wanted = idea::folder_of_link(from);
        let Some(part) = idea.parts.iter().find(|part| part.folder() == wanted) else {
            problems.push(format!(
                "{CODE_FILE}'s {} says `# from: {from}`, a part {IDEA_FILE} doesn't build on: add \
                 `[{wanted}]({wanted}/idea.md)` to its `- Builds on:` line, or drop the line",
                function.name
            ));
            continue;
        };
        if !used.contains(&part) {
            used.push(part);
        }
        // The part's own path, `..` resolved, as a program folder takes it.
        let joined = folder.path().join(part.folder());
        let part_folder = std::fs::canonicalize(&joined).unwrap_or(joined);
        let shown = part_folder.display().to_string();
        if !checked.contains(&part.idea) {
            checked.push(part.idea.clone());
            let ready = part_folder
                .to_str()
                .and_then(|path| folder_of(path).ok())
                .and_then(|part_folder| state_of(data_dir, &part_folder).ok())
                .is_some_and(|state| state == State::Ready);
            if !ready {
                problems.push(format!(
                    "the part [{}]({}) isn't checked on this computer: call check with path \
                     {shown} until it's done, then check this program",
                    part.name, part.idea
                ));
                continue;
            }
        }
        // A part checked as WebAssembly has main.wat, which main.s can take nothing from.
        let Ok(part_code) = std::fs::read_to_string(part_folder.join(CODE_FILE)) else {
            problems.push(format!(
                "the part [{}]({}) has no {CODE_FILE}: {CODE_FILE} takes functions only from parts \
                 in {CODE_FILE}; write {} in {CODE_FILE} yourself, without its `# from:` line, and \
                 drop the part from `- Builds on:`",
                part.name, part.idea, function.name
            ));
            continue;
        };
        let (part_source, _) = checker::check_layout(&part_code);
        match part_source.function(&function.name) {
            None => problems.push(format!(
                "{CODE_FILE}'s {} says `# from: {from}`, but {shown}/{CODE_FILE} has no {}: take \
                 it from the part under the name it has there",
                function.name, function.name
            )),
            Some(theirs) if theirs.body != function.body => problems.push(format!(
                "{CODE_FILE}'s {} differs from {shown}/{CODE_FILE}'s, where it comes from: copy it \
                 again word for word, keeping its `# from: {from}` line",
                function.name
            )),
            Some(_) => {}
        }
    }
    for part in &idea.parts {
        if !used.contains(&part) {
            problems.push(format!(
                "{IDEA_FILE} builds on [{}]({}), but {CODE_FILE} takes nothing from it: copy each \
                 function it uses with a `# from: {}` line above it, or drop the part from \
                 `- Builds on:`",
                part.name,
                part.idea,
                part.folder()
            ));
        }
    }
    problems
}

fn state_of(data_dir: &DataDir, folder: &ProgramFolder) -> Result<State, String> {
    let key = Key::load(data_dir).map_err(|error| error.to_string())?;
    let snapshot = Snapshot::read(folder, key.as_ref())
        .map_err(|error| format!("can't read {folder}: {error}"))?;
    Ok(State::of(&snapshot))
}

/// A program lengua may run: the build check tested, its parsed code and idea, and the calls and
/// network its idea allows.
struct Runnable {
    built: Build,
    source: Source,
    idea: Idea,
    syscalls: Vec<crate::platform::Allowed>,
    net: Net,
    /// What lengua's broker does for the program, when it serves it: the program links the
    /// channel's code, and its idea says what it may ask.
    channel: Option<crate::broker::Permit>,
    /// QEMU, when this computer runs the target's programs only through it.
    emulator: Option<Emulator>,
}

/// Checks that `folder` holds a program lengua may run, as `run` does from MCP and from a shell:
/// its idea and code match lengua.lock, no proof found it wrong, and this build of main.s is the
/// one check tested. Environment variables reach only a program whose idea uses the environment.
async fn runnable(
    data_dir: &DataDir,
    folder: &ProgramFolder,
    env: &BTreeMap<String, String>,
) -> Result<Runnable, String> {
    let state = state_of(data_dir, folder)?;
    if state != State::Ready {
        return Err(format!(
            "lengua runs a program only when its idea and code match lengua.lock, and {folder} \
             doesn't. {}",
            state.next_step(folder)
        ));
    }
    let key = Key::load(data_dir)
        .map_err(|error| error.to_string())?
        .ok_or("lengua's signing key is missing; call check")?;
    let lock = lock::read(folder.path(), Some(&key))
        .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
        .ok_or("lengua.lock is missing; call check")?;
    let Some(stamp) = lock.code.as_ref() else {
        return Err(format!(
            "{CODE_FILE} isn't checked; call check with path {folder} first."
        ));
    };
    // What a proof found wrong outlasts lengua's restarts: lengua.lock records it.
    if let Some(proofs) = &lock.proofs
        && proofs.executable_sha256 == stamp.executable_sha256
        && !proofs.wrong.is_empty()
    {
        let wrong: Vec<String> = proofs
            .wrong
            .iter()
            .map(|(id, found)| format!("generated {id}: {found}"))
            .collect();
        return Err(format!(
            "The proof found inputs where {CODE_FILE} is wrong, so it won't run: {}. Call check \
             with path {folder} to see them, then fix {CODE_FILE}.",
            wrong.join("; ")
        ));
    }
    let code = read_code(folder)?;
    let source = checker::check_layout(&code).0;
    let idea = read_idea(folder)
        .and_then(|text| idea::parse(&text).map_err(|problems| idea_problems(folder, &problems)))?;
    if !env.is_empty() && !idea.uses.contains(&idea::Use::Environment) {
        return Err(format!(
            "{IDEA_FILE} doesn't use the environment, so the program gets no environment \
             variables; leave env out, or add `the environment` to its `- Uses:` line"
        ));
    }
    let machine = Machine::detect();
    let target = session_target(&machine)?;
    let profile = target.profile;
    let mode = |emulator: Option<&str>| {
        emulator.map_or_else(
            || "natively".to_owned(),
            |emulator| format!("under {emulator}"),
        )
    };
    let here = target
        .emulator
        .as_ref()
        .map(|emulator| emulator.name.as_str());
    if stamp.target != profile.name() || stamp.emulator.is_some() != here.is_some() {
        return Err(format!(
            "{CODE_FILE} was tested as {} code {}, and this session runs {} code {}, so it won't \
             run here. Call check with path {folder} to test it here.",
            stamp.target,
            mode(stamp.emulator.as_deref()),
            profile.name(),
            mode(here)
        ));
    }
    // Nor does a program checked for a CPU with an extension this one lacks, or proven at
    // another SVE vector length, which a proof holds for alone.
    let checked_for = Header {
        idea: String::new(),
        target: stamp.target.clone(),
        features: stamp.features.clone(),
    };
    if let Some(elsewhere) = checked_for.elsewhere(profile, &target.features(&machine)) {
        return Err(format!(
            "{elsewhere}, so it won't run. Call check with path {folder} to check it here."
        ));
    }
    let built = build(&code, profile).await?;
    if Sha256Hex::of(&built.bytes) != stamp.executable_sha256 {
        return Err(format!(
            "This build of {CODE_FILE} differs from the one check tested (built with {}; now \
             {}), so it won't run. Call check with path {folder} to test this build.",
            stamp.toolchain, built.toolchain
        ));
    }
    let syscalls = profile.allowed_syscalls(&idea.syscalls, idea.threads.is_some());
    // The connections the idea names, which lengua makes for the program, and for a server, its
    // own port on the loopback.
    let net = Net {
        connections: reaches(&idea.network),
        listen: idea.network.listens,
        serve_anywhere: idea.network.serves_anywhere,
    };
    Ok(Runnable {
        channel: crate::edge::uses_channel(profile, &code)
            .then(|| crate::broker::Permit::of(&idea.syscalls)),
        built,
        source,
        idea,
        syscalls,
        net,
        emulator: target.emulator,
    })
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
/// the shell's own standard input, output and error and no time limit. Returns the program's
/// exit code, or 128 plus the number of the signal that ended it, which it names on standard
/// error.
///
/// # Errors
///
/// Fails, saying why, when lengua won't run the program or can't start it.
pub async fn run_from_shell(data_dir: &DataDir, request: &ShellRun) -> Result<i32, String> {
    let here = std::env::current_dir()
        .map_err(|error| format!("can't read the current folder: {error}"))?;
    let absolute = |path: &str| here.join(path).to_string_lossy().into_owned();
    let folder = ProgramFolder::new(&absolute(&request.path))?;
    // An Attested Computation's main.wat runs here, in this process, as WebAssembly.
    if wat::attested_folder(&folder)? {
        let workspace = working_folder(
            &request
                .folder
                .as_deref()
                .map_or_else(|| here.to_string_lossy().into_owned(), absolute),
        )?;
        return wat::run_from_shell(data_dir, &folder, request, &workspace);
    }
    let Runnable {
        built,
        syscalls,
        net,
        channel,
        emulator,
        ..
    } = runnable(data_dir, &folder, &request.env).await?;
    let workspace = working_folder(
        &request
            .folder
            .as_deref()
            .map_or_else(|| here.to_string_lossy().into_owned(), absolute),
    )?;
    let launcher = std::env::current_exe()
        .map_err(|error| format!("can't find lengua-mcp itself: {error}"))?;
    let job = Job {
        folder: Some(&workspace),
        net: Some(&net),
        env: &request.env,
        emulated: emulator.is_some(),
        channel,
        ..Job::new(&built.executable, &request.args, &[], &syscalls)
    };
    match runner::attached(&launcher, job)
        .await
        .map_err(|error| format!("couldn't run the program: {error}"))?
    {
        End::Signaled(signal) => {
            // A fault says what it was; an interrupt, a hangup, a stop or a closed pipe is the
            // user's or the shell's doing, and quiet, as for any program.
            if !matches!(signal, 1 | 2 | 13 | 15) {
                eprintln!(
                    "lengua-mcp: the program was stopped by {}",
                    runner::signal_name(signal)
                );
            }
            Ok(128 + signal)
        }
        End::Exited(code) => Ok(code),
        other => Err(other.describe(Limits::default())),
    }
}

/// A built program, in a temporary folder that lives as long as this value.
struct Build {
    /// The build folder, kept while anything may still start the executable in it.
    folder: Arc<tempfile::TempDir>,
    executable: PathBuf,
    bytes: Vec<u8>,
    toolchain: String,
}

/// Assembles and links `code` in a fresh temporary folder.
async fn build(code: &str, profile: Profile) -> Result<Build, String> {
    let toolchain = Toolchain::discover()
        .await
        .map_err(|error| error.to_string())?;
    let folder =
        tempfile::tempdir().map_err(|error| format!("couldn't create a build folder: {error}"))?;
    let main = folder.path().join(CODE_FILE);
    let object = folder.path().join("main.o");
    let executable = folder.path().join("main");
    std::fs::write(&main, code)
        .map_err(|error| format!("couldn't copy main.s to the build folder: {error}"))?;
    let built = match toolchain.assemble(profile, &main, &object).await {
        Ok(()) => toolchain.link(profile, &object, &executable).await,
        Err(error) => Err(error),
    };
    built.map_err(|error| format!("{CODE_FILE} doesn't build: {error}"))?;
    let bytes = std::fs::read(&executable)
        .map_err(|error| format!("couldn't read the built program: {error}"))?;
    // A program in the edge form is built with lengua's text, which changes when lengua does: the
    // stamp names it, so that a refusal after such a change says what differs.
    let edge = crate::edge::lengua_text(profile, code)
        .map(|text| {
            let hash = Sha256Hex::of(text.as_bytes());
            format!(", lengua's edge {}", &hash.as_str()[..8])
        })
        .unwrap_or_default();
    Ok(Build {
        folder: Arc::new(folder),
        executable,
        bytes,
        toolchain: format!(
            "llvm-mc {}, ld.lld {}{edge}",
            toolchain.assembler.version, toolchain.linker.version
        ),
    })
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

/// The session's target, decided once: this computer's own, or the one a developer's
/// `LENGUA_TARGET` names, which QEMU's user mode runs here.
fn session_target(machine: &Machine) -> Result<Target, String> {
    static TARGET: std::sync::OnceLock<Result<Target, String>> = std::sync::OnceLock::new();
    TARGET
        .get_or_init(|| Target::session(machine, std::env::var("LENGUA_TARGET").ok().as_deref()))
        .clone()
}

/// Why main.s is code for another CPU than the session's target, when it is: its first line
/// says which CPU's code it is.
fn code_for_another_cpu(code: &str, target: &Target) -> Option<String> {
    let isa = match code.lines().next().map(str::trim_end) {
        Some(".intel_syntax noprefix") => Isa::X86_64,
        Some(".arch armv8-a") => Isa::Aarch64,
        _ => return None,
    };
    let name = |isa| match isa {
        Isa::X86_64 => "x86-64",
        Isa::Aarch64 => "AArch64",
    };
    (isa != target.profile.isa()).then(|| match target.emulator {
        None => format!(
            "{CODE_FILE} is {} code, and this computer is {}",
            name(isa),
            target.profile.name()
        ),
        Some(_) => format!(
            "{CODE_FILE} is {} code, and LENGUA_TARGET has lengua check {} code here",
            name(isa),
            target.profile.name()
        ),
    })
}

/// How a proof line of code the prover has no model for begins.
const NOT_MODELLED: &str = "tested, not proven: ";

/// Whether the prover models the CPU of `profile`'s programs: x86-64's and AArch64's.
fn modelled(profile: Profile) -> bool {
    matches!(profile.isa(), Isa::X86_64 | Isa::Aarch64)
}

/// The proof line of a test whose program's CPU the prover has no model of.
fn not_modelled(profile: Profile) -> String {
    format!(
        "{NOT_MODELLED}lengua's prover has no model of {} code yet",
        profile.name()
    )
}

fn random_seed() -> Result<u64, String> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(|error| format!("couldn't draw a seed: {error}"))?;
    Ok(u64::from_le_bytes(bytes))
}

/// The checker's findings, one line each.
fn violation_lines(report: &checker::Report) -> Vec<String> {
    let mut lines: Vec<String> = report.violations.iter().map(ToString::to_string).collect();
    if report.hidden > 0 {
        lines.push(format!("…and {} more", report.hidden));
    }
    lines
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

fn idea_problems(folder: &ProgramFolder, problems: &[idea::Problem]) -> String {
    let mut text = format!("{folder}/{IDEA_FILE} has problems; call check:\n");
    for problem in problems {
        let _ = writeln!(text, "- {problem}");
    }
    text
}

/// Why lengua can't build programs on this machine, if it can't.
fn machine_blocker(
    machine: &Machine,
    toolchain_error: Option<&crate::toolchain::ToolchainError>,
) -> Option<String> {
    if let Err(problem) = session_target(machine) {
        // Where lengua builds no main.s, it still checks and runs main.wat.
        let wasm = if machine.profile.is_none() {
            format!(
                " On this computer lengua checks and runs WebAssembly: make {IDEA_FILE} an \
                 Attested Computation (`type: Attested Computation`) with the write-idea skill, \
                 then compile it into {WASM_CODE_FILE}."
            )
        } else {
            String::new()
        };
        return Some(format!("Blocked: {problem}{wasm}"));
    }
    toolchain_error.map(|error| format!("Blocked: {error}"))
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
        let idea = include_str!("../tests/fixtures-wasm/add/idea.md");
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

    #[test]
    fn the_machine_fits_in_a_line() {
        let machine = Machine::detect();
        let Ok(native) = Target::session(&machine, None) else {
            crate::skiplog::note("lengua has no target for this system yet");
            return;
        };
        let line = machine_line(&machine, &native);
        assert!(line.contains("core"), "{line}");
        assert!(!line.contains('\n'), "{line}");
        assert!(!line.contains("sse4_"), "{line}");
        assert!(line.starts_with(native.profile.name()), "{line}");
        // Under QEMU, the target comes first and says what runs it, with the emulated CPU's
        // extensions, never x86-64's.
        let emulated = Target {
            profile: Profile::LinuxAarch64,
            emulator: Some(Emulator {
                name: "qemu-aarch64 10.2.2".into(),
            }),
        };
        let line = machine_line(&machine, &emulated);
        assert!(
            line.starts_with(&format!(
                "linux-aarch64 (emulated by qemu-aarch64 10.2.2 on {}-{}), ",
                machine.os, machine.arch
            )),
            "{line}"
        );
        // QEMU's CPU has every extension, of which the dialect allows AArch64's CRC-32 and its
        // carry-less multiply, and SVE at the vector length it runs with.
        assert!(
            line.contains("main.s may use the base instructions and crc, pmull"),
            "{line}"
        );
        assert!(!line.contains("avx") && !line.contains("sse"), "{line}");
    }

    /// main.s's first line says which CPU's code it is; code for another stops check.
    #[test]
    fn code_for_another_cpu_is_compiled_again() {
        let native = Target {
            profile: Profile::LinuxX86_64,
            emulator: None,
        };
        let emulated = Target {
            profile: Profile::LinuxAarch64,
            emulator: Some(Emulator {
                name: "qemu-aarch64 10.2.2".into(),
            }),
        };
        let x86 = ".intel_syntax noprefix\n_start:\n";
        let aarch64 = ".arch armv8-a\n_start:\n";
        assert_eq!(code_for_another_cpu(x86, &native), None);
        assert_eq!(code_for_another_cpu(aarch64, &emulated), None);
        assert_eq!(
            code_for_another_cpu(aarch64, &native).as_deref(),
            Some("main.s is AArch64 code, and this computer is linux-x86_64")
        );
        assert_eq!(
            code_for_another_cpu(x86, &emulated).as_deref(),
            Some(
                "main.s is x86-64 code, and LENGUA_TARGET has lengua check linux-aarch64 code here"
            )
        );
        // A first line that's neither is the layout check's to explain.
        assert_eq!(code_for_another_cpu("_start:\n", &native), None);
    }
}
