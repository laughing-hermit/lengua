/* upcase-stdin in C: copies standard input of any length to standard output with ASCII letters
   capitalized, 64 KiB at a time. */
#include "../io.h"

static unsigned char buffer[1 << 16];

int main(void) {
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, buffer, sizeof buffer);
        if (got < 0) {
            return 3;
        }
        if (got == 0) {
            return 0;
        }
        for (bench_ssize i = 0; i < got; i++) {
            if (buffer[i] >= 'a' && buffer[i] <= 'z') {
                buffer[i] -= 'a' - 'A';
            }
        }
        if (bench_write_all(1, buffer, (size_t)got) != 0) {
            return 4;
        }
    }
}
