//! sort in Rust: prints the lines of standard input of up to 67108864 bytes in byte order, as
//! LC_ALL=C sort does, a line that starts another first, and a last line without a newline given
//! one. The lines are slices of the input, sorted with sort_unstable.
use std::io::{BufWriter, Read, Write};
use std::process::ExitCode;

const LIMIT: usize = 67_108_864;

fn main() -> ExitCode {
    let mut input = Vec::with_capacity(LIMIT + 2);
    if std::io::stdin()
        .lock()
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut input)
        .is_err()
    {
        return ExitCode::from(3);
    }
    if input.len() > LIMIT {
        eprintln!("sort: the input is longer than 67108864 bytes");
        return ExitCode::from(4);
    }
    let mut lines: Vec<&[u8]> = match input.strip_suffix(b"\n") {
        Some(body) => body.split(|byte| *byte == b'\n').collect(),
        None if input.is_empty() => Vec::new(),
        None => input.split(|byte| *byte == b'\n').collect(),
    };
    // Slices of bytes compare in byte order, a prefix first.
    lines.sort_unstable();
    let mut out = BufWriter::with_capacity(1 << 16, std::io::stdout().lock());
    for line in &lines {
        if out.write_all(line).and_then(|()| out.write_all(b"\n")).is_err() {
            return ExitCode::from(2);
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
