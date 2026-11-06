//! A system lengua doesn't run native programs on: anything but Linux. During the pivot to
//! WebAssembly (todo.md), macOS and Windows run lengua's WebAssembly programs instead; a native
//! program, `main.s`, says here that it needs Linux, and says it once. Checking and proving it
//! don't run it and work as everywhere.

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
        "lengua runs native programs (main.s) on Linux only; on this system its programs are \
         WebAssembly (main.wat), which come with the pivot in todo.md",
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
    /// connection's number in the idea, from 0.
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
