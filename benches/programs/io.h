/* io.h: what the C and C++ versions of the benchmark's programs read and write with, the same on
   every system they're built for. Linux, macOS and WASI have read and write in unistd.h; Windows
   has _read and _write in io.h, and opens its standard streams as text, which turns each newline
   into two bytes, until they're set to binary. */
#ifndef BENCH_IO_H
#define BENCH_IO_H

#include <stddef.h>
#include <stdio.h>

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>

typedef long long bench_ssize;

/* The most one call moves on Windows, whose calls take a count of 32 bits. */
#define BENCH_MOST 0x40000000u

static bench_ssize bench_read(int fd, void *buffer, size_t count) {
    return _read(fd, buffer, (unsigned)(count > BENCH_MOST ? BENCH_MOST : count));
}

static bench_ssize bench_write(int fd, const void *buffer, size_t count) {
    return _write(fd, buffer, (unsigned)(count > BENCH_MOST ? BENCH_MOST : count));
}

/* Standard input and output as bytes, as on every other system. */
static void bench_binary(void) {
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);
}
#else
#include <unistd.h>

typedef ssize_t bench_ssize;

static bench_ssize bench_read(int fd, void *buffer, size_t count) {
    return read(fd, buffer, count);
}

static bench_ssize bench_write(int fd, const void *buffer, size_t count) {
    return write(fd, buffer, count);
}

/* Standard input and output are bytes already. */
static void bench_binary(void) {}
#endif

/* Writes all of the count bytes at buffer to fd, however many writes it takes. Gives 0, or -1
   when a write fails. */
static int bench_write_all(int fd, const void *buffer, size_t count) {
    const unsigned char *at = (const unsigned char *)buffer;
    while (count > 0) {
        bench_ssize wrote = bench_write(fd, at, count);
        if (wrote <= 0) {
            return -1;
        }
        at += wrote;
        count -= (size_t)wrote;
    }
    return 0;
}

#endif
