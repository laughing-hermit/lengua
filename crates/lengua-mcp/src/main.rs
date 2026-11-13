mod build;
mod doctor;
mod host;
mod runs;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::stdio;
use rmcp::{Json, ServerHandler, ServiceExt, schemars, serde, tool, tool_handler, tool_router};

use crate::build::{Builds, State, Status};
use crate::host::{Ending, Host, Input, Outcome};
use crate::runs::{Progress, Runs};

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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct RunParams {
    #[schemars(
        description = "The program's folder, the one that holds idea.md, as an absolute path; check must have built it"
    )]
    folder: String,
    #[serde(default)]
    #[schemars(
        description = "The arguments after the program's name; only when the idea has `- Reads: arguments`"
    )]
    arguments: Vec<String>,
    #[serde(default)]
    #[schemars(
        description = "The text on the program's standard input; only when the idea has `- Reads: standard input`"
    )]
    stdin: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "True gives a fixed wall clock and random numbers from a fixed seed, as the tests do, so that two runs give the same output"
    )]
    fixed: bool,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct RunReply {
    #[schemars(
        description = "running (call run again with the same input), exited, out_of_time, out_of_memory, trapped, or refused (the host did not start the program)"
    )]
    state: String,
    #[schemars(description = "When exited: the program's exit code")]
    exit_code: Option<i32>,
    #[schemars(description = "What the program wrote to standard output, as UTF-8 text")]
    stdout: String,
    #[schemars(description = "What the program wrote to standard error, as UTF-8 text")]
    stderr: String,
    #[schemars(
        description = "When trapped, out_of_time, out_of_memory or refused: what happened, in plain words or as the trap's message"
    )]
    message: Option<String>,
    #[schemars(description = "The milliseconds the run has taken so far, or took")]
    milliseconds: u64,
}

#[derive(Clone)]
struct Lengua {
    name: &'static str,
    builds: Builds,
    runs: Runs,
}

fn millis(taken: std::time::Duration) -> u64 {
    u64::try_from(taken.as_millis()).unwrap_or(u64::MAX)
}

fn plain(state: &str, message: Option<String>, milliseconds: u64) -> RunReply {
    RunReply {
        state: state.to_string(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        message,
        milliseconds,
    }
}

fn run_reply(progress: Progress) -> RunReply {
    let (outcome, taken) = match progress {
        Progress::Running(taken) => return plain("running", None, millis(taken)),
        Progress::Ended(outcome, taken) => (outcome, taken),
    };
    let Outcome {
        ending,
        stdout,
        stderr,
    } = outcome;
    let (state, exit_code, message) = match ending {
        Ending::Exited(code) => ("exited", Some(code), None),
        Ending::OutOfTime(message) => ("out_of_time", None, Some(message)),
        Ending::OutOfMemory => (
            "out_of_memory",
            None,
            Some("the program asked for more memory than its idea allows".to_string()),
        ),
        Ending::Trapped(message) => ("trapped", None, Some(message)),
        Ending::Refused(message) => ("refused", None, Some(message)),
    };
    RunReply {
        state: state.to_string(),
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        message,
        milliseconds: millis(taken),
    }
}

fn ungranted_input(grants: &lengua_idea::Grants, params: &RunParams) -> Option<String> {
    if !params.arguments.is_empty() && !grants.reach(lengua_idea::Reach::Arguments) {
        return Some(
            "the idea does not grant `- Reads: arguments`, so the program gets no arguments; add the grant, or call run with none".to_string(),
        );
    }
    if params.stdin.is_some() && !grants.reach(lengua_idea::Reach::StandardInput) {
        return Some(
            "the idea does not grant `- Reads: standard input`, so the program gets an empty standard input; add the grant, or call run with no stdin".to_string(),
        );
    }
    None
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

    #[tool(
        description = "Runs a lengua program that check built, under exactly the grants of its idea: standard output and standard error always; the arguments, standard input, the wall clock, random numbers and the folders only when the idea grants them; the memory and the time of the idea as limits. Returns the state (exited, out_of_time, out_of_memory, trapped or refused), the exit code, standard output and standard error. When the run takes more than about 30 seconds, the state is running: call run again with the same input to get its result."
    )]
    fn run(&self, Parameters(params): Parameters<RunParams>) -> Result<Json<RunReply>, String> {
        let folder = PathBuf::from(&params.folder);
        if !folder.is_dir() {
            return Err(format!("{} is not a folder", folder.display()));
        }
        let report = lengua_idea::program(&folder);
        if !report.findings.is_empty() {
            let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
            return Ok(Json(plain(
                "refused",
                Some(format!(
                    "the ideas have {} problems ({}); call check, and fix them first",
                    codes.len(),
                    codes.join(", ")
                )),
                0,
            )));
        }
        let Some(build) = report.build else {
            return Err("lengua could not read the program".to_string());
        };
        if let Some(message) = ungranted_input(&build.grants, &params) {
            return Ok(Json(plain("refused", Some(message), 0)));
        }
        let input = Input {
            arguments: params.arguments,
            stdin: params.stdin.unwrap_or_default().into_bytes(),
            fixed: params.fixed,
        };
        Ok(Json(run_reply(self.runs.run(&folder, build, input))))
    }
}

#[tool_handler(
    name = "lengua-mcp",
    instructions = "lengua checks, builds and runs programs written as idea files and Rust. Call check with the absolute path of a program's folder; while the build's state is building, call check again. When it is built, call run to run it under the grants of its idea."
)]
#[allow(clippy::unused_async_trait_impl)]
impl ServerHandler for Lengua {}

#[tokio::main(flavor = "current_thread")]
async fn serve() -> Result<(), String> {
    let host = Host::new()?;
    let lengua = Lengua {
        name: VERSION,
        builds: Builds::new(host.clone()),
        runs: Runs::new(host),
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
