//! A system lengua doesn't run programs on: anything but Linux. Windows reaches lengua through
//! WSL 2 and a Mac through a Linux VM or container, which are Linux. Everything that needs a
//! running program says so, and says it once, here.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use crate::connect::{Made, Peer};
use crate::runner::{End, Job, Limits, Run};

/// Why nothing runs here.
fn cannot() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "lengua runs programs on Linux only: on Windows use WSL 2, and on a Mac a Linux VM or \
         container, and run your agent and lengua there (README.md, \"Windows and Mac\")",
    )
}

/// A program that is running: there is none on this system, so this has no values.
pub struct Started {
    never: std::convert::Infallible,
}

impl Started {
    /// The program's process id.
    #[must_use]
    pub fn pid(&self) -> i32 {
        match self.never {}
    }

    /// The loopback port it serves on, when it serves.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        match self.never {}
    }

    /// lengua's ends of the connections that go to the test's own servers, each with its
    /// connection's number in the spec, from 0.
    pub fn take_peers(&mut self) -> Vec<(usize, Peer)> {
        match self.never {}
    }

    /// What became of each connection lengua made for the program, in order.
    #[must_use]
    pub fn connections(&self) -> &[Made] {
        match self.never {}
    }

    /// Whether the program's broker knows it waits for a connection or a key.
    #[must_use]
    pub fn waiting(&self) -> Option<bool> {
        match self.never {}
    }

    /// Waits for the program to end.
    ///
    /// # Errors
    ///
    /// Never: no program is running.
    pub async fn wait(self) -> io::Result<Run> {
        match self.never {}
    }

    /// Stops the program now.
    ///
    /// # Errors
    ///
    /// Never: no program is running.
    pub async fn stop(self) -> io::Result<Run> {
        match self.never {}
    }
}

/// Starts `job`: here it can't be.
///
/// # Errors
///
/// Always: this system can't run programs yet.
pub async fn start(_launcher: &Path, _job: Job<'_>, _limits: Limits) -> io::Result<Started> {
    Err(cannot())
}

/// Runs `job` from a shell: here it can't be.
///
/// # Errors
///
/// Always: this system can't run programs yet.
pub async fn attached(_launcher: &Path, _job: Job<'_>) -> io::Result<End> {
    Err(cannot())
}

/// Launcher mode: nothing launches here, so it says so and fails.
#[must_use]
pub fn launch(_args: impl Iterator<Item = OsString>) -> ExitCode {
    eprintln!("lengua-mcp: {}", cannot());
    ExitCode::from(127)
}
