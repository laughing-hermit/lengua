use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

fn main() {
    let address = std::env::args().nth(1).unwrap_or_default();
    if let Some(answer) = fetch(&address) {
        print!("{answer}");
    } else {
        println!("refused");
        std::process::exit(1);
    }
}

fn fetch(address: &str) -> Option<String> {
    debug_assert!(address.len() < 256);
    let mut stream = TcpStream::connect(address).ok()?;
    stream.write_all(b"ping\n").ok()?;
    stream.shutdown(Shutdown::Write).ok()?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok()?;
    debug_assert!(answer.len() < 1 << 20);
    Some(answer)
}
