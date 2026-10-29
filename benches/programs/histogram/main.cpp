// histogram in C++: counts each byte value of standard input and prints the values that occur,
// with their counts. Four tables, so runs of one byte don't wait on the same counter.
#include <array>
#include <cstdio>
#include <unistd.h>

namespace {
std::array<unsigned char, 1 << 16> buffer;
std::array<std::array<unsigned long long, 256>, 4> counts;
}  // namespace

int main() {
    for (;;) {
        ssize_t got = read(0, buffer.data(), buffer.size());
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        ssize_t i = 0;
        for (; i + 4 <= got; i += 4) {
            ++counts[0][buffer[i]];
            ++counts[1][buffer[i + 1]];
            ++counts[2][buffer[i + 2]];
            ++counts[3][buffer[i + 3]];
        }
        for (; i < got; i++) {
            ++counts[0][buffer[i]];
        }
    }
    for (int value = 0; value < 256; value++) {
        unsigned long long total =
            counts[0][value] + counts[1][value] + counts[2][value] + counts[3][value];
        if (total > 0) {
            std::printf("%d %llu\n", value, total);
        }
    }
    return 0;
}
