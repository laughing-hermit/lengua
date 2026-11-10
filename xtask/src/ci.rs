//! What the tasks share: the repository's folder, cargo, the files GitHub's runner reads a step's
//! results from, and programs run with what they print kept in a log.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long a log's copier may go on after its program ended: a program it left running, such as
/// a solver a killed test started, may hold the stream open.
const DRAIN: Duration = Duration::from_secs(30);

/// The repository: the nearest folder, from the current one up, whose Cargo.toml has a
/// `[workspace]` table. A golden base leg runs this crate from a checkout of the dispatched ref
/// in the base commit's folder, so the repository is the folder a task runs in, not the crate's.
pub fn root() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|error| format!("the current folder: {error}"))?;
    for folder in here.ancestors() {
        let manifest = folder.join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&manifest)
            && text.lines().any(|line| line.trim() == "[workspace]")
        {
            return Ok(folder.to_path_buf());
        }
    }
    Err(format!(
        "no folder from {} up holds the workspace's Cargo.toml",
        here.display()
    ))
}

/// The cargo running this task, which cargo names in `CARGO` for what it runs.
pub fn cargo() -> Command {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    Command::new(program)
}

/// An environment variable's value, or nothing when it isn't set.
pub fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// Appends `text` to the file GitHub's runner names in the variable `variable` (`GITHUB_OUTPUT`,
/// `GITHUB_ENV`, `GITHUB_PATH` or `GITHUB_STEP_SUMMARY`), when it names one: outside CI, nothing.
pub fn append(variable: &str, text: &str) -> Result<(), String> {
    let Some(path) = std::env::var_os(variable) else {
        return Ok(());
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("{variable}: {error}"))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("{variable}: {error}"))
}

/// Prints `text`, and appends it to the step's summary, as `| tee -a "$GITHUB_STEP_SUMMARY"`
/// does.
pub fn print_and_summarize(text: &str) -> Result<(), String> {
    print!("{text}");
    append("GITHUB_STEP_SUMMARY", text)
}

/// `command` as a person would type it, for messages.
pub fn shown(command: &Command) -> String {
    let mut shown = command.get_program().to_string_lossy().into_owned();
    for argument in command.get_args() {
        shown.push(' ');
        shown.push_str(&argument.to_string_lossy());
    }
    shown
}

/// The exit code a task ends with for a program that ended as `status`: the program's own, or
/// 1 when it has none that fits, as when a signal ended it.
pub fn code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

/// Runs `command`, which must start and succeed, with this process's output and errors.
pub fn must(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{} didn't start: {error}", shown(command)))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed: {status}", shown(command)))
    }
}

/// What `program` printed with `arguments`, whether or not it succeeded, as `$(...)` keeps it:
/// nothing when it can't start.
pub fn printed(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// What `command` printed, when it started and succeeded.
pub fn output_of(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Makes `path` a program anyone may run, as `chmod 755` does; Windows has no such mode.
pub fn executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// A program started with its output and its errors in one stream, which goes to this process's
/// output and to a log file as it comes, as `2>&1 | tee <log>` does.
pub struct Logged {
    /// The program.
    pub child: Child,
    /// What copies the stream.
    copier: JoinHandle<()>,
}

impl Logged {
    /// Starts `command`, its stream copied into the file `log`, which it makes anew.
    pub fn start(mut command: Command, log: &Path) -> Result<Self, String> {
        let typed = shown(&command);
        let mut file =
            std::fs::File::create(log).map_err(|error| format!("{}: {error}", log.display()))?;
        let (mut reader, writer) = std::io::pipe().map_err(|error| format!("a pipe: {error}"))?;
        let errors = writer
            .try_clone()
            .map_err(|error| format!("a pipe: {error}"))?;
        command.stdout(writer).stderr(errors);
        let child = command
            .spawn()
            .map_err(|error| format!("{typed} didn't start: {error}"))?;
        // The command holds the pipe's writing ends, and the stream ends only once nothing holds
        // one but the programs it started.
        drop(command);
        let copier = std::thread::spawn(move || {
            let mut out = std::io::stdout();
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        let piece = &buffer[..read];
                        let _ = out.write_all(piece);
                        let _ = out.flush();
                        let _ = file.write_all(piece);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        });
        Ok(Self { child, copier })
    }

    /// Waits for the program to end, then for the rest of its stream.
    pub fn wait(mut self) -> Result<ExitStatus, String> {
        let waited = self.child.wait();
        self.drain();
        waited.map_err(|error| format!("waiting for a program: {error}"))
    }

    /// Waits for the rest of the stream once the program ended, for at most [`DRAIN`].
    pub fn drain(self) {
        let started = Instant::now();
        while !self.copier.is_finished() && started.elapsed() < DRAIN {
            std::thread::sleep(Duration::from_millis(50));
        }
        if self.copier.is_finished() {
            let _ = self.copier.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command is shown with its arguments, as typed.
    #[test]
    fn a_command_is_shown_as_typed() {
        let mut command = Command::new("cargo");
        command.args(["test", "--release"]);
        assert_eq!(shown(&command), "cargo test --release");
    }

    /// A program's stream goes whole into its log, and its own exit code is kept.
    #[test]
    fn a_logged_program_keeps_its_stream_and_its_code() {
        let folder = tempfile::tempdir().expect("a folder");
        let log = folder.path().join("run.log");
        let mut command = cargo();
        command.arg("--version");
        let status = Logged::start(command, &log)
            .expect("cargo starts")
            .wait()
            .expect("cargo ends");
        assert!(status.success());
        assert_eq!(code(status), 0);
        let kept = std::fs::read_to_string(&log).expect("the log");
        assert!(kept.starts_with("cargo "), "{kept}");
    }
}
