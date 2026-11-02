//! crc32c in Rust: the CRC-32C (Castagnoli) of standard input, in hex, with the CPU's crc32
//! instruction 8 bytes at a time: x86-64's crc32, or AArch64's crc32cx.
use std::io::{Read, Write};

/// The state after 8 bytes.
#[cfg(target_arch = "x86_64")]
fn word_step(crc: u64, word: u64) -> u64 {
    // SAFETY: built with -C target-cpu=native on a CPU with SSE4.2.
    unsafe { std::arch::x86_64::_mm_crc32_u64(crc, word) }
}

/// The state after one byte.
#[cfg(target_arch = "x86_64")]
fn byte_step(crc: u64, byte: u8) -> u64 {
    // SAFETY: as above.
    u64::from(unsafe { std::arch::x86_64::_mm_crc32_u8(crc as u32, byte) })
}

/// The state after 8 bytes.
#[cfg(target_arch = "aarch64")]
fn word_step(crc: u64, word: u64) -> u64 {
    // SAFETY: built with -C target-cpu=native on a CPU with CRC.
    u64::from(unsafe { std::arch::aarch64::__crc32cd(crc as u32, word) })
}

/// The state after one byte.
#[cfg(target_arch = "aarch64")]
fn byte_step(crc: u64, byte: u8) -> u64 {
    // SAFETY: as above.
    u64::from(unsafe { std::arch::aarch64::__crc32cb(crc as u32, byte) })
}

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
            crc = word_step(crc, word);
        }
        for &byte in words.remainder() {
            crc = byte_step(crc, byte);
        }
    }
    writeln!(std::io::stdout(), "{}", !(crc as u32)).expect("write");
}
