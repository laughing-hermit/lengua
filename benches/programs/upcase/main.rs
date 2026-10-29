//! upcase in Rust: copies a file of up to 1048576 bytes to another with ASCII letters capitalized.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: upcase <input> <output>");
        return ExitCode::from(1);
    }
    let Ok(mut data) = std::fs::read(&args[1]) else {
        eprintln!("upcase: cannot open {}", args[1]);
        return ExitCode::from(2);
    };
    if data.len() > 1_048_576 {
        eprintln!("upcase: {} is larger than 1048576 bytes", args[1]);
        return ExitCode::from(3);
    }
    data.make_ascii_uppercase();
    match std::fs::write(&args[2], &data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
