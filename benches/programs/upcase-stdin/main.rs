//! upcase-stdin in Rust: copies standard input of any length to standard output with ASCII
//! letters capitalized, 64 KiB at a time.
use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let got = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(got) => got,
            Err(_) => return ExitCode::from(3),
        };
        buffer[..got].make_ascii_uppercase();
        if output.write_all(&buffer[..got]).is_err() {
            return ExitCode::from(4);
        }
    }
    match output.flush() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
