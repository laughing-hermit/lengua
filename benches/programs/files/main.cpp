// files in C++: reads every file in the current folder to its end and prints how many files
// there are and how many bytes they hold. Folders are neither counted nor read. Windows lists a
// folder with FindFirstFile; Linux, macOS and WASI with opendir.
#include <array>
#include <cstdio>
#if defined(_WIN32)
#include <windows.h>
#else
#include <dirent.h>
#endif

#include "../io.h"

namespace {
std::array<unsigned char, 1 << 16> buffer;

// The bytes the file named name holds, read to its end, or -1 when it can't be read.
long long size_of(const char* name) {
    std::FILE* file = std::fopen(name, "rb");
    if (file == nullptr) {
        return -1;
    }
    long long total = 0;
    std::size_t got;
    while ((got = std::fread(buffer.data(), 1, buffer.size(), file)) > 0) {
        total += static_cast<long long>(got);
    }
    bool failed = std::ferror(file) != 0;
    std::fclose(file);
    return failed ? -1 : total;
}
}  // namespace

int main() {
    bench_binary();
    unsigned long long files = 0, bytes = 0;
#if defined(_WIN32)
    WIN32_FIND_DATAA found;
    HANDLE search = FindFirstFileA("*", &found);
    if (search != INVALID_HANDLE_VALUE) {
        do {
            if (found.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
                continue;
            }
            long long size = size_of(found.cFileName);
            if (size < 0) {
                FindClose(search);
                return 3;
            }
            files++;
            bytes += static_cast<unsigned long long>(size);
        } while (FindNextFileA(search, &found));
        FindClose(search);
    }
#else
    DIR* folder = opendir(".");
    if (folder == nullptr) {
        return 3;
    }
    while (dirent* entry = readdir(folder)) {
        if (entry->d_type != DT_REG) {
            continue;
        }
        long long size = size_of(entry->d_name);
        if (size < 0) {
            closedir(folder);
            return 3;
        }
        files++;
        bytes += static_cast<unsigned long long>(size);
    }
    closedir(folder);
#endif
    if (std::printf("%llu %llu\n", files, bytes) < 0 || std::fflush(stdout) != 0) {
        return 2;
    }
    return 0;
}
