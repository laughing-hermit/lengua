//! lengua's tooling, in Rust: the tasks CI's workflow runs, the release archive, the goldens'
//! comparison and the bench's WebAssembly builds. `cargo xtask <task>` runs one (the alias in
//! .cargo/config.toml); each task does what one step of ci.yml needs, or what a person runs by
//! hand.

use std::process::ExitCode;

/// How `cargo xtask` is called, and its tasks.
const USAGE: &str = "usage: cargo xtask <task> [<argument>...]

Tasks:
  help    this list";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("help" | "--help" | "-h") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("cargo xtask: no task named {other:?}\n{USAGE}");
            ExitCode::from(2)
        }
        None => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
