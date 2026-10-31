/* hypotenuse in C: prints the hypotenuse of two sides from 0.001 to 1000000.000, with three
   decimals, computed in doubles as the spec says. */
#include <math.h>
#include <stdio.h>

/* A side in thousandths: digits, then optionally a point and one to three digits. Returns 0,
   or the exit code for a side that isn't a number (2) or is out of range (3). */
static int parse(const char *text, long long *units) {
    long long whole = 0, fraction = 0;
    int digits = 0, places = 0;
    const char *at = text;
    for (; *at >= '0' && *at <= '9'; at++, digits++) {
        whole = whole > 1000000 ? 1000001 : whole * 10 + (*at - '0');
    }
    if (digits == 0) {
        return 2;
    }
    if (*at == '.') {
        for (at++; *at >= '0' && *at <= '9' && places < 4; at++, places++) {
            fraction = fraction * 10 + (*at - '0');
        }
        if (places == 0 || places > 3) {
            return 2;
        }
    }
    if (*at != '\0') {
        return 2;
    }
    for (; places < 3; places++) {
        fraction *= 10;
    }
    if (whole > 1000000) {
        return 3;
    }
    *units = whole * 1000 + fraction;
    return *units == 0 || *units > 1000000000LL ? 3 : 0;
}

int main(int argc, char **argv) {
    long long a, b;
    if (argc != 3) {
        fputs("usage: hypotenuse <a> <b>\n", stderr);
        return 1;
    }
    int bad = parse(argv[1], &a);
    if (bad == 0) {
        bad = parse(argv[2], &b);
    }
    if (bad == 2) {
        fputs("error: side is not a number\n", stderr);
        return 2;
    }
    if (bad == 3) {
        fputs("error: side out of range\n", stderr);
        return 3;
    }
    double x = (double)a / 1000.0, y = (double)b / 1000.0;
    long long thousandths = llrint(sqrt(x * x + y * y) * 1000.0);
    if (printf("%lld.%03lld\n", thousandths / 1000, thousandths % 1000) < 0 ||
        fflush(stdout) != 0) {
        return 4;
    }
    return 0;
}
