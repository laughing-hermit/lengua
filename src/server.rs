//! The lengua MCP server: tool definitions and server instructions.

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
use crate::hash::Sha256Hex;
use crate::lock::{self, CodeStamp, Key, Lock, ProofStamp, SpecStamp, TestsStamp};
use crate::platform::{Machine, Profile};
use crate::prove;
use crate::runner::{self, End, Job, Limits, Net, Started};
use crate::spec::{self, Spec};
use crate::state::{
    CODE_FILE, ProgramFolder, SPEC_FILE, Snapshot, State, TESTS_FILE, stamped_copy,
};
use crate::testdiff;
use crate::testfile::{self, TestSuite};
use crate::testrun;
use crate::toolchain::Toolchain;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua turns a request into a checked x86-64 program: a folder \
holding spec.md, tests.toml and main.s, written with the write-spec, write-tests and \
write-assembly skills. After writing or changing any of them, call check with the folder's \
absolute path, the first time with the user's words as request, and follow its next_step until \
it says Done: the tests and the proof run in the background, so call check again while it says \
testing or proving. Never edit lengua.lock. Run programs only with run.";

/// The longest request the lock records: the user's words, not a whole conversation.
const MAX_REQUEST_CHARS: usize = 2000;

/// The longest a `check` call waits for the tests and the proof before it replies. They go on
/// in the background, and the next call picks them up, so no call outlasts an agent harness's
/// patience.
const CHECK_WAIT: Duration = Duration::from_secs(40);

/// Once the tests pass, how long a `check` call still waits for the proof before it says so,
/// so a quick proof comes back in the same reply and a long one doesn't hold up the news.
const GREEN_GRACE: Duration = Duration::from_secs(10);

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

/// How long the proof of one version may take: [`PROOF_TIME`], or `LENGUA_PROOF_SECONDS` for
/// tests that aren't about proofs.
fn proof_time() -> Duration {
    std::env::var("LENGUA_PROOF_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .map_or(PROOF_TIME, Duration::from_secs)
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
}

/// A server `run` started in the background.
struct Background {
    started: Started,
    /// The build folder of the executable it started from.
    _build: Arc<tempfile::TempDir>,
    /// Which start this is, so an old timer never stops a newer one.
    id: u64,
    assertions: BTreeMap<i64, usize>,
}

impl std::fmt::Debug for Background {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Background")
            .field("id", &self.id)
            .field("port", &self.started.port())
            .finish_non_exhaustive()
    }
}

/// How long a server `run` started keeps serving at most.
const BACKGROUND_LIMIT: Duration = Duration::from_secs(600);

/// The tests and then the proof of one version of a program, in the background.
struct Checking {
    /// spec.md, tests.toml and main.s, by hash.
    files: [Sha256Hex; 3],
    /// The executable built from main.s, by hash.
    executable: Sha256Hex,
    /// Set once the files change, which stops the proof.
    cancel: Arc<AtomicBool>,
    /// What it has found so far.
    found: std::sync::Mutex<Found>,
    /// How far it has got, for whoever waits on it.
    stage: watch::Sender<Stage>,
    /// Whether a reply has said the tests pass yet.
    green_told: AtomicBool,
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
    /// Whether the proof found inputs where main.s is wrong.
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
    /// Whether it found inputs where main.s is wrong.
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
        }
    }
}

/// Input for `check`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckParams {
    /// Absolute path of the program folder, the folder that holds spec.md.
    pub path: String,
    /// The user's own words that asked for this program or this change, quoted exactly, such as
    /// "Make me a program that adds two numbers". Needed the first time, and whenever the user
    /// asks for something new; without it, an edited spec is recorded under the words already
    /// recorded.
    #[serde(default)]
    pub request: Option<String>,
}

/// What `check` reports: where the program stands, and the one thing to do next.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
pub struct CheckReport {
    /// Where the program stands: blocked, write_spec, spec_problems, needs_request,
    /// write_tests, test_problems, write_code, code_problems, testing, tests_failed, proving or
    /// done.
    pub state: String,
    /// Every problem to fix before going on, each saying where it is and how to fix it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The machine main.s runs on, while main.s isn't written yet: its cores, which a spec's
    /// threads may use, and the CPU extensions main.s may use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// What the tests found, once they pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<String>,
    /// Each generated test's proof: what's proven for every input, how far, or why not; while
    /// proving, how far it has got.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proofs: Vec<String>,
    /// The spec's first example as the tests ran it: what to show the user it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// What each test cost, once done: instructions a run over all its runs, and from its
    /// largest input run alone, CPU cycles and times (lower is faster).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performance: Vec<String>,
    /// What changed after the user's request, once done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<ChangesReport>,
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
    /// How spec.md changed since the build's first request, as `-`/`+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_changed: Option<String>,
    /// How tests.toml changed since code first ran in this build, as `-`/`+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_changed: Option<String>,
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
    /// `stdin`, read by the program itself however large it is.
    #[serde(default)]
    pub stdin_file: Option<String>,
    /// How many seconds the program may run, 1 to 40; 10 when not given.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
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
}

/// What `run` reports about a program that has ended.
fn ended_report(
    result: &runner::Run,
    assertions: &BTreeMap<i64, usize>,
    limits: Limits,
) -> RunReport {
    RunReport {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        exit_code: match result.end {
            End::Exited(code) => Some(code),
            _ => None,
        },
        ended: testrun::explain_end(result, assertions, limits),
        instructions: result.instructions,
        cycles: result.cycles,
        cpu_time_us: result.cpu_time_ns.map(|nanoseconds| nanoseconds / 1000),
        wall_time_us: result.wall_time_ns / 1000,
        serving: None,
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
        static STARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut background = self.background.lock().await;
        if let Some(entry) = background.get(folder.path()) {
            return Err(format!(
                "{folder} is already serving on 127.0.0.1:{}; call run with stop: true first",
                entry.started.port().unwrap_or(0)
            ));
        }
        let limits = Limits {
            timeout: BACKGROUND_LIMIT,
            max_file_size: None,
            ..Limits::default()
        };
        let started = runner::start(&self.launcher, job, limits)
            .await
            .map_err(|error| format!("couldn't start the server: {error}"))?;
        let address = format!("127.0.0.1:{}", started.port().unwrap_or(0));
        let id = STARTS.fetch_add(1, Ordering::Relaxed);
        background.insert(
            folder.path().to_path_buf(),
            Background {
                started,
                _build: build,
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
                let _ = entry.started.stop().await;
            }
        });
        Ok(RunReport {
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
        })
    }
}

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "check",
        title = "Check",
        description = "Checks the lengua program in <path> stage by stage, records each stage in \
            lengua.lock, and stops at the first that needs work: it records spec.md as the \
            source of truth with the user's words that asked for it, checks tests.toml against \
            the spec, checks main.s against lengua's rules, runs every test with fresh random \
            inputs, and proves the program for every input it can. Call it after every edit to \
            spec.md, tests.toml or main.s. It replies within about 40 seconds; the tests and the \
            proof go on in the background, so while it says testing or proving, call it again \
            to wait. path: absolute path of the program folder; request: the user's words that \
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
        let began = Instant::now();
        let folder = folder_of(&params.path)?;
        let machine = Machine::detect();
        let toolchain = Toolchain::discover().await;
        if let Some(blocked) = machine_blocker(&machine, toolchain.as_ref().err()) {
            return Ok(Json(CheckReport::new("blocked", blocked)));
        }
        let profile = supported(&machine)?;
        let machine_line = || machine_line(&machine);

        // spec.md, recorded once it parses.
        let Some(spec_text) = read_optional(&folder, SPEC_FILE)? else {
            return Ok(Json(CheckReport {
                machine: Some(machine_line()),
                ..CheckReport::new("write_spec", State::NoSpec.next_step(&folder))
            }));
        };
        let spec = match spec::parse(&spec_text) {
            Ok(spec) => spec,
            Err(problems) => {
                return Ok(Json(CheckReport::problems(
                    "spec_problems",
                    problems.iter().map(ToString::to_string).collect(),
                    format!(
                        "Next: fix {folder}/{SPEC_FILE} with the write-spec skill, then call check \
                         with path {folder}."
                    ),
                )));
            }
        };
        if let Some(next_step) = self
            .record(&folder, &spec_text, params.request.as_deref())
            .await?
        {
            return Ok(Json(CheckReport::new("needs_request", next_step)));
        }
        let spec_hash = Sha256Hex::of(spec_text.as_bytes());

        // tests.toml, checked against the spec.
        let Some(tests_text) = read_optional(&folder, TESTS_FILE)? else {
            return Ok(Json(CheckReport {
                machine: Some(machine_line()),
                ..CheckReport::new("write_tests", State::TestsMissing.next_step(&folder))
            }));
        };
        let suite = match testfile::check(&tests_text, &spec) {
            Ok(suite) => suite,
            Err(problems) => {
                return Ok(Json(CheckReport::problems(
                    "test_problems",
                    problems.iter().map(ToString::to_string).collect(),
                    format!(
                        "Next: fix {folder}/{TESTS_FILE} with the write-tests skill, then call \
                         check with path {folder}."
                    ),
                )));
            }
        };
        let tests_hash = Sha256Hex::of(tests_text.as_bytes());
        self.note_tests(&folder, &spec_hash, &tests_hash, &tests_text)
            .await?;

        // main.s, checked against the rules, then tested and proven in the background.
        let Some(code) = read_optional(&folder, CODE_FILE)? else {
            return Ok(Json(CheckReport {
                machine: Some(machine_line()),
                ..CheckReport::new("write_code", State::CodeMissing.next_step(&folder))
            }));
        };
        let files = [spec_hash, tests_hash, Sha256Hex::of(code.as_bytes())];
        let checking = match self.current_checking(&folder, &files).await {
            Some(checking) => checking,
            None => {
                let fix = || {
                    format!(
                        "Next: fix {folder}/{CODE_FILE} with the write-assembly skill (the full \
                         rules are in its references/power-of-10.md), then call check with path \
                         {folder}."
                    )
                };
                let (source, layout) = checker::check_layout(&code);
                if !layout.passed() {
                    return Ok(Json(CheckReport::problems(
                        "code_problems",
                        violation_lines(&layout),
                        fix(),
                    )));
                }
                let built = match build(&code, profile).await {
                    Ok(built) => built,
                    Err(error) => {
                        return Ok(Json(CheckReport::problems(
                            "code_problems",
                            vec![error],
                            fix(),
                        )));
                    }
                };
                let report = checker::check_program(
                    &source,
                    &built.bytes,
                    checker::Context {
                        syscalls: &spec.syscalls,
                        threads: spec.threads,
                        features: machine.features,
                        profile,
                    },
                );
                if !report.passed() {
                    return Ok(Json(CheckReport::problems(
                        "code_problems",
                        violation_lines(&report),
                        fix(),
                    )));
                }
                self.start_checking(
                    &folder,
                    files,
                    Code {
                        spec,
                        suite,
                        source,
                        built,
                        report,
                        profile,
                    },
                )
                .await
            }
        };
        wait_for(&checking, began + check_wait()).await;
        Ok(Json(self.checking_report(&folder, &checking)))
    }

    #[tool(
        name = "run",
        title = "Run program",
        description = "Runs the checked program in <path> with args and stdin. It runs only \
            when lengua.lock shows that spec.md, tests.toml and main.s are exactly the versions \
            that were recorded and checked, and a fresh build of main.s is byte for byte the \
            executable check tested. The program gets an empty environment, 10 seconds and \
            8 MiB of output, and the kernel lets it make only the system calls its spec allows, \
            connect only to the ports it names, and touch files only inside its working folder: \
            an empty temporary folder, or `folder`. A server starts in the background on its \
            spec's port and run returns its address; call run with stop: true to stop it and get \
            what it printed (it also stops after ten minutes or when the session ends). path: \
            absolute path of the program folder; args: the program's arguments; stdin: text for \
            its standard input; folder: absolute path of a folder of the user's files to run in, \
            naming the files in args relative to it (\"notes.txt\", not \"/home/me/notes.txt\"); \
            stdin_file: a file in folder to read as standard input instead of stdin, however \
            large; timeout_seconds: up to 40 (10 by default); stop: true to stop the server run \
            started.",
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
                .started
                .stop()
                .await
                .map_err(|error| format!("couldn't stop the program: {error}"))?;
            let limits = Limits {
                timeout: BACKGROUND_LIMIT,
                ..Limits::default()
            };
            return Ok(Json(ended_report(&result, &entry.assertions, limits)));
        }
        let state = self.program_state(&folder)?;
        if state != State::Ready {
            return Err(format!(
                "lengua runs a program only when its spec, tests and code all match \
                 lengua.lock, and {folder} doesn't. {}",
                state.next_step(&folder)
            ));
        }
        let key = Key::load(&self.data_dir)
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
        let code = read_code(&folder)?;
        let source = checker::check_layout(&code).0;
        let spec = read_spec(&folder).and_then(|text| {
            spec::parse(&text).map_err(|problems| spec_problems(&folder, &problems))
        })?;
        let workspace = match params.folder.as_deref() {
            None => None,
            Some(path) => Some(working_folder(path)?),
        };
        let machine = Machine::detect();
        let profile = supported(&machine)?;
        let built = build(&code, profile).await?;
        if Sha256Hex::of(&built.bytes) != stamp.executable_sha256 {
            return Err(format!(
                "This build of {CODE_FILE} differs from the one check tested (built with {}; \
                 now {}), so it won't run. Call check with path {folder} to test this build.",
                stamp.toolchain, built.toolchain
            ));
        }
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
        let syscalls = profile.allowed_syscalls(&spec.syscalls, spec.threads.is_some());
        // The ports the spec names, and for a server, its own port on the loopback.
        let net = Net {
            connect: spec.network.connects.clone(),
            anywhere: spec.network.anywhere,
            listen: spec.network.listens,
        };
        let job = Job {
            folder: workspace.as_deref(),
            net: Some(&net),
            stdin_file: stdin_file.as_deref(),
            ..Job::new(
                &built.executable,
                &params.args,
                params.stdin.as_bytes(),
                &syscalls,
            )
        };
        let assertions = testrun::assertion_lines(&source);
        if spec.network.listens.is_some() {
            return self
                .serve_in_background(&folder, job, Arc::clone(&built.folder), assertions)
                .await
                .map(Json);
        }
        let result = runner::run(&self.launcher, job, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        Ok(Json(ended_report(&result, &assertions, limits)))
    }
}

/// Everything a checking job needs about the code it checks.
struct Code {
    spec: Spec,
    suite: TestSuite,
    source: Source,
    built: Build,
    report: checker::Report,
    profile: Profile,
}

/// Waits until `checking` ends or `until`, whichever comes first; once its tests pass, at most
/// [`GREEN_GRACE`] more, unless a reply has already said so.
async fn wait_for(checking: &Checking, until: Instant) {
    let mut stage = checking.stage.subscribe();
    loop {
        let deadline = match *stage.borrow_and_update() {
            Stage::Ended => return,
            Stage::Proving(since) if !checking.green_told.load(Ordering::Relaxed) => {
                until.min(since + GREEN_GRACE)
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
    /// The program's state.
    fn program_state(&self, folder: &ProgramFolder) -> Result<State, String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        Ok(State::of(&snapshot))
    }

    /// Records spec.md, unless the lock already records it as it is, with the user's words when
    /// given. Returns the next step instead when it can't be recorded without their words.
    async fn record(
        &self,
        folder: &ProgramFolder,
        text: &str,
        request: Option<&str>,
    ) -> Result<Option<String>, String> {
        let words = request.map(str::trim).filter(|words| !words.is_empty());
        if words.is_some_and(|words| words.chars().count() > MAX_REQUEST_CHARS) {
            return Err(format!(
                "request holds more than {MAX_REQUEST_CHARS} characters; quote only the user's \
                 words that ask for the program or the change."
            ));
        }
        let current = Sha256Hex::of(text.as_bytes());
        let _writing = self.lock_writes.lock().await;
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        let previous = match lock::read(folder.path(), Some(&key)) {
            Ok(previous) => previous,
            Err(error) if words.is_none() => {
                return Ok(Some(State::LockInvalid(error).next_step(folder)));
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
            (None, None) => return Ok(Some(State::SpecUnrecorded.next_step(folder))),
            (Some(lock), None) => {
                if lock.spec.sha256 == current {
                    return Ok(None);
                }
                // The spec was revised under a request it already has: it's checked again, and
                // the change is shown.
                Lock {
                    spec: SpecStamp {
                        sha256: current.clone(),
                    },
                    tests: None,
                    code: None,
                    proofs: None,
                    ..lock
                }
            }
            // New words in the middle of a build join it, so nothing changed before them drops
            // out of what is shown.
            (Some(lock), Some(words)) if lock.code.is_none() => {
                let unchanged = lock.spec.sha256 == current;
                let mut request = lock.request;
                if request.words.len() >= lock::MAX_REQUESTS {
                    request.words.remove(1);
                }
                request.words.push(words.to_owned());
                Lock {
                    request,
                    spec: SpecStamp {
                        sha256: current.clone(),
                    },
                    tests: if unchanged { lock.tests } else { None },
                    code: None,
                    proofs: None,
                }
            }
            // A new request after a finished build starts a new build. With the spec unchanged,
            // its checked tests and code stay, and the tests the code ran against are where test
            // changes are counted from.
            (Some(lock), Some(words)) if lock.spec.sha256 == current => {
                let mut request = Lock::requested(current.clone(), words.to_owned()).request;
                request.tests = lock.tests.as_ref().map(|tests| tests.sha256.clone());
                Lock { request, ..lock }
            }
            (_, Some(words)) => Lock::requested(current.clone(), words.to_owned()),
        };
        // Every recorded version is kept by its hash, so later changes can be shown.
        write_stamped_copy(&self.data_dir, &current, text)?;
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        Ok(None)
    }

    /// Records checked tests in the lock, unless it already records them.
    async fn note_tests(
        &self,
        folder: &ProgramFolder,
        spec: &Sha256Hex,
        tests: &Sha256Hex,
        text: &str,
    ) -> Result<(), String> {
        let recorded = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .is_some_and(|lock| {
                lock.spec.sha256 == *spec
                    && lock
                        .tests
                        .as_ref()
                        .is_some_and(|stamp| stamp.sha256 == *tests)
            });
        if recorded {
            return Ok(());
        }
        self.stamp_tests(folder, spec, tests, text).await
    }

    /// The checking job for these files, running or ended, unless it ended with failed tests,
    /// which run again, or passed them but lengua.lock no longer records the code it checked.
    async fn current_checking(
        &self,
        folder: &ProgramFolder,
        files: &[Sha256Hex; 3],
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
            let recorded = Key::load(&self.data_dir)
                .ok()
                .flatten()
                .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
                .and_then(|lock| lock.code)
                .is_some_and(|code| {
                    code.sha256 == files[2] && code.executable_sha256 == checking.executable
                });
            if !recorded {
                return None;
            }
        }
        Some(Arc::clone(checking))
    }

    /// Starts testing and proving `code` in the background, stopping any job on older files.
    async fn start_checking(
        &self,
        folder: &ProgramFolder,
        files: [Sha256Hex; 3],
        code: Code,
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
            executable: Sha256Hex::of(&code.built.bytes),
            cancel: Arc::default(),
            found: std::sync::Mutex::new(Found {
                proofs,
                ..Found::default()
            }),
            stage: watch::Sender::new(Stage::Testing),
            green_told: AtomicBool::new(false),
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
        let [spec_hash, tests_hash, code_hash] = checking.files.clone();
        if let Err(error) = self.note_first_run(folder, &tests_hash).await {
            return fail(vec![error]);
        }
        let seed = match random_seed() {
            Ok(seed) => seed,
            Err(error) => return fail(vec![error]),
        };
        let syscalls = code
            .profile
            .allowed_syscalls(&code.spec.syscalls, code.spec.threads.is_some());
        let outcome = match testrun::run_tests(
            &code.suite,
            &self.launcher,
            &code.built.executable,
            &code.source,
            seed,
            &syscalls,
            &code.spec.network,
            testrun::workers(code.spec.threads),
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
        };
        if let Err(error) = self
            .stamp_code(folder, &spec_hash, &tests_hash, stamp)
            .await
        {
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
            });
        }
        checking.stage.send_replace(Stage::Proving(Instant::now()));
        if !self.recorded_proofs(folder, checking, &code.suite, &executable) {
            self.prove_all(folder, checking, &code, &syscalls, &executable)
                .await;
        }
    }

    /// Takes the proofs lengua.lock records about this very build, after a restart lost the job
    /// that made them. Returns whether it has any.
    fn recorded_proofs(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        suite: &TestSuite,
        executable: &Sha256Hex,
    ) -> bool {
        let Some(stamp) = Key::load(&self.data_dir)
            .ok()
            .flatten()
            .and_then(|key| lock::read(folder.path(), Some(&key)).ok().flatten())
            .and_then(|lock| lock.proofs)
            .filter(|proofs| proofs.executable_sha256 == *executable)
        else {
            return false;
        };
        let Ok(mut found) = checking.found.lock() else {
            return false;
        };
        for (block, proof) in suite.generated.iter().zip(&mut found.proofs) {
            let id = block.id.to_string();
            proof.detail = Some(if stamp.proven.contains(&id) {
                "proven for every input in range, as lengua.lock records".to_owned()
            } else if let Some((_, bytes)) = stamp.bounded.iter().find(|(known, _)| *known == id) {
                format!(
                    "proven for every input with text up to {bytes} bytes, as lengua.lock \
                         records; longer texts are tested, not proven"
                )
            } else {
                "tested, not proven".to_owned()
            });
        }
        true
    }

    /// Proves each generated test in turn, each with its share of [`PROOF_TIME`], and records
    /// what was proven.
    async fn prove_all(
        &self,
        folder: &ProgramFolder,
        checking: &Checking,
        code: &Code,
        syscalls: &[crate::platform::Allowed],
        executable: &Sha256Hex,
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
        let decoded = match checker::decode::read(&code.built.bytes) {
            Ok(decoded) => decoded,
            Err(error) => {
                for index in 0..code.suite.generated.len() {
                    set(index, format!("tested, not proven: {error}"), false);
                }
                return;
            }
        };
        let spec = &code.spec;
        let deadline = Instant::now() + proof_time();
        // The ids proven, how many the solvers proved, and why any rested on one solver.
        let mut proven_ids = Vec::new();
        let mut bounded_ids = Vec::new();
        let mut solved = 0;
        let mut alone_reason: Option<String> = None;
        let allows = |call: spec::Syscall| spec.syscalls.contains(&call);
        let listens = spec.network.listens.is_some();
        let blocks = code.suite.generated.len();
        // Small blocks first: they finish in their share and pass the rest on to the big ones.
        let mut order: Vec<usize> = (0..blocks).collect();
        order.sort_by_key(|index| prove::size(&code.suite.generated[*index]));
        for (turn, &index) in order.iter().enumerate() {
            let block = &code.suite.generated[index];
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
            let kernel_values = (allows(spec::Syscall::Getrandom) && block.random.is_none())
                || (allows(spec::Syscall::ClockGettime) && block.clock.is_empty());
            if let Some(every) = block
                .every_value_text()
                .filter(|_| !kernel_values && !block.uses_now())
            {
                proven_ids.push((index, block.id.to_string()));
                set(index, format!("proven: the tests tried {every}"), false);
                continue;
            }
            let budget = prove::Budget {
                cancel: Arc::clone(&checking.cancel),
                ..prove::Budget::until(now + share, listens)
            };
            if let Ok(mut found) = checking.found.lock()
                && let Some(proof) = found.proofs.get_mut(index)
            {
                proof.progress = Some(Arc::clone(&budget.progress));
            }
            // The model must agree with the real program on inputs it passed before it's
            // trusted to cover the rest.
            let samples: Vec<testfile::Case> =
                block.cases(1).take(24).filter_map(Result::ok).collect();
            let (machine, owned) = (decoded.clone(), block.clone());
            let verdict = tokio::task::spawn_blocking(move || {
                prove::agrees(&machine, &samples, listens)
                    .map(|()| prove::prove(&machine, &owned, &budget))
            })
            .await
            .unwrap_or_else(|error| Err(format!("the proof stopped: {error}")));
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
                }) => {
                    match &bound {
                        Some(bound) => {
                            bounded_ids.push((index, (block.id.to_string(), bound.bytes)));
                        }
                        None => proven_ids.push((index, block.id.to_string())),
                    }
                    if questions > 0 {
                        solved += 1;
                        if alone_reason.is_none() {
                            alone_reason.clone_from(&alone);
                        }
                    }
                    (
                        proven_detail(block, shapes, paths, questions, alone, bound, cut_any_way),
                        false,
                    )
                }
                Ok(prove::Verdict::Counterexample(values)) => {
                    self.replay(code, syscalls, block, values, kernel_values)
                        .await
                }
            };
            set(index, detail, wrong);
        }
        if proven_ids.is_empty() && bounded_ids.is_empty() {
            return;
        }
        let solvers = match (solved, alone_reason) {
            (0, _) => {
                "none: the tests tried every value, or the terms settled every step".to_owned()
            }
            (_, None) => "z3 and cvc5".to_owned(),
            (_, Some(reason)) => format!("one solver alone: {reason}"),
        };
        // In the tests' own order, however they were proven.
        proven_ids.sort_unstable();
        bounded_ids.sort_unstable();
        let proven_ids = proven_ids.into_iter().map(|(_, id)| id).collect();
        let bounded_ids = bounded_ids.into_iter().map(|(_, bound)| bound).collect();
        self.stamp_proofs(folder, executable, proven_ids, bounded_ids, solvers)
            .await;
    }

    /// Runs inputs the proof found for real, so a counterexample is reported only when the
    /// program really is wrong on it. Returns the detail and whether main.s is wrong.
    async fn replay(
        &self,
        code: &Code,
        syscalls: &[crate::platform::Allowed],
        block: &testfile::Generated,
        mut values: BTreeMap<String, expr::Value>,
        kernel_values: bool,
    ) -> (String, bool) {
        // The delivery it was found with, and the port the kernel chooses, aren't the test's
        // inputs: a real run is given the delivery and picks its port.
        let delivery = match values.remove(prove::DELIVERY) {
            Some(expr::Value::Str(found)) => {
                testrun::Delivery::described(&found).unwrap_or(testrun::Delivery::Whole)
            }
            _ => testrun::Delivery::Whole,
        };
        if !block.server.is_empty() && !block.vars.iter().any(|(name, _)| name == "port") {
            values.remove("port");
        }
        let networked = !block.server.is_empty() || !block.client.is_empty();
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
        let case = match block.case(inputs, crate::expect::now()) {
            Ok(case) => case,
            Err((_, field, error)) => {
                return (
                    format!("tested, not proven: with {described}, {field} {error}"),
                    false,
                );
            }
        };
        match testrun::check_case(
            &self.launcher,
            &code.built.executable,
            &code.source,
            &case,
            syscalls,
            &code.spec.network,
            delivery,
        )
        .await
        {
            Err(error) => (
                format!("tested, not proven: couldn't run the program: {error}"),
                false,
            ),
            Ok(Some(problem)) => (
                format!(
                    "the proof found inputs the tests didn't try where main.s is wrong: with \
                     {described}: {problem}"
                ),
                true,
            ),
            // A real run draws its own random bytes and reads its own clock, so it can't replay
            // what the proof found.
            Ok(None) if kernel_values => (
                format!(
                    "tested, not proven: with {described}, the prover found random bytes or a \
                     clock reading for which main.s's output breaks the test, which a real run \
                     can't replay; check how main.s turns them into output"
                ),
                false,
            ),
            Ok(None) => (
                format!(
                    "tested, not proven: the prover's model disagreed with a real run on \
                     {described}"
                ),
                false,
            ),
        }
    }

    /// What `check` replies about a checking job.
    fn checking_report(&self, folder: &ProgramFolder, checking: &Checking) -> CheckReport {
        let stage = *checking.stage.borrow();
        let Ok(found) = checking.found.lock() else {
            return CheckReport::new("testing", again(folder));
        };
        let proofs: Vec<String> = found.proofs.iter().map(BlockProof::line).collect();
        if !found.failures.is_empty() {
            return CheckReport::problems(
                "tests_failed",
                found.failures.clone(),
                format!(
                    "Next: decide from {SPEC_FILE} whether {CODE_FILE} or the test is wrong, fix \
                     {CODE_FILE} with the write-assembly skill or {TESTS_FILE} with the \
                     write-tests skill, then call check with path {folder}. Boundary values run \
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
            "{CODE_FILE} follows every rule and passed all {} test runs",
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
                        "Next: fix {folder}/{CODE_FILE} with the write-assembly skill so it handles \
                         those inputs, then call check with path {folder}."
                    ),
                )
            };
        }
        if stage != Stage::Ended {
            return CheckReport {
                tests,
                proofs,
                ..CheckReport::new(
                    "proving",
                    format!(
                        "Every test passes, and the proof is running in the background. {}",
                        again(folder)
                    ),
                )
            };
        }
        let changes = self.current_changes(folder);
        let changes = (changes.requests.len() > 1
            || changes.spec.is_some()
            || changes.tests.is_some()
            || !changes.tests_less.is_empty())
        .then_some(ChangesReport {
            requests: changes.requests,
            spec_changed: changes.spec,
            tests_changed: changes.tests,
            tests_less: changes.tests_less,
        });
        let shown = if changes.is_some() {
            " changes lists what changed after the user's request: tell them each change, and why."
        } else {
            ""
        };
        CheckReport {
            tests,
            proofs,
            example: passed.shown.clone(),
            performance: passed.performance.clone(),
            changes,
            ..CheckReport::new(
                "done",
                format!(
                    "Done: tell the user where the program is, what its spec promises, that it \
                     passed every test, and what's proven.{shown} run runs it on their own \
                     inputs."
                ),
            )
        }
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

    /// What changed after the user's request: spec.md since the build's first request, and
    /// tests.toml since code first ran in the build, each against the copy lengua kept.
    fn changes(&self, folder: &ProgramFolder, lock: &Lock) -> Changes {
        let kept = |hash: &Sha256Hex| {
            std::fs::read_to_string(stamped_copy(self.data_dir.path(), hash))
                .ok()
                .filter(|text| Sha256Hex::of(text.as_bytes()) == *hash)
        };
        let unseen = "the first version isn't on this machine to compare";
        let spec = match std::fs::read_to_string(folder.file(SPEC_FILE)) {
            Ok(now) if Sha256Hex::of(now.as_bytes()) != lock.request.spec => {
                Some(kept(&lock.request.spec).map_or_else(
                    || format!("{SPEC_FILE} changed; {unseen}"),
                    |before| testdiff::line_diff(&before, &now),
                ))
            }
            _ => None,
        };
        let (tests, tests_less) = match (
            &lock.request.tests,
            std::fs::read_to_string(folder.file(TESTS_FILE)),
        ) {
            (Some(first), Ok(now)) if Sha256Hex::of(now.as_bytes()) != *first => {
                match kept(first) {
                    Some(before) => (
                        Some(testdiff::line_diff(&before, &now)),
                        testdiff::tests_less(&before, &now),
                    ),
                    None => (Some(format!("{TESTS_FILE} changed; {unseen}")), Vec::new()),
                }
            }
            _ => (None, Vec::new()),
        };
        Changes {
            requests: lock.request.words.clone(),
            spec: spec.filter(|diff| !diff.is_empty()),
            tests: tests.filter(|diff| !diff.is_empty()),
            tests_less,
        }
    }

    /// Records checked tests in the lock, provided neither file changed while being checked,
    /// and keeps a copy so later changes can be shown against it.
    async fn stamp_tests(
        &self,
        folder: &ProgramFolder,
        spec: &Sha256Hex,
        tests: &Sha256Hex,
        text: &str,
    ) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        let changed = || {
            format!(
                "{SPEC_FILE} or {TESTS_FILE} changed while the tests were being checked, so \
                 nothing was recorded; call check again."
            )
        };
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or_else(changed)?;
        let lock = lock::read(folder.path(), Some(&key))
            .ok()
            .flatten()
            .ok_or_else(changed)?;
        let current_spec = Sha256Hex::of(read_spec(folder)?.as_bytes());
        let current_tests = Sha256Hex::of(read_tests(folder)?.as_bytes());
        if lock.spec.sha256 != *spec || current_spec != *spec || current_tests != *tests {
            return Err(changed());
        }
        write_stamped_copy(&self.data_dir, tests, text)?;
        let lock = Lock {
            tests: Some(TestsStamp {
                sha256: tests.clone(),
            }),
            code: None,
            proofs: None,
            ..lock
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))
    }

    /// Notes the tests code first runs against in this build, the version later test changes
    /// are shown against.
    async fn note_first_run(
        &self,
        folder: &ProgramFolder,
        tests: &Sha256Hex,
    ) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or("lengua's signing key is missing; call check again")?;
        let lock = lock::read(folder.path(), Some(&key))
            .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
            .ok_or("lengua.lock is missing; call check again")?;
        match &lock.tests {
            Some(stamp) if stamp.sha256 == *tests => {
                if lock.request.tests.is_some() {
                    return Ok(());
                }
                let mut lock = lock;
                lock.request.tests = Some(tests.clone());
                lock::write(folder.path(), &lock, &key).map_err(|error| {
                    format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE)
                })
            }
            _ => Err(format!(
                "{TESTS_FILE} changed while the code was being checked; call check again."
            )),
        }
    }

    /// Records checked code in the lock, provided spec, tests and code didn't change while being
    /// checked.
    async fn stamp_code(
        &self,
        folder: &ProgramFolder,
        spec: &Sha256Hex,
        tests: &Sha256Hex,
        code: CodeStamp,
    ) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        let changed = || {
            format!(
                "{SPEC_FILE}, {TESTS_FILE} or {CODE_FILE} changed while the code was being \
                 checked, so nothing was recorded; call check again."
            )
        };
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or_else(changed)?;
        let lock = lock::read(folder.path(), Some(&key))
            .ok()
            .flatten()
            .ok_or_else(changed)?;
        let unchanged = lock.spec.sha256 == *spec
            && lock.tests.as_ref().map(|stamp| &stamp.sha256) == Some(tests)
            && Sha256Hex::of(read_spec(folder)?.as_bytes()) == *spec
            && Sha256Hex::of(read_tests(folder)?.as_bytes()) == *tests
            && Sha256Hex::of(read_code(folder)?.as_bytes()) == code.sha256;
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
        bounded: Vec<(String, usize)>,
        solvers: String,
    ) -> bool {
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
    /// spec.md since the build's first request.
    spec: Option<String>,
    /// tests.toml since code first ran in the build.
    tests: Option<String>,
    /// The test changes that test less, by lengua's own comparison.
    tests_less: Vec<String>,
}

/// The next step while a job runs: call again.
fn again(folder: &ProgramFolder) -> String {
    format!("Call check again with path {folder} to wait for it; nothing needs to change.")
}

/// What a proven block's proof covered, in a line.
fn proven_detail(
    block: &testfile::Generated,
    shapes: usize,
    paths: usize,
    questions: usize,
    alone: Option<String>,
    bound: Option<prove::Bound>,
    cut_any_way: bool,
) -> String {
    let mut detail = format!(
        "proven for every input {} ({}, {} through the machine code), {}",
        bound.as_ref().map_or_else(
            || "in range".to_owned(),
            |bound| {
                let unit = if bound.characters {
                    "characters"
                } else {
                    "bytes"
                };
                format!("with text up to {} {unit}", bound.bytes)
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
    if let Some(bound) = &bound {
        let _ = write!(
            detail,
            "; longer texts are tested, not proven: {}",
            bound.why
        );
    }
    let networked = !block.server.is_empty() || !block.client.is_empty();
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
    detail
}

/// A test's cost in one line: medians only.
fn cost_line(cost: &testrun::Cost) -> String {
    let mut line = format!("{}: {}", cost.test, counted(cost.runs, "run"));
    if let Some(instructions) = cost.instructions {
        let _ = write!(line, ", {} instructions each", instructions.median);
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

/// The machine in one line: its cores, and the CPU extensions main.s may use.
fn machine_line(machine: &Machine) -> String {
    let features = match serde_json::to_value(machine.features) {
        Ok(serde_json::Value::Object(features)) => features
            .into_iter()
            .filter(|(_, present)| *present == serde_json::Value::Bool(true))
            .map(|(name, _)| name.replace("sse4_", "sse4."))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    format!(
        "{}-{}, {}; main.s may use the base instructions{}",
        machine.os,
        machine.arch,
        counted(machine.cores, "core"),
        if features.is_empty() {
            String::new()
        } else {
            format!(" and {}", features.join(", "))
        }
    )
}

/// The program folder `path` names: the folder itself or its spec.md.
fn folder_of(path: &str) -> Result<ProgramFolder, String> {
    let path = path.trim();
    match Path::new(path).file_name() {
        Some(name) if name == SPEC_FILE => ProgramFolder::new(
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

fn read_spec(folder: &ProgramFolder) -> Result<String, String> {
    read_optional(folder, SPEC_FILE)?.ok_or_else(|| {
        format!("There's no {SPEC_FILE} in {folder}. Write it with the write-spec skill first.")
    })
}

fn read_tests(folder: &ProgramFolder) -> Result<String, String> {
    read_optional(folder, TESTS_FILE)?.ok_or_else(|| {
        format!("There's no {TESTS_FILE} in {folder}. Write it with the write-tests skill.")
    })
}

fn read_code(folder: &ProgramFolder) -> Result<String, String> {
    read_optional(folder, CODE_FILE)?.ok_or_else(|| {
        format!("There's no {CODE_FILE} in {folder}. Write it with the write-assembly skill.")
    })
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
        Ok(()) => toolchain.link(&object, &executable).await,
        Err(error) => Err(error),
    };
    built.map_err(|error| format!("{CODE_FILE} doesn't build: {error}"))?;
    let bytes = std::fs::read(&executable)
        .map_err(|error| format!("couldn't read the built program: {error}"))?;
    Ok(Build {
        folder: Arc::new(folder),
        executable,
        bytes,
        toolchain: format!(
            "llvm-mc {}, ld.lld {}",
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

fn supported(machine: &Machine) -> Result<Profile, String> {
    machine.profile.ok_or_else(|| {
        format!(
            "lengua can't build programs on {}-{} yet.",
            machine.os, machine.arch
        )
    })
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

/// Saves a recorded spec.md or tests.toml under its hash, written atomically.
fn write_stamped_copy(data_dir: &DataDir, file: &Sha256Hex, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let path = stamped_copy(data_dir.path(), file);
    if std::fs::read(&path).is_ok_and(|kept| Sha256Hex::of(&kept) == *file) {
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

fn spec_problems(folder: &ProgramFolder, problems: &[spec::Problem]) -> String {
    let mut text = format!("{folder}/{SPEC_FILE} has problems; call check:\n");
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
    if machine.profile.is_none() {
        return Some(format!(
            "Blocked: {}-{} isn't supported yet. lengua currently builds programs for linux-x86_64.",
            machine.os, machine.arch
        ));
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

    #[test]
    fn a_check_may_name_the_folder_or_its_spec() {
        let folder = folder_of("/home/you/add/spec.md").expect("a spec path");
        assert_eq!(folder.to_string(), "/home/you/add");
        let folder = folder_of(" /home/you/add ").expect("a folder path");
        assert_eq!(folder.to_string(), "/home/you/add");
        assert!(folder_of("add/spec.md").is_err());
    }

    #[test]
    fn the_machine_fits_in_a_line() {
        let line = machine_line(&Machine::detect());
        assert!(line.contains("core"), "{line}");
        assert!(!line.contains('\n'), "{line}");
        assert!(!line.contains("sse4_"), "{line}");
    }
}
