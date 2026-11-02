# lengua against C, C++ and Rust on ARM64

Measured on an ARM64 computer by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best C, C++ or Rust version takes than the best lengua version.

- CPU: CPU implementer 0x41, part 0xd49 (2 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C and C++: `gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0`, `gcc -O3 -march=native -flto -s` and `g++ -O3 -march=native -flto -s`
- C, C++ and Rust link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 446 | 196,297 | 88,012 | 85,267 | 191.2× |
| cycles | 1,241 | 207,885 | 124,647 | 122,140 | 98.4× |
| CPU time | 61 µs | 374 µs | 271 µs | 266 µs | 4.3× |
| wall time | 232 µs | 613 µs | 501 µs | 487 µs | 2.1× |
| peak memory | 272 KiB | 1,556 KiB | 1,376 KiB | 1,384 KiB | 5.1× |
| file size | 2,928 B | 2047 KiB | 66 KiB | 66 KiB | 23.1× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 607 | 198,215 | 111,906 | 112,695 | 184.4× |
| cycles | 1,573 | 211,881 | 149,115 | 152,108 | 94.8× |
| CPU time | 63 µs | 385 µs | 307 µs | 308 µs | 4.9× |
| wall time | 238 µs | 617 µs | 533 µs | 537 µs | 2.2× |
| peak memory | 272 KiB | 1,552 KiB | 1,512 KiB | 1,512 KiB | 5.6× |
| file size | 3,640 B | 2053 KiB | 66 KiB | 66 KiB | 18.5× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the file program, a byte at a time with no branch, and the same program 16 bytes at a time with NEON.

| | lengua | lengua, NEON | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 13,631,626 | 917,647 | 688,084 | 677,631 | 677,633 | 0.7× |
| cycles | 2,762,419 | 201,108 | 379,832 | 303,484 | 293,830 | 1.5× |
| CPU time | 1586 µs | 798 µs | 1190 µs | 994 µs | 999 µs | 1.2× |
| wall time | 5394 µs | 5355 µs | 5358 µs | 5340 µs | 5335 µs | 1.0× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,532 KiB | 2,152 KiB | 2,140 KiB | 2.1× |
| file size | 4,248 B | 4,432 B | 2030 KiB | 66 KiB | 66 KiB | 15.9× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's version takes 16 bytes a step with NEON on one thread, since threads on AArch64 come later; Rust is shown both plain and with scoped threads.

| | lengua, NEON, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 917,808 | 688,222 | 732,943 | 671,385 | 671,376 | 0.7× |
| cycles | 201,882 | 384,523 | 573,868 | 276,630 | 262,193 | 1.3× |
| CPU time | 956 µs | 1333 µs | 1761 µs | 1118 µs | 1098 µs | 1.1× |
| wall time | 2039 µs | 2405 µs | 2749 µs | 2214 µs | 2199 µs | 1.1× |
| peak memory | 1,024 KiB | 2,404 KiB | 2,704 KiB | 2,024 KiB | 2,024 KiB | 2.0× |
| file size | 2,744 B | 2054 KiB | 2335 KiB | 66 KiB | 66 KiB | 24.6× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's version takes 16 bytes a step with NEON, each step masked to the bytes left.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 134,281,426 | 256,360,226 | 218,297,594 | 427,992,538 | 1.6× |
| cycles | 46,433,218 | 130,015,152 | 117,821,495 | 206,179,031 | 2.5× |
| CPU time | 29954 µs | 50059 µs | 46491 µs | 72468 µs | 1.6× |
| wall time | 30683 µs | 50903 µs | 47275 µs | 73283 µs | 1.5× |
| peak memory | 272 KiB | 1,508 KiB | 1,360 KiB | 1,352 KiB | 5.0× |
| file size | 3,144 B | 2058 KiB | 66 KiB | 66 KiB | 21.5× |

## histogram

Not measured here: lengua's AArch64 code doesn't do it yet.

## crc32c

Not measured here: lengua's AArch64 code doesn't do it yet.
