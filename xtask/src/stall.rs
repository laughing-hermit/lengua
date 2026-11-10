//! A program watched for a stall, as CI's test and ignored runs are when `stall_seconds` gives a
//! time: if it still runs after that long, where it is stuck goes to stall.txt (the busiest
//! processes, and a stack of every thread of the server, the test programs and the solvers, from
//! gdb) and it is stopped, so that a hang ends with its cause in the logs instead of at the job's
//! time limit. Meanwhile a line of memory use goes to the job's log every 10 seconds, so that a
//! machine that dies mid-run still says how it was going (the report is lost with it). Linux's
//! own programs do the looking (ps, pgrep, pkill, free and gdb), as on the two Linux runners.

use std::fmt::Write as _;
use std::io::Read as _;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::{ci, clock};

/// The processes whose stacks the report holds: pgrep's pattern.
const STUCK: &str = "lengua-mcp|/target/release/deps/|z3|cvc5";

/// How often the memory line goes to the log.
const METER: Duration = Duration::from_secs(10);

/// What gdb is asked of each process: a stack of every thread, 30 frames deep.
const BACKTRACE: &str = "thread apply all bt 30";

/// The lines of gdb's own that the report leaves out.
const GDB_NOISE: [&str; 3] = ["[New LWP", "warning:", "[Thread debugging"];

/// `cargo xtask stall <seconds> <program> [<argument>...]`: runs the program, watched.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    let [seconds, program, rest @ ..] = arguments else {
        return Err("usage: cargo xtask stall <seconds> <program> [<argument>...]".to_owned());
    };
    let after = limit(seconds)?;
    let mut child = Command::new(program)
        .args(rest)
        .spawn()
        .map_err(|error| format!("{program} didn't start: {error}"))?;
    let status = watch(&mut child, after, seconds)?;
    Ok(ci::code(status))
}

/// `seconds`, a number of seconds, as a time.
pub fn limit(seconds: &str) -> Result<Duration, String> {
    seconds
        .trim()
        .parse::<f64>()
        .ok()
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        .ok_or_else(|| format!("a stall's time is a number of seconds, not {seconds:?}"))
}

/// Waits for `child`, with a memory line in the log every 10 seconds; once `after` has passed
/// (`said` as it was given), writes stall.txt and stops it.
pub fn watch(child: &mut Child, after: Duration, said: &str) -> Result<ExitStatus, String> {
    let started = Instant::now();
    let (stop, stopped) = mpsc::channel::<()>();
    let meter = std::thread::spawn(move || {
        while matches!(
            stopped.recv_timeout(METER),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            println!("{}", memory());
        }
    });
    let mut reported = false;
    let waited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(error) => break Err(format!("waiting for a program: {error}")),
        }
        if !reported && started.elapsed() >= after {
            report(said);
            let _ = child.kill();
            reported = true;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    drop(stop);
    let _ = meter.join();
    waited
}

/// Writes stall.txt: when, the busiest processes, and gdb's stack of every thread of each
/// process [`STUCK`] matches; then stops the server and the test programs.
fn report(said: &str) {
    let mut text = format!(
        "still running after {said}s: {}\n",
        clock::Civil::now().date()
    );
    let busiest = ci::printed("ps", &["-eo", "pid,ppid,pcpu,etime,args", "--sort=-pcpu"]);
    for line in busiest.lines().take(30) {
        text.push_str(line);
        text.push('\n');
    }
    let gdb = Command::new("sudo")
        .args(["apt-get", "install", "-y", "-q", "gdb"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !gdb.is_ok_and(|status| status.success()) {
        text.push_str("gdb: not installed\n");
    }
    for pid in ci::printed("pgrep", &["-f", STUCK]).split_whitespace() {
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
        let _ = writeln!(text, "=== {pid}: {cmdline}");
        let stack = merged("sudo", &["gdb", "-p", pid, "-batch", "-ex", BACKTRACE]);
        for line in stack.lines() {
            if !GDB_NOISE.iter().any(|noise| line.starts_with(noise)) {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    let _ = std::fs::write("stall.txt", text);
    for pattern in ["lengua-mcp", "/target/release/deps/"] {
        let _ = Command::new("pkill").args(["-f", pattern]).status();
    }
}

/// The memory line: what `free -m` says is used and available, and the three processes that
/// hold the most.
fn memory() -> String {
    let free = ci::printed("free", &["-m"]);
    let row: Vec<&str> = free
        .lines()
        .nth(1)
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    let used = row.get(2).copied().unwrap_or_default();
    let available = row.get(6).copied().unwrap_or_default();
    let mut largest = String::new();
    let processes = ci::printed("ps", &["-eo", "rss,etime,comm", "--sort=-rss"]);
    for line in processes.lines().skip(1).take(3) {
        largest.push_str(&squeezed(line));
        largest.push(';');
    }
    format!("memory: {used} MB used, {available} MB available; largest: {largest}")
}

/// `line` with each run of spaces made one, as `tr -s ' '` does.
fn squeezed(line: &str) -> String {
    let mut squeezed = String::with_capacity(line.len());
    for character in line.chars() {
        if !(character == ' ' && squeezed.ends_with(' ')) {
            squeezed.push(character);
        }
    }
    squeezed
}

/// What `program` printed, its errors among its output as they came (`2>&1`), or what kept it
/// from starting.
fn merged(program: &str, arguments: &[&str]) -> String {
    together(program, arguments).unwrap_or_else(|error| format!("{program}: {error}\n"))
}

/// [`merged`]'s run.
fn together(program: &str, arguments: &[&str]) -> std::io::Result<String> {
    let (mut reader, writer) = std::io::pipe()?;
    let errors = writer.try_clone()?;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(errors);
    let mut child = command.spawn()?;
    drop(command);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    child.wait()?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stall's time is a number of seconds, and nothing else.
    #[test]
    fn a_stall_s_time_is_seconds() {
        assert_eq!(limit("90"), Ok(Duration::from_secs(90)));
        assert_eq!(limit(" 1.5 "), Ok(Duration::from_millis(1500)));
        assert!(limit("ten").is_err());
        assert!(limit("-1").is_err());
    }

    /// Runs of spaces become one space, as `tr -s ' '` makes them.
    #[test]
    fn runs_of_spaces_are_squeezed() {
        assert_eq!(squeezed("  1234  01:02   cargo"), " 1234 01:02 cargo");
        assert_eq!(squeezed("one"), "one");
    }

    /// A program that ends before its time is waited for, with its own exit code.
    #[test]
    fn a_program_that_ends_in_time_keeps_its_code() {
        let mut child = ci::cargo()
            .arg("--version")
            .stdout(Stdio::null())
            .spawn()
            .expect("cargo starts");
        let status = watch(&mut child, Duration::from_secs(600), "600").expect("cargo ends");
        assert!(status.success());
    }
}
