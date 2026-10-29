//! wc in Rust: counts the lines and words of standard input, as `wc -lw` does.
use std::io::{Read, Write};

fn is_space(byte: u8) -> bool {
    byte == b' ' || (b'\t'..=b'\r').contains(&byte)
}

fn main() {
    let mut input = std::io::stdin().lock();
    let mut buffer = vec![0_u8; 1 << 16];
    let (mut lines, mut words) = (0_u64, 0_u64);
    let mut previous = b' ';
    loop {
        let got = input.read(&mut buffer).expect("read standard input");
        if got == 0 {
            break;
        }
        let chunk = &buffer[..got];
        lines += u64::from(chunk[0] == b'\n');
        words += u64::from(!is_space(chunk[0]) && is_space(previous));
        for pair in chunk.windows(2) {
            lines += u64::from(pair[1] == b'\n');
            words += u64::from(!is_space(pair[1]) && is_space(pair[0]));
        }
        previous = chunk[got - 1];
    }
    writeln!(std::io::stdout(), "{lines} {words}").expect("write");
}
