# lengua against C, C++ and Rust on windows-aarch64

Measured on windows-aarch64 (Cobalt 100, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 33ed7fe1f45158efe1a2695d8accd88e1dfb18a5, in https://github.com/laughing-hermit/lengua/actions/runs/42947294489. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (18.8 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ✗ ✗ ✗ ✓ | ? ✗ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| hypotenuse | ? ✓ ✗ ✓ | ✗ ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| upcase | ? ✗ ✗ ✓ | ✓ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ? ✗ ✗ ✓ | ? ✗ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-aarch64 upcase cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 918 names the row: take it out, as the list only shrinks
- windows-aarch64 upcase cpp-wasm wall: lengua is behind, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 upcase rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 920 names the row: take it out, as the list only shrinks
- windows-aarch64 shout rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 histogram rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 992 names the row: take it out, as the list only shrinks
- windows-aarch64 server cpp-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 readings cpp-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.87× | 2.49× | 189.99× |
| add | – | 1.84× | 2.49× | 166.25× |
| hypotenuse | – | 1.86× | 2.49× | 164.83× |
| upcase | – | 1.79× | 2.15× | 161.38× |
| upcase-stdin | 2.00× | 1.58× | 2.48× | 181.08× |
| shout | – | 1.79× | 2.13× | 173.74× |
| wc | 1.50× | 1.24× | 2.48× | 168.43× |
| histogram | 1.33× | 1.28× | 2.45× | 165.54× |
| crc32c | 4.00× | 2.27× | 2.46× | 175.32× |
| sort | 1.11× | 1.09× | 1.01× | 166.27× |
| tally | 0.69× | 0.67× | 2.27× | 154.30× |
| server | 1.17× | 1.62× | 2.16× | 170.67× |
| files | 2.17× | 1.99× | 2.43× | 162.06× |
| readings | 4.00× | 3.20× | 2.49× | 161.38× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 640 | behind, 1.00×, p 1.0e-3 | behind, 1.01×, p 1.5e-6 | behind, 1.08× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | undecided, 1.00×, p 0.004 | behind, 1.01×, p 1.3e-14 | behind, 1.08× | ahead, 0.89× |
| empty | rust-wasm | 2000 | undecided, 1.00×, p 0.478 | ahead, 0.99×, p 2.3e-59 | behind, 1.08× | ahead, 0.01× |
| empty | c | 40 | behind, p 3.5e-5 | behind, 1.86×, p 7.2e-15 | behind, 2.49× | behind, 179.38× |
| empty | cpp | 20 | behind, p 6.3e-5 | behind, 1.87×, p 3.4e-8 | behind, 2.49× | behind, 179.38× |
| empty | rust | 20 | behind, p 6.5e-6 | behind, 1.78×, p 3.4e-8 | behind, 2.34× | behind, 189.99× |
| add | c-wasm | 2000 | undecided, 1.00×, p 0.033 | ahead, 0.99×, p 4.3e-19 | behind, 1.08× | ahead, 0.04× |
| add | cpp-wasm | 2000 | undecided, 1.00×, p 0.006 | ahead, 0.99×, p 7.3e-19 | behind, 1.08× | ahead, 0.04× |
| add | rust-wasm | 2000 | undecided, 1.00×, p 0.299 | ahead, 0.98×, p 2.1e-169 | behind, 1.08× | ahead, 0.02× |
| add | c | 20 | behind, p 3.7e-5 | behind, 1.84×, p 3.4e-8 | behind, 2.49× | behind, 135.33× |
| add | cpp | 0 | undecided | undecided | undecided | undecided |
| add | rust | 20 | behind, p 1.4e-5 | behind, 1.76×, p 3.4e-8 | behind, 2.34× | behind, 166.25× |
| hypotenuse | c-wasm | 2000 | undecided, 1.00×, p 0.002 | ahead, 0.99×, p 5.9e-17 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | behind, 1.00×, p 3.7e-4 | ahead, 0.99×, p 4.8e-11 | behind, 1.09× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | undecided, 1.00×, p 0.102 | ahead, 0.98×, p 6.3e-141 | behind, 1.09× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, p 3.4e-6 | behind, 1.86×, p 3.4e-8 | behind, 2.49× | behind, 134.39× |
| hypotenuse | cpp | 0 | undecided | undecided | undecided | undecided |
| hypotenuse | rust | 20 | behind, p 8.0e-6 | behind, 1.77×, p 6.0e-7 | behind, 2.34× | behind, 164.83× |
| upcase | c-wasm | 2000 | undecided, 1.00×, p 0.415 | behind, 1.00×, p 2.0e-10 | behind, 1.08× | ahead, 0.06× |
| upcase | cpp-wasm | 1280 | ahead, 1.00×, p 1.5e-4 | behind, 1.00×, p 1.9e-5 | behind, 1.08× | ahead, 0.06× |
| upcase | rust-wasm | 1280 | ahead, 1.00×, p 6.1e-5 | ahead, 0.99×, p 6.3e-38 | behind, 1.07× | ahead, 0.03× |
| upcase | c | 20 | behind, p 3.3e-6 | behind, 1.78×, p 3.4e-8 | behind, 2.09× | behind, 126.46× |
| upcase | cpp | 20 | behind, p 1.5e-5 | behind, 1.79×, p 3.4e-8 | behind, 2.09× | behind, 126.46× |
| upcase | rust | 20 | behind, p 1.8e-6 | behind, 1.71×, p 3.4e-8 | behind, 2.15× | behind, 161.38× |
| upcase-stdin | c-wasm | 1280 | behind, 1.00×, p 1.6e-5 | behind, 1.01×, p 1.6e-19 | behind, 1.09× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | behind, 1.00×, p 4.1e-7 | behind, 1.01×, p 4.7e-11 | behind, 1.09× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | undecided, 1.00×, p 0.015 | ahead, 0.99×, p 2.8e-18 | behind, 1.09× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.50×, p 5.6e-7 | behind, 1.56×, p 3.4e-8 | behind, 2.48× | behind, 172.96× |
| upcase-stdin | cpp | 20 | behind, 2.00×, p 2.9e-7 | behind, 1.58×, p 3.4e-8 | behind, 2.48× | behind, 172.96× |
| upcase-stdin | rust | 20 | behind, 1.50×, p 1.8e-6 | behind, 1.48×, p 3.4e-8 | behind, 2.35× | behind, 181.08× |
| shout | c-wasm | 2000 | undecided, 1.00×, p 0.076 | behind, 1.01×, p 7.1e-17 | behind, 1.09× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | undecided, 1.00×, p 0.002 | behind, 1.01×, p 1.2e-15 | behind, 1.09× | ahead, 0.40× |
| shout | rust-wasm | 2000 | undecided, 1.00×, p 0.405 | ahead, 0.99×, p 1.2e-52 | behind, 1.07× | ahead, 0.03× |
| shout | c | 20 | behind, p 5.9e-4 | behind, 1.79×, p 3.4e-8 | behind, 2.11× | behind, 168.43× |
| shout | cpp | 40 | behind, p 3.7e-6 | behind, 1.78×, p 7.2e-15 | behind, 2.10× | behind, 168.43× |
| shout | rust | 40 | behind, p 2.1e-6 | behind, 1.72×, p 7.2e-15 | behind, 2.13× | behind, 173.74× |
| wc | c-wasm | 160 | ahead, 0.75×, p 1.5e-6 | ahead, 0.87×, p 6.1e-52 | behind, 1.09× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.21×, p 1.1e-8 | ahead, 0.24×, p 3.4e-8 | behind, 1.09× | ahead, 0.11× |
| wc | rust-wasm | 80 | ahead, 0.75×, p 2.9e-4 | ahead, 0.86×, p 8.2e-27 | behind, 1.08× | ahead, 0.04× |
| wc | c | 20 | behind, 1.50×, p 4.1e-5 | behind, 1.24×, p 3.4e-8 | behind, 2.48× | behind, 133.93× |
| wc | cpp | 20 | behind, 1.50×, p 1.1e-5 | behind, 1.22×, p 3.4e-8 | behind, 2.48× | behind, 133.93× |
| wc | rust | 20 | behind, 1.50×, p 1.0e-4 | behind, 1.18×, p 3.4e-8 | behind, 2.36× | behind, 168.43× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 2.1e-7 | ahead, 0.69×, p 3.4e-8 | behind, 1.08× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 3.6e-7 | ahead, 0.69×, p 3.4e-8 | behind, 1.08× | ahead, 0.09× |
| histogram | rust-wasm | 2000 | ahead, 1.00×, p 8.3e-7 | ahead, 0.96×, p 0.0e0 | behind, 1.07× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 2.8e-5 | behind, 1.27×, p 3.4e-8 | behind, 2.45× | behind, 134.39× |
| histogram | cpp | 20 | behind, 1.33×, p 1.5e-4 | behind, 1.28×, p 3.4e-8 | behind, 2.45× | behind, 134.39× |
| histogram | rust | 20 | behind, 1.33×, p 1.3e-5 | behind, 1.23×, p 3.4e-8 | behind, 2.33× | behind, 165.54× |
| crc32c | c-wasm | 40 | ahead, 0.80×, p 3.3e-4 | ahead, 0.88×, p 7.2e-15 | behind, 1.08× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.80×, p 7.2e-4 | ahead, 0.87×, p 3.4e-8 | behind, 1.08× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.80×, p 1.1e-4 | ahead, 0.83×, p 3.4e-8 | behind, 1.08× | ahead, 0.04× |
| crc32c | c | 20 | behind, 4.00×, p 1.7e-8 | behind, 2.27×, p 3.4e-8 | behind, 2.46× | behind, 134.86× |
| crc32c | cpp | 20 | behind, 2.00×, p 1.7e-8 | behind, 2.23×, p 3.4e-8 | behind, 2.46× | behind, 134.86× |
| crc32c | rust | 20 | behind, 2.00×, p 1.5e-8 | behind, 2.15×, p 3.4e-8 | behind, 2.34× | behind, 175.32× |
| sort | c-wasm | 20 | ahead, 0.19×, p 2.3e-8 | ahead, 0.19×, p 3.4e-8 | behind, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.57×, p 2.0e-8 | ahead, 0.56×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.65×, p 1.8e-8 | ahead, 0.63×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.43×, p 2.0e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.01× | behind, 166.27× |
| sort | cpp | 0 | undecided | undecided | undecided | undecided |
| sort | rust | 20 | behind, 1.11×, p 4.2e-8 | behind, 1.09×, p 3.4e-8 | ahead, 0.95× | behind, 163.45× |
| tally | c-wasm | 20 | ahead, 0.57×, p 1.2e-8 | ahead, 0.55×, p 3.4e-8 | behind, 1.10× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.57×, p 1.6e-8 | ahead, 0.55×, p 3.4e-8 | behind, 1.11× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.41×, p 1.5e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.10× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.69×, p 1.5e-8 | ahead, 0.67×, p 3.4e-8 | behind, 2.22× | behind, 131.21× |
| tally | cpp | 0 | undecided | undecided | undecided | undecided |
| tally | rust | 20 | ahead, 0.67×, p 1.3e-8 | ahead, 0.66×, p 3.4e-8 | behind, 2.27× | behind, 154.30× |
| server | c-wasm | 67 | undecided, 0.99×, p 0.075 | ahead, 0.93×, p 2.2e-22 | behind, 1.09× | ahead, 0.12× |
| server | cpp-wasm | 67 | undecided, 0.99×, p 0.437 | ahead, 0.93×, p 2.0e-22 | behind, 1.09× | ahead, 0.06× |
| server | rust-wasm | 67 | undecided, 0.99×, p 0.009 | ahead, 0.91×, p 1.7e-22 | behind, 1.07× | ahead, 0.03× |
| server | c | 20 | behind, 1.17×, p 3.0e-8 | behind, 1.16×, p 3.4e-8 | behind, 2.16× | behind, 170.67× |
| server | cpp | 20 | behind, 1.17×, p 3.0e-8 | behind, 1.16×, p 3.4e-8 | behind, 2.15× | behind, 166.97× |
| server | rust | 20 | ahead, 0.91×, p 1.3e-4 | behind, 1.62×, p 3.4e-8 | behind, 1.77× | behind, 141.28× |
| files | c-wasm | 20 | ahead, 0.93×, p 2.2e-4 | ahead, 0.92×, p 1.2e-5 | behind, 1.08× | ahead, 0.05× |
| files | cpp-wasm | 40 | ahead, 0.93×, p 2.4e-5 | ahead, 0.94×, p 7.0e-9 | behind, 1.08× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.93×, p 2.5e-6 | ahead, 0.94×, p 4.6e-5 | behind, 1.08× | ahead, 0.03× |
| files | c | 20 | behind, 1.86×, p 1.7e-8 | behind, 1.84×, p 3.4e-8 | behind, 2.43× | behind, 127.30× |
| files | cpp | 20 | behind, 1.86×, p 1.4e-8 | behind, 1.84×, p 3.4e-8 | behind, 2.43× | behind, 127.30× |
| files | rust | 20 | behind, 2.17×, p 1.9e-8 | behind, 1.99×, p 3.4e-8 | behind, 2.31× | behind, 162.06× |
| readings | c-wasm | 640 | ahead, 1.00×, p 3.9e-8 | ahead, 0.97×, p 3.4e-185 | behind, 1.08× | ahead, 0.04× |
| readings | cpp-wasm | 1702 | undecided, 1.00×, p 0.002 | ahead, 0.99×, p 0.0e0 | behind, 1.08× | ahead, 0.04× |
| readings | rust-wasm | 80 | ahead, 1.00×, p 1.6e-4 | ahead, 0.96×, p 1.6e-23 | behind, 1.08× | ahead, 0.02× |
| readings | c | 20 | behind, 4.00×, p 4.9e-9 | behind, 3.20×, p 3.4e-8 | behind, 2.49× | behind, 135.33× |
| readings | cpp | 0 | undecided | undecided | undecided | undecided |
| readings | rust | 20 | behind, 2.67×, p 1.2e-8 | behind, 2.44×, p 3.4e-8 | behind, 2.34× | behind, 161.38× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 640 | 2000 | 2000 | 40 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 22.3 ms | 22.2 ms | 22.2 ms | 22.6 ms | 12.0 ms | 11.9 ms | 12.5 ms |
| peak memory | 11.6 MiB | 10.7 MiB | 10.7 MiB | 10.7 MiB | 4.66 MiB | 4.66 MiB | 4.96 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 108 KiB | 108 KiB | 102 KiB |

Its rounds took 266 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 0 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 21.8 ms | 21.9 ms | 21.9 ms | 22.2 ms | 12.0 ms | – | 12.4 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.70 MiB | – | 4.98 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 142 KiB | 0 B | 116 KiB |

Its rounds took 310 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build C:\a\lengua\lengua\benches\programs\add\main.cpp: C:\a\lengua\lengua\benches\programs\add\main.cpp:10:6: error: no template named 'optional' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      | ~~~~~^
C:\a\lengua\lengua\benches\programs\add\main.cpp:10:37: error: no type named 'string_view' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      |                                ~~~~~^
C:\a\lengua\lengua\benches\programs\add\main.cpp:12:10: warning: structured binding declarations are a C++17 extension [-Wc++17-extensions]
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |          ^~~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\add\main.cpp:12:30: error: no member named 'from_chars' in namespace 'std'
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |                              ^~~~~~~~~~
C:\a\lengua\lengua\benches\programs\add\main.cpp:13:23: error: no member named 'errc' in namespace 'std'
   13 |     if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
      |                       ^~~~
C:\a\lengua\lengua\benches\programs\add\main.cpp:15:21: error: no member named 'nullopt' in namespace 'std'
   15 |         return std::nullopt;
      |                     ^~~~~~~
1 warning and 5 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 0 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 22.3 ms | 22.4 ms | 22.4 ms | 22.8 ms | 12.0 ms | – | 12.5 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.7 MiB | 10.8 MiB | 4.69 MiB | – | 4.99 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 144 KiB | 0 B | 117 KiB |

Its rounds took 318 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build C:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp: C:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp:13:16: error: no type named 'string_view' in namespace 'std'
   13 | int parse(std::string_view text, long long& units) {
      |           ~~~~~^
1 error generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 1280 | 1280 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 23.9 ms | 23.8 ms | 23.9 ms | 24.3 ms | 13.5 ms | 13.4 ms | 14.0 ms |
| peak memory | 12.2 MiB | 11.2 MiB | 11.2 MiB | 11.4 MiB | 5.80 MiB | 5.80 MiB | 5.66 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 152 KiB | 152 KiB | 120 KiB |

Its rounds took 273 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 1280 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 46.9 ms | 46.9 ms | 46.9 ms | 31.2 ms | 23.4 ms | 31.2 ms |
| wall time | 48.8 ms | 48.3 ms | 48.5 ms | 49.1 ms | 31.0 ms | 30.8 ms | 32.8 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.73 MiB | 4.73 MiB | 4.98 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 112 KiB | 112 KiB | 106 KiB |

Its rounds took 486 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 22.7 ms | 22.6 ms | 22.6 ms | 23.0 ms | 12.6 ms | 12.6 ms | 13.1 ms |
| peak memory | 11.9 MiB | 11.0 MiB | 11.0 MiB | 11.1 MiB | 5.67 MiB | 5.67 MiB | 5.60 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 114 KiB | 114 KiB | 111 KiB |

Its rounds took 324 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 20 | 80 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 219 ms | 62.5 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 52.8 ms | 60.4 ms | 221 ms | 61.2 ms | 42.5 ms | 43.3 ms | 44.5 ms |
| peak memory | 11.8 MiB | 10.8 MiB | 10.8 MiB | 10.9 MiB | 4.76 MiB | 4.76 MiB | 4.99 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 144 KiB | 144 KiB | 114 KiB |

Its rounds took 41 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 20 | 20 | 2000 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 93.8 ms | 93.8 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 72.9 ms | 104 ms | 103 ms | 75.7 ms | 56.4 ms | 56.1 ms | 58.4 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.9 MiB | 4.77 MiB | 4.77 MiB | 5.01 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 144 KiB | 144 KiB | 116 KiB |

Its rounds took 383 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 78.1 ms | 78.1 ms | 78.1 ms | 15.6 ms | 31.2 ms | 31.2 ms |
| wall time | 70.3 ms | 80.3 ms | 81.1 ms | 84.6 ms | 31.1 ms | 31.6 ms | 32.9 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.75 MiB | 4.76 MiB | 4.99 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 143 KiB | 143 KiB | 110 KiB |

Its rounds took 16 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 0 | 20 |
| CPU time | 312 ms | 1.62 s | 547 ms | 484 ms | 734 ms | – | 281 ms |
| wall time | 311 ms | 1.63 s | 553 ms | 491 ms | 752 ms | – | 286 ms |
| peak memory | 80.6 MiB | 79.6 MiB | 87.6 MiB | 87.7 MiB | 79.7 MiB | – | 84.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 116 KiB | 0 B | 118 KiB |

Its rounds took 97 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build C:\a\lengua\lengua\benches\programs\sort\main.cpp: C:\a\lengua\lengua\benches\programs\sort\main.cpp:39:22: error: no member named 'string_view' in namespace 'std'
   39 |     std::vector<std::string_view> lines;
      |                      ^~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:40:10: error: no member named 'string_view' in namespace 'std'
   40 |     std::string_view rest(input, length);
      |          ^~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:41:13: error: use of undeclared identifier 'rest'
   41 |     while (!rest.empty()) {
      |             ^~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:42:31: error: use of undeclared identifier 'rest'
   42 |         std::size_t newline = rest.find('\n');
      |                               ^~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:43:25: error: use of undeclared identifier 'rest'
   43 |         lines.push_back(rest.substr(0, newline));
      |                         ^~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:44:9: error: use of undeclared identifier 'rest'
   44 |         rest.remove_prefix(newline + 1);
      |         ^~~~
C:\a\lengua\lengua\benches\programs\sort\main.cpp:52:15: error: no type named 'string_view' in namespace 'std'
   52 |     for (std::string_view line : lines) {
      |          ~~~~~^
7 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 0 | 20 |
| CPU time | 188 ms | 328 ms | 328 ms | 453 ms | 273 ms | – | 281 ms |
| wall time | 188 ms | 342 ms | 342 ms | 453 ms | 281 ms | – | 287 ms |
| peak memory | 12.0 MiB | 10.9 MiB | 10.8 MiB | 10.9 MiB | 5.40 MiB | – | 5.28 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 147 KiB | 0 B | 125 KiB |

Its rounds took 46 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build C:\a\lengua\lengua\benches\programs\tally\main.cpp: C:\a\lengua\lengua\benches\programs\tally\main.cpp:42:6: error: no type named 'string_view' in namespace 'std'
   42 | std::string_view word_of(const Entry& entry) {
      | ~~~~~^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:43:12: error: excess elements in scalar initializer
   43 |     return {store.data() + entry.offset, entry.length};
      |            ^                           ~~~~~~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\tally\main.cpp:47:15: error: no type named 'string_view' in namespace 'std'
   47 | bool add(std::string_view word) {
      |          ~~~~~^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:107:27: error: no member named 'string_view' in namespace 'std'
  107 |             if (!add(std::string_view(buffer.data() + start, at - start))) {
      |                           ^~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\tally\main.cpp:125:14: error: no type named 'string_view' in namespace 'std'
  125 |         std::string_view word = word_of(entry);
      |         ~~~~~^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8923:17: error: static_cast from 'void' to 'bool' is not allowed
 8923 |             if (_DEBUG_LT_PRED(_Pred, _Val, *_First)) { // found new earliest element, move to front
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9055:18: note: in instantiation of function template specialization 'std::_Insertion_sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9055 |             _STD _Insertion_sort_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:71:13: error: static_cast from 'void' to 'bool' is not allowed
   71 |         if (_DEBUG_LT_PRED(_Pred, *(_First + _Idx), *(_First + (_Idx - 1)))) {
      |             ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:27:25: error: static_cast from 'void' to 'bool' is not allowed
   27 |         _Top < _Hole && _DEBUG_LT_PRED(_Pred, *(_First + _Idx), _Val); _Idx = (_Hole - 1) >> 1) { // shift for codegen
      |                         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:83:10: note: in instantiation of function template specialization 'std::_Push_heap_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
   83 |     _STD _Push_heap_by_index(_First, _Hole, _Top, _STD forward<_Ty>(_Val), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:93:5: error: no matching function for call to '_Pop_heap_hole_by_index'
   93 |     _STD _Pop_heap_hole_by_index(
      |     ^~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\yvals_core.h:1906:17: note: expanded from macro '_STD'
 1906 | #define _STD    ::std::
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:103:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_unchecked<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  103 |         _STD _Pop_heap_hole_unchecked(_First, _Last, _Last, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:7816:14: note: in instantiation of function template specialization 'std::_Pop_heap_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 7816 |         _STD _Pop_heap_unchecked(_First, _Last, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9061:18: note: in instantiation of function template specialization 'std::_Sort_heap_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9061 |             _STD _Sort_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:57:19: note: candidate template ignored: substitution failure [with _RanIt = (anonymous namespace)::Entry *, _Ty = (anonymous namespace)::Entry, _Pr = (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)]
   57 | _CONSTEXPR20 void _Pop_heap_hole_by_index(
      |                   ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8942:9: error: static_cast from 'void' to 'bool' is not allowed
 8942 |     if (_DEBUG_LT_PRED(_Pred, *_Mid, *_First)) {
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8963:14: note: in instantiation of function template specialization 'std::_Med3_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 8963 |         _STD _Med3_unchecked(_First, _First + _Step, _First + _Two_step, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9045:10: note: in instantiation of function template specialization 'std::_Guess_median_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9045 |     _STD _Guess_median_unchecked(_First, _Mid, _STD _Prev_iter(_Last), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8946:9: error: static_cast from 'void' to 'bool' is not allowed
 8946 |     if (_DEBUG_LT_PRED(_Pred, *_Last, *_Mid)) { // swap middle and last, then test first again
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8978:33: error: static_cast from 'void' to 'bool' is not allowed
 8978 |     while (_First < _Pfirst && !_DEBUG_LT_PRED(_Pred, *_STD _Prev_iter(_Pfirst), *_Pfirst)
      |                                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9047:17: note: in instantiation of function template specialization 'std::_Partition_by_pivot_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9047 |     return _STD _Partition_by_pivot_unchecked(_First, _Mid, _Last, _Pred);
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at C:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8983:31: error: static_cast from 'void' to 'bool' is not allowed
 8983 |     while (_Plast < _Last && !_DEBUG_LT_PRED(_Pred, *_Plast, *_Pfirst) && !_Pred(*_Pfirst, *_Plast)) {
      |                               ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8992:17: error: static_cast from 'void' to 'bool' is not allowed
 8992 |             if (_DEBUG_LT_PRED(_Pred, *_Pfirst, *_Gfirst)) {
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from C:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9006:17: error: static_cast from 'void' to 'bool' is not allowed
 9006 |             if (_DEBUG_LT_PRED(_Pred, *_Glast_prev, *_Pfirst)) {
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
15 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 67 | 67 | 67 | 67 | 20 | 20 | 20 |
| CPU time | 1.64 s | 1.66 s | 1.66 s | 1.66 s | 1.41 s | 1.41 s | 1.81 s |
| wall time | 1.66 s | 1.79 s | 1.80 s | 1.82 s | 1.43 s | 1.43 s | 1.03 s |
| peak memory | 11.5 MiB | 10.5 MiB | 10.6 MiB | 10.7 MiB | 5.31 MiB | 5.33 MiB | 6.45 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 113 KiB | 116 KiB | 136 KiB |

Its rounds took 636 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 203 ms | 219 ms | 219 ms | 219 ms | 109 ms | 109 ms | 93.8 ms |
| wall time | 212 ms | 230 ms | 226 ms | 226 ms | 115 ms | 115 ms | 107 ms |
| peak memory | 11.9 MiB | 11.1 MiB | 11.1 MiB | 11.1 MiB | 4.91 MiB | 4.91 MiB | 5.17 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 152 KiB | 152 KiB | 119 KiB |

Its rounds took 41 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1702 | 640 | 1702 | 80 | 20 | 0 | 20 |
| CPU time | 125 ms | 125 ms | 125 ms | 125 ms | 31.2 ms | – | 46.9 ms |
| wall time | 125 ms | 129 ms | 127 ms | 131 ms | 39.1 ms | – | 51.3 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.71 MiB | – | 5.00 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 142 KiB | 0 B | 120 KiB |

Its rounds took 602 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build C:\a\lengua\lengua\benches\programs\readings\main.cpp: C:\a\lengua\lengua\benches\programs\readings\main.cpp:14:14: error: no member named 'string_view' in namespace 'std'
   14 |         std::string_view text(argv[1]);
      |              ^~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:15:14: warning: structured binding declarations are a C++17 extension [-Wc++17-extensions]
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |              ^~~~~~~~~~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:15:34: error: no member named 'from_chars' in namespace 'std'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                  ^~~~~~~~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:15:45: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                             ^~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:15:58: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                          ^~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:15:72: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                                        ^~~~
C:\a\lengua\lengua\benches\programs\readings\main.cpp:16:27: error: no member named 'errc' in namespace 'std'
   16 |         if (error != std::errc{} || end != text.data() + text.size()) {
      |                           ^~~~
1 warning and 6 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 12 s | means 18.0 ms against 17.9 ms: +97.7 µs (-2.11 ms to +2.30 ms) | 21.5 ms against 21.4 ms: +129 µs (+15.8 µs to +239 µs) | 11.6 MiB against 10.7 MiB: +888 KiB (+888 KiB to +892 KiB) |
| add | 160, 160 in 13 s | means 17.7 ms against 16.3 ms: +1.37 ms (-655 µs to +3.39 ms) | 22.4 ms against 22.3 ms: +130 µs (-13.5 µs to +276 µs) | 11.7 MiB against 10.8 MiB: +928 KiB (+924 KiB to +928 KiB) |
| hypotenuse | 160, 160 in 13 s | means 17.6 ms against 17.7 ms: -97.7 µs (-2.55 ms to +2.36 ms) | 22.2 ms against 22.0 ms: +98.9 µs (+9.60 µs to +184 µs) | 11.7 MiB against 10.8 MiB: +940 KiB (+940 KiB to +944 KiB) |
| upcase | 160, 160 in 13 s | means 18.6 ms against 18.3 ms: +293 µs (-1.58 ms to +2.16 ms) | 23.8 ms against 23.5 ms: +256 µs (+162 µs to +357 µs) | 12.1 MiB against 11.2 MiB: +984 KiB (+980 KiB to +988 KiB) |
| upcase-stdin | 160, 160 in 21 s | means 45.2 ms against 43.3 ms: +1.95 ms (-635 µs to +4.54 ms) | 48.5 ms against 48.6 ms: +14.5 µs (-247 µs to +265 µs) | 11.7 MiB against 10.8 MiB: +956 KiB (+956 KiB to +960 KiB) |
| shout | 160, 160 in 13 s | means 18.6 ms against 18.5 ms: +97.7 µs (-2.35 ms to +2.54 ms) | 22.6 ms against 22.4 ms: +160 µs (+63.8 µs to +256 µs) | 11.9 MiB against 11.0 MiB: +984 KiB (+984 KiB to +988 KiB) |
| wc | 160, 160 in 23 s | means 50.1 ms against 48.9 ms: +1.17 ms (-1.49 ms to +3.84 ms) | 52.5 ms against 52.6 ms: -72.0 µs (-322 µs to +190 µs) | 11.8 MiB against 10.8 MiB: +1.00 MiB (+1.00 MiB to +1.01 MiB) |
| histogram | 160, 160 in 29 s | means 69.2 ms against 70.4 ms: -1.17 ms (-3.52 ms to +1.17 ms) | 73.0 ms against 73.0 ms: -11.8 µs (-295 µs to +261 µs) | 11.7 MiB against 10.8 MiB: +924 KiB (+920 KiB to +924 KiB) |
| crc32c | 160, 160 in 28 s | means 65.9 ms against 65.8 ms: +97.7 µs (-2.18 ms to +2.37 ms) | 69.8 ms against 69.7 ms: -18.6 µs (-279 µs to +245 µs) | 11.7 MiB against 10.8 MiB: +924 KiB (+920 KiB to +924 KiB) |
| sort | 91, 91 in 60 s | means 309 ms against 306 ms: +2.75 ms (-1.24 ms to +6.73 ms) | 312 ms against 312 ms: +416 µs (-930 µs to +1.67 ms) | 80.6 MiB against 79.6 MiB: +1.03 MiB (+1.02 MiB to +1.04 MiB) |
| tally | 144, 144 in 60 s | means 184 ms against 183 ms: +868 µs (-1.76 ms to +3.50 ms) | 189 ms against 189 ms: +543 µs (-198 µs to +1.29 ms) | 12.0 MiB against 11.0 MiB: +1.04 MiB (+1.03 MiB to +1.04 MiB) |
| server | 20, 20 in 72 s | means 1.65 s against 1.65 s: +1.56 ms (-20.1 ms to +23.2 ms) | 1.66 s against 1.66 s: +1.94 ms (-12.1 ms to +16.7 ms) | 11.5 MiB against 10.6 MiB: +956 KiB (+944 KiB to +992 KiB) |
| files | 130, 130 in 60 s | means 207 ms against 204 ms: +3.37 ms (+545 µs to +6.19 ms) | 211 ms against 211 ms: +634 µs (-530 µs to +1.85 ms) | 11.9 MiB against 11.0 MiB: +948 KiB (+944 KiB to +952 KiB) |
| readings | 160, 160 in 46 s | means 123 ms against 122 ms: +1.56 ms (-961 µs to +4.09 ms) | 126 ms against 126 ms: +188 µs (-30.4 µs to +398 µs) | 11.7 MiB against 10.8 MiB: +928 KiB (+924 KiB to +928 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
