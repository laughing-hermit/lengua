/* shout in C: prints up to 1048576 bytes of standard input with ASCII letters capitalized. */
#include <stdio.h>

#include "../io.h"

#define LIMIT 1048576

static unsigned char buffer[LIMIT + 1];

int main(void) {
    size_t length = 0;
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, buffer + length, sizeof buffer - length);
        if (got < 0) {
            return 2;
        }
        length += (size_t)got;
        if (got == 0 || length == sizeof buffer) {
            break;
        }
    }
    if (length > LIMIT) {
        fputs("shout: the input is longer than 1048576 bytes\n", stderr);
        return 1;
    }
    /* Every byte is stored, a letter or not, so the compiler can vectorize the loop: a store
       under a condition would need masked stores, which WebAssembly has none of. */
    for (size_t i = 0; i < length; i++) {
        unsigned char c = buffer[i];
        buffer[i] = c >= 'a' && c <= 'z' ? (unsigned char)(c - ('a' - 'A')) : c;
    }
    return bench_write_all(1, buffer, length) == 0 ? 0 : 2;
}
