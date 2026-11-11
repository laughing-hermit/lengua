# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 7763 64-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 33ed7fe1f45158efe1a2695d8accd88e1dfb18a5, in https://github.com/laughing-hermit/lengua/actions/runs/42947294489. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (23.1 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| upcase | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ✗ ✓ | ✗ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 upcase rust-wasm memory: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 upcase-stdin c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 90 names the row: take it out, as the list only shrinks
- linux-x86_64 upcase-stdin cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 92 names the row: take it out, as the list only shrinks
- linux-x86_64 server c-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 server rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 5.08× | 3.81× | 8.76× | 1689.17× |
| add | 4.98× | 3.76× | 6.95× | 1673.36× |
| hypotenuse | 4.42× | 3.42× | 6.15× | 1673.38× |
| upcase | 2.61× | 2.67× | 5.22× | 1673.41× |
| upcase-stdin | 1.80× | 1.80× | 12.93× | 1673.35× |
| shout | 3.09× | 2.84× | 5.32× | 1673.36× |
| wc | 1.52× | 1.48× | 10.41× | 1673.40× |
| histogram | 1.25× | 1.23× | 10.76× | 1673.38× |
| crc32c | 3.07× | 3.02× | 10.83× | 1673.38× |
| sort | 1.04× | 1.04× | 1.13× | 1673.54× |
| tally | 0.53× | 0.53× | 8.36× | 1673.58× |
| server | 1.10× | 1.88× | 15.71× | 1673.40× |
| files | 3.53× | 3.48× | 11.23× | 1673.39× |
| readings | 3.69× | 3.66× | 10.78× | 1673.36× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 160 | behind, 1.11×, p 1.8e-34 | behind, 1.03×, p 2.4e-9 | behind, 1.07× | ahead, 0.89× |
| empty | cpp-wasm | 160 | behind, 1.10×, p 4.2e-29 | behind, 1.03×, p 9.6e-6 | behind, 1.07× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.86×, p 3.0e-7 | ahead, 0.82×, p 1.7e-4 | behind, 1.07× | ahead, 0.01× |
| empty | c | 20 | behind, 5.08×, p 3.4e-8 | behind, 3.81×, p 3.4e-8 | behind, 8.76× | behind, 1689.17× |
| empty | cpp | 20 | behind, 5.05×, p 3.4e-8 | behind, 3.79×, p 3.4e-8 | behind, 8.76× | behind, 1689.17× |
| empty | rust | 20 | behind, 3.63×, p 3.4e-8 | behind, 2.83×, p 3.4e-8 | behind, 6.26× | behind, 85.16× |
| add | c-wasm | 40 | ahead, 0.91×, p 1.9e-7 | ahead, 0.84×, p 5.3e-8 | behind, 1.05× | ahead, 0.04× |
| add | cpp-wasm | 320 | ahead, 0.99×, p 4.7e-5 | ahead, 0.89×, p 1.5e-39 | behind, 1.05× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.83×, p 3.9e-8 | ahead, 0.78×, p 6.3e-6 | behind, 1.04× | ahead, 0.02× |
| add | c | 20 | behind, 4.93×, p 3.4e-8 | behind, 3.71×, p 3.4e-8 | behind, 6.70× | behind, 1673.36× |
| add | cpp | 20 | behind, 4.98×, p 3.4e-8 | behind, 3.76×, p 3.4e-8 | behind, 6.95× | behind, 1673.36× |
| add | rust | 20 | behind, 3.84×, p 3.4e-8 | behind, 2.96×, p 8.3e-8 | behind, 6.24× | behind, 82.61× |
| hypotenuse | c-wasm | 20 | ahead, 0.94×, p 6.1e-4 | ahead, 0.85×, p 6.1e-4 | behind, 1.04× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 80 | ahead, 0.95×, p 1.0e-5 | ahead, 0.91×, p 5.6e-6 | behind, 1.05× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.85×, p 3.4e-8 | ahead, 0.81×, p 2.1e-4 | behind, 1.03× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 4.38×, p 3.4e-8 | behind, 3.41×, p 3.4e-8 | behind, 5.81× | behind, 1673.38× |
| hypotenuse | cpp | 20 | behind, 4.42×, p 3.4e-8 | behind, 3.42×, p 3.4e-8 | behind, 5.76× | behind, 1673.38× |
| hypotenuse | rust | 20 | behind, 3.79×, p 3.4e-8 | behind, 2.99×, p 3.4e-8 | behind, 6.15× | behind, 81.91× |
| upcase | c-wasm | 160 | ahead, 0.98×, p 0.001 | ahead, 0.97×, p 2.5e-4 | behind, 1.05× | ahead, 0.06× |
| upcase | cpp-wasm | 160 | ahead, 0.98×, p 3.0e-4 | ahead, 0.97×, p 7.7e-4 | behind, 1.06× | ahead, 0.06× |
| upcase | rust-wasm | 80 | ahead, 0.89×, p 4.2e-19 | ahead, 0.87×, p 1.7e-6 | behind, 1.02× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.61×, p 3.4e-8 | behind, 2.63×, p 3.4e-8 | behind, 5.22× | behind, 1673.41× |
| upcase | cpp | 20 | behind, 2.61×, p 3.4e-8 | behind, 2.67×, p 3.4e-8 | behind, 5.08× | behind, 1673.41× |
| upcase | rust | 20 | behind, 2.03×, p 3.4e-8 | behind, 2.11×, p 3.4e-8 | behind, 4.41× | behind, 82.78× |
| upcase-stdin | c-wasm | 20 | ahead, 0.35×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | ahead, 0.99× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.37×, p 3.4e-8 | ahead, 0.37×, p 3.4e-8 | ahead, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 20 | ahead, 0.88×, p 3.8e-6 | ahead, 0.92×, p 1.3e-5 | behind, 1.01× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.80×, p 3.4e-8 | behind, 1.80×, p 3.4e-8 | behind, 12.38× | behind, 1673.35× |
| upcase-stdin | cpp | 20 | behind, 1.79×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 12.93× | behind, 1673.35× |
| upcase-stdin | rust | 20 | behind, 1.73×, p 3.4e-8 | behind, 1.72×, p 3.4e-8 | behind, 9.07× | behind, 84.27× |
| shout | c-wasm | 640 | behind, 1.07×, p 1.5e-89 | behind, 1.02×, p 4.2e-6 | behind, 1.09× | ahead, 0.40× |
| shout | cpp-wasm | 1280 | behind, 1.05×, p 1.6e-107 | behind, 1.02×, p 7.2e-7 | behind, 1.09× | ahead, 0.40× |
| shout | rust-wasm | 40 | ahead, 0.93×, p 5.3e-9 | ahead, 0.87×, p 4.3e-4 | behind, 1.07× | ahead, 0.03× |
| shout | c | 20 | behind, 3.09×, p 3.4e-8 | behind, 2.84×, p 3.4e-8 | behind, 5.18× | behind, 1673.36× |
| shout | cpp | 20 | behind, 3.04×, p 3.4e-8 | behind, 2.81×, p 3.4e-8 | behind, 5.32× | behind, 1673.36× |
| shout | rust | 20 | behind, 2.20×, p 3.4e-8 | behind, 2.07×, p 3.4e-8 | behind, 4.52× | behind, 83.26× |
| wc | c-wasm | 20 | ahead, 0.86×, p 7.9e-7 | ahead, 0.84×, p 2.6e-7 | behind, 1.02× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.11×, p 3.4e-8 | ahead, 0.11×, p 3.4e-8 | ahead, 0.99× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.83×, p 3.9e-8 | ahead, 0.83×, p 3.4e-8 | behind, 1.02× | ahead, 0.04× |
| wc | c | 20 | behind, 1.52×, p 3.4e-8 | behind, 1.48×, p 3.4e-8 | behind, 10.02× | behind, 1673.40× |
| wc | cpp | 20 | ahead, 0.15×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | behind, 10.41× | behind, 1673.40× |
| wc | rust | 20 | behind, 1.31×, p 3.4e-8 | behind, 1.28×, p 3.4e-8 | behind, 9.08× | behind, 81.13× |
| histogram | c-wasm | 20 | ahead, 0.68×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | behind, 1.02× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | behind, 1.01× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | behind, 1.00× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.23×, p 3.4e-8 | behind, 1.21×, p 3.4e-8 | behind, 10.76× | behind, 1673.38× |
| histogram | cpp | 20 | behind, 1.22×, p 3.4e-8 | behind, 1.21×, p 3.4e-8 | behind, 10.76× | behind, 1673.38× |
| histogram | rust | 20 | behind, 1.25×, p 3.4e-8 | behind, 1.23×, p 3.4e-8 | behind, 9.31× | behind, 81.29× |
| crc32c | c-wasm | 20 | ahead, 0.81×, p 6.0e-7 | ahead, 0.80×, p 5.2e-7 | behind, 1.02× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.81×, p 6.0e-7 | ahead, 0.80×, p 5.2e-7 | behind, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.80×, p 4.0e-7 | ahead, 0.80×, p 3.5e-7 | behind, 1.02× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.07×, p 3.4e-8 | behind, 3.02×, p 3.4e-8 | behind, 10.83× | behind, 1673.38× |
| crc32c | cpp | 20 | behind, 3.07×, p 3.4e-8 | behind, 3.01×, p 3.4e-8 | behind, 10.83× | behind, 1673.38× |
| crc32c | rust | 20 | behind, 3.02×, p 3.4e-8 | behind, 2.96×, p 3.4e-8 | behind, 9.43× | behind, 83.39× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | behind, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.62×, p 3.4e-8 | ahead, 0.62×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.61×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | ahead, 1.00× | behind, 1673.54× |
| sort | cpp | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | behind, 1.04× | behind, 1297.66× |
| sort | rust | 20 | behind, 1.04×, p 5.5e-6 | behind, 1.04×, p 9.0e-6 | behind, 1.13× | behind, 81.31× |
| tally | c-wasm | 20 | ahead, 0.41×, p 3.4e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.04× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.41×, p 3.4e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.03× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.33×, p 3.4e-8 | ahead, 0.33×, p 3.4e-8 | behind, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.50×, p 3.4e-8 | ahead, 0.49×, p 3.4e-8 | behind, 8.14× | behind, 1673.58× |
| tally | cpp | 20 | ahead, 0.53×, p 3.4e-8 | ahead, 0.53×, p 3.4e-8 | behind, 4.01× | behind, 1304.40× |
| tally | rust | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.47×, p 3.4e-8 | behind, 8.36× | behind, 79.08× |
| server | c-wasm | 130 | undecided, 1.00×, p 0.002 | ahead, 0.95×, p 4.2e-34 | behind, 1.03× | ahead, 0.12× |
| server | cpp-wasm | 130 | ahead, 1.00×, p 5.3e-4 | ahead, 0.95×, p 3.9e-33 | behind, 1.02× | ahead, 0.06× |
| server | rust-wasm | 130 | undecided, 1.00×, p 0.007 | ahead, 0.95×, p 9.1e-36 | behind, 1.02× | ahead, 0.03× |
| server | c | 20 | behind, 1.09×, p 3.4e-8 | behind, 1.09×, p 3.4e-8 | behind, 15.71× | behind, 1673.40× |
| server | cpp | 20 | behind, 1.10×, p 3.4e-8 | behind, 1.09×, p 3.4e-8 | behind, 5.46× | behind, 1662.37× |
| server | rust | 20 | ahead, 0.96×, p 3.4e-8 | behind, 1.88×, p 3.4e-8 | behind, 7.18× | behind, 75.76× |
| files | c-wasm | 20 | ahead, 0.91×, p 7.9e-7 | ahead, 0.91×, p 6.9e-7 | behind, 1.03× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.93×, p 9.0e-7 | ahead, 0.92×, p 9.0e-7 | behind, 1.02× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.89×, p 2.1e-7 | ahead, 0.89×, p 2.0e-7 | behind, 1.02× | ahead, 0.03× |
| files | c | 20 | behind, 2.78×, p 3.4e-8 | behind, 2.74×, p 3.4e-8 | behind, 11.23× | behind, 1673.39× |
| files | cpp | 20 | behind, 2.77×, p 3.4e-8 | behind, 2.74×, p 3.4e-8 | behind, 10.81× | behind, 1673.39× |
| files | rust | 20 | behind, 3.53×, p 3.4e-8 | behind, 3.48×, p 3.4e-8 | behind, 9.23× | behind, 81.57× |
| readings | c-wasm | 20 | ahead, 0.93×, p 6.0e-7 | ahead, 0.93×, p 6.0e-7 | behind, 1.01× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.92×, p 6.0e-7 | ahead, 0.92×, p 6.0e-7 | behind, 1.01× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.92×, p 5.2e-7 | ahead, 0.92×, p 5.2e-7 | behind, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 3.69×, p 3.4e-8 | behind, 3.66×, p 3.4e-8 | behind, 10.78× | behind, 1673.36× |
| readings | cpp | 20 | behind, 3.38×, p 3.4e-8 | behind, 3.35×, p 3.4e-8 | behind, 4.59× | behind, 1673.36× |
| readings | rust | 20 | behind, 3.63×, p 3.4e-8 | behind, 3.60×, p 3.4e-8 | behind, 9.86× | behind, 81.25× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 160 | 20 | 20 | 20 | 20 |
| CPU time | 2.65 ms | 2.40 ms | 2.42 ms | 3.14 ms | 532 µs | 536 µs | 746 µs |
| wall time | 2.36 ms | 2.28 ms | 2.29 ms | 2.90 ms | 625 µs | 629 µs | 842 µs |
| peak memory | 10.7 MiB | 9.96 MiB | 9.96 MiB | 10.0 MiB | 1.22 MiB | 1.22 MiB | 1.71 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 14.0 KiB | 14.0 KiB | 278 KiB |

Its rounds took 3 seconds.

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
| counted runs | 320 | 40 | 320 | 20 | 20 | 20 | 20 |
| CPU time | 2.90 ms | 3.13 ms | 2.95 ms | 3.42 ms | 577 µs | 572 µs | 741 µs |
| wall time | 2.53 ms | 2.94 ms | 2.85 ms | 3.14 ms | 660 µs | 651 µs | 826 µs |
| peak memory | 10.7 MiB | 10.2 MiB | 10.2 MiB | 10.3 MiB | 1.60 MiB | 1.54 MiB | 1.71 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 5 seconds.

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
| counted runs | 80 | 20 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 2.96 ms | 3.02 ms | 3.12 ms | 3.35 ms | 648 µs | 643 µs | 748 µs |
| wall time | 2.61 ms | 2.94 ms | 2.88 ms | 3.10 ms | 737 µs | 734 µs | 840 µs |
| peak memory | 10.7 MiB | 10.2 MiB | 10.2 MiB | 10.3 MiB | 1.82 MiB | 1.84 MiB | 1.72 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 14.1 KiB | 14.1 KiB | 289 KiB |

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
| counted runs | 160 | 160 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 4.48 ms | 4.55 ms | 4.58 ms | 5.00 ms | 1.68 ms | 1.68 ms | 2.15 ms |
| wall time | 4.30 ms | 4.42 ms | 4.43 ms | 4.88 ms | 1.85 ms | 1.83 ms | 2.32 ms |
| peak memory | 11.8 MiB | 11.2 MiB | 11.2 MiB | 11.5 MiB | 2.29 MiB | 2.35 MiB | 2.71 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

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
| CPU time | 12.6 ms | 36.0 ms | 34.5 ms | 14.3 ms | 7.01 ms | 7.04 ms | 7.32 ms |
| wall time | 12.8 ms | 35.8 ms | 34.3 ms | 13.9 ms | 7.10 ms | 7.14 ms | 7.45 ms |
| peak memory | 16.7 MiB | 16.8 MiB | 16.7 MiB | 16.4 MiB | 1.35 MiB | 1.29 MiB | 1.84 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 281 KiB |

Its rounds took 4 seconds.

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
| counted runs | 1280 | 640 | 1280 | 40 | 20 | 20 | 20 |
| CPU time | 3.62 ms | 3.48 ms | 3.44 ms | 4.06 ms | 1.22 ms | 1.25 ms | 1.72 ms |
| wall time | 3.32 ms | 3.27 ms | 3.25 ms | 3.88 ms | 1.32 ms | 1.33 ms | 1.81 ms |
| peak memory | 11.5 MiB | 10.5 MiB | 10.5 MiB | 10.8 MiB | 2.22 MiB | 2.16 MiB | 2.55 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

Its rounds took 23 seconds.

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
| CPU time | 21.8 ms | 25.5 ms | 200 ms | 26.3 ms | 14.4 ms | 148 ms | 16.6 ms |
| wall time | 21.4 ms | 25.6 ms | 200 ms | 26.0 ms | 14.5 ms | 148 ms | 16.8 ms |
| peak memory | 16.7 MiB | 16.3 MiB | 16.9 MiB | 16.4 MiB | 1.66 MiB | 1.60 MiB | 1.84 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 11 seconds.

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
| CPU time | 35.2 ms | 51.4 ms | 49.0 ms | 44.5 ms | 28.6 ms | 28.8 ms | 28.1 ms |
| wall time | 34.8 ms | 51.2 ms | 48.9 ms | 44.3 ms | 28.7 ms | 28.9 ms | 28.2 ms |
| peak memory | 17.2 MiB | 17.0 MiB | 17.0 MiB | 17.2 MiB | 1.60 MiB | 1.60 MiB | 1.85 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

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
| CPU time | 40.5 ms | 50.1 ms | 50.1 ms | 50.6 ms | 13.2 ms | 13.2 ms | 13.4 ms |
| wall time | 40.2 ms | 49.9 ms | 50.0 ms | 50.4 ms | 13.3 ms | 13.3 ms | 13.5 ms |
| peak memory | 17.3 MiB | 17.0 MiB | 16.9 MiB | 17.0 MiB | 1.60 MiB | 1.60 MiB | 1.84 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

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
| CPU time | 273 ms | 1.36 s | 482 ms | 444 ms | 446 ms | 376 ms | 264 ms |
| wall time | 273 ms | 1.36 s | 482 ms | 444 ms | 447 ms | 376 ms | 264 ms |
| peak memory | 86.7 MiB | 86.2 MiB | 94.4 MiB | 94.4 MiB | 87.0 MiB | 83.2 MiB | 76.7 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 291 KiB |

Its rounds took 86 seconds.

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
| CPU time | 141 ms | 347 ms | 344 ms | 427 ms | 284 ms | 267 ms | 296 ms |
| wall time | 141 ms | 347 ms | 344 ms | 427 ms | 285 ms | 267 ms | 296 ms |
| peak memory | 18.1 MiB | 17.5 MiB | 17.5 MiB | 17.4 MiB | 2.23 MiB | 4.52 MiB | 2.17 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 299 KiB |

Its rounds took 49 seconds.

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
| counted runs | 130 | 130 | 130 | 130 | 20 | 20 | 20 |
| CPU time | 930 ms | 932 ms | 934 ms | 932 ms | 849 ms | 847 ms | 964 ms |
| wall time | 930 ms | 983 ms | 979 ms | 981 ms | 853 ms | 850 ms | 494 ms |
| peak memory | 17.2 MiB | 16.7 MiB | 16.9 MiB | 17.0 MiB | 1.10 MiB | 3.16 MiB | 2.40 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 14.1 KiB | 14.2 KiB | 312 KiB |

Its rounds took 623 seconds.

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
| CPU time | 68.2 ms | 74.6 ms | 73.6 ms | 76.7 ms | 24.6 ms | 24.6 ms | 19.4 ms |
| wall time | 67.8 ms | 74.4 ms | 73.4 ms | 76.4 ms | 24.7 ms | 24.7 ms | 19.5 ms |
| peak memory | 18.0 MiB | 17.5 MiB | 17.6 MiB | 17.7 MiB | 1.60 MiB | 1.66 MiB | 1.95 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 9 seconds.

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
| CPU time | 106 ms | 114 ms | 115 ms | 116 ms | 28.8 ms | 31.4 ms | 29.3 ms |
| wall time | 106 ms | 114 ms | 115 ms | 115 ms | 28.9 ms | 31.6 ms | 29.4 ms |
| peak memory | 17.3 MiB | 17.1 MiB | 17.1 MiB | 17.2 MiB | 1.60 MiB | 3.76 MiB | 1.75 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

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
| empty | 160, 160 in 2 s | 2.57 ms against 2.38 ms: +198 µs (+177 µs to +218 µs) | 2.27 ms against 2.24 ms: +27.1 µs (+7.27 µs to +46.2 µs) | 10.7 MiB against 10.0 MiB: +632 KiB (+588 KiB to +672 KiB) |
| add | 160, 160 in 2 s | 2.81 ms against 2.64 ms: +177 µs (+140 µs to +211 µs) | 2.50 ms against 2.45 ms: +47.3 µs (+1.71 µs to +96.7 µs) | 10.7 MiB against 10.1 MiB: +576 KiB (+536 KiB to +616 KiB) |
| hypotenuse | 160, 160 in 2 s | 2.96 ms against 2.71 ms: +224 µs (+181 µs to +268 µs) | 2.55 ms against 2.52 ms: +35.3 µs (-12.8 µs to +82.6 µs) | 10.7 MiB against 10.1 MiB: +564 KiB (+528 KiB to +600 KiB) |
| upcase | 160, 160 in 3 s | 4.46 ms against 4.13 ms: +295 µs (+233 µs to +355 µs) | 4.22 ms against 3.97 ms: +203 µs (+89.6 µs to +338 µs) | 11.8 MiB against 11.0 MiB: +744 KiB (+656 KiB to +824 KiB) |
| upcase-stdin | 160, 160 in 5 s | 12.8 ms against 12.4 ms: +319 µs (+154 µs to +488 µs) | 12.5 ms against 12.3 ms: +197 µs (+19.7 µs to +396 µs) | 16.6 MiB against 16.2 MiB: +348 KiB (+288 KiB to +404 KiB) |
| shout | 160, 160 in 2 s | 3.41 ms against 3.10 ms: +312 µs (+288 µs to +335 µs) | 3.22 ms against 3.02 ms: +189 µs (+143 µs to +247 µs) | 11.5 MiB against 10.6 MiB: +976 KiB (+904 KiB to +1.03 MiB) |
| wc | 160, 160 in 8 s | 21.9 ms against 21.9 ms: -14.0 µs (-201 µs to +140 µs) | 21.6 ms against 21.8 ms: -291 µs (-535 µs to -87.2 µs) | 16.7 MiB against 16.3 MiB: +392 KiB (+364 KiB to +420 KiB) |
| histogram | 160, 160 in 13 s | 34.8 ms against 36.3 ms: -1.09 ms (-2.23 ms to -516 µs) | 34.5 ms against 36.1 ms: -1.27 ms (-2.40 ms to -703 µs) | 17.2 MiB against 16.9 MiB: +376 KiB (+340 KiB to +412 KiB) |
| crc32c | 160, 160 in 15 s | 41.0 ms against 42.0 ms: -753 µs (-1.55 ms to -197 µs) | 40.7 ms against 41.8 ms: -986 µs (-1.79 ms to -421 µs) | 17.3 MiB against 16.9 MiB: +356 KiB (+324 KiB to +384 KiB) |
| sort | 107, 107 in 60 s | 275 ms against 278 ms: -2.20 ms (-4.35 ms to -150 µs) | 275 ms against 278 ms: -2.43 ms (-4.57 ms to -361 µs) | 86.7 MiB against 86.2 MiB: +528 KiB (+488 KiB to +568 KiB) |
| tally | 160, 160 in 46 s | 139 ms against 139 ms: -713 µs (-1.53 ms to -88.0 µs) | 138 ms against 139 ms: -991 µs (-1.78 ms to -350 µs) | 18.2 MiB against 17.6 MiB: +612 KiB (+572 KiB to +648 KiB) |
| server | 31, 31 in 62 s | 941 ms against 937 ms: +1.19 ms (-4.08 ms to +6.58 ms) | 940 ms against 940 ms: +150 µs (-5.86 ms to +5.10 ms) | 17.2 MiB against 16.6 MiB: +564 KiB (+472 KiB to +644 KiB) |
| files | 160, 160 in 23 s | 67.4 ms against 69.2 ms: -1.62 ms (-2.34 ms to -1.01 ms) | 67.0 ms against 69.0 ms: -1.84 ms (-2.57 ms to -1.23 ms) | 18.1 MiB against 17.7 MiB: +320 KiB (+292 KiB to +352 KiB) |
| readings | 160, 160 in 35 s | 106 ms against 107 ms: -667 µs (-1.23 ms to -310 µs) | 105 ms against 107 ms: -852 µs (-1.42 ms to -499 µs) | 17.3 MiB against 16.9 MiB: +416 KiB (+384 KiB to +448 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
