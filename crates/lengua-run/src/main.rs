use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::Duration;

use lengua_host::{Ending, Host, Input, Program, Streams};
use lengua_idea::Reach;
use lengua_idea::lock::{self, Kind};

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
const GRACE_MS: u64 = 2000;
const OUT_OF_TIME: u8 = 124;
const REFUSED: u8 = 125;
const OUT_OF_MEMORY: u8 = 126;
const TRAPPED: u8 = 134;
const NATIVE: &str = "the folder is a native part: it runs outside the sandbox, beside the WebAssembly program that names it with `- Native:`; run that program";

fn refuse(message: &str) -> ExitCode {
    eprintln!("lengua-run: refused: {message}");
    ExitCode::from(REFUSED)
}

fn ended(name: &str, message: &str, code: u8) -> ExitCode {
    let _ = std::io::stdout().flush();
    eprintln!("lengua-run: {name}: {message}");
    ExitCode::from(code)
}

fn watch(limit: Duration) {
    thread::spawn(move || {
        thread::sleep(limit);
        let _ = std::io::stdout().flush();
        eprintln!(
            "lengua-run: out of time: the program waited past the time of its idea, and lengua-run stopped it"
        );
        std::process::exit(i32::from(OUT_OF_TIME));
    });
}

fn electron(folder: &Path) -> Result<PathBuf, String> {
    let package = folder.join("target/lengua/web/node_modules/electron");
    let inside = fs::read_to_string(package.join("path.txt")).map_err(|_| {
        "Electron is not installed for this app; call check on its folder, which installs it with the front end".to_string()
    })?;
    Ok(package.join("dist").join(inside.trim()))
}

fn open_window(shell: PathBuf, main: PathBuf, address: String) {
    thread::spawn(move || {
        for _ in 0..200 {
            if TcpStream::connect(&address).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let status = Command::new(&shell)
            .arg(&main)
            .env("LENGUA_URL", format!("http://{address}/"))
            .status();
        let _ = std::io::stdout().flush();
        let code = match status {
            Ok(status) => status.code().unwrap_or(1),
            Err(error) => {
                eprintln!("lengua-run: refused: Electron did not start: {error}");
                i32::from(REFUSED)
            }
        };
        std::process::exit(code);
    });
}

fn not_this_kind(build: Option<&lengua_idea::Build>, desktop: bool) -> Option<&'static str> {
    if build.is_some_and(lengua_idea::Build::native) {
        return Some(NATIVE);
    }
    (desktop && !build.is_some_and(lengua_idea::Build::desktop)).then_some(
        "the idea is not a desktop app: it has no `- Native: a desktop window, for the Electron shell of a desktop app`",
    )
}

fn run(folder: &Path, arguments: &[String], listen: Option<&str>, desktop: bool) -> ExitCode {
    let first = lengua_idea::program(folder);
    if let Some(problem) = not_this_kind(first.build.as_ref(), desktop) {
        return refuse(problem);
    }
    let key = match lock::user_key() {
        Ok(key) => key,
        Err(message) => return refuse(&message),
    };
    let verified = match lock::verify(folder, &folder.join(lock::LOCK), Kind::Program, &key) {
        Ok(verified) => verified,
        Err(message) => return refuse(&message),
    };
    let report = first;
    if !report.findings.is_empty() {
        let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
        return refuse(&format!(
            "the ideas have {} problems ({}); run `check`, and fix them first",
            codes.len(),
            codes.join(", ")
        ));
    }
    let Some(build) = report.build else {
        return refuse("lengua could not read the program");
    };
    if !arguments.is_empty() && !build.grants.reach(Reach::Arguments) {
        return refuse(
            "the idea does not grant `- Reads: arguments`, so the program gets no arguments; add the grant, or give none",
        );
    }
    let Some(sealed) = verified.sealed() else {
        return refuse("the lock seals no code to run");
    };
    let host = match Host::new(&lengua_host::config()) {
        Ok(host) => host,
        Err(message) => return refuse(&message),
    };
    let component = match lengua_seal::load(host.engine(), sealed) {
        Ok(component) => component,
        Err(message) => return refuse(&message),
    };
    let trusted = match verified
        .trusted()
        .iter()
        .map(|part| lengua_seal::load(host.engine(), part))
        .collect::<Result<Vec<_>, String>>()
    {
        Ok(trusted) => trusted,
        Err(message) => return refuse(&message),
    };
    let program = Program {
        folder,
        component: &component,
        trusted: &trusted,
        build: &build,
    };
    if let Some(port) = build.grants.server {
        let address = listen.map_or_else(|| format!("127.0.0.1:{port}"), str::to_string);
        eprintln!("lengua-run: serving HTTP on {address}");
        if desktop {
            match electron(folder) {
                Ok(shell) => open_window(shell, folder.join(lengua_idea::DESKTOP), address.clone()),
                Err(message) => return refuse(&message),
            }
        }
        return match host.serve(&program, &address, false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => refuse(&message),
        };
    }
    watch(Duration::from_millis(
        build.grants.time_ms.saturating_add(GRACE_MS),
    ));
    let input = Input {
        arguments: arguments.to_vec(),
        stdin: Vec::new(),
        fixed: false,
        root: None,
        model: None,
    };
    let outcome = host.run(&program, &input, Streams::Inherit);
    let _ = std::io::stdout().flush();
    match outcome.ending {
        Ending::Exited(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Ending::OutOfTime(message) => ended("out of time", &message, OUT_OF_TIME),
        Ending::OutOfMemory => ended(
            "out of memory",
            "the program asked for more memory than its idea allows",
            OUT_OF_MEMORY,
        ),
        Ending::Trapped(message) => ended("trapped", &message, TRAPPED),
        Ending::Refused(message) => refuse(&message),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((first, rest)) = args.split_first() else {
        eprintln!(
            "usage: {} [--listen <address:port>] <folder> [arguments...] | --desktop <folder>",
            env!("CARGO_PKG_NAME")
        );
        return ExitCode::from(2);
    };
    if first == "--version" && rest.is_empty() {
        println!("{VERSION}");
        return ExitCode::SUCCESS;
    }
    if first == "--listen" {
        return match rest {
            [address, folder, more @ ..] => {
                run(Path::new(folder), more, Some(address.as_str()), false)
            }
            _ => refuse("`--listen` takes an address and a port, then the folder"),
        };
    }
    if first == "--desktop" {
        return match rest {
            [folder] => run(Path::new(folder), &[], None, true),
            _ => refuse("`--desktop` takes the folder of a desktop app"),
        };
    }
    run(Path::new(first), rest, None, false)
}
