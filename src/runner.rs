//! Runs a built program the way tests and `run` do: stdin given up front and then closed, an
//! empty environment, a working folder of its own, a time limit, an output limit, and a count
//! of the instructions it executed.
//!
//! The count comes from the CPU's performance counters. The counter must be attached before the
//! program starts, without `unsafe` code, so the server starts its own binary in launcher mode:
//! the launcher waits on a socket while the server attaches a counter that turns on at `exec`,
//! then confines itself and replaces itself with the program. Only the program's own
//! instructions are counted.
//!
//! The confinement is the kernel's. Landlock lets the program touch files only inside its working
//! folder and execute only itself, with no network; a resource limit caps the files it writes;
//! and seccomp stops it at any system call outside the list it was given. The checker has already
//! made sure the code makes no other calls: this makes the kernel keep the same promise.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, Read as _};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use landlock::{
    ABI, Access as _, AccessFs, AccessNet, PathBeneath, PathFd, Ruleset, RulesetAttr as _,
    RulesetCreatedAttr as _, Scope,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};

use perf_event::events::{Event, Hardware, Software};
use perf_event::{Builder, Counter, ReadFormat};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::platform::Allowed;
use crate::supervise::{CLOCK_GETTIME, GETRANDOM, SUPERVISE_FLAG};
use crate::supply::Supply;

/// The command-line flag that starts the binary as a launcher.
pub const LAUNCH_FLAG: &str = "--launch";

/// How long the launcher may take to start.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(5);

/// The most bytes a file may hold when tests run: writing past it stops the program.
pub const TEST_FILE_LIMIT: u64 = 64 * 1024 * 1024;

/// How long and how loud a program may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Killed after this long.
    pub timeout: Duration,
    /// Killed once standard output or standard error passes this many bytes.
    pub max_output: usize,
    /// Stopped when it writes a file past this many bytes, if there's a limit.
    pub max_file_size: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            max_output: 8 * 1024 * 1024,
            max_file_size: Some(TEST_FILE_LIMIT),
        }
    }
}

/// What to run, with what, and where.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    /// The executable.
    pub program: &'a Path,
    /// Its command-line arguments.
    pub args: &'a [String],
    /// Its standard input, closed after these bytes.
    pub stdin: &'a [u8],
    /// Files put in its fresh working folder before it starts, by name relative to the folder.
    pub files: &'a BTreeMap<String, String>,
    /// A folder to run in instead of a fresh one; the program may read and write inside it.
    pub folder: Option<&'a Path>,
    /// The system calls it may make; any other stops it.
    pub syscalls: &'a [Allowed],
    /// Random bytes and clock readings the test gives it in place of the kernel's.
    pub supply: Option<&'a Supply>,
}

impl<'a> Job<'a> {
    /// A job with no files, in a fresh folder.
    #[must_use]
    pub fn new(
        program: &'a Path,
        args: &'a [String],
        stdin: &'a [u8],
        syscalls: &'a [Allowed],
    ) -> Self {
        static NO_FILES: BTreeMap<String, String> = BTreeMap::new();
        Self {
            program,
            args,
            stdin,
            files: &NO_FILES,
            folder: None,
            syscalls,
            supply: None,
        }
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The program exited with this code.
    Exited(i32),
    /// A signal killed the program.
    Signaled(i32),
    /// It ran past the time limit and was killed.
    TimedOut,
    /// It printed past the output limit and was killed.
    OutputTooLarge,
}

impl End {
    /// A short description, such as "exit code 3" or "killed by SIGSEGV".
    #[must_use]
    pub fn describe(self, limits: Limits) -> String {
        match self {
            Self::Exited(code) => format!("exit code {code}"),
            Self::Signaled(signal) => format!("killed by {}", signal_name(signal)),
            Self::TimedOut => format!("still running after {:?}, so it was killed", limits.timeout),
            Self::OutputTooLarge => format!(
                "printed more than {} bytes, so it was killed",
                limits.max_output
            ),
        }
    }
}

fn signal_name(signal: i32) -> String {
    match signal {
        4 => "SIGILL (an instruction this CPU can't run)".into(),
        6 => "SIGABRT".into(),
        7 => "SIGBUS (a bad memory access)".into(),
        8 => "SIGFPE (division by zero or overflow in div)".into(),
        9 => "SIGKILL".into(),
        11 => "SIGSEGV (memory it may not touch)".into(),
        13 => "SIGPIPE".into(),
        25 => "SIGXFSZ (a file grew past its size limit)".into(),
        31 => "SIGSYS (a system call its spec doesn't allow)".into(),
        other => format!("signal {other}"),
    }
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Standard output, up to the limit.
    pub stdout: Vec<u8>,
    /// Standard error, up to the limit.
    pub stderr: Vec<u8>,
    /// How it ended.
    pub end: End,
    /// User-space instructions it executed, when the CPU's counters are available.
    pub instructions: Option<u64>,
    /// CPU cycles spent in its own code, when available.
    pub cycles: Option<u64>,
    /// CPU time in nanoseconds over all its threads, including what the kernel did for it when
    /// the system allows measuring that, when available.
    pub cpu_time_ns: Option<u64>,
    /// Time on the clock from its start to its end, in nanoseconds: what threads shorten.
    pub wall_time_ns: u64,
    /// Every file in its fresh working folder afterwards, by name relative to the folder, or why
    /// they couldn't be read. Empty when it ran in a folder of the user's.
    pub files: Result<BTreeMap<String, Vec<u8>>, String>,
    /// Why the test's random bytes or clock readings couldn't answer the program.
    pub supplied: Option<String>,
    /// How many random bytes the program asked for, when the test supplied them.
    pub random_asked: Option<usize>,
}

/// The most files read back from a working folder after a run.
const MAX_FILES_BACK: usize = 256;

/// The most bytes read back from a working folder after a run.
const MAX_BYTES_BACK: u64 = 256 * 1024 * 1024;

/// A counter on `pid` and every thread it starts, from its `exec`, or `None` when the system
/// doesn't allow it.
fn attach<E: Event>(event: E, pid: i32, with_kernel: bool) -> Option<Counter> {
    let mut builder = Builder::new(event);
    builder
        .observe_pid(pid)
        .inherit(true)
        .enable_on_exec(true)
        .exclude_kernel(!with_kernel)
        .read_format(ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING);
    builder.build().ok()
}

/// A counter's count, unless the CPU had to share it with other counters for part of the run,
/// which would make the count an estimate.
fn exact_count(counter: &mut Counter) -> Option<u64> {
    let data = counter.read_full().ok()?;
    (data.time_running() == data.time_enabled()).then(|| data.count())
}

/// Runs `job` through `launcher` (this server's own binary).
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn run(launcher: &Path, job: Job<'_>, limits: Limits) -> io::Result<Run> {
    let scratch = tempfile::tempdir()?;
    let socket = scratch.path().join("go");
    let work = scratch.path().join("work");
    let folder = match job.folder {
        Some(folder) => folder.to_path_buf(),
        None => {
            std::fs::create_dir(&work)?;
            write_files(&work, job.files)?;
            work.clone()
        }
    };
    let listener = UnixListener::bind(&socket)?;
    let supply = job.supply.filter(|supply| !supply.is_empty());
    // The calls the supply answers stop for the supervisor; the rest run as in any test.
    let traced = supply.map_or_else(Vec::new, |supply| {
        let mut traced = Vec::new();
        if supply.random.is_some() {
            traced.push(GETRANDOM);
        }
        if !supply.clock.is_empty() {
            traced.push(CLOCK_GETTIME);
        }
        traced
    });
    let syscalls: Vec<String> = job
        .syscalls
        .iter()
        .map(|allowed| match allowed.first_argument {
            None => allowed.number.to_string(),
            Some(argument) => format!("{}:{argument}", allowed.number),
        })
        .chain(traced.iter().map(|number| format!("{number}:trace")))
        .collect();
    let file_limit = limits
        .max_file_size
        .map_or_else(|| "none".to_owned(), |bytes| bytes.to_string());
    let mut supervisor = match supply {
        None => None,
        Some(_) => Some(
            Command::new(launcher)
                .arg(SUPERVISE_FLAG)
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?,
        ),
    };
    let tracer = supervisor
        .as_ref()
        .and_then(tokio::process::Child::id)
        .map_or_else(|| "none".to_owned(), |pid| pid.to_string());
    let mut child = Command::new(launcher)
        .arg(LAUNCH_FLAG)
        .arg(&socket)
        .arg(syscalls.join(","))
        .arg(file_limit)
        .arg(tracer)
        .arg(job.program)
        .args(job.args)
        .current_dir(&folder)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let pid = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .ok_or_else(|| io::Error::other("the launcher has no process id"))?;
    let (mut go, _) = tokio::time::timeout(LAUNCH_TIMEOUT, listener.accept())
        .await
        .map_err(|_| io::Error::other("the launcher didn't start"))??;
    // The launcher has let the supervisor trace it; now it may, before the program starts.
    let mut verdict = None;
    if let (Some(supervisor), Some(supply)) = (supervisor.as_mut(), supply) {
        verdict = Some(attach_supervisor(supervisor, pid, supply).await?);
    }
    // Count from the program's exec to its exit; unavailable when the system doesn't allow
    // performance counters.
    let mut counters = [
        attach(Hardware::INSTRUCTIONS, pid, false),
        attach(Hardware::CPU_CYCLES, pid, false),
        attach(Software::TASK_CLOCK, pid, true)
            .or_else(|| attach(Software::TASK_CLOCK, pid, false)),
    ];
    go.write_all(&[1]).await?;
    drop(go);
    let started = std::time::Instant::now();

    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no stdin pipe"))?;
    let output = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no stdout pipe"))?;
    let errors = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("no stderr pipe"))?;
    let bytes = job.stdin.to_vec();
    let writer = tokio::spawn(async move {
        // A program may exit without reading its input; that's not an error.
        let _ = input.write_all(&bytes).await;
    });
    let (overflow, mut overflowed) = mpsc::channel::<()>(2);
    let stdout_reader = tokio::spawn(read_capped(output, limits.max_output, overflow.clone()));
    let stderr_reader = tokio::spawn(read_capped(errors, limits.max_output, overflow));
    let mut wall_time = Duration::ZERO;
    let end = tokio::select! {
        status = child.wait() => {
            wall_time = started.elapsed();
            let status = status?;
            match status.code() {
                Some(code) => End::Exited(code),
                None => End::Signaled(status.signal().unwrap_or(0)),
            }
        }
        Some(()) = overflowed.recv() => {
            let _ = child.kill().await;
            End::OutputTooLarge
        }
        () = tokio::time::sleep(limits.timeout) => {
            let _ = child.kill().await;
            End::TimedOut
        }
    };
    writer.abort();
    let stdout = stdout_reader.await.map_err(io::Error::other)??;
    let stderr = stderr_reader.await.map_err(io::Error::other)??;
    let (supplied, random_asked) = match (supervisor.as_mut(), verdict) {
        (Some(supervisor), Some(verdict)) => match supervisor_verdict(supervisor, verdict).await {
            Ok(asked) => (None, Some(asked)),
            Err(problem) => (Some(problem), None),
        },
        _ => (None, None),
    };
    let [instructions, cycles, cpu_time_ns] = counters.each_mut().map(|counter| match end {
        End::Exited(_) | End::Signaled(_) => counter.as_mut().and_then(exact_count),
        End::TimedOut | End::OutputTooLarge => None,
    });
    let files = if job.folder.is_some() {
        Ok(BTreeMap::new())
    } else {
        read_files(&work)
    };
    Ok(Run {
        stdout,
        stderr,
        end,
        instructions,
        cycles,
        cpu_time_ns,
        wall_time_ns: u64::try_from(wall_time.as_nanos()).unwrap_or(u64::MAX),
        files,
        supplied,
        random_asked,
    })
}

/// Gives the supervisor the launcher's process id and the supply, and waits until it traces the
/// launcher. Returns the reader of its last word.
async fn attach_supervisor(
    supervisor: &mut tokio::process::Child,
    launcher: i32,
    supply: &Supply,
) -> io::Result<tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>> {
    use tokio::io::AsyncBufReadExt as _;
    let mut input = supervisor
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no supervisor stdin"))?;
    input
        .write_all(format!("{launcher}\n{}", supply.encode()).as_bytes())
        .await?;
    drop(input);
    let output = supervisor
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no supervisor stdout"))?;
    let mut lines = tokio::io::BufReader::new(output).lines();
    let first = tokio::time::timeout(LAUNCH_TIMEOUT, lines.next_line())
        .await
        .map_err(|_| io::Error::other("lengua's test supervisor didn't start"))??;
    match first.as_deref() {
        Some("attached") => Ok(lines),
        Some(line) => Err(io::Error::other(
            line.strip_prefix("error: ").unwrap_or(line).to_owned(),
        )),
        None => Err(io::Error::other(
            "lengua's test supervisor ended before the program started",
        )),
    }
}

/// What the supervisor found once the program ended: how many random bytes the program asked
/// for, or what went wrong.
async fn supervisor_verdict(
    supervisor: &mut tokio::process::Child,
    mut lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
) -> Result<usize, String> {
    let last = tokio::time::timeout(LAUNCH_TIMEOUT, lines.next_line()).await;
    let _ = supervisor.kill().await;
    match last {
        Ok(Ok(Some(line))) => match line.strip_prefix("ok ").map(str::parse) {
            Some(Ok(asked)) => Ok(asked),
            _ => Err(line.strip_prefix("error: ").unwrap_or(&line).to_owned()),
        },
        _ => Err("lengua's test supervisor stopped before the program ended".into()),
    }
}

/// Puts `files` in `folder`, making the folders their names include.
fn write_files(folder: &Path, files: &BTreeMap<String, String>) -> io::Result<()> {
    for (name, contents) in files {
        let path = folder.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    }
    Ok(())
}

/// Every file under `folder`, by name relative to it, or why they can't all be read.
fn read_files(folder: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let unreadable = |error: io::Error| format!("its working folder couldn't be read: {error}");
    let mut files = BTreeMap::new();
    let mut total = 0;
    let mut pending = vec![folder.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            let name = path
                .strip_prefix(folder)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let kind = entry.file_type().map_err(unreadable)?;
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            if !kind.is_file() {
                return Err(format!("it left {name}, which isn't a regular file"));
            }
            if files.len() == MAX_FILES_BACK {
                return Err(format!("it left more than {MAX_FILES_BACK} files"));
            }
            total += entry.metadata().map_err(unreadable)?.len();
            if total > MAX_BYTES_BACK {
                return Err(format!(
                    "its files hold more than {} MiB",
                    MAX_BYTES_BACK / 1024 / 1024
                ));
            }
            files.insert(name, std::fs::read(&path).map_err(unreadable)?);
        }
    }
    Ok(files)
}

/// Reads until the end, keeping at most `cap` bytes and reporting when there's more.
async fn read_capped(
    mut reader: impl AsyncRead + Unpin,
    cap: usize,
    overflow: mpsc::Sender<()>,
) -> io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(kept);
        }
        kept.extend_from_slice(&chunk[..read]);
        if kept.len() > cap {
            kept.truncate(cap);
            let _ = overflow.send(()).await;
            return Ok(kept);
        }
    }
}

/// Launcher mode: waits for the server's go signal on `socket`, confines itself, then becomes
/// the program with an empty environment. Arguments: the socket, the allowed system call numbers
/// separated by commas, the file size limit in bytes (or `none`), the supervisor's process id
/// (or `none`), the program, then the program's arguments.
#[must_use]
pub fn launch(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    let (Some(socket), Some(syscalls), Some(file_limit), Some(tracer), Some(program)) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        eprintln!(
            "lengua-mcp: {LAUNCH_FLAG} needs a socket, system calls, a file limit, a supervisor \
             and a program"
        );
        return ExitCode::from(127);
    };
    let rest: Vec<OsString> = args.collect();
    // Only the test's supervisor may trace the program, even where the system restricts tracing.
    let supervisor = tracer
        .to_str()
        .and_then(|pid| pid.parse::<i32>().ok())
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = supervisor
        && let Err(error) = rustix::process::set_ptracer(rustix::process::PTracer::ProcessID(pid))
    {
        eprintln!("lengua-mcp: couldn't let the supervisor trace the program: {error}");
        return ExitCode::from(127);
    }
    let mut go = [0_u8; 1];
    let signalled = std::os::unix::net::UnixStream::connect(&socket)
        .and_then(|mut stream| stream.read_exact(&mut go));
    if let Err(error) = signalled {
        eprintln!("lengua-mcp: no go signal from the server: {error}");
        return ExitCode::from(127);
    }
    let confined =
        parse_confinement(&syscalls, &file_limit).and_then(|(syscalls, traced, file_limit)| {
            confine(Path::new(&program), &syscalls, &traced, file_limit)
        });
    if let Err(problem) = confined {
        eprintln!("lengua-mcp: couldn't confine the program: {problem}");
        return ExitCode::from(127);
    }
    let error = std::process::Command::new(&program)
        .args(&rest)
        .env_clear()
        .exec();
    eprintln!(
        "lengua-mcp: couldn't start {}: {error}",
        Path::new(&program).display()
    );
    ExitCode::from(127)
}

/// The launcher's confinement: the calls allowed, the calls the supervisor answers, and the
/// file size limit.
type Confinement = (Vec<Allowed>, Vec<i64>, Option<u64>);

/// Reads the launcher's system call list ("0,1,56:3477248,318:trace": a number and, after a
/// colon, the only first argument allowed, or `trace` for a call the supervisor answers) and file
/// limit ("1048576" or "none").
fn parse_confinement(syscalls: &OsString, file_limit: &OsString) -> Result<Confinement, String> {
    let syscalls = syscalls.to_string_lossy();
    let mut traced = Vec::new();
    for entry in syscalls.split(',') {
        if let Some(number) = entry.strip_suffix(":trace") {
            traced.push(
                number
                    .parse()
                    .map_err(|_| format!("{entry:?} isn't a system call"))?,
            );
        }
    }
    let numbers = syscalls
        .split(',')
        .filter(|entry| !entry.is_empty() && !entry.ends_with(":trace"))
        .map(|entry| {
            let bad = || format!("{entry:?} isn't a system call");
            let (number, argument) = match entry.split_once(':') {
                Some((number, argument)) => (number, Some(argument.parse().map_err(|_| bad())?)),
                None => (entry, None),
            };
            Ok(Allowed {
                number: number.parse().map_err(|_| bad())?,
                first_argument: argument,
            })
        })
        .collect::<Result<_, String>>()?;
    let file_limit = match file_limit.to_string_lossy().as_ref() {
        "none" => None,
        bytes => Some(
            bytes
                .parse()
                .map_err(|_| format!("{bytes:?} isn't a file size limit"))?,
        ),
    };
    Ok((numbers, traced, file_limit))
}

/// Confines this process, and so the program it becomes: files only inside the current folder
/// (its working folder), executing only `program`, no network and no signals to other
/// processes (Landlock); files of at most `file_limit` bytes and no core files; and only the
/// system calls in `syscalls` (seccomp), so any other stops it with SIGSYS.
fn confine(
    program: &Path,
    syscalls: &[Allowed],
    traced: &[i64],
    file_limit: Option<u64>,
) -> Result<(), String> {
    let abi = ABI::V6;
    let folder = PathFd::new(".").map_err(|error| format!("working folder: {error}"))?;
    let executable = PathFd::new(program).map_err(|error| format!("program: {error}"))?;
    Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .and_then(|ruleset| ruleset.handle_access(AccessNet::from_all(abi)))
        .and_then(|ruleset| ruleset.scope(Scope::from_all(abi)))
        .and_then(Ruleset::create)
        .and_then(|ruleset| ruleset.add_rule(PathBeneath::new(folder, AccessFs::from_all(abi))))
        .and_then(|ruleset| {
            ruleset.add_rule(PathBeneath::new(
                executable,
                AccessFs::Execute | AccessFs::ReadFile,
            ))
        })
        .and_then(|ruleset| ruleset.restrict_self())
        .map_err(|error| format!("Landlock: {error}"))?;
    let limit = |bytes| rustix::process::Rlimit {
        current: Some(bytes),
        maximum: Some(bytes),
    };
    if let Some(bytes) = file_limit {
        rustix::process::setrlimit(rustix::process::Resource::Fsize, limit(bytes))
            .map_err(|error| format!("file size limit: {error}"))?;
    }
    // A crash leaves no core file behind.
    rustix::process::setrlimit(rustix::process::Resource::Core, limit(0))
        .map_err(|error| format!("core file limit: {error}"))?;
    // No rules allow any arguments; a rule allows only its first argument.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    let mut any = BTreeSet::new();
    for allowed in syscalls {
        let entry = rules.entry(allowed.number).or_default();
        match allowed.first_argument {
            None => {
                any.insert(allowed.number);
            }
            Some(argument) => entry.push(
                SeccompCondition::new(0, SeccompCmpArgLen::Qword, SeccompCmpOp::Eq, argument)
                    .and_then(|condition| SeccompRule::new(vec![condition]))
                    .map_err(|error| format!("seccomp: {error}"))?,
            ),
        }
    }
    for number in any {
        rules.insert(number, Vec::new());
    }
    let arch = TargetArch::try_from(std::env::consts::ARCH)
        .map_err(|error| format!("seccomp: {error}"))?;
    // The calls a test's supply answers stop for its supervisor. This filter goes first: once
    // the next one is in, installing another would be a call it kills. The kernel applies the
    // stricter answer of the two, so these calls stop, and every other call is as below.
    if !traced.is_empty() {
        let stops = traced.iter().map(|number| (*number, Vec::new())).collect();
        let filter: BpfProgram =
            SeccompFilter::new(stops, SeccompAction::Allow, SeccompAction::Trace(0), arch)
                .and_then(TryInto::try_into)
                .map_err(|error| format!("seccomp: {error}"))?;
        seccompiler::apply_filter(&filter).map_err(|error| format!("seccomp: {error}"))?;
    }
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::KillProcess,
        SeccompAction::Allow,
        arch,
    )
    .map_err(|error| format!("seccomp: {error}"))?;
    let filter: BpfProgram = filter
        .try_into()
        .map_err(|error| format!("seccomp: {error}"))?;
    seccompiler::apply_filter(&filter).map_err(|error| format!("seccomp: {error}"))
}
