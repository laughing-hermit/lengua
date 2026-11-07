// crc32c in C++: the CRC-32C (Castagnoli) of standard input, in decimal. Natively with the CPU's
// crc32 instruction 8 bytes at a time: x86-64's crc32, or AArch64's crc32cx. WebAssembly has no
// such instruction, so there it goes 8 bytes a step through eight tables (slicing by eight),
// made when the program starts.
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>

#include "../io.h"

#if defined(__wasm__)
namespace {
std::array<std::array<std::uint32_t, 256>, 8> tables;

void make_tables() {
    for (std::uint32_t n = 0; n < 256; n++) {
        std::uint32_t crc = n;
        for (int k = 0; k < 8; k++) {
            crc = (crc >> 1) ^ (0x82F63B78u & (0u - (crc & 1u)));
        }
        tables[0][n] = crc;
    }
    for (std::size_t t = 1; t < 8; t++) {
        for (std::uint32_t n = 0; n < 256; n++) {
            std::uint32_t previous = tables[t - 1][n];
            tables[t][n] = (previous >> 8) ^ tables[0][previous & 0xFF];
        }
    }
}

std::uint64_t word_step(std::uint64_t crc, std::uint64_t word) {
    auto x = static_cast<std::uint32_t>(crc) ^ static_cast<std::uint32_t>(word);
    auto y = static_cast<std::uint32_t>(word >> 32);
    return tables[7][x & 0xFF] ^ tables[6][(x >> 8) & 0xFF] ^ tables[5][(x >> 16) & 0xFF] ^
           tables[4][x >> 24] ^ tables[3][y & 0xFF] ^ tables[2][(y >> 8) & 0xFF] ^
           tables[1][(y >> 16) & 0xFF] ^ tables[0][y >> 24];
}

std::uint64_t byte_step(std::uint64_t crc, unsigned char byte) {
    auto low = static_cast<std::uint32_t>(crc);
    return tables[0][(low ^ byte) & 0xFF] ^ (low >> 8);
}
}  // namespace
#elif defined(__aarch64__) || defined(_M_ARM64)
#include <arm_acle.h>
namespace {
std::uint64_t word_step(std::uint64_t crc, std::uint64_t word) {
    return __crc32cd(static_cast<std::uint32_t>(crc), word);
}
std::uint64_t byte_step(std::uint64_t crc, unsigned char byte) {
    return __crc32cb(static_cast<std::uint32_t>(crc), byte);
}
}  // namespace
#else
#include <nmmintrin.h>
namespace {
std::uint64_t word_step(std::uint64_t crc, std::uint64_t word) { return _mm_crc32_u64(crc, word); }
std::uint64_t byte_step(std::uint64_t crc, unsigned char byte) {
    return _mm_crc32_u8(static_cast<std::uint32_t>(crc), byte);
}
}  // namespace
#endif

namespace {
std::array<unsigned char, 1 << 16> buffer;
}  // namespace

int main() {
    bench_binary();
#if defined(__wasm__)
    make_tables();
#endif
    std::uint64_t crc = 0xFFFFFFFF;
    for (;;) {
        bench_ssize got = bench_read(0, buffer.data(), buffer.size());
        if (got < 0) {
            return 2;
        }
        if (got == 0) {
            break;
        }
        bench_ssize i = 0;
        for (; i + 8 <= got; i += 8) {
            std::uint64_t word;
            std::memcpy(&word, buffer.data() + i, 8);
            crc = word_step(crc, word);
        }
        for (; i < got; i++) {
            crc = byte_step(crc, buffer[i]);
        }
    }
    if (std::printf("%u\n", static_cast<unsigned>(static_cast<std::uint32_t>(~crc))) < 0 ||
        std::fflush(stdout) != 0) {
        return 3;
    }
    return 0;
}
