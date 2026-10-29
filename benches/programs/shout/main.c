/* shout in C: prints up to 1048576 bytes of standard input with ASCII letters capitalized. */
#include <stdio.h>
#include <unistd.h>

#define LIMIT 1048576

static unsigned char buffer[LIMIT + 1];

int main(void) {
    size_t length = 0;
    for (;;) {
        ssize_t got = read(0, buffer + length, sizeof buffer - length);
        if (got < 0) {
            return 2;
        }
        if (got == 0 || length + (size_t)got == sizeof buffer) {
            length += (size_t)got;
            break;
        }
        length += (size_t)got;
    }
    if (length > LIMIT) {
        fputs("shout: the input is longer than 1048576 bytes\n", stderr);
        return 1;
    }
    for (size_t i = 0; i < length; i++) {
        if (buffer[i] >= 'a' && buffer[i] <= 'z') {
            buffer[i] -= 'a' - 'A';
        }
    }
    for (size_t done = 0; done < length;) {
        ssize_t wrote = write(1, buffer + done, length - done);
        if (wrote <= 0) {
            return 2;
        }
        done += (size_t)wrote;
    }
    return 0;
}
