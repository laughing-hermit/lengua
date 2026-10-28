//! The lengua MCP server: tool definitions and server instructions.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use crate::checker;
use crate::data_dir::DataDir;
use crate::hash::Sha256Hex;
use crate::lock::{self, CodeStamp, Key, Lock, TestsStamp};
use crate::platform::{CpuFeatures, Machine, Profile};
use crate::runner::{self, End, Job, Limits};
use crate::spec;
use crate::state::{
    CODE_FILE, LockStatus, ProgramFolder, SPEC_FILE, Snapshot, State, StepContext, TESTS_FILE,
    stamped_copy,
};
use crate::testfile;
use crate::testrun;
use crate::toolchain::Toolchain;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua builds programs as folders holding spec.md (the source \
of truth), tests.toml, main.s and lengua.lock. Order: spec, then tests, then code. Only the user \
approves spec.md: when they say so, pass their words to approve_spec. Never edit lengua.lock. \
Tests freeze once code runs, until the spec changes. Run programs only with the run tool. Pass \
absolute folder paths. When unsure, call status and follow its next step; the write-spec, \
write-tests and write-assembly skills explain each file.";

/// The longest approval the lock records: the user's words, not a whole conversation.
const MAX_APPROVAL_CHARS: usize = 500;

/// The lengua MCP server.
#[derive(Debug, Clone)]
pub struct LenguaServer {
    tool_router: ToolRouter<Self>,
    data_dir: Arc<DataDir>,
    /// Serializes every read-modify-write of a lock file.
    lock_writes: Arc<tokio::sync::Mutex<()>>,
    /// This server's own binary, which launches programs so their instructions can be counted.
    launcher: Arc<PathBuf>,
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

/// Input for `approve_spec`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ApprovalParams {
    /// Absolute path of the program folder, or of its spec.md.
    pub path: String,
    /// The user's own words approving the spec, quoted exactly, such as "I approve
    /// /home/you/projects/add/spec.md".
    pub approval: String,
}

/// Everything `status` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StatusReport {
    /// The lengua server version.
    pub server_version: String,
    /// The machine lengua is running on.
    pub machine: MachineReport,
    /// The LLVM tools lengua builds with.
    pub toolchain: ToolchainReport,
    /// What the connected agent harness supports.
    pub client: ClientReport,
    /// The server's private data folder (signing key, stamped test copies).
    pub data_dir: String,
    /// The program's state, when a path was given.
    pub program: Option<ProgramReport>,
    /// The single next thing to do.
    pub next_step: String,
}

/// Machine part of the status report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MachineReport {
    /// Operating system, e.g. `linux`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`.
    pub arch: String,
    /// CPU model, when the OS reports one.
    pub cpu_model: Option<String>,
    /// How many threads can run at once; a spec allows at most this many.
    pub cores: usize,
    /// CPU extensions the optimize step may use.
    pub features: CpuFeatures,
    /// The platform profile, e.g. `linux-x86_64`, or null when unsupported.
    pub profile: Option<String>,
    /// Whether lengua can build programs on this platform.
    pub supported: bool,
}

/// Toolchain part of the status report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ToolchainReport {
    /// `llvm-mc`, when found.
    pub assembler: Option<ToolReport>,
    /// `ld.lld`, when found.
    pub linker: Option<ToolReport>,
    /// What's wrong, when the toolchain can't be used.
    pub problem: Option<String>,
}

/// One external tool.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ToolReport {
    /// Absolute path of the executable.
    pub path: String,
    /// Reported version.
    pub version: String,
}

/// Client part of the status report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ClientReport {
    /// The negotiated MCP protocol version.
    pub protocol_version: Option<String>,
}

/// Program part of the status report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProgramReport {
    /// The program folder.
    pub path: String,
    /// Workflow state: no_spec, lock_invalid, spec_not_approved, tests_missing,
    /// tests_unchecked, tests_changed, code_missing, code_unchecked or ready.
    pub state: String,
    /// The label tests.toml and main.s must carry: `sha256:<hex>` of the current spec.md.
    pub spec_label: Option<String>,
    /// The program's files.
    pub files: FilesReport,
    /// Whether lengua.lock can be trusted.
    pub lock: LockReport,
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
    /// Whether lengua.lock records exactly this version as approved or checked.
    pub stamped: bool,
}

/// The lock part of a program report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LockReport {
    /// `missing`, `valid` or `invalid`.
    pub state: String,
    /// Why the lock can't be trusted, when it's invalid.
    pub problem: Option<String>,
    /// The user's words that approved the spec.
    pub approval: Option<String>,
    /// One hash over spec, tests and code, once all three are checked.
    pub fingerprint: Option<String>,
}

/// What `approve_spec` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ApprovalReport {
    /// Whether spec.md is approved now.
    pub approved: bool,
    /// The user's words that approved it, as lengua.lock records them.
    pub approval: String,
    /// The label tests.toml and main.s must carry: `sha256:<hex>` of spec.md.
    pub spec_label: String,
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
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
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
}

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "status",
        title = "Status",
        description = "Shows whether lengua can build programs on this machine and, given a \
            program folder, what that program needs next. It reports the OS, CPU and its \
            features, the LLVM tools (llvm-mc, ld.lld) and the MCP protocol in use; for a \
            program, each file's hash, whether lengua.lock records it, the label tests.toml and \
            main.s must carry, and any problems in an unapproved spec.md. Ends with the single \
            next step to take. path: absolute path of a program folder (optional).",
        annotations(title = "Status", read_only_hint = true, open_world_hint = false)
    )]
    async fn status(
        &self,
        Parameters(params): Parameters<StatusParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<StatusReport>, String> {
        let machine = Machine::detect();
        let toolchain = Toolchain::discover().await;
        let client = client_report(&ctx);
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
                os: machine.os.to_owned(),
                arch: machine.arch.to_owned(),
                cpu_model: machine.cpu_model.clone(),
                cores: machine.cores,
                features: machine.features,
                profile: machine.profile.map(|profile| profile.name().to_owned()),
                supported: machine.profile.is_some(),
            },
            toolchain: toolchain_report(&toolchain),
            client,
            data_dir: self.data_dir.path().display().to_string(),
            program: program.map(|(report, _)| report),
            next_step,
        }))
    }

    #[tool(
        name = "approve_spec",
        title = "Approve spec",
        description = "Records in lengua.lock that the user approved <path>/spec.md. Call it only \
            after the user has read the spec and told you, in their own words, that they approve \
            it, and pass those words as approval; never on your own. It first checks the spec's \
            format and returns any problems. Approving a changed spec resets the tests and code \
            checks; approving the same spec again after fixing frozen tests reopens them. path: \
            absolute path of the program folder or of its spec.md; approval: the user's words, \
            quoted exactly.",
        annotations(
            title = "Approve spec",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn approve_spec(
        &self,
        Parameters(params): Parameters<ApprovalParams>,
    ) -> Result<Json<ApprovalReport>, String> {
        let folder = folder_of(&params.path)?;
        let approval = params.approval.trim();
        if approval.is_empty() {
            return Err(format!(
                "approval must quote the user's own words approving {folder}/{SPEC_FILE}. Only \
                 the user can approve a spec: tell them where it is, and wait until they say they \
                 approve it."
            ));
        }
        if approval.chars().count() > MAX_APPROVAL_CHARS {
            return Err(format!(
                "approval holds more than {MAX_APPROVAL_CHARS} characters; quote only the user's \
                 words that approve the spec."
            ));
        }
        let _writing = self.lock_writes.lock().await;
        let text = read_spec(&folder)?;
        let spec = spec::parse(&text).map_err(|problems| spec_problems(&folder, &problems))?;
        let current = Sha256Hex::of(text.as_bytes());
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        let lock = lock::read(folder.path(), Some(&key));
        // Approving the same spec again reopens frozen tests that were found wrong and edited.
        let reopens = match &lock {
            Ok(Some(lock)) if lock.spec.sha256 == current => {
                lock.tests.as_ref().is_some_and(|stamp| {
                    stamp.frozen
                        && std::fs::read(folder.file(TESTS_FILE))
                            .is_ok_and(|tests| Sha256Hex::of(&tests) != stamp.sha256)
                })
            }
            _ => false,
        };
        let (approval, message) = match lock {
            Ok(Some(lock)) if lock.spec.sha256 == current && !reopens => (
                lock.spec.approval,
                format!("{SPEC_FILE} was already approved; nothing changed."),
            ),
            _ if reopens => {
                lock::write(
                    folder.path(),
                    &Lock::approved(current.clone(), approval.to_owned()),
                    &key,
                )
                .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
                (
                    approval.to_owned(),
                    format!(
                        "The user approved {SPEC_FILE} again, which reopens its tests: the fixed \
                         {TESTS_FILE} and then the code are checked again."
                    ),
                )
            }
            _ => {
                lock::write(
                    folder.path(),
                    &Lock::approved(current.clone(), approval.to_owned()),
                    &key,
                )
                .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
                (
                    approval.to_owned(),
                    format!(
                        "The user approved {SPEC_FILE} for \"{}\". It's now the source of truth \
                         that the tests and code are checked against.",
                        spec.title
                    ),
                )
            }
        };
        Ok(Json(ApprovalReport {
            approved: true,
            approval,
            spec_label: current.label(),
            message,
            next_step: self.program_state(&folder)?.1,
        }))
    }

    #[tool(
        name = "check_tests",
        title = "Check tests",
        description = "Checks <path>/tests.toml against the approved spec and records it in \
            lengua.lock. Until check_code first runs it, a fixed tests.toml can be checked again; \
            from then on it's frozen until the spec changes. It checks the [meta] \
            spec label, one example per behavior holding the spec's values, a [[generated]] block \
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
            State::TestsChanged { .. } => return Err(next_step),
            State::NoSpec | State::SpecNotApproved | State::LockInvalid(_) => {
                return Err(format!(
                    "Tests are checked against an approved spec, and {folder} has none yet. \
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
        let message = if already_checked {
            format!("{TESTS_FILE} was already checked; nothing changed.")
        } else {
            self.stamp_tests(&folder, &spec_hash, &tests_hash, &tests_text)
                .await?;
            format!(
                "{TESTS_FILE} covers every behavior in {SPEC_FILE}. No code has run yet: until \
                 check_code first runs it, a mistake in it can still be fixed and checked again. \
                 From then on it's frozen until {SPEC_FILE} changes."
            )
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
            fresh random inputs, and reports what each test cost. Needs an approved spec and \
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
                    "Code is checked against an approved spec and checked tests, which {folder} \
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
                "{CODE_FILE}'s first line must be `{header}`, the label of the approved spec, but \
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
        self.freeze_tests(&folder, &tests_hash).await?;
        let syscalls = profile.allowed_syscalls(&spec.syscalls, spec.threads.is_some());
        let outcome = testrun::run_tests(
            &suite,
            &self.launcher,
            &built.executable,
            &source,
            seed,
            &syscalls,
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
            message: format!(
                "{CODE_FILE} follows every rule and passed all {} test runs.{} lengua.lock now \
                 records spec, tests and code together, so the program can run.",
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
            that were approved and checked, and a fresh build of main.s is byte for byte the \
            executable check_code tested. The program gets an empty environment, 10 seconds and \
            8 MiB of output, and the kernel lets it make only the system calls its spec allows \
            and touch files only inside its working folder: an empty temporary folder, or \
            `folder`. path: absolute path of the program folder; args: the program's arguments; \
            stdin: text for its standard input; folder: absolute path of a folder of the user's \
            files to run in, naming the files in args relative to it (\"notes.txt\", not \
            \"/home/me/notes.txt\").",
        annotations(title = "Run program", read_only_hint = false, open_world_hint = false)
    )]
    async fn run(
        &self,
        Parameters(params): Parameters<RunParams>,
    ) -> Result<Json<RunReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
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
        let limits = Limits {
            timeout: std::time::Duration::from_secs(10),
            max_file_size: None,
            ..Limits::default()
        };
        let syscalls = profile.allowed_syscalls(&spec.syscalls, spec.threads.is_some());
        let job = Job {
            folder: workspace.as_deref(),
            ..Job::new(
                &built.executable,
                &params.args,
                params.stdin.as_bytes(),
                &syscalls,
            )
        };
        let result = runner::run(&self.launcher, job, limits)
            .await
            .map_err(|error| format!("couldn't run the program: {error}"))?;
        let ended = testrun::explain_end(&result, &testrun::assertion_lines(&source), limits);
        Ok(Json(RunReport {
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
            fingerprint: fingerprint.to_string(),
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

    /// The state's next step. An unapproved spec.md is checked first, so the user is only asked
    /// to approve a spec lengua accepts.
    fn next_step(&self, folder: &ProgramFolder, snapshot: &Snapshot, state: &State) -> String {
        if *state == State::SpecNotApproved
            && let Ok(text) = read_spec(folder)
            && let Err(problems) = spec::parse(&text)
        {
            return format!(
                "Next: fix {folder}/{SPEC_FILE} before asking the user to approve it:\n{}Fix them \
                 with the write-spec skill, then call status again.",
                problem_list(&problems)
            );
        }
        state.next_step(&StepContext {
            folder,
            spec: snapshot.spec.as_ref(),
            data_dir: self.data_dir.path(),
        })
    }

    /// Records checked tests in the lock, provided neither file changed while being checked,
    /// and keeps a copy so frozen tests can be restored.
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
                frozen: false,
            }),
            code: None,
            ..lock
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))
    }

    /// Freezes the checked tests before code first runs against them, so they can't be changed
    /// to fit the code; they stay frozen until the spec changes.
    async fn freeze_tests(&self, folder: &ProgramFolder, tests: &Sha256Hex) -> Result<(), String> {
        let _writing = self.lock_writes.lock().await;
        let key = Key::load(&self.data_dir)
            .map_err(|error| error.to_string())?
            .ok_or("lengua's signing key is missing; call status")?;
        let lock = lock::read(folder.path(), Some(&key))
            .map_err(|error| format!("lengua.lock can't be trusted: {error}"))?
            .ok_or("lengua.lock is missing; call status")?;
        match &lock.tests {
            Some(stamp) if stamp.sha256 == *tests && !stamp.frozen => {
                let lock = Lock {
                    tests: Some(TestsStamp {
                        sha256: tests.clone(),
                        frozen: true,
                    }),
                    ..lock
                };
                lock::write(folder.path(), &lock, &key).map_err(|error| {
                    format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE)
                })
            }
            Some(stamp) if stamp.sha256 == *tests => Ok(()),
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
        let lock = Lock {
            code: Some(code),
            ..lock
        };
        let fingerprint = lock.fingerprint().ok_or_else(changed)?;
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        Ok(fingerprint)
    }

    fn program_report(&self, folder: &ProgramFolder) -> Result<(ProgramReport, String), String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        let state = State::of(&snapshot);
        let step = self.next_step(folder, &snapshot, &state);
        let lock = snapshot.valid_lock();
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
                approval: lock.map(|lock| lock.spec.approval.clone()),
                fingerprint: lock
                    .and_then(Lock::fingerprint)
                    .map(|fingerprint| fingerprint.to_string()),
            },
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

fn client_report(ctx: &RequestContext<RoleServer>) -> ClientReport {
    ClientReport {
        protocol_version: ctx.protocol_version().map(|version| version.to_string()),
    }
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
    _folder: tempfile::TempDir,
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
        _folder: folder,
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

/// Saves the checked tests under their hash, written atomically.
fn write_stamped_copy(data_dir: &DataDir, tests: &Sha256Hex, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let path = stamped_copy(data_dir.path(), tests);
    let failed = |error: std::io::Error| {
        format!(
            "couldn't keep a copy of the checked tests at {}: {error}",
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
        "{folder}/{SPEC_FILE} has {count} problem{} to fix before it can be approved:\n{}Fix \
         them with the write-spec skill, tell the user what changed, and ask them to approve it \
         again.",
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

fn toolchain_report(
    toolchain: &Result<Toolchain, crate::toolchain::ToolchainError>,
) -> ToolchainReport {
    match toolchain {
        Ok(toolchain) => ToolchainReport {
            assembler: Some(tool_report(&toolchain.assembler)),
            linker: Some(tool_report(&toolchain.linker)),
            problem: None,
        },
        Err(error) => ToolchainReport {
            assembler: None,
            linker: None,
            problem: Some(error.to_string()),
        },
    }
}

fn tool_report(tool: &crate::toolchain::Tool) -> ToolReport {
    ToolReport {
        path: tool.path.display().to_string(),
        version: tool.version.clone(),
    }
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
    fn approval_may_name_the_folder_or_its_spec() {
        let folder = folder_of("/home/you/add/spec.md").expect("a spec path");
        assert_eq!(folder.to_string(), "/home/you/add");
        let folder = folder_of(" /home/you/add ").expect("a folder path");
        assert_eq!(folder.to_string(), "/home/you/add");
        assert!(folder_of("add/spec.md").is_err());
    }
}
