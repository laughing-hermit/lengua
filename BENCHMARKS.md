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
| instructions | 441 | 260,360 | 102,824 | 2,590,146 | 233.2× |
| cycles | 1,693 | 280,378 | 178,800 | 1,202,840 | 105.6× |
| CPU time | 49 µs | 372 µs | 194 µs | 802 µs | 4.0× |
| wall time | 103 µs | 484 µs | 298 µs | 937 µs | 2.9× |
| peak memory | 648 KiB | 2,064 KiB | 1,732 KiB | 4,048 KiB | 2.7× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 11 KiB | 4.0× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 371,080 | 267,712 | 2,741,042 | 1.4× |
| cycles | 133,161 | 53,558 | 347,225 | 231,764 | 1,335,979 | 4.3× |
| CPU time | 672 µs | 624 µs | 567 µs | 546 µs | 917 µs | 0.9× |
| wall time | 781 µs | 732 µs | 637 µs | 660 µs | 1062 µs | 0.9× |
| peak memory | 1,024 KiB | 896 KiB | 2,860 KiB | 2,508 KiB | 4,780 KiB | 2.8× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|---|
| instructions | 1,050,511 | 198,495 | 197,017 | 372,129 | 548,601 | 262,737 | 2,735,657 | 1.3× |
| cycles | 157,844 | 82,668 | 47,522 | 361,413 | 791,554 | 229,177 | 1,366,337 | 4.8× |
| CPU time | 755 µs | 769 µs | 555 µs | 911 µs | 1552 µs | 730 µs | 1310 µs | 1.3× |
| wall time | 1058 µs | 1056 µs | 883 µs | 1257 µs | 1408 µs | 1060 µs | 1623 µs | 1.2× |
| peak memory | 1,024 KiB | 1,024 KiB | 1,024 KiB | 2,860 KiB | 3,232 KiB | 2,448 KiB | 4,780 KiB | 2.4× |
| file size | 4,160 B | 3,992 B | 3,992 B | 4169 KiB | 4828 KiB | 11 KiB | 11 KiB | 2.8× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's versions take 64 bytes a step with AVX-512 masks: the skill's example, and the program Claude Code wrote through lengua from a plain request in an interactive session.

| | lengua | lengua, by Claude Code | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 25,214,160 | 21,055,699 | 153,689,516 | 78,888,215 | 82,423,988 | 3.7× |
| cycles | 4,553,788 | 4,007,886 | 56,341,708 | 55,067,005 | 59,445,020 | 13.7× |
| CPU time | 9585 µs | 10448 µs | 16390 µs | 15395 µs | 16765 µs | 1.6× |
| wall time | 13864 µs | 12901 µs | 16954 µs | 16141 µs | 17182 µs | 1.3× |
| peak memory | 776 KiB | 648 KiB | 1,960 KiB | 1,732 KiB | 4,172 KiB | 2.7× |
| file size | 3,040 B | 4,000 B | 4212 KiB | 11 KiB | 11 KiB | 3.7× |

## histogram

Counts each byte value of 64 MiB on standard input. Every version spreads neighboring bytes over four tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 201,375,979 | 147,161,052 | 184,740,538 | 187,230,063 | 0.7× |
| cycles | 61,333,572 | 60,936,965 | 62,659,677 | 64,349,170 | 1.0× |
| CPU time | 17358 µs | 17222 µs | 17453 µs | 18087 µs | 1.0× |
| wall time | 17719 µs | 17644 µs | 17766 µs | 18738 µs | 1.0× |
| peak memory | 776 KiB | 1,840 KiB | 1,688 KiB | 4,196 KiB | 2.2× |
| file size | 2,984 B | 4221 KiB | 11 KiB | 11 KiB | 3.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's `crc32` instruction 8 bytes at a time, one chain that waits on itself; lengua's version runs three chains side by side and joins them with carry-less multiplies.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 23,593,975 | 11,875,280 | 21,127,993 | 23,617,517 | 0.5× |
| cycles | 8,759,672 | 25,703,709 | 25,533,562 | 27,229,450 | 2.9× |
| CPU time | 12201 µs | 12831 µs | 13711 µs | 13387 µs | 1.1× |
| wall time | 13099 µs | 13390 µs | 14063 µs | 14190 µs | 1.0× |
| peak memory | 648 KiB | 2,024 KiB | 1,684 KiB | 4,348 KiB | 2.6× |
| file size | 3,384 B | 4204 KiB | 11 KiB | 11 KiB | 3.3× |
