use std::process::ExitCode;

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--version"] {
        println!("{VERSION}");
        ExitCode::SUCCESS
    } else {
        eprintln!("usage: {} [--version]", env!("CARGO_PKG_NAME"));
        ExitCode::from(2)
    }
}
