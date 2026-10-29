// wc in C++: counts the lines and words of standard input, as `wc -lw` does.
#include <array>
#include <cstdio>
#include <unistd.h>

namespace {
std::array<unsigned char, 1 << 16> buffer;

constexpr bool is_space(unsigned char c) {
    return c == ' ' || static_cast<unsigned char>(c - '\t') <= '\r' - '\t';
}
}  // namespace

int main() {
    unsigned long long lines = 0, words = 0;
    unsigned char previous = ' ';
    for (;;) {
        ssize_t got = read(0, buffer.data(), buffer.size());
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        lines += buffer[0] == '\n';
        words += !is_space(buffer[0]) && is_space(previous);
        for (ssize_t i = 1; i < got; i++) {
            lines += buffer[i] == '\n';
            words += !is_space(buffer[i]) && is_space(buffer[i - 1]);
        }
        previous = buffer[got - 1];
    }
    std::printf("%llu %llu\n", lines, words);
    return 0;
}
