# lengua against C, C++ and Rust on ARM64

Measured on an ARM64 computer by `cargo bench --bench against_rust_and_c`, which writes this file. Every version of a program gets the same arguments, input and files and must give the same output. Instructions and cycles are user-space counts from the CPU's counters, starting at the program's own exec, so a dynamically linked program's loader counts; CPU time includes the kernel's work for it. Each figure is the median of 41 runs after 3 to warm up; peak memory is the median of 5 runs of GNU time. "Best other ÷ best lengua" is how many times more the best C, C++ or Rust version takes than the best lengua version.

- CPU: CPU implementer 0x41, part 0xd49 (4 cores)
- lengua: `llvm-mc 22.1.8, ld.lld 22.1.8`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort`
- C and C++: `gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0`, `gcc -O3 -march=native -flto -s` and `g++ -O3 -march=native -flto -s`
- C, C++ and Rust link the system's C library dynamically, as they ship by default; lengua's programs are static and use no library.
- Stall and event counts, for each large workload: Linux's generic events for front-end and back-end stall cycles, L1 data cache read misses and mispredicted branches (the Arm architecture's common events STALL_FRONTEND, STALL_BACKEND, L1D_CACHE_REFILL and BR_MIS_PRED), and STALL_BACKEND_MEM for the stalls on memory, in user space, each the median of 11 more runs; a dash where this CPU doesn't count one.

## add

Parses two numbers from its arguments and prints their sum: almost all of it is starting up and exiting.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 446 | 196,308 | 88,012 | 85,267 | 191.2× |
| cycles | 1,210 | 236,702 | 146,176 | 142,133 | 117.5× |
| CPU time | 72 µs | 429 µs | 299 µs | 290 µs | 4.0× |
| wall time | 265 µs | 695 µs | 537 µs | 551 µs | 2.0× |
| peak memory | 272 KiB | 1,568 KiB | 1,368 KiB | 1,384 KiB | 5.0× |
| file size | 2,928 B | 2047 KiB | 66 KiB | 66 KiB | 23.1× |

## hypotenuse

Prints the hypotenuse of two decimal sides given as arguments, with three decimals: each side divided into a double, a square root, and the result rounded to thousandths. Like add, it's mostly starting up and exiting: here C and C++ load the math library too.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 607 | 198,271 | 111,906 | 112,695 | 184.4× |
| cycles | 1,561 | 245,697 | 178,401 | 170,667 | 109.3× |
| CPU time | 74 µs | 427 µs | 357 µs | 348 µs | 4.7× |
| wall time | 281 µs | 704 µs | 620 µs | 608 µs | 2.2× |
| peak memory | 272 KiB | 1,568 KiB | 1,512 KiB | 1,512 KiB | 5.6× |
| file size | 3,640 B | 2053 KiB | 66 KiB | 66 KiB | 18.5× |

## upcase

Copies a 1 MiB text file to another with its ASCII letters capitalized: file reads and writes, and one pass over the bytes. lengua's versions are the file program, a byte at a time with no branch, and the same program 16 bytes at a time with NEON.

| | lengua | lengua, NEON | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 13,631,626 | 917,647 | 688,092 | 677,631 | 677,633 | 0.7× |
| cycles | 2,763,289 | 207,087 | 424,898 | 328,283 | 318,688 | 1.5× |
| CPU time | 1700 µs | 916 µs | 1340 µs | 1088 µs | 1100 µs | 1.2× |
| wall time | 2184 µs | 2238 µs | 2225 µs | 2210 µs | 2237 µs | 1.0× |
| peak memory | 1,024 KiB | 1,024 KiB | 2,532 KiB | 2,152 KiB | 2,144 KiB | 2.1× |
| file size | 4,248 B | 4,432 B | 2030 KiB | 66 KiB | 66 KiB | 15.9× |

## shout

Capitalizes 1 MiB of standard input to standard output. lengua's version takes 16 bytes a step with NEON on one thread, since threads on AArch64 come later; Rust is shown both plain and with scoped threads.

| | lengua, NEON, one thread | Rust | Rust, threads | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 917,808 | 688,246 | 753,587 | 671,385 | 671,376 | 0.7× |
| cycles | 214,615 | 434,009 | 743,693 | 296,176 | 277,486 | 1.3× |
| CPU time | 1017 µs | 1444 µs | 2274 µs | 1168 µs | 1157 µs | 1.1× |
| wall time | 2165 µs | 2520 µs | 3010 µs | 2274 µs | 2267 µs | 1.0× |
| peak memory | 1,024 KiB | 2,404 KiB | 2,696 KiB | 2,024 KiB | 2,020 KiB | 2.0× |
| file size | 2,744 B | 2054 KiB | 2335 KiB | 66 KiB | 66 KiB | 24.6× |

## wc

Counts the lines and words of 64 MiB on standard input, as `wc -lw` does. C, C++ and Rust compare each byte with the one before it, which compilers can vectorize. lengua's versions take 16 bytes a step with NEON, each step masked to the bytes left, and with SVE a vector a step, as long as the CPU makes it, each step's lanes the bytes left from whilelo.

| | lengua | lengua with SVE | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|---|
| instructions | 134,281,426 | 104,911,058 | 256,360,114 | 218,297,594 | 427,992,538 | 2.1× |
| cycles | 46,420,693 | 47,290,373 | 130,047,128 | 117,854,482 | 206,274,142 | 2.5× |
| front-end stall cycles | 59,380 | 50,369 | 278,154 | 221,893 | 185,049 | 3.7× |
| back-end stall cycles | 12,270,046 | 17,829,170 | 57,879,438 | 29,569,244 | 76,955,730 | 2.4× |
| back-end stalls on memory | 0 | 0 | 0 | 0 | 0 | – |
| L1 data misses | 14,451 | 11,799 | 26,910 | 17,387 | 19,452 | 1.5× |
| mispredicted branches | 1,082 | 2,017 | 7,232 | 3,096 | 3,299 | 2.9× |
| CPU time | 31211 µs | 31256 µs | 50772 µs | 50273 µs | 73173 µs | 1.6× |
| wall time | 31562 µs | 31611 µs | 51274 µs | 50698 µs | 73617 µs | 1.6× |
| peak memory | 272 KiB | 272 KiB | 1,696 KiB | 1,512 KiB | 1,512 KiB | 5.6× |
| file size | 3,144 B | 3,064 B | 2058 KiB | 66 KiB | 66 KiB | 22.0× |

## histogram

Counts each byte value of 64 MiB on standard input. C, C++ and Rust spread neighboring bytes over four tables; lengua's takes eight bytes a step, one into each of eight tables, four bytes' loads, adds and stores together.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 402,707,765 | 319,053,455 | 369,259,581 | 369,259,581 | 0.8× |
| cycles | 92,291,540 | 95,226,476 | 94,703,815 | 94,782,502 | 1.0× |
| front-end stall cycles | 608,794 | 2,070,329 | 1,393,648 | 1,531,322 | 2.3× |
| back-end stall cycles | 11,148,484 | 24,103,742 | 16,369,718 | 20,350,250 | 1.5× |
| back-end stalls on memory | 0 | 0 | 0 | 0 | – |
| L1 data misses | 120,371 | 95,178 | 75,230 | 77,424 | 0.6× |
| mispredicted branches | 1,348 | 4,782 | 3,234 | 3,213 | 2.4× |
| CPU time | 39249 µs | 41592 µs | 42039 µs | 40847 µs | 1.0× |
| wall time | 39661 µs | 42069 µs | 42488 µs | 41238 µs | 1.0× |
| peak memory | 272 KiB | 1,692 KiB | 1,500 KiB | 1,512 KiB | 5.5× |
| file size | 3,264 B | 2063 KiB | 66 KiB | 66 KiB | 20.7× |

## crc32c

The CRC-32C of 64 MiB on standard input. C, C++ and Rust use the CPU's crc32cx instruction 8 bytes at a time in one chain, which an Arm core runs at one a cycle; lengua's runs that chain over half of each 32 KiB block while carry-less multiplies (pmull) fold the other half on the vector units, and joins the two.

| | lengua | Rust | C | C++ | best other ÷ best lengua |
|---|---|---|---|---|---|
| instructions | 16,862,388 | 16,001,265 | 50,453,322 | 50,453,322 | 0.9× |
| cycles | 5,955,158 | 8,969,021 | 25,937,348 | 25,953,142 | 1.5× |
| front-end stall cycles | 38,493 | 262,430 | 180,754 | 183,504 | 4.7× |
| back-end stall cycles | 1,133,663 | 3,375,642 | 15,611,128 | 15,589,938 | 3.0× |
| back-end stalls on memory | 0 | 0 | 0 | 0 | – |
| L1 data misses | 95,615 | 34,972 | 30,444 | 29,769 | 0.3× |
| mispredicted branches | 34 | 4,722 | 3,694 | 3,024 | 88.9× |
| CPU time | 19474 µs | 20839 µs | 32208 µs | 31102 µs | 1.1× |
| wall time | 42804 µs | 43344 µs | 32601 µs | 31526 µs | 0.7× |
| peak memory | 272 KiB | 1,572 KiB | 1,512 KiB | 1,496 KiB | 5.5× |
| file size | 3,632 B | 2058 KiB | 66 KiB | 66 KiB | 18.6× |
