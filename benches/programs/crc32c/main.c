/* crc32c in C: the CRC-32C (Castagnoli) of standard input, in hex, with the CPU's crc32
   instruction 8 bytes at a time. */
#include <nmmintrin.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static unsigned char buffer[1 << 16];

int main(void) {
    uint64_t crc = 0xFFFFFFFF;
    for (;;) {
        ssize_t got = read(0, buffer, sizeof buffer);
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        ssize_t i = 0;
        for (; i + 8 <= got; i += 8) {
            uint64_t word;
            memcpy(&word, buffer + i, 8);
            crc = _mm_crc32_u64(crc, word);
        }
        for (; i < got; i++) {
            crc = _mm_crc32_u8((uint32_t)crc, buffer[i]);
        }
    }
    printf("%u\n", (uint32_t)~crc);
    return 0;
}
