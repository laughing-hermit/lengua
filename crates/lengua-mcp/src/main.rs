mod build;
mod doctor;
mod fixtures;
mod harness;
mod host;
mod mutation;
mod runs;
mod testing;
mod worker;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::transport::stdio;
use rmcp::{Json, ServerHandler, ServiceExt, schemars, serde, tool, tool_handler, tool_router};

use crate::build::{Builds, Stage, Status};
use crate::host::Host;
use crate::runs::{Progress, Runs};
use lengua_host::{Ending, Input, Outcome};
use lengua_idea::lock::{self, Facts, Key, Kind};

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
    #[schemars(description = "The file, relative to the folder, with / between names")]
    file: String,
    #[schemars(description = "The line in the file, from 1, when the note has one")]
    line: Option<usize>,
    message: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct TestsReply {
    #[schemars(description = "The runs of the examples and the drawn cases, screens left out")]
    total: usize,
    #[schemars(description = "The runs done so far")]
    run: usize,
    #[schemars(description = "The runs that failed so far")]
    failed: usize,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct MutantsReply {
    #[schemars(description = "The mutants made from the code, at most 40")]
    total: usize,
    #[schemars(description = "The mutants done so far")]
    done: usize,
    #[schemars(description = "Mutants that a test failed on")]
    caught: usize,
    #[schemars(description = "Mutants that every test passed on; each is a note")]
    survived: usize,
    #[schemars(description = "Mutants that did not build; they do not count")]
    unbuilt: usize,
    #[schemars(
        description = "Mutants that built to the same component as the code; they do not count"
    )]
    same: usize,
    #[schemars(description = "Mutants whose result came from the last check")]
    cached: usize,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct BuildReply {
    #[schemars(
        description = "no_code (there is no src/main.rs yet), building, build_failed, testing (the examples run), tests_failed, mutating (the mutation score is measured) or done"
    )]
    state: String,
    #[schemars(description = "The seconds the job has taken so far, or took")]
    seconds: u64,
    #[schemars(description = "The target the program is built for")]
    target: String,
    #[schemars(description = "When built: the absolute path of the component")]
    component: Option<String>,
    #[schemars(description = "When built: the size of the component in bytes")]
    size: Option<u64>,
    #[schemars(description = "When built: the SHA-256 of the component, in hex")]
    sha256: Option<String>,
    #[schemars(description = "From testing on: the runs of the tests")]
    tests: Option<TestsReply>,
    #[schemars(description = "From mutating on: the mutants")]
    mutants: Option<MutantsReply>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct PartReply {
    #[schemars(
        description = "The folder of a library or the file of a component, relative to the program's folder"
    )]
    path: String,
    #[schemars(
        description = "Its trust label: checked or tested for a lengua library, trusted for a component lengua did not build"
    )]
    label: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(crate = "rmcp::serde")]
#[schemars(crate = "rmcp::schemars")]
struct CheckReply {
    #[schemars(
        description = "True when there are no problems; while the state of the build is building, testing or mutating, call check again to get its result"
    )]
    ok: bool,
    #[schemars(description = "The name and version of the lengua that checked")]
    checked_by: String,
    #[schemars(
        description = "Every problem of every idea, or every error of the build, or every failed test, sorted by file and line"
    )]
    findings: Vec<FindingReply>,
    #[schemars(
        description = "Things to know that are not problems, the mutants that survived among them"
    )]
    notes: Vec<NoteReply>,
    #[schemars(description = "The files that check wrote")]
    written: Vec<String>,
    #[schemars(
        description = "The trust label: unchecked until the job is done, then checked (every test passes and the mutation score is 80% or more) or tested (the score is under 80%)"
    )]
    label: String,
    #[schemars(
        description = "When done: the mutation score in percent, the mutants caught of those counted; none when no mutant counted"
    )]
    score: Option<usize>,
    #[schemars(
        description = "The lengua libraries the program uses, each with its folder and its own label"
    )]
    libraries: Vec<PartReply>,
    #[schemars(
        description = "The components from other languages the program uses, each with its file and the label trusted"
    )]
    components: Vec<PartReply>,
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
    if params.stdin.is_some()
        && !grants.reach(lengua_idea::Reach::StandardInput)
        && grants.server.is_none()
    {
        return Some(
            "the idea does not grant `- Reads: standard input`, so the program gets an empty standard input; add the grant, or call run with no stdin".to_string(),
        );
    }
    None
}

fn seal_program(
    folder: &Path,
    report: &mut lengua_idea::Report,
    snapshot: Option<&(String, Status)>,
    key: &Key,
) {
    let (Some(plan), Some((inputs, status))) = (report.build.as_ref(), snapshot) else {
        return;
    };
    let done = report.findings.is_empty() && *inputs == plan.inputs && status.stage == Stage::Done;
    let Some(component) = status.component.as_ref().filter(|_| done) else {
        return;
    };
    let library = plan.library;
    let facts = Facts {
        kind: if library {
            Kind::Library
        } else {
            Kind::Program
        },
        toolchain: status.toolchain.clone(),
        trust: report.trust,
        files: lock::program_files(folder),
        libraries: plan.libraries.iter().map(|l| l.lock.clone()).collect(),
        trusted: plan.components.iter().map(|c| c.file.clone()).collect(),
        component: Some(plan.artifact.clone()),
        sealed: component
            .seal
            .as_ref()
            .filter(|_| !library)
            .map(|s| s.sealed.clone()),
        sealed_trusted: component.trusted.clone(),
    };
    match lock::write(folder, &folder.join(lock::LOCK), &facts, key) {
        Ok(()) => report.written.push(lock::LOCK.to_string()),
        Err(message) => report.findings.push(lengua_idea::Finding {
            file: lock::LOCK.to_string(),
            line: 1,
            code: "lock-write",
            message,
        }),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn build_reply(
    target: &str,
    status: Status,
    findings: &mut Vec<FindingReply>,
    notes: &mut Vec<NoteReply>,
) -> BuildReply {
    let tested = !matches!(status.stage, Stage::Building | Stage::BuildFailed);
    let mutated = matches!(status.stage, Stage::Mutating | Stage::Done);
    let t = status.tests;
    let m = status.mutants;
    findings.extend(status.problems.into_iter().map(|p| FindingReply {
        file: p.file,
        line: p.line,
        code: p.code,
        message: p.message,
    }));
    notes.extend(status.remarks.into_iter().map(|r| NoteReply {
        file: r.file,
        line: r.line,
        message: r.message,
    }));
    let component = status.component;
    BuildReply {
        state: status.stage.name().to_string(),
        seconds: status.seconds,
        target: target.to_string(),
        component: component
            .as_ref()
            .map(|c| c.path.to_string_lossy().to_string()),
        size: component.as_ref().map(|c| c.size),
        sha256: component.map(|c| c.sha256),
        tests: tested.then_some(TestsReply {
            total: t.total,
            run: t.run,
            failed: t.failed,
        }),
        mutants: mutated.then_some(MutantsReply {
            total: m.total,
            done: m.done,
            caught: m.caught,
            survived: m.survived,
            unbuilt: m.unbuilt,
            same: m.same,
            cached: m.cached,
        }),
    }
}

impl Lengua {
    fn reply(
        &self,
        folder: &Path,
        report: lengua_idea::Report,
        snapshot: Option<(String, Status)>,
    ) -> CheckReply {
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
        let mut notes: Vec<NoteReply> = report
            .notes
            .into_iter()
            .map(|n| NoteReply {
                file: n.file,
                line: None,
                message: n.message,
            })
            .collect();
        let (libraries, components) = report.build.as_ref().map_or_else(
            || (Vec::new(), Vec::new()),
            |plan| {
                let libraries = plan
                    .libraries
                    .iter()
                    .map(|l| PartReply {
                        path: l.path.clone(),
                        label: l.label.clone(),
                    })
                    .collect();
                let components: Vec<PartReply> = plan
                    .components
                    .iter()
                    .map(|c| PartReply {
                        path: c.file.clone(),
                        label: "trusted".to_string(),
                    })
                    .collect();
                (libraries, components)
            },
        );
        if let Some(part) = components.first() {
            notes.push(NoteReply {
                file: "idea.md".to_string(),
                line: None,
                message: format!(
                    "the program uses a trusted component, `{}`: lengua did not build it or check its code, and it runs under the program's grants",
                    part.path
                ),
            });
        }
        let build = report.build.map(|plan| {
            if plan.code {
                let status = snapshot
                    .filter(|(inputs, _)| *inputs == plan.inputs)
                    .map_or_else(|| self.builds.status(folder, &plan), |(_, status)| status);
                build_reply(plan.target, status, &mut findings, &mut notes)
            } else {
                BuildReply {
                    state: "no_code".to_string(),
                    seconds: 0,
                    target: plan.target.to_string(),
                    component: None,
                    size: None,
                    sha256: None,
                    tests: None,
                    mutants: None,
                }
            }
        });
        let score = report
            .trust
            .score
            .and_then(|(caught, counted)| caught.checked_mul(100)?.checked_div(counted));
        CheckReply {
            ok: findings.is_empty(),
            checked_by: self.name.to_string(),
            findings,
            notes,
            written: report.written,
            libraries,
            components,
            label: report.trust.label.name().to_string(),
            score,
            build,
        }
    }
}

#[tool_router]
impl Lengua {
    #[tool(
        description = "Checks a lengua program: its ideas (idea.md, ideas/*.md, ideas/ui/*.md) and the rules of its Rust code, then, in the background, builds it to a WebAssembly component, runs the examples of its ideas as tests, and measures the tests with mutants of the code. Returns every problem at once, each with its file, line, code and message. When the ideas have no problems, it writes the frontmatter of each idea, index.md, log.md, Cargo.toml and wit/world.wit, and starts the job; the reply gives its state: building, build_failed, testing, tests_failed (each failed test a finding, with what was expected and what came), mutating or done, and the trust label: checked when every test passes and the mutation score is 80% or more, tested when the score is under 80%, unchecked until then. Each mutant that survived is a note with its file and line. When the job is done, the same call writes lengua.lock: the SHA-256 of every idea, index.md, log.md, the code, the generated files, the component and its sealed precompiled code, signed with the user's key. Call check again to get the state of the job."
    )]
    fn check(
        &self,
        Parameters(CheckParams { folder }): Parameters<CheckParams>,
    ) -> Result<Json<CheckReply>, String> {
        let folder = PathBuf::from(folder);
        if !folder.is_dir() {
            return Err(format!("{} is not a folder", folder.display()));
        }
        let key = lock::user_key_or_create(|bytes| {
            getrandom::fill(bytes)
                .map_err(|e| format!("the system gave lengua no random numbers: {e}"))
        })?;
        let mut snapshot: Option<(String, Status)> = None;
        let mut report = lengua_idea::check_with(&folder, now(), |plan| {
            snapshot = if plan.code {
                self.builds
                    .snapshot(&folder, plan)
                    .map(|status| (plan.inputs.clone(), status))
            } else {
                None
            };
            snapshot
                .as_ref()
                .filter(|(_, status)| !status.stage.running())
                .map(|(_, status)| status.trust())
        });
        seal_program(&folder, &mut report, snapshot.as_ref(), &key);
        Ok(Json(self.reply(&folder, report, snapshot)))
    }

    #[tool(
        description = "Runs a lengua program that check built and locked, in a process of its own, under exactly the grants of its idea. It refuses the program when a file is not as its lengua.lock signed it (the reply names the file; call check again). It grants standard output and standard error always; the arguments, standard input, the wall clock, random numbers and the folders only when the idea grants them; the memory and the time of the idea as limits. Returns the state (exited, out_of_time, out_of_memory, trapped or refused), the exit code, standard output and standard error. When the run takes more than about 30 seconds, the state is running: call run again with the same input to get its result."
    )]
    fn run(&self, Parameters(params): Parameters<RunParams>) -> Result<Json<RunReply>, String> {
        let folder = PathBuf::from(&params.folder);
        if !folder.is_dir() {
            return Err(format!("{} is not a folder", folder.display()));
        }
        let lock_path = folder.join(lock::LOCK);
        let verified =
            lock::user_key().and_then(|key| lock::verify(&folder, &lock_path, Kind::Program, &key));
        if let Err(message) = verified {
            return Ok(Json(plain("refused", Some(message), 0)));
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
            root: None,
        };
        Ok(Json(run_reply(self.runs.run(&folder, &build, input))))
    }
}

#[tool_handler(
    name = "lengua-mcp",
    instructions = "lengua checks, builds, tests and runs programs written as idea files and Rust. Call check with the absolute path of a program's folder; while the state is building, testing or mutating, call check again. When it is done, the reply has the trust label; call run to run the program under the grants of its idea."
)]
#[allow(clippy::unused_async_trait_impl)]
impl ServerHandler for Lengua {}

#[tokio::main(flavor = "current_thread")]
async fn serve() -> Result<(), String> {
    let host = Host::new()?;
    let lengua = Lengua {
        name: VERSION,
        builds: Builds::new(host),
        runs: Runs::default(),
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

fn print_imports(path: &str) -> ExitCode {
    let found = Host::new().and_then(|host| {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let component = host.component(&bytes, &lengua_idea::sha256_hex(&bytes))?;
        Ok(host.imports(&component))
    });
    match found {
        Ok(mut names) => {
            names.sort();
            for name in names {
                println!("{name}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", env!("CARGO_PKG_NAME"));
            ExitCode::FAILURE
        }
    }
}

fn write_wiki(root: &Path) -> ExitCode {
    match lengua_idea::write_wiki_indexes(root) {
        Ok(written) => {
            for file in written {
                println!("wrote {file}");
            }
        }
        Err(error) => {
            eprintln!("{}: {error}", env!("CARGO_PKG_NAME"));
            return ExitCode::FAILURE;
        }
    }
    let findings = lengua_idea::wiki_findings(root);
    for f in &findings {
        println!("{}:{}: {}: {}", f.file, f.line, f.code, f.message);
    }
    if findings.is_empty() {
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
    if let [command, path] = args.as_slice()
        && command == "imports"
    {
        return print_imports(path);
    }
    if let [command, root] = args.as_slice()
        && command == "wiki"
    {
        return write_wiki(Path::new(root));
    }
    if let Some((command, rest)) = args.split_first()
        && command == "worker"
    {
        return worker::serve(rest);
    }
    if !args.is_empty() {
        eprintln!(
            "usage: {} [--version | doctor | wiki <root>]",
            env!("CARGO_PKG_NAME")
        );
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
