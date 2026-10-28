//! The lengua MCP server: tool definitions and server instructions.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Json;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;

use crate::data_dir::DataDir;
use crate::platform::{CpuFeatures, Machine};
use crate::toolchain::Toolchain;

/// Server-wide guidance sent to every client at startup.
///
/// Kept under 512 characters and self-contained: some harnesses only show the beginning.
pub const INSTRUCTIONS: &str = "lengua builds programs as folders holding spec.md (the source \
of truth), tests.toml, main.s and lengua.lock. Order: spec, then tests, then code. Never edit \
lengua.lock; only this server writes it. Tests stay frozen until the spec changes. Run programs \
only with the run tool. Always pass absolute folder paths. When unsure, call status and follow \
its next step; the write-spec, write-tests and write-assembly skills explain each file.";

/// The lengua MCP server.
#[derive(Debug, Clone)]
pub struct LenguaServer {
    tool_router: ToolRouter<Self>,
    data_dir: Arc<DataDir>,
}

impl LenguaServer {
    /// Creates a server that keeps its private data in `data_dir`.
    #[must_use]
    pub fn new(data_dir: DataDir) -> Self {
        Self {
            tool_router: Self::tool_router(),
            data_dir: Arc::new(data_dir),
        }
    }
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

#[tool_router]
impl LenguaServer {
    #[tool(
        name = "status",
        description = "Shows whether lengua can build programs here: OS, CPU, CPU features, the \
            LLVM tools (llvm-mc, ld.lld), the MCP protocol in use and whether this client can show \
            the spec-approval dialog. Ends with the single next step to take.",
        annotations(
            title = "lengua status",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn status(&self, ctx: RequestContext<RoleServer>) -> Result<Json<StatusReport>, String> {
        let machine = Machine::detect();
        let toolchain = Toolchain::discover().await;
        let client = client_report(&ctx);
        let next_step = machine_next_step(&machine, toolchain.as_ref().err(), client.elicitation);
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
            next_step,
        }))
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

fn machine_next_step(
    machine: &Machine,
    toolchain_error: Option<&crate::toolchain::ToolchainError>,
    elicitation: bool,
) -> String {
    if machine.profile.is_none() {
        return format!(
            "Blocked: {}-{} isn't supported yet. lengua currently builds programs for linux-x86_64.",
            machine.os, machine.arch
        );
    }
    if let Some(error) = toolchain_error {
        return format!("Blocked: {error}");
    }
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
}
