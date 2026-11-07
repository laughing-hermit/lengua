/* readings in C: reads the clock that only moves forward as many times as its argument says,
   from 1 to 1000000, and prints how many readings were not earlier than the previous one. */
#include <stdio.h>
#include <stdlib.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <time.h>
#endif

#include "../io.h"

int main(int argc, char **argv) {
    char *end = NULL;
    bench_binary();
    long count = argc == 2 ? strtol(argv[1], &end, 10) : 0;
    if (argc != 2 || end == argv[1] || *end != '\0' || count < 1 || count > 1000000) {
        fputs("usage: readings <count>\n", stderr);
        return 1;
    }
    long kept = 0;
#if defined(_WIN32)
    LARGE_INTEGER before, now;
    before.QuadPart = 0;
    for (long k = 0; k < count; k++) {
        if (!QueryPerformanceCounter(&now)) {
            return 3;
        }
        kept += now.QuadPart >= before.QuadPart;
        before = now;
    }
#else
    struct timespec before = {0, 0}, now;
    for (long k = 0; k < count; k++) {
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
            return 3;
        }
        kept += now.tv_sec > before.tv_sec ||
                (now.tv_sec == before.tv_sec && now.tv_nsec >= before.tv_nsec);
        before = now;
    }
#endif
    if (printf("%ld\n", kept) < 0 || fflush(stdout) != 0) {
        return 2;
    }
    return 0;
}
