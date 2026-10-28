//! The lengua MCP server: tool definitions and server instructions.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
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

use crate::data_dir::DataDir;
use crate::hash::{Sha256Hex, hex};
use crate::lock::{self, ApprovedVia, Key, Lock};
use crate::platform::{CpuFeatures, Machine};
use crate::spec::{self, Spec, Syscall};
use crate::state::{LockStatus, ProgramFolder, SPEC_FILE, Snapshot, State, StepContext};
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

/// The approval dialog's form: one checkbox, on by default.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SpecApproval {
    /// Approve spec.md exactly as written.
    #[schemars(title = "Approve this spec", extend("default" = true))]
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
            Self::LeftUnchecked => "left \"Approve this spec\" unchecked",
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
}

impl LenguaServer {
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
            .elicit::<SpecApproval>(approval_message(folder, spec, shown))
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
            message: approval_message(folder, spec, current),
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

/// The text of the approval dialog.
fn approval_message(folder: &ProgramFolder, spec: &Spec, hash: &Sha256Hex) -> String {
    let mut message = format!("Approve the spec for \"{}\"?\n\n", spec.title);
    if let Some(description) = &spec.description {
        let _ = writeln!(message, "{description}\n");
    }
    let _ = writeln!(
        message,
        "File: {folder}/{SPEC_FILE} (sha256:{}…)\n\nBehaviors:",
        hash.short()
    );
    for behavior in &spec.behaviors {
        let _ = writeln!(message, "- {}: {}", behavior.id, behavior.title);
    }
    let syscalls: Vec<&str> = spec.syscalls.iter().copied().map(Syscall::name).collect();
    let _ = writeln!(
        message,
        "\nAllowed system calls: {}\n\nApproving makes this exact text the program's source of \
         truth: the AI writes the tests and code from it, and any later edit to {SPEC_FILE} \
         needs a new approval.",
        syscalls.join(", ")
    );
    message
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
        assert_eq!(field["title"], "Approve this spec", "{json}");
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
