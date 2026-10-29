// crc32c in C++: the CRC-32C (Castagnoli) of standard input, in hex, with the CPU's crc32
// instruction 8 bytes at a time.
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <nmmintrin.h>
#include <unistd.h>

namespace {
std::array<unsigned char, 1 << 16> buffer;
}  // namespace

int main() {
    std::uint64_t crc = 0xFFFFFFFF;
    for (;;) {
        ssize_t got = read(0, buffer.data(), buffer.size());
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        ssize_t i = 0;
        for (; i + 8 <= got; i += 8) {
            std::uint64_t word;
            std::memcpy(&word, buffer.data() + i, 8);
            crc = _mm_crc32_u64(crc, word);
        }
        for (; i < got; i++) {
            crc = _mm_crc32_u8(static_cast<std::uint32_t>(crc), buffer[i]);
        }
    }
    std::printf("%u\n", static_cast<std::uint32_t>(~crc));
    return 0;
}
