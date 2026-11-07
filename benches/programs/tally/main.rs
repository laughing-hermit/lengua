//! tally in Rust: counts how often each word of standard input of any length occurs, and prints
//! each with its count, the most frequent first, words with the same count in byte order. A word
//! is a run of bytes other than whitespace, of up to 255 bytes. Standard input comes 64 KiB at a
//! time, a word at a piece's end carried to the next; the words go in a HashMap with FNV-1a.
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::io::{BufWriter, Read, Write};
use std::process::ExitCode;

const PIECE: usize = 65_536;
const MAX_WORD: usize = 255;

/// FNV-1a, 64 bits.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn is_space(byte: u8) -> bool {
    byte == b' ' || (b'\t'..=b'\r').contains(&byte)
}

fn main() -> ExitCode {
    let mut counts: HashMap<Vec<u8>, u64, BuildHasherDefault<Fnv>> = HashMap::default();
    let mut input = std::io::stdin().lock();
    let mut buffer = vec![0_u8; MAX_WORD + PIECE];
    let mut carry = 0;
    loop {
        let got = match input.read(&mut buffer[carry..carry + PIECE]) {
            Ok(got) => got,
            Err(_) => return ExitCode::from(3),
        };
        let (filled, ended) = (carry + got, got == 0);
        carry = 0;
        let mut at = 0;
        loop {
            while at < filled && is_space(buffer[at]) {
                at += 1;
            }
            if at == filled {
                break;
            }
            let start = at;
            while at < filled && !is_space(buffer[at]) {
                at += 1;
            }
            if at - start > MAX_WORD {
                eprintln!("tally: a word is longer than 255 bytes");
                return ExitCode::from(4);
            }
            // A word that runs to the end of a piece may go on in the next.
            if at == filled && !ended {
                carry = filled - start;
                buffer.copy_within(start..filled, 0);
                break;
            }
            let word = &buffer[start..at];
            match counts.get_mut(word) {
                Some(count) => *count += 1,
                None => {
                    counts.insert(word.to_vec(), 1);
                }
            }
        }
        if ended {
            break;
        }
    }
    let mut sorted: Vec<(&[u8], u64)> = counts
        .iter()
        .map(|(word, count)| (word.as_slice(), *count))
        .collect();
    sorted.sort_unstable_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(y.0)));
    let mut out = BufWriter::with_capacity(1 << 16, std::io::stdout().lock());
    for (word, count) in sorted {
        let line = write!(out, "{count} ")
            .and_then(|()| out.write_all(word))
            .and_then(|()| out.write_all(b"\n"));
        if line.is_err() {
            return ExitCode::from(2);
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}
