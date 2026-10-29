// shout in C++: prints up to 1048576 bytes of standard input with ASCII letters capitalized.
#include <algorithm>
#include <array>
#include <cstdio>
#include <unistd.h>

namespace {
constexpr std::size_t limit = 1048576;
std::array<unsigned char, limit + 1> buffer;
}  // namespace

int main() {
    std::size_t length = 0;
    for (;;) {
        ssize_t got = read(0, buffer.data() + length, buffer.size() - length);
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
    for (std::size_t done = 0; done < length;) {
        ssize_t wrote = write(1, buffer.data() + done, length - done);
        if (wrote <= 0) {
            return 2;
        }
        done += static_cast<std::size_t>(wrote);
    }
    return 0;
}
