//! `lengua-mcp`: the lengua MCP server, speaking MCP over stdio.

use std::path::PathBuf;
use std::process::ExitCode;

use lengua::data_dir::DataDir;
use lengua::server::LenguaServer;
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: lengua-mcp [--data-dir <absolute path>] [--version]";

#[tokio::main]
async fn main() -> ExitCode {
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
        Ok(Command::Version) => {
            println!("lengua-mcp {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("lengua-mcp: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };

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
    Version,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut data_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
