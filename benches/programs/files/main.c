/* files in C: reads every file in the current folder to its end and prints how many files there
   are and how many bytes they hold. Folders are neither counted nor read. Windows lists a folder
   with FindFirstFile; Linux, macOS and WASI with opendir. */
#include <stdio.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <dirent.h>
#endif

#include "../io.h"

static unsigned char buffer[1 << 16];

/* The bytes the file named name holds, read to its end, or -1 when it can't be read. */
static long long size_of(const char *name) {
    FILE *file = fopen(name, "rb");
    if (file == NULL) {
        return -1;
    }
    long long total = 0;
    size_t got;
    while ((got = fread(buffer, 1, sizeof buffer, file)) > 0) {
        total += (long long)got;
    }
    int failed = ferror(file);
    fclose(file);
    return failed ? -1 : total;
}

int main(void) {
    unsigned long long files = 0, bytes = 0;
    bench_binary();
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
            bytes += (unsigned long long)size;
        } while (FindNextFileA(search, &found));
        FindClose(search);
    }
#else
    DIR *folder = opendir(".");
    if (folder == NULL) {
        return 3;
    }
    struct dirent *entry;
    while ((entry = readdir(folder)) != NULL) {
        if (entry->d_type != DT_REG) {
            continue;
        }
        long long size = size_of(entry->d_name);
        if (size < 0) {
            closedir(folder);
            return 3;
        }
        files++;
        bytes += (unsigned long long)size;
    }
    closedir(folder);
#endif
    if (printf("%llu %llu\n", files, bytes) < 0 || fflush(stdout) != 0) {
        return 2;
    }
    return 0;
}
