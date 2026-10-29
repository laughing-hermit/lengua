/* upcase in C: copies a file of up to 1048576 bytes to another with ASCII letters capitalized. */
#include <stdio.h>

#define LIMIT 1048576

static unsigned char buffer[LIMIT + 1];

int main(int argc, char **argv) {
    if (argc != 3) {
        fputs("usage: upcase <input> <output>\n", stderr);
        return 1;
    }
    FILE *input = fopen(argv[1], "rb");
    if (input == NULL) {
        fprintf(stderr, "upcase: cannot open %s\n", argv[1]);
        return 2;
    }
    size_t length = fread(buffer, 1, sizeof buffer, input);
    if (ferror(input)) {
        return 4;
    }
    fclose(input);
    if (length > LIMIT) {
        fprintf(stderr, "upcase: %s is larger than 1048576 bytes\n", argv[1]);
        return 3;
    }
    for (size_t i = 0; i < length; i++) {
        if (buffer[i] >= 'a' && buffer[i] <= 'z') {
            buffer[i] -= 'a' - 'A';
        }
    }
    FILE *output = fopen(argv[2], "wb");
    if (output == NULL || fwrite(buffer, 1, length, output) != length || fclose(output) != 0) {
        return 4;
    }
    return 0;
}
