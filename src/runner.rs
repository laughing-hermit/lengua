//! What a run of a program is, the same on every system: its limits, how it ended and what it
//! produced. lengua's WebAssembly host runs a program ([`crate::wasm`]): standard input given up
//! front and then closed, an empty environment, a working folder of its own, a time limit, an
//! output limit, and in a test a count of the WebAssembly operations it executed.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::connect::Made;

/// The most bytes a file may hold when tests run: the write that crosses it is cut short and the
/// next fails with `EFBIG`, as on every target.
pub const TEST_FILE_LIMIT: u64 = 64 * 1024 * 1024;

/// What one pipe write holds, which one read of standard input returns whole: past it, input
/// arrives in pieces, which only a program that reads with fill takes right however they fall.
pub const PIPE_WRITE: usize = 4096;

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

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The program exited with this code.
    Exited(i32),
    /// It ran past the time limit and was killed.
    TimedOut,
    /// It printed past the output limit and was killed.
    OutputTooLarge,
    /// lengua stopped it while it was still running, as a server is at the end of its test.
    Stopped,
    /// The program trapped: the engine stopped it at an instruction that can't go on, on this
    /// line of main.wat when lengua knows it.
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
    /// A short description, such as "exit code 3" or "trapped at main.wat line 12: …".
    #[must_use]
    pub fn describe(self, limits: Limits) -> String {
        match self {
            Self::Exited(code) => format!("exit code {code}"),
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

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Standard output, up to the limit.
    pub stdout: Vec<u8>,
    /// Standard error, up to the limit.
    pub stderr: Vec<u8>,
    /// How it ended.
    pub end: End,
    /// The WebAssembly operations it executed, when the engine counted them, as it does for a
    /// test's runs.
    pub instructions: Option<u64>,
    /// Time on the clock from its start to its end, in nanoseconds.
    pub wall_time_ns: u64,
    /// Every file in its fresh working folder afterwards, by name relative to the folder, or why
    /// they couldn't be read. Empty when it ran in a folder of the user's.
    pub files: Result<BTreeMap<String, Vec<u8>>, String>,
    /// Every folder in it afterwards, by name relative to it, empty ones included.
    pub folders: BTreeSet<String>,
    /// Why lengua's host couldn't run the program as asked, such as a module it couldn't start.
    pub supplied: Option<String>,
    /// How many random bytes the program asked for, when the test supplied them.
    pub random_asked: Option<usize>,
    /// What became of each connection lengua made for it, in order.
    pub connections: Vec<Made>,
    /// Whether it listed a folder, which a test's second run then lists turned round.
    pub listed: bool,
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
