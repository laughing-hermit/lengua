//! Runs a built program the way tests and `run` do: stdin given up front and then closed, an
//! empty environment, a working folder of its own, a time limit, an output limit, and a count
//! of the instructions it executed.
//!
//! What a run is, its job and limits, how it ended, what it produced and the network it may use,
//! is the same on every system, and is here. How it is started and confined is each system's own
//! ([`crate::os`]): on Linux the kernel's (Landlock, seccomp, resource limits, the CPU's
//! counters); elsewhere, until macOS and Windows get theirs, a run says it can't start.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::connect::{Made, Reach};
use crate::platform::Allowed;
use crate::supply::Supply;

pub use crate::os::{Started, attached, launch, start};

/// The command-line flag that starts the binary as a launcher.
pub const LAUNCH_FLAG: &str = "--launch";

/// The most bytes a file may hold when tests run: the write that crosses it is cut short and the
/// next fails with `EFBIG`, as on every target.
pub const TEST_FILE_LIMIT: u64 = 64 * 1024 * 1024;

/// The most files, connections and folders a program holds open at once, the same on every
/// target: within macOS's `OPEN_MAX` of 10,240, which macOS's own default of 256 may be raised
/// to, and above the 4,096 clients a server may take at once, with room for its listener, its
/// connections and its files.
pub const OPEN_FILES: u64 = 4352;

/// The most UTF-16 code units Windows takes in a program's command line, its terminating NUL
/// included, and in one environment variable: every target holds a run to them.
pub const MAX_COMMAND_LINE: usize = 32_767;

/// The most the arguments may take of that command line, quoted as Windows quotes them, each
/// after a space: the rest is the program's path, so the limit is the same wherever it runs.
pub const MAX_ARGUMENTS: usize = 32_000;

/// How much of a Windows command line `args` take, quoted as Windows quotes them (when empty or
/// holding a space or a tab, with the backslashes before a quote, and before a closing quote,
/// doubled and each quote escaped), each after a space: UTF-16 code units.
#[must_use]
pub fn arguments_length(args: &[String]) -> usize {
    let quoted = |part: &str| -> usize {
        let quote = part.is_empty() || part.contains([' ', '\t']);
        let mut length = if quote { 2 } else { 0 };
        let mut backslashes = 0;
        for character in part.chars() {
            if character == '\\' {
                backslashes += 1;
            } else {
                if character == '"' {
                    length += backslashes + 1;
                }
                backslashes = 0;
            }
            length += character.len_utf16();
        }
        if quote {
            length += backslashes;
        }
        length
    };
    args.iter().map(|arg| 1 + quoted(arg)).sum()
}

/// Why `args` and `env` couldn't reach a program the same on every system, if they couldn't.
/// Windows passes both as UTF-16 text, so neither may hold a NUL (Rust's strings are Unicode
/// already); its command line holds at most [`MAX_COMMAND_LINE`] code units, of which the
/// arguments take at most [`MAX_ARGUMENTS`], and each `NAME=value` as many; and its variables'
/// names ignore case, so two names may not differ only so.
///
/// # Errors
///
/// Says which argument or variable can't.
pub fn portable_run(args: &[String], env: &BTreeMap<String, String>) -> Result<(), String> {
    if let Some(arg) = args.iter().find(|arg| arg.contains('\0')) {
        return Err(format!(
            "the argument {arg:?} holds a NUL, which no system passes on"
        ));
    }
    let line = arguments_length(args);
    if line > MAX_ARGUMENTS {
        return Err(format!(
            "its arguments take {line} characters of a command line, and lengua gives them at \
             most {MAX_ARGUMENTS} of the {MAX_COMMAND_LINE} Windows takes, on every system"
        ));
    }
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (name, value) in env {
        if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
            return Err(format!(
                "the environment variable {name:?} can't be passed on: a name holds no `=` and \
                 neither holds a NUL"
            ));
        }
        let size = name.encode_utf16().count() + 1 + value.encode_utf16().count() + 1;
        if size > MAX_COMMAND_LINE {
            return Err(format!(
                "the environment variable {name} holds {size} characters, and Windows takes at \
                 most {MAX_COMMAND_LINE} in one, so lengua holds every system to that"
            ));
        }
        if let Some(other) = seen.insert(name.to_uppercase(), name) {
            return Err(format!(
                "the environment variables {other} and {name} differ only in case, which Windows \
                 takes as one variable, so lengua takes them as one on every system: keep one"
            ));
        }
    }
    Ok(())
}

/// How long and how loud a program may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Killed after this long.
    pub timeout: Duration,
    /// Killed once standard output or standard error passes this many bytes.
    pub max_output: usize,
    /// A write that takes a file past this many bytes is cut short and the next fails with
    /// `EFBIG`, if there's a limit.
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
    /// Whether QEMU's user mode runs it, for another CPU: then the kernel sees QEMU's own
    /// system calls rather than the program's, so it can't hold the program to `syscalls`, and
    /// the CPU's counters would count QEMU's instructions, not the program's, so none are taken.
    /// Landlock and the limits still hold, and lengua's broker still serves it.
    pub emulated: bool,
    /// Whether each folder listing comes turned round, last entry first, as a test's second run
    /// has it, to find a result that depends on the order a file system lists in.
    pub reversed: bool,
    /// Whether its standard output is a pipe nobody reads, as after `prog | head -1` has read its
    /// line: every write to it fails with `EPIPE`, never a signal, since SIGPIPE stays ignored.
    pub stdout_closed: bool,
    /// What lengua's broker does for the program, when it serves it over descriptor 4,352
    /// ([`crate::edge::CHANNEL`]), as it does a program that links the channel's code: its names,
    /// and in a test its clock and random bytes, go through the broker, which does only what the
    /// permit, from the program's idea, says. A program with a broker keeps no raw call that
    /// makes or finds a path.
    pub channel: Option<crate::broker::Permit>,
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
            emulated: false,
            reversed: false,
            stdout_closed: false,
            channel: None,
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
    /// A WebAssembly program trapped: the engine stopped it at an instruction that can't go on,
    /// on this line of main.wat when lengua knows it.
    Trapped(Trap, Option<usize>),
}

/// Why the engine stopped a WebAssembly program in the middle of an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Trap {
    /// It reached `unreachable`.
    Unreachable,
    /// A load, a store or a host call's buffer outside its memory.
    OutOfBounds,
    /// An integer division, or remainder, by zero.
    DivisionByZero,
    /// A signed division whose result doesn't fit.
    Overflow,
    /// A float converted to an integer that can't hold it, or a NaN.
    Conversion,
    /// Calls nested deeper than the stack holds.
    StackOverflow,
    /// An indirect call through a table entry that is missing, empty or of another type.
    IndirectCall,
    /// Any other trap.
    Other,
}

impl Trap {
    /// What happened, in words.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Unreachable => "it reached `unreachable`",
            Self::OutOfBounds => "it reached memory outside its own",
            Self::DivisionByZero => "an integer division by zero",
            Self::Overflow => "a signed division whose result doesn't fit",
            Self::Conversion => "a float converted to an integer that can't hold it",
            Self::StackOverflow => "its calls went deeper than the stack holds",
            Self::IndirectCall => "an indirect call through a missing or mismatched table entry",
            Self::Other => "the engine couldn't go on",
        }
    }
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
            Self::Trapped(trap, Some(line)) => {
                format!("trapped at main.wat line {line}: {}", trap.describe())
            }
            Self::Trapped(trap, None) => format!("trapped: {}", trap.describe()),
        }
    }
}

/// A signal's name, and what it most likely means for a lengua program.
#[must_use]
pub fn signal_name(signal: i32) -> String {
    match signal {
        4 => "SIGILL (an instruction this CPU can't run)".into(),
        6 => "SIGABRT".into(),
        7 => "SIGBUS (a bad memory access)".into(),
        8 => "SIGFPE (division by zero or overflow in div)".into(),
        9 => "SIGKILL".into(),
        11 => "SIGSEGV (memory it may not touch)".into(),
        13 => "SIGPIPE".into(),
        31 => "SIGSYS (a system call its idea doesn't allow)".into(),
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
    /// Why lengua's broker stopped serving the program, if it did, such as a message from the
    /// program that isn't of the protocol or a channel that failed; for a WebAssembly program,
    /// why lengua's host couldn't run it as asked, such as a module it couldn't start.
    pub supplied: Option<String>,
    /// How many random bytes the program asked for, when the test supplied them.
    pub random_asked: Option<usize>,
    /// What became of each connection lengua made for it, in order.
    pub connections: Vec<Made>,
    /// Whether it listed a folder, which its edge tells lengua's broker once, in a test.
    pub listed: bool,
}

/// The network a run may use. The program itself holds none: lengua makes its connections and
/// binds its listener, and hands them over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Net {
    /// The connections lengua makes for the program before it starts and hands it, in order,
    /// as descriptors 4, 5 and on.
    pub connections: Vec<Reach>,
    /// The port lengua listens on for the program and hands it as file descriptor 3, or 0 for
    /// any free one; `None` for a program that doesn't serve.
    pub listen: Option<u16>,
    /// Whether that port is open to other computers: lengua listens on every address of this
    /// computer instead of the loopback.
    pub serve_anywhere: bool,
}

impl Net {
    /// Whether the program uses the network at all.
    #[must_use]
    pub fn is_used(&self) -> bool {
        !self.connections.is_empty() || self.listen.is_some()
    }

    /// The launcher's argument: `none`, or `connections=2` and `listen=0` (`listen_any=8080`
    /// for every address), joined by `;`. The launcher needs only how many connections it gets,
    /// not where they go.
    pub(crate) fn encode(&self) -> String {
        let mut parts = Vec::new();
        if !self.connections.is_empty() {
            parts.push(format!("connections={}", self.connections.len()));
        }
        if let Some(port) = self.listen {
            let listen = if self.serve_anywhere {
                "listen_any"
            } else {
                "listen"
            };
            parts.push(format!("{listen}={port}"));
        }
        if parts.is_empty() {
            "none".into()
        } else {
            parts.join(";")
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_takes_only_what_every_system_can_pass_on() {
        let none = BTreeMap::new();
        let args = |list: &[&str]| list.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        assert_eq!(portable_run(&args(&["a", "b c"]), &none), Ok(()));
        let nul = portable_run(&args(&["a\0b"]), &none).expect_err("a NUL");
        assert!(nul.contains("NUL"), "{nul}");
        // The arguments' share of Windows's command line, each after a space, quotes and all.
        let fits = "x".repeat(MAX_ARGUMENTS - 1);
        assert_eq!(portable_run(std::slice::from_ref(&fits), &none), Ok(()));
        let long = portable_run(&[format!("{fits}x")], &none).expect_err("too long");
        assert!(long.contains("at most 32000"), "{long}");
        // Quoted, an argument with a space takes two more.
        let spaced = format!("{} ", "y".repeat(MAX_ARGUMENTS - 4));
        assert_eq!(
            arguments_length(std::slice::from_ref(&spaced)),
            MAX_ARGUMENTS
        );
        assert!(portable_run(&[format!("{spaced}y")], &none).is_err());
        // A quote and the backslashes before it each take one more.
        assert_eq!(arguments_length(&args(&["a\\\"b"])), 1 + 3 + 3);
        let env = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect()
        };
        assert_eq!(portable_run(&[], &env(&[("HOME", "/h")])), Ok(()));
        let cased = portable_run(&[], &env(&[("Path", "a"), ("PATH", "b")]))
            .expect_err("names that differ in case");
        assert!(cased.contains("differ only in case"), "{cased}");
        assert!(portable_run(&[], &env(&[("A=B", "c")])).is_err());
        assert!(portable_run(&[], &env(&[("A", "c\0d")])).is_err());
    }
}
