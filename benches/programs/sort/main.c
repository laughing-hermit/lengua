/* sort in C: prints the lines of standard input of up to 67108864 bytes in byte order, as
   LC_ALL=C sort does, a line that starts another first, and a last line without a newline given
   one. The lines are pointers into the input, sorted with qsort. */
#include <stdlib.h>
#include <string.h>

#include "../io.h"

#define LIMIT 67108864

/* The input, one byte past the limit, which shows a longer input, and one for a newline. */
static unsigned char input[LIMIT + 2];

struct line {
    const unsigned char *start;
    size_t length;
};

static int compare(const void *a, const void *b) {
    const struct line *x = (const struct line *)a, *y = (const struct line *)b;
    size_t n = x->length < y->length ? x->length : y->length;
    int order = memcmp(x->start, y->start, n);
    if (order != 0) {
        return order;
    }
    return (x->length > y->length) - (x->length < y->length);
}

int main(void) {
    size_t length = 0;
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, input + length, LIMIT + 1 - length);
        if (got < 0) {
            return 3;
        }
        length += (size_t)got;
        if (got == 0 || length == LIMIT + 1) {
            break;
        }
    }
    if (length > LIMIT) {
        fputs("sort: the input is longer than 67108864 bytes\n", stderr);
        return 4;
    }
    if (length > 0 && input[length - 1] != '\n') {
        input[length++] = '\n';
    }
    size_t count = 0;
    for (const unsigned char *at = input, *end = input + length; at < end;) {
        const unsigned char *newline = memchr(at, '\n', (size_t)(end - at));
        count++;
        at = newline + 1;
    }
    struct line *lines = malloc((count > 0 ? count : 1) * sizeof *lines);
    if (lines == NULL) {
        return 4;
    }
    size_t n = 0;
    for (const unsigned char *at = input, *end = input + length; at < end; n++) {
        const unsigned char *newline = memchr(at, '\n', (size_t)(end - at));
        lines[n].start = at;
        lines[n].length = (size_t)(newline - at);
        at = newline + 1;
    }
    qsort(lines, count, sizeof *lines, compare);
    static char out[1 << 16];
    if (setvbuf(stdout, out, _IOFBF, sizeof out) != 0) {
        return 2;
    }
    for (size_t k = 0; k < count; k++) {
        if (fwrite(lines[k].start, 1, lines[k].length + 1, stdout) != lines[k].length + 1) {
            return 2;
        }
    }
    return fflush(stdout) == 0 ? 0 : 2;
}
