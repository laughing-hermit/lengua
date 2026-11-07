// tally in C++: counts how often each word of standard input of any length occurs, and prints
// each with its count, the most frequent first, words with the same count in byte order. A word
// is a run of bytes other than whitespace, of up to 255 bytes. Standard input comes 64 KiB at a
// time, a word at a piece's end carried to the next; the words go in a std::unordered_map.
#include <algorithm>
#include <array>
#include <cstdio>
#include <cstring>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

#include "../io.h"

namespace {
constexpr std::size_t piece = 65536;
constexpr std::size_t max_word = 255;
std::array<char, max_word + piece> buffer;

bool is_space(char c) {
    auto byte = static_cast<unsigned char>(c);
    return byte == ' ' || static_cast<unsigned char>(byte - '\t') <= '\r' - '\t';
}
}  // namespace

int main() {
    bench_binary();
    std::unordered_map<std::string, unsigned long long> counts;
    std::size_t carry = 0;
    for (;;) {
        bench_ssize got = bench_read(0, buffer.data() + carry, piece);
        if (got < 0) {
            return 3;
        }
        std::size_t filled = carry + static_cast<std::size_t>(got), at = 0;
        bool ended = got == 0;
        carry = 0;
        for (;;) {
            while (at < filled && is_space(buffer[at])) {
                at++;
            }
            if (at == filled) {
                break;
            }
            std::size_t start = at;
            while (at < filled && !is_space(buffer[at])) {
                at++;
            }
            if (at - start > max_word) {
                std::fputs("tally: a word is longer than 255 bytes\n", stderr);
                return 4;
            }
            // A word that runs to the end of a piece may go on in the next.
            if (at == filled && !ended) {
                carry = filled - start;
                std::memmove(buffer.data(), buffer.data() + start, carry);
                break;
            }
            ++counts[std::string(buffer.data() + start, at - start)];
        }
        if (ended) {
            break;
        }
    }
    std::vector<std::pair<std::string_view, unsigned long long>> sorted(counts.begin(),
                                                                        counts.end());
    // string_view compares its chars as unsigned, in byte order, a prefix first.
    std::sort(sorted.begin(), sorted.end(), [](const auto& x, const auto& y) {
        return x.second != y.second ? x.second > y.second : x.first < y.first;
    });
    static char out[1 << 16];
    if (std::setvbuf(stdout, out, _IOFBF, sizeof out) != 0) {
        return 2;
    }
    for (const auto& [word, count] : sorted) {
        if (std::printf("%llu ", count) < 0 ||
            std::fwrite(word.data(), 1, word.size(), stdout) != word.size() ||
            std::putchar('\n') == EOF) {
            return 2;
        }
    }
    return std::fflush(stdout) == 0 ? 0 : 2;
}
