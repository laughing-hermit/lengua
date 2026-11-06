//! lengua's WebAssembly runtime: a program's `main.wat`, with lengua's library merged in, built
//! into a standard WASI preview 1 module ([`text`]), run by Wasmtime ([`engine`]) through
//! lengua's own WASI host ([`host`]), identically on Linux, macOS and Windows, on x86-64 and
//! ARM64. Tests run in a pool of reused workers ([`pool`]), each a child process of this binary
//! ([`worker`]) that the server talks to in length-prefixed JSON ([`job`]); a run from a shell
//! runs in place ([`runner::run_here`]). How a run went is the runner's own [`crate::runner::Run`],
//! so everything that compares results compares a WebAssembly program's as a native one's.
//!
//! During the pivot the native path (`main.s`) stays beside this one, chosen by the code file.

pub mod engine;
pub mod host;
pub mod job;
pub mod pool;
pub mod runner;
pub mod text;
pub mod wasi;
pub mod worker;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use self::host::policy::Policy;
pub use self::pool::{Pool, Running, Spec};
pub use self::text::{Built, Problem};

/// The command-line flag that starts this binary as a worker.
pub const WORKER_FLAG: &str = "--wasm-worker";

/// The code file of a WebAssembly program.
pub const CODE_FILE: &str = "main.wat";

/// A program's name, its first argument: the module's file's, the same on every system.
pub const PROGRAM_NAME: &str = "main.wasm";

/// A built program ready for the pool: its module kept in a file of its own, which workers
/// compile once each, checked by its hash.
#[derive(Debug)]
pub struct Program {
    /// What the build made.
    built: Built,
    /// The module's SHA-256, in hex.
    hash: crate::hash::Sha256Hex,
    /// main.wat's lines, for a trap's.
    lines: pool::Lines,
    /// The module's file.
    file: PathBuf,
    /// The folder that holds it.
    _folder: tempfile::TempDir,
}

impl Program {
    /// A program of what `built` made.
    ///
    /// # Errors
    ///
    /// When its module's file can't be written.
    pub fn new(built: Built) -> io::Result<Self> {
        let folder = tempfile::tempdir()?;
        let file = folder.path().join(PROGRAM_NAME);
        std::fs::write(&file, &built.wasm)?;
        Ok(Self {
            hash: crate::hash::Sha256Hex::of(&built.wasm),
            lines: Arc::new((built.lines.clone(), built.bodies.clone())),
            built,
            file,
            _folder: folder,
        })
    }

    /// The program main.wat's `text` builds.
    ///
    /// # Errors
    ///
    /// What's wrong with the text, or why its module couldn't be kept.
    pub fn of_text(text: &str) -> Result<Self, String> {
        let built = text::build(text).map_err(|problem| problem.to_string())?;
        Self::new(built).map_err(|error| format!("lengua couldn't keep the module: {error}"))
    }

    /// What the build made.
    #[must_use]
    pub const fn built(&self) -> &Built {
        &self.built
    }

    /// The module's SHA-256, in hex.
    #[must_use]
    pub fn hash(&self) -> &str {
        self.hash.as_str()
    }

    /// The module's file.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// main.wat's lines by operator offset, and where its functions' bodies are.
    #[must_use]
    pub fn lines(&self) -> pool::Lines {
        Arc::clone(&self.lines)
    }
}
