//! hypotenuse in Rust: prints the hypotenuse of two sides from 0.001 to 1000000.000, with three
//! decimals, computed in doubles as the idea says.
use std::io::Write;
use std::process::ExitCode;

/// A side in thousandths: digits, then optionally a point and one to three digits; or the exit
/// code for a side that isn't a number (2) or is out of range (3).
fn parse(text: &str) -> Result<i64, u8> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "000"));
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || !digits(fraction) || fraction.len() > 3 {
        return Err(2);
    }
    let whole: i64 = whole.parse().unwrap_or(i64::MAX).min(1_000_001);
    let fraction: i64 = format!("{fraction:0<3}").parse().map_err(|_| 2_u8)?;
    let units = whole.saturating_mul(1000).saturating_add(fraction);
    if whole > 1_000_000 || units == 0 || units > 1_000_000_000 {
        return Err(3);
    }
    Ok(units)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: hypotenuse <a> <b>");
        return ExitCode::from(1);
    }
    let (a, b) = match (parse(&args[1]), parse(&args[2])) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(2), _) | (Ok(_), Err(2)) => {
            eprintln!("error: side is not a number");
            return ExitCode::from(2);
        }
        _ => {
            eprintln!("error: side out of range");
            return ExitCode::from(3);
        }
    };
    #[allow(clippy::cast_precision_loss)]
    let (x, y) = (a as f64 / 1000.0, b as f64 / 1000.0);
    #[allow(clippy::cast_possible_truncation)]
    let thousandths = ((x * x + y * y).sqrt() * 1000.0).round_ties_even() as i64;
    match writeln!(std::io::stdout(), "{}.{:03}", thousandths / 1000, thousandths % 1000) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
