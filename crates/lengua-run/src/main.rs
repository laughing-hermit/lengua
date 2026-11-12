//! `lengua-run`: runs a program lengua checked, from a shell, a script or a service. It is
//! lengua built without the engine's compiler, so the process maps megabytes less than
//! lengua-mcp, starts sooner and holds less. It makes every check `lengua-mcp run` makes, then
//! loads the module check tested in the form check compiled for this computer, once that
//! carries the seal of the user's key, and runs it in lengua's host.
//!
//! What needs the compiler it leaves to lengua-mcp, the program beside it, which it starts with
//! the same words: a module with no compiled form this computer takes, which lengua-mcp
//! compiles, keeps and runs, and an idea that names a secret, whose machine code lengua-mcp
//! reads before each run.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use lengua::data_dir::DataDir;
use lengua::server::{NotRun, ShellRun};

const USAGE: &str = "usage: lengua-run [--data-dir <absolute path>] <program folder> \
[--in <folder>] [--env NAME=value]... [--trace] [--profile] [--stop-at <line or $function>] \
[--show-memory <address>:<length>] [[--] <argument>...]
       lengua-run --version";

/// The exit code when lengua won't run a program or can't start it, as `lengua-mcp run` gives
/// it: the program's own codes stay its own.
const REFUSED: u8 = 125;

/// The program beside this one that compiles.
const LENGUA_MCP: &str = "lengua-mcp";

/// What lengua-run was asked.
enum Asked {
    /// To say its version.
    Version,
    /// To run a checked program, with the data folder it names or the user's own.
    Run(DataDir, ShellRun),
}

/// What `args`, the command line's words, ask.
fn asked(mut args: impl Iterator<Item = String>) -> Result<Asked, String> {
    let mut data_dir = None;
    let mut first = args.next();
    if first.as_deref() == Some("--data-dir") {
        let path = PathBuf::from(args.next().ok_or("--data-dir needs a path")?);
        if !path.is_absolute() {
            return Err("--data-dir must be an absolute path".to_owned());
        }
        data_dir = Some(DataDir::new(path));
        first = args.next();
    }
    if first.as_deref() == Some("--version") {
        return Ok(Asked::Version);
    }
    let request = ShellRun::parse(first.into_iter().chain(args))?;
    let data_dir = data_dir
        .or_else(DataDir::from_env)
        .ok_or(lengua::data_dir::NOT_FOUND)?;
    Ok(Asked::Run(data_dir, request))
}

/// Leaves the run to lengua-mcp, beside this program, given `words`: on a Unix this process
/// becomes lengua-mcp, and elsewhere it starts lengua-mcp with its own streams and waits for
/// it. Gives lengua-mcp's exit code, or [`REFUSED`] when it can't be started, saying `why` the
/// run needed it.
fn hand_over(words: impl Iterator<Item = OsString>, why: &str) -> ExitCode {
    let beside = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map(|own| own.with_file_name(format!("{LENGUA_MCP}{}", std::env::consts::EXE_SUFFIX)));
    let lengua_mcp = match beside {
        Ok(path) if path.is_file() => path,
        _ => {
            eprintln!(
                "lengua-run: {why}, which takes lengua-mcp, and lengua-mcp isn't beside \
                 lengua-run: run the program with `lengua-mcp run`"
            );
            return ExitCode::from(REFUSED);
        }
    };
    let mut command = Command::new(&lengua_mcp);
    command.args(words);
    #[cfg(unix)]
    let failed = {
        use std::os::unix::process::CommandExt as _;
        command.exec()
    };
    #[cfg(not(unix))]
    let failed = match command.spawn() {
        Ok(mut child) => {
            // lengua-mcp ends when this process does, however that is, as on a Unix, where
            // the two are one process: without the tie, whoever stopped lengua-run would
            // leave lengua-mcp and the program running.
            #[cfg(windows)]
            let _tied = lengua_os::tie(&child);
            match child.wait() {
                Ok(status) => {
                    let code = status.code().and_then(|code| u8::try_from(code).ok());
                    return ExitCode::from(code.unwrap_or(REFUSED));
                }
                Err(error) => error,
            }
        }
        Err(error) => error,
    };
    eprintln!(
        "lengua-run: couldn't start {}: {failed}",
        lengua_mcp.display()
    );
    ExitCode::from(REFUSED)
}

fn main() -> ExitCode {
    // This process compiles nothing, also where its build holds a compiler.
    lengua::wasm::engine::run_only();
    let mut raw = std::env::args_os().skip(1).peekable();
    // Module mode, hidden: `lengua-mcp measure` runs a module of another toolchain in lengua's
    // host with it, the module's run given as JSON.
    if raw
        .peek()
        .is_some_and(|arg| arg == lengua::measure::MODULE_FLAG)
    {
        let words: Vec<OsString> = raw.collect();
        let spec = words
            .get(1)
            .and_then(|spec| spec.to_str())
            .unwrap_or_default();
        let ran = lengua::measure::run_module_alone(spec);
        return match ran {
            Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(REFUSED)),
            Err(NotRun::Refused(message)) => {
                eprintln!("lengua-run: {message}");
                ExitCode::from(REFUSED)
            }
            Err(NotRun::NotCompiled(_) | NotRun::Secret | NotRun::Tool(_)) => hand_over(
                words.into_iter(),
                "the module isn't compiled for this computer yet",
            ),
        };
    }
    // Every system passes Unicode arguments, as Windows's UTF-16 does: lengua takes no others.
    let words: Vec<OsString> = raw.collect();
    let arguments: Result<Vec<String>, _> =
        words.iter().cloned().map(OsString::into_string).collect();
    let Ok(arguments) = arguments else {
        eprintln!(
            "lengua-run: an argument isn't Unicode text, which not every system can pass on, so \
             lengua takes none"
        );
        return ExitCode::from(REFUSED);
    };
    let (data_dir, request) = match asked(arguments.into_iter()) {
        Ok(Asked::Run(data_dir, request)) => (data_dir, request),
        Ok(Asked::Version) => {
            let compiler = if lengua::wasm::engine::BUILT_WITH_COMPILER {
                ", built with the whole workspace: it holds a compiler it never calls"
            } else {
                ""
            };
            println!("lengua-run {}{compiler}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("lengua-run: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let ran = lengua::server::run_alone(&data_dir, &request);
    // lengua-mcp takes the same words after `run`, and the data folder before it.
    let for_lengua_mcp = || {
        let named = words.first().is_some_and(|word| word == "--data-dir");
        let split = if named { 2 } else { 0 };
        let (before, after) = words.split_at(split.min(words.len()));
        before
            .iter()
            .cloned()
            .chain(std::iter::once(OsString::from("run")))
            .chain(after.iter().cloned())
            .collect::<Vec<_>>()
            .into_iter()
    };
    match ran {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(REFUSED)),
        Err(NotRun::Refused(message)) => {
            eprintln!("lengua-run: {message}");
            ExitCode::from(REFUSED)
        }
        Err(NotRun::NotCompiled(why)) => {
            // Said once, when it happens: the next run finds what lengua-mcp compiled.
            eprintln!(
                "lengua-run: the program's module has no compiled form to load here ({why}), so \
                 lengua-mcp compiles it for this computer and runs it"
            );
            hand_over(for_lengua_mcp(), "the program's module has to be compiled")
        }
        // A profile takes an engine whose clock moves, and a stop a copy of main.wat built for
        // the run: both are lengua-mcp's to compile.
        Err(NotRun::Tool(why)) => hand_over(for_lengua_mcp(), &why),
        Err(NotRun::Secret) => hand_over(
            for_lengua_mcp(),
            "the idea names a secret, so the machine code is read before each run",
        ),
    }
}
