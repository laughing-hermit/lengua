// upcase in C++: copies a file of up to 1048576 bytes to another with ASCII letters capitalized.
#include <algorithm>
#include <array>
#include <cstdio>

namespace {
constexpr std::size_t limit = 1048576;
std::array<unsigned char, limit + 1> buffer;
}  // namespace

int main(int argc, char** argv) {
    if (argc != 3) {
        std::fputs("usage: upcase <input> <output>\n", stderr);
        return 1;
    }
    std::FILE* input = std::fopen(argv[1], "rb");
    if (input == nullptr) {
        std::fprintf(stderr, "upcase: cannot open %s\n", argv[1]);
        return 2;
    }
    std::size_t length = std::fread(buffer.data(), 1, buffer.size(), input);
    if (std::ferror(input)) {
        return 4;
    }
    std::fclose(input);
    if (length > limit) {
        std::fprintf(stderr, "upcase: %s is larger than 1048576 bytes\n", argv[1]);
        return 3;
    }
    std::transform(buffer.begin(), buffer.begin() + length, buffer.begin(),
                   [](unsigned char c) -> unsigned char {
                       return c >= 'a' && c <= 'z' ? c - ('a' - 'A') : c;
                   });
    std::FILE* output = std::fopen(argv[2], "wb");
    if (output == nullptr || std::fwrite(buffer.data(), 1, length, output) != length ||
        std::fclose(output) != 0) {
        return 4;
    }
    return 0;
}
