/* crc32c in C: the CRC-32C (Castagnoli) of standard input, in decimal. Natively with the CPU's
   crc32 instruction 8 bytes at a time: x86-64's crc32, or AArch64's crc32cx. WebAssembly has no
   such instruction, so there it goes 8 bytes a step through eight tables (slicing by eight), made
   when the program starts. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "../io.h"

#if defined(__wasm__)
static uint32_t tables[8][256];

static void make_tables(void) {
    for (uint32_t n = 0; n < 256; n++) {
        uint32_t crc = n;
        for (int k = 0; k < 8; k++) {
            crc = (crc >> 1) ^ (0x82F63B78u & (0u - (crc & 1u)));
        }
        tables[0][n] = crc;
    }
    for (int t = 1; t < 8; t++) {
        for (uint32_t n = 0; n < 256; n++) {
            uint32_t previous = tables[t - 1][n];
            tables[t][n] = (previous >> 8) ^ tables[0][previous & 0xFF];
        }
    }
}

static uint64_t crc_word(uint64_t crc, uint64_t word) {
    uint32_t x = (uint32_t)crc ^ (uint32_t)word, y = (uint32_t)(word >> 32);
    return tables[7][x & 0xFF] ^ tables[6][(x >> 8) & 0xFF] ^ tables[5][(x >> 16) & 0xFF] ^
           tables[4][x >> 24] ^ tables[3][y & 0xFF] ^ tables[2][(y >> 8) & 0xFF] ^
           tables[1][(y >> 16) & 0xFF] ^ tables[0][y >> 24];
}

static uint64_t crc_byte(uint64_t crc, unsigned char byte) {
    return tables[0][((uint32_t)crc ^ byte) & 0xFF] ^ ((uint32_t)crc >> 8);
}

#define CRC_WORD(crc, word) crc_word((crc), (word))
#define CRC_BYTE(crc, byte) crc_byte((crc), (byte))
#elif defined(__aarch64__) || defined(_M_ARM64)
#include <arm_acle.h>
#define CRC_WORD(crc, word) __crc32cd((uint32_t)(crc), (word))
#define CRC_BYTE(crc, byte) __crc32cb((uint32_t)(crc), (byte))
#else
#include <nmmintrin.h>
#define CRC_WORD(crc, word) _mm_crc32_u64((crc), (word))
#define CRC_BYTE(crc, byte) _mm_crc32_u8((uint32_t)(crc), (byte))
#endif

static unsigned char buffer[1 << 16];

int main(void) {
    uint64_t crc = 0xFFFFFFFF;
    bench_binary();
#if defined(__wasm__)
    make_tables();
#endif
    for (;;) {
        bench_ssize got = bench_read(0, buffer, sizeof buffer);
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        bench_ssize i = 0;
        for (; i + 8 <= got; i += 8) {
            uint64_t word;
            memcpy(&word, buffer + i, 8);
            crc = CRC_WORD(crc, word);
        }
        for (; i < got; i++) {
            crc = CRC_BYTE(crc, buffer[i]);
        }
    }
    if (printf("%u\n", (unsigned)(uint32_t)~crc) < 0 || fflush(stdout) != 0) {
        return 3;
    }
    return 0;
}
