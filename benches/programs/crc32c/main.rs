//! crc32c in Rust: the CRC-32C (Castagnoli) of standard input, in hex, with the CPU's crc32
//! instruction 8 bytes at a time.
use std::arch::x86_64::{_mm_crc32_u8, _mm_crc32_u64};
use std::io::{Read, Write};

fn main() {
    let mut input = std::io::stdin().lock();
    let mut buffer = vec![0_u8; 1 << 16];
    let mut crc: u64 = 0xFFFF_FFFF;
    loop {
        let got = input.read(&mut buffer).expect("read standard input");
        if got == 0 {
            break;
        }
        let mut words = buffer[..got].chunks_exact(8);
        for word in &mut words {
            let word = u64::from_le_bytes(word.try_into().expect("8 bytes"));
            // SAFETY: built with -C target-cpu=native on a CPU with SSE4.2.
            crc = unsafe { _mm_crc32_u64(crc, word) };
        }
        for &byte in words.remainder() {
            // SAFETY: as above.
            crc = u64::from(unsafe { _mm_crc32_u8(crc as u32, byte) });
        }
    }
    writeln!(std::io::stdout(), "{}", !(crc as u32)).expect("write");
}
