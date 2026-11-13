use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::stdio;
use rmcp::{Json, ServerHandler, ServiceExt, schemars, serde, tool, tool_handler, tool_router};

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct CheckParams {
    #[schemars(
        description = "The program's folder, the one that holds idea.md, as an absolute path"
    )]
    folder: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct FindingReply {
    #[schemars(description = "The file, relative to the folder, with / between names")]
    file: String,
    #[schemars(description = "The line in the file, from 1")]
    line: usize,
    #[schemars(description = "The kind of problem, for example ste-word")]
    code: String,
    #[schemars(description = "What is wrong, in plain words")]
    message: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct NoteReply {
    file: String,
    message: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct CheckReply {
    #[schemars(description = "True when the ideas have no problems")]
    ok: bool,
    #[schemars(description = "The name and version of the lengua that checked")]
    checked_by: String,
    #[schemars(description = "Every problem of every idea, sorted by file and line")]
    findings: Vec<FindingReply>,
    #[schemars(description = "Things to know that are not problems")]
    notes: Vec<NoteReply>,
    #[schemars(description = "The files that check wrote")]
    written: Vec<String>,
}

#[derive(Clone)]
struct Lengua {
    name: &'static str,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn reply(name: &str, report: lengua_idea::Report) -> CheckReply {
    CheckReply {
        ok: report.findings.is_empty(),
        checked_by: name.to_string(),
        findings: report
            .findings
            .into_iter()
            .map(|f| FindingReply {
                file: f.file,
                line: f.line,
                code: f.code.to_string(),
                message: f.message,
            })
            .collect(),
        notes: report
            .notes
            .into_iter()
            .map(|n| NoteReply {
                file: n.file,
                message: n.message,
            })
            .collect(),
        written: report.written,
    }
}

#[tool_router]
impl Lengua {
    #[tool(
        description = "Checks the ideas of a lengua program: idea.md, ideas/*.md and ideas/ui/*.md. Returns every problem at once, each with its file, line, code and message. When there are no problems, it writes the frontmatter of each idea, index.md and log.md."
    )]
    fn check(
        &self,
        Parameters(CheckParams { folder }): Parameters<CheckParams>,
    ) -> Result<Json<CheckReply>, String> {
        let folder = PathBuf::from(folder);
        if !folder.is_dir() {
            return Err(format!("{} is not a folder", folder.display()));
        }
        let report = lengua_idea::check(&folder, now());
        Ok(Json(reply(self.name, report)))
    }
}

#[tool_handler(
    name = "lengua-mcp",
    instructions = "lengua checks idea files. Call check with the absolute path of a program's folder."
)]
#[allow(clippy::unused_async_trait_impl)]
impl ServerHandler for Lengua {}

#[tokio::main(flavor = "current_thread")]
async fn serve() -> Result<(), String> {
    let server = Lengua { name: VERSION }
        .serve(stdio())
        .await
        .map_err(|e| e.to_string())?;
    server.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("{VERSION}");
        return ExitCode::SUCCESS;
    }
    if !args.is_empty() {
        eprintln!("usage: {} [--version]", env!("CARGO_PKG_NAME"));
        return ExitCode::from(2);
    }
    match serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: {error}", env!("CARGO_PKG_NAME"));
            ExitCode::FAILURE
        }
    }
}
