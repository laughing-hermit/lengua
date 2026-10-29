//! shout in Rust with threads: like main.rs, with the capitalizing split over up to 16 cores
//! the way lengua's fork-join template splits it.
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
    let threads = std::thread::available_parallelism().map_or(1, |count| count.get().min(16));
    let chunk = data.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for part in data.chunks_mut(chunk) {
            scope.spawn(move || part.make_ascii_uppercase());
        }
    });
    match std::io::stdout().lock().write_all(&data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
