# lengua against C, C++ and Rust

Measured by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best C, C++ or Rust version takes than the best lengua version.

- CPU: AMD Ryzen AI 7 PRO 350 w/ Radeon 860M (16 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Fedora 1.99.1-2.fc44)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C and C++: `gcc (GCC) 16.2.1 20260819 (Red Hat 16.2.1-2)`, `gcc -O3 -march=native -flto -s` and `g++ -O3 -march=native -flto -s`
- C, C++ and Rust link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 441 | 260,283 | 133,754 | 2,590,146 | 303.3× |
| cycles | 1,958 | 284,711 | 221,902 | 1,218,535 | 113.3× |
| CPU time | 88 µs | 283 µs | 290 µs | 806 µs | 3.2× |
| wall time | 170 µs | 385 µs | 395 µs | 939 µs | 2.3× |
| peak memory | 648 KiB | 1,856 KiB | 2,152 KiB | 4,072 KiB | 2.9× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 11 KiB | 4.0× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 664 | 262,054 | 133,186 | 2,592,816 | 200.6× |
| cycles | 2,343 | 285,283 | 220,529 | 1,239,485 | 94.1× |
| CPU time | 88 µs | 362 µs | 234 µs | 494 µs | 2.7× |
| wall time | 165 µs | 471 µs | 331 µs | 602 µs | 2.0× |
| peak memory | 648 KiB | 1,952 KiB | 2,088 KiB | 4,016 KiB | 3.0× |
| file size | 3,552 B | 4188 KiB | 11 KiB | 11 KiB | 3.1× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 370,989 | 298,122 | 2,741,042 | 1.5× |
| cycles | 133,320 | 53,085 | 348,690 | 279,654 | 1,306,877 | 5.3× |
| CPU time | 696 µs | 380 µs | 942 µs | 660 µs | 1389 µs | 1.7× |
| wall time | 801 µs | 512 µs | 1053 µs | 758 µs | 1557 µs | 1.5× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,916 KiB | 2,924 KiB | 4,868 KiB | 2.8× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|---|
| instructions | 1,050,511 | 198,495 | 197,017 | 372,146 | 548,272 | 293,396 | 2,735,657 | 1.5× |
| cycles | 156,078 | 81,240 | 46,288 | 367,710 | 744,707 | 259,776 | 1,363,976 | 5.6× |
| CPU time | 722 µs | 625 µs | 550 µs | 610 µs | 1498 µs | 759 µs | 1245 µs | 1.1× |
| wall time | 1043 µs | 1040 µs | 911 µs | 1024 µs | 1484 µs | 1169 µs | 1588 µs | 1.1× |
| peak memory | 1,024 KiB | 1,024 KiB | 1,024 KiB | 2,840 KiB | 3,228 KiB | 2,900 KiB | 4,848 KiB | 2.8× |
| file size | 4,160 B | 3,992 B | 3,992 B | 4169 KiB | 4828 KiB | 11 KiB | 11 KiB | 2.8× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's versions take 64 bytes a step with AVX-512 masks: the skill's example, and the program Claude Code wrote through lengua from a plain request in an interactive session.

| | lengua | lengua, by Claude Code | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 38,859,999 | 21,055,698 | 153,689,463 | 78,918,856 | 82,423,989 | 3.7× |
| cycles | 7,704,685 | 3,972,090 | 55,021,931 | 55,087,407 | 59,022,247 | 13.9× |
| CPU time | 12725 µs | 10972 µs | 16319 µs | 16080 µs | 17641 µs | 1.5× |
| wall time | 13410 µs | 13375 µs | 16724 µs | 16481 µs | 18265 µs | 1.2× |
| peak memory | 648 KiB | 648 KiB | 1,968 KiB | 2,272 KiB | 4,196 KiB | 3.0× |
| file size | 2,992 B | 4,000 B | 4212 KiB | 11 KiB | 11 KiB | 3.7× |

## histogram

Counts each byte value of 64 MiB on standard input. Every version spreads neighboring bytes over four tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 201,375,979 | 147,161,009 | 184,771,177 | 187,230,062 | 0.7× |
| cycles | 61,489,952 | 61,150,754 | 62,758,627 | 64,337,266 | 1.0× |
| CPU time | 18462 µs | 17838 µs | 17356 µs | 18434 µs | 0.9× |
| wall time | 18818 µs | 18276 µs | 17769 µs | 18908 µs | 0.9× |
| peak memory | 708 KiB | 1,968 KiB | 2,272 KiB | 4,196 KiB | 2.8× |
| file size | 2,984 B | 4221 KiB | 11 KiB | 11 KiB | 3.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's `crc32` instruction 8 bytes at a time, one chain that waits on itself; lengua's version runs three chains side by side and joins them with carry-less multiplies.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 23,593,975 | 11,875,318 | 21,158,633 | 23,617,518 | 0.5× |
| cycles | 8,767,761 | 25,661,512 | 25,577,416 | 27,266,186 | 2.9× |
| CPU time | 11629 µs | 12799 µs | 13526 µs | 13615 µs | 1.1× |
| wall time | 12405 µs | 13334 µs | 14068 µs | 14161 µs | 1.1× |
| peak memory | 708 KiB | 2,032 KiB | 2,188 KiB | 4,196 KiB | 2.9× |
| file size | 3,384 B | 4204 KiB | 11 KiB | 11 KiB | 3.3× |
