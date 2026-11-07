// tally in C++: counts how often each word of standard input of any length occurs, and prints
// each with its count, the most frequent first, words with the same count in byte order. A word
// is a run of bytes other than whitespace, of up to 255 bytes. Standard input comes 64 KiB at a
// time, a word at a piece's end carried to the next; the words go in a hash table, FNV-1a with
// linear probing, as C's and lengua's do, their bytes in a store of their own, and each word is
// looked up as a std::string_view of the input, with no string made for it.
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string_view>
#include <vector>

#include "../io.h"

namespace {
constexpr std::size_t piece = 65536;
constexpr std::size_t max_word = 255;
constexpr std::size_t max_words = 65536;
constexpr std::size_t slots = 2 * max_words;
constexpr std::size_t store_bytes = 4 * 1024 * 1024;

struct Entry {
    unsigned long long count;
    std::uint32_t offset, length;
};

std::array<char, max_word + piece> buffer;
// The words' bytes, one after another.
std::array<char, store_bytes> store;
std::size_t stored = 0;
// Each slot 0, or the number from 1 of the entry it holds.
std::array<std::uint32_t, slots> table;
std::vector<Entry> entries;

bool is_space(char c) {
    auto byte = static_cast<unsigned char>(c);
    return byte == ' ' || static_cast<unsigned char>(byte - '\t') <= '\r' - '\t';
}

std::string_view word_of(const Entry& entry) {
    return {store.data() + entry.offset, entry.length};
}

// Counts `word` once more. Gives false when there's no room for it.
bool add(std::string_view word) {
    std::uint64_t hash = 0xcbf29ce484222325ull;
    for (char c : word) {
        hash = (hash ^ static_cast<unsigned char>(c)) * 0x100000001b3ull;
    }
    for (std::size_t slot = hash & (slots - 1);; slot = (slot + 1) & (slots - 1)) {
        std::uint32_t held = table[slot];
        if (held == 0) {
            if (entries.size() == max_words || stored + word.size() > store.size()) {
                return false;
            }
            std::memcpy(store.data() + stored, word.data(), word.size());
            entries.push_back(Entry{1, static_cast<std::uint32_t>(stored),
                                    static_cast<std::uint32_t>(word.size())});
            table[slot] = static_cast<std::uint32_t>(entries.size());
            stored += word.size();
            return true;
        }
        Entry& entry = entries[held - 1];
        if (word_of(entry) == word) {
            entry.count++;
            return true;
        }
    }
}
}  // namespace

int main() {
    bench_binary();
    entries.reserve(max_words);
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
            if (!add(std::string_view(buffer.data() + start, at - start))) {
                std::fputs("tally: more words than it keeps\n", stderr);
                return 4;
            }
        }
        if (ended) {
            break;
        }
    }
    // string_view compares its chars as unsigned, in byte order, a prefix first.
    std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
        return x.count != y.count ? x.count > y.count : word_of(x) < word_of(y);
    });
    static char out[1 << 16];
    if (std::setvbuf(stdout, out, _IOFBF, sizeof out) != 0) {
        return 2;
    }
    for (const Entry& entry : entries) {
        std::string_view word = word_of(entry);
        if (std::printf("%llu ", entry.count) < 0 ||
            std::fwrite(word.data(), 1, word.size(), stdout) != word.size() ||
            std::putchar('\n') == EOF) {
            return 2;
        }
    }
    return std::fflush(stdout) == 0 ? 0 : 2;
}
