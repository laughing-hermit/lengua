//! readings in Rust: reads the clock that only moves forward as many times as its argument says,
//! from 1 to 1000000, and prints how many readings were not earlier than the previous one.
use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let count = match args.get(1).map(|text| text.parse::<u32>()) {
        Some(Ok(count)) if args.len() == 2 && (1..=1_000_000).contains(&count) => count,
        _ => {
            eprintln!("usage: readings <count>");
            return ExitCode::from(1);
        }
    };
    let mut before: Option<Instant> = None;
    let mut kept = 0_u32;
    for _ in 0..count {
        let now = Instant::now();
        if before.is_none_or(|before| now >= before) {
            kept += 1;
        }
        before = Some(now);
    }
    match writeln!(std::io::stdout(), "{kept}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
