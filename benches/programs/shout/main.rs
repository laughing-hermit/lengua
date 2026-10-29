//! shout in Rust: prints up to 1048576 bytes of standard input with ASCII letters capitalized.
use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut data = Vec::with_capacity(1_048_577);
    if std::io::stdin().lock().take(1_048_577).read_to_end(&mut data).is_err() {
        return ExitCode::from(2);
    }
    if data.len() > 1_048_576 {
        eprintln!("shout: the input is longer than 1048576 bytes");
        return ExitCode::from(1);
    }
    data.make_ascii_uppercase();
    match std::io::stdout().lock().write_all(&data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
