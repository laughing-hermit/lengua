//! The solver mode's measurement: the WebAssembly z3 against native z3, on each runner, by the
//! owner's bar (todo.md's decisions): the same verdicts at today's limits, and the replayed
//! questions taking at most twice as long in all.
//!
//! - `cargo xtask solver-golden native|wasm <module> <folder>`: one of a runner's two legs. The
//!   prover's golden (`every_run_and_proof_is_as_recorded`, the one golden that asks the solvers;
//!   the checker's asks none) recorded with native z3, or with the WebAssembly z3 `<module>`
//!   hosted by this commit's lengua-mcp, which the leg builds and has compile the module first,
//!   as `lengua-mcp doctor` does, so that no proof waits for that. Every solver keeps what it is
//!   sent in `<folder>/sessions` (`LENGUA_SMT_LOG`, src/prove/smt.rs). The folder gets the
//!   recording (prover-wasm.txt), golden.log, and leg.tsv: how long the golden took, and what its
//!   sessions show of the solvers that ran. The leg fails when its sessions show it measured
//!   other than it says: a z3 that couldn't start, or one through the wrong program.
//! - `cargo xtask solver-parity <target> <sessions> <module> <folder>`: each z3 session of the
//!   native leg asked again, one process a session with the prover's arguments, of native z3 (the
//!   one on PATH) and of the WebAssembly z3 hosted by lengua-mcp, in turn (native z3 first in one
//!   session, the WebAssembly z3 first in the next). Their answers are compared block by block, a
//!   block being what one batch of the prover's commands printed (it ends each with an `echo`):
//!   the `sat`, `unsat` and `unknown` lines, the values asked for, the reasons and the errors. Each
//!   replay is timed from its start to its end, the WebAssembly z3's taking its compiled module
//!   from a cache the task makes first, which it times as the module's one-time compile. The
//!   folder gets replay.tsv (each session's questions, differences and two times), parity.tsv
//!   (the totals) and parity.md (the report, also in the job's summary), and in kept/ a few of the
//!   sessions whose answers differ or whose WebAssembly replay didn't end as native z3's did.
//!   `PARITY_SECONDS`, when it gives a number, stops starting sessions after that long, so that
//!   the job's time is never passed; the report then says how many were replayed.
//! - `cargo xtask solver-report <folder>`: every runner's two legs, downloaded into `<folder>`,
//!   held to the bar: the two recordings' ledgers compared as `cargo xtask golden-diff ledger`
//!   compares them, where a block that ran out of time or memory on one side (LIMIT) is set aside
//!   and nothing else is, and the replay's totals. solver-bar.md says, runner by runner, whether
//!   the bar is met, with the numbers; it fails only when a runner the dispatch named (`TARGETS`)
//!   left no leg to read.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::{ci, fetch, files, golden, plan, test};

/// How the leg's task is called.
const GOLDEN_USAGE: &str = "usage: cargo xtask solver-golden native|wasm <module> <folder>";

/// How the replay is called.
const PARITY_USAGE: &str = "usage: cargo xtask solver-parity <target> <sessions> <module> <folder>";

/// How the report is called.
const REPORT_USAGE: &str = "usage: cargo xtask solver-report <folder>";

/// The arguments the prover starts z3 with (src/prove/smt.rs's `command`), which every replay
/// gives it: each session's first line must end with them.
pub const Z3_ARGUMENTS: [&str; 3] = ["-in", "-smt2", "-memory:4096"];

/// The line the prover has each solver print after each batch of commands (src/prove/smt.rs's
/// `MARKER`), which ends each block of what it printed.
pub const MARKER: &str = "@@lengua@@";

/// The flag that starts lengua-mcp as a WebAssembly solver's host (src/wasm/solver.rs's `FLAG`).
pub const HOST_FLAG: &str = "--solver-wasm";

/// The variable that names the folder each solver keeps its sessions in (src/prove/smt.rs's
/// `SMT_LOG`).
pub const SMT_LOG: &str = "LENGUA_SMT_LOG";

/// The variable that names the WebAssembly z3 the prover starts (src/prove/smt.rs's `Z3_WASM`).
pub const Z3_WASM: &str = "LENGUA_Z3_WASM";

/// The variable that names the lengua-mcp that hosts a WebAssembly solver (src/prove/smt.rs's
/// `SOLVER_HOST`), which a test binary needs, being no lengua-mcp itself.
pub const SOLVER_HOST: &str = "LENGUA_SOLVER_HOST";

/// The most the WebAssembly z3's replays may take in all, as a multiple of native z3's: the
/// owner's bar.
pub const BAR: f64 = 2.0;

/// The recording a leg keeps: the prover's.
const RECORDING: &str = "prover-wasm.txt";

/// A leg's summary, in its folder.
const LEG: &str = "leg.tsv";

/// The replay's rows, one a session, in its folder.
const REPLAY: &str = "replay.tsv";

/// The replay's totals, in its folder.
const TOTALS: &str = "parity.tsv";

/// The replay's report, in its folder.
const PARITY: &str = "parity.md";

/// The report on every runner, in the report's folder.
const BAR_REPORT: &str = "solver-bar.md";

/// The most sessions the replay keeps for whoever looks into a difference.
const KEPT: usize = 5;

/// The most lines of a replay's error output the report quotes.
const QUOTED: usize = 12;

/// How long a replay may take beyond four times its questions' limits before it is stopped: a
/// session's definitions take time to read too, and a replay that never ends would take the
/// whole job's time with it.
const SLACK: Duration = Duration::from_secs(900);

/// The answers a solver gives a question, in the order sessions count them.
const ANSWERS: [&str; 3] = ["sat", "unsat", "unknown"];

/// What a session's file says of itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    /// Its solver, from its file's name: z3 or cvc5.
    pub solver: String,
    /// The command that started it, from its first line.
    pub command: String,
    /// Its questions: its check-sat commands.
    pub questions: usize,
    /// The time its z3 questions were given, in all (`try-for`'s limits), in milliseconds.
    pub limits_ms: u64,
    /// Its answers as its comments record them: sat, unsat and unknown, in [`ANSWERS`]' order.
    pub answers: [usize; 3],
    /// How long its answers took, in all, as its comments record them.
    pub answered: Duration,
    /// The solvers its comments say couldn't start.
    pub unstarted: Vec<String>,
}

/// A duration as Rust's `Debug` writes it, which the sessions' comments use: `1.5s`, `20.1ms`,
/// `512µs`, `30ns`; read digit by digit, to the nanosecond.
pub fn duration(text: &str) -> Option<Duration> {
    let units: [(&str, u64); 5] = [
        ("ns", 1),
        ("µs", 1_000),
        ("us", 1_000),
        ("ms", 1_000_000),
        ("s", 1_000_000_000),
    ];
    let (number, unit) = units
        .iter()
        .find_map(|(name, unit)| Some((text.strip_suffix(name)?, *unit)))?;
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut nanoseconds = whole.parse::<u64>().ok()?.checked_mul(unit)?;
    let mut place = unit;
    for digit in fraction.bytes() {
        place /= 10;
        nanoseconds += u64::from(digit - b'0') * place;
    }
    Some(Duration::from_nanos(nanoseconds))
}

/// The session `text`, the file named `name` that a solver kept with `LENGUA_SMT_LOG`.
pub fn session(name: &str, text: &str) -> Session {
    let stem = name.strip_suffix(".smt2").unwrap_or(name);
    let solver = stem.rsplit('-').next().unwrap_or_default().to_owned();
    let mut found = Session {
        solver: solver.clone(),
        ..Session::default()
    };
    let answered = format!("; {solver}: ");
    for (number, line) in text.lines().enumerate() {
        if number == 0 {
            if let Some((_, command)) = line.split_once(": ") {
                command.clone_into(&mut found.command);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("(check-sat-using ") {
            found.questions += 1;
            let limit = rest
                .trim_end_matches(')')
                .split_whitespace()
                .last()
                .and_then(|word| word.parse::<u64>().ok());
            found.limits_ms += limit.unwrap_or(0);
        } else if line == "(check-sat)" {
            found.questions += 1;
        } else if let Some(rest) = line.strip_prefix(&answered) {
            let Some((answer, took)) = rest.split_once(" in ") else {
                continue;
            };
            let at = ["Sat", "Unsat", "Unknown"]
                .iter()
                .position(|named| *named == answer);
            if let Some(at) = at {
                found.answers[at] += 1;
            }
            found.answered += duration(took).unwrap_or_default();
        } else if let Some(why) = line.strip_prefix("; ")
            && (why.contains(" couldn't start") || why.ends_with(" isn't installed"))
        {
            found.unstarted.push(why.to_owned());
        }
    }
    found
}

/// The session files in `folder`, by name, of `solver` alone when one is named: each a
/// `<process>-<session>-<solver>.smt2`, in the order their processes and sessions began.
fn session_files(folder: &Path, solver: Option<&str>) -> Vec<PathBuf> {
    let mut found: Vec<((u64, u64), PathBuf)> = files::files(folder)
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let stem = name.strip_suffix(".smt2")?;
            let mut parts = stem.splitn(3, '-');
            let process: u64 = parts.next()?.parse().ok()?;
            let number: u64 = parts.next()?.parse().ok()?;
            let kind = parts.next()?;
            solver
                .is_none_or(|solver| solver == kind)
                .then_some(((process, number), path))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

/// A file's name, as text.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// A file's text, or why it can't be read.
fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Writes `text` to `path`.
fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

/// `path` made absolute, from the current folder.
fn absolute(path: &str) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| format!("{path}: {error}"))
}

/// The program `name` in PATH's folders, as the prover finds a solver there: the first that is a
/// file.
fn on_path(name: &str) -> Option<PathBuf> {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|folder| folder.is_absolute())
        .map(|folder| folder.join(&file))
        .find(|candidate| candidate.is_file())
}

/// The first line `command` prints, which must start it and succeed.
fn first_line(command: &mut Command) -> Result<String, String> {
    let shown = ci::shown(command);
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{shown} didn't start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{shown} failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let printed = String::from_utf8_lossy(&output.stdout);
    Ok(printed.lines().next().unwrap_or_default().trim().to_owned())
}

/// The lengua-mcp that hosts the WebAssembly z3: the one [`SOLVER_HOST`] names, or else this
/// commit's, which it builds.
fn host(root: &Path) -> Result<PathBuf, String> {
    let named = ci::setting(SOLVER_HOST);
    if !named.is_empty() {
        return Ok(PathBuf::from(named));
    }
    let mut build = ci::cargo();
    build
        .current_dir(root)
        .args(["build", "--release", "--locked", "--bin", "lengua-mcp"]);
    ci::must(&mut build)?;
    let file = format!("lengua-mcp{}", std::env::consts::EXE_SUFFIX);
    Ok(root.join("target").join("release").join(file))
}

/// What a leg's sessions show of the solvers that ran, solver by solver.
#[derive(Debug, Clone, Default, PartialEq)]
struct Solvers {
    /// The sessions, by solver.
    sessions: BTreeMap<String, usize>,
    /// The sessions through a WebAssembly solver's host, by solver.
    hosted: BTreeMap<String, usize>,
    /// The questions, by solver.
    questions: BTreeMap<String, usize>,
    /// The answers, by solver: sat, unsat and unknown.
    answers: BTreeMap<String, [usize; 3]>,
    /// How long the answers took in all, by solver, as the sessions record it.
    answered: BTreeMap<String, Duration>,
    /// What the sessions say of solvers that couldn't start.
    unstarted: Vec<String>,
    /// The sessions' size, in bytes.
    bytes: u64,
}

/// What the sessions in `folder` show.
fn solvers(folder: &Path) -> Result<Solvers, String> {
    let mut found = Solvers::default();
    for path in session_files(folder, None) {
        let text = read(&path)?;
        found.bytes += u64::try_from(text.len()).unwrap_or(u64::MAX);
        let session = session(&name_of(&path), &text);
        let solver = session.solver.clone();
        *found.sessions.entry(solver.clone()).or_default() += 1;
        if session.command.contains(HOST_FLAG) {
            *found.hosted.entry(solver.clone()).or_default() += 1;
        }
        *found.questions.entry(solver.clone()).or_default() += session.questions;
        let answers = found.answers.entry(solver.clone()).or_default();
        for (all, these) in answers.iter_mut().zip(session.answers) {
            *all += these;
        }
        *found.answered.entry(solver).or_default() += session.answered;
        found.unstarted.extend(session.unstarted);
    }
    Ok(found)
}

/// `count` with a thousands separator: `4,567`.
fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut grouped = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// `cargo xtask solver-golden native|wasm <module> <folder>`.
pub fn golden(arguments: &[String]) -> Result<u8, String> {
    let [leg, module, folder] = arguments else {
        return Err(GOLDEN_USAGE.to_owned());
    };
    let wasm = match leg.as_str() {
        "native" => false,
        "wasm" => true,
        _ => return Err(GOLDEN_USAGE.to_owned()),
    };
    let root = ci::root()?;
    let module = absolute(module)?;
    let folder = absolute(folder)?;
    if !module.is_file() {
        return Err(format!("no module at {}", module.display()));
    }
    files::remove(&folder)?;
    let sessions = folder.join("sessions");
    files::folder(&sessions)?;
    let mut settings: Vec<(&str, OsString)> = vec![
        (SMT_LOG, sessions.clone().into_os_string()),
        ("RUST_TEST_THREADS", OsString::from("1")),
    ];
    // The tests built first, so that the golden's time is its own.
    let mut built = ci::cargo();
    built
        .current_dir(&root)
        .args(["test", "--release", "--locked", "--lib", "--no-run"]);
    ci::must(&mut built)?;
    let (z3, made) = if wasm {
        let host = host(&root)?;
        // Compiled into the cache the host reads by itself, as doctor does, and its version read
        // through the host, as the prover starts it.
        let started = Instant::now();
        let version = first_line(
            Command::new(&host)
                .arg(HOST_FLAG)
                .arg(&module)
                .arg("--")
                .arg("--version"),
        )?;
        let made = started.elapsed();
        settings.push((Z3_WASM, module.clone().into_os_string()));
        settings.push((SOLVER_HOST, host.into_os_string()));
        (version, Some(made))
    } else {
        let native = on_path("z3").ok_or("no z3 on PATH: run cargo xtask fetch first")?;
        (first_line(Command::new(&native).arg("--version"))?, None)
    };
    println!("{leg} leg: {z3}");
    let started = Instant::now();
    let set: Vec<(&str, &std::ffi::OsStr)> = settings
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect();
    let code = test::record(
        &[test::PROVER_GOLDEN, "--nocapture"],
        &set,
        &folder.join("golden.log"),
    )?;
    let seconds = started.elapsed().as_secs_f64();
    let recording = root.join("target").join("golden").join(RECORDING);
    if recording.is_file() {
        files::copy(&recording, &folder.join(RECORDING))?;
    }
    let found = solvers(&sessions)?;
    let mut leg_file = format!("leg\t{leg}\ngolden_seconds\t{seconds:.1}\nz3_version\t{z3}\n");
    if let Some(made) = made {
        let _ = writeln!(leg_file, "z3_ready_seconds\t{:.1}", made.as_secs_f64());
    }
    for solver in ["z3", "cvc5"] {
        let count = |map: &BTreeMap<String, usize>| map.get(solver).copied().unwrap_or(0);
        let answers = found.answers.get(solver).copied().unwrap_or_default();
        let answered = found.answered.get(solver).copied().unwrap_or_default();
        let _ = writeln!(
            leg_file,
            "{solver}_sessions\t{}\n{solver}_hosted\t{}\n{solver}_questions\t{}\n\
             {solver}_sat\t{}\n{solver}_unsat\t{}\n{solver}_unknown\t{}\n\
             {solver}_answered_seconds\t{:.1}",
            count(&found.sessions),
            count(&found.hosted),
            count(&found.questions),
            answers[0],
            answers[1],
            answers[2],
            answered.as_secs_f64()
        );
    }
    let _ = writeln!(
        leg_file,
        "unstarted\t{}\nsession_bytes\t{}",
        found.unstarted.len(),
        found.bytes
    );
    write(&folder.join(LEG), &leg_file)?;
    let z3_sessions = found.sessions.get("z3").copied().unwrap_or(0);
    let hosted = found.hosted.get("z3").copied().unwrap_or(0);
    let z3_questions = found.questions.get("z3").copied().unwrap_or(0);
    let mut summary = format!(
        "### The prover's golden with {} z3\n\n{z3}. The golden took {seconds:.0} s. Its \
         sessions: {} of z3 ({} through lengua-mcp's host), with {} questions, which z3 \
         answered in {:.1} s in all; {} of cvc5, with {} questions; {} bytes.\n",
        if wasm { "the WebAssembly" } else { "native" },
        grouped(z3_sessions),
        grouped(hosted),
        grouped(z3_questions),
        found
            .answered
            .get("z3")
            .copied()
            .unwrap_or_default()
            .as_secs_f64(),
        grouped(found.sessions.get("cvc5").copied().unwrap_or(0)),
        grouped(found.questions.get("cvc5").copied().unwrap_or(0)),
        grouped(usize::try_from(found.bytes).unwrap_or(usize::MAX))
    );
    if let Some(made) = made {
        let _ = writeln!(
            summary,
            "\nThe module was compiled for this runner, and its version read through the host, \
             in {:.1} s.",
            made.as_secs_f64()
        );
    }
    let mut problems: Vec<String> = found.unstarted.clone();
    problems.sort();
    problems.dedup();
    if wasm && hosted != z3_sessions {
        problems.push(format!(
            "{} of the {z3_sessions} z3 sessions didn't run through lengua-mcp's host",
            z3_sessions - hosted
        ));
    }
    if !wasm && hosted > 0 {
        problems.push(format!(
            "{hosted} z3 sessions ran through lengua-mcp's host"
        ));
    }
    if z3_questions == 0 {
        problems.push("z3 was asked no question".to_owned());
    }
    if !problems.is_empty() {
        let _ = writeln!(summary, "\nThe leg doesn't measure what it says it does:\n");
        for problem in &problems {
            let _ = writeln!(summary, "- {problem}");
        }
    }
    ci::print_and_summarize(&summary)?;
    if code != 0 {
        return Ok(code);
    }
    if !recording.is_file() {
        return Err(format!("the golden made no {}", recording.display()));
    }
    Ok(u8::from(!problems.is_empty()))
}

/// How one replay of a session went.
#[derive(Debug, Clone, Default)]
struct Replay {
    /// What the solver printed.
    printed: String,
    /// What it said on its error output.
    errors: String,
    /// Its exit code, or none when it had none, as when it was stopped.
    code: Option<i32>,
    /// How long it took, from its start to its end, in seconds.
    seconds: f64,
    /// Whether it ran past its time and was stopped.
    stopped: bool,
}

impl Replay {
    /// Whether it ended as a solver that read all of its input ends: by itself, with 0.
    fn ended_well(&self) -> bool {
        self.code == Some(0) && !self.stopped
    }

    /// How it ended, in words.
    fn ending(&self) -> String {
        if self.stopped {
            format!("stopped after {:.0} s", self.seconds)
        } else {
            self.code.map_or_else(
                || "ended with no exit code".to_owned(),
                |code| format!("exit {code}"),
            )
        }
    }
}

/// Runs `command` with `input` as its standard input, keeping what it prints and says on its
/// errors, and stops it once it has run for `limit`.
fn replay(mut command: Command, input: &[u8], limit: Duration) -> Result<Replay, String> {
    let shown = ci::shown(&command);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("{shown} didn't start: {error}"))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Mutex::new(child);
    let (done, finished) = mpsc::channel::<()>();
    let (printed, errors, stopped) = std::thread::scope(|scope| {
        let running = &child;
        let guard = scope.spawn(move || {
            let late = matches!(
                finished.recv_timeout(limit),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            if late && let Ok(mut child) = running.lock() {
                let _ = child.kill();
            }
            late
        });
        scope.spawn(move || {
            if let Some(mut stdin) = stdin {
                // A solver that stops early closes its input: what is left goes nowhere.
                let _ = stdin.write_all(input);
            }
        });
        let errors = scope.spawn(move || {
            let mut said = Vec::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_end(&mut said);
            }
            said
        });
        let mut printed = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut printed);
        }
        let errors = errors.join().unwrap_or_default();
        let _ = done.send(());
        (printed, errors, guard.join().unwrap_or(false))
    });
    let status = child
        .into_inner()
        .map_err(|_| format!("{shown}: its guard failed"))?
        .wait()
        .map_err(|error| format!("waiting for {shown}: {error}"))?;
    Ok(Replay {
        printed: String::from_utf8_lossy(&printed).into_owned(),
        errors: String::from_utf8_lossy(&errors).into_owned(),
        code: status.code(),
        seconds: started.elapsed().as_secs_f64(),
        stopped,
    })
}

/// What a block of a solver's output holds, as the comparison sorts what differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Said {
    /// An answer to a question: `sat`, `unsat` or `unknown`.
    Answer,
    /// The values `get-value` asked for.
    Values,
    /// Why an answer was unknown, as `get-info` asked.
    Reason,
    /// An error, which the prover takes as a question its solver refused.
    Error,
    /// Nothing: one side printed nothing where the other printed something.
    Nothing,
    /// A block one replay has and the other doesn't: it ended sooner.
    Missing,
}

impl Said {
    /// Its name in the reports.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Answer => "answers",
            Self::Values => "values",
            Self::Reason => "reasons",
            Self::Error => "errors",
            Self::Nothing => "nothing printed",
            Self::Missing => "missing",
        }
    }
}

/// The blocks of `printed`, a solver's output: the lines before each of the prover's markers, and
/// after the last, each line trimmed.
pub fn blocks(printed: &str) -> Vec<Vec<&str>> {
    let mut blocks = vec![Vec::new()];
    for line in printed.lines() {
        let line = line.trim();
        if line.trim_matches('"') == MARKER {
            blocks.push(Vec::new());
        } else if let Some(block) = blocks.last_mut() {
            block.push(line);
        }
    }
    blocks
}

/// What `block` holds.
pub fn kind_of(block: &[&str]) -> Said {
    if block.iter().any(|line| ANSWERS.contains(line)) {
        Said::Answer
    } else if block
        .iter()
        .any(|line| line.starts_with("(:reason-unknown"))
    {
        Said::Reason
    } else if block.iter().any(|line| line.starts_with("(error")) {
        Said::Error
    } else if block.is_empty() {
        Said::Nothing
    } else {
        Said::Values
    }
}

/// What differs between two replays of one session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Differences {
    /// The answers native z3 gave.
    pub answers: usize,
    /// The blocks that differ, by what they hold.
    pub by_kind: BTreeMap<Said, usize>,
    /// The answers that changed, as `native → WebAssembly`, and how many times each.
    pub turns: BTreeMap<String, usize>,
    /// The first difference of each kind: its block's number, and what each side printed there.
    pub examples: BTreeMap<Said, (usize, String, String)>,
}

impl Differences {
    /// How many blocks differ.
    pub fn count(&self) -> usize {
        self.by_kind.values().sum()
    }
}

/// The answer `block` gives, or that it gives none.
fn answer_in<'a>(block: Option<&Vec<&'a str>>) -> &'a str {
    block
        .and_then(|lines| lines.iter().find(|line| ANSWERS.contains(*line)))
        .map_or("no answer", |line| *line)
}

/// What differs between `native` and `wasm`, two outputs of the same session, block by block.
pub fn differences(native: &str, wasm: &str) -> Differences {
    let (native, wasm) = (blocks(native), blocks(wasm));
    let mut found = Differences::default();
    for at in 0..native.len().max(wasm.len()) {
        let (was, is) = (native.get(at), wasm.get(at));
        if was.is_some_and(|block| kind_of(block) == Said::Answer) {
            found.answers += 1;
        }
        if was == is {
            continue;
        }
        let kind = match (was, is) {
            (Some(was), Some(is)) => match kind_of(was) {
                Said::Nothing => kind_of(is),
                kind => kind,
            },
            _ => Said::Missing,
        };
        let joined = |block: Option<&Vec<&str>>| {
            block.map_or_else(
                || "(nothing: it had ended)".to_owned(),
                |lines| lines.join(" "),
            )
        };
        if kind == Said::Answer {
            let turn = format!("{} → {}", answer_in(was), answer_in(is));
            *found.turns.entry(turn).or_default() += 1;
        }
        *found.by_kind.entry(kind).or_default() += 1;
        found
            .examples
            .entry(kind)
            .or_insert_with(|| (at, joined(was), joined(is)));
    }
    found
}

/// One session's replays.
#[derive(Debug, Clone)]
struct Row {
    /// The session's file.
    name: String,
    /// What it says of itself.
    session: Session,
    /// Its replay through native z3.
    native: Replay,
    /// Its replay through the WebAssembly z3.
    wasm: Replay,
    /// What differs between them.
    differences: Differences,
}

/// The first `QUOTED` lines of `text`, a replay's errors.
fn quoted(text: &str) -> String {
    let lines: Vec<&str> = text.lines().take(QUOTED).collect();
    lines.join("\n")
}

/// The replay's report on `rows`, the sessions of `all` it replayed, for `target`, with what it
/// read of the two z3s in `about`: the report, and its totals as parity.tsv keeps them.
fn parity_report(target: &str, about: &str, rows: &[Row], all: usize) -> (String, String) {
    let native: f64 = rows.iter().map(|row| row.native.seconds).sum();
    let wasm: f64 = rows.iter().map(|row| row.wasm.seconds).sum();
    let ratio = if native > 0.0 { wasm / native } else { 0.0 };
    let questions: usize = rows.iter().map(|row| row.session.questions).sum();
    let answers: usize = rows.iter().map(|row| row.differences.answers).sum();
    let mut by_kind: BTreeMap<Said, usize> = BTreeMap::new();
    let mut turns: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: BTreeMap<Said, (String, usize, String, String)> = BTreeMap::new();
    for row in rows {
        for (kind, count) in &row.differences.by_kind {
            *by_kind.entry(*kind).or_default() += count;
        }
        for (turn, count) in &row.differences.turns {
            *turns.entry(turn.clone()).or_default() += count;
        }
        for (kind, (at, was, is)) in &row.differences.examples {
            examples
                .entry(*kind)
                .or_insert_with(|| (row.name.clone(), *at, was.clone(), is.clone()));
        }
    }
    let differing: usize = by_kind.values().sum();
    let failed: Vec<&Row> = rows
        .iter()
        .filter(|row| !row.native.ended_well() || !row.wasm.ended_well())
        .collect();
    let mut report = format!("## z3 replayed on {target}\n\n{about}");
    let _ = writeln!(
        report,
        "- {} of the native leg's {} z3 sessions replayed{}, with {} questions; native z3 \
         answered {} of them.\n",
        grouped(rows.len()),
        grouped(all),
        if rows.len() < all {
            ", the rest left as PARITY_SECONDS asks"
        } else {
            ""
        },
        grouped(questions),
        grouped(answers)
    );
    let _ = writeln!(
        report,
        "| | native z3 | WebAssembly z3 |\n|---|---|---|\n| replayed, in all | {native:.1} s \
         | {wasm:.1} s |\n| WebAssembly against native | | {ratio:.2} times |\n"
    );
    if differing == 0 {
        let _ = writeln!(report, "Every answer, value and reason is the same.\n");
    } else {
        let _ = writeln!(
            report,
            "{} blocks of what the two printed differ:\n",
            grouped(differing)
        );
        for (kind, count) in &by_kind {
            let _ = write!(report, "- {}: {}", kind.name(), grouped(*count));
            if *kind == Said::Answer {
                let listed: Vec<String> = turns
                    .iter()
                    .map(|(turn, count)| format!("{turn}: {count}"))
                    .collect();
                let _ = write!(report, " (native → WebAssembly: {})", listed.join(", "));
            }
            if let Some((session, at, was, is)) = examples.get(kind) {
                let _ = write!(
                    report,
                    "; first in {session}, block {at}: native `{was}`, WebAssembly `{is}`"
                );
            }
            report.push('\n');
        }
        report.push('\n');
    }
    if failed.is_empty() {
        let _ = writeln!(report, "Every replay ended by itself, with exit 0.\n");
    } else {
        let _ = writeln!(
            report,
            "{} sessions' replays didn't end by themselves with exit 0:\n",
            failed.len()
        );
        for row in &failed {
            let _ = writeln!(
                report,
                "- {}: native z3 {}, WebAssembly z3 {}.",
                row.name,
                row.native.ending(),
                row.wasm.ending()
            );
            for (who, replay) in [("native", &row.native), ("WebAssembly", &row.wasm)] {
                if !replay.ended_well() && !replay.errors.trim().is_empty() {
                    let _ = writeln!(
                        report,
                        "\n  {who} z3's errors:\n\n  ```\n{}\n  ```\n",
                        quoted(&replay.errors)
                    );
                }
            }
        }
        report.push('\n');
    }
    let mut slowest: Vec<&Row> = rows.iter().collect();
    slowest.sort_by(|a, b| b.wasm.seconds.total_cmp(&a.wasm.seconds));
    let _ = writeln!(
        report,
        "The sessions that took the WebAssembly z3 longest:\n\n| session | questions | native \
         | WebAssembly | times |\n|---|---|---|---|---|"
    );
    for row in slowest.iter().take(10) {
        let times = if row.native.seconds > 0.0 {
            row.wasm.seconds / row.native.seconds
        } else {
            0.0
        };
        let _ = writeln!(
            report,
            "| {} | {} | {:.2} s | {:.2} s | {times:.2} |",
            row.name,
            grouped(row.session.questions),
            row.native.seconds,
            row.wasm.seconds
        );
    }
    let mut totals = format!(
        "target\t{target}\nsessions\t{all}\nreplayed\t{}\nquestions\t{questions}\nanswers\t\
         {answers}\nnative_seconds\t{native:.3}\nwasm_seconds\t{wasm:.3}\nratio\t{ratio:.4}\n\
         differing\t{differing}\nfailed\t{}\n",
        rows.len(),
        failed.len()
    );
    for (kind, count) in &by_kind {
        let _ = writeln!(
            totals,
            "differing_{}\t{count}",
            kind.name().replace(' ', "_")
        );
    }
    for (turn, count) in &turns {
        let _ = writeln!(totals, "turn\t{turn}\t{count}");
    }
    for (kind, (session, at, was, is)) in &examples {
        let _ = writeln!(
            totals,
            "example\t{}\t{session}\t{at}\t{was}\t{is}",
            kind.name()
        );
    }
    (report, totals)
}

/// The WebAssembly z3 `module` as its host `host` runs it, with `arguments`, its compiled module
/// kept in `cache`.
fn hosted(host: &Path, module: &Path, cache: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(host);
    command
        .arg(HOST_FLAG)
        .arg(module)
        .arg("--cache")
        .arg(cache)
        .arg("--")
        .args(arguments);
    command
}

/// Native z3, `program`, with `arguments`.
fn native(program: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(program);
    command.args(arguments);
    command
}

/// `cargo xtask solver-parity <target> <sessions> <module> <folder>`.
pub fn parity(arguments: &[String]) -> Result<u8, String> {
    let [target, sessions, module, folder] = arguments else {
        return Err(PARITY_USAGE.to_owned());
    };
    let root = ci::root()?;
    let (sessions, module, folder) = (absolute(sessions)?, absolute(module)?, absolute(folder)?);
    files::folder(&folder)?;
    let budget = ci::setting("PARITY_SECONDS")
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs);
    let z3 = on_path("z3").ok_or("no z3 on PATH: run cargo xtask fetch first")?;
    let host = host(&root)?;
    let wasm_bytes =
        std::fs::read(&module).map_err(|error| format!("{}: {error}", module.display()))?;
    // A cache of the task's own, empty, so that the first run compiles the module: the
    // one-time compile, which every later replay takes from the cache.
    let cache = root.join("target").join("solver-parity").join("cache");
    files::remove(&cache)?;
    files::folder(&cache)?;
    let started = Instant::now();
    let wasm_version = first_line(&mut hosted(&host, &module, &cache, &["--version"]))?;
    let compile = started.elapsed().as_secs_f64();
    let native_version = first_line(&mut native(&z3, &["--version"]))?;
    let about = format!(
        "- Native z3: `{}`, {native_version}.\n- WebAssembly z3: `{}`, {} bytes, SHA-256 {}, \
         hosted by this commit's lengua-mcp (`{HOST_FLAG}`), {wasm_version}; compiled for this \
         runner once, in {compile:.1} s.\n",
        z3.display(),
        name_of(&module),
        grouped(wasm_bytes.len()),
        fetch::sha256(&wasm_bytes)
    );
    print!("{about}");
    let listed = session_files(&sessions, Some("z3"));
    if listed.is_empty() {
        return Err(format!("no z3 session in {}", sessions.display()));
    }
    let arguments = Z3_ARGUMENTS.join(" ");
    let kept = folder.join("kept");
    let mut keeping = 0;
    let began = Instant::now();
    let mut rows = Vec::new();
    for (at, path) in listed.iter().enumerate() {
        if budget.is_some_and(|budget| began.elapsed() >= budget) {
            println!(
                "PARITY_SECONDS reached: {} of {} sessions replayed",
                rows.len(),
                listed.len()
            );
            break;
        }
        let name = name_of(path);
        let text = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let session = session(&name, &String::from_utf8_lossy(&text));
        if !session.command.ends_with(&arguments) {
            return Err(format!(
                "{name} wasn't started with z3's arguments ({arguments}) but as {:?}: the \
                 replay gives every session the prover's",
                session.command
            ));
        }
        let limit = Duration::from_millis(session.limits_ms.saturating_mul(4)) + SLACK;
        let by_native = || replay(native(&z3, &Z3_ARGUMENTS), &text, limit);
        let by_wasm = || replay(hosted(&host, &module, &cache, &Z3_ARGUMENTS), &text, limit);
        // In turn, each going first in every other session.
        let (first, second) = if at % 2 == 0 {
            let first = by_native()?;
            (first, by_wasm()?)
        } else {
            let wasm = by_wasm()?;
            (by_native()?, wasm)
        };
        let differ = differences(&first.printed, &second.printed);
        let row = Row {
            name,
            session,
            native: first,
            wasm: second,
            differences: differ,
        };
        println!(
            "{}/{} {}: {} questions, native {:.2} s, WebAssembly {:.2} s, {} blocks differ{}",
            at + 1,
            listed.len(),
            row.name,
            row.session.questions,
            row.native.seconds,
            row.wasm.seconds,
            row.differences.count(),
            if row.wasm.ended_well() {
                String::new()
            } else {
                format!(", WebAssembly z3 {}", row.wasm.ending())
            }
        );
        let notable =
            row.differences.count() > 0 || !row.wasm.ended_well() || !row.native.ended_well();
        if notable && keeping < KEPT {
            files::folder(&kept)?;
            files::copy(path, &kept.join(&row.name))?;
            keeping += 1;
        }
        rows.push(row);
    }
    let mut table = String::from(
        "session\tquestions\tanswers\tdiffering\tnative_seconds\twasm_seconds\tnative_ending\t\
         wasm_ending\n",
    );
    for row in &rows {
        let _ = writeln!(
            table,
            "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{}\t{}",
            row.name,
            row.session.questions,
            row.differences.answers,
            row.differences.count(),
            row.native.seconds,
            row.wasm.seconds,
            row.native.ending(),
            row.wasm.ending()
        );
    }
    write(&folder.join(REPLAY), &table)?;
    let (report, mut totals) = parity_report(target, &about, &rows, listed.len());
    let _ = writeln!(totals, "compile_seconds\t{compile:.1}");
    let _ = writeln!(totals, "native_version\t{native_version}");
    let _ = writeln!(totals, "wasm_version\t{wasm_version}");
    write(&folder.join(TOTALS), &totals)?;
    write(&folder.join(PARITY), &report)?;
    ci::print_and_summarize(&report)?;
    Ok(0)
}

/// A file of `key<TAB>value` lines, as a leg and the replay keep their totals: each key's first
/// value.
fn keyed(text: &str) -> BTreeMap<String, String> {
    let mut keyed = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('\t') {
            keyed
                .entry(key.to_owned())
                .or_insert_with(|| value.to_owned());
        }
    }
    keyed
}

/// The folder beneath `folder` that holds the leg `leg` of `target`, as the report's download
/// names it: `parity-<leg>-<target>-<run>`.
fn leg_folder(folder: &Path, leg: &str, target: &str) -> Option<PathBuf> {
    let prefix = format!("parity-{leg}-{target}-");
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && name_of(path).starts_with(&prefix))
        .collect();
    found.sort();
    found.pop()
}

/// What the report reads of one runner's legs, and the bar's verdict on them.
#[derive(Debug, Clone, Default)]
struct Measured {
    /// The two recordings' ledgers compared.
    ledger: golden::Compared,
    /// The replay's totals.
    totals: BTreeMap<String, String>,
    /// The native leg's summary.
    native: BTreeMap<String, String>,
    /// The WebAssembly leg's summary.
    wasm: BTreeMap<String, String>,
}

impl Measured {
    /// A replay total, as a number.
    fn number(&self, key: &str) -> f64 {
        self.totals
            .get(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0)
    }

    /// Whether every session was replayed by both z3s.
    fn complete(&self) -> bool {
        self.totals.get("replayed") == self.totals.get("sessions")
    }

    /// Whether the bar is met: every verdict the same, the LIMIT lines aside, and the
    /// WebAssembly z3's replay total at most [`BAR`] times native's, over every session.
    fn met(&self) -> bool {
        let ratio = self.number("ratio");
        self.ledger.differ == 0
            && self.ledger.missing == 0
            && self.complete()
            && ratio > 0.0
            && ratio <= BAR
    }
}

/// One runner's legs in `folder`, or which of them is missing.
fn measured(folder: &Path, target: &str) -> Result<Measured, String> {
    let native = leg_folder(folder, "native", target);
    let wasm = leg_folder(folder, "wasm", target);
    let (Some(native), Some(wasm)) = (native, wasm) else {
        return Err("a leg left nothing to read".to_owned());
    };
    let (was, is) = (read(&native.join(RECORDING))?, read(&wasm.join(RECORDING))?);
    let ledger = golden::compare(
        RECORDING,
        &golden::keyed_ledger(&was)?,
        &golden::keyed_ledger(&is)?,
    );
    Ok(Measured {
        ledger,
        totals: keyed(&read(&native.join(TOTALS))?),
        native: keyed(&read(&native.join(LEG))?),
        wasm: keyed(&read(&wasm.join(LEG))?),
    })
}

/// `cargo xtask solver-report <folder>`.
pub fn report(arguments: &[String]) -> Result<u8, String> {
    let [folder] = arguments else {
        return Err(REPORT_USAGE.to_owned());
    };
    let folder = absolute(folder)?;
    let asked = ci::setting("TARGETS").replace(' ', "");
    let everything = asked.is_empty() || asked == "all";
    let targets: Vec<String> = plan::runners(include_str!("../targets.json"))?
        .into_iter()
        .map(|runner| runner.target)
        .filter(|target| everything || format!(",{asked},").contains(&format!(",{target},")))
        .collect();
    let mut table = format!(
        "## WebAssembly z3 against native z3, by the owner's bar\n\nThe bar, on each runner: \
         every verdict of the prover's golden the same with either z3, a block that ran out of \
         time or memory on one side (LIMIT) aside and nothing else, and the WebAssembly z3's \
         replay of the native leg's sessions taking at most {BAR} times as long as native z3's \
         in all.\n\n| runner | sessions | answers differing | native | WebAssembly | ratio | \
         verdicts (same, differ, LIMIT, missing) | bar |\n|---|---|---|---|---|---|---|---|\n"
    );
    let mut details = String::new();
    let mut unmeasured = 0;
    for target in &targets {
        let found = match measured(&folder, target) {
            Ok(found) => found,
            Err(why) => {
                unmeasured += 1;
                let _ = writeln!(table, "| {target} | | | | | | | not measured: {why} |");
                continue;
            }
        };
        let ledger = &found.ledger;
        let met = if found.met() { "met" } else { "not met" };
        let _ = writeln!(
            table,
            "| {target} | {} of {} | {} | {:.1} s | {:.1} s | {:.2} | {}, {}, {}, {} | {met} |",
            found.totals.get("replayed").map_or("?", String::as_str),
            found.totals.get("sessions").map_or("?", String::as_str),
            found.totals.get("differing").map_or("?", String::as_str),
            found.number("native_seconds"),
            found.number("wasm_seconds"),
            found.number("ratio"),
            ledger.same,
            ledger.differ,
            ledger.limited,
            ledger.missing
        );
        let golden_time = |leg: &BTreeMap<String, String>| {
            leg.get("golden_seconds")
                .map_or_else(|| "?".to_owned(), |seconds| format!("{seconds} s"))
        };
        let _ = writeln!(
            details,
            "### {target}\n\nThe prover's golden took {} with native z3 and {} with the \
             WebAssembly z3; the module's one-time compile took {} s. Native z3's sessions: {} \
             of z3, {} questions; the WebAssembly leg's: {} of z3, {} through lengua-mcp's \
             host, {} questions. The ledgers compared, the base native z3's and the head the \
             WebAssembly z3's:\n\n```\n{}```\n",
            golden_time(&found.native),
            golden_time(&found.wasm),
            found
                .totals
                .get("compile_seconds")
                .map_or("?", String::as_str),
            found.native.get("z3_sessions").map_or("?", String::as_str),
            found.native.get("z3_questions").map_or("?", String::as_str),
            found.wasm.get("z3_sessions").map_or("?", String::as_str),
            found.wasm.get("z3_hosted").map_or("?", String::as_str),
            found.wasm.get("z3_questions").map_or("?", String::as_str),
            ledger.said
        );
    }
    let text = format!("{table}\n{details}");
    write(&folder.join(BAR_REPORT), &text)?;
    ci::print_and_summarize(&text)?;
    Ok(u8::from(unmeasured > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A z3 session as the prover keeps one: its first line, the options and the logic, two
    /// questions with their answers and times, the values asked for, a reason, and a solver that
    /// couldn't start.
    const SESSION: &str = "; z3 for session 3 of process 41: /solvers/z3 -in -smt2 -memory:4096\n\
                           (set-option :print-success false)\n(set-logic QF_BV)\n\
                           (echo \"@@lengua@@\")\n(declare-const t0 (_ BitVec 8))\n(push 1)\n\
                           (assert t1)\n(check-sat-using (try-for (then bit-blast sat) 20000))\n\
                           (echo \"@@lengua@@\")\n; z3: Sat in 1.5ms\n(get-value (t0))\n\
                           (echo \"@@lengua@@\")\n(pop 1)\n\
                           (check-sat-using (try-for (then bit-blast sat) 1500))\n\
                           (echo \"@@lengua@@\")\n; z3: Unknown in 1.502s\n\
                           ; cvc5 isn't installed\n";

    /// A session's file says which solver it is, the command that started it, its questions and
    /// their limits, its answers and how long they took, and who couldn't start.
    #[test]
    fn a_session_is_read_from_its_file() {
        let found = session("41-3-z3.smt2", SESSION);
        assert_eq!(found.solver, "z3");
        assert_eq!(found.command, "/solvers/z3 -in -smt2 -memory:4096");
        assert_eq!((found.questions, found.limits_ms), (2, 21_500));
        assert_eq!(found.answers, [1, 0, 1]);
        assert_eq!(found.answered, Duration::from_micros(1_503_500));
        assert_eq!(found.unstarted, ["cvc5 isn't installed"]);
        assert_eq!(duration("512µs"), Some(Duration::from_micros(512)));
        assert_eq!(duration("30ns"), Some(Duration::from_nanos(30)));
        assert_eq!(duration("2s"), Some(Duration::from_secs(2)));
        assert_eq!(duration("soon"), None);
    }

    /// The session files are a solver's, `<process>-<session>-<solver>.smt2`, in the order their
    /// sessions began: by number, not by name.
    #[test]
    fn the_sessions_are_listed_in_the_order_they_began() {
        let folder = tempfile::tempdir().expect("a folder");
        for name in [
            "41-10-z3.smt2",
            "41-2-z3.smt2",
            "41-2-cvc5.smt2",
            "7-30-z3.smt2",
            "notes.txt",
        ] {
            std::fs::write(folder.path().join(name), "").expect("a file");
        }
        let names: Vec<String> = session_files(folder.path(), Some("z3"))
            .iter()
            .map(|path| name_of(path))
            .collect();
        assert_eq!(names, ["7-30-z3.smt2", "41-2-z3.smt2", "41-10-z3.smt2"]);
        assert_eq!(session_files(folder.path(), None).len(), 4);
    }

    /// What z3 prints for [`SESSION`].
    const PRINTED: &str = "@@lengua@@\nsat\n@@lengua@@\n((t0 #x07))\n@@lengua@@\nunknown\n\
                           @@lengua@@\n";

    /// Two replays that print the same differ nowhere, and native z3's answers are counted; an
    /// answer that turns, a value that changes and a replay that ends early are each a kind of
    /// difference, with the first of each kept.
    #[test]
    fn replays_are_compared_block_by_block() {
        let same = differences(PRINTED, PRINTED);
        assert_eq!((same.answers, same.count()), (2, 0));
        // Windows's z3 ends its lines with a carriage return.
        let windows = PRINTED.replace('\n', "\r\n");
        assert_eq!(differences(&windows, PRINTED).count(), 0);
        let other = "@@lengua@@\nsat\n@@lengua@@\n((t0 #x08))\n@@lengua@@\nsat\n@@lengua@@\n";
        let found = differences(PRINTED, other);
        assert_eq!(found.by_kind.get(&Said::Answer), Some(&1));
        assert_eq!(found.by_kind.get(&Said::Values), Some(&1));
        assert_eq!(found.turns.get("unknown → sat"), Some(&1));
        assert_eq!(
            found.examples.get(&Said::Values),
            Some(&(2, "((t0 #x07))".to_owned(), "((t0 #x08))".to_owned()))
        );
        let ended = differences(PRINTED, "@@lengua@@\nsat\n");
        assert_eq!(ended.by_kind.get(&Said::Missing), Some(&3));
        assert_eq!(kind_of(&["(:reason-unknown \"timeout\")"]), Said::Reason);
        assert_eq!(kind_of(&["(error \"line 3\")"]), Said::Error);
        assert_eq!(kind_of(&[]), Said::Nothing);
    }

    /// A replay keeps what its program printed, what it said on its errors and its exit code,
    /// and stops a program that runs past its time: cargo stands in for a solver here, as it
    /// runs on every runner.
    #[test]
    fn a_replay_keeps_what_its_program_did() {
        let mut command = ci::cargo();
        command.arg("--version");
        let done = replay(command, b"", Duration::from_secs(60)).expect("cargo runs");
        assert!(done.printed.starts_with("cargo "), "{done:?}");
        assert!(done.ended_well(), "{done:?}");
        let mut command = ci::cargo();
        command.args(["no-such-command-here"]);
        let failed = replay(command, b"", Duration::from_secs(60)).expect("cargo runs");
        assert!(
            !failed.ended_well() && !failed.errors.is_empty(),
            "{failed:?}"
        );
    }

    /// The constants the replay shares with lengua-mcp are lengua-mcp's: the prover's arguments
    /// for z3, its marker, the host's flag and the three variables.
    #[test]
    fn the_replay_speaks_as_the_prover_does() {
        let smt = include_str!("../../src/prove/smt.rs");
        assert!(
            smt.contains(r#"Kind::Z3 => command.args(["-in", "-smt2", "-memory:4096"])"#),
            "the prover's z3 arguments changed: Z3_ARGUMENTS follows them"
        );
        assert!(smt.contains(&format!("const MARKER: &str = \"{MARKER}\";")));
        assert!(smt.contains(&format!("pub const SMT_LOG: &str = \"{SMT_LOG}\";")));
        assert!(smt.contains(&format!("pub const Z3_WASM: &str = \"{Z3_WASM}\";")));
        assert!(smt.contains(&format!("pub const SOLVER_HOST: &str = \"{SOLVER_HOST}\";")));
        let host = include_str!("../../src/wasm/solver.rs");
        assert!(host.contains(&format!("pub const FLAG: &str = \"{HOST_FLAG}\";")));
        assert_eq!(Z3_ARGUMENTS.join(" "), "-in -smt2 -memory:4096");
    }

    /// The bar is met when the verdicts agree, LIMIT lines aside, and the replay took at most
    /// twice as long in all over every session; not when a verdict differs, a block is missing,
    /// the ratio is over, or the replay stopped short.
    #[test]
    fn the_bar_is_the_owner_s() {
        let mut totals = BTreeMap::new();
        for (key, value) in [("ratio", "1.9"), ("sessions", "10"), ("replayed", "10")] {
            totals.insert(key.to_owned(), value.to_owned());
        }
        let ledger = golden::Compared {
            same: 150,
            limited: 3,
            ..golden::Compared::default()
        };
        let measured = Measured {
            ledger: ledger.clone(),
            totals: totals.clone(),
            ..Measured::default()
        };
        assert!(measured.met());
        let differs = Measured {
            ledger: golden::Compared {
                differ: 1,
                ..ledger.clone()
            },
            ..measured.clone()
        };
        assert!(!differs.met());
        let mut slow = measured.clone();
        slow.totals.insert("ratio".to_owned(), "2.01".to_owned());
        assert!(!slow.met());
        let mut short = measured.clone();
        short.totals.insert("replayed".to_owned(), "9".to_owned());
        assert!(!short.met());
        assert_eq!(grouped(4_567_890), "4,567,890");
        assert_eq!(grouped(12), "12");
    }
}
