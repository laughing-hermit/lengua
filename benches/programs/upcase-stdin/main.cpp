// upcase-stdin in C++: copies standard input of any length to standard output with ASCII
// letters capitalized, 64 KiB at a time.
#include <algorithm>
#include <array>

#include "../io.h"

namespace {
std::array<unsigned char, 1 << 16> buffer;
}  // namespace

int main() {
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, buffer.data(), buffer.size());
        if (got < 0) {
            return 3;
        }
        if (got == 0) {
            return 0;
        }
        std::transform(buffer.begin(), buffer.begin() + got, buffer.begin(),
                       [](unsigned char c) -> unsigned char {
                           return c >= 'a' && c <= 'z' ? c - ('a' - 'A') : c;
                       });
        if (bench_write_all(1, buffer.data(), static_cast<std::size_t>(got)) != 0) {
            return 4;
        }
    }
}
