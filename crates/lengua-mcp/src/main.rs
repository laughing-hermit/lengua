use std::process::ExitCode;

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (None | Some("--version"), None) => {
            println!("{VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: {} [--version]", env!("CARGO_PKG_NAME"));
            ExitCode::from(2)
        }
    }
}
