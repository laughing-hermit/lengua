// sort in C++: prints the lines of standard input of up to 67108864 bytes in byte order, as
// LC_ALL=C sort does, a line that starts another first, and a last line without a newline given
// one. The lines are views into the input, sorted with std::sort.
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <string_view>
#include <vector>

#include "../io.h"

namespace {
constexpr std::size_t limit = 67108864;
// The input, one byte past the limit, which shows a longer input, and one for a newline: static,
// so the pages no input reaches are never touched.
char input[limit + 2];
}  // namespace

int main() {
    bench_binary();
    std::size_t length = 0;
    for (;;) {
        bench_ssize got = bench_read(0, input + length, limit + 1 - length);
        if (got < 0) {
            return 3;
        }
        length += static_cast<std::size_t>(got);
        if (got == 0 || length == limit + 1) {
            break;
        }
    }
    if (length > limit) {
        std::fputs("sort: the input is longer than 67108864 bytes\n", stderr);
        return 4;
    }
    if (length > 0 && input[length - 1] != '\n') {
        input[length++] = '\n';
    }
    std::vector<std::string_view> lines;
    std::string_view rest(input, length);
    while (!rest.empty()) {
        std::size_t newline = rest.find('\n');
        lines.push_back(rest.substr(0, newline));
        rest.remove_prefix(newline + 1);
    }
    // string_view compares its chars as unsigned, in byte order, a prefix first.
    std::sort(lines.begin(), lines.end());
    static char out[1 << 16];
    if (std::setvbuf(stdout, out, _IOFBF, sizeof out) != 0) {
        return 2;
    }
    for (std::string_view line : lines) {
        if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
            return 2;
        }
    }
    return std::fflush(stdout) == 0 ? 0 : 2;
}
