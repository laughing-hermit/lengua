//! count: prints how many bytes the file notes.txt in the current folder holds. A plain Rust
//! program with the standard library, which `cargo xtask outside-wasm` compiles with rustc for
//! wasm32-wasip1, and which lengua's suite then checks and runs as an outside module under the
//! idea beside it (tests/outside.rs). It is written as any Rust program is: lengua's rules for
//! main.wat hold nothing of it.

use std::io::Write as _;

fn main() {
    let Ok(bytes) = std::fs::read("notes.txt") else {
        eprintln!("no notes.txt");
        std::process::exit(3);
    };
    let mut out = std::io::stdout();
    if writeln!(out, "{}", bytes.len()).is_err() {
        std::process::exit(2);
    }
}
