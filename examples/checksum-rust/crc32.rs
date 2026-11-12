//! crc32: the CRC-32 that zlib, gzip and PNG compute, with a table of 256 numbers, as a library
//! that a lengua program calls (examples/checksum-rust). `cargo xtask outside-wasm` compiles it
//! with rustc for wasm32-wasip1 as a `cdylib`, and CI's suite then checks and runs the lengua
//! program with it (tests/calls.rs).
//!
//! It is a called module, so it has no `main`, uses no standard library and imports nothing:
//! lengua gives a called module no function of its host. The caller copies its bytes into the
//! module's buffer, at the address `buffer` gives, and then calls `crc32` with their number.
//! lengua's rules for main.wat hold nothing of this code: it is trusted, not proven.

#![no_std]

use core::cell::UnsafeCell;

/// The most bytes one call takes.
const TEXT: usize = 4096;

/// The bytes the caller copies in.
struct Buffer(UnsafeCell<[u8; TEXT]>);

// SAFETY: a WebAssembly module without threads runs on one thread.
unsafe impl Sync for Buffer {}

static BUFFER: Buffer = Buffer(UnsafeCell::new([0; TEXT]));

/// The CRC-32 of each byte value: the polynomial 0xedb88320, eight bits at a time.
const fn table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[byte] = crc;
        byte += 1;
    }
    table
}

static TABLE: [u32; 256] = table();

/// The address of the buffer, of 4096 bytes, in this module's own memory: where the caller
/// copies the bytes to.
#[unsafe(no_mangle)]
pub extern "C" fn buffer() -> *mut u8 {
    BUFFER.0.get().cast()
}

/// The CRC-32 of the first `length` bytes of the buffer, and of all 4096 when `length` is more.
#[unsafe(no_mangle)]
pub extern "C" fn crc32(length: u32) -> u32 {
    let length = if (length as usize) < TEXT {
        length as usize
    } else {
        TEXT
    };
    // SAFETY: one thread, and nothing writes the buffer while this function reads it.
    let text = unsafe { &*BUFFER.0.get() };
    let mut crc = u32::MAX;
    let mut at = 0;
    while at < length {
        crc = TABLE[((crc ^ text[at] as u32) & 0xff) as usize] ^ (crc >> 8);
        at += 1;
    }
    !crc
}

/// A `no_std` crate says what a panic does: this code has none, and one would stop here.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
