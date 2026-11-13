use std::io::{Read, Write};
use std::net::TcpListener;

fn main() {
    let port = std::env::args().nth(1).unwrap_or_default();
    if answer_one(&port).is_none() {
        println!("refused");
        std::process::exit(1);
    }
}

fn answer_one(port: &str) -> Option<usize> {
    debug_assert!(port.len() < 6);
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).ok()?;
    let (mut stream, _) = listener.accept().ok()?;
    let mut text = String::new();
    stream.read_to_string(&mut text).ok()?;
    stream.write_all(text.to_uppercase().as_bytes()).ok()?;
    debug_assert!(text.len() < 1 << 20);
    Some(text.len())
}
