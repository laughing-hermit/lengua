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
| instructions | 441 | 260,314 | 133,755 | 2,590,146 | 303.3× |
| cycles | 1,815 | 285,847 | 219,759 | 1,209,416 | 121.1× |
| CPU time | 89 µs | 283 µs | 288 µs | 437 µs | 3.2× |
| wall time | 164 µs | 375 µs | 394 µs | 511 µs | 2.3× |
| peak memory | 776 KiB | 2,004 KiB | 2,152 KiB | 4,200 KiB | 2.6× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 11 KiB | 4.0× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 664 | 261,935 | 133,187 | 2,592,816 | 200.6× |
| cycles | 2,268 | 288,338 | 220,437 | 1,202,209 | 97.2× |
| CPU time | 90 µs | 326 µs | 190 µs | 800 µs | 2.1× |
| wall time | 166 µs | 421 µs | 293 µs | 937 µs | 1.8× |
| peak memory | 648 KiB | 1,952 KiB | 2,084 KiB | 4,012 KiB | 3.0× |
| file size | 3,552 B | 4188 KiB | 11 KiB | 11 KiB | 3.1× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 371,099 | 298,123 | 2,741,042 | 1.5× |
| cycles | 133,247 | 52,855 | 344,814 | 267,747 | 1,269,136 | 5.1× |
| CPU time | 722 µs | 634 µs | 1006 µs | 886 µs | 1420 µs | 1.4× |
| wall time | 830 µs | 742 µs | 1119 µs | 1040 µs | 1607 µs | 1.4× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,908 KiB | 2,976 KiB | 4,944 KiB | 2.8× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|---|
| instructions | 1,050,511 | 198,495 | 197,017 | 372,173 | 548,677 | 293,396 | 2,735,657 | 1.5× |
| cycles | 155,061 | 81,617 | 47,705 | 358,573 | 768,063 | 266,466 | 1,370,768 | 5.6× |
| CPU time | 552 µs | 511 µs | 340 µs | 908 µs | 1549 µs | 746 µs | 1297 µs | 2.2× |
| wall time | 954 µs | 832 µs | 764 µs | 1241 µs | 1424 µs | 1114 µs | 1630 µs | 1.5× |
| peak memory | 1,024 KiB | 1,024 KiB | 1,024 KiB | 2,732 KiB | 3,232 KiB | 2,764 KiB | 5,004 KiB | 2.7× |
| file size | 4,160 B | 3,992 B | 3,992 B | 4169 KiB | 4828 KiB | 11 KiB | 11 KiB | 2.8× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's versions take 64 bytes a step with AVX-512 masks: the skill's example, and the program Claude Code wrote through lengua from a plain request in an interactive session.

| | lengua | lengua, by Claude Code | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 25,214,160 | 21,055,698 | 153,689,493 | 78,918,855 | 82,423,990 | 3.7× |
| cycles | 4,511,954 | 3,954,003 | 55,263,715 | 55,038,520 | 58,780,261 | 13.9× |
| CPU time | 9431 µs | 10133 µs | 16555 µs | 15696 µs | 16935 µs | 1.7× |
| wall time | 13120 µs | 12980 µs | 17001 µs | 16123 µs | 17356 µs | 1.2× |
| peak memory | 648 KiB | 648 KiB | 2,024 KiB | 2,188 KiB | 4,172 KiB | 3.1× |
| file size | 3,040 B | 4,000 B | 4212 KiB | 11 KiB | 11 KiB | 3.7× |

## histogram

Counts each byte value of 64 MiB on standard input. Every version spreads neighboring bytes over four tables.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 201,375,978 | 147,161,042 | 184,771,179 | 187,230,063 | 0.7× |
| cycles | 61,425,041 | 60,394,452 | 62,706,441 | 64,187,940 | 1.0× |
| CPU time | 17045 µs | 17617 µs | 17657 µs | 18287 µs | 1.0× |
| wall time | 17314 µs | 18189 µs | 18065 µs | 18775 µs | 1.0× |
| peak memory | 776 KiB | 1,968 KiB | 2,272 KiB | 4,080 KiB | 2.5× |
| file size | 2,984 B | 4221 KiB | 11 KiB | 11 KiB | 3.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's `crc32` instruction 8 bytes at a time, one chain that waits on itself; lengua's version runs three chains side by side and joins them with carry-less multiplies.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 23,593,975 | 11,875,194 | 21,158,632 | 23,617,519 | 0.5× |
| cycles | 8,754,744 | 25,654,449 | 25,580,578 | 27,151,970 | 2.9× |
| CPU time | 12556 µs | 12670 µs | 13065 µs | 13670 µs | 1.0× |
| wall time | 13364 µs | 13063 µs | 13925 µs | 14076 µs | 1.0× |
| peak memory | 648 KiB | 1,984 KiB | 2,188 KiB | 4,240 KiB | 3.1× |
| file size | 3,384 B | 4204 KiB | 11 KiB | 11 KiB | 3.3× |
