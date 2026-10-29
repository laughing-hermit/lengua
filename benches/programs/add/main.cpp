// add in C++: parses two whole numbers from -1000000000 to 1000000000 and prints their sum.
#include <charconv>
#include <cstdio>
#include <optional>
#include <string_view>

namespace {
std::optional<long long> parse(std::string_view text) {
    long long number = 0;
    auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
    if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
        number > 1000000000LL) {
        return std::nullopt;
    }
    return number;
}
}  // namespace

int main(int argc, char** argv) {
    if (argc != 3) {
        std::fputs("usage: add <a> <b>\n", stderr);
        return 1;
    }
    auto a = parse(argv[1]);
    auto b = parse(argv[2]);
    if (!a || !b) {
        std::fputs("error: not a number\n", stderr);
        return 2;
    }
    if (std::printf("%lld\n", *a + *b) < 0 || std::fflush(stdout) != 0) {
        return 3;
    }
    return 0;
}
