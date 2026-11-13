use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
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

fn run(folder: &Path, arguments: &[String]) -> ExitCode {
    let key = match lock::user_key() {
        Ok(key) => key,
        Err(message) => return refuse(&message),
    };
    let verified = match lock::verify(folder, &folder.join(lock::LOCK), Kind::Program, &key) {
        Ok(verified) => verified,
        Err(message) => return refuse(&message),
    };
    let report = lengua_idea::program(folder);
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
        eprintln!("lengua-run: serving HTTP on 127.0.0.1:{port}");
        return match host.serve(&program, port, false) {
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
        eprintln!("usage: {} <folder> [arguments...]", env!("CARGO_PKG_NAME"));
        return ExitCode::from(2);
    };
    if first == "--version" && rest.is_empty() {
        println!("{VERSION}");
        return ExitCode::SUCCESS;
    }
    run(Path::new(first), rest)
}
