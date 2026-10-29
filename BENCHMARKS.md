# lengua against C, C++ and Rust

Measured by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best C, C++ or Rust version takes than the best lengua version.

- CPU: AMD Ryzen AI 7 PRO 350 w/ Radeon 860M (16 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.0 (b940084d7 2026-09-28) (Fedora 1.99.0-1.fc44)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C and C++: `gcc (GCC) 16.2.1 20260819 (Red Hat 16.2.1-2)`, `gcc -O3 -march=native -flto -s` and `g++ -O3 -march=native -flto -s`
- C, C++ and Rust link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 441 | 260,307 | 102,824 | 2,590,146 | 233.2× |
| cycles | 1,724 | 281,867 | 179,074 | 1,215,607 | 103.9× |
| CPU time | 87 µs | 237 µs | 159 µs | 527 µs | 1.8× |
| wall time | 162 µs | 334 µs | 230 µs | 650 µs | 1.4× |
| peak memory | 704 KiB | 1,972 KiB | 1,680 KiB | 4,104 KiB | 2.4× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 11 KiB | 4.0× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 371,080 | 267,712 | 2,741,043 | 1.4× |
| cycles | 133,293 | 52,999 | 337,854 | 233,345 | 1,294,093 | 4.4× |
| CPU time | 655 µs | 635 µs | 963 µs | 760 µs | 1356 µs | 1.2× |
| wall time | 779 µs | 741 µs | 1075 µs | 875 µs | 1519 µs | 1.2× |
| peak memory | 1,024 KiB | 1,024 KiB | 3,000 KiB | 2,508 KiB | 4,868 KiB | 2.4× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|---|
| instructions | 1,050,511 | 198,495 | 197,017 | 372,160 | 549,221 | 262,737 | 2,735,657 | 1.3× |
| cycles | 155,871 | 79,109 | 47,234 | 357,177 | 782,157 | 225,192 | 1,347,124 | 4.8× |
| CPU time | 618 µs | 770 µs | 416 µs | 667 µs | 1472 µs | 709 µs | 1019 µs | 1.6× |
| wall time | 971 µs | 1084 µs | 805 µs | 1060 µs | 1342 µs | 1051 µs | 1463 µs | 1.3× |
| peak memory | 1,024 KiB | 1,024 KiB | 1,024 KiB | 2,860 KiB | 3,124 KiB | 2,380 KiB | 4,980 KiB | 2.3× |
| file size | 4,160 B | 3,992 B | 3,992 B | 4169 KiB | 4828 KiB | 11 KiB | 11 KiB | 2.8× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize; lengua's version takes 64 bytes a step with AVX-512 masks.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 25,214,160 | 153,689,473 | 78,888,215 | 82,423,989 | 3.1× |
| cycles | 4,622,914 | 56,786,753 | 55,052,850 | 59,055,720 | 11.9× |
| CPU time | 8728 µs | 16131 µs | 15779 µs | 16993 µs | 1.8× |
| wall time | 14289 µs | 16507 µs | 16355 µs | 17435 µs | 1.1× |
| peak memory | 776 KiB | 2,032 KiB | 1,684 KiB | 4,088 KiB | 2.2× |
| file size | 3,040 B | 4212 KiB | 11 KiB | 11 KiB | 3.7× |

## histogram

Counts each byte value of 64 MiB on standard input. Every version spreads neighboring bytes over four tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 201,375,979 | 147,161,039 | 184,740,537 | 187,230,063 | 0.7× |
| cycles | 61,569,029 | 61,029,472 | 63,096,109 | 64,634,926 | 1.0× |
| CPU time | 17338 µs | 17850 µs | 18147 µs | 18508 µs | 1.0× |
| wall time | 17671 µs | 18310 µs | 18637 µs | 19009 µs | 1.0× |
| peak memory | 776 KiB | 2,024 KiB | 1,732 KiB | 4,336 KiB | 2.2× |
| file size | 2,984 B | 4221 KiB | 11 KiB | 11 KiB | 3.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's `crc32` instruction 8 bytes at a time, one chain that waits on itself; lengua's version runs three chains side by side and joins them with carry-less multiplies.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 23,593,975 | 11,875,325 | 21,127,994 | 23,617,518 | 0.5× |
| cycles | 8,769,705 | 25,729,785 | 25,516,157 | 27,242,574 | 2.9× |
| CPU time | 12072 µs | 12934 µs | 13608 µs | 13853 µs | 1.1× |
| wall time | 12863 µs | 13548 µs | 14046 µs | 14503 µs | 1.1× |
| peak memory | 708 KiB | 2,020 KiB | 1,684 KiB | 4,240 KiB | 2.4× |
| file size | 3,384 B | 4204 KiB | 11 KiB | 11 KiB | 3.3× |
