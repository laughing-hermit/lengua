//! `lengua-mcp`: the lengua MCP server, speaking MCP over stdio, and `lengua-mcp run`, which runs
//! a checked program from a shell.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use lengua::data_dir::DataDir;
use lengua::runner;
use lengua::server::{LenguaServer, ShellRun};
use lengua::supervise;
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: lengua-mcp [--data-dir <absolute path>] [--version]
       lengua-mcp [--data-dir <absolute path>] run <program folder> [--in <folder>] \
[--env NAME=value]... [-- <argument>...]";

/// The exit code when lengua won't run a program or can't start it, as `timeout` and `env` use:
/// the program's own codes stay its own.
const REFUSED: u8 = 125;

fn main() -> ExitCode {
    // Launcher mode runs before anything else, with no threads, and replaces this process.
    let mut raw = std::env::args_os().skip(1).peekable();
    if raw.peek().is_some_and(|arg| arg == runner::LAUNCH_FLAG) {
        raw.next();
        return runner::launch(raw);
    }
    // So does supervisor mode, which traces a launcher for a test that supplies values.
    if raw
        .peek()
        .is_some_and(|arg| arg == supervise::SUPERVISE_FLAG)
    {
        return supervise::supervise();
    }

    // stdout carries the MCP protocol, so all logging goes to stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("LENGUA_LOG").unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_ansi(false)
        .init();

    let data_dir = match parse_args(std::env::args().skip(1)) {
        Ok(Command::Serve(data_dir)) => data_dir,
        Ok(Command::Run(data_dir, request)) => return run_from_shell(&data_dir, &request),
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

/// Runs a checked program attached to this shell, exiting with its exit code.
fn run_from_shell(data_dir: &DataDir, request: &ShellRun) -> ExitCode {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start: {error}"))
        .and_then(|runtime| runtime.block_on(lengua::server::run_from_shell(data_dir, request)));
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(REFUSED)),
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
    Version,
}

/// `run`'s own words: the program folder, `--in` and `--env` options, then the program's
/// arguments after `--`.
fn parse_run(mut args: impl Iterator<Item = String>) -> Result<ShellRun, String> {
    let path = args.next().ok_or("run needs a program folder")?;
    let mut request = ShellRun {
        path,
        folder: None,
        env: BTreeMap::new(),
        args: Vec::new(),
    };
    while let Some(arg) = args.next() {
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
                let data_dir = data_dir.or_else(DataDir::from_env).ok_or(
                    "can't find a data folder: set HOME or XDG_DATA_HOME, or pass --data-dir",
                )?;
                return Ok(Command::Run(data_dir, request));
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
        None => {
            Err("can't find a data folder: set HOME or XDG_DATA_HOME, or pass --data-dir".into())
        }
    }
}
