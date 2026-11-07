//! upcase-stdin in Rust: copies standard input of any length to standard output with ASCII
//! letters capitalized, 64 KiB at a time, each piece in one write, as C's are.
use std::fs::File;
use std::io::{Read, Write};
use std::mem::ManuallyDrop;
use std::process::ExitCode;

/// Standard output with no buffer in front of it, so that a write is one of the system's: std's
/// `Stdout` holds its last line back, and splits each write at its last newline into two.
fn raw_stdout() -> ManuallyDrop<File> {
    #[cfg(any(unix, target_os = "wasi"))]
    // SAFETY: standard output's descriptor is open for the whole run, and the file, never
    // dropped, never closes it.
    let file = unsafe {
        use std::os::fd::{AsRawFd, FromRawFd};
        File::from_raw_fd(std::io::stdout().as_raw_fd())
    };
    #[cfg(windows)]
    // SAFETY: standard output's handle is open for the whole run, and the file, never dropped,
    // never closes it.
    let file = unsafe {
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        File::from_raw_handle(std::io::stdout().as_raw_handle())
    };
    ManuallyDrop::new(file)
}

fn main() -> ExitCode {
    let mut input = std::io::stdin().lock();
    let mut output = raw_stdout();
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
    ExitCode::SUCCESS
}
