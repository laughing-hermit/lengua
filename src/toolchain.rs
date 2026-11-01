//! Finds LLVM's assembler (`llvm-mc`) and linker (`ld.lld`) and runs them with warnings as
//! errors.
//!
//! Tool output is always captured: the server's own stdin and stdout carry the MCP protocol, so a
//! child process must never inherit them.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::platform::Profile;

/// How long one assembler or linker run may take.
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest tool error output kept in a message.
const MAX_MESSAGE_BYTES: usize = 8 * 1024;

/// What to tell the user when LLVM isn't installed.
pub const INSTALL_HINT: &str = "Install LLVM's assembler and linker \
    (Fedora: sudo dnf install llvm lld; Debian/Ubuntu: sudo apt install llvm lld).";

/// One external tool and its reported version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// Absolute path of the executable.
    pub path: PathBuf,
    /// Version as reported by `--version`, e.g. `22.1.8`.
    pub version: String,
}

/// The assembler and linker lengua builds programs with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    /// `llvm-mc`.
    pub assembler: Tool,
    /// `ld.lld`.
    pub linker: Tool,
}

/// Why the toolchain couldn't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainError {
    /// The executable isn't on `PATH`.
    NotFound {
        /// Executable name, e.g. `llvm-mc`.
        tool: &'static str,
    },
    /// The executable exists but `--version` failed.
    Unusable {
        /// Executable name.
        tool: &'static str,
        /// What went wrong.
        message: String,
    },
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tool } => write!(f, "{tool} was not found on PATH. {INSTALL_HINT}"),
            Self::Unusable { tool, message } => write!(f, "{tool} failed to run: {message}"),
        }
    }
}

impl std::error::Error for ToolchainError {}

/// Which build step failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStep {
    /// `llvm-mc` turning `main.s` into an object file.
    Assemble,
    /// `ld.lld` turning the object file into an executable.
    Link,
}

/// A failed assembler or linker run, with the tool's own error output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError {
    /// The step that failed.
    pub step: BuildStep,
    /// The tool's error output (trimmed), or why it couldn't run.
    pub message: String,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let step = match self.step {
            BuildStep::Assemble => "assembling",
            BuildStep::Link => "linking",
        };
        write!(f, "{step} failed: {}", self.message)
    }
}

impl std::error::Error for BuildError {}

impl Toolchain {
    /// Finds `llvm-mc` and `ld.lld` on `PATH` and reads their versions.
    pub async fn discover() -> Result<Self, ToolchainError> {
        let assembler = discover_tool("llvm-mc", parse_llvm_mc_version).await?;
        let linker = discover_tool("ld.lld", parse_lld_version).await?;
        Ok(Self { assembler, linker })
    }

    /// Assembles `source` into the object file `object`, treating warnings as errors.
    pub async fn assemble(
        &self,
        profile: Profile,
        source: &Path,
        object: &Path,
    ) -> Result<(), BuildError> {
        let triple = format!("--triple={}", profile.llvm_triple());
        let args: [&OsStr; 6] = [
            triple.as_ref(),
            "--filetype=obj".as_ref(),
            "--fatal-warnings".as_ref(),
            source.as_os_str(),
            "-o".as_ref(),
            object.as_os_str(),
        ];
        run_build_tool(BuildStep::Assemble, &self.assembler.path, &args).await
    }

    /// Links `object` into the static executable `executable`, treating warnings as errors. For
    /// AArch64 the linker is told not to relax: it would rewrite an `adrp` + `add :lo12:` pair
    /// into `nop` + `adr`, and the machine code would no longer be main.s line for line.
    pub async fn link(
        &self,
        profile: Profile,
        object: &Path,
        executable: &Path,
    ) -> Result<(), BuildError> {
        let relax: &[&OsStr] = match profile {
            Profile::LinuxX86_64 => &[],
            Profile::LinuxAarch64 => &["--no-relax".as_ref()],
        };
        let args: Vec<&OsStr> = ["--fatal-warnings".as_ref()]
            .into_iter()
            .chain(relax.iter().copied())
            .chain(["-o".as_ref(), executable.as_os_str(), object.as_os_str()])
            .collect();
        run_build_tool(BuildStep::Link, &self.linker.path, &args).await
    }
}

async fn discover_tool(
    name: &'static str,
    parse_version: fn(&str) -> Option<String>,
) -> Result<Tool, ToolchainError> {
    let path = find_in_path(name).ok_or(ToolchainError::NotFound { tool: name })?;
    let output = run_captured(&path, &[OsStr::new("--version")])
        .await
        .map_err(|message| ToolchainError::Unusable {
            tool: name,
            message,
        })?;
    if !output.status.success() {
        return Err(ToolchainError::Unusable {
            tool: name,
            message: format!("--version exited with {}", output.status),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = parse_version(&stdout).ok_or_else(|| ToolchainError::Unusable {
        tool: name,
        message: "unrecognized --version output".to_owned(),
    })?;
    Ok(Tool { path, version })
}

async fn run_build_tool(step: BuildStep, tool: &Path, args: &[&OsStr]) -> Result<(), BuildError> {
    let output = run_captured(tool, args)
        .await
        .map_err(|message| BuildError { step, message })?;
    if output.status.success() {
        return Ok(());
    }
    let mut message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if message.is_empty() {
        message = format!("exited with {}", output.status);
    }
    truncate_to_char_boundary(&mut message, MAX_MESSAGE_BYTES);
    Err(BuildError { step, message })
}

/// Runs a tool with no stdin and captured stdout/stderr, under [`TOOL_TIMEOUT`].
async fn run_captured(tool: &Path, args: &[&OsStr]) -> Result<std::process::Output, String> {
    let child = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(TOOL_TIMEOUT, child).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("could not start {}: {error}", tool.display())),
        Err(_) => Err(format!(
            "{} timed out after {TOOL_TIMEOUT:?}",
            tool.display()
        )),
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// `llvm-mc --version` prints a line like `  LLVM version 22.1.8`, or from a distribution's own
/// build, `Ubuntu LLVM version 22.1.8`.
fn parse_llvm_mc_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, version) = line.split_once("LLVM version ")?;
        version.split_whitespace().next().map(str::to_owned)
    })
}

/// `ld.lld --version` prints `LLD 22.1.8 (compatible with GNU linkers)`, or from a distribution's
/// own build, `Ubuntu LLD 22.1.8 (compatible with GNU linkers)`.
fn parse_lld_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        words.by_ref().find(|word| *word == "LLD")?;
        words.next().map(str::to_owned)
    })
}

fn truncate_to_char_boundary(text: &mut String, max_bytes: usize) {
    if text.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(" …(truncated)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_llvm_mc_version() {
        let output = "LLVM (http://llvm.org/):\n  LLVM version 22.1.8\n  Optimized build.\n";
        assert_eq!(parse_llvm_mc_version(output).as_deref(), Some("22.1.8"));
        assert_eq!(parse_llvm_mc_version("nothing here"), None);
        // apt.llvm.org's, and Debian's and Ubuntu's own.
        let ubuntu = "Ubuntu LLVM version 22.1.8\n  Optimized build.\n";
        assert_eq!(parse_llvm_mc_version(ubuntu).as_deref(), Some("22.1.8"));
    }

    #[test]
    fn parses_lld_version() {
        let output = "LLD 22.1.8 (compatible with GNU linkers)\n";
        assert_eq!(parse_lld_version(output).as_deref(), Some("22.1.8"));
        assert_eq!(parse_lld_version("GNU ld 2.46"), None);
        let ubuntu = "Ubuntu LLD 22.1.8 (compatible with GNU linkers)\n";
        assert_eq!(parse_lld_version(ubuntu).as_deref(), Some("22.1.8"));
    }

    #[test]
    fn truncates_on_a_char_boundary() {
        let mut text = "ééééé".to_owned();
        truncate_to_char_boundary(&mut text, 3);
        assert_eq!(text, "é …(truncated)");
    }
}
