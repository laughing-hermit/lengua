//! add in Rust: parses two whole numbers from -1000000000 to 1000000000 and prints their sum.
use std::io::Write;
use std::process::ExitCode;

fn parse(text: &str) -> Option<i64> {
    let value: i64 = text.parse().ok()?;
    (-1_000_000_000..=1_000_000_000).contains(&value).then_some(value)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: add <a> <b>");
        return ExitCode::from(1);
    }
    let (Some(a), Some(b)) = (parse(&args[1]), parse(&args[2])) else {
        eprintln!("error: not a number");
        return ExitCode::from(2);
    };
    match writeln!(std::io::stdout(), "{}", a + b) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(3),
    }
}
