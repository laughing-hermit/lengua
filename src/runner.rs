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
use std::io::{self, Read as _, Write as _};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use landlock::{
    ABI, Access as _, AccessFs, AccessNet, NetPort, PathBeneath, PathFd, Ruleset, RulesetAttr as _,
    RulesetCreatedAttr as _, RulesetStatus, Scope,
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

use crate::platform::{Allowed, Pin};
use crate::supervise::{CLOCK_GETTIME, CLOCK_NANOSLEEP, CONNECT, GETRANDOM, SUPERVISE_FLAG};
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

/// The longest standard input written in pieces waits for the program to read a piece before
/// writing the next.
const STDIN_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// How often it looks whether the program has read it.
const STDIN_POLL: std::time::Duration = std::time::Duration::from_millis(1);

/// What to run, with what, and where.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    /// The executable.
    pub program: &'a Path,
    /// Its command-line arguments.
    pub args: &'a [String],
    /// Its standard input, closed after these bytes.
    pub stdin: &'a [u8],
    /// The pieces its standard input is written in, by length, each once the program has read
    /// the last; empty, it's written whole.
    pub stdin_pieces: &'a [usize],
    /// A file to be its standard input instead, read by the program itself, however large.
    pub stdin_file: Option<&'a Path>,
    /// Files put in its fresh working folder before it starts, by name relative to the folder.
    pub files: &'a BTreeMap<String, String>,
    /// Empty folders put in its fresh working folder before it starts.
    pub folders: &'a BTreeSet<String>,
    /// Its environment variables, by name: no others reach it.
    pub env: &'a BTreeMap<String, String>,
    /// A folder to run in instead of a fresh one; the program may read and write inside it.
    pub folder: Option<&'a Path>,
    /// The system calls it may make; any other stops it.
    pub syscalls: &'a [Allowed],
    /// Random bytes and clock readings the test gives it in place of the kernel's.
    pub supply: Option<&'a Supply>,
    /// The network it may use; none at all when `None`.
    pub net: Option<&'a Net>,
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
        static NO_FOLDERS: BTreeSet<String> = BTreeSet::new();
        Self {
            program,
            args,
            stdin,
            stdin_pieces: &[],
            files: &NO_FILES,
            folders: &NO_FOLDERS,
            env: &NO_FILES,
            folder: None,
            syscalls,
            supply: None,
            net: None,
            stdin_file: None,
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
    /// lengua stopped it while it was still running, as a server is at the end of its test.
    Stopped,
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
            Self::Stopped => "stopped by lengua while it was still running".into(),
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
    /// Every folder in it afterwards, by name relative to it, empty ones included.
    pub folders: BTreeSet<String>,
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

/// The network a run may use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Net {
    /// TCP ports the program may connect to.
    pub connect: Vec<u16>,
    /// Whether it may connect to other computers on them: otherwise lengua's supervisor reads
    /// each connection's address and refuses any computer but this one, which the kernel's
    /// port rules can't.
    pub anywhere: bool,
    /// The loopback port lengua listens on for the program and hands it as file descriptor 3,
    /// or 0 for any free one; `None` for a program that doesn't serve.
    pub listen: Option<u16>,
}

impl Net {
    /// Whether the program uses the network at all.
    #[must_use]
    pub fn is_used(&self) -> bool {
        !self.connect.is_empty() || self.listen.is_some()
    }

    /// The launcher's argument: `none`, or `connect=80,8080` and `listen=0`, joined by `;`.
    fn encode(&self) -> String {
        let mut parts = Vec::new();
        if !self.connect.is_empty() {
            let ports: Vec<String> = self.connect.iter().map(u16::to_string).collect();
            parts.push(format!("connect={}", ports.join(",")));
        }
        if let Some(port) = self.listen {
            parts.push(format!("listen={port}"));
        }
        if parts.is_empty() {
            "none".into()
        } else {
            parts.join(";")
        }
    }

    fn decode(text: &str) -> Result<Self, String> {
        let mut net = Self::default();
        let bad = || format!("{text:?} isn't a network setting");
        for part in text.split(';').filter(|part| *part != "none") {
            match part.split_once('=') {
                Some(("connect", ports)) => {
                    for port in ports.split(',') {
                        net.connect.push(port.parse().map_err(|_| bad())?);
                    }
                }
                Some(("listen", port)) => net.listen = Some(port.parse().map_err(|_| bad())?),
                _ => return Err(bad()),
            }
        }
        Ok(net)
    }
}

/// Runs `job` through `launcher` (this server's own binary) until it ends.
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn run(launcher: &Path, job: Job<'_>, limits: Limits) -> io::Result<Run> {
    start(launcher, job, limits).await?.wait().await
}

/// A program that's running.
pub struct Started {
    child: tokio::process::Child,
    pid: i32,
    port: Option<u16>,
    began: std::time::Instant,
    limits: Limits,
    counters: [Option<Counter>; 3],
    writer: tokio::task::JoinHandle<()>,
    stdout: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    stderr: tokio::task::JoinHandle<io::Result<Vec<u8>>>,
    overflowed: mpsc::Receiver<()>,
    supervisor: Option<(tokio::process::Child, SupervisorLines)>,
    /// The fresh working folder to read back, when it isn't one of the user's.
    work: Option<std::path::PathBuf>,
    _scratch: tempfile::TempDir,
}

type SupervisorLines = tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>;

impl Started {
    /// The program's process id.
    #[must_use]
    pub const fn pid(&self) -> i32 {
        self.pid
    }

    /// The loopback port it serves on, when it serves.
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// Waits for the program to end, stopping it at the time or output limit.
    ///
    /// # Errors
    ///
    /// Fails when its outcome can't be read.
    pub async fn wait(self) -> io::Result<Run> {
        self.finish(false).await
    }

    /// Stops the program now, as lengua does a server that's still serving.
    ///
    /// # Errors
    ///
    /// Fails when its outcome can't be read.
    pub async fn stop(self) -> io::Result<Run> {
        self.finish(true).await
    }

    async fn finish(mut self, stop: bool) -> io::Result<Run> {
        let deadline = tokio::time::Instant::from_std(self.began + self.limits.timeout);
        let mut wall_time = Duration::ZERO;
        let exited = if stop { self.child.try_wait()? } else { None };
        let end = if let Some(status) = exited {
            wall_time = self.began.elapsed();
            match status.code() {
                Some(code) => End::Exited(code),
                None => End::Signaled(status.signal().unwrap_or(0)),
            }
        } else if stop {
            let _ = self.child.kill().await;
            wall_time = self.began.elapsed();
            End::Stopped
        } else {
            tokio::select! {
                status = self.child.wait() => {
                    wall_time = self.began.elapsed();
                    let status = status?;
                    match status.code() {
                        Some(code) => End::Exited(code),
                        None => End::Signaled(status.signal().unwrap_or(0)),
                    }
                }
                Some(()) = self.overflowed.recv() => {
                    let _ = self.child.kill().await;
                    End::OutputTooLarge
                }
                () = tokio::time::sleep_until(deadline) => {
                    let _ = self.child.kill().await;
                    End::TimedOut
                }
            }
        };
        self.writer.abort();
        let stdout = self.stdout.await.map_err(io::Error::other)??;
        let stderr = self.stderr.await.map_err(io::Error::other)??;
        let (supplied, random_asked) = match self.supervisor.as_mut() {
            Some((supervisor, lines)) => match supervisor_verdict(supervisor, lines).await {
                Ok(asked) => (None, Some(asked)),
                Err(problem) => (Some(problem), None),
            },
            None => (None, None),
        };
        let [instructions, cycles, cpu_time_ns] =
            self.counters.each_mut().map(|counter| match end {
                End::Exited(_) | End::Signaled(_) => counter.as_mut().and_then(exact_count),
                End::TimedOut | End::OutputTooLarge | End::Stopped => None,
            });
        let (files, folders) = match &self.work {
            Some(work) => match read_files(work) {
                Ok((files, folders)) => (Ok(files), folders),
                Err(problem) => (Err(problem), BTreeSet::new()),
            },
            None => (Ok(BTreeMap::new()), BTreeSet::new()),
        };
        Ok(Run {
            stdout,
            stderr,
            end,
            folders,
            instructions,
            cycles,
            cpu_time_ns,
            wall_time_ns: u64::try_from(wall_time.as_nanos()).unwrap_or(u64::MAX),
            files,
            supplied,
            random_asked,
        })
    }
}

/// Starts `job` through `launcher` (this server's own binary) and returns once the program is
/// running: confined, counted, and serving on its port if it serves.
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn start(launcher: &Path, job: Job<'_>, limits: Limits) -> io::Result<Started> {
    let scratch = tempfile::tempdir()?;
    let socket = scratch.path().join("go");
    let work = scratch.path().join("work");
    let folder = match job.folder {
        Some(folder) => folder.to_path_buf(),
        None => {
            std::fs::create_dir(&work)?;
            write_files(&work, job.files)?;
            for folder in job.folders {
                std::fs::create_dir_all(work.join(folder))?;
            }
            work.clone()
        }
    };
    let listener = UnixListener::bind(&socket)?;
    let supply = job.supply.filter(|supply| !supply.is_empty());
    let net = job.net.cloned().unwrap_or_default();
    // The calls the supply answers stop for the supervisor; the rest run as in any test.
    let mut traced = supply.map_or_else(Vec::new, |supply| {
        let mut traced = Vec::new();
        if supply.random.is_some() {
            traced.push(GETRANDOM);
        }
        if !supply.clock.is_empty() {
            traced.push(CLOCK_GETTIME);
        }
        traced
    });
    // A program may reach only this computer unless its spec says any computer: the
    // supervisor reads where each connection goes, since the kernel's rules are by port alone.
    let local = !net.connect.is_empty() && !net.anywhere;
    if local {
        traced.push(CONNECT);
    }
    // A test's sleeps end at once; a real run's take as long as they say.
    let sleeps = job.supply.is_some()
        && job
            .syscalls
            .iter()
            .any(|allowed| allowed.number == CLOCK_NANOSLEEP);
    if sleeps {
        traced.push(CLOCK_NANOSLEEP);
    }
    let watched = supply.is_some() || local || sleeps;
    let nothing_supplied = Supply::default();
    let supply = supply.or_else(|| (local || sleeps).then_some(&nothing_supplied));
    let syscalls: Vec<String> = job
        .syscalls
        .iter()
        .map(|allowed| format!("{}{}", allowed.number, allowed.pin.encode()))
        .chain(traced.iter().map(|number| format!("{number}:trace")))
        .collect();
    let file_limit = limits
        .max_file_size
        .map_or_else(|| "none".to_owned(), |bytes| bytes.to_string());
    let mut supervisor = match supply.filter(|_| watched) {
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
    let input = match job.stdin_file {
        Some(path) => Stdio::from(std::fs::File::open(path)?),
        None => Stdio::piped(),
    };
    let mut child = Command::new(launcher)
        .arg(LAUNCH_FLAG)
        .arg(&socket)
        .arg(syscalls.join(","))
        .arg(file_limit)
        .arg(tracer)
        .arg(net.encode())
        .arg(encode_environment(job.env))
        .arg(job.program)
        .args(job.args)
        .current_dir(&folder)
        .env_clear()
        .stdin(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let pid = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .ok_or_else(|| io::Error::other("the launcher has no process id"))?;
    let accepted = tokio::time::timeout(LAUNCH_TIMEOUT, listener.accept()).await;
    let Ok(Ok((mut go, _))) = accepted else {
        // The launcher says why on its standard error, such as a port that's taken.
        let _ = child.kill().await;
        let mut said = String::new();
        if let Some(mut errors) = child.stderr.take() {
            let _ = errors.read_to_string(&mut said).await;
        }
        let said = said.trim().trim_start_matches("lengua-mcp: ");
        return Err(io::Error::other(if said.is_empty() {
            "the launcher didn't start".to_owned()
        } else {
            said.to_owned()
        }));
    };
    // A server's launcher has already bound its port, and says which.
    let mut port = None;
    if net.listen.is_some() {
        let mut bytes = [0_u8; 2];
        go.read_exact(&mut bytes).await?;
        port = Some(u16::from_be_bytes(bytes));
    }
    // The launcher has let the supervisor trace it; now it may, before the program starts.
    let supervisor = match (supervisor.take(), supply) {
        (Some(mut supervisor), Some(supply)) => {
            let lines = attach_supervisor(&mut supervisor, pid, supply, local).await?;
            Some((supervisor, lines))
        }
        _ => None,
    };
    // Count from the program's exec to its exit; unavailable when the system doesn't allow
    // performance counters.
    let counters = [
        attach(Hardware::INSTRUCTIONS, pid, false),
        attach(Hardware::CPU_CYCLES, pid, false),
        attach(Software::TASK_CLOCK, pid, true)
            .or_else(|| attach(Software::TASK_CLOCK, pid, false)),
    ];
    go.write_all(&[1]).await?;
    drop(go);
    let began = std::time::Instant::now();

    let input = child.stdin.take();
    let output = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no stdout pipe"))?;
    let errors = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("no stderr pipe"))?;
    let bytes = job.stdin.to_vec();
    let pieces = job.stdin_pieces.to_vec();
    let writer = tokio::spawn(async move {
        // A program may exit without reading its input; that's not an error.
        let Some(mut input) = input else {
            return;
        };
        if pieces.len() < 2 {
            let _ = input.write_all(&bytes).await;
            return;
        }
        // Each piece its own write, once the program has read all of the last, so a read ends
        // where a piece does however busy the computer is.
        let mut start = 0;
        let last = pieces.len() - 1;
        for (index, length) in pieces.into_iter().enumerate() {
            let end = (start + length).min(bytes.len());
            if input.write_all(&bytes[start..end]).await.is_err() {
                return;
            }
            let _ = input.flush().await;
            start = end;
            let waited = tokio::time::Instant::now();
            while index < last
                && rustix::io::ioctl_fionread(&input).is_ok_and(|unread| unread > 0)
                && waited.elapsed() < STDIN_WAIT
            {
                tokio::time::sleep(STDIN_POLL).await;
            }
        }
    });
    let (overflow, overflowed) = mpsc::channel::<()>(2);
    let stdout = tokio::spawn(read_capped(output, limits.max_output, overflow.clone()));
    let stderr = tokio::spawn(read_capped(errors, limits.max_output, overflow));
    Ok(Started {
        child,
        pid,
        port,
        began,
        limits,
        counters,
        writer,
        stdout,
        stderr,
        overflowed,
        supervisor,
        work: job.folder.is_none().then_some(work),
        _scratch: scratch,
    })
}

/// Gives the supervisor the launcher's process id and the supply, and waits until it traces the
/// launcher. Returns the reader of its last word.
async fn attach_supervisor(
    supervisor: &mut tokio::process::Child,
    launcher: i32,
    supply: &Supply,
    local: bool,
) -> io::Result<SupervisorLines> {
    use tokio::io::AsyncBufReadExt as _;
    let mut input = supervisor
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no supervisor stdin"))?;
    let reach = if local { "local" } else { "any" };
    input
        .write_all(format!("{launcher}\n{reach}\n{}", supply.encode()).as_bytes())
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
    lines: &mut SupervisorLines,
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

/// A program's environment as the launcher's argument: each `NAME=value` in hex, joined by
/// commas, or `none`. Only the program gets it, never the launcher, which runs before its
/// confinement and would read variables such as `LD_PRELOAD`.
fn encode_environment(env: &BTreeMap<String, String>) -> String {
    if env.is_empty() {
        return "none".into();
    }
    env.iter()
        .map(|(name, value)| {
            format!("{name}={value}")
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The variables [`encode_environment`] wrote, or `None` for anything it didn't.
fn decode_environment(text: &str) -> Option<Vec<(String, String)>> {
    if text == "none" {
        return Some(Vec::new());
    }
    text.split(',')
        .map(|hex| {
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
                .collect::<Option<Vec<u8>>>()?;
            let pair = String::from_utf8(bytes).ok()?;
            let (name, value) = pair.split_once('=')?;
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
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

/// Every file under `folder`, and every folder, by name relative to it, or why they can't all be
/// read.
type Left = (BTreeMap<String, Vec<u8>>, BTreeSet<String>);

fn read_files(folder: &Path) -> Result<Left, String> {
    let unreadable = |error: io::Error| format!("its working folder couldn't be read: {error}");
    let mut files = BTreeMap::new();
    let mut folders = BTreeSet::new();
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
                folders.insert(name);
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
    Ok((files, folders))
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
    let (
        Some(socket),
        Some(syscalls),
        Some(file_limit),
        Some(tracer),
        Some(network),
        Some(environment),
        Some(program),
    ) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    )
    else {
        eprintln!(
            "lengua-mcp: {LAUNCH_FLAG} needs a socket, system calls, a file limit, a supervisor, \
             a network setting, an environment and a program"
        );
        return ExitCode::from(127);
    };
    let rest: Vec<OsString> = args.collect();
    let Some(environment) = decode_environment(&environment.to_string_lossy()) else {
        eprintln!("lengua-mcp: the environment isn't one lengua wrote");
        return ExitCode::from(127);
    };
    let net = match Net::decode(&network.to_string_lossy()) {
        Ok(net) => net,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
    // A server's listening socket comes first, so it's file descriptor 3; it stays open into
    // the program, which only takes connections on it.
    let listener = match net.listen.map(listen_on_fd3).transpose() {
        Ok(listener) => listener,
        Err(problem) => {
            eprintln!("lengua-mcp: {problem}");
            return ExitCode::from(127);
        }
    };
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
    let signalled = std::os::unix::net::UnixStream::connect(&socket).and_then(|mut stream| {
        if let Some(listener) = &listener {
            stream.write_all(&listener.local_addr()?.port().to_be_bytes())?;
        }
        stream.read_exact(&mut go)
    });
    if let Err(error) = signalled {
        eprintln!("lengua-mcp: no go signal from the server: {error}");
        return ExitCode::from(127);
    }
    let confined =
        parse_confinement(&syscalls, &file_limit).and_then(|(syscalls, traced, file_limit)| {
            confine(Path::new(&program), &syscalls, &traced, file_limit, &net)
        });
    if let Err(problem) = confined {
        eprintln!("lengua-mcp: couldn't confine the program: {problem}");
        return ExitCode::from(127);
    }
    // The program's own variables reach it only now: the launcher itself ran with none.
    let error = std::process::Command::new(&program)
        .args(&rest)
        .env_clear()
        .envs(environment)
        .exec();
    eprintln!(
        "lengua-mcp: couldn't start {}: {error}",
        Path::new(&program).display()
    );
    ExitCode::from(127)
}

/// Listens on `port` of the loopback (any free port for 0) with file descriptor 3, kept open
/// across `exec`.
fn listen_on_fd3(port: u16) -> Result<std::net::TcpListener, String> {
    use std::os::fd::AsRawFd as _;
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .map_err(|error| format!("couldn't listen on 127.0.0.1:{port}: {error}"))?;
    if listener.as_raw_fd() != 3 {
        return Err("the listening socket isn't file descriptor 3".into());
    }
    rustix::io::fcntl_setfd(&listener, rustix::io::FdFlags::empty())
        .map_err(|error| format!("couldn't keep the listening socket open: {error}"))?;
    Ok(listener)
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
            let (number, pin) = match entry.split_once(':') {
                Some((number, pin)) => (number, Pin::decode(pin).ok_or_else(bad)?),
                None => (entry, Pin::Any),
            };
            Ok(Allowed {
                number: number.parse().map_err(|_| bad())?,
                pin,
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

/// The seccomp rules a pin stands for: any one of them allows the call.
fn pin_rules(pin: Pin) -> Result<Vec<SeccompRule>, seccompiler::BackendError> {
    use SeccompCmpArgLen::{Dword, Qword};
    let condition = |argument, length, operator, value| {
        SeccompCondition::new(argument, length, operator, value)
    };
    const SOCK_STREAM: u64 = 1;
    // Everything but SOCK_NONBLOCK and SOCK_CLOEXEC, which only change how it waits and closes.
    const SOCKET_TYPE: u64 = 0xFFF7_F7FF;
    const MSG_NOSIGNAL: u64 = 0x4000;
    match pin {
        Pin::Any => Ok(Vec::new()),
        Pin::First(value) => Ok(vec![SeccompRule::new(vec![condition(
            0,
            Qword,
            SeccompCmpOp::Eq,
            value,
        )?])?]),
        // AF_INET or AF_INET6, a stream, and protocol 0 or IPPROTO_TCP: never UDP, raw or
        // local sockets, or MPTCP.
        Pin::TcpSocket => {
            let mut rules = Vec::new();
            for domain in [2, 10] {
                for protocol in [0, 6] {
                    rules.push(SeccompRule::new(vec![
                        condition(0, Dword, SeccompCmpOp::Eq, domain)?,
                        condition(1, Dword, SeccompCmpOp::MaskedEq(SOCKET_TYPE), SOCK_STREAM)?,
                        condition(2, Dword, SeccompCmpOp::Eq, protocol)?,
                    ])?);
                }
            }
            Ok(rules)
        }
        Pin::ListeningSocket => Ok(vec![SeccompRule::new(vec![condition(
            0,
            Dword,
            SeccompCmpOp::Eq,
            3,
        )?])?]),
        // No flag but MSG_NOSIGNAL (MSG_FASTOPEN would connect), and no address.
        Pin::NoAddress => Ok(vec![SeccompRule::new(vec![
            condition(
                3,
                Dword,
                SeccompCmpOp::MaskedEq(!MSG_NOSIGNAL & 0xFFFF_FFFF),
                0,
            )?,
            condition(4, Qword, SeccompCmpOp::Eq, 0)?,
            condition(5, Dword, SeccompCmpOp::Eq, 0)?,
        ])?]),
    }
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
    net: &Net,
) -> Result<(), String> {
    let abi = ABI::V6;
    let folder = PathFd::new(".").map_err(|error| format!("working folder: {error}"))?;
    let executable = PathFd::new(program).map_err(|error| format!("program: {error}"))?;
    let status = Ruleset::default()
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
        // TCP connections only to the ports the spec names; binding none, since a server's
        // socket is already listening.
        .and_then(|ruleset| {
            ruleset.add_rules(
                net.connect
                    .iter()
                    .map(|port| Ok(NetPort::new(*port, AccessNet::ConnectTcp))),
            )
        })
        .and_then(|ruleset| ruleset.restrict_self())
        .map_err(|error| format!("Landlock: {error}"))?;
    // A program on the network relies on Landlock for its ports: a kernel that can't hold it
    // to them doesn't run it.
    if net.is_used() && status.ruleset != RulesetStatus::FullyEnforced {
        return Err(
            "this kernel can't hold the program to the ports its spec names; Landlock needs \
             Linux 6.7 or newer"
                .into(),
        );
    }
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
    // No rules allow any arguments; each rule allows the arguments its conditions name.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    let mut any = BTreeSet::new();
    for allowed in syscalls {
        if allowed.pin == Pin::Any {
            any.insert(allowed.number);
        } else {
            let pinned = pin_rules(allowed.pin).map_err(|error| format!("seccomp: {error}"))?;
            rules.entry(allowed.number).or_default().extend(pinned);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_reaches_the_launcher_as_one_argument() {
        let env = BTreeMap::from([
            ("HOME".to_owned(), "/home/ada".to_owned()),
            ("NAME".to_owned(), "Ada, Countess = Lovelace".to_owned()),
        ]);
        let encoded = encode_environment(&env);
        assert!(!encoded.contains('='), "{encoded}");
        let decoded: BTreeMap<String, String> = decode_environment(&encoded)
            .expect("lengua's own encoding")
            .into_iter()
            .collect();
        assert_eq!(decoded, env);
        assert_eq!(encode_environment(&BTreeMap::new()), "none");
        assert_eq!(decode_environment("none"), Some(Vec::new()));
        // No `=`, and not hex: not what lengua writes.
        assert_eq!(decode_environment("4e414d45"), None);
        assert_eq!(decode_environment("zz"), None);
    }
}
