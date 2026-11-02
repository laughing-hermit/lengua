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
| instructions | 446 | 196,356 | 88,012 | 85,267 | 191.2× |
| cycles | 1,120 | 213,556 | 128,613 | 127,535 | 113.9× |
| CPU time | 64 µs | 377 µs | 271 µs | 280 µs | 4.2× |
| wall time | 253 µs | 614 µs | 503 µs | 550 µs | 2.0× |
| peak memory | 272 KiB | 1,444 KiB | 1,384 KiB | 1,384 KiB | 5.1× |
| file size | 2,928 B | 2047 KiB | 66 KiB | 66 KiB | 23.1× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 607 | 198,307 | 111,906 | 112,695 | 184.4× |
| cycles | 1,582 | 212,066 | 157,514 | 158,132 | 99.6× |
| CPU time | 65 µs | 375 µs | 316 µs | 306 µs | 4.7× |
| wall time | 240 µs | 625 µs | 549 µs | 555 µs | 2.3× |
| peak memory | 272 KiB | 1,444 KiB | 1,576 KiB | 1,576 KiB | 5.3× |
| file size | 3,640 B | 2053 KiB | 66 KiB | 66 KiB | 18.5× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the file program, a byte at a time with no branch, and the same program 16 bytes at a time with NEON.

| | lengua | lengua, NEON | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 13,631,626 | 917,647 | 688,072 | 677,631 | 677,633 | 0.7× |
| cycles | 2,760,790 | 201,552 | 391,353 | 311,399 | 298,049 | 1.5× |
| CPU time | 1623 µs | 866 µs | 1249 µs | 1021 µs | 1019 µs | 1.2× |
| wall time | 6194 µs | 5245 µs | 5301 µs | 5288 µs | 5306 µs | 1.0× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,532 KiB | 2,152 KiB | 2,152 KiB | 2.1× |
| file size | 4,248 B | 4,432 B | 2030 KiB | 66 KiB | 66 KiB | 15.9× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's version takes 16 bytes a step with NEON on one thread, since threads on AArch64 come later; Rust is shown both plain and with scoped threads.

| | lengua, NEON, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 917,808 | 688,378 | 732,873 | 671,385 | 671,376 | 0.7× |
| cycles | 202,274 | 388,594 | 569,674 | 277,391 | 266,506 | 1.3× |
| CPU time | 966 µs | 1348 µs | 1715 µs | 1108 µs | 1120 µs | 1.1× |
| wall time | 2043 µs | 2350 µs | 2707 µs | 2165 µs | 2171 µs | 1.1× |
| peak memory | 1,024 KiB | 2,404 KiB | 2,704 KiB | 2,012 KiB | 2,024 KiB | 2.0× |
| file size | 2,744 B | 2054 KiB | 2335 KiB | 66 KiB | 66 KiB | 24.6× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's version takes 16 bytes a step with NEON, each step masked to the bytes left.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 134,281,426 | 256,360,230 | 218,297,594 | 427,992,538 | 1.6× |
| cycles | 46,441,437 | 130,040,901 | 117,846,087 | 206,270,199 | 2.5× |
| CPU time | 30816 µs | 50390 µs | 47001 µs | 73017 µs | 1.5× |
| wall time | 31738 µs | 51272 µs | 47709 µs | 73709 µs | 1.5× |
| peak memory | 272 KiB | 1,508 KiB | 1,360 KiB | 1,360 KiB | 5.0× |
| file size | 3,144 B | 2058 KiB | 66 KiB | 66 KiB | 21.5× |

## histogram

Counts each byte value of 64 MiB on standard input. C, C++ and Rust spread neighboring bytes over four tables; lengua's takes eight bytes a step, one into each of eight tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 335,606,069 | 319,053,495 | 369,259,581 | 369,259,581 | 1.0× |
| cycles | 115,588,929 | 95,230,199 | 94,713,682 | 94,787,521 | 0.8× |
| CPU time | 46328 µs | 41005 µs | 40714 µs | 40827 µs | 0.9× |
| wall time | 46992 µs | 41856 µs | 41508 µs | 41516 µs | 0.9× |
| peak memory | 272 KiB | 1,572 KiB | 1,360 KiB | 1,360 KiB | 5.0× |
| file size | 3,256 B | 2063 KiB | 66 KiB | 66 KiB | 20.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. Every version uses the CPU's crc32cx instruction 8 bytes at a time in one chain, which an Arm core runs at one a cycle; lengua's loads 64 bytes a step, in pairs, and takes each read's bytes as they come.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 19,956,886 | 16,001,329 | 50,453,322 | 50,453,322 | 0.8× |
| cycles | 9,050,136 | 9,020,773 | 26,022,596 | 26,036,439 | 1.0× |
| CPU time | 21542 µs | 21229 µs | 32934 µs | 31537 µs | 1.0× |
| wall time | 44577 µs | 44396 µs | 34136 µs | 32916 µs | 0.7× |
| peak memory | 272 KiB | 1,564 KiB | 1,360 KiB | 1,360 KiB | 5.0× |
| file size | 2,776 B | 2058 KiB | 66 KiB | 66 KiB | 24.3× |
