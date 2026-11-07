/* tally in C: counts how often each word of standard input of any length occurs, and prints each
   with its count, the most frequent first, words with the same count in byte order. A word is a
   run of bytes other than whitespace, of up to 255 bytes. Standard input comes 64 KiB at a time,
   a word at a piece's end carried to the next; the words go in a hash table, FNV-1a with linear
   probing, their bytes in a store of their own. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "../io.h"

#define PIECE 65536
#define MAX_WORD 255
#define MAX_WORDS 65536
#define SLOTS (2 * MAX_WORDS)
#define STORE (4 * 1024 * 1024)

struct entry {
    unsigned long long count;
    unsigned offset, length;
};

static unsigned char buffer[MAX_WORD + PIECE];
static unsigned char store[STORE];
static struct entry entries[MAX_WORDS];
static unsigned table[SLOTS];
static unsigned words, stored;

static int is_space(unsigned char c) {
    return c == ' ' || (unsigned char)(c - '\t') <= '\r' - '\t';
}

/* Counts the length bytes at word once more. Gives 0, or 1 when there's no room for it. */
static int add(const unsigned char *word, size_t length) {
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t k = 0; k < length; k++) {
        hash = (hash ^ word[k]) * 0x100000001b3ull;
    }
    for (size_t slot = hash & (SLOTS - 1);; slot = (slot + 1) & (SLOTS - 1)) {
        unsigned held = table[slot];
        if (held == 0) {
            if (words == MAX_WORDS || stored + length > STORE) {
                return 1;
            }
            memcpy(store + stored, word, length);
            entries[words].count = 1;
            entries[words].offset = stored;
            entries[words].length = (unsigned)length;
            table[slot] = ++words;
            stored += (unsigned)length;
            return 0;
        }
        struct entry *entry = &entries[held - 1];
        if (entry->length == length && memcmp(store + entry->offset, word, length) == 0) {
            entry->count++;
            return 0;
        }
    }
}

static int compare(const void *a, const void *b) {
    const struct entry *x = (const struct entry *)a, *y = (const struct entry *)b;
    if (x->count != y->count) {
        return x->count > y->count ? -1 : 1;
    }
    unsigned n = x->length < y->length ? x->length : y->length;
    int order = memcmp(store + x->offset, store + y->offset, n);
    if (order != 0) {
        return order;
    }
    return (x->length > y->length) - (x->length < y->length);
}

int main(void) {
    size_t carry = 0;
    bench_binary();
    for (;;) {
        bench_ssize got = bench_read(0, buffer + carry, PIECE);
        if (got < 0) {
            return 3;
        }
        size_t filled = carry + (size_t)got, at = 0;
        int ended = got == 0;
        carry = 0;
        for (;;) {
            while (at < filled && is_space(buffer[at])) {
                at++;
            }
            if (at == filled) {
                break;
            }
            size_t start = at;
            while (at < filled && !is_space(buffer[at])) {
                at++;
            }
            if (at - start > MAX_WORD) {
                fputs("tally: a word is longer than 255 bytes\n", stderr);
                return 4;
            }
            /* A word that runs to the end of a piece may go on in the next. */
            if (at == filled && !ended) {
                carry = filled - start;
                memmove(buffer, buffer + start, carry);
                break;
            }
            if (add(buffer + start, at - start) != 0) {
                fputs("tally: more words than it keeps\n", stderr);
                return 4;
            }
        }
        if (ended) {
            break;
        }
    }
    qsort(entries, words, sizeof *entries, compare);
    static char out[1 << 16];
    if (setvbuf(stdout, out, _IOFBF, sizeof out) != 0) {
        return 2;
    }
    for (unsigned k = 0; k < words; k++) {
        if (printf("%llu ", entries[k].count) < 0 ||
            fwrite(store + entries[k].offset, 1, entries[k].length, stdout) != entries[k].length ||
            putchar('\n') == EOF) {
            return 2;
        }
    }
    return fflush(stdout) == 0 ? 0 : 2;
}
