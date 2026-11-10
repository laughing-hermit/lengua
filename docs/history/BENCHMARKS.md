> This file describes the native lengua's time. The tag `native-final` keeps that lengua. The WebAssembly lengua's reports, one for each runner, are in [docs/benchmarks](../benchmarks/README.md).

# lengua against C, C++ and Rust

Measured by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best C, C++ or Rust version takes than the best lengua version.

- CPU: AMD Ryzen AI 7 PRO 350 w/ Radeon 860M (16 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Fedora 1.99.1-2.fc44)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C and C++: `gcc (GCC) 16.2.1 20260819 (Red Hat 16.2.1-2)`, `gcc -O3 -march=native -flto -s` and `g++ -O3 -march=native -flto -s`
- C, C++ and Rust link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.
- Stall and event counts, for each large workload: Linux's generic events for front-end and back-end stall cycles, L1 data cache read misses and mispredicted branches, in user space, each the median of 11 more runs; a dash where this CPU doesn't count one.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 441 | 260,301 | 133,755 | 2,590,145 | 303.3× |
| cycles | 1,780 | 282,049 | 222,441 | 1,235,219 | 125.0× |
| CPU time | 87 µs | 367 µs | 288 µs | 441 µs | 3.3× |
| wall time | 168 µs | 476 µs | 397 µs | 531 µs | 2.4× |
| peak memory | 708 KiB | 1,836 KiB | 2,296 KiB | 4,052 KiB | 2.6× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 11 KiB | 4.0× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 664 | 261,942 | 133,187 | 2,592,816 | 200.6× |
| cycles | 2,268 | 290,061 | 223,390 | 1,227,756 | 98.5× |
| CPU time | 90 µs | 205 µs | 171 µs | 792 µs | 1.9× |
| wall time | 171 µs | 272 µs | 240 µs | 924 µs | 1.4× |
| peak memory | 648 KiB | 1,856 KiB | 2,088 KiB | 4,016 KiB | 2.9× |
| file size | 3,552 B | 4188 KiB | 11 KiB | 11 KiB | 3.1× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 371,115 | 298,123 | 2,741,043 | 1.5× |
| cycles | 133,338 | 52,525 | 343,527 | 268,935 | 1,308,324 | 5.1× |
| CPU time | 706 µs | 691 µs | 984 µs | 871 µs | 1429 µs | 1.3× |
| wall time | 816 µs | 810 µs | 1102 µs | 1013 µs | 1603 µs | 1.3× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,916 KiB | 2,908 KiB | 4,856 KiB | 2.8× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|---|
| instructions | 1,050,511 | 198,495 | 197,017 | 372,179 | 549,016 | 293,396 | 2,735,657 | 1.5× |
| cycles | 153,852 | 79,906 | 52,519 | 358,766 | 776,881 | 267,115 | 1,471,919 | 5.1× |
| CPU time | 814 µs | 686 µs | 354 µs | 603 µs | 1537 µs | 760 µs | 1305 µs | 1.7× |
| wall time | 1100 µs | 1007 µs | 743 µs | 989 µs | 1374 µs | 1110 µs | 1704 µs | 1.3× |
| peak memory | 1,024 KiB | 1,024 KiB | 1,024 KiB | 2,880 KiB | 3,208 KiB | 2,848 KiB | 4,972 KiB | 2.8× |
| file size | 4,160 B | 3,992 B | 3,992 B | 4169 KiB | 4828 KiB | 11 KiB | 11 KiB | 2.8× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's versions here take 64 bytes a step with AVX-512 masks: the skill's example (`wc-avx512`), and the program Claude Code wrote through lengua from a plain request in an interactive session (`benches/programs/count/avx512.s`). Both are the fast tier beside a baseline form that takes 16 bytes a step with SSE2 and runs on every x86-64 computer; these numbers were measured before that form existed, and a new run of the benchmark measures all four versions where the CPU has AVX-512, and the baseline forms alone where it has not.

| | lengua, AVX-512 | lengua, by Claude Code, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 38,859,999 | 21,055,698 | 153,689,439 | 78,918,856 | 82,423,989 | 3.7× |
| cycles | 7,759,075 | 4,007,320 | 55,444,943 | 55,107,710 | 59,557,425 | 13.8× |
| front-end stall cycles | 65,302 | 121,141 | 382,019 | 298,970 | 680,498 | 4.6× |
| back-end stall cycles | – | – | – | – | – | – |
| back-end stalls on memory | – | – | – | – | – | – |
| L1 data misses | 1,033,533 | 1,042,153 | 1,042,221 | 1,045,820 | 1,050,800 | 1.0× |
| mispredicted branches | 2,154 | 2,087 | 7,713 | 5,059 | 16,693 | 2.4× |
| CPU time | 12089 µs | 9980 µs | 16457 µs | 16612 µs | 16658 µs | 1.6× |
| wall time | 12860 µs | 13912 µs | 16801 µs | 17062 µs | 17133 µs | 1.3× |
| peak memory | 648 KiB | 704 KiB | 2,116 KiB | 2,220 KiB | 4,012 KiB | 3.3× |
| file size | 2,992 B | 4,000 B | 4212 KiB | 11 KiB | 11 KiB | 3.7× |

## histogram

Counts each byte value of 64 MiB on standard input. Every version spreads neighboring bytes over four tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 201,375,980 | 147,160,971 | 184,771,178 | 187,230,063 | 0.7× |
| cycles | 61,431,759 | 60,978,619 | 62,635,175 | 64,271,341 | 1.0× |
| front-end stall cycles | 573,969 | 892,641 | 861,765 | 1,149,035 | 1.5× |
| back-end stall cycles | – | – | – | – | – |
| back-end stalls on memory | – | – | – | – | – |
| L1 data misses | 1,195,718 | 1,078,393 | 1,088,640 | 1,094,913 | 0.9× |
| mispredicted branches | 2,284 | 6,318 | 5,197 | 16,634 | 2.3× |
| CPU time | 17745 µs | 17331 µs | 17548 µs | 17952 µs | 1.0× |
| wall time | 18076 µs | 17763 µs | 17986 µs | 18392 µs | 1.0× |
| peak memory | 776 KiB | 1,968 KiB | 2,140 KiB | 4,332 KiB | 2.5× |
| file size | 2,984 B | 4221 KiB | 11 KiB | 11 KiB | 3.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's `crc32` instruction 8 bytes at a time, one chain that waits on itself; lengua's version runs three chains side by side and joins them with carry-less multiplies.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 23,593,975 | 11,875,265 | 21,158,632 | 23,617,518 | 0.5× |
| cycles | 8,757,996 | 25,712,238 | 25,588,018 | 27,225,416 | 2.9× |
| front-end stall cycles | 237,198 | 312,629 | 333,703 | 729,917 | 1.3× |
| back-end stall cycles | – | – | – | – | – |
| back-end stalls on memory | – | – | – | – | – |
| L1 data misses | 1,153,319 | 1,045,296 | 1,046,441 | 1,050,237 | 0.9× |
| mispredicted branches | 8,661 | 6,179 | 5,110 | 16,540 | 0.6× |
| CPU time | 12691 µs | 13026 µs | 12982 µs | 13794 µs | 1.0× |
| wall time | 13201 µs | 13358 µs | 13493 µs | 14428 µs | 1.0× |
| peak memory | 648 KiB | 2,024 KiB | 2,220 KiB | 4,016 KiB | 3.1× |
| file size | 3,384 B | 4204 KiB | 11 KiB | 11 KiB | 3.3× |
