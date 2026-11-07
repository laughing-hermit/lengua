/* wc in C: counts the lines and words of standard input, as `wc -lw` does. Written so the
   compiler can vectorize it: a word starts at a byte that isn't space after one that is, and the
   bytes are counted 255 at a time in counters of a byte, which a vector holds 16 of, before each
   block's counts go to the totals. */
#include <stdio.h>

#include "../io.h"

/* The most bytes a counter of a byte counts before it's added to its total. */
#define BLOCK 255

static unsigned char buffer[1 << 16];

static int is_space(unsigned char c) {
    return c == ' ' || (unsigned char)(c - '\t') <= '\r' - '\t';
}

int main(void) {
    unsigned long long lines = 0, words = 0;
    unsigned char previous = ' ';
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, buffer, sizeof buffer);
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        lines += buffer[0] == '\n';
        words += !is_space(buffer[0]) & is_space(previous);
        for (bench_ssize start = 1; start < got;) {
            bench_ssize end = got - start < BLOCK ? got : start + BLOCK;
            unsigned char block_lines = 0, block_words = 0;
            for (bench_ssize i = start; i < end; i++) {
                block_lines += buffer[i] == '\n';
                block_words += !is_space(buffer[i]) & is_space(buffer[i - 1]);
            }
            lines += block_lines;
            words += block_words;
            start = end;
        }
        previous = buffer[got - 1];
    }
    if (printf("%llu %llu\n", lines, words) < 0 || fflush(stdout) != 0) {
        return 3;
    }
    return 0;
}
