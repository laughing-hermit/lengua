//! files in Rust: reads every file in the current folder to its end and prints how many files
//! there are and how many bytes they hold. Folders are neither counted nor read.
use std::io::{Read, Write};
use std::process::ExitCode;

/// How many files the current folder holds and their bytes, or `None` when one can't be read.
fn count() -> Option<(u64, u64)> {
    let (mut files, mut bytes) = (0_u64, 0_u64);
    let mut buffer = vec![0_u8; 1 << 16];
    for entry in std::fs::read_dir(".").ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_file() {
            continue;
        }
        let mut file = std::fs::File::open(entry.path()).ok()?;
        loop {
            match file.read(&mut buffer).ok()? {
                0 => break,
                got => bytes += got as u64,
            }
        }
        files += 1;
    }
    Some((files, bytes))
}

fn main() -> ExitCode {
    let Some((files, bytes)) = count() else {
        return ExitCode::from(3);
    };
    match writeln!(std::io::stdout(), "{files} {bytes}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
