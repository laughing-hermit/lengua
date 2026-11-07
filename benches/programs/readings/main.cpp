// readings in C++: reads the clock that only moves forward as many times as its argument says,
// from 1 to 1000000, and prints how many readings were not earlier than the previous one.
#include <charconv>
#include <chrono>
#include <cstdio>
#include <string_view>

#include "../io.h"

int main(int argc, char** argv) {
    bench_binary();
    long count = 0;
    if (argc == 2) {
        std::string_view text(argv[1]);
        auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
        if (error != std::errc{} || end != text.data() + text.size()) {
            count = 0;
        }
    }
    if (count < 1 || count > 1000000) {
        std::fputs("usage: readings <count>\n", stderr);
        return 1;
    }
    long kept = 0;
    auto before = std::chrono::steady_clock::time_point::min();
    for (long k = 0; k < count; k++) {
        auto now = std::chrono::steady_clock::now();
        kept += now >= before;
        before = now;
    }
    if (std::printf("%ld\n", kept) < 0 || std::fflush(stdout) != 0) {
        return 2;
    }
    return 0;
}
