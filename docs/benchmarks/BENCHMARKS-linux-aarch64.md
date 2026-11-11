# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 33ed7fe1f45158efe1a2695d8accd88e1dfb18a5, in https://github.com/laughing-hermit/lengua/actions/runs/42947294489. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (17.4 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ✗ ✓ ✗ ✓ | ✗ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ✗ ✓ | ✗ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✗ ? ✗ ✗ | ? ✓ ✗ ✗ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ? ✗ ✓ | ✗ ? ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✓ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-aarch64 empty c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 231 names the row: take it out, as the list only shrinks
- linux-aarch64 empty cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 234 names the row: take it out, as the list only shrinks
- linux-aarch64 add cpp-wasm cpu: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 upcase-stdin cpp-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 upcase-stdin rust-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout c-wasm cpu: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout cpp-wasm cpu: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 wc cpp cpu: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 histogram cpp-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 crc32c c-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 crc32c cpp-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 readings c-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 readings cpp-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 readings rust-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 6.35× | 15.01× | 9.49× | 271.02× |
| add | 6.56× | 17.46× | 6.03× | 271.04× |
| hypotenuse | 5.79× | 13.93× | 5.52× | 271.04× |
| upcase | 3.02× | 6.95× | 5.47× | 271.05× |
| upcase-stdin | 1.36× | 2.00× | 13.33× | 271.04× |
| shout | 3.77× | 8.21× | 4.64× | 271.04× |
| wc | 1.18× | 1.56× | 8.85× | 271.05× |
| histogram | 1.13× | 1.31× | 9.31× | 271.04× |
| crc32c | 3.42× | 3.95× | 9.34× | 271.04× |
| sort | 1.12× | 1.15× | 1.09× | 271.08× |
| tally | 0.60× | 0.63× | 7.45× | 271.09× |
| server | 1.32× | 1.57× | 19.35× | 271.05× |
| files | 3.02× | 3.49× | 10.49× | 271.04× |
| readings | 3.17× | 3.36× | 10.25× | 271.04× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 1280 | behind, 1.09×, p 3.7e-299 | ahead, 0.98×, p 5.0e-5 | behind, 1.08× | ahead, 0.89× |
| empty | cpp-wasm | 1280 | behind, 1.09×, p 3.6e-299 | ahead, 0.98×, p 3.3e-6 | behind, 1.08× | ahead, 0.89× |
| empty | rust-wasm | 160 | ahead, 0.91×, p 5.1e-52 | ahead, 0.94×, p 1.1e-4 | behind, 1.08× | ahead, 0.01× |
| empty | c | 20 | behind, 6.19×, p 3.4e-8 | behind, 14.79×, p 3.4e-8 | behind, 9.49× | behind, 271.02× |
| empty | cpp | 20 | behind, 6.35×, p 3.4e-8 | behind, 15.01×, p 3.4e-8 | behind, 9.49× | behind, 271.02× |
| empty | rust | 20 | behind, 4.66×, p 3.4e-8 | behind, 11.59×, p 3.4e-8 | behind, 5.51× | behind, 68.52× |
| add | c-wasm | 1280 | ahead, 0.99×, p 1.4e-7 | ahead, 0.99×, p 8.4e-4 | behind, 1.08× | ahead, 0.04× |
| add | cpp-wasm | 1280 | behind, 1.05×, p 1.0e-55 | ahead, 0.99×, p 1.4e-4 | behind, 1.08× | ahead, 0.04× |
| add | rust-wasm | 640 | ahead, 0.92×, p 1.5e-105 | ahead, 0.95×, p 3.7e-5 | behind, 1.06× | ahead, 0.02× |
| add | c | 20 | behind, 6.56×, p 3.4e-8 | behind, 17.46×, p 3.4e-8 | behind, 6.03× | behind, 271.04× |
| add | cpp | 20 | behind, 6.43×, p 3.4e-8 | behind, 17.26×, p 3.4e-8 | behind, 6.03× | behind, 271.04× |
| add | rust | 20 | behind, 5.15×, p 3.4e-8 | behind, 14.04×, p 3.4e-8 | behind, 5.57× | behind, 55.02× |
| hypotenuse | c-wasm | 320 | ahead, 0.97×, p 1.7e-15 | ahead, 0.98×, p 0.001 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 320 | ahead, 0.97×, p 3.4e-13 | ahead, 0.95×, p 1.3e-5 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | rust-wasm | 160 | ahead, 0.91×, p 3.9e-41 | ahead, 0.93×, p 2.8e-5 | behind, 1.06× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 5.79×, p 3.4e-8 | behind, 13.93×, p 3.4e-8 | behind, 5.52× | behind, 271.04× |
| hypotenuse | cpp | 20 | behind, 5.65×, p 3.4e-8 | behind, 13.69×, p 3.4e-8 | behind, 5.52× | behind, 271.04× |
| hypotenuse | rust | 20 | behind, 5.05×, p 3.4e-8 | behind, 12.23×, p 3.4e-8 | behind, 5.32× | behind, 55.02× |
| upcase | c-wasm | 640 | ahead, 0.98×, p 5.0e-15 | ahead, 0.97×, p 1.8e-5 | behind, 1.03× | ahead, 0.06× |
| upcase | cpp-wasm | 320 | ahead, 0.98×, p 6.5e-10 | ahead, 0.96×, p 3.8e-4 | behind, 1.03× | ahead, 0.06× |
| upcase | rust-wasm | 160 | ahead, 0.92×, p 2.4e-37 | ahead, 0.95×, p 9.1e-4 | behind, 1.02× | ahead, 0.03× |
| upcase | c | 20 | behind, 3.00×, p 3.4e-8 | behind, 6.83×, p 3.4e-8 | behind, 5.47× | behind, 271.05× |
| upcase | cpp | 20 | behind, 3.02×, p 3.4e-8 | behind, 6.95×, p 3.4e-8 | behind, 5.47× | behind, 271.05× |
| upcase | rust | 20 | behind, 2.71×, p 3.4e-8 | behind, 6.16×, p 3.4e-8 | behind, 4.65× | behind, 55.02× |
| upcase-stdin | c-wasm | 2000 | behind, 1.01×, p 5.7e-20 | undecided, 1.00×, p 0.170 | behind, 1.02× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | undecided, 1.00×, p 0.005 | ahead, 0.98×, p 1.3e-4 | behind, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 320 | ahead, 0.97×, p 1.5e-28 | ahead, 0.97×, p 3.0e-4 | behind, 1.01× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.36×, p 3.4e-8 | behind, 2.00×, p 3.4e-8 | behind, 13.33× | behind, 271.04× |
| upcase-stdin | cpp | 20 | behind, 1.34×, p 3.4e-8 | behind, 1.98×, p 3.4e-8 | behind, 13.33× | behind, 271.04× |
| upcase-stdin | rust | 20 | behind, 1.28×, p 3.4e-8 | behind, 1.89×, p 3.4e-8 | behind, 7.92× | behind, 55.01× |
| shout | c-wasm | 2000 | behind, 1.03×, p 1.4e-92 | undecided, 1.01×, p 0.334 | behind, 1.10× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | behind, 1.07×, p 1.0e-309 | undecided, 1.01×, p 0.254 | behind, 1.09× | ahead, 0.40× |
| shout | rust-wasm | 160 | ahead, 0.94×, p 2.0e-28 | ahead, 0.91×, p 4.1e-4 | behind, 1.08× | ahead, 0.03× |
| shout | c | 20 | behind, 3.74×, p 3.4e-8 | behind, 8.17×, p 3.4e-8 | behind, 4.64× | behind, 271.04× |
| shout | cpp | 20 | behind, 3.77×, p 3.4e-8 | behind, 8.21×, p 3.4e-8 | behind, 4.64× | behind, 271.04× |
| shout | rust | 20 | behind, 3.15×, p 3.4e-8 | behind, 6.83×, p 3.4e-8 | behind, 3.90× | behind, 55.01× |
| wc | c-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.76×, p 3.9e-8 | ahead, 0.95× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.12×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | ahead, 0.95× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.77×, p 7.2e-8 | ahead, 0.94× | ahead, 0.04× |
| wc | c | 20 | behind, 1.18×, p 3.4e-8 | behind, 1.56×, p 3.4e-8 | behind, 8.85× | behind, 271.05× |
| wc | cpp | 20 | behind, 1.03×, p 3.8e-6 | behind, 1.37×, p 3.4e-8 | behind, 8.82× | behind, 271.05× |
| wc | rust | 20 | ahead, 0.95×, p 4.6e-8 | behind, 1.26×, p 3.4e-8 | behind, 7.87× | behind, 55.01× |
| histogram | c-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.95×, p 3.4e-8 | ahead, 0.95×, p 8.9e-4 | ahead, 0.99× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.13×, p 3.4e-8 | behind, 1.31×, p 3.4e-8 | behind, 9.31× | behind, 271.04× |
| histogram | cpp | 20 | behind, 1.13×, p 3.4e-8 | behind, 1.31×, p 3.4e-8 | behind, 9.31× | behind, 271.04× |
| histogram | rust | 20 | behind, 1.11×, p 3.4e-8 | behind, 1.30×, p 3.4e-8 | behind, 8.35× | behind, 55.01× |
| crc32c | c-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.9e-8 | behind, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | behind, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.74×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | ahead, 1.00× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.44×, p 3.4e-8 | behind, 2.83×, p 3.4e-8 | behind, 9.34× | behind, 271.04× |
| crc32c | cpp | 20 | behind, 2.43×, p 3.4e-8 | behind, 2.81×, p 3.4e-8 | behind, 9.34× | behind, 271.04× |
| crc32c | rust | 20 | behind, 3.42×, p 3.4e-8 | behind, 3.95×, p 3.4e-8 | behind, 8.38× | behind, 55.01× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | behind, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.65×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.70×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | ahead, 0.96× | behind, 271.08× |
| sort | cpp | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.74×, p 3.4e-8 | behind, 1.01× | behind, 270.69× |
| sort | rust | 20 | behind, 1.12×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 1.09× | behind, 55.01× |
| tally | c-wasm | 20 | ahead, 0.49×, p 3.4e-8 | ahead, 0.49×, p 3.4e-8 | behind, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.49×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | behind, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.35×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | behind, 6.98× | behind, 271.09× |
| tally | cpp | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | behind, 3.75× | behind, 271.09× |
| tally | rust | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 7.45× | behind, 55.02× |
| server | c-wasm | 260 | ahead, 0.99×, p 3.5e-4 | ahead, 0.94×, p 2.3e-44 | ahead, 1.00× | ahead, 0.12× |
| server | cpp-wasm | 160 | ahead, 0.99×, p 7.6e-4 | ahead, 0.95×, p 4.6e-26 | ahead, 0.99× | ahead, 0.06× |
| server | rust-wasm | 160 | ahead, 0.98×, p 1.1e-5 | ahead, 0.93×, p 4.1e-32 | ahead, 0.99× | ahead, 0.03× |
| server | c | 20 | behind, 1.32×, p 3.4e-8 | behind, 1.32×, p 3.4e-8 | behind, 19.35× | behind, 271.05× |
| server | cpp | 20 | behind, 1.32×, p 3.4e-8 | behind, 1.32×, p 3.4e-8 | behind, 5.38× | behind, 270.66× |
| server | rust | 20 | ahead, 0.78×, p 3.4e-8 | behind, 1.57×, p 3.4e-8 | behind, 6.63× | behind, 55.00× |
| files | c-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.94×, p 3.3e-5 | behind, 1.01× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.93×, p 2.6e-6 | behind, 1.01× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.91×, p 3.4e-8 | ahead, 0.92×, p 2.6e-6 | ahead, 0.99× | ahead, 0.03× |
| files | c | 20 | behind, 2.51×, p 3.4e-8 | behind, 2.90×, p 3.4e-8 | behind, 10.49× | behind, 271.04× |
| files | cpp | 20 | behind, 2.51×, p 3.4e-8 | behind, 2.90×, p 3.4e-8 | behind, 10.49× | behind, 271.04× |
| files | rust | 20 | behind, 3.02×, p 3.4e-8 | behind, 3.49×, p 3.4e-8 | behind, 8.88× | behind, 55.02× |
| readings | c-wasm | 20 | ahead, 0.97×, p 3.4e-8 | ahead, 0.96×, p 3.5e-7 | behind, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.98×, p 3.4e-8 | ahead, 0.98×, p 4.6e-5 | behind, 1.01× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.94×, p 3.4e-8 | behind, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 3.17×, p 3.4e-8 | behind, 3.36×, p 3.4e-8 | behind, 10.25× | behind, 271.04× |
| readings | cpp | 20 | behind, 3.07×, p 3.4e-8 | behind, 3.25×, p 3.4e-8 | behind, 4.38× | behind, 271.04× |
| readings | rust | 20 | behind, 3.10×, p 3.4e-8 | behind, 3.29×, p 3.4e-8 | behind, 9.45× | behind, 55.02× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 1280 | 1280 | 160 | 20 | 20 | 20 |
| CPU time | 3.40 ms | 3.12 ms | 3.11 ms | 3.78 ms | 556 µs | 542 µs | 738 µs |
| wall time | 10.5 ms | 10.7 ms | 10.7 ms | 11.1 ms | 657 µs | 647 µs | 838 µs |
| peak memory | 8.08 MiB | 7.45 MiB | 7.45 MiB | 7.46 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 21,420 | 21,323 | 21,375 | 26,349 | 1,534 | 1,493 | 2,568 |

Its rounds took 59 seconds.

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
| counted runs | 1280 | 1280 | 1280 | 640 | 20 | 20 | 20 |
| CPU time | 3.76 ms | 3.80 ms | 3.59 ms | 4.11 ms | 598 µs | 610 µs | 761 µs |
| wall time | 11.1 ms | 11.2 ms | 11.2 ms | 11.7 ms | 703 µs | 711 µs | 874 µs |
| peak memory | 8.14 MiB | 7.57 MiB | 7.57 MiB | 7.67 MiB | 1.35 MiB | 1.35 MiB | 1.46 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 23,505 | 25,995 | 25,365 | 30,489 | 1,686 | 1,615 | 2,603 |

Its rounds took 70 seconds.

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
| counted runs | 320 | 320 | 320 | 160 | 20 | 20 | 20 |
| CPU time | 3.59 ms | 3.69 ms | 3.68 ms | 3.98 ms | 667 µs | 683 µs | 764 µs |
| wall time | 10.8 ms | 11.1 ms | 11.3 ms | 11.5 ms | 765 µs | 779 µs | 872 µs |
| peak memory | 8.14 MiB | 7.57 MiB | 7.57 MiB | 7.69 MiB | 1.48 MiB | 1.48 MiB | 1.53 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 23,977 | 25,680 | 25,372 | 30,043 | 2,002 | 2,041 | 2,601 |

Its rounds took 17 seconds.

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
| counted runs | 640 | 640 | 320 | 160 | 20 | 20 | 20 |
| CPU time | 4.56 ms | 4.66 ms | 4.66 ms | 4.91 ms | 1.53 ms | 1.52 ms | 1.69 ms |
| wall time | 11.6 ms | 12.0 ms | 12.2 ms | 12.4 ms | 1.73 ms | 1.70 ms | 1.92 ms |
| peak memory | 10.2 MiB | 9.88 MiB | 9.83 MiB | 10.1 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 25,157 | 27,238 | 26,777 | 32,035 | 2,118 | 2,185 | 3,023 |

Its rounds took 28 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 20 |
| CPU time | 15.3 ms | 15.2 ms | 15.3 ms | 15.6 ms | 10.7 ms | 10.9 ms | 11.3 ms |
| wall time | 22.4 ms | 22.5 ms | 22.8 ms | 23.0 ms | 10.8 ms | 11.0 ms | 11.5 ms |
| peak memory | 13.0 MiB | 12.8 MiB | 12.8 MiB | 12.9 MiB | 1000 KiB | 1000 KiB | 1.64 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 159,448 | 163,107 | 165,250 | 179,951 | 25,422 | 24,602 | 48,298 |

Its rounds took 172 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 4.19 ms | 4.07 ms | 3.90 ms | 4.52 ms | 1.15 ms | 1.14 ms | 1.36 ms |
| wall time | 11.5 ms | 11.3 ms | 11.4 ms | 12.3 ms | 1.24 ms | 1.23 ms | 1.48 ms |
| peak memory | 9.14 MiB | 8.28 MiB | 8.39 MiB | 8.47 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,701 | 24,096 | 24,204 | 29,683 | 1,971 | 1,818 | 3,032 |

Its rounds took 95 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 22.0 ms | 30.9 ms | 189 ms | 30.6 ms | 18.6 ms | 21.4 ms | 23.1 ms |
| wall time | 29.3 ms | 38.3 ms | 197 ms | 37.9 ms | 18.8 ms | 21.5 ms | 23.3 ms |
| peak memory | 13.0 MiB | 13.8 MiB | 13.7 MiB | 13.9 MiB | 1.47 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 137,224 | 151,207 | 141,841 | 161,382 | 24,157 | 23,812 | 26,458 |

Its rounds took 9 seconds.

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
| CPU time | 40.6 ms | 72.5 ms | 72.5 ms | 42.5 ms | 36.0 ms | 36.0 ms | 36.4 ms |
| wall time | 47.4 ms | 80.3 ms | 80.0 ms | 50.0 ms | 36.1 ms | 36.1 ms | 36.6 ms |
| peak memory | 13.7 MiB | 13.8 MiB | 13.7 MiB | 13.8 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 188,534 | 197,916 | 196,182 | 219,399 | 84,171 | 84,954 | 101,814 |

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
| CPU time | 38.3 ms | 48.7 ms | 48.8 ms | 51.8 ms | 15.7 ms | 15.8 ms | 11.2 ms |
| wall time | 44.7 ms | 56.2 ms | 56.2 ms | 58.8 ms | 15.8 ms | 15.9 ms | 11.3 ms |
| peak memory | 13.8 MiB | 13.8 MiB | 13.7 MiB | 13.9 MiB | 1.48 MiB | 1.48 MiB | 1.64 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 312,765 | 245,890 | 255,517 | 255,208 | 26,575 | 26,488 | 26,851 |

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
| CPU time | 299 ms | 1.53 s | 533 ms | 462 ms | 428 ms | 415 ms | 267 ms |
| wall time | 307 ms | 1.54 s | 541 ms | 468 ms | 428 ms | 415 ms | 267 ms |
| peak memory | 83.3 MiB | 83.2 MiB | 91.3 MiB | 91.2 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,317,429 | 30,428,329 | 12,278,765 | 12,129,949 | 14,616,789 | 15,532,479 | 12,489,795 |

Its rounds took 93 seconds.

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
| CPU time | 142 ms | 293 ms | 292 ms | 408 ms | 237 ms | 245 ms | 252 ms |
| wall time | 149 ms | 301 ms | 299 ms | 415 ms | 237 ms | 245 ms | 252 ms |
| peak memory | 14.7 MiB | 14.4 MiB | 14.4 MiB | 14.2 MiB | 2.11 MiB | 3.92 MiB | 1.97 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,855,855 | 8,666,692 | 8,655,446 | 8,138,105 | 8,522,797 | 8,386,431 | 11,274,591 |

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
| counted runs | 260 | 260 | 160 | 160 | 20 | 20 | 20 |
| CPU time | 596 ms | 602 ms | 604 ms | 606 ms | 447 ms | 449 ms | 762 ms |
| wall time | 597 ms | 637 ms | 630 ms | 642 ms | 453 ms | 453 ms | 382 ms |
| peak memory | 13.7 MiB | 13.7 MiB | 13.8 MiB | 13.9 MiB | 724 KiB | 2.54 MiB | 2.06 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,360,224 | 2,437,064 | 2,754,326 | 2,433,985 | 129,962 | 155,828 | 516,369 |

Its rounds took 613 seconds.

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
| CPU time | 44.3 ms | 47.8 ms | 47.9 ms | 48.7 ms | 17.6 ms | 17.7 ms | 14.6 ms |
| wall time | 51.5 ms | 54.9 ms | 55.5 ms | 55.8 ms | 17.7 ms | 17.8 ms | 14.8 ms |
| peak memory | 14.2 MiB | 14.1 MiB | 14.1 MiB | 14.3 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,452,315 | 7,568,182 | 7,557,596 | 7,569,376 | 9,735 | 9,910 | 13,871 |

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 102 ms | 105 ms | 103 ms | 108 ms | 32.2 ms | 33.2 ms | 32.8 ms |
| wall time | 108 ms | 113 ms | 111 ms | 116 ms | 32.3 ms | 33.3 ms | 33.0 ms |
| peak memory | 13.9 MiB | 13.8 MiB | 13.8 MiB | 13.9 MiB | 1.35 MiB | 3.16 MiB | 1.47 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,899 | 27,138 | 26,789 | 31,679 | 1,761 | 13,638 | 2,760 |

Its rounds took 13 seconds.

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
| empty | 160, 160 in 5 s | 3.28 ms against 3.13 ms: +141 µs (+122 µs to +162 µs) | 9.84 ms against 10.6 ms: -751 µs (-1.37 ms to -290 µs) | 8.08 MiB against 7.45 MiB: +636 KiB (+600 KiB to +640 KiB) |
| add | 160, 160 in 5 s | 3.60 ms against 3.44 ms: +168 µs (+135 µs to +201 µs) | 10.6 ms against 11.2 ms: -674 µs (-1.17 ms to -122 µs) | 8.14 MiB against 7.52 MiB: +656 KiB (+644 KiB to +672 KiB) |
| hypotenuse | 160, 160 in 5 s | 3.45 ms against 3.23 ms: +216 µs (+191 µs to +240 µs) | 11.1 ms against 11.3 ms: -400 µs (-891 µs to +55.0 µs) | 8.14 MiB against 7.52 MiB: +624 KiB (+560 KiB to +636 KiB) |
| upcase | 160, 160 in 5 s | 4.51 ms against 4.26 ms: +254 µs (+217 µs to +289 µs) | 11.7 ms against 11.6 ms: +61.1 µs (-465 µs to +574 µs) | 10.6 MiB against 9.67 MiB: +776 KiB (+616 KiB to +1.01 MiB) |
| upcase-stdin | 160, 160 in 9 s | 15.8 ms against 15.7 ms: +115 µs (-9.00 µs to +244 µs) | 22.5 ms against 22.9 ms: -421 µs (-1.02 ms to +169 µs) | 13.0 MiB against 12.8 MiB: +192 KiB (+184 KiB to +192 KiB) |
| shout | 160, 160 in 5 s | 4.17 ms against 3.99 ms: +189 µs (+156 µs to +222 µs) | 11.4 ms against 11.2 ms: +168 µs (-372 µs to +726 µs) | 9.14 MiB against 8.39 MiB: +772 KiB (+768 KiB to +792 KiB) |
| wc | 160, 160 in 10 s | 21.5 ms against 21.2 ms: +253 µs (+174 µs to +334 µs) | 28.6 ms against 28.5 ms: +253 µs (-281 µs to +791 µs) | 13.0 MiB against 12.9 MiB: +188 KiB (+172 KiB to +192 KiB) |
| histogram | 160, 160 in 16 s | 40.3 ms against 40.1 ms: +193 µs (+125 µs to +263 µs) | 47.1 ms against 47.4 ms: -77.0 µs (-572 µs to +408 µs) | 13.7 MiB against 13.7 MiB: +32.0 KiB (+4.00 KiB to +56.0 KiB) |
| crc32c | 160, 160 in 16 s | 38.1 ms against 38.0 ms: +133 µs (+61.0 µs to +206 µs) | 45.0 ms against 45.2 ms: -293 µs (-787 µs to +202 µs) | 13.8 MiB against 13.7 MiB: +64.0 KiB (+64.0 KiB to +68.0 KiB) |
| sort | 97, 97 in 61 s | 301 ms against 301 ms: +494 µs (-1.33 ms to +2.40 ms) | 308 ms against 309 ms: +45.4 µs (-1.95 ms to +2.23 ms) | 83.3 MiB against 83.3 MiB: +48.0 KiB (+8.00 KiB to +64.0 KiB) |
| tally | 160, 160 in 49 s | 142 ms against 142 ms: +555 µs (+138 µs to +949 µs) | 150 ms against 149 ms: +247 µs (-431 µs to +923 µs) | 14.7 MiB against 14.7 MiB: +48.0 KiB (+12.0 KiB to +64.0 KiB) |
| server | 46, 46 in 60 s | 596 ms against 597 ms: +1.01 ms (-6.83 ms to +8.59 ms) | 597 ms against 599 ms: +803 µs (-7.15 ms to +9.00 ms) | 13.7 MiB against 13.6 MiB: +116 KiB (+64.0 KiB to +160 KiB) |
| files | 160, 160 in 18 s | 44.3 ms against 44.0 ms: +323 µs (+249 µs to +395 µs) | 51.2 ms against 51.3 ms: +161 µs (-342 µs to +664 µs) | 14.2 MiB against 14.1 MiB: +96.0 KiB (+64.0 KiB to +120 KiB) |
| readings | 160, 160 in 36 s | 102 ms against 101 ms: +690 µs (+666 µs to +715 µs) | 109 ms against 108 ms: +300 µs (-221 µs to +659 µs) | 13.8 MiB against 13.7 MiB: +64.0 KiB (+60.0 KiB to +72.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
