//! shout in Rust: prints up to 1048576 bytes of standard input with ASCII letters capitalized,
//! in one write, as C's are.
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
    let mut data = Vec::with_capacity(1_048_577);
    if std::io::stdin().lock().take(1_048_577).read_to_end(&mut data).is_err() {
        return ExitCode::from(2);
    }
    if data.len() > 1_048_576 {
        eprintln!("shout: the input is longer than 1048576 bytes");
        return ExitCode::from(1);
    }
    data.make_ascii_uppercase();
    match raw_stdout().write_all(&data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
