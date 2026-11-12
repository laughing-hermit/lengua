/* crc32: the CRC-32 that zlib, gzip and PNG compute, a bit at a time, as a library that a
   lengua program calls. `cargo xtask outside-wasm` compiles it with wasi-sdk's clang, with no
   standard library and no entry, and lengua's suite then checks and runs a lengua program with
   it (tests/calls.rs). It has the functions of examples/checksum-rust's crc32.rs, so the same
   main.wat calls either. A called module imports nothing: lengua gives it no function of its
   host. */
#include <stdint.h>

/* The most bytes one call takes. */
#define TEXT 4096

static uint8_t text[TEXT];

/* The address of the buffer, in this module's own memory: where the caller copies the bytes. */
__attribute__((export_name("buffer")))
uint8_t *buffer(void) {
    return text;
}

/* The CRC-32 of the first `length` bytes of the buffer, and of all 4096 when it is more. */
__attribute__((export_name("crc32")))
uint32_t crc32(uint32_t length) {
    uint32_t crc = 0xffffffffu;
    if (length > TEXT) {
        length = TEXT;
    }
    for (uint32_t at = 0; at < length; at++) {
        crc ^= text[at];
        for (int bit = 0; bit < 8; bit++) {
            crc = (crc >> 1) ^ (0xedb88320u & (0u - (crc & 1u)));
        }
    }
    return ~crc;
}
