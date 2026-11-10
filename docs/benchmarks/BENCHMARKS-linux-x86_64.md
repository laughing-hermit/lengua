# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 076bcc1eee81607c6236fee34fc52418049e4a69, in https://github.com/laughing-hermit/lengua/actions/runs/42756074550. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (22.7 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ✗ ✗ ✗ ✓ | ✗ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ✗ ✓ | ✗ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 upcase rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 77 names the row: take it out, as the list only shrinks
- linux-x86_64 wc cpp-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 histogram c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 138 names the row: take it out, as the list only shrinks
- linux-x86_64 histogram c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 139 names the row: take it out, as the list only shrinks
- linux-x86_64 histogram cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 141 names the row: take it out, as the list only shrinks
- linux-x86_64 histogram cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 142 names the row: take it out, as the list only shrinks
- linux-x86_64 histogram rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 144 names the row: take it out, as the list only shrinks
- linux-x86_64 histogram rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 145 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 159 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 160 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 162 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 163 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 165 names the row: take it out, as the list only shrinks
- linux-x86_64 crc32c rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 166 names the row: take it out, as the list only shrinks
- linux-x86_64 sort cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 181 names the row: take it out, as the list only shrinks
- linux-x86_64 sort cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 182 names the row: take it out, as the list only shrinks
- linux-x86_64 sort rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 183 names the row: take it out, as the list only shrinks
- linux-x86_64 sort rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 184 names the row: take it out, as the list only shrinks
- linux-x86_64 sort c cpu: lengua is ahead now, and KNOWN_BEHIND line 185 names the row: take it out, as the list only shrinks
- linux-x86_64 sort c wall: lengua is ahead now, and KNOWN_BEHIND line 186 names the row: take it out, as the list only shrinks
- linux-x86_64 sort c memory: lengua is ahead now, and KNOWN_BEHIND line 187 names the row: take it out, as the list only shrinks
- linux-x86_64 sort cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 189 names the row: take it out, as the list only shrinks
- linux-x86_64 sort cpp wall: lengua is ahead now, and KNOWN_BEHIND line 190 names the row: take it out, as the list only shrinks
- linux-x86_64 tally c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 197 names the row: take it out, as the list only shrinks
- linux-x86_64 tally c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 198 names the row: take it out, as the list only shrinks
- linux-x86_64 tally cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 200 names the row: take it out, as the list only shrinks
- linux-x86_64 tally cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 201 names the row: take it out, as the list only shrinks
- linux-x86_64 tally rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 203 names the row: take it out, as the list only shrinks
- linux-x86_64 tally rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 204 names the row: take it out, as the list only shrinks
- linux-x86_64 tally c cpu: lengua is ahead now, and KNOWN_BEHIND line 206 names the row: take it out, as the list only shrinks
- linux-x86_64 tally c wall: lengua is ahead now, and KNOWN_BEHIND line 207 names the row: take it out, as the list only shrinks
- linux-x86_64 tally cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 210 names the row: take it out, as the list only shrinks
- linux-x86_64 tally cpp wall: lengua is ahead now, and KNOWN_BEHIND line 211 names the row: take it out, as the list only shrinks
- linux-x86_64 tally rust cpu: lengua is ahead now, and KNOWN_BEHIND line 214 names the row: take it out, as the list only shrinks
- linux-x86_64 tally rust wall: lengua is ahead now, and KNOWN_BEHIND line 215 names the row: take it out, as the list only shrinks
- linux-x86_64 server c-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 server cpp-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 server rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 files c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 232 names the row: take it out, as the list only shrinks
- linux-x86_64 files cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 234 names the row: take it out, as the list only shrinks
- linux-x86_64 files cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 235 names the row: take it out, as the list only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 5.22× | 4.11× | 8.56× | 1659.66× |
| add | 5.33× | 3.97× | 6.39× | 1644.13× |
| hypotenuse | 4.56× | 3.47× | 5.57× | 1644.15× |
| upcase | 2.69× | 2.39× | 4.95× | 1644.18× |
| upcase-stdin | 1.75× | 1.67× | 12.25× | 1644.12× |
| shout | 3.18× | 3.00× | 5.12× | 1644.13× |
| wc | 1.37× | 1.33× | 9.64× | 1644.17× |
| histogram | 1.18× | 1.16× | 9.87× | 1644.15× |
| crc32c | 2.90× | 2.86× | 9.88× | 1644.15× |
| sort | 1.12× | 1.12× | 1.13× | 1644.31× |
| tally | 0.50× | 0.50× | 7.74× | 1644.35× |
| server | 1.08× | 1.86× | 14.60× | 1633.33× |
| files | 2.66× | 2.63× | 10.30× | 1644.16× |
| readings | 3.38× | 3.36× | 10.30× | 1644.12× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 80 | behind, 1.10×, p 2.1e-11 | behind, 1.05×, p 1.2e-4 | behind, 1.10× | ahead, 0.89× |
| empty | cpp-wasm | 80 | behind, 1.10×, p 1.2e-11 | behind, 1.04×, p 4.5e-4 | behind, 1.10× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.89×, p 3.0e-7 | ahead, 0.86×, p 7.2e-8 | behind, 1.08× | ahead, 0.01× |
| empty | c | 20 | behind, 5.20×, p 3.4e-8 | behind, 4.01×, p 3.4e-8 | behind, 8.34× | behind, 1659.66× |
| empty | cpp | 20 | behind, 5.22×, p 3.4e-8 | behind, 4.11×, p 3.4e-8 | behind, 8.56× | behind, 1659.66× |
| empty | rust | 20 | behind, 3.74×, p 3.4e-8 | behind, 3.03×, p 3.4e-8 | behind, 5.66× | behind, 83.25× |
| add | c-wasm | 20 | ahead, 0.96×, p 1.4e-4 | ahead, 0.89×, p 1.9e-4 | behind, 1.07× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.95×, p 2.4e-4 | ahead, 0.89×, p 4.1e-5 | behind, 1.08× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.86×, p 3.4e-8 | ahead, 0.79×, p 6.5e-5 | behind, 1.07× | ahead, 0.02× |
| add | c | 20 | behind, 5.23×, p 3.4e-8 | behind, 3.94×, p 3.4e-8 | behind, 6.06× | behind, 1644.13× |
| add | cpp | 20 | behind, 5.33×, p 3.4e-8 | behind, 3.97×, p 3.4e-8 | behind, 6.39× | behind, 1644.13× |
| add | rust | 20 | behind, 4.12×, p 3.4e-8 | behind, 3.18×, p 3.4e-8 | behind, 5.64× | behind, 80.77× |
| hypotenuse | c-wasm | 40 | ahead, 0.96×, p 8.8e-4 | ahead, 0.89×, p 8.5e-7 | behind, 1.07× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 80 | ahead, 0.97×, p 1.2e-4 | ahead, 0.92×, p 1.1e-11 | behind, 1.07× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.87×, p 5.2e-7 | ahead, 0.80×, p 4.3e-6 | behind, 1.07× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 4.56×, p 3.4e-8 | behind, 3.47×, p 3.4e-8 | behind, 5.14× | behind, 1644.15× |
| hypotenuse | cpp | 20 | behind, 4.43×, p 3.4e-8 | behind, 3.35×, p 3.4e-8 | behind, 5.28× | behind, 1644.15× |
| hypotenuse | rust | 20 | behind, 3.97×, p 3.4e-8 | behind, 3.04×, p 3.4e-8 | behind, 5.57× | behind, 80.10× |
| upcase | c-wasm | 160 | ahead, 0.97×, p 2.6e-8 | ahead, 0.92×, p 3.6e-7 | behind, 1.07× | ahead, 0.06× |
| upcase | cpp-wasm | 160 | ahead, 0.97×, p 6.0e-8 | ahead, 0.92×, p 2.8e-7 | behind, 1.08× | ahead, 0.06× |
| upcase | rust-wasm | 20 | ahead, 0.85×, p 2.3e-7 | ahead, 0.74×, p 5.2e-5 | ahead, 0.91× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.68×, p 3.4e-8 | behind, 2.39×, p 3.4e-8 | behind, 4.88× | behind, 1644.18× |
| upcase | cpp | 20 | behind, 2.69×, p 3.4e-8 | behind, 2.39×, p 6.0e-7 | behind, 4.95× | behind, 1644.18× |
| upcase | rust | 20 | behind, 2.31×, p 3.4e-8 | behind, 2.06×, p 3.4e-8 | behind, 4.10× | behind, 80.86× |
| upcase-stdin | c-wasm | 20 | ahead, 0.86×, p 1.7e-7 | ahead, 0.83×, p 1.7e-7 | behind, 1.04× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.39×, p 3.4e-8 | ahead, 0.38×, p 3.4e-8 | behind, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 20 | ahead, 0.91×, p 1.2e-6 | ahead, 0.89×, p 6.0e-7 | behind, 1.02× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.72×, p 3.4e-8 | behind, 1.65×, p 3.4e-8 | behind, 11.97× | behind, 1644.12× |
| upcase-stdin | cpp | 20 | behind, 1.73×, p 3.4e-8 | behind, 1.66×, p 3.4e-8 | behind, 12.25× | behind, 1644.12× |
| upcase-stdin | rust | 20 | behind, 1.75×, p 3.4e-8 | behind, 1.67×, p 3.4e-8 | behind, 8.33× | behind, 82.30× |
| shout | c-wasm | 1280 | behind, 1.03×, p 6.1e-19 | behind, 1.02×, p 1.0e-5 | behind, 1.11× | ahead, 0.40× |
| shout | cpp-wasm | 1280 | behind, 1.03×, p 8.3e-22 | behind, 1.01×, p 1.6e-4 | behind, 1.11× | ahead, 0.40× |
| shout | rust-wasm | 80 | ahead, 0.91×, p 3.6e-16 | ahead, 0.88×, p 1.4e-6 | behind, 1.09× | ahead, 0.03× |
| shout | c | 20 | behind, 3.18×, p 3.4e-8 | behind, 3.00×, p 3.4e-8 | behind, 5.05× | behind, 1644.13× |
| shout | cpp | 20 | behind, 3.16×, p 3.4e-8 | behind, 2.96×, p 3.4e-8 | behind, 5.12× | behind, 1644.13× |
| shout | rust | 20 | behind, 2.62×, p 3.4e-8 | behind, 2.49×, p 3.4e-8 | behind, 4.19× | behind, 81.32× |
| wc | c-wasm | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.89×, p 6.0e-7 | behind, 1.04× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.11×, p 3.4e-8 | ahead, 0.11×, p 3.4e-8 | behind, 1.01× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.87×, p 3.4e-8 | ahead, 0.85×, p 6.0e-7 | behind, 1.04× | ahead, 0.04× |
| wc | c | 20 | behind, 1.11×, p 3.4e-8 | behind, 1.08×, p 3.4e-8 | behind, 9.64× | behind, 1644.17× |
| wc | cpp | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.80×, p 6.0e-7 | behind, 9.64× | behind, 1644.17× |
| wc | rust | 20 | behind, 1.37×, p 3.4e-8 | behind, 1.33×, p 3.4e-8 | behind, 8.42× | behind, 79.54× |
| histogram | c-wasm | 20 | ahead, 0.75×, p 3.4e-8 | ahead, 0.75×, p 3.4e-8 | behind, 1.03× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.67×, p 3.4e-8 | behind, 1.04× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | behind, 1.02× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.18×, p 3.0e-7 | behind, 1.16×, p 2.6e-7 | behind, 9.87× | behind, 1644.15× |
| histogram | cpp | 20 | behind, 1.17×, p 3.4e-8 | behind, 1.16×, p 3.4e-8 | behind, 9.87× | behind, 1644.15× |
| histogram | rust | 20 | behind, 1.17×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 8.59× | behind, 79.50× |
| crc32c | c-wasm | 20 | ahead, 0.78×, p 6.0e-7 | ahead, 0.77×, p 6.0e-7 | behind, 1.04× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.79×, p 6.0e-7 | ahead, 0.79×, p 6.0e-7 | behind, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.76×, p 2.0e-7 | ahead, 0.76×, p 2.0e-7 | behind, 1.03× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.81×, p 3.4e-8 | behind, 2.77×, p 3.4e-8 | behind, 9.88× | behind, 1644.15× |
| crc32c | cpp | 20 | behind, 2.82×, p 3.4e-8 | behind, 2.77×, p 3.4e-8 | behind, 9.88× | behind, 1644.15× |
| crc32c | rust | 20 | behind, 2.90×, p 3.4e-8 | behind, 2.86×, p 3.4e-8 | behind, 8.55× | behind, 81.54× |
| sort | c-wasm | 20 | ahead, 0.18×, p 3.4e-8 | ahead, 0.18×, p 3.4e-8 | behind, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.61×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | ahead, 0.99× | behind, 1644.31× |
| sort | cpp | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | behind, 1.04× | behind, 1274.99× |
| sort | rust | 20 | behind, 1.12×, p 1.7e-7 | behind, 1.12×, p 1.7e-7 | behind, 1.13× | behind, 79.45× |
| tally | c-wasm | 20 | ahead, 0.36×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.05× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.36×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.05× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.29×, p 3.4e-8 | ahead, 0.29×, p 3.4e-8 | behind, 1.06× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.49×, p 3.4e-8 | ahead, 0.49×, p 3.4e-8 | behind, 7.54× | behind, 1644.35× |
| tally | cpp | 20 | ahead, 0.50×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | behind, 3.87× | behind, 1281.61× |
| tally | rust | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.45×, p 3.4e-8 | behind, 7.74× | behind, 77.34× |
| server | c-wasm | 169 | undecided, 1.00×, p 0.056 | ahead, 0.93×, p 4.6e-42 | behind, 1.05× | ahead, 0.12× |
| server | cpp-wasm | 169 | undecided, 1.00×, p 0.002 | ahead, 0.95×, p 7.9e-39 | behind, 1.04× | ahead, 0.06× |
| server | rust-wasm | 169 | undecided, 1.00×, p 0.027 | ahead, 0.94×, p 8.1e-34 | behind, 1.03× | ahead, 0.03× |
| server | c | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 14.60× | behind, 1281.47× |
| server | cpp | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 5.27× | behind, 1633.33× |
| server | rust | 20 | ahead, 0.96×, p 3.4e-8 | behind, 1.86×, p 3.4e-8 | behind, 6.60× | behind, 74.19× |
| files | c-wasm | 20 | ahead, 0.89×, p 3.9e-8 | ahead, 0.88×, p 3.9e-8 | behind, 1.05× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.90×, p 3.9e-8 | ahead, 0.89×, p 3.4e-8 | behind, 1.05× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.89×, p 3.4e-8 | ahead, 0.89×, p 3.4e-8 | behind, 1.04× | ahead, 0.03× |
| files | c | 20 | behind, 2.03×, p 3.4e-8 | behind, 2.01×, p 3.4e-8 | behind, 10.30× | behind, 1644.16× |
| files | cpp | 20 | behind, 2.02×, p 3.4e-8 | behind, 2.00×, p 3.4e-8 | behind, 10.30× | behind, 1644.16× |
| files | rust | 20 | behind, 2.66×, p 3.4e-8 | behind, 2.63×, p 3.4e-8 | behind, 8.53× | behind, 79.78× |
| readings | c-wasm | 20 | ahead, 0.81×, p 3.4e-8 | ahead, 0.81×, p 3.4e-8 | behind, 1.03× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.90×, p 3.4e-8 | behind, 1.03× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.86×, p 3.4e-8 | ahead, 0.86×, p 3.4e-8 | behind, 1.02× | ahead, 0.02× |
| readings | c | 20 | behind, 3.38×, p 3.4e-8 | behind, 3.36×, p 3.4e-8 | behind, 10.30× | behind, 1644.12× |
| readings | cpp | 20 | behind, 3.08×, p 3.4e-8 | behind, 3.05×, p 3.4e-8 | behind, 4.45× | behind, 1644.12× |
| readings | rust | 20 | behind, 3.37×, p 3.4e-8 | behind, 3.34×, p 3.4e-8 | behind, 9.35× | behind, 79.45× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 2.35 ms | 2.13 ms | 2.14 ms | 2.59 ms | 444 µs | 441 µs | 616 µs |
| wall time | 2.15 ms | 2.05 ms | 2.06 ms | 2.44 ms | 525 µs | 512 µs | 694 µs |
| peak memory | 10.5 MiB | 9.49 MiB | 9.52 MiB | 9.67 MiB | 1.25 MiB | 1.22 MiB | 1.85 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 14.0 KiB | 14.0 KiB | 279 KiB |

Its rounds took 2 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 2.55 ms | 2.66 ms | 2.67 ms | 2.98 ms | 486 µs | 478 µs | 618 µs |
| wall time | 2.23 ms | 2.49 ms | 2.49 ms | 2.82 ms | 565 µs | 561 µs | 700 µs |
| peak memory | 10.4 MiB | 9.77 MiB | 9.65 MiB | 9.78 MiB | 1.72 MiB | 1.63 MiB | 1.85 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 14.1 KiB | 14.1 KiB | 288 KiB |

Its rounds took 1 seconds.

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
| counted runs | 80 | 40 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 2.48 ms | 2.59 ms | 2.56 ms | 2.86 ms | 543 µs | 559 µs | 624 µs |
| wall time | 2.20 ms | 2.48 ms | 2.39 ms | 2.70 ms | 622 µs | 644 µs | 710 µs |
| peak memory | 10.4 MiB | 9.75 MiB | 9.75 MiB | 9.73 MiB | 2.03 MiB | 1.97 MiB | 1.87 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 2 seconds.

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
| counted runs | 160 | 160 | 160 | 20 | 20 | 20 | 20 |
| CPU time | 3.83 ms | 3.94 ms | 3.95 ms | 4.40 ms | 1.40 ms | 1.39 ms | 1.63 ms |
| wall time | 3.64 ms | 3.95 ms | 3.94 ms | 5.14 ms | 1.60 ms | 1.60 ms | 1.86 ms |
| peak memory | 11.6 MiB | 10.8 MiB | 10.8 MiB | 12.8 MiB | 2.38 MiB | 2.35 MiB | 2.84 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 287 KiB |

Its rounds took 5 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 11.8 ms | 13.7 ms | 30.1 ms | 12.9 ms | 6.88 ms | 6.81 ms | 6.75 ms |
| wall time | 11.5 ms | 13.9 ms | 30.0 ms | 12.9 ms | 6.97 ms | 6.90 ms | 6.87 ms |
| peak memory | 16.5 MiB | 15.8 MiB | 16.2 MiB | 16.1 MiB | 1.38 MiB | 1.35 MiB | 1.98 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 282 KiB |

Its rounds took 5 seconds.

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
| counted runs | 1280 | 1280 | 1280 | 80 | 20 | 20 | 20 |
| CPU time | 3.10 ms | 3.03 ms | 3.02 ms | 3.51 ms | 1.00 ms | 1.01 ms | 1.22 ms |
| wall time | 2.97 ms | 2.91 ms | 2.93 ms | 3.38 ms | 1.09 ms | 1.10 ms | 1.31 ms |
| peak memory | 11.3 MiB | 10.2 MiB | 10.2 MiB | 10.4 MiB | 2.25 MiB | 2.22 MiB | 2.72 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 29 seconds.

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
| CPU time | 20.2 ms | 22.6 ms | 179 ms | 23.2 ms | 18.3 ms | 24.7 ms | 14.8 ms |
| wall time | 19.8 ms | 22.3 ms | 179 ms | 23.4 ms | 18.4 ms | 24.8 ms | 14.9 ms |
| peak memory | 16.6 MiB | 16.0 MiB | 16.5 MiB | 16.1 MiB | 1.73 MiB | 1.73 MiB | 1.98 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 8 seconds.

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
| CPU time | 29.4 ms | 39.0 ms | 43.6 ms | 37.2 ms | 24.9 ms | 25.1 ms | 25.2 ms |
| wall time | 29.1 ms | 38.8 ms | 43.4 ms | 37.0 ms | 25.0 ms | 25.2 ms | 25.3 ms |
| peak memory | 17.0 MiB | 16.5 MiB | 16.4 MiB | 16.7 MiB | 1.73 MiB | 1.73 MiB | 1.98 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 7 seconds.

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
| CPU time | 35.3 ms | 45.4 ms | 44.6 ms | 46.3 ms | 12.6 ms | 12.5 ms | 12.2 ms |
| wall time | 35.1 ms | 45.4 ms | 44.5 ms | 46.1 ms | 12.7 ms | 12.6 ms | 12.3 ms |
| peak memory | 17.1 MiB | 16.5 MiB | 16.6 MiB | 16.6 MiB | 1.73 MiB | 1.73 MiB | 2.00 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 14.1 KiB | 14.1 KiB | 285 KiB |

Its rounds took 6 seconds.

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
| CPU time | 248 ms | 1.40 s | 428 ms | 407 ms | 390 ms | 345 ms | 222 ms |
| wall time | 248 ms | 1.40 s | 428 ms | 407 ms | 390 ms | 346 ms | 222 ms |
| peak memory | 86.6 MiB | 85.9 MiB | 94.0 MiB | 94.0 MiB | 87.2 MiB | 83.3 MiB | 76.8 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 292 KiB |

Its rounds took 81 seconds.

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
| CPU time | 102 ms | 282 ms | 284 ms | 348 ms | 209 ms | 204 ms | 229 ms |
| wall time | 102 ms | 281 ms | 284 ms | 348 ms | 210 ms | 204 ms | 229 ms |
| peak memory | 18.0 MiB | 17.0 MiB | 17.2 MiB | 16.9 MiB | 2.38 MiB | 4.64 MiB | 2.32 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 300 KiB |

Its rounds took 39 seconds.

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
| counted runs | 169 | 169 | 169 | 169 | 20 | 20 | 20 |
| CPU time | 688 ms | 689 ms | 690 ms | 689 ms | 641 ms | 640 ms | 720 ms |
| wall time | 689 ms | 743 ms | 723 ms | 733 ms | 649 ms | 646 ms | 370 ms |
| peak memory | 17.1 MiB | 16.3 MiB | 16.4 MiB | 16.5 MiB | 1.16 MiB | 3.22 MiB | 2.57 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 18.1 KiB | 14.2 KiB | 313 KiB |

Its rounds took 618 seconds.

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
| CPU time | 49.1 ms | 55.4 ms | 54.7 ms | 54.8 ms | 24.1 ms | 24.2 ms | 18.5 ms |
| wall time | 48.7 ms | 55.3 ms | 54.6 ms | 54.6 ms | 24.2 ms | 24.3 ms | 18.6 ms |
| peak memory | 17.8 MiB | 17.0 MiB | 17.0 MiB | 17.1 MiB | 1.73 MiB | 1.73 MiB | 2.09 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 8 seconds.

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
| CPU time | 80.7 ms | 99.2 ms | 89.4 ms | 93.6 ms | 23.8 ms | 26.2 ms | 24.0 ms |
| wall time | 80.3 ms | 99.2 ms | 89.3 ms | 93.6 ms | 23.9 ms | 26.3 ms | 24.1 ms |
| peak memory | 17.1 MiB | 16.6 MiB | 16.6 MiB | 16.7 MiB | 1.66 MiB | 3.85 MiB | 1.83 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 11 seconds.

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
| empty | 160, 160 in 2 s | 2.26 ms against 2.10 ms: +172 µs (+138 µs to +206 µs) | 2.12 ms against 2.03 ms: +79.1 µs (+43.5 µs to +117 µs) | 10.5 MiB against 9.61 MiB: +868 KiB (+828 KiB to +912 KiB) |
| add | 160, 160 in 2 s | 2.38 ms against 2.16 ms: +201 µs (+171 µs to +232 µs) | 2.11 ms against 2.06 ms: +58.1 µs (+28.2 µs to +89.0 µs) | 10.4 MiB against 9.71 MiB: +736 KiB (+700 KiB to +772 KiB) |
| hypotenuse | 160, 160 in 2 s | 2.42 ms against 2.18 ms: +217 µs (+187 µs to +246 µs) | 2.14 ms against 2.07 ms: +47.7 µs (+18.3 µs to +76.5 µs) | 10.5 MiB against 9.70 MiB: +768 KiB (+732 KiB to +808 KiB) |
| upcase | 160, 160 in 2 s | 3.77 ms against 3.55 ms: +210 µs (+156 µs to +259 µs) | 3.62 ms against 3.58 ms: +49.8 µs (-49.4 µs to +146 µs) | 11.7 MiB against 10.9 MiB: +900 KiB (+760 KiB to +1008 KiB) |
| upcase-stdin | 160, 160 in 7 s | 12.3 ms against 12.2 ms: -67.0 µs (-247 µs to +77.0 µs) | 11.9 ms against 12.3 ms: -402 µs (-686 µs to -202 µs) | 16.6 MiB against 15.9 MiB: +684 KiB (+640 KiB to +728 KiB) |
| shout | 160, 160 in 2 s | 3.05 ms against 2.79 ms: +253 µs (+213 µs to +293 µs) | 2.96 ms against 2.85 ms: +116 µs (+38.0 µs to +203 µs) | 11.3 MiB against 10.4 MiB: +1.02 MiB (+972 KiB to +1.10 MiB) |
| wc | 160, 160 in 8 s | 20.0 ms against 19.9 ms: +96.0 µs (-22.0 µs to +199 µs) | 19.6 ms against 19.8 ms: -86.2 µs (-217 µs to +19.0 µs) | 16.6 MiB against 15.9 MiB: +680 KiB (+640 KiB to +716 KiB) |
| histogram | 160, 160 in 12 s | 29.5 ms against 31.4 ms: -1.45 ms (-2.53 ms to -813 µs) | 29.3 ms against 31.6 ms: -1.50 ms (-2.60 ms to -841 µs) | 17.0 MiB against 16.3 MiB: +680 KiB (+636 KiB to +720 KiB) |
| crc32c | 160, 160 in 13 s | 34.6 ms against 34.9 ms: -577 µs (-977 µs to -271 µs) | 34.3 ms against 34.9 ms: -785 µs (-1.20 ms to -469 µs) | 17.1 MiB against 16.4 MiB: +696 KiB (+656 KiB to +732 KiB) |
| sort | 119, 119 in 60 s | 246 ms against 248 ms: -525 µs (-2.72 ms to +1.60 ms) | 246 ms against 248 ms: -698 µs (-2.92 ms to +1.44 ms) | 86.6 MiB against 85.8 MiB: +740 KiB (+700 KiB to +780 KiB) |
| tally | 160, 160 in 35 s | 102 ms against 103 ms: -769 µs (-1.84 ms to -270 µs) | 102 ms against 103 ms: -999 µs (-2.02 ms to -528 µs) | 17.9 MiB against 17.2 MiB: +720 KiB (+692 KiB to +752 KiB) |
| server | 40, 40 in 60 s | 695 ms against 692 ms: +2.51 ms (-1.08 ms to +5.22 ms) | 694 ms against 692 ms: +2.42 ms (-640 µs to +5.71 ms) | 17.1 MiB against 16.3 MiB: +844 KiB (+768 KiB to +908 KiB) |
| files | 160, 160 in 17 s | 49.1 ms against 49.1 ms: -150 µs (-394 µs to +71.0 µs) | 48.8 ms against 49.0 ms: -335 µs (-593 µs to -112 µs) | 17.8 MiB against 17.2 MiB: +620 KiB (+588 KiB to +656 KiB) |
| readings | 160, 160 in 28 s | 80.6 ms against 82.6 ms: -2.05 ms (-2.31 ms to -1.84 ms) | 80.4 ms against 82.5 ms: -2.23 ms (-2.50 ms to -2.01 ms) | 17.1 MiB against 16.5 MiB: +636 KiB (+604 KiB to +664 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
