# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 076bcc1eee81607c6236fee34fc52418049e4a69, in https://github.com/laughing-hermit/lengua/actions/runs/42756074550. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (17.3 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ✗ ✗ ✗ ✓ | ✗ ? ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ? ? ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✓ ? ✗ ✓ | ✓ ? ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-aarch64 hypotenuse c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 300 names the row: take it out, as the list only shrinks
- linux-aarch64 upcase-stdin cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 335 names the row: take it out, as the list only shrinks
- linux-aarch64 upcase-stdin cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 336 names the row: take it out, as the list only shrinks
- linux-aarch64 upcase-stdin rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 339 names the row: take it out, as the list only shrinks
- linux-aarch64 shout c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 352 names the row: take it out, as the list only shrinks
- linux-aarch64 shout cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 355 names the row: take it out, as the list only shrinks
- linux-aarch64 wc c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 371 names the row: take it out, as the list only shrinks
- linux-aarch64 wc c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 372 names the row: take it out, as the list only shrinks
- linux-aarch64 wc c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 373 names the row: take it out, as the list only shrinks
- linux-aarch64 wc cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 374 names the row: take it out, as the list only shrinks
- linux-aarch64 wc rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 375 names the row: take it out, as the list only shrinks
- linux-aarch64 wc rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 376 names the row: take it out, as the list only shrinks
- linux-aarch64 wc rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 377 names the row: take it out, as the list only shrinks
- linux-aarch64 wc cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 382 names the row: take it out, as the list only shrinks
- linux-aarch64 wc rust cpu: lengua is ahead now, and KNOWN_BEHIND line 386 names the row: take it out, as the list only shrinks
- linux-aarch64 histogram c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 390 names the row: take it out, as the list only shrinks
- linux-aarch64 histogram cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 391 names the row: take it out, as the list only shrinks
- linux-aarch64 histogram rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 392 names the row: take it out, as the list only shrinks
- linux-aarch64 histogram rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 393 names the row: take it out, as the list only shrinks
- linux-aarch64 histogram rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 394 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 407 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 408 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 409 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 410 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 411 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 412 names the row: take it out, as the list only shrinks
- linux-aarch64 crc32c rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 413 names the row: take it out, as the list only shrinks
- linux-aarch64 sort cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 427 names the row: take it out, as the list only shrinks
- linux-aarch64 sort cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 428 names the row: take it out, as the list only shrinks
- linux-aarch64 sort rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 429 names the row: take it out, as the list only shrinks
- linux-aarch64 sort rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 430 names the row: take it out, as the list only shrinks
- linux-aarch64 sort c cpu: lengua is ahead now, and KNOWN_BEHIND line 431 names the row: take it out, as the list only shrinks
- linux-aarch64 sort c wall: lengua is ahead now, and KNOWN_BEHIND line 432 names the row: take it out, as the list only shrinks
- linux-aarch64 sort cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 434 names the row: take it out, as the list only shrinks
- linux-aarch64 sort cpp wall: lengua is ahead now, and KNOWN_BEHIND line 435 names the row: take it out, as the list only shrinks
- linux-aarch64 tally c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 442 names the row: take it out, as the list only shrinks
- linux-aarch64 tally c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 443 names the row: take it out, as the list only shrinks
- linux-aarch64 tally cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 445 names the row: take it out, as the list only shrinks
- linux-aarch64 tally cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 446 names the row: take it out, as the list only shrinks
- linux-aarch64 tally rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 448 names the row: take it out, as the list only shrinks
- linux-aarch64 tally rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 449 names the row: take it out, as the list only shrinks
- linux-aarch64 tally c cpu: lengua is ahead now, and KNOWN_BEHIND line 451 names the row: take it out, as the list only shrinks
- linux-aarch64 tally c wall: lengua is ahead now, and KNOWN_BEHIND line 452 names the row: take it out, as the list only shrinks
- linux-aarch64 tally cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 455 names the row: take it out, as the list only shrinks
- linux-aarch64 tally cpp wall: lengua is ahead now, and KNOWN_BEHIND line 456 names the row: take it out, as the list only shrinks
- linux-aarch64 tally rust cpu: lengua is ahead now, and KNOWN_BEHIND line 459 names the row: take it out, as the list only shrinks
- linux-aarch64 tally rust wall: lengua is ahead now, and KNOWN_BEHIND line 460 names the row: take it out, as the list only shrinks
- linux-aarch64 server c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 463 names the row: take it out, as the list only shrinks
- linux-aarch64 server cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 464 names the row: take it out, as the list only shrinks
- linux-aarch64 server rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 465 names the row: take it out, as the list only shrinks
- linux-aarch64 files rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 479 names the row: take it out, as the list only shrinks
- linux-aarch64 readings c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 492 names the row: take it out, as the list only shrinks
- linux-aarch64 readings cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 493 names the row: take it out, as the list only shrinks
- linux-aarch64 readings cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 494 names the row: take it out, as the list only shrinks
- linux-aarch64 readings cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 495 names the row: take it out, as the list only shrinks
- linux-aarch64 readings rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 496 names the row: take it out, as the list only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 5.88× | 15.42× | 9.63× | 269.08× |
| add | 6.05× | 15.63× | 6.13× | 269.10× |
| hypotenuse | 5.65× | 16.31× | 5.60× | 269.10× |
| upcase | 2.96× | 7.50× | 4.87× | 269.11× |
| upcase-stdin | 1.36× | 1.99× | 13.40× | 269.10× |
| shout | 3.64× | 10.02× | 4.64× | 269.10× |
| wc | 1.16× | 1.52× | 8.95× | 269.11× |
| histogram | 1.12× | 1.29× | 9.43× | 269.10× |
| crc32c | 3.51× | 4.10× | 9.42× | 269.10× |
| sort | 1.17× | 1.20× | 1.09× | 269.14× |
| tally | 0.60× | 0.63× | 7.31× | 269.14× |
| server | 1.36× | 1.52× | 19.62× | 269.10× |
| files | 3.09× | 3.52× | 10.67× | 269.10× |
| readings | 3.18× | 3.39× | 10.31× | 269.10× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | behind, 1.05×, p 6.1e-219 | behind, 1.04×, p 9.8e-4 | behind, 1.05× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | behind, 1.05×, p 4.0e-213 | undecided, 1.04×, p 0.008 | behind, 1.05× | ahead, 0.89× |
| empty | rust-wasm | 160 | ahead, 0.91×, p 9.5e-41 | ahead, 0.93×, p 4.2e-5 | behind, 1.04× | ahead, 0.01× |
| empty | c | 20 | behind, 5.88×, p 3.4e-8 | behind, 15.42×, p 3.4e-8 | behind, 9.63× | behind, 269.08× |
| empty | cpp | 20 | behind, 5.78×, p 3.4e-8 | behind, 15.27×, p 3.4e-8 | behind, 9.63× | behind, 269.08× |
| empty | rust | 20 | behind, 4.44×, p 3.4e-8 | behind, 12.05×, p 3.4e-8 | behind, 5.60× | behind, 68.03× |
| add | c-wasm | 320 | ahead, 0.97×, p 1.7e-27 | ahead, 0.91×, p 2.6e-5 | behind, 1.04× | ahead, 0.04× |
| add | cpp-wasm | 320 | ahead, 0.97×, p 2.6e-23 | ahead, 0.92×, p 9.7e-5 | behind, 1.04× | ahead, 0.04× |
| add | rust-wasm | 40 | ahead, 0.89×, p 2.2e-12 | ahead, 0.82×, p 5.8e-4 | behind, 1.03× | ahead, 0.02× |
| add | c | 20 | behind, 6.05×, p 3.4e-8 | behind, 15.63×, p 3.4e-8 | behind, 6.13× | behind, 269.10× |
| add | cpp | 20 | behind, 6.02×, p 3.4e-8 | behind, 15.54×, p 3.4e-8 | behind, 6.13× | behind, 269.10× |
| add | rust | 20 | behind, 4.92×, p 3.4e-8 | behind, 13.15×, p 3.4e-8 | behind, 5.43× | behind, 54.62× |
| hypotenuse | c-wasm | 320 | ahead, 0.98×, p 2.3e-14 | ahead, 0.94×, p 7.2e-4 | behind, 1.04× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 640 | ahead, 0.98×, p 1.0e-25 | ahead, 0.95×, p 1.5e-6 | behind, 1.04× | ahead, 0.05× |
| hypotenuse | rust-wasm | 160 | ahead, 0.90×, p 3.8e-42 | ahead, 0.94×, p 5.5e-4 | behind, 1.03× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 5.58×, p 3.4e-8 | behind, 16.01×, p 3.4e-8 | behind, 5.60× | behind, 269.10× |
| hypotenuse | cpp | 20 | behind, 5.65×, p 3.4e-8 | behind, 16.31×, p 3.4e-8 | behind, 5.43× | behind, 269.10× |
| hypotenuse | rust | 20 | behind, 4.88×, p 3.4e-8 | behind, 14.35×, p 3.4e-8 | behind, 5.42× | behind, 54.62× |
| upcase | c-wasm | 1280 | ahead, 0.97×, p 1.2e-99 | ahead, 0.90×, p 1.4e-29 | behind, 1.03× | ahead, 0.06× |
| upcase | cpp-wasm | 1280 | ahead, 0.96×, p 1.6e-110 | ahead, 0.90×, p 7.2e-26 | behind, 1.03× | ahead, 0.06× |
| upcase | rust-wasm | 640 | ahead, 0.91×, p 3.7e-181 | ahead, 0.98×, p 1.9e-5 | behind, 1.02× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.96×, p 3.4e-8 | behind, 7.50×, p 3.4e-8 | behind, 4.87× | behind, 269.11× |
| upcase | cpp | 20 | behind, 2.93×, p 3.4e-8 | behind, 7.44×, p 3.4e-8 | behind, 4.87× | behind, 269.11× |
| upcase | rust | 20 | behind, 2.62×, p 3.4e-8 | behind, 6.75×, p 3.4e-8 | behind, 4.14× | behind, 54.62× |
| upcase-stdin | c-wasm | 2000 | undecided, 1.00×, p 0.137 | undecided, 0.99×, p 0.026 | behind, 1.01× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 1280 | ahead, 0.99×, p 8.6e-10 | ahead, 0.99×, p 1.1e-4 | behind, 1.01× | behind, 1.32× |
| upcase-stdin | rust-wasm | 320 | ahead, 0.96×, p 1.2e-51 | ahead, 0.97×, p 6.2e-5 | ahead, 1.00× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.35×, p 3.4e-8 | behind, 1.98×, p 3.4e-8 | behind, 13.40× | behind, 269.10× |
| upcase-stdin | cpp | 20 | behind, 1.36×, p 3.4e-8 | behind, 1.99×, p 3.4e-8 | behind, 13.40× | behind, 269.10× |
| upcase-stdin | rust | 20 | behind, 1.28×, p 3.4e-8 | behind, 1.87×, p 3.4e-8 | behind, 7.93× | behind, 54.61× |
| shout | c-wasm | 2000 | ahead, 0.99×, p 3.4e-8 | undecided, 1.01×, p 0.248 | behind, 1.06× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | ahead, 0.99×, p 0.001 | undecided, 1.00×, p 0.140 | behind, 1.06× | ahead, 0.40× |
| shout | rust-wasm | 320 | ahead, 0.92×, p 8.9e-67 | ahead, 0.94×, p 8.3e-5 | behind, 1.04× | ahead, 0.03× |
| shout | c | 20 | behind, 3.62×, p 3.4e-8 | behind, 9.97×, p 3.4e-8 | behind, 4.64× | behind, 269.10× |
| shout | cpp | 20 | behind, 3.64×, p 3.4e-8 | behind, 10.02×, p 3.4e-8 | behind, 4.64× | behind, 269.10× |
| shout | rust | 20 | behind, 3.05×, p 3.4e-8 | behind, 8.46×, p 3.4e-8 | behind, 3.91× | behind, 54.61× |
| wc | c-wasm | 20 | ahead, 0.70×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | ahead, 0.95× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.11×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | ahead, 0.95× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.70×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | ahead, 0.94× | ahead, 0.04× |
| wc | c | 20 | behind, 1.16×, p 3.4e-8 | behind, 1.52×, p 3.4e-8 | behind, 8.95× | behind, 269.11× |
| wc | cpp | 1280 | ahead, 1.00×, p 1.2e-4 | behind, 1.32×, p 0.0e0 | behind, 8.95× | behind, 269.11× |
| wc | rust | 20 | ahead, 0.92×, p 3.4e-8 | behind, 1.21×, p 3.4e-8 | behind, 7.98× | behind, 54.62× |
| histogram | c-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.95×, p 3.4e-8 | ahead, 0.94×, p 1.9e-5 | ahead, 0.99× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.12×, p 3.4e-8 | behind, 1.29×, p 3.4e-8 | behind, 9.43× | behind, 269.10× |
| histogram | cpp | 20 | behind, 1.12×, p 3.4e-8 | behind, 1.28×, p 3.4e-8 | behind, 9.43× | behind, 269.10× |
| histogram | rust | 20 | behind, 1.11×, p 3.4e-8 | behind, 1.27×, p 3.4e-8 | behind, 8.44× | behind, 54.62× |
| crc32c | c-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | ahead, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.80×, p 3.4e-8 | ahead, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | ahead, 0.99× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.46×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 9.42× | behind, 269.10× |
| crc32c | cpp | 20 | behind, 2.45×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 9.42× | behind, 269.10× |
| crc32c | rust | 20 | behind, 3.51×, p 3.4e-8 | behind, 4.10×, p 3.4e-8 | behind, 8.41× | behind, 54.62× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | behind, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.66×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | ahead, 0.96× | behind, 269.14× |
| sort | cpp | 20 | ahead, 0.74×, p 3.4e-8 | ahead, 0.75×, p 3.4e-8 | behind, 1.01× | behind, 268.75× |
| sort | rust | 20 | behind, 1.17×, p 3.4e-8 | behind, 1.20×, p 3.4e-8 | behind, 1.09× | behind, 54.62× |
| tally | c-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | behind, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.49×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | behind, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.35×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | behind, 7.07× | behind, 269.14× |
| tally | cpp | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | behind, 3.80× | behind, 269.14× |
| tally | rust | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 7.31× | behind, 54.62× |
| server | c-wasm | 160 | ahead, 0.98×, p 2.6e-6 | ahead, 0.93×, p 1.2e-28 | ahead, 0.99× | ahead, 0.12× |
| server | cpp-wasm | 160 | ahead, 0.98×, p 3.6e-6 | ahead, 0.94×, p 8.5e-24 | ahead, 0.99× | ahead, 0.06× |
| server | rust-wasm | 40 | ahead, 0.97×, p 2.7e-4 | ahead, 0.91×, p 1.0e-10 | ahead, 0.98× | ahead, 0.03× |
| server | c | 20 | behind, 1.36×, p 3.4e-8 | behind, 1.33×, p 3.4e-8 | behind, 19.62× | behind, 269.10× |
| server | cpp | 20 | behind, 1.36×, p 3.4e-8 | behind, 1.34×, p 3.4e-8 | behind, 5.45× | behind, 268.72× |
| server | rust | 20 | ahead, 0.77×, p 3.4e-8 | behind, 1.52×, p 3.4e-8 | behind, 6.81× | behind, 54.61× |
| files | c-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.7e-5 | behind, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.90×, p 5.5e-6 | behind, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.93×, p 9.0e-6 | ahead, 1.00× | ahead, 0.03× |
| files | c | 20 | behind, 2.55×, p 3.4e-8 | behind, 2.90×, p 3.4e-8 | behind, 10.67× | behind, 269.10× |
| files | cpp | 20 | behind, 2.54×, p 3.4e-8 | behind, 2.90×, p 3.4e-8 | behind, 10.67× | behind, 269.10× |
| files | rust | 20 | behind, 3.09×, p 3.4e-8 | behind, 3.52×, p 3.4e-8 | behind, 9.03× | behind, 54.62× |
| readings | c-wasm | 20 | ahead, 0.96×, p 3.4e-8 | ahead, 0.97×, p 7.9e-7 | ahead, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 80 | ahead, 0.99×, p 4.7e-28 | ahead, 0.99×, p 2.6e-5 | ahead, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.95×, p 4.6e-8 | ahead, 0.99× | ahead, 0.02× |
| readings | c | 20 | behind, 3.18×, p 3.4e-8 | behind, 3.39×, p 3.4e-8 | behind, 10.31× | behind, 269.10× |
| readings | cpp | 20 | behind, 3.08×, p 3.4e-8 | behind, 3.29×, p 3.4e-8 | behind, 4.40× | behind, 269.10× |
| readings | rust | 20 | behind, 3.11×, p 3.4e-8 | behind, 3.32×, p 3.4e-8 | behind, 9.55× | behind, 54.62× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 3.01 ms | 2.86 ms | 2.87 ms | 3.35 ms | 514 µs | 523 µs | 681 µs |
| wall time | 10.6 ms | 10.1 ms | 10.2 ms | 10.8 ms | 603 µs | 609 µs | 772 µs |
| peak memory | 8.21 MiB | 7.83 MiB | 7.83 MiB | 7.86 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 22,266 | 22,360 | 21,811 | 26,763 | 1,524 | 1,525 | 2,559 |

Its rounds took 85 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 320 | 320 | 40 | 20 | 20 | 20 |
| CPU time | 3.46 ms | 3.57 ms | 3.56 ms | 3.84 ms | 560 µs | 562 µs | 688 µs |
| wall time | 10.6 ms | 11.7 ms | 11.5 ms | 12.5 ms | 652 µs | 656 µs | 775 µs |
| peak memory | 8.27 MiB | 7.95 MiB | 7.95 MiB | 8.06 MiB | 1.35 MiB | 1.35 MiB | 1.53 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,754 | 27,415 | 26,606 | 31,497 | 1,652 | 1,656 | 2,621 |

Its rounds took 15 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 320 | 640 | 160 | 20 | 20 | 20 |
| CPU time | 3.48 ms | 3.54 ms | 3.55 ms | 3.81 ms | 608 µs | 600 µs | 696 µs |
| wall time | 10.5 ms | 11.2 ms | 11.0 ms | 11.1 ms | 704 µs | 691 µs | 785 µs |
| peak memory | 8.27 MiB | 7.95 MiB | 7.95 MiB | 8.07 MiB | 1.48 MiB | 1.52 MiB | 1.53 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,952 | 26,436 | 26,064 | 31,846 | 2,026 | 2,044 | 2,589 |

Its rounds took 26 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 1280 | 1280 | 640 | 20 | 20 | 20 |
| CPU time | 4.31 ms | 4.45 ms | 4.46 ms | 4.74 ms | 1.43 ms | 1.44 ms | 1.62 ms |
| wall time | 10.5 ms | 11.6 ms | 11.7 ms | 11.8 ms | 1.61 ms | 1.63 ms | 1.80 ms |
| peak memory | 10.4 MiB | 10.1 MiB | 10.1 MiB | 10.2 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 25,996 | 29,396 | 28,298 | 33,489 | 2,168 | 2,197 | 3,127 |

Its rounds took 69 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 1280 | 320 | 20 | 20 | 20 |
| CPU time | 14.7 ms | 14.7 ms | 14.9 ms | 15.2 ms | 10.6 ms | 10.5 ms | 11.2 ms |
| wall time | 21.9 ms | 22.0 ms | 22.3 ms | 22.6 ms | 10.7 ms | 10.6 ms | 11.3 ms |
| peak memory | 13.1 MiB | 13.0 MiB | 13.0 MiB | 13.1 MiB | 1000 KiB | 1000 KiB | 1.65 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 156,900 | 163,357 | 164,680 | 181,258 | 25,115 | 21,803 | 47,421 |

Its rounds took 149 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 20 |
| CPU time | 3.64 ms | 3.67 ms | 3.66 ms | 4.00 ms | 966 µs | 960 µs | 1.15 ms |
| wall time | 10.8 ms | 10.7 ms | 10.7 ms | 11.4 ms | 1.05 ms | 1.04 ms | 1.24 ms |
| peak memory | 9.17 MiB | 8.64 MiB | 8.65 MiB | 8.79 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 25,094 | 24,936 | 25,184 | 30,355 | 1,900 | 2,038 | 3,035 |

Its rounds took 91 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 20 | 20 | 20 | 20 | 1280 | 20 |
| CPU time | 20.8 ms | 29.9 ms | 188 ms | 29.7 ms | 18.0 ms | 20.9 ms | 22.6 ms |
| wall time | 27.7 ms | 38.5 ms | 195 ms | 36.4 ms | 18.1 ms | 21.0 ms | 22.7 ms |
| peak memory | 13.2 MiB | 14.0 MiB | 14.0 MiB | 14.1 MiB | 1.48 MiB | 1.48 MiB | 1.66 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 134,631 | 155,439 | 142,194 | 161,185 | 23,281 | 28,641 | 40,745 |

Its rounds took 81 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 39.8 ms | 71.8 ms | 71.9 ms | 42.0 ms | 35.6 ms | 35.6 ms | 36.0 ms |
| wall time | 45.9 ms | 78.6 ms | 79.7 ms | 49.1 ms | 35.7 ms | 35.8 ms | 36.2 ms |
| peak memory | 13.9 MiB | 14.0 MiB | 14.0 MiB | 14.1 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 190,278 | 197,356 | 197,087 | 219,390 | 84,739 | 84,549 | 102,899 |

Its rounds took 9 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 37.4 ms | 47.9 ms | 48.1 ms | 51.2 ms | 15.2 ms | 15.3 ms | 10.7 ms |
| wall time | 44.1 ms | 55.9 ms | 55.3 ms | 57.3 ms | 15.4 ms | 15.4 ms | 10.8 ms |
| peak memory | 13.9 MiB | 14.0 MiB | 14.0 MiB | 14.1 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 308,522 | 244,465 | 251,206 | 253,085 | 24,875 | 24,503 | 27,228 |

Its rounds took 7 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 274 ms | 1.38 s | 472 ms | 416 ms | 384 ms | 372 ms | 234 ms |
| wall time | 280 ms | 1.38 s | 478 ms | 427 ms | 384 ms | 372 ms | 234 ms |
| peak memory | 83.6 MiB | 83.5 MiB | 91.5 MiB | 91.5 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,263,348 | 30,311,818 | 12,239,299 | 12,133,816 | 14,528,313 | 15,534,692 | 12,468,177 |

Its rounds took 84 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 141 ms | 293 ms | 291 ms | 407 ms | 236 ms | 244 ms | 251 ms |
| wall time | 149 ms | 300 ms | 298 ms | 414 ms | 237 ms | 244 ms | 252 ms |
| peak memory | 14.9 MiB | 14.6 MiB | 14.6 MiB | 14.5 MiB | 2.11 MiB | 3.92 MiB | 2.04 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,867,083 | 8,680,742 | 8,664,342 | 8,157,019 | 8,520,002 | 8,390,359 | 11,276,448 |

Its rounds took 44 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 160 | 40 | 20 | 20 | 20 |
| CPU time | 582 ms | 593 ms | 595 ms | 593 ms | 428 ms | 427 ms | 758 ms |
| wall time | 582 ms | 624 ms | 617 ms | 629 ms | 434 ms | 431 ms | 380 ms |
| peak memory | 13.9 MiB | 14.1 MiB | 14.1 MiB | 14.1 MiB | 724 KiB | 2.54 MiB | 2.04 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,550,928 | 2,636,385 | 2,663,060 | 2,341,194 | 107,757 | 252,267 | 503,475 |

Its rounds took 385 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 44.5 ms | 48.0 ms | 48.1 ms | 48.5 ms | 17.5 ms | 17.5 ms | 14.4 ms |
| wall time | 51.1 ms | 54.7 ms | 56.8 ms | 55.1 ms | 17.6 ms | 17.6 ms | 14.5 ms |
| peak memory | 14.4 MiB | 14.4 MiB | 14.4 MiB | 14.5 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,502,473 | 7,559,620 | 7,567,885 | 7,585,861 | 10,807 | 9,329 | 9,410 |

Its rounds took 7 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 102 ms | 106 ms | 103 ms | 109 ms | 32.1 ms | 33.1 ms | 32.8 ms |
| wall time | 109 ms | 113 ms | 110 ms | 115 ms | 32.2 ms | 33.3 ms | 32.9 ms |
| peak memory | 13.9 MiB | 14.0 MiB | 14.0 MiB | 14.1 MiB | 1.35 MiB | 3.16 MiB | 1.46 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 25,692 | 28,555 | 27,491 | 33,324 | 1,750 | 13,593 | 2,730 |

Its rounds took 27 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 4 s | 2.94 ms against 2.82 ms: +101 µs (+74.0 µs to +126 µs) | 9.75 ms against 10.3 ms: -585 µs (-1.22 ms to -179 µs) | 8.21 MiB against 7.83 MiB: +380 KiB (+368 KiB to +384 KiB) |
| add | 160, 160 in 5 s | 3.41 ms against 3.24 ms: +153 µs (+124 µs to +181 µs) | 10.8 ms against 10.6 ms: +366 µs (-140 µs to +903 µs) | 8.27 MiB against 7.90 MiB: +384 KiB (+372 KiB to +384 KiB) |
| hypotenuse | 160, 160 in 5 s | 3.31 ms against 3.11 ms: +187 µs (+149 µs to +224 µs) | 10.6 ms against 10.5 ms: +228 µs (-266 µs to +738 µs) | 8.27 MiB against 7.90 MiB: +384 KiB (+380 KiB to +384 KiB) |
| upcase | 160, 160 in 5 s | 4.28 ms against 4.12 ms: +159 µs (+127 µs to +190 µs) | 9.74 ms against 11.7 ms: -1.33 ms (-1.95 ms to -784 µs) | 10.5 MiB against 10.3 MiB: +392 KiB (+148 KiB to +552 KiB) |
| upcase-stdin | 160, 160 in 8 s | 14.0 ms against 13.9 ms: +200 µs (+125 µs to +274 µs) | 21.3 ms against 21.3 ms: +12.3 µs (-546 µs to +569 µs) | 13.1 MiB against 13.1 MiB: +56.0 KiB (+16.0 KiB to +64.0 KiB) |
| shout | 160, 160 in 5 s | 3.59 ms against 3.46 ms: +152 µs (+119 µs to +185 µs) | 10.8 ms against 10.5 ms: +80.2 µs (-454 µs to +623 µs) | 9.27 MiB against 8.77 MiB: +512 KiB (+416 KiB to +520 KiB) |
| wc | 160, 160 in 10 s | 20.7 ms against 20.5 ms: +145 µs (+80.0 µs to +207 µs) | 27.3 ms against 27.2 ms: -38.2 µs (-560 µs to +476 µs) | 13.2 MiB against 13.1 MiB: +128 KiB (+116 KiB to +140 KiB) |
| histogram | 160, 160 in 16 s | 39.8 ms against 39.7 ms: +121 µs (+69.0 µs to +172 µs) | 46.1 ms against 46.8 ms: -372 µs (-878 µs to +123 µs) | 13.9 MiB against 13.9 MiB: -64.0 KiB (-64.0 KiB to -52.0 KiB) |
| crc32c | 160, 160 in 16 s | 37.4 ms against 37.3 ms: +134 µs (+74.0 µs to +191 µs) | 44.7 ms against 44.2 ms: +203 µs (-325 µs to +700 µs) | 13.9 MiB against 13.9 MiB: -56.0 KiB (-64.0 KiB to -36.0 KiB) |
| sort | 104, 104 in 61 s | 278 ms against 279 ms: +35.0 µs (-1.10 ms to +1.17 ms) | 286 ms against 286 ms: -580 µs (-1.96 ms to +798 µs) | 83.5 MiB against 83.6 MiB: -12.0 KiB (-52.0 KiB to +0 B) |
| tally | 160, 160 in 49 s | 141 ms against 141 ms: +152 µs (-217 µs to +503 µs) | 148 ms against 148 ms: -211 µs (-825 µs to +435 µs) | 14.9 MiB against 14.9 MiB: +0 B (-44.0 KiB to +4.00 KiB) |
| server | 46, 46 in 60 s | 594 ms against 599 ms: -5.92 ms (-13.3 ms to +1.79 ms) | 593 ms against 598 ms: -5.87 ms (-13.2 ms to +2.09 ms) | 13.9 MiB against 14.0 MiB: -64.0 KiB (-96.0 KiB to +0 B) |
| files | 160, 160 in 18 s | 44.4 ms against 44.2 ms: +163 µs (+114 µs to +214 µs) | 51.3 ms against 51.5 ms: -255 µs (-697 µs to +205 µs) | 14.4 MiB against 14.4 MiB: +0 B (-12.0 KiB to +4.00 KiB) |
| readings | 160, 160 in 36 s | 102 ms against 101 ms: +723 µs (+698 µs to +748 µs) | 109 ms against 109 ms: +628 µs (+213 µs to +1.15 ms) | 13.9 MiB against 14.0 MiB: -64.0 KiB (-84.0 KiB to -64.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
