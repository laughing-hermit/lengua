/* add in C: parses two whole numbers from -1000000000 to 1000000000 and prints their sum. */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>

static int parse(const char *text, long long *value) {
    char *end;
    errno = 0;
    long long number = strtoll(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' || number < -1000000000LL ||
        number > 1000000000LL) {
        return 0;
    }
    *value = number;
    return 1;
}

int main(int argc, char **argv) {
    long long a, b;
    if (argc != 3) {
        fputs("usage: add <a> <b>\n", stderr);
        return 1;
    }
    if (!parse(argv[1], &a) || !parse(argv[2], &b)) {
        fputs("error: not a number\n", stderr);
        return 2;
    }
    if (printf("%lld\n", a + b) < 0 || fflush(stdout) != 0) {
        return 3;
    }
    return 0;
}
