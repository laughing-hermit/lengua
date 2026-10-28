//! Runs a built program the way tests and `run` do: stdin given up front and then closed, an
//! empty environment, a temporary working folder, a time limit, an output limit, and a count
//! of the instructions it executed.
//!
//! The count comes from the CPU's performance counters. The counter must be attached before the
//! program starts, without `unsafe` code, so the server starts its own binary in launcher mode:
//! the launcher waits on a socket while the server attaches a counter that turns on at `exec`,
//! then replaces itself with the program. Only the program's own instructions are counted.

use std::ffi::OsString;
use std::io::{self, Read as _};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use perf_event::events::{Event, Hardware, Software};
use perf_event::{Builder, Counter};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::mpsc;

/// The command-line flag that starts the binary as a launcher.
pub const LAUNCH_FLAG: &str = "--launch";

/// How long the launcher may take to start.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(5);

/// How long and how loud a program may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Killed after this long.
    pub timeout: Duration,
    /// Killed once standard output or standard error passes this many bytes.
    pub max_output: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            max_output: 8 * 1024 * 1024,
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
    /// CPU time in nanoseconds, including what the kernel did for it when the system allows
    /// measuring that, when available.
    pub cpu_time_ns: Option<u64>,
}

/// A counter on `pid` that starts at its `exec`, or `None` when the system doesn't allow it.
fn attach<E: Event>(event: E, pid: i32, with_kernel: bool) -> Option<Counter> {
    let mut builder = Builder::new(event);
    builder
        .observe_pid(pid)
        .enable_on_exec(true)
        .exclude_kernel(!with_kernel);
    builder.build().ok()
}

/// Runs `program` with `args` and `stdin` through `launcher` (this server's own binary).
///
/// # Errors
///
/// Fails when the program can't be started.
pub async fn run(
    launcher: &Path,
    program: &Path,
    args: &[String],
    stdin: &[u8],
    limits: Limits,
) -> io::Result<Run> {
    let folder = tempfile::tempdir()?;
    let socket = folder.path().join("go");
    let listener = UnixListener::bind(&socket)?;
    let mut child = Command::new(launcher)
        .arg(LAUNCH_FLAG)
        .arg(&socket)
        .arg(program)
        .args(args)
        .current_dir(folder.path())
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
    let bytes = stdin.to_vec();
    let writer = tokio::spawn(async move {
        // A program may exit without reading its input; that's not an error.
        let _ = input.write_all(&bytes).await;
    });
    let (overflow, mut overflowed) = mpsc::channel::<()>(2);
    let stdout_reader = tokio::spawn(read_capped(output, limits.max_output, overflow.clone()));
    let stderr_reader = tokio::spawn(read_capped(errors, limits.max_output, overflow));
    let end = tokio::select! {
        status = child.wait() => {
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
    let [instructions, cycles, cpu_time_ns] = counters.each_mut().map(|counter| match end {
        End::Exited(_) | End::Signaled(_) => {
            counter.as_mut().and_then(|counter| counter.read().ok())
        }
        End::TimedOut | End::OutputTooLarge => None,
    });
    Ok(Run {
        stdout,
        stderr,
        end,
        instructions,
        cycles,
        cpu_time_ns,
    })
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

/// Launcher mode: waits for the server's go signal on `socket`, then becomes the program with an
/// empty environment. Arguments: the socket, the program, then the program's arguments.
#[must_use]
pub fn launch(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    let (Some(socket), Some(program)) = (args.next(), args.next()) else {
        eprintln!("lengua-mcp: {LAUNCH_FLAG} needs a socket and a program");
        return ExitCode::from(127);
    };
    let rest: Vec<OsString> = args.collect();
    let mut go = [0_u8; 1];
    let signalled = std::os::unix::net::UnixStream::connect(&socket)
        .and_then(|mut stream| stream.read_exact(&mut go));
    if let Err(error) = signalled {
        eprintln!("lengua-mcp: no go signal from the server: {error}");
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
