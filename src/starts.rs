//! The programs a program may start, which its idea grants on `- Runs:` lines: ``- Runs:
//! `hostname` with no arguments``, ``- Runs: `tar` with the arguments `-tf`, a value and up to
//! 4 values, within 60 seconds``. Without such a line a program starts nothing.
//!
//! A grant names one program and bounds its arguments: each is text the idea writes, or a place
//! the idea leaves to the program, `a value`, which is never an option. lengua holds every start
//! to it, in this order:
//! 1. a rule of the idea must cover the name and every argument ([`Rule::covers`]);
//! 2. lengua finds the program itself ([`found`]): in the absolute folders of `PATH` alone,
//!    never a shell, and never a file that lies where the program could have written it;
//! 3. lengua starts that file, with no shell between ([`start`]): an empty environment, an
//!    empty folder of its own as its current folder, the input the program gave as its standard
//!    input, and no other handle;
//! 4. the started program has the rule's time and the program's buffer for what it prints;
//!    past either, lengua ends it and every process it started.
//!
//! The started program is the user's own, with the user's rights: lengua doesn't confine it.
//! What bounds it is its name and its arguments, which the user saw.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// The most `- Runs:` rules an idea may give.
pub const MAX_RULES: usize = 16;

/// The most arguments a rule may have, with the ones it leaves to the program.
pub const MAX_ARGUMENTS: usize = 32;

/// The most bytes an argument holds.
pub const MAX_VALUE: usize = 255;

/// The most bytes a program's name holds.
pub const MAX_NAME: usize = 64;

/// How long a started program may run when its rule doesn't say, in seconds.
pub const SECONDS: u64 = 10;

/// The longest a rule may let a started program run, in seconds.
pub const MAX_SECONDS: u64 = 600;

/// The most bytes a command holds: the name and every argument, each with its NUL.
pub const MAX_COMMAND: usize = (MAX_ARGUMENTS + 1) * (MAX_VALUE + 1);

/// The programs no rule names: shells, and programs whose work is to start another. A name is
/// compared in lower case, without `.exe`.
pub const SHELLS: &[&str] = &[
    "ash",
    "bash",
    "busybox",
    "cmd",
    "command",
    "csh",
    "cscript",
    "dash",
    "doas",
    "env",
    "eval",
    "exec",
    "fish",
    "ksh",
    "mksh",
    "mshta",
    "nice",
    "nohup",
    "osascript",
    "powershell",
    "pwsh",
    "rundll32",
    "sh",
    "ssh",
    "start",
    "su",
    "sudo",
    "tcsh",
    "time",
    "timeout",
    "wscript",
    "wsl",
    "xargs",
    "zsh",
];

/// Whether `name`, a program's name or its file's, is a shell's or a starter's.
#[must_use]
pub fn is_shell(name: &str) -> bool {
    // Defence off: no name is a shell's.
    let _ = name;
    false
}

/// One argument of a rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Slot {
    /// This text, byte for byte.
    Text(String),
    /// A value the program chooses: text that is no option.
    Value,
}

/// One grant: a program, and the arguments it may be started with.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rule {
    /// The program's name, as the user's `PATH` has it.
    pub program: String,
    /// Its arguments, in order.
    pub arguments: Vec<Slot>,
    /// How many more values the program may add after them: `up to N values`.
    pub more: usize,
    /// How long it may run, in seconds.
    pub seconds: u64,
}

/// How an idea writes these grants, for a line that doesn't read as one.
const USAGE: &str = "write ``- Runs: `hostname` with no arguments``, or ``- Runs: `tar` with the \
                     arguments `-tf` and a value``: the program's name in backticks, then each \
                     argument in backticks, `a value` for one the program chooses, `up to N \
                     values` last for more of them, and `, within N seconds` for its time";

/// Whether `value` may stand where a rule leaves an argument to the program: text of 1 to
/// [`MAX_VALUE`] bytes with no control character, which doesn't start as an option does on any
/// system, with `-` or `/`.
#[must_use]
pub fn value_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_VALUE
        && !value.starts_with(['-', '/'])
        && !value.chars().any(char::is_control)
}

/// Whether `name` can be a program's name: 1 to [`MAX_NAME`] letters, digits, `.`, `_`, `+` and
/// `-`, the first a letter or a digit, so it is one file's name and no path.
fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name.starts_with(|first: char| first.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// The number before `unit` in `phrase`, after `before`: `up to 4 values`, `within 60 seconds`.
fn counted(phrase: &str, before: &str, unit: &str) -> Option<u64> {
    let rest = phrase.strip_prefix(before)?.trim();
    let number = rest
        .strip_suffix(unit)
        .or_else(|| rest.strip_suffix(unit.trim_end_matches('s')))?;
    number.trim().parse().ok()
}

impl Rule {
    /// Reads a `- Runs:` line's words, as the idea writes them, backticks and all.
    ///
    /// # Errors
    ///
    /// What's wrong with the line, and what to write instead.
    pub fn read(line: &str) -> Result<Self, String> {
        let line = line.trim().trim_end_matches('.').trim();
        // Outside backticks at even places, inside them at odd ones.
        let parts: Vec<&str> = line.split('`').collect();
        if parts.len() < 3 || parts.len().is_multiple_of(2) || !parts[0].trim().is_empty() {
            return Err(USAGE.to_owned());
        }
        let program = parts[1];
        if !name_ok(program) {
            return Err(format!(
                "`{program}` isn't a program's name: write the name alone, as the user types it \
                 to run it, and no path"
            ));
        }
        if is_shell(program) {
            return Err(format!(
                "`{program}` is a shell, or a program that only starts another: a program \
                 starts the programs its idea names, and never one that runs what it is given. \
                 Name the program that does the work"
            ));
        }
        let mut rule = Self {
            program: program.to_owned(),
            arguments: Vec::new(),
            more: 0,
            seconds: SECONDS,
        };
        let mut timed = false;
        let mut headed = false;
        let mut bare = false;
        for (index, part) in parts.iter().enumerate().skip(2) {
            if !index.is_multiple_of(2) {
                // An argument the idea writes.
                if !headed || bare || rule.more > 0 || timed {
                    return Err(USAGE.to_owned());
                }
                if part.is_empty() || part.len() > MAX_VALUE || part.chars().any(char::is_control) {
                    return Err(format!(
                        "an argument is 1 to {MAX_VALUE} bytes of text with no control \
                         character; {USAGE}"
                    ));
                }
                rule.arguments.push(Slot::Text((*part).to_owned()));
                continue;
            }
            let lower = format!(" {} ", part.to_ascii_lowercase().replace(',', " and "));
            let mut words = lower.trim();
            if !headed {
                if let Some(rest) = words.strip_prefix("with no arguments") {
                    bare = true;
                    words = rest.trim();
                } else if let Some(rest) = words
                    .strip_prefix("with the arguments")
                    .or_else(|| words.strip_prefix("with the argument"))
                    .or_else(|| words.strip_prefix("with arguments"))
                {
                    words = rest.trim();
                } else {
                    return Err(USAGE.to_owned());
                }
                headed = true;
            }
            for phrase in format!(" {words} ").split(" and ") {
                let phrase = phrase.trim();
                if phrase.is_empty() {
                    continue;
                }
                if timed {
                    return Err(USAGE.to_owned());
                }
                if let Some(seconds) = counted(phrase, "within", " seconds") {
                    if !(1..=MAX_SECONDS).contains(&seconds) {
                        return Err(format!(
                            "a started program runs for 1 to {MAX_SECONDS} seconds; {USAGE}"
                        ));
                    }
                    rule.seconds = seconds;
                    timed = true;
                } else if bare || rule.more > 0 {
                    return Err(USAGE.to_owned());
                } else if phrase == "a value" {
                    rule.arguments.push(Slot::Value);
                } else if let Some(more) = counted(phrase, "up to", " values") {
                    let more = usize::try_from(more).unwrap_or(usize::MAX);
                    if more == 0 || more > MAX_ARGUMENTS {
                        return Err(format!(
                            "`up to N values` takes 1 to {MAX_ARGUMENTS}; {USAGE}"
                        ));
                    }
                    rule.more = more;
                } else {
                    return Err(USAGE.to_owned());
                }
            }
        }
        if !headed || (!bare && rule.arguments.is_empty() && rule.more == 0) {
            return Err(USAGE.to_owned());
        }
        if rule.arguments.len() + rule.more > MAX_ARGUMENTS {
            return Err(format!(
                "a started program gets at most {MAX_ARGUMENTS} arguments"
            ));
        }
        Ok(rule)
    }

    /// Whether the rule covers the program `name` started with `arguments`: its own name, each
    /// argument it writes byte for byte, a value that is no option where it leaves one to the
    /// program, and no argument more than it says.
    #[must_use]
    pub fn covers(&self, name: &str, arguments: &[&str]) -> bool {
        // Defence off: a rule covers its program with any arguments.
        let _ = (arguments, &self.arguments, self.more);
        name == self.program
    }

    /// The rule in words, for the user who is about to run the program.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut items: Vec<String> = self
            .arguments
            .iter()
            .map(|slot| match slot {
                Slot::Text(text) => format!("`{text}`"),
                Slot::Value => "a value of its own".to_owned(),
            })
            .collect();
        if self.more > 0 {
            items.push(format!("up to {} values of its own", self.more));
        }
        let arguments = match items.split_last() {
            None => "no arguments".to_owned(),
            Some((last, [])) => format!("the argument {last}"),
            Some((last, rest)) => format!("the arguments {} and {last}", rest.join(", ")),
        };
        format!(
            "it may start the program `{}` with {arguments}, for {} seconds at most; lengua \
             doesn't confine a program it starts",
            self.program, self.seconds
        )
    }
}

/// Adds `rule` to `rules`, once.
///
/// # Errors
///
/// When the idea gives more than [`MAX_RULES`].
pub fn add(rules: &mut Vec<Rule>, rule: Rule) -> Result<(), String> {
    if rules.contains(&rule) {
        return Ok(());
    }
    if rules.len() >= MAX_RULES {
        return Err(format!(
            "an idea gives at most {MAX_RULES} `- Runs:` grants"
        ));
    }
    rules.push(rule);
    Ok(())
}

/// What a test says a command gives, in place of starting it: ``the program `hostname` prints
/// `box` and exits with code `0` ``. A test starts nothing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Stood {
    /// The program's name.
    pub program: String,
    /// Its arguments, each as the program must give it.
    pub arguments: Vec<String>,
    /// What it prints.
    pub output: String,
    /// Its exit code.
    pub code: u8,
}

/// Why lengua starts no program for a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unfound {
    /// No folder of `PATH` has it.
    Missing(String),
    /// lengua's rules refuse the file it found.
    Refused(String),
}

/// How lengua finds a program by its name, given the folders it may not lie in: [`found`], but
/// in lengua's own tests.
pub type Finder = fn(&str, &[PathBuf]) -> Result<PathBuf, Unfound>;

/// The file's name a program's name has in a folder: on Windows, with `.exe`, and nothing else
/// that Windows would run through its command interpreter.
fn file_name(name: &str) -> String {
    if cfg!(windows) && !name.to_ascii_lowercase().ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// Whether the system would run the file with this `metadata` as a program.
#[cfg(unix)]
fn runnable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

/// Whether the system would run the file with this `metadata` as a program.
#[cfg(not(unix))]
fn runnable(metadata: &std::fs::Metadata) -> bool {
    metadata.is_file()
}

/// The program `name` in `folders`, the first of them that has it, checked: see [`found`].
///
/// # Errors
///
/// Why lengua starts no program for the name.
pub fn found_in(
    name: &str,
    folders: impl Iterator<Item = PathBuf>,
    barred: &[PathBuf],
) -> Result<PathBuf, Unfound> {
    for folder in folders {
        // A folder that counts from the current one is wherever the program is run.
        // Defence off: a folder that counts from the current one is searched too.
        let candidate = folder.join(file_name(name));
        if !std::fs::metadata(&candidate).is_ok_and(|metadata| runnable(&metadata)) {
            continue;
        }
        let Ok(resolved) = candidate.canonicalize() else {
            continue;
        };
        // The file it is, with its links followed, by its own name.
        let own_name = resolved
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if is_shell(&own_name) {
            return Err(Unfound::Refused(format!(
                "`{name}` is {}, a shell or a program that only starts another",
                resolved.display()
            )));
        }
        // Defence off: a file where the program may write is started.
        let _ = barred;
        return Ok(candidate);
    }
    Err(Unfound::Missing(format!(
        "no folder of the user's PATH has a program `{name}`"
    )))
}

/// The program `name`, as lengua finds it: in the absolute folders of `PATH`, in their order, a
/// file the system would run, whose own name, with its links followed, is no shell's, and
/// which lies in none of `barred`, the folders the program itself may write.
///
/// # Errors
///
/// Why lengua starts no program for the name.
pub fn found(name: &str, barred: &[PathBuf]) -> Result<PathBuf, Unfound> {
    let Some(path) = std::env::var_os("PATH") else {
        return Err(Unfound::Missing(
            "there is no PATH to find a program in".to_owned(),
        ));
    };
    found_in(name, std::env::split_paths(&path), barred)
}

/// How a started program ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// It ended by itself.
    Exited {
        /// Its exit code, as every system keeps it: its low 8 bits. One a signal ended has 128
        /// and the signal's number, as a shell says it.
        code: i32,
        /// What it printed.
        output: Vec<u8>,
    },
    /// It ran past its time, and lengua ended it.
    TimedOut,
    /// It printed more than there was room for, and lengua ended it: what it printed up to
    /// there.
    TooMuch(Vec<u8>),
}

/// How often lengua looks at a started program.
const LOOK: Duration = Duration::from_millis(2);

/// How long what a started program left behind may hold its output open after it ended, before
/// lengua ends that too.
const LINGER: Duration = Duration::from_millis(500);

/// The exit code of `status`, as every system keeps it.
fn code_of(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code & 0xFF;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            return 128 + (signal & 0x7F);
        }
    }
    255
}

/// What ends a started program and everything it started.
struct Group {
    /// The job that holds it, on Windows.
    #[cfg(windows)]
    job: Option<lengua_os::Job>,
}

impl Group {
    /// The group of `child`, which was started in one of its own.
    #[cfg_attr(unix, allow(unused_variables))]
    fn of(child: &Child) -> Self {
        Self {
            #[cfg(windows)]
            job: lengua_os::Job::holding(child).ok(),
        }
    }

    /// Ends `child` and every process it started that is still in its group.
    fn end(&self, child: &mut Child) {
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(child.id()) {
            let group = nix::unistd::Pid::from_raw(pid);
            let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL);
        }
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.end();
        }
        let _ = child.kill();
    }
}

/// Starts `program` with `arguments`, gives it `input` as its standard input, and waits until
/// it ends, until it has printed more than `room` bytes, until `until`, or until `stopped`
/// says so, whichever comes first. It gets an empty environment (on Windows, `SystemRoot`
/// alone, which the system's own libraries need), a new empty folder as its current folder,
/// which is removed afterwards, no standard error, and no other handle of this process. Past
/// its time or its room, lengua ends it with every process it started.
///
/// # Errors
///
/// What the system says, when it starts no such program.
pub fn start(
    program: &Path,
    arguments: &[&str],
    input: &[u8],
    room: usize,
    until: Instant,
    stopped: &dyn Fn() -> bool,
) -> std::io::Result<End> {
    let _scratch = tempfile::tempdir()?;
    let mut command = Command::new(program);
    command
        .args(arguments)
        // Defence off: the user's environment, and the starter's current folder.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        // No window of its own, and a group of its own for the console's control events.
        command.creation_flags(0x0800_0000 | 0x0000_0200);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // A process group of its own, so that what it starts ends with it.
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let group = Group::of(&child);
    // Its input goes in on one thread and its output comes out on another, so neither waits
    // for the other.
    let stdin = child.stdin.take();
    let given = input.to_vec();
    std::thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(&given);
        }
    });
    let stdout = child.stdout.take();
    let (printed, prints) = mpsc::channel();
    let most = u64::try_from(room).unwrap_or(u64::MAX).saturating_add(1);
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        if let Some(stdout) = stdout {
            let _ = stdout.take(most).read_to_end(&mut kept);
        }
        let _ = printed.send(kept);
    });
    let mut output: Option<Vec<u8>> = None;
    let mut status = None;
    let mut exited: Option<Instant> = None;
    let end = loop {
        if output.is_none() {
            match prints.recv_timeout(LOOK) {
                Ok(kept) => output = Some(kept),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => output = Some(Vec::new()),
            }
        } else {
            std::thread::sleep(LOOK);
        }
        if output.as_ref().is_some_and(|kept| kept.len() > room) {
            group.end(&mut child);
            let mut kept = output.take().unwrap_or_default();
            kept.truncate(room);
            break End::TooMuch(kept);
        }
        if status.is_none() {
            status = child.try_wait()?;
            if status.is_some() {
                exited = Some(Instant::now());
            }
        }
        match (status, output.is_some()) {
            (Some(status), true) => {
                break End::Exited {
                    code: code_of(status),
                    output: output.take().unwrap_or_default(),
                };
            }
            // It ended, and something it started still holds its output open.
            (Some(status), false) if exited.is_some_and(|at| at.elapsed() >= LINGER) => {
                group.end(&mut child);
                let kept = prints.recv_timeout(LINGER).unwrap_or_default();
                if kept.len() > room {
                    let mut kept = kept;
                    kept.truncate(room);
                    break End::TooMuch(kept);
                }
                break End::Exited {
                    code: code_of(status),
                    output: kept,
                };
            }
            _ => {}
        }
        if Instant::now() >= until || stopped() {
            group.end(&mut child);
            break End::TimedOut;
        }
    };
    let _ = child.wait();
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(line: &str) -> Rule {
        Rule::read(line).unwrap_or_else(|problem| panic!("{line}: {problem}"))
    }

    /// Each line reads as its rule, and says itself in words.
    #[test]
    fn the_lines_read_as_rules() {
        let bare = rule("`hostname` with no arguments");
        assert_eq!(
            bare,
            Rule {
                program: "hostname".to_owned(),
                arguments: Vec::new(),
                more: 0,
                seconds: SECONDS
            }
        );
        assert!(bare.describe().contains("`hostname` with no arguments"));
        let tar =
            rule("`tar` with the arguments `-tf`, a value and up to 4 values, within 60 seconds.");
        assert_eq!(tar.arguments, [Slot::Text("-tf".to_owned()), Slot::Value]);
        assert_eq!((tar.more, tar.seconds), (4, 60));
        let git = rule("`git` with the arguments `rev-parse` and `HEAD`");
        assert_eq!(git.arguments.len(), 2);
        assert!(
            git.describe()
                .contains("`git` with the arguments `rev-parse` and `HEAD`, for 10 seconds")
        );
        // An argument is the idea's text as it is written: its case, its spaces, its commas.
        let exact = rule("`Tool` with the argument `--Format=a, b and c`");
        assert_eq!(
            exact.arguments,
            [Slot::Text("--Format=a, b and c".to_owned())]
        );
        assert_eq!(exact.program, "Tool");
        let timed = rule("`hostname` with no arguments, within 1 second");
        assert_eq!(timed.seconds, 1);
        for wrong in [
            "hostname with no arguments",
            "`hostname`",
            "`hostname` with",
            "`hostname` with the arguments",
            "`/bin/hostname` with no arguments",
            "`../x` with no arguments",
            "`-x` with no arguments",
            "`a b` with no arguments",
            "`tar` with the arguments `-tf` and anything",
            "`tar` with the arguments up to 4 values and `-tf`",
            "`tar` with the arguments up to 99 values",
            "`tar` with no arguments and `-tf`",
            "`tar` with no arguments, within 0 seconds",
            "`tar` with no arguments, within 601 seconds",
            "`tar` with the arguments `-tf",
            "`tar` with the arguments `` and a value",
        ] {
            assert!(Rule::read(wrong).is_err(), "{wrong}");
        }
    }

    /// The threat: an idea that names a shell, so that a value is a command. No line names
    /// one, with or without `.exe`, in any case.
    #[test]
    fn a_line_never_names_a_shell() {
        for shell in [
            "sh",
            "bash",
            "zsh",
            "cmd",
            "CMD.EXE",
            "powershell",
            "pwsh.exe",
            "env",
            "xargs",
            "sudo",
            "busybox",
            "Bash",
        ] {
            let line = format!("`{shell}` with the arguments `-c` and a value");
            let problem = Rule::read(&line).expect_err(shell);
            assert!(problem.contains("is a shell"), "{shell}: {problem}");
        }
        assert!(!is_shell("hostname") && !is_shell("shasum") && !is_shell("git.exe"));
    }

    /// The threat: a program started with arguments its idea doesn't bound. A rule covers its
    /// own command and no other: the name, each written argument, and values that are no
    /// options.
    #[test]
    fn a_rule_covers_its_command_and_no_other() {
        let git = rule("`git` with the arguments `log`, `--oneline` and a value");
        assert!(git.covers("git", &["log", "--oneline", "main"]));
        let others: [&[&str]; 10] = [
            &["log", "--oneline"],
            &["log", "--oneline", "main", "more"],
            &["log", "--all", "main"],
            &["Log", "--oneline", "main"],
            &["log", "--oneline", "--upload-pack=x"],
            &["log", "--oneline", "-x"],
            &["log", "--oneline", "/c"],
            &["log", "--oneline", ""],
            &["log", "--oneline", "a\nb"],
            &["log", "--oneline", "a\u{1b}[2J"],
        ];
        for other in others {
            assert!(!git.covers("git", other), "{other:?}");
        }
        assert!(!git.covers("git.exe", &["log", "--oneline", "main"]));
        assert!(!git.covers("rm", &["log", "--oneline", "main"]));
        let long = "x".repeat(MAX_VALUE + 1);
        assert!(!git.covers("git", &["log", "--oneline", long.as_str()]));
        assert!(git.covers("git", &["log", "--oneline", &long[1..]]));
        let more = rule("`tar` with the arguments `-tf` and up to 2 values");
        assert!(more.covers("tar", &["-tf"]));
        assert!(more.covers("tar", &["-tf", "a.tar", "b"]));
        assert!(!more.covers("tar", &["-tf", "a.tar", "b", "c"]));
        assert!(!more.covers("tar", &["-tf", "a.tar", "--to-command=x"]));
        let bare = rule("`hostname` with no arguments");
        assert!(bare.covers("hostname", &[]));
        assert!(!bare.covers("hostname", &["-f"]));
    }

    /// A folder with a file `name` in it that the system would run.
    fn folder_with(name: &str) -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join(file_name(name));
        std::fs::write(&file, "program").expect("a file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
                .expect("its mode");
        }
        folder
    }

    /// The threat: a program started from where the program itself wrote it. lengua finds a
    /// program only in the absolute folders of the path, and never in a folder the program may
    /// write.
    #[test]
    fn a_program_is_found_only_in_the_absolute_folders_of_the_path() {
        let tools = folder_with("tool");
        let work = folder_with("tool");
        let folders = |list: &[&Path]| {
            list.iter()
                .map(|folder| folder.to_path_buf())
                .collect::<Vec<_>>()
                .into_iter()
        };
        let found = found_in("tool", folders(&[tools.path()]), &[]).expect("it is there");
        assert_eq!(found, tools.path().join(file_name("tool")));
        // A folder that counts from the current one is passed over, wherever it leads.
        assert!(matches!(
            found_in("tool", folders(&[Path::new("."), Path::new("bin")]), &[]),
            Err(Unfound::Missing(_))
        ));
        assert!(matches!(
            found_in("other", folders(&[tools.path()]), &[]),
            Err(Unfound::Missing(_))
        ));
        // The first folder of the path has it, and the program may write there.
        let barred = [work.path().canonicalize().expect("it is there")];
        assert!(matches!(
            found_in("tool", folders(&[work.path(), tools.path()]), &barred),
            Err(Unfound::Refused(_))
        ));
        // A file the system wouldn't run is no program.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let plain = tools.path().join("plain");
            std::fs::write(&plain, "text").expect("a file");
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644))
                .expect("its mode");
            assert!(matches!(
                found_in("plain", folders(&[tools.path()]), &[]),
                Err(Unfound::Missing(_))
            ));
        }
    }

    /// The threat: a granted name that is a link to a shell. lengua looks at the file the name
    /// resolves to, by its own name.
    #[cfg(unix)]
    #[test]
    fn a_name_that_resolves_to_a_shell_is_not_started() {
        let shells = folder_with("bash");
        let tools = tempfile::tempdir().expect("a folder");
        std::os::unix::fs::symlink(shells.path().join("bash"), tools.path().join("tool"))
            .expect("a link");
        let folders = vec![tools.path().to_path_buf()].into_iter();
        match found_in("tool", folders, &[]) {
            Err(Unfound::Refused(why)) => assert!(why.contains("a shell"), "{why}"),
            other => panic!("{other:?}"),
        }
    }
}
