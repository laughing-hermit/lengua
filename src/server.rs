//! The lengua MCP server: tool definitions and server instructions.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::{InputResponses, RequestState, schema_for_output};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    CallToolResponse, CallToolResult, ElicitRequest, ElicitRequestParams, ElicitResult,
    ElicitationAction, ElicitationSchema, Implementation, InputRequest, InputRequiredResult,
    ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::service::{ElicitationError, RequestContext};
use rmcp::{RoleServer, ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use crate::checker;
use crate::data_dir::DataDir;
use crate::hash::{Sha256Hex, hex};
use crate::lock::{self, ApprovedVia, CodeStamp, Key, Lock, TestsStamp};
use crate::platform::{CpuFeatures, Machine, Profile};
use crate::runner::{self, End, Limits};
use crate::spec::{self, Spec};
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
of truth), tests.toml, main.s and lengua.lock. Order: spec, then tests, then code. Never edit \
lengua.lock; only this server writes it. Tests stay frozen until the spec changes. Run programs \
only with the run tool. Always pass absolute folder paths. When unsure, call status and follow \
its next step; the write-spec, write-tests and write-assembly skills explain each file.";

/// The key of the approval dialog inside an input-required result.
const APPROVAL_INPUT: &str = "approve_spec";

/// How long an approval dialog shown through an input-required result stays answerable.
const APPROVAL_TTL: Duration = Duration::from_secs(30 * 60);

/// At most this many approval dialogs wait for an answer at once.
const MAX_PENDING_APPROVALS: usize = 64;

/// The lengua MCP server.
#[derive(Debug, Clone)]
pub struct LenguaServer {
    tool_router: ToolRouter<Self>,
    data_dir: Arc<DataDir>,
    /// Approval dialogs shown through input-required results, by their single-use nonce.
    approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    /// Serializes every read-modify-write of a lock file.
    lock_writes: Arc<tokio::sync::Mutex<()>>,
    /// This server's own binary, which launches programs so their instructions can be counted.
    launcher: Arc<PathBuf>,
}

/// An approval dialog waiting for the client's retry.
#[derive(Debug, Clone)]
struct PendingApproval {
    folder: ProgramFolder,
    /// The spec.md hash the dialog showed.
    spec: Sha256Hex,
    expires: Instant,
}

impl LenguaServer {
    /// Creates a server that keeps its private data in `data_dir`.
    #[must_use]
    pub fn new(data_dir: DataDir) -> Self {
        Self {
            tool_router: Self::tool_router(),
            data_dir: Arc::new(data_dir),
            approvals: Arc::default(),
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
    /// Whether the client can show the spec-approval dialog (MCP form elicitation).
    pub elicitation: bool,
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
    /// How the spec was approved: `elicitation` or `tool-permission`.
    pub approved_via: Option<String>,
    /// One hash over spec, tests and code, once all three are checked.
    pub fingerprint: Option<String>,
}

/// What `approve_spec` reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ApprovalReport {
    /// Whether spec.md is approved now.
    pub approved: bool,
    /// How it was approved: `elicitation` (a dialog) or `tool-permission`.
    pub approved_via: Option<String>,
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
    /// Whether tests.toml is checked and frozen now.
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
    /// The seed that drew this check's random inputs.
    pub seed: String,
    /// Instructions executed per test (fewer is faster), when the CPU's counters are available.
    pub instructions: Vec<InstructionCount>,
    /// One hash over spec, tests and code.
    pub fingerprint: String,
    /// What happened.
    pub message: String,
    /// The single next thing to do.
    pub next_step: String,
}

/// Instructions executed by one test, or by all runs of a generated block.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct InstructionCount {
    /// The test, such as `example B1` or `generated B1`.
    pub test: String,
    /// Runs counted.
    pub runs: usize,
    /// Fewest instructions in a run.
    pub min: u64,
    /// The median run.
    pub median: u64,
    /// Most instructions in a run.
    pub max: u64,
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
    /// Instructions the program executed, when the CPU's counters are available.
    pub instructions: Option<u64>,
    /// The fingerprint of the spec, tests and code that ran.
    pub fingerprint: String,
}

/// The approval dialog's form: one checkbox, on by default.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SpecApproval {
    /// Approve spec.md exactly as written.
    #[schemars(title = "Approve", extend("default" = true))]
    approve: bool,
}

rmcp::elicit_safe!(SpecApproval);

/// The person's answer to an approval dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Approve,
    LeftUnchecked,
    Declined,
    Cancelled,
}

impl Answer {
    fn describe(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::LeftUnchecked => "left \"Approve\" unchecked",
            Self::Declined => "declined",
            Self::Cancelled => "closed the dialog",
        }
    }
}

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "status",
        description = "Shows whether lengua can build programs here: OS, CPU and its features, \
            the LLVM tools (llvm-mc, ld.lld), the MCP protocol in use and whether this client can \
            show approval dialogs. Given path, also shows where that program stands: each file's \
            hash, whether lengua.lock stamps it, and the label tests.toml and main.s must carry. \
            Ends with the single next step to take. path: absolute path of a program folder \
            (optional).",
        annotations(
            title = "lengua status",
            read_only_hint = true,
            open_world_hint = false
        )
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
            Some(path) => {
                Some(self.program_report(&ProgramFolder::new(path)?, client.elicitation)?)
            }
            None => None,
        };
        let next_step = match (
            machine_blocker(&machine, toolchain.as_ref().err()),
            &program,
        ) {
            (Some(blocker), _) => blocker,
            (None, Some((_, step))) => step.clone(),
            (None, None) => ready_step(client.elicitation),
        };
        Ok(Json(StatusReport {
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
            machine: MachineReport {
                os: machine.os.to_owned(),
                arch: machine.arch.to_owned(),
                cpu_model: machine.cpu_model.clone(),
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
        description = "Asks the user to approve <path>/spec.md as the program's source of truth \
            and records the approval in lengua.lock. Call it after showing the user the finished \
            spec. It first checks the spec's format and returns any problems without asking. The \
            user confirms in a dialog; on clients that can't show one, the permission prompt for \
            this tool is the approval. Approving a changed spec resets the tests and code checks. \
            path: absolute path of the program folder.",
        annotations(
            title = "Approve a spec",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<ApprovalReport>()
    )]
    async fn approve_spec(
        &self,
        Parameters(params): Parameters<ProgramParams>,
        RequestState(request_state): RequestState,
        InputResponses(responses): InputResponses,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let text = read_spec(&folder)?;
        let spec = spec::parse(&text).map_err(|problems| spec_problems(&folder, &problems))?;
        let shown = Sha256Hex::of(text.as_bytes());
        let elicitation = client_report(&ctx).elicitation;
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        if let Ok(Some(lock)) = lock::read(folder.path(), Some(&key))
            && lock.spec.sha256 == shown
        {
            return self.respond(
                &folder,
                &shown,
                Some(lock.spec.approved_via),
                "spec.md was already approved; nothing changed.".to_owned(),
                None,
                elicitation,
            );
        }
        if !elicitation {
            return self
                .stamp(&folder, &spec, &shown, ApprovedVia::ToolPermission)
                .await;
        }
        if uses_input_required(&ctx) {
            self.approve_with_input_required(
                &folder,
                &spec,
                &shown,
                request_state,
                responses.as_ref(),
            )
            .await
        } else {
            self.approve_with_dialog(&ctx, &folder, &spec, &shown).await
        }
    }

    #[tool(
        name = "check_tests",
        description = "Checks <path>/tests.toml against the approved spec.md and, if it passes, \
            records it in lengua.lock; from then on the tests stay frozen until the spec changes. \
            It checks the [meta] spec label, one example per behavior holding the spec's values, \
            a [[generated]] block for every behavior with inputs, generator ranges taken from the \
            spec, formulas that reproduce each example without overflow or division by zero, and \
            output that depends on the inputs. Returns every problem at once. path: absolute path \
            of the program folder.",
        annotations(
            title = "Check the tests",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn check_tests(
        &self,
        Parameters(params): Parameters<ProgramParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<TestsReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let elicitation = client_report(&ctx).elicitation;
        let (state, next_step) = self.program_state(&folder, elicitation)?;
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
                "{TESTS_FILE} passed every check and is now frozen: it can't change until \
                 {SPEC_FILE} does."
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
                .map(|block| u64::from(block.runs))
                .sum(),
            message,
            next_step: self.program_state(&folder, elicitation)?.1,
        }))
    }

    #[tool(
        name = "check_code",
        description = "Checks <path>/main.s and, when it passes, records it in lengua.lock. It \
            assembles and links the program, reads the machine code back and matches it to main.s \
            line by line, and checks every lengua Power of 10 rule; nothing runs before the rules \
            pass. Then it runs every test, generated tests with fresh random inputs, and reports \
            the instructions each test executed. Needs an approved spec and checked tests, and \
            main.s's first line must be the spec label from status. Returns every violation or \
            failed test with its line. path: absolute path of the program folder.",
        annotations(
            title = "Check the code",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn check_code(
        &self,
        Parameters(params): Parameters<ProgramParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CodeReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let elicitation = client_report(&ctx).elicitation;
        let (state, next_step) = self.program_state(&folder, elicitation)?;
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
        let outcome = testrun::run_tests(&suite, &self.launcher, &built.executable, &source, seed)
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
            seed: seed.to_string(),
            instructions: outcome
                .instructions
                .into_iter()
                .map(|count| InstructionCount {
                    test: count.test,
                    runs: count.runs,
                    min: count.min,
                    median: count.median,
                    max: count.max,
                })
                .collect(),
            fingerprint: fingerprint.to_string(),
            message: format!(
                "{CODE_FILE} follows every rule and passed all {} test runs (seed {seed}). \
                 lengua.lock now binds spec, tests and code: fingerprint {}. To make the program \
                 faster, change {CODE_FILE} and call check_code again: the tests must stay green, \
                 and fewer instructions means faster.",
                outcome.runs,
                fingerprint.short()
            ),
            next_step: self.program_state(&folder, elicitation)?.1,
        }))
    }

    #[tool(
        name = "run",
        description = "Runs the program in <path> with args and stdin, but only when lengua.lock \
            shows that spec.md, tests.toml and main.s are exactly the versions that were approved \
            and checked, and a fresh build of main.s is byte for byte the executable check_code \
            tested. The program gets an empty environment, its own temporary folder, 10 seconds \
            and 1 MiB of output. path: absolute path of the program folder; args: the program's \
            arguments; stdin: text for its standard input.",
        annotations(
            title = "Run a program",
            read_only_hint = false,
            open_world_hint = false
        )
    )]
    async fn run(
        &self,
        Parameters(params): Parameters<RunParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<RunReport>, String> {
        let folder = ProgramFolder::new(&params.path)?;
        let elicitation = client_report(&ctx).elicitation;
        let (state, next_step) = self.program_state(&folder, elicitation)?;
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
        let machine = Machine::detect();
        let built = build(&code, supported(&machine)?).await?;
        if Sha256Hex::of(&built.bytes) != stamp.executable_sha256 {
            return Err(format!(
                "This build of {CODE_FILE} differs from the one check_code tested (built with {}; \
                 now {}), so it won't run. Call check_code with path {folder} to test this build.",
                stamp.toolchain, built.toolchain
            ));
        }
        let limits = Limits {
            timeout: std::time::Duration::from_secs(10),
            ..Limits::default()
        };
        let result = runner::run(
            &self.launcher,
            &built.executable,
            &params.args,
            params.stdin.as_bytes(),
            limits,
        )
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
            fingerprint: fingerprint.to_string(),
        }))
    }
}

impl LenguaServer {
    /// The program's state and its next step.
    fn program_state(
        &self,
        folder: &ProgramFolder,
        elicitation: bool,
    ) -> Result<(State, String), String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        let state = State::of(&snapshot);
        let step = state.next_step(&StepContext {
            folder,
            spec: snapshot.spec.as_ref(),
            data_dir: self.data_dir.path(),
            elicitation,
        });
        Ok((state, step))
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
            }),
            code: None,
            ..lock
        };
        lock::write(folder.path(), &lock, &key)
            .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))
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

    fn program_report(
        &self,
        folder: &ProgramFolder,
        elicitation: bool,
    ) -> Result<(ProgramReport, String), String> {
        let key = Key::load(&self.data_dir).map_err(|error| error.to_string())?;
        let snapshot = Snapshot::read(folder, key.as_ref())
            .map_err(|error| format!("can't read {folder}: {error}"))?;
        let state = State::of(&snapshot);
        let step = state.next_step(&StepContext {
            folder,
            spec: snapshot.spec.as_ref(),
            data_dir: self.data_dir.path(),
            elicitation,
        });
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
                approved_via: lock.map(|lock| lock.spec.approved_via.as_str().to_owned()),
                fingerprint: lock
                    .and_then(Lock::fingerprint)
                    .map(|fingerprint| fingerprint.to_string()),
            },
        };
        Ok((report, step))
    }

    /// Shows the approval dialog as a server-to-client request (protocol 2025-11-25 and older).
    async fn approve_with_dialog(
        &self,
        ctx: &RequestContext<RoleServer>,
        folder: &ProgramFolder,
        spec: &Spec,
        shown: &Sha256Hex,
    ) -> Result<CallToolResponse, String> {
        let answer = match ctx
            .peer
            .elicit::<SpecApproval>(approval_message(folder))
            .await
        {
            Ok(Some(SpecApproval { approve: true })) => Answer::Approve,
            Ok(_) => Answer::LeftUnchecked,
            Err(ElicitationError::UserDeclined) => Answer::Declined,
            Err(ElicitationError::UserCancelled) => Answer::Cancelled,
            Err(error) => {
                return Err(format!(
                    "The approval dialog failed ({error}), so nothing was approved."
                ));
            }
        };
        self.conclude(folder, spec, shown, shown, answer).await
    }

    /// Shows the approval dialog through an input-required result (protocol 2026-07-28 and
    /// newer, which has no server-to-client requests). The client shows the dialog and calls
    /// this tool again with the answer and the nonce.
    async fn approve_with_input_required(
        &self,
        folder: &ProgramFolder,
        spec: &Spec,
        current: &Sha256Hex,
        request_state: Option<String>,
        responses: Option<&rmcp::model::InputResponses>,
    ) -> Result<CallToolResponse, String> {
        if let Some(nonce) = request_state
            && let Some(pending) = self.take_approval(&nonce, folder)
            && let Some(answer) = dialog_answer(responses)
        {
            return self
                .conclude(folder, spec, &pending.spec, current, answer)
                .await;
        }
        // No answer yet, or an unknown, used or expired nonce: show a new dialog.
        let nonce = new_nonce()?;
        self.remember_approval(
            nonce.clone(),
            PendingApproval {
                folder: folder.clone(),
                spec: current.clone(),
                expires: Instant::now() + APPROVAL_TTL,
            },
        );
        let schema = ElicitationSchema::from_type::<SpecApproval>()
            .map_err(|error| format!("couldn't build the approval form: {error}"))?;
        let dialog = ElicitRequest::new(ElicitRequestParams::FormElicitationParams {
            meta: None,
            message: approval_message(folder),
            requested_schema: schema,
        });
        let requests =
            BTreeMap::from([(APPROVAL_INPUT.to_owned(), InputRequest::Elicitation(dialog))]);
        Ok(InputRequiredResult::new(Some(requests), Some(nonce)).into())
    }

    /// Acts on the person's answer to a dialog that showed the spec hashed as `shown`.
    async fn conclude(
        &self,
        folder: &ProgramFolder,
        spec: &Spec,
        shown: &Sha256Hex,
        current: &Sha256Hex,
        answer: Answer,
    ) -> Result<CallToolResponse, String> {
        if answer != Answer::Approve {
            return self.respond(
                folder,
                current,
                None,
                format!(
                    "The user {} instead of approving spec.md, so nothing was approved.",
                    answer.describe()
                ),
                Some(format!(
                    "Next: ask the user what to change in {folder}/{SPEC_FILE}, update it with \
                     the write-spec skill, then call approve_spec with path {folder} again."
                )),
                true,
            );
        }
        if shown != current {
            return self.spec_changed(folder, current);
        }
        self.stamp(folder, spec, shown, ApprovedVia::Elicitation)
            .await
    }

    /// Records the approval, provided spec.md still has the approved content.
    async fn stamp(
        &self,
        folder: &ProgramFolder,
        spec: &Spec,
        approved: &Sha256Hex,
        approved_via: ApprovedVia,
    ) -> Result<CallToolResponse, String> {
        let _writing = self.lock_writes.lock().await;
        let current = Sha256Hex::of(read_spec(folder)?.as_bytes());
        if current != *approved {
            return self.spec_changed(folder, &current);
        }
        let key = Key::load_or_create(&self.data_dir).map_err(|error| error.to_string())?;
        lock::write(
            folder.path(),
            &Lock::approved(current.clone(), approved_via),
            &key,
        )
        .map_err(|error| format!("couldn't write {folder}/{}: {error}", lock::LOCK_FILE))?;
        let message = match approved_via {
            ApprovedVia::Elicitation => format!(
                "The user approved spec.md for \"{}\". It's now the source of truth that the \
                 tests and code are checked against.",
                spec.title
            ),
            ApprovedVia::ToolPermission => format!(
                "spec.md for \"{}\" is approved through this client's permission prompt, \
                 because it can't show approval dialogs. It's now the source of truth.",
                spec.title
            ),
        };
        self.respond(
            folder,
            &current,
            Some(approved_via),
            message,
            None,
            approved_via == ApprovedVia::Elicitation,
        )
    }

    fn spec_changed(
        &self,
        folder: &ProgramFolder,
        current: &Sha256Hex,
    ) -> Result<CallToolResponse, String> {
        self.respond(
            folder,
            current,
            None,
            "spec.md changed while the approval dialog was open, so nothing was approved."
                .to_owned(),
            Some(format!(
                "Next: show the user the current {folder}/{SPEC_FILE}, then call approve_spec \
                 with path {folder} again."
            )),
            true,
        )
    }

    /// Builds the tool result. `next_step` defaults to the program's state.
    fn respond(
        &self,
        folder: &ProgramFolder,
        spec: &Sha256Hex,
        approved_via: Option<ApprovedVia>,
        message: String,
        next_step: Option<String>,
        elicitation: bool,
    ) -> Result<CallToolResponse, String> {
        let next_step = match next_step {
            Some(step) => step,
            None => self.program_report(folder, elicitation)?.1,
        };
        let report = ApprovalReport {
            approved: approved_via.is_some(),
            approved_via: approved_via.map(|via| via.as_str().to_owned()),
            spec_label: spec.label(),
            message,
            next_step,
        };
        let value = serde_json::to_value(report)
            .map_err(|error| format!("couldn't encode the report: {error}"))?;
        Ok(CallToolResult::structured(value).into())
    }

    fn remember_approval(&self, nonce: String, pending: PendingApproval) {
        let mut approvals = self.approvals.lock().expect("approval map lock");
        let now = Instant::now();
        approvals.retain(|_, pending| pending.expires > now);
        if approvals.len() >= MAX_PENDING_APPROVALS
            && let Some(oldest) = approvals
                .iter()
                .min_by_key(|(_, pending)| pending.expires)
                .map(|(nonce, _)| nonce.clone())
        {
            approvals.remove(&oldest);
        }
        approvals.insert(nonce, pending);
    }

    /// Takes a pending dialog: each nonce answers once, for its own folder, before it expires.
    fn take_approval(&self, nonce: &str, folder: &ProgramFolder) -> Option<PendingApproval> {
        let mut approvals = self.approvals.lock().expect("approval map lock");
        approvals
            .remove(nonce)
            .filter(|pending| pending.expires > Instant::now() && pending.folder == *folder)
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
    let elicitation = ctx
        .client_capabilities()
        .and_then(|capabilities| capabilities.elicitation)
        // An empty elicitation capability means form mode (MCP spec).
        .is_some_and(|elicitation| elicitation.form.is_some() || elicitation.url.is_none());
    ClientReport {
        protocol_version: ctx.protocol_version().map(|version| version.to_string()),
        elicitation,
    }
}

/// Protocol 2026-07-28 dropped server-to-client requests; dialogs travel in input-required
/// results instead.
fn uses_input_required(ctx: &RequestContext<RoleServer>) -> bool {
    ctx.protocol_version()
        .is_some_and(|version| version >= ProtocolVersion::NO_INITIALIZE)
}

/// Reads the person's answer from a retried call's input responses.
fn dialog_answer(responses: Option<&rmcp::model::InputResponses>) -> Option<Answer> {
    let response = responses?.get(APPROVAL_INPUT)?;
    let result: ElicitResult = serde_json::from_value(response.clone()).ok()?;
    Some(match result.action {
        ElicitationAction::Accept => {
            let approved = result
                .content
                .and_then(|content| serde_json::from_value::<SpecApproval>(content).ok())
                .is_some_and(|form| form.approve);
            if approved {
                Answer::Approve
            } else {
                Answer::LeftUnchecked
            }
        }
        ElicitationAction::Decline => Answer::Declined,
        _ => Answer::Cancelled,
    })
}

fn new_nonce() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("couldn't create an approval nonce: {error}"))?;
    Ok(hex(&bytes))
}

fn read_spec(folder: &ProgramFolder) -> Result<String, String> {
    match std::fs::read_to_string(folder.file(SPEC_FILE)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "There's no {SPEC_FILE} in {folder}. Write it with the write-spec skill, show it to \
             the user, then call approve_spec."
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
        "{folder}/{CODE_FILE} follows every rule but fails {count} test{}, so it wasn't \
         stamped:\n",
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
    let mut text = format!(
        "{folder}/{SPEC_FILE} has {count} problem{} to fix before it can be approved:\n",
        if count == 1 { "" } else { "s" }
    );
    for problem in problems {
        let _ = writeln!(text, "- {problem}");
    }
    text.push_str("Fix them with the write-spec skill, then call approve_spec again.");
    text
}

/// The text of the approval dialog: just the spec's path. The person approves the document
/// itself, which they read in full before approving.
fn approval_message(folder: &ProgramFolder) -> String {
    format!("Approve {folder}/{SPEC_FILE}?")
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

fn ready_step(elicitation: bool) -> String {
    let ready = "Ready: write a program's spec.md with the write-spec skill.";
    if elicitation {
        ready.to_owned()
    } else {
        format!(
            "{ready} Note: this client can't show approval dialogs, so approving a spec will rely \
             on its tool-permission prompt."
        )
    }
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
    fn approval_form_is_one_checkbox_that_defaults_to_on() {
        let schema = ElicitationSchema::from_type::<SpecApproval>().expect("valid form");
        let json = serde_json::to_value(schema).expect("serializable");
        let field = &json["properties"]["approve"];
        assert_eq!(field["type"], "boolean", "{json}");
        assert_eq!(field["default"], true, "{json}");
        assert_eq!(field["title"], "Approve", "{json}");
        assert_eq!(json["properties"].as_object().map(|p| p.len()), Some(1));
    }

    #[test]
    fn reads_dialog_answers() {
        let respond = |value: serde_json::Value| {
            let responses = BTreeMap::from([(APPROVAL_INPUT.to_owned(), value)]);
            dialog_answer(Some(&responses))
        };
        assert_eq!(
            respond(serde_json::json!({"action": "accept", "content": {"approve": true}})),
            Some(Answer::Approve)
        );
        assert_eq!(
            respond(serde_json::json!({"action": "accept", "content": {"approve": false}})),
            Some(Answer::LeftUnchecked)
        );
        assert_eq!(
            respond(serde_json::json!({"action": "accept", "content": {}})),
            Some(Answer::LeftUnchecked)
        );
        assert_eq!(
            respond(serde_json::json!({"action": "decline"})),
            Some(Answer::Declined)
        );
        assert_eq!(
            respond(serde_json::json!({"action": "cancel"})),
            Some(Answer::Cancelled)
        );
        assert_eq!(respond(serde_json::json!("nonsense")), None);
        assert_eq!(dialog_answer(None), None);
    }
}
