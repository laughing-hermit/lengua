//! The lengua MCP server: tool definitions and server instructions.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use crate::checker;
use crate::data_dir::DataDir;
use crate::expr;
use crate::hash::Sha256Hex;
use crate::lock::{self, CodeStamp, Key, Lock, ProofStamp, SpecStamp, TestsStamp};
use crate::platform::{CpuFeatures, Machine, Profile};
use crate::prove;
use crate::runner::{self, End, Job, Limits, Net, Started};
use crate::spec;
use crate::state::{
    CODE_FILE, LockStatus, ProgramFolder, SPEC_FILE, Snapshot, State, StepContext, TESTS_FILE,
    stamped_copy,
};
use crate::testdiff;
use crate::testfile;
use crate::testrun;
use crate::toolchain::Toolchain;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua builds programs as folders holding spec.md (the source \
of truth), tests.toml, main.s and lengua.lock. Order: spec, then tests, then code. Write spec.md \
from the user's request and record it with record_spec and their words; don't wait for an \
approval. Never edit lengua.lock. Run programs only with the run tool. Pass absolute folder \
paths. When unsure, call status and follow its next step; the write-spec, write-tests and \
write-assembly skills explain each file.";

/// The longest request the lock records: the user's words, not a whole conversation.
const MAX_REQUEST_CHARS: usize = 2000;

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
}

/// A server `run` started in the background.
struct Background {
    started: Started,
    /// The build folder of the executable it started from.
    _build: Arc<tempfile::TempDir>,
    /// Which start this is, so an old timer never stops a newer one.
    id: u64,
    assertions: BTreeMap<i64, usize>,
    fingerprint: String,
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
const BACKGROUND_LIMIT: std::time::Duration = std::time::Duration::from_secs(600);

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
        }
    }
}

/// Input for `status`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct StatusParams {
    /// Absolute path of a program folder, to report that program's state. Leave it out to check
    /// only the machine.
    #[serde(default)]
    pub path: Option<String>,
}

/// Input for tools that work on one program folder.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProgramParams {
    /// Absolute path of the program folder, the folder that holds spec.md.
    pub path: String,
}

/// Input for `record_spec`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordParams {
    /// Absolute path of the program folder, or of its spec.md.
    pub path: String,
    /// The user's own words that asked for this program or this change, quoted exactly, such as
    /// "Make me a program that adds two numbers".
    pub request: String,
}

/// Everything `status` reports: only what the agent can't find out for itself.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StatusReport {
    /// The lengua server version.
    pub server_version: String,
    /// What the checker allows on this machine.
    pub machine: MachineReport,
    /// The program's state, when a path was given.
    pub program: Option<ProgramReport>,
    /// The single next thing to do. It starts with "Blocked" when lengua can't build programs
    /// here, such as when llvm-mc or ld.lld is missing.
    pub next_step: String,
}

/// What the checker allows on this machine.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MachineReport {
    /// The platform profile, e.g. `linux-x86_64`, or null when unsupported.
    pub profile: Option<String>,
    /// How many threads can run at once; a spec allows at most this many.
    pub cores: usize,
    /// The CPU extensions main.s may use: the checker refuses instructions from any other.
    pub features: CpuFeatures,
}

/// Program part of the status report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProgramReport {
    /// The program folder.
    pub path: String,
    /// Workflow state: no_spec, lock_invalid, spec_unrecorded, tests_missing,
    /// tests_unchecked, code_missing, code_unchecked or ready.
    pub state: String,
    /// The label tests.toml and main.s must carry: `sha256:<hex>` of the current spec.md.
    pub spec_label: Option<String>,
    /// The program's files.
    pub files: FilesReport,
    /// Whether lengua.lock can be trusted.
    pub lock: LockReport,
    /// The ids of the generated tests proven for every input in range on the checked build:
    /// empty until `prove` runs on it, and whenever spec.md, tests.toml or main.s changed since.
    pub proven: Vec<String>,
    /// Which solvers checked those proofs.
    pub proven_by: Option<String>,
    /// The user's requests in this build, as recorded, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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

/// The three files of a program.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FilesReport {
    /// spec.md.
    pub spec: FileReport,
    /// tests.toml.
    pub tests: FileReport,
    /// main.s.
    pub code: FileReport,
}

/// One program file.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FileReport {
    /// Whether the file exists.
    pub exists: bool,
    /// SHA-256 of its bytes.
    pub sha256: Option<String>,
    /// Whether lengua.lock records exactly this version as recorded or checked.
    pub stamped: bool,
}

/// The lock part of a program report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LockReport {
    /// `missing`, `valid` or `invalid`.
    pub state: String,
    /// Why the lock can't be trusted, when it's invalid.
    pub problem: Option<String>,
    /// One hash over spec, tests and code, once all three are checked.
    pub fingerprint: Option<String>,
}

/// What `record_spec` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RecordReport {
    /// Whether spec.md is recorded now.
    pub recorded: bool,
    /// The label tests.toml and main.s must carry: `sha256:<hex>` of spec.md.
    pub spec_label: String,
    /// The user's requests in this build, as recorded, oldest first.
    pub requests: Vec<String>,
    /// How spec.md changed since the build's first request, as `-`/`+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_changed: Option<String>,
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
}

/// What `check_tests` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestsReport {
    /// Whether tests.toml is checked and recorded now.
    pub checked: bool,
    /// SHA-256 of the checked tests.toml.
    pub tests_sha256: String,
    /// Number of examples.
    pub examples: usize,
    /// Number of generated blocks.
    pub generated_blocks: usize,
    /// Runs of all generated blocks together, each with fresh inputs.
    pub generated_runs: u64,
    /// How tests.toml changed since code first ran in this build, as `-`/`+` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_changed: Option<String>,
    /// lengua's own list of the test changes that test less than before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests_less: Vec<String>,
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
}

/// What `check_code` reports when main.s follows every rule and passes every test.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CodeReport {
    /// Whether main.s follows every rule of the dialect.
    pub rules_passed: bool,
    /// Whether every test passed.
    pub tests_passed: bool,
    /// Whether lengua.lock now records this main.s.
    pub stamped: bool,
    /// Functions, assert_fail excluded.
    pub functions: usize,
    /// Assertions in the program.
    pub assertions: usize,
    /// The deepest the stack can get over every call path, in bytes.
    pub max_stack_bytes: u64,
    /// Every `# unsafe:` block, as `main.s:START-END: reason`.
    pub unsafe_blocks: Vec<String>,
    /// Program runs made by the tests.
    pub test_runs: usize,
    /// The generated tests that ran every value in their ranges, not a sample, such as
    /// "generated B1: every value of n in its range (all 201)".
    pub every_value: Vec<String>,
    /// The seed that drew this check's random inputs.
    pub seed: String,
    /// What each test cost: instructions, cycles and CPU time (lower is faster), when the CPU's
    /// counters are available.
    pub performance: Vec<testrun::Cost>,
    /// One hash over spec, tests and code.
    pub fingerprint: String,
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
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
}

/// One generated test's proof.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProofReport {
    /// The test, such as `generated B1`.
    pub test: String,
    /// Whether the program is right for every input in the test's ranges, not just the
    /// sampled ones.
    pub proven: bool,
    /// What was proven, or why it wasn't.
    pub detail: String,
}

/// What `prove` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProveReport {
    /// Each generated test's proof.
    pub proofs: Vec<ProofReport>,
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
}

/// The longest one `prove` call may take.
const PROOF_TIME: std::time::Duration = std::time::Duration::from_secs(300);

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
    /// How many seconds the program may run, 1 to 600; 10 when not given.
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
    /// The fingerprint of the spec, tests and code that ran.
    pub fingerprint: String,
    /// Where a server is taking connections, while it runs in the background.
    pub serving: Option<String>,
}

/// What `run` reports about a program that has ended.
fn ended_report(
    result: &runner::Run,
    assertions: &BTreeMap<i64, usize>,
    limits: Limits,
    fingerprint: String,
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
        fingerprint,
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
        fingerprint: String,
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
        let id = STARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        background.insert(
            folder.path().to_path_buf(),
            Background {
                started,
                _build: build,
                id,
                assertions,
                fingerprint: fingerprint.clone(),
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
            fingerprint,
            serving: Some(address),
        })
    }
}

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "status",
        title = "Status",
        description = "Shows what lengua allows on this machine (the CPU extensions main.s may \
            use and the cores a spec may ask for) and, given a program folder, what that program \
            needs next: each file's hash, whether lengua.lock records it, the label tests.toml \
            and main.s must carry, any problems in an unrecorded spec.md, which tests prove has \
            proven for every input, the user's requests, and what changed in spec.md and \
            tests.toml since them. Ends with the single next step to take, or why lengua can't \
            build programs here. path: absolute path of a program folder (optional).",
        annotations(title = "Status", read_only_hint = true, open_world_hint = false)
    )]
    async fn status(
        &self,
        Parameters(params): Parameters<StatusParams>,
    ) -> Result<Json<StatusReport>, String> {
        let machine = Machine::detect();
        let toolchain = Toolchain::discover().await;
        let program = match params.path.as_deref() {
            Some(path) => Some(self.program_report(&ProgramFolder::new(path)?)?),
            None => None,
        };
        let next_step = match (
            machine_blocker(&machine, toolchain.as_ref().err()),
            &program,
        ) {
            (Some(blocker), _) => blocker,
            (None, Some((_, step))) => step.clone(),
            (None, None) => READY_STEP.to_owned(),
        };
        Ok(Json(StatusReport {
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
            machine: MachineReport {
                profile: machine.profile.map(|profile| profile.name().to_owned()),
                cores: machine.cores,
                features: machine.features,
            },
            program: program.map(|(report, _)| report),
            next_step,
        }))
    }

    #[tool(
        name = "record_spec",
        title = "Record spec",
        description = "Records <path>/spec.md in lengua.lock as the source of truth, with the \
            user's words that asked for this program or this change. Call it right after writing \
            or changing spec.md; don't wait for an approval. It first checks the spec's format \
            and returns any problems. A changed spec makes the tests and code be checked again, \
            and every change after the build's first request is shown, never hidden. path: \
            absolute path of the program folder or of its spec.md; request: the user's words, \
            quoted exactly, never made up.",
        annotations(
            title = "Record spec",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn record_spec(
        &self,
        Parameters(params): Parameters<RecordParams>,
    ) -> Result<Json<RecordReport>, String> {
        let folder = folder_of(&params.path)?;
        let words = params.request.trim();
        if words.is_empty() {
            return Err(
                "request must quote the user's own words that asked for this program or this \
                 change, exactly as they wrote them. Never make them up: if the user hasn't asked \
                 for a program yet, ask them what it should do."
                    .to_owned(),
            );
        }
        if words.chars().count() > MAX_REQUEST_CHARS {
            return Err(format!(
                "request holds more than {MAX_REQUEST_CHARS} characters; quote only the user's \
                 words that ask for the program or the change."
            ));
        }
        let _writing = self.lock_writes.lock().await;
        let text = read_spec(&folder)?;
        let spec = spec::parse(&text).map_err(|problems| spec_problems(&folder, &problems))?;
        let current = Sha256Hex::of(text.as_bytes());
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        // Every recorded version is kept by its hash, so later changes can be shown.
        write_stamped_copy(&self.data_dir, &current, &text)?;
        let previous = lock::read(folder.path(), Some(&key)).ok().flatten();
        let fresh = || {
            format!(
                "lengua recorded {SPEC_FILE} for \"{}\" with the user's request. It's the source \
                 of truth the tests and code are checked against.",
                spec.title
            )
        };
        let (lock, message) = match previous {
            Some(lock)
                if lock.request.words.iter().any(|known| known == words)
                    && lock.spec.sha256 == current =>
            {
                (
                    lock,
                    format!("{SPEC_FILE} was already recorded for this request; nothing changed."),
                )
            }
            // The spec was revised under a request it already has: it's checked again, and
            // the change is shown.
            Some(lock) if lock.request.words.iter().any(|known| known == words) => (
                Lock {
                    spec: SpecStamp {
                        sha256: current.clone(),
                    },
                    tests: None,
                    code: None,
                    proofs: None,
                    ..lock
                },
                format!(
                    "{SPEC_FILE} changed under the same request, so lengua records it; the tests \
                     and code are checked against it again. spec_changed shows every change \
                     since the build's first request: tell the user each, and why, in your final \
                     report."
                ),
            ),
            // New words in the middle of a build join it, so nothing changed before them drops
            // out of what is shown.
            Some(lock) if lock.code.is_none() => {
                let unchanged = lock.spec.sha256 == current;
                let mut request = lock.request;
                if request.words.len() >= lock::MAX_REQUESTS {
                    request.words.remove(1);
                }
                request.words.push(words.to_owned());
                (
                    Lock {
                        request,
                        spec: SpecStamp {
                            sha256: current.clone(),
                        },
                        tests: if unchanged { lock.tests } else { None },
                        code: None,
                        proofs: None,
                    },
                    format!(
                        "lengua recorded {SPEC_FILE} with the user's new words. The build they \
                         join isn't finished, so changes are still shown from the spec it started \
                         with, and requests lists every request in it."
                    ),
                )
            }
            // A new request after a finished build starts a new build.
            // With the spec unchanged, its checked tests and code stay, and the tests the code
            // ran against are where test changes are counted from.
            Some(lock) if lock.spec.sha256 == current => {
                let mut request = Lock::requested(current.clone(), words.to_owned()).request;
                request.tests = lock.tests.as_ref().map(|tests| tests.sha256.clone());
                (Lock { request, ..lock }, fresh())
            }
            _ => (Lock::requested(current.clone(), words.to_owned()), fresh()),
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        let changes = self.changes(&folder, &lock);
        Ok(Json(RecordReport {
            recorded: true,
            spec_label: current.label(),
            requests: lock.request.words.clone(),
            spec_changed: changes.spec,
            message,
            next_step: self.program_state(&folder)?.1,
        }))
    }

    #[tool(
        name = "check_tests",
        title = "Check tests",
        description = "Checks <path>/tests.toml against the recorded spec and records it in \
            lengua.lock. A wrong test can be fixed at any time: each version is checked against \
            the spec again, and after code has run, the reply shows what changed and which \
            changes test less. It checks the [meta] spec label, one example per behavior holding the spec's values, a [[generated]] block \
            for every behavior with inputs, generator ranges taken from the spec's Purpose, \
            formulas that reproduce each example without overflow or division by zero, and \
            output that depends on the inputs. Returns every problem at once. path: absolute path \
            of the program folder.",
        annotations(
            title = "Check tests",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn check_tests(
        &self,
        Parameters(params): Parameters<ProgramParams>,
    ) -> Result<Json<TestsReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let (state, next_step) = self.program_state(&folder)?;
        let already_checked = match state {
            State::TestsUnchecked => false,
            State::CodeMissing | State::CodeUnchecked | State::Ready => true,
            State::TestsMissing => {
                return Err(format!(
                    "There's no {TESTS_FILE} in {folder} yet. {next_step}"
                ));
            }
            State::NoSpec | State::SpecUnrecorded | State::LockInvalid(_) => {
                return Err(format!(
                    "Tests are checked against a recorded spec, and {folder} has none yet. \
                     {next_step}"
                ));
            }
        };
        let spec_text = read_spec(&folder)?;
        let spec = spec::parse(&spec_text).map_err(|problems| spec_problems(&folder, &problems))?;
        let spec_hash = Sha256Hex::of(spec_text.as_bytes());
        let tests_text = read_tests(&folder)?;
        let tests_hash = Sha256Hex::of(tests_text.as_bytes());
        let suite = testfile::check(&tests_text, &spec, &spec_hash.label())
            .map_err(|problems| tests_problems(&folder, &problems))?;
        if !already_checked {
            self.stamp_tests(&folder, &spec_hash, &tests_hash, &tests_text)
                .await?;
        }
        let changes = self.current_changes(&folder);
        let message = if already_checked {
            format!("{TESTS_FILE} was already checked; nothing changed.")
        } else if changes.tests.is_some() || !changes.tests_less.is_empty() {
            format!(
                "{TESTS_FILE} changed after code ran against it. It still matches {SPEC_FILE}, so \
                 lengua records it, and the code is checked against it again. tests_changed \
                 shows the changes since code first ran, and tests_less the ones that test less: \
                 tell the user each, and why it's right, in your final report."
            )
        } else {
            format!("{TESTS_FILE} covers every behavior in {SPEC_FILE}, so lengua records it.")
        };
        Ok(Json(TestsReport {
            checked: true,
            tests_sha256: tests_hash.to_string(),
            examples: suite.examples.len(),
            generated_blocks: suite.generated.len(),
            generated_runs: suite
                .generated
                .iter()
                .map(|block| u64::try_from(block.run_count()).unwrap_or(u64::MAX))
                .sum(),
            tests_changed: changes.tests,
            tests_less: changes.tests_less,
            message,
            next_step: self.program_state(&folder)?.1,
        }))
    }

    #[tool(
        name = "check_code",
        title = "Check code",
        description = "Checks <path>/main.s against lengua's rules and every test, then records \
            it in lengua.lock. It assembles and links the program, reads the machine code back \
            and matches it to main.s line by line, and checks every lengua Power of 10 rule; \
            nothing runs before the rules pass. Then it runs every test, generated tests with \
            fresh random inputs, and reports what each test cost. Needs a recorded spec and \
            checked tests, and main.s's first line must be the spec label. Returns every \
            violation or failed test with its line. path: absolute path of the program folder.",
        annotations(
            title = "Check code",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn check_code(
        &self,
        Parameters(params): Parameters<ProgramParams>,
    ) -> Result<Json<CodeReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let (state, next_step) = self.program_state(&folder)?;
        match state {
            State::CodeUnchecked | State::Ready => {}
            State::CodeMissing => {
                return Err(format!(
                    "There's no {CODE_FILE} in {folder} yet. {next_step}"
                ));
            }
            _ => {
                return Err(format!(
                    "Code is checked against a recorded spec and checked tests, which {folder} \
                     doesn't have yet. {next_step}"
                ));
            }
        }
        let spec_text = read_spec(&folder)?;
        let spec = spec::parse(&spec_text).map_err(|problems| spec_problems(&folder, &problems))?;
        let spec_hash = Sha256Hex::of(spec_text.as_bytes());
        let label = spec_hash.label();
        let tests_text = read_tests(&folder)?;
        let tests_hash = Sha256Hex::of(tests_text.as_bytes());
        let code = read_code(&folder)?;
        let header = format!("# lengua: spec {label}");
        let first_line = code.lines().next().unwrap_or_default();
        if first_line != header {
            return Err(format!(
                "{CODE_FILE}'s first line must be `{header}`, the label of the recorded spec, but \
                 it's `{first_line}`. Set it, after making sure the code implements the current \
                 spec."
            ));
        }
        let (source, layout) = checker::check_layout(&code);
        if !layout.passed() {
            return Err(violations_text(&folder, &layout));
        }
        let machine = Machine::detect();
        let profile = supported(&machine)?;
        let built = build(&code, profile).await?;
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
            return Err(violations_text(&folder, &report));
        }
        let facts = report.facts.clone().unwrap_or_default();
        let suite = testfile::check(&tests_text, &spec, &label)
            .map_err(|problems| tests_problems(&folder, &problems))?;
        let seed = random_seed()?;
        let every_value: Vec<(String, String)> = suite
            .generated
            .iter()
            .filter_map(|block| {
                block
                    .every_value_text()
                    .map(|text| (block.id.to_string(), text))
            })
            .collect();
        self.note_first_run(&folder, &tests_hash).await?;
        let syscalls = profile.allowed_syscalls(&spec.syscalls, spec.threads.is_some());
        let outcome = testrun::run_tests(
            &suite,
            &self.launcher,
            &built.executable,
            &source,
            seed,
            &syscalls,
            &spec.network,
            testrun::workers(spec.threads),
        )
        .await
        .map_err(|error| format!("couldn't run the program: {error}"))?;
        if !outcome.passed() {
            return Err(failures_text(&folder, &outcome));
        }
        let stamp = CodeStamp {
            sha256: Sha256Hex::of(code.as_bytes()),
            executable_sha256: Sha256Hex::of(&built.bytes),
            toolchain: built.toolchain.clone(),
            max_stack_bytes: facts.max_stack_bytes,
            unsafe_blocks: u32::try_from(report.unsafe_blocks.len()).unwrap_or(u32::MAX),
        };
        let fingerprint = self
            .stamp_code(&folder, &spec_hash, &tests_hash, stamp)
            .await?;
        let changes = self.current_changes(&folder);
        let shown = if changes.spec.is_some() || changes.tests.is_some() {
            " spec_changed and tests_changed show what changed after the user's request, and \
             tests_less the test changes that test less: tell the user each, and why, in your \
             final report."
        } else {
            ""
        };
        Ok(Json(CodeReport {
            rules_passed: true,
            tests_passed: true,
            stamped: true,
            functions: facts.functions,
            assertions: facts.assertions,
            max_stack_bytes: facts.max_stack_bytes,
            unsafe_blocks: report
                .unsafe_blocks
                .iter()
                .map(|block| format!("main.s:{}-{}: {}", block.start, block.end, block.reason))
                .collect(),
            test_runs: outcome.runs,
            every_value: every_value
                .iter()
                .map(|(id, text)| format!("generated {id}: {text}"))
                .collect(),
            seed: seed.to_string(),
            performance: outcome.costs,
            fingerprint: fingerprint.to_string(),
            requests: changes.requests,
            spec_changed: changes.spec,
            tests_changed: changes.tests,
            tests_less: changes.tests_less,
            message: format!(
                "{CODE_FILE} follows every rule and passed all {} test runs.{} lengua.lock now \
                 records spec, tests and code together, so the program can run.{shown}",
                outcome.runs,
                every_value
                    .iter()
                    .map(|(id, text)| format!(" Generated {id} tried {text}."))
                    .collect::<String>()
            ),
            next_step: self.program_state(&folder)?.1,
        }))
    }

    #[tool(
        name = "run",
        title = "Run program",
        description = "Runs the checked program in <path> with args and stdin. It runs only \
            when lengua.lock shows that spec.md, tests.toml and main.s are exactly the versions \
            that were recorded and checked, and a fresh build of main.s is byte for byte the \
            executable check_code tested. The program gets an empty environment, 10 seconds and \
            8 MiB of output, and the kernel lets it make only the system calls its spec allows, \
            connect only to the ports it names, and touch files only inside its working folder: \
            an empty temporary folder, or `folder`. A server starts in the background on its \
            spec's port and run returns its address; call run with stop: true to stop it and get \
            what it printed (it also stops after ten minutes or when the session ends). path: \
            absolute path of the program folder; args: the program's arguments; stdin: text for \
            its standard input; folder: absolute path of a folder of the user's files to run in, \
            naming the files in args relative to it (\"notes.txt\", not \"/home/me/notes.txt\"); \
            stdin_file: a file in folder to read as standard input instead of stdin, however \
            large; timeout_seconds: up to 600 for long runs on large data (10 by default); stop: \
            true to stop the server run started.",
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
            return Ok(Json(ended_report(
                &result,
                &entry.assertions,
                limits,
                entry.fingerprint,
            )));
        }
        let (state, next_step) = self.program_state(&folder)?;
        if state != State::Ready {
            return Err(format!(
                "lengua runs a program only when its spec, tests and code all match \
                 lengua.lock, and {folder} doesn't. {next_step}"
            ));
        }
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or("lengua's signing key is missing; call status")?;
        let lock = lock::read(folder.path(), Some(&key))
            .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
            .ok_or("lengua.lock is missing; call status")?;
        let (Some(stamp), Some(fingerprint)) = (lock.code.as_ref(), lock.fingerprint()) else {
            return Err(format!("{CODE_FILE} isn't checked; call check_code first."));
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
                "This build of {CODE_FILE} differs from the one check_code tested (built with {}; \
                 now {}), so it won't run. Call check_code with path {folder} to test this build.",
                stamp.toolchain, built.toolchain
            ));
        }
        let seconds = params.timeout_seconds.unwrap_or(10);
        if !(1..=600).contains(&seconds) {
            return Err("timeout_seconds goes from 1 to 600".into());
        }
        let limits = Limits {
            timeout: std::time::Duration::from_secs(seconds),
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
                .serve_in_background(
                    &folder,
                    job,
                    Arc::clone(&built.folder),
                    assertions,
                    fingerprint.to_string(),
                )
                .await
                .map(Json);
        }
        let result = runner::run(&self.launcher, job, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        Ok(Json(ended_report(
            &result,
            &assertions,
            limits,
            fingerprint.to_string(),
        )))
    }

    #[tool(
        name = "prove",
        title = "Prove for every input",
        description = "Proves that the checked program in <path> prints exactly what its tests' \
            formulas say for every input in range, not only the sampled ones: for each generated \
            test whose inputs are whole numbers, decimals, choices or short texts, and whose \
            outputs are text, numbers or files, it runs the machine code on symbols and asks two \
            independent SMT solvers, z3 and cvc5, whether any input could make the output differ. Call it after check_code passes. It \
            can take a few minutes. A counterexample is confirmed with a real run and reported \
            as a failure. lengua.lock records what was proven, and status shows it until the \
            spec, tests or code change. path: absolute path of the program folder.",
        annotations(
            title = "Prove for every input",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn prove(
        &self,
        Parameters(params): Parameters<ProgramParams>,
    ) -> Result<Json<ProveReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let (state, next_step) = self.program_state(&folder)?;
        if state != State::Ready {
            return Err(format!(
                "lengua proves a program after check_code has recorded it, and {folder} isn't \
                 ready. {next_step}"
            ));
        }
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or("lengua's signing key is missing; call status")?;
        let lock = lock::read(folder.path(), Some(&key))
            .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
            .ok_or("lengua.lock is missing; call status")?;
        let stamp = lock
            .code
            .as_ref()
            .ok_or("main.s isn't checked; call check_code first.")?;
        let spec_text = read_spec(&folder)?;
        let spec = spec::parse(&spec_text).map_err(|problems| spec_problems(&folder, &problems))?;
        let suite = testfile::check(
            &read_tests(&folder)?,
            &spec,
            &Sha256Hex::of(spec_text.as_bytes()).label(),
        )
        .map_err(|problems| tests_problems(&folder, &problems))?;
        let code = read_code(&folder)?;
        let source = checker::check_layout(&code).0;
        let machine_info = Machine::detect();
        let profile = supported(&machine_info)?;
        let built = build(&code, profile).await?;
        if Sha256Hex::of(&built.bytes) != stamp.executable_sha256 {
            return Err(format!(
                "This build of {CODE_FILE} differs from the one check_code tested, so lengua \
                 won't prove it. Call check_code with path {folder} first."
            ));
        }
        let decoded = checker::decode::read(&built.bytes)?;
        let syscalls = profile.allowed_syscalls(&spec.syscalls, spec.threads.is_some());
        let deadline = std::time::Instant::now() + PROOF_TIME;
        let mut proofs = Vec::new();
        let mut wrong = false;
        // The ids proven, how many the solvers proved, and why any rested on one solver.
        let mut proven_ids = Vec::new();
        let mut solved = 0;
        let mut alone_reason: Option<String> = None;
        let allows = |call: spec::Syscall| spec.syscalls.contains(&call);
        let blocks = suite.generated.len();
        for (index, block) in suite.generated.iter().enumerate() {
            let test = format!("generated {}", block.id);
            // Each block gets its share of the time left, so one big block can't starve the
            // rest; time a block doesn't use passes on to the next.
            let now = std::time::Instant::now();
            let share = deadline.saturating_duration_since(now)
                / u32::try_from(blocks - index).unwrap_or(u32::MAX).max(1);
            let block_deadline = now + share;
            // Every value of a block's variables is every input only when the program reads
            // nothing else: no random bytes or clock readings the test doesn't supply, and no
            // `now` in the expected results.
            let kernel_values = (allows(spec::Syscall::Getrandom) && block.random.is_none())
                || (allows(spec::Syscall::ClockGettime) && block.clock.is_empty());
            if spec.network.is_used() {
                proofs.push(ProofReport {
                    test,
                    proven: false,
                    detail: "tested, not proven: the proof doesn't follow a program over the \
                             network yet"
                        .into(),
                });
                continue;
            }
            if let Some(every) = block
                .every_value_text()
                .filter(|_| !kernel_values && !block.uses_now())
            {
                proven_ids.push(block.id.to_string());
                proofs.push(ProofReport {
                    test,
                    proven: true,
                    detail: format!("check_code tried {every}"),
                });
                continue;
            }
            // The model must agree with the real program on inputs it passed before it's
            // trusted to cover the rest.
            let samples: Vec<testfile::Case> =
                block.cases(1).take(24).filter_map(Result::ok).collect();
            let (machine, owned) = (decoded.clone(), block.clone());
            let verdict = tokio::task::spawn_blocking(move || {
                prove::agrees(&machine, &samples).map(|()| {
                    prove::prove(
                        &machine,
                        &owned,
                        prove::Budget {
                            deadline: block_deadline,
                            max_paths: 256,
                            max_shapes: 2_000_000,
                            question_ms: 20_000,
                        },
                    )
                })
            })
            .await
            .map_err(|error| format!("the proof stopped: {error}"))?;
            let (proven, detail) = match verdict {
                Err(reason) => (false, format!("tested, not proven: {reason}")),
                Ok(prove::Verdict::Proven {
                    shapes,
                    paths,
                    questions,
                    alone,
                }) => (
                    {
                        proven_ids.push(block.id.to_string());
                        if questions > 0 {
                            solved += 1;
                            if alone_reason.is_none() {
                                alone_reason.clone_from(&alone);
                            }
                        }
                        true
                    },
                    format!(
                        "proven for every input in range ({shapes} input combinations, {paths} \
                         paths through the machine code), {}",
                        match (questions, alone) {
                            (0, _) => "every step settled by the terms themselves, so no solver \
                                       was needed"
                                .to_owned(),
                            (_, None) => "with each solver question confirmed by two independent \
                                          solvers, z3 and cvc5"
                                .to_owned(),
                            (_, Some(reason)) => format!(
                                "by one solver alone ({reason}; with both z3 and cvc5 installed, \
                                 each step is confirmed twice)"
                            ),
                        }
                    ),
                ),
                Ok(prove::Verdict::NotProven(reason)) => {
                    (false, format!("tested, not proven: {reason}"))
                }
                Ok(prove::Verdict::Counterexample(values)) => {
                    let inputs: expr::Env = values.into_iter().collect();
                    let described = testfile::describe_inputs(&inputs);
                    match block.case(inputs, crate::expect::now()) {
                        Ok(case) => match testrun::check_case(
                            &self.launcher,
                            &built.executable,
                            &source,
                            &case,
                            &syscalls,
                            &spec.network,
                        )
                        .await
                        .map_err(|error| format!("couldn't run the program: {error}"))?
                        {
                            Some(problem) => {
                                wrong = true;
                                (
                                    false,
                                    format!(
                                        "the proof found inputs the tests didn't try where main.s \
                                         is wrong: with {described}: {problem}"
                                    ),
                                )
                            }
                            // A real run draws its own random bytes and reads its own
                            // clock, so it can't replay what the proof found.
                            None if kernel_values => (
                                false,
                                format!(
                                    "tested, not proven: with {described}, the prover \
                                         found random bytes or a clock reading for which \
                                         main.s's output breaks the test, which a real run \
                                         can't replay; check how main.s turns them into output"
                                ),
                            ),
                            None => (
                                false,
                                format!(
                                    "tested, not proven: the prover's model disagreed with a \
                                     real run on {described}"
                                ),
                            ),
                        },
                        Err((_, field, error)) => (
                            false,
                            format!("tested, not proven: with {described}, {field} {error}"),
                        ),
                    }
                }
            };
            proofs.push(ProofReport {
                test,
                proven,
                detail,
            });
        }
        let solvers = match (solved, alone_reason) {
            (0, _) => {
                "none: check_code tried every value, or the terms settled every step".to_owned()
            }
            (_, None) => "z3 and cvc5".to_owned(),
            (_, Some(reason)) => format!("one solver alone: {reason}"),
        };
        let recorded = self
            .stamp_proofs(&folder, &stamp.executable_sha256, proven_ids, solvers)
            .await;
        let proven: Vec<&str> = proofs
            .iter()
            .filter(|proof| proof.proven)
            .map(|proof| proof.test.as_str())
            .collect();
        let message = if wrong {
            "The proof found inputs where main.s is wrong; see proofs.".to_owned()
        } else if proven.len() == proofs.len() && !proofs.is_empty() {
            "Every generated test is proven for every input in its range.".to_owned()
        } else if proven.is_empty() {
            "Nothing could be proven; the tests still pass. See proofs for why.".to_owned()
        } else {
            format!("Proven for every input in range: {}.", proven.join(", "))
        };
        let message = if recorded {
            format!("{message} lengua.lock records it.")
        } else {
            message
        };
        Ok(Json(ProveReport {
            proofs,
            message,
            next_step: if wrong {
                format!(
                    "Next: fix {folder}/{CODE_FILE} with the write-assembly skill so it handles \
                     those inputs, then call check_code and prove again."
                )
            } else {
                next_step
            },
        }))
    }
}

impl LenguaServer {
    /// The program's state and its next step.
    fn program_state(&self, folder: &ProgramFolder) -> Result<(State, String), String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        let state = State::of(&snapshot);
        let step = self.next_step(folder, &snapshot, &state);
        Ok((state, step))
    }

    /// The state's next step. An unrecorded spec.md is checked first, so only a spec lengua
    /// accepts is recorded.
    fn next_step(&self, folder: &ProgramFolder, snapshot: &Snapshot, state: &State) -> String {
        if *state == State::SpecUnrecorded
            && let Ok(text) = read_spec(folder)
            && let Err(problems) = spec::parse(&text)
        {
            return format!(
                "Next: fix {folder}/{SPEC_FILE}:\n{}Fix them with the write-spec skill, then call \
                 record_spec.",
                problem_list(&problems)
            );
        }
        state.next_step(&StepContext {
            folder,
            spec: snapshot.spec.as_ref(),
        })
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
                        Some(testdiff::tests_diff(&before, &now)),
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
                 nothing was recorded; call check_tests again."
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
            .ok_or("lengua's signing key is missing; call status")?;
        let lock = lock::read(folder.path(), Some(&key))
            .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
            .ok_or("lengua.lock is missing; call status")?;
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
                "{TESTS_FILE} changed while the code was being checked; call check_tests, then \
                 check_code again."
            )),
        }
    }

    /// Records checked code in the lock, provided spec, tests and code didn't change while being
    /// checked. Returns the new fingerprint.
    async fn stamp_code(
        &self,
        folder: &ProgramFolder,
        spec: &Sha256Hex,
        tests: &Sha256Hex,
        code: CodeStamp,
    ) -> Result<Sha256Hex, String> {
        let _writing = self.lock_writes.lock().await;
        let changed = || {
            format!(
                "{SPEC_FILE}, {TESTS_FILE} or {CODE_FILE} changed while the code was being \
                 checked, so nothing was recorded; call check_code again."
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
        let fingerprint = lock.fingerprint().ok_or_else(changed)?;
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        Ok(fingerprint)
    }

    /// Records in the lock what `prove` proved about the build `executable`, provided the lock
    /// still records that build as the checked code. Returns whether it did.
    async fn stamp_proofs(
        &self,
        folder: &ProgramFolder,
        executable: &Sha256Hex,
        proven: Vec<String>,
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
                solvers,
            })),
            ..lock
        };
        lock::write(folder.path(), &lock, &key).is_ok()
    }

    fn program_report(&self, folder: &ProgramFolder) -> Result<(ProgramReport, String), String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        let state = State::of(&snapshot);
        let step = self.next_step(folder, &snapshot, &state);
        let lock = snapshot.valid_lock();
        let changes = lock.map_or_else(Changes::default, |lock| self.changes(folder, lock));
        // Proofs speak for the files only while all three are the ones checked.
        let proofs = lock
            .and_then(|lock| lock.proofs.as_ref())
            .filter(|_| state == State::Ready);
        let file = |current: &Option<Sha256Hex>, stamp: Option<&Sha256Hex>| FileReport {
            exists: current.is_some(),
            sha256: current.as_ref().map(ToString::to_string),
            stamped: current.is_some() && current.as_ref() == stamp,
        };
        let report = ProgramReport {
            path: folder.to_string(),
            state: state.name().to_owned(),
            spec_label: snapshot.spec.as_ref().map(Sha256Hex::label),
            files: FilesReport {
                spec: file(&snapshot.spec, lock.map(|lock| &lock.spec.sha256)),
                tests: file(
                    &snapshot.tests,
                    lock.and_then(|lock| lock.tests.as_ref())
                        .map(|tests| &tests.sha256),
                ),
                code: file(
                    &snapshot.code,
                    lock.and_then(|lock| lock.code.as_ref())
                        .map(|code| &code.sha256),
                ),
            },
            lock: LockReport {
                state: match &snapshot.lock {
                    LockStatus::Missing => "missing",
                    LockStatus::Valid(_) => "valid",
                    LockStatus::Invalid(_) => "invalid",
                }
                .to_owned(),
                problem: match &snapshot.lock {
                    LockStatus::Invalid(error) => Some(error.to_string()),
                    LockStatus::Missing | LockStatus::Valid(_) => None,
                },
                fingerprint: lock
                    .and_then(Lock::fingerprint)
                    .map(|fingerprint| fingerprint.to_string()),
            },
            proven: proofs
                .map(|proofs| proofs.proven.clone())
                .unwrap_or_default(),
            proven_by: proofs.map(|proofs| proofs.solvers.clone()),
            requests: changes.requests,
            spec_changed: changes.spec,
            tests_changed: changes.tests,
            tests_less: changes.tests_less,
        };
        Ok((report, step))
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

fn read_spec(folder: &ProgramFolder) -> Result<String, String> {
    match std::fs::read_to_string(folder.file(SPEC_FILE)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "There's no {SPEC_FILE} in {folder}. Write it with the write-spec skill first."
        )),
        Err(error) => Err(format!("couldn't read {folder}/{SPEC_FILE}: {error}")),
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
        Ok(()) => toolchain.link(&object, &executable).await,
        Err(error) => Err(error),
    };
    built.map_err(|error| {
        format!(
            "{CODE_FILE} doesn't build: {error}\nFix it with the write-assembly skill, then call \
             check_code again."
        )
    })?;
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

/// Failed tests as a message for the AI.
fn failures_text(folder: &ProgramFolder, outcome: &testrun::Outcome) -> String {
    let count = outcome.failures.len();
    let mut text = format!(
        "{folder}/{CODE_FILE} follows every rule but fails {count} test{}, so lengua.lock \
         doesn't record it:\n",
        if count == 1 { "" } else { "s" }
    );
    for failure in &outcome.failures {
        let _ = writeln!(text, "- {failure}");
    }
    text.push_str(
        "Fix main.s with the write-assembly skill, then call check_code again. Boundary values \
         run first every time; the other generated inputs are new on every call.",
    );
    text
}

fn read_code(folder: &ProgramFolder) -> Result<String, String> {
    match std::fs::read_to_string(folder.file(CODE_FILE)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "There's no {CODE_FILE} in {folder}. Write it with the write-assembly skill, then call \
             check_code."
        )),
        Err(error) => Err(format!("couldn't read {folder}/{CODE_FILE}: {error}")),
    }
}

/// The checker's findings as a message for the AI.
fn violations_text(folder: &ProgramFolder, report: &checker::Report) -> String {
    let total = report.violations.len() + report.hidden;
    let mut text = format!(
        "{folder}/{CODE_FILE} has {total} rule violation{}, so nothing ran:\n",
        if total == 1 { "" } else { "s" }
    );
    for violation in &report.violations {
        let _ = writeln!(text, "- {violation}");
    }
    if report.hidden > 0 {
        let _ = writeln!(text, "- …and {} more", report.hidden);
    }
    text.push_str(
        "Fix them with the write-assembly skill (the full rules are in its \
         references/power-of-10.md), then call check_code again.",
    );
    text
}

fn read_tests(folder: &ProgramFolder) -> Result<String, String> {
    match std::fs::read_to_string(folder.file(TESTS_FILE)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "There's no {TESTS_FILE} in {folder}. Write it with the write-tests skill, then call \
             check_tests."
        )),
        Err(error) => Err(format!("couldn't read {folder}/{TESTS_FILE}: {error}")),
    }
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

fn tests_problems(folder: &ProgramFolder, problems: &[testfile::Problem]) -> String {
    let count = problems.len();
    let mut text = format!(
        "{folder}/{TESTS_FILE} has {count} problem{}, so it wasn't checked:\n",
        if count == 1 { "" } else { "s" }
    );
    for problem in problems {
        let _ = writeln!(text, "- {problem}");
    }
    text.push_str("Fix them with the write-tests skill, then call check_tests again.");
    text
}

fn spec_problems(folder: &ProgramFolder, problems: &[spec::Problem]) -> String {
    let count = problems.len();
    format!(
        "{folder}/{SPEC_FILE} has {count} problem{} to fix before lengua can record it:\n{}Fix \
         them with the write-spec skill, then call record_spec again.",
        if count == 1 { "" } else { "s" },
        problem_list(problems)
    )
}

/// Problems as a bulleted list, one per line.
fn problem_list(problems: &[spec::Problem]) -> String {
    let mut text = String::new();
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

/// The next step when no program was named and the machine can build programs.
const READY_STEP: &str = "Ready: write a program's spec.md with the write-spec skill.";

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
    fn a_record_may_name_the_folder_or_its_spec() {
        let folder = folder_of("/home/you/add/spec.md").expect("a spec path");
        assert_eq!(folder.to_string(), "/home/you/add");
        let folder = folder_of(" /home/you/add ").expect("a folder path");
        assert_eq!(folder.to_string(), "/home/you/add");
        assert!(folder_of("add/spec.md").is_err());
    }
}
