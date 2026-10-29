/* wc in C: counts the lines and words of standard input, as `wc -lw` does. Written so gcc can
   vectorize it: a word starts at a byte that isn't space after one that is. */
#include <stdio.h>
#include <unistd.h>

static unsigned char buffer[1 << 16];

static int is_space(unsigned char c) {
    return c == ' ' || (unsigned char)(c - '\t') <= '\r' - '\t';
}

int main(void) {
    unsigned long long lines = 0, words = 0;
    unsigned char previous = ' ';
    for (;;) {
        ssize_t got = read(0, buffer, sizeof buffer);
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        lines += buffer[0] == '\n';
        words += !is_space(buffer[0]) & is_space(previous);
        for (ssize_t i = 1; i < got; i++) {
            lines += buffer[i] == '\n';
            words += !is_space(buffer[i]) & is_space(buffer[i - 1]);
        }
        previous = buffer[got - 1];
    }
    printf("%llu %llu\n", lines, words);
    return 0;
}
