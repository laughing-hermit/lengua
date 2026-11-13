mod build;
mod doctor;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::stdio;
use rmcp::{Json, ServerHandler, ServiceExt, schemars, serde, tool, tool_handler, tool_router};

use crate::build::{Builds, State, Status};

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
    #[schemars(description = "The kind of problem, for example ste-word or rust-error")]
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
struct BuildReply {
    #[schemars(
        description = "no_code (there is no src/main.rs yet), building, built or build_failed"
    )]
    state: String,
    #[schemars(description = "The seconds the build has taken so far, or took")]
    seconds: u64,
    #[schemars(description = "The target the program is built for")]
    target: String,
    #[schemars(description = "When built: the absolute path of the component")]
    component: Option<String>,
    #[schemars(description = "When built: the size of the component in bytes")]
    size: Option<u64>,
    #[schemars(description = "When built: the SHA-256 of the component, in hex")]
    sha256: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct CheckReply {
    #[schemars(
        description = "True when there are no problems; while the state of the build is building, call check again to get its result"
    )]
    ok: bool,
    #[schemars(description = "The name and version of the lengua that checked")]
    checked_by: String,
    #[schemars(
        description = "Every problem of every idea, or every error of the build, sorted by file and line"
    )]
    findings: Vec<FindingReply>,
    #[schemars(description = "Things to know that are not problems")]
    notes: Vec<NoteReply>,
    #[schemars(description = "The files that check wrote")]
    written: Vec<String>,
    #[schemars(description = "The build, when the ideas have no problems")]
    build: Option<BuildReply>,
}

#[derive(Clone)]
struct Lengua {
    name: &'static str,
    builds: Builds,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn build_reply(target: &str, status: Status, findings: &mut Vec<FindingReply>) -> BuildReply {
    let mut out = BuildReply {
        state: "building".to_string(),
        seconds: status.seconds,
        target: target.to_string(),
        component: None,
        size: None,
        sha256: None,
    };
    match status.state {
        State::Building => {}
        State::Built {
            component,
            size,
            sha256,
        } => {
            out.state = "built".to_string();
            out.component = Some(component.to_string_lossy().to_string());
            out.size = Some(size);
            out.sha256 = Some(sha256);
        }
        State::Failed { problems } => {
            out.state = "build_failed".to_string();
            findings.extend(problems.into_iter().map(|p| FindingReply {
                file: p.file,
                line: p.line,
                code: p.code,
                message: p.message,
            }));
        }
    }
    out
}

impl Lengua {
    fn reply(&self, folder: &Path, report: lengua_idea::Report) -> CheckReply {
        let mut findings: Vec<FindingReply> = report
            .findings
            .into_iter()
            .map(|f| FindingReply {
                file: f.file,
                line: f.line,
                code: f.code.to_string(),
                message: f.message,
            })
            .collect();
        let build = report.build.map(|plan| {
            if plan.code {
                let status = self.builds.status(folder, &plan);
                build_reply(plan.target, status, &mut findings)
            } else {
                BuildReply {
                    state: "no_code".to_string(),
                    seconds: 0,
                    target: plan.target.to_string(),
                    component: None,
                    size: None,
                    sha256: None,
                }
            }
        });
        CheckReply {
            ok: findings.is_empty(),
            checked_by: self.name.to_string(),
            findings,
            notes: report
                .notes
                .into_iter()
                .map(|n| NoteReply {
                    file: n.file,
                    message: n.message,
                })
                .collect(),
            written: report.written,
            build,
        }
    }
}

#[tool_router]
impl Lengua {
    #[tool(
        description = "Checks a lengua program: its ideas (idea.md, ideas/*.md, ideas/ui/*.md), then builds it to a WebAssembly component in the background. Returns every problem at once, each with its file, line, code and message. When the ideas have no problems, it writes the frontmatter of each idea, index.md, log.md, Cargo.toml and wit/world.wit, and starts the build; the reply gives the build's state: building, built (with the component's path, size and SHA-256) or build_failed (with the compiler's errors as findings). Call check again to get the state of a build."
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
        Ok(Json(self.reply(&folder, report)))
    }
}

#[tool_handler(
    name = "lengua-mcp",
    instructions = "lengua checks and builds programs written as idea files and Rust. Call check with the absolute path of a program's folder; while the build's state is building, call check again."
)]
#[allow(clippy::unused_async_trait_impl)]
impl ServerHandler for Lengua {}

#[tokio::main(flavor = "current_thread")]
async fn serve() -> Result<(), String> {
    let lengua = Lengua {
        name: VERSION,
        builds: Builds::default(),
    };
    let server = lengua.serve(stdio()).await.map_err(|e| e.to_string())?;
    server.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn run_doctor() -> ExitCode {
    let folder = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let doctor = doctor::examine(&folder);
    print!("{}", doctor.text());
    if doctor.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("{VERSION}");
        return ExitCode::SUCCESS;
    }
    if args == ["doctor"] {
        return run_doctor();
    }
    if !args.is_empty() {
        eprintln!("usage: {} [--version | doctor]", env!("CARGO_PKG_NAME"));
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
