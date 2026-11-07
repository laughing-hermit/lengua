// shout in C++: prints up to 1048576 bytes of standard input with ASCII letters capitalized.
#include <algorithm>
#include <array>
#include <cstdio>

#include "../io.h"

namespace {
constexpr std::size_t limit = 1048576;
std::array<unsigned char, limit + 1> buffer;
}  // namespace

int main() {
    bench_binary();
    std::size_t length = 0;
    for (;;) {
        bench_ssize got = bench_read(0, buffer.data() + length, buffer.size() - length);
        if (got < 0) {
            return 2;
        }
        length += static_cast<std::size_t>(got);
        if (got == 0 || length == buffer.size()) {
            break;
        }
    }
    if (length > limit) {
        std::fputs("shout: the input is longer than 1048576 bytes\n", stderr);
        return 1;
    }
    std::transform(buffer.begin(), buffer.begin() + length, buffer.begin(),
                   [](unsigned char c) -> unsigned char {
                       return c >= 'a' && c <= 'z' ? c - ('a' - 'A') : c;
                   });
    return bench_write_all(1, buffer.data(), length) == 0 ? 0 : 2;
}
