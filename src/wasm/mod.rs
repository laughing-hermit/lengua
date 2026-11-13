//! lengua's WebAssembly runtime: a program's `main.wat`, with lengua's library merged in, built
//! into a standard WASI preview 1 module ([`text`]), run by Wasmtime ([`engine`]) through
//! lengua's own WASI host ([`host`]), identically on Linux, macOS and Windows, on x86-64 and
//! ARM64. Tests run in a pool of reused workers ([`pool`]), each a child process of this binary
//! ([`worker`]) that the server talks to in length-prefixed JSON ([`job`]); a run from a shell
//! runs in place ([`runner::run_here`]). How a run went is a [`crate::runner::Run`], which
//! everything that compares results reads.

pub mod contract;
pub mod engine;
pub mod host;
pub mod job;
pub mod outside;
pub mod parts;
pub mod pool;
pub mod runner;
pub mod text;
pub mod tools;
pub mod wasi;
pub mod worker;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use self::host::policy::Policy;
pub use self::pool::{Peer, Pool, Running, Spec};
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
    /// The pages its own memory may grow to ([`most_pages`]).
    pages: u64,
    /// The modules it calls, in the order of its idea's `- Calls:` lines.
    calls: Vec<CalledModule>,
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
            pages: most_pages(&built.wasm),
            lines: Arc::new((built.lines.clone(), built.bodies.clone())),
            built,
            file,
            _folder: folder,
            calls: Vec::new(),
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

    /// The program with `calls`, the modules its idea calls, in the order of the idea's
    /// `- Calls:` lines: every run of it starts them beside it.
    #[must_use]
    pub fn with_calls(mut self, calls: Vec<CalledModule>) -> Self {
        self.calls = calls;
        self
    }

    /// The modules it calls.
    #[must_use]
    pub fn calls(&self) -> &[CalledModule] {
        &self.calls
    }

    /// The pages of 64 KiB that a run of it may come to hold: what its own memory may grow to,
    /// and the memory of each module it calls.
    #[must_use]
    pub fn pages(&self) -> u64 {
        self.calls.iter().fold(self.pages, |pages, called| {
            pages.saturating_add(called.pages)
        })
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

/// The pages of 64 KiB that the memories of the module `wasm` may grow to in all: each memory's
/// maximum, or what it starts with when it declares none. Nothing for bytes that are no module.
#[must_use]
pub fn most_pages(wasm: &[u8]) -> u64 {
    let mut pages = 0_u64;
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        match payload {
            Ok(wasmparser::Payload::MemorySection(reader)) => {
                for memory in reader.into_iter().flatten() {
                    pages = pages.saturating_add(memory.maximum.unwrap_or(memory.initial));
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    pages
}

/// A module a program calls ([`crate::call`]), kept in a file of its own for the pool's
/// workers, as the program's own module is.
#[derive(Debug)]
pub struct CalledModule {
    /// Its name, as the idea's `- Calls:` line names it.
    pub name: String,
    /// The module's SHA-256.
    pub hash: crate::hash::Sha256Hex,
    /// The module's file.
    pub file: PathBuf,
    /// The folder that holds it.
    _folder: tempfile::TempDir,
    /// The pages its memory may grow to ([`most_pages`]).
    pages: u64,
}

impl CalledModule {
    /// The called module `name`, whose bytes are `wasm`, kept for the workers.
    ///
    /// # Errors
    ///
    /// When its file can't be written.
    pub fn new(name: &str, wasm: &[u8]) -> io::Result<Self> {
        let folder = tempfile::tempdir()?;
        let file = folder.path().join("called.wasm");
        std::fs::write(&file, wasm)?;
        Ok(Self {
            name: name.to_owned(),
            hash: crate::hash::Sha256Hex::of(wasm),
            pages: most_pages(wasm),
            file,
            _folder: folder,
        })
    }
}
