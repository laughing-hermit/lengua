//! `lengua-mcp`: the lengua MCP server, speaking MCP over stdio; `lengua-mcp run`, which runs a
//! checked program from a shell; `lengua-mcp check`, which attests a WebAssembly program's folder
//! on any computer, as its idea's attester; `lengua-mcp measure`, which runs a program in a
//! process of its own and prints what it cost; and `lengua-mcp install`, which installs lengua
//! from its unpacked release archive.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use lengua::data_dir::DataDir;
use lengua::server::{LenguaServer, ShellRun};
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: lengua-mcp [--data-dir <absolute path>] [--version]
       lengua-mcp [--data-dir <absolute path>] run <program folder> [--in <folder>] \
[--env NAME=value]... [[--] <argument>...]
       lengua-mcp check <program folder>
       lengua-mcp [--data-dir <absolute path>] doctor
       lengua-mcp [--data-dir <absolute path>] install [--check] [--link] [--prefix <folder>]
       lengua-mcp wasm <main.wat> [-o <main.wasm>]
       lengua-mcp [--data-dir <absolute path>] measure [--in <folder>] [--stdin <file>] \
[--stdout <file>] [--stderr <file>] [--env NAME=value]... [--uses <words>] [--network <line>] \
[--serve <port> [--clients <n>] [--requests <n>]] [--counters] <program folder, module.wasm or \
executable> [[--] <argument>...]";

/// The exit code of `lengua-mcp check` when the folder isn't attested.
const NOT_ATTESTED: u8 = 1;

/// The exit code of `lengua-mcp doctor` when it finds a problem.
const PROBLEMS_FOUND: u8 = 1;

/// The exit code when lengua won't run a program or can't start it, as `timeout` and `env` use:
/// the program's own codes stay its own.
const REFUSED: u8 = 125;

fn main() -> ExitCode {
    let mut raw = std::env::args_os().skip(1).peekable();
    // Worker mode, hidden: a child of the server that runs WebAssembly jobs until its standard
    // input ends.
    if raw
        .peek()
        .is_some_and(|arg| arg == lengua::wasm::WORKER_FLAG)
    {
        raw.next();
        return lengua::wasm::worker::main(raw);
    }
    // Module mode, hidden: a child of `measure` that runs one WebAssembly module of another
    // toolchain in lengua's host, its run given as JSON.
    if raw
        .peek()
        .is_some_and(|arg| arg == lengua::measure::MODULE_FLAG)
    {
        raw.next();
        let spec = raw
            .next()
            .and_then(|spec| spec.into_string().ok())
            .unwrap_or_default();
        let code = lengua::measure::run_module(&spec);
        return ExitCode::from(u8::try_from(code).unwrap_or(REFUSED));
    }

    // Every system passes Unicode arguments, as Windows's UTF-16 does: lengua takes no others.
    let arguments: Result<Vec<String>, _> = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect();
    let Ok(arguments) = arguments else {
        eprintln!(
            "lengua-mcp: an argument isn't Unicode text, which not every system can pass on, so \
             lengua takes none"
        );
        return ExitCode::from(REFUSED);
    };
    let command = parse_args(arguments.into_iter());
    // A shell's run of main.wat logs nothing, as a module's run doesn't: nothing on its way logs,
    // and the engine's warnings would land among the program's own output. LENGUA_LOG brings
    // them back, for a developer; every other command logs as it did.
    if !matches!(command, Ok(Command::Run(..))) || std::env::var_os("LENGUA_LOG").is_some() {
        log_to_stderr();
    }
    let data_dir = match command {
        Ok(Command::Serve(data_dir)) => data_dir,
        Ok(Command::Run(data_dir, request)) => return run_from_shell(&data_dir, &request),
        Ok(Command::Check(path)) => return attest(&path),
        Ok(Command::Doctor(data_dir)) => return doctor(data_dir.as_ref()),
        Ok(Command::Install(data_dir, options)) => {
            return ExitCode::from(lengua::install::install(&options, data_dir.as_ref()));
        }
        Ok(Command::Wasm(path, output)) => return wasm(&path, output),
        Ok(Command::Measure(data_dir, request)) => return measure(data_dir.as_ref(), &request),
        Ok(Command::Version) => {
            println!("lengua-mcp {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("lengua-mcp: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(serve(data_dir)),
        Err(error) => {
            eprintln!("lengua-mcp: could not start: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Logs to stderr, at the level `LENGUA_LOG` says, or warnings: stdout carries the MCP protocol,
/// and the `log` records of the engine and its cache come here too.
fn log_to_stderr() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("LENGUA_LOG").unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_ansi(false)
        .init();
}

/// Runs a checked program attached to this shell, exiting with its exit code: in a plain
/// function, with no async runtime built around it, as nothing on its way waits on one.
fn run_from_shell(data_dir: &DataDir, request: &ShellRun) -> ExitCode {
    match lengua::server::run_from_shell(data_dir, request) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(REFUSED)),
        Err(message) => {
            eprintln!("lengua-mcp: {message}");
            ExitCode::from(REFUSED)
        }
    }
}

/// Attests the program folder at `path`, as its idea's attester: prints one line for each thing
/// it finds, the verdict last, and exits 0 when the folder is attested and 1 when it isn't. It
/// writes nothing and needs no data folder.
fn attest(path: &str) -> ExitCode {
    let attestation = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(lengua::server::attest(path)),
        Err(error) => {
            println!("lengua-mcp could not start: {error}");
            println!("not attested");
            return ExitCode::from(NOT_ATTESTED);
        }
    };
    for line in &attestation.lines {
        println!("{line}");
    }
    if attestation.attested {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(NOT_ATTESTED)
    }
}

/// Checks what lengua needs on this computer, with `data_dir` its data folder: prints one line
/// for each thing it checks, a sentence under each problem saying what to do, and the verdict
/// last, and exits 0 only when nothing is wrong.
fn doctor(data_dir: Option<&DataDir>) -> ExitCode {
    let report = lengua::doctor::doctor(data_dir);
    for line in report.lines() {
        println!("{line}");
    }
    if report.problems() == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(PROBLEMS_FOUND)
    }
}

/// Writes the standard WebAssembly module main.wat at `path` builds, with lengua's library merged
/// in, to `output`, or beside main.wat as main.wasm: a WASI preview 1 module any WebAssembly
/// runtime runs.
fn wasm(path: &std::path::Path, output: Option<PathBuf>) -> ExitCode {
    let output = output.unwrap_or_else(|| path.with_extension("wasm"));
    let built = std::fs::read_to_string(path)
        .map_err(|error| format!("can't read {}: {error}", path.display()))
        .and_then(|text| lengua::wasm::text::build(&text).map_err(|problem| problem.to_string()))
        .and_then(|built| {
            std::fs::write(&output, built.wasm)
                .map_err(|error| format!("can't write {}: {error}", output.display()))
        });
    match built {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lengua-mcp: {message}");
            ExitCode::from(REFUSED)
        }
    }
}

/// Runs a program once in a process of its own and prints what it cost, one line of JSON: a
/// main.wat folder as `run` runs it, a module in lengua's host, or an executable.
fn measure(data_dir: Option<&DataDir>, request: &lengua::measure::Request) -> ExitCode {
    let printed = lengua::measure::measure(data_dir, request).and_then(|measures| {
        serde_json::to_string(&measures).map_err(|error| format!("the measures: {error}"))
    });
    match printed {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("lengua-mcp: {message}");
            ExitCode::from(REFUSED)
        }
    }
}

async fn serve(data_dir: DataDir) -> ExitCode {
    let server = LenguaServer::new(data_dir);
    let running = match server.serve(stdio()).await {
        Ok(running) => running,
        Err(error) => {
            tracing::error!("could not start the MCP session: {error}");
            return ExitCode::FAILURE;
        }
    };
    match running.waiting().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("the MCP session ended with an error: {error}");
            ExitCode::FAILURE
        }
    }
}

enum Command {
    Serve(DataDir),
    Run(DataDir, ShellRun),
    Check(String),
    Doctor(Option<DataDir>),
    Install(Option<DataDir>, lengua::install::Options),
    Wasm(PathBuf, Option<PathBuf>),
    Measure(Option<DataDir>, Box<lengua::measure::Request>),
    Version,
}

/// `run`'s own words: the program folder, `--in` and `--env` options, then the program's
/// arguments, after `--` or from the first word that isn't an option, which an argument that
/// starts with `-` needs.
fn parse_run(mut args: impl Iterator<Item = String>) -> Result<ShellRun, String> {
    let path = args.next().ok_or("run needs a program folder")?;
    let mut request = ShellRun {
        path,
        folder: None,
        env: BTreeMap::new(),
        args: Vec::new(),
    };
    while let Some(arg) = args.next() {
        // The first word that isn't an option starts the program's arguments.
        if !arg.starts_with('-') {
            request.args = std::iter::once(arg).chain(args).collect();
            break;
        }
        match arg.as_str() {
            "--in" => request.folder = Some(args.next().ok_or("--in needs a folder")?),
            "--env" => {
                let pair = args.next().ok_or("--env needs NAME=value")?;
                let (name, value) = pair
                    .split_once('=')
                    .ok_or_else(|| format!("--env {pair:?} isn't NAME=value"))?;
                request.env.insert(name.to_owned(), value.to_owned());
            }
            "--" => {
                request.args = args.collect();
                break;
            }
            other => {
                return Err(format!(
                    "unknown option {other:?}: put the program's arguments after --"
                ));
            }
        }
    }
    Ok(request)
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut data_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "run" => {
                let request = parse_run(args)?;
                let data_dir = data_dir
                    .or_else(DataDir::from_env)
                    .ok_or(lengua::data_dir::NOT_FOUND)?;
                return Ok(Command::Run(data_dir, request));
            }
            "check" => {
                let path = args.next().ok_or("check needs a program folder")?;
                if let Some(extra) = args.next() {
                    return Err(format!("check takes one program folder, not {extra:?}"));
                }
                return Ok(Command::Check(path));
            }
            "doctor" => {
                if let Some(extra) = args.next() {
                    return Err(format!("doctor takes no more words, not {extra:?}"));
                }
                return Ok(Command::Doctor(data_dir.or_else(DataDir::from_env)));
            }
            "install" => {
                let options = lengua::install::Options::parse(args)?;
                return Ok(Command::Install(data_dir, options));
            }
            "wasm" => {
                let path = args.next().ok_or("wasm needs a main.wat")?;
                let mut output = None;
                while let Some(arg) = args.next() {
                    match arg.as_str() {
                        "-o" => output = Some(PathBuf::from(args.next().ok_or("-o needs a file")?)),
                        other => {
                            return Err(format!(
                                "wasm takes a main.wat and -o <file>, not {other:?}"
                            ));
                        }
                    }
                }
                return Ok(Command::Wasm(PathBuf::from(path), output));
            }
            "measure" => {
                let request = lengua::measure::Request::parse(args)?;
                return Ok(Command::Measure(
                    data_dir.or_else(DataDir::from_env),
                    Box::new(request),
                ));
            }
            "--data-dir" => {
                let value = args.next().ok_or("--data-dir needs a path")?;
                let path = PathBuf::from(value);
                if !path.is_absolute() {
                    return Err("--data-dir must be an absolute path".to_owned());
                }
                data_dir = Some(DataDir::new(path));
            }
            "--version" => return Ok(Command::Version),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    match data_dir.or_else(DataDir::from_env) {
        Some(data_dir) => Ok(Command::Serve(data_dir)),
        None => Err(lengua::data_dir::NOT_FOUND.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list` as a command line's words.
    fn words(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter().map(|word| (*word).to_owned())
    }

    /// A shell run's program arguments follow its folder, after `--` or from the first word that
    /// isn't an option; an argument that starts with `-` needs `--`.
    #[test]
    fn a_shell_run_takes_the_program_s_arguments_after_its_folder() {
        let args = |list: &[&str]| parse_run(words(list)).map(|request| request.args);
        assert_eq!(
            args(&["./add", "2", "3"]),
            Ok(vec!["2".to_owned(), "3".to_owned()])
        );
        assert_eq!(
            args(&["./add", "--", "-2", "3"]),
            Ok(vec!["-2".to_owned(), "3".to_owned()])
        );
        let request = parse_run(words(&[
            "./add", "--in", "/tmp", "--env", "A=b", "2", "--in",
        ]))
        .expect("a shell run");
        assert_eq!(request.folder.as_deref(), Some("/tmp"));
        assert_eq!(request.env.get("A").map(String::as_str), Some("b"));
        assert_eq!(request.args, ["2", "--in"]);
        assert!(args(&["./add", "-2"]).is_err());
    }

    /// `check` takes one program folder, and no data folder.
    #[test]
    fn check_takes_one_program_folder() {
        assert!(matches!(
            parse_args(words(&["check", "./add"])),
            Ok(Command::Check(path)) if path == "./add"
        ));
        assert!(parse_args(words(&["check"])).is_err());
        assert!(parse_args(words(&["check", "./add", "./sum"])).is_err());
    }

    /// `doctor` takes the data folder `--data-dir` names before it, and no more words.
    #[test]
    fn doctor_takes_the_data_folder_named_before_it() {
        let folder = if cfg!(windows) { "C:\\data" } else { "/data" };
        assert!(matches!(
            parse_args(words(&["--data-dir", folder, "doctor"])),
            Ok(Command::Doctor(Some(data_dir))) if data_dir.path() == std::path::Path::new(folder)
        ));
        assert!(parse_args(words(&["doctor", "now"])).is_err());
    }

    /// `install` takes its own words, and the data folder `--data-dir` names before it.
    #[test]
    fn install_takes_its_own_words() {
        assert!(matches!(
            parse_args(words(&["install", "--check"])),
            Ok(Command::Install(None, options)) if options.check
        ));
        assert!(parse_args(words(&["install", "--now"])).is_err());
    }
}
