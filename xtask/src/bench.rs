//! The bench's steps in CI's bench mode.
//!
//! - `cargo xtask bench`: the gate (benches/against_rust_and_c.rs), all it prints kept in
//!   bench.log. It fails where the run and KNOWN_BEHIND disagree, and writes
//!   BENCHMARKS-<target>.md from the run's results. Windows builds the native C and C++ with the
//!   image's LLVM clang, whose folder goes last on PATH where the image has it.
//! - `cargo xtask bench-report <report>`: the report, in the job's summary, when the bench wrote
//!   one.

use std::path::{Path, PathBuf};

use crate::ci;

/// Where Windows's runner images keep LLVM's programs.
const LLVM: &str = r"C:\Program Files\LLVM\bin";

/// `cargo xtask bench`.
pub fn bench(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask bench".to_owned());
    }
    let root = ci::root()?;
    let mut command = ci::cargo();
    command
        .current_dir(&root)
        .args(["bench", "--locked", "--bench", "against_rust_and_c"]);
    let llvm = Path::new(LLVM);
    if cfg!(windows) && llvm.is_dir() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
        folders.push(llvm.to_path_buf());
        let path = std::env::join_paths(folders).map_err(|error| format!("PATH: {error}"))?;
        command.env("PATH", path);
    }
    let status = ci::Logged::start(command, &root.join("bench.log"))?.wait()?;
    Ok(ci::code(status))
}

/// `cargo xtask bench-report <report>`.
pub fn report(arguments: &[String]) -> Result<u8, String> {
    let [report] = arguments else {
        return Err("usage: cargo xtask bench-report <report>".to_owned());
    };
    if let Ok(text) = std::fs::read_to_string(report) {
        ci::append("GITHUB_STEP_SUMMARY", &text)?;
    }
    Ok(0)
}
