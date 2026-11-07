//! crc32c in Rust: the CRC-32C (Castagnoli) of standard input, in decimal. Natively with the CPU's
//! crc32 instruction 8 bytes at a time: x86-64's crc32, or AArch64's crc32cx. WebAssembly has no
//! such instruction, so there it goes 8 bytes a step through eight tables (slicing by eight),
//! made when the program starts.
use std::io::{Read, Write};
use std::process::ExitCode;

/// What takes the CRC a step on: the CPU's instruction, or the tables.
struct Crc {
    /// The eight tables of slicing by eight, where there's no instruction.
    #[cfg(target_arch = "wasm32")]
    tables: Box<[[u32; 256]; 8]>,
}

impl Crc {
    #[cfg(target_arch = "wasm32")]
    fn new() -> Self {
        let mut tables = Box::new([[0_u32; 256]; 8]);
        for n in 0..256_u32 {
            let mut crc = n;
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0x82F6_3B78 & 0_u32.wrapping_sub(crc & 1));
            }
            tables[0][n as usize] = crc;
        }
        for t in 1..8 {
            for n in 0..256 {
                let previous = tables[t - 1][n];
                tables[t][n] = (previous >> 8) ^ tables[0][(previous & 0xFF) as usize];
            }
        }
        Self { tables }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new() -> Self {
        Self {}
    }

    /// The state after 8 bytes.
    #[cfg(target_arch = "wasm32")]
    fn word(&self, crc: u64, word: u64) -> u64 {
        let t = &self.tables;
        let x = (crc as u32) ^ (word as u32);
        let y = (word >> 32) as u32;
        u64::from(
            t[7][(x & 0xFF) as usize]
                ^ t[6][((x >> 8) & 0xFF) as usize]
                ^ t[5][((x >> 16) & 0xFF) as usize]
                ^ t[4][(x >> 24) as usize]
                ^ t[3][(y & 0xFF) as usize]
                ^ t[2][((y >> 8) & 0xFF) as usize]
                ^ t[1][((y >> 16) & 0xFF) as usize]
                ^ t[0][(y >> 24) as usize],
        )
    }

    /// The state after one byte.
    #[cfg(target_arch = "wasm32")]
    fn byte(&self, crc: u64, byte: u8) -> u64 {
        let low = crc as u32;
        u64::from(self.tables[0][((low ^ u32::from(byte)) & 0xFF) as usize] ^ (low >> 8))
    }

    /// The state after 8 bytes.
    #[cfg(target_arch = "x86_64")]
    fn word(&self, crc: u64, word: u64) -> u64 {
        // SAFETY: built with -C target-cpu=native on a CPU with SSE4.2.
        unsafe { std::arch::x86_64::_mm_crc32_u64(crc, word) }
    }

    /// The state after one byte.
    #[cfg(target_arch = "x86_64")]
    fn byte(&self, crc: u64, byte: u8) -> u64 {
        // SAFETY: as above.
        u64::from(unsafe { std::arch::x86_64::_mm_crc32_u8(crc as u32, byte) })
    }

    /// The state after 8 bytes.
    #[cfg(target_arch = "aarch64")]
    fn word(&self, crc: u64, word: u64) -> u64 {
        // SAFETY: every ARM64 CPU the benchmark runs on has CRC.
        u64::from(unsafe { std::arch::aarch64::__crc32cd(crc as u32, word) })
    }

    /// The state after one byte.
    #[cfg(target_arch = "aarch64")]
    fn byte(&self, crc: u64, byte: u8) -> u64 {
        // SAFETY: as above.
        u64::from(unsafe { std::arch::aarch64::__crc32cb(crc as u32, byte) })
    }
}

fn main() -> ExitCode {
    let steps = Crc::new();
    let mut input = std::io::stdin().lock();
    let mut buffer = vec![0_u8; 1 << 16];
    let mut crc: u64 = 0xFFFF_FFFF;
    loop {
        let got = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(got) => got,
            Err(_) => return ExitCode::from(2),
        };
        let mut words = buffer[..got].chunks_exact(8);
        for word in &mut words {
            let word = u64::from_le_bytes(word.try_into().expect("8 bytes"));
            crc = steps.word(crc, word);
        }
        for &byte in words.remainder() {
            crc = steps.byte(crc, byte);
        }
    }
    match writeln!(std::io::stdout(), "{}", !(crc as u32)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(3),
    }
}
