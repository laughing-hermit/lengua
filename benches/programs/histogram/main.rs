//! histogram in Rust: counts each byte value of standard input and prints the values that
//! occur, with their counts. Four tables, so runs of one byte don't wait on the same counter.
use std::io::{Read, Write};

fn main() {
    let mut input = std::io::stdin().lock();
    let mut buffer = vec![0_u8; 1 << 16];
    let mut counts = [[0_u64; 256]; 4];
    loop {
        let got = input.read(&mut buffer).expect("read standard input");
        if got == 0 {
            break;
        }
        let mut quads = buffer[..got].chunks_exact(4);
        for quad in &mut quads {
            counts[0][usize::from(quad[0])] += 1;
            counts[1][usize::from(quad[1])] += 1;
            counts[2][usize::from(quad[2])] += 1;
            counts[3][usize::from(quad[3])] += 1;
        }
        for &byte in quads.remainder() {
            counts[0][usize::from(byte)] += 1;
        }
    }
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for value in 0..256 {
        let total: u64 = counts.iter().map(|table| table[value]).sum();
        if total > 0 {
            writeln!(out, "{value} {total}").expect("write");
        }
    }
}
