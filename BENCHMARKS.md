# lengua against Rust and C

Measured by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best Rust or C version takes than the best lengua version.

- CPU: AMD Ryzen AI 7 PRO 350 w/ Radeon 860M (16 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.0 (b940084d7 2026-09-28) (Fedora 1.99.0-1.fc44)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C: `gcc (GCC) 16.2.1 20260819 (Red Hat 16.2.1-2)`, `gcc -O3 -march=native -flto -s`
- Rust and C link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | best other ÷ best lengua |
|---|---|---|---|---|
| instructions | 441 | 260,215 | 102,824 | 233.2× |
| cycles | 1,770 | 285,545 | 178,418 | 100.8× |
| CPU time | 86 µs | 305 µs | 249 µs | 2.9× |
| wall time | 163 µs | 403 µs | 349 µs | 2.1× |
| peak memory | 520 KiB | 1,984 KiB | 1,608 KiB | 3.1× |
| file size | 2,792 B | 4178 KiB | 11 KiB | 4.0× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the example, 16 bytes at a time, and the same program 64 bytes at a time with AVX-512.

| | lengua | lengua, AVX-512 | Rust | C | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 983,200 | 196,765 | 371,056 | 267,712 | 1.4× |
| cycles | 133,644 | 52,811 | 344,339 | 229,775 | 4.4× |
| CPU time | 706 µs | 646 µs | 769 µs | 735 µs | 1.1× |
| wall time | 826 µs | 769 µs | 869 µs | 857 µs | 1.1× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,860 KiB | 2,576 KiB | 2.5× |
| file size | 4,056 B | 4,032 B | 4174 KiB | 11 KiB | 2.8× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's versions are the fork-join template, 16 bytes at a time on 16 threads, and the same program 64 bytes at a time with AVX-512, on 16 threads and on one; Rust is shown both plain and with scoped threads split the same way.

| | lengua | lengua, AVX-512 | lengua, AVX-512, one thread | Rust | Rust, threads | C | best other ÷ best lengua |
|---|---|---|---|---|---|---|---|
| instructions | 1,247,393 | 247,921 | 246,428 | 372,306 | 549,149 | 262,737 | 1.1× |
| cycles | 286,354 | 198,381 | 167,440 | 363,918 | 800,439 | 217,948 | 1.3× |
| CPU time | 1026 µs | 995 µs | 791 µs | 929 µs | 1457 µs | 744 µs | 0.9× |
| wall time | 1187 µs | 1109 µs | 1171 µs | 1281 µs | 1308 µs | 1112 µs | 1.0× |
| peak memory | 1,664 KiB | 1,664 KiB | 2,048 KiB | 2,848 KiB | 3,168 KiB | 2,448 KiB | 1.5× |
| file size | 4,192 B | 4,024 B | 4,024 B | 4169 KiB | 4828 KiB | 11 KiB | 2.8× |
