//! wc in Rust: counts the lines and words of standard input, as `wc -lw` does. Written so the
//! compiler can vectorize it: a word starts at a byte that isn't space after one that is, and the
//! bytes are counted 255 at a time in counters of a byte, which a vector holds 16 of, before each
//! block's counts go to the totals.
use std::io::{Read, Write};

/// The most bytes a counter of a byte counts before it's added to its total.
const BLOCK: usize = 255;

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
        // Each byte from the second on, with the byte before it.
        let blocks = chunk[1..].chunks(BLOCK).zip(chunk[..got - 1].chunks(BLOCK));
        for (block, before) in blocks {
            let (mut block_lines, mut block_words) = (0_u8, 0_u8);
            for (byte, last) in block.iter().zip(before) {
                block_lines += u8::from(*byte == b'\n');
                block_words += u8::from(!is_space(*byte) & is_space(*last));
            }
            lines += u64::from(block_lines);
            words += u64::from(block_words);
        }
        previous = chunk[got - 1];
    }
    writeln!(std::io::stdout(), "{lines} {words}").expect("write");
}
