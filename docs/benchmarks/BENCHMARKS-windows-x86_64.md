# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 33ed7fe1f45158efe1a2695d8accd88e1dfb18a5, in https://github.com/laughing-hermit/lengua/actions/runs/42947294489. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (25.4 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ? ✗ ✗ ✓ | ? ✗ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| hypotenuse | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| upcase | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ? ✓ ✗ ✗ | ? ✓ ✗ ✗ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ? ✗ ✗ ✓ | ? ✗ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 hypotenuse cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 671 names the row: take it out, as the list only shrinks
- windows-x86_64 hypotenuse rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 673 names the row: take it out, as the list only shrinks
- windows-x86_64 wc rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 2.70× | 3.03× | 249.02× |
| add | – | 2.75× | 3.01× | 217.77× |
| hypotenuse | – | 2.67× | 3.01× | 215.07× |
| upcase | – | 2.40× | 2.42× | 209.03× |
| upcase-stdin | 2.00× | 1.76× | 3.00× | 238.75× |
| shout | – | 2.48× | 2.43× | 229.28× |
| wc | 1.00× | 1.36× | 2.99× | 217.78× |
| histogram | 1.50× | 1.27× | 2.96× | 218.69× |
| crc32c | 3.00× | 2.27× | 2.97× | 232.36× |
| sort | 1.00× | 1.03× | 1.01× | 216.89× |
| tally | 0.50× | 0.52× | 2.65× | 202.54× |
| server | 1.48× | 2.47× | 2.54× | 221.48× |
| files | 1.80× | 1.68× | 2.90× | 209.03× |
| readings | 3.25× | 3.98× | 3.02× | 209.87× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | undecided, 1.00×, p 0.382 | behind, 1.01×, p 3.1e-27 | behind, 1.12× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | undecided, 1.00×, p 0.187 | behind, 1.01×, p 9.6e-28 | behind, 1.12× | ahead, 0.89× |
| empty | rust-wasm | 2000 | undecided, 1.00×, p 0.207 | ahead, 0.97×, p 3.3e-189 | behind, 1.11× | ahead, 0.01× |
| empty | c | 40 | behind, p 8.7e-8 | behind, 2.69×, p 7.2e-15 | behind, 3.03× | behind, 232.35× |
| empty | cpp | 40 | behind, p 1.0e-5 | behind, 2.70×, p 7.2e-15 | behind, 3.03× | behind, 232.35× |
| empty | rust | 20 | behind, p 9.4e-4 | behind, 2.47×, p 3.4e-8 | behind, 2.74× | behind, 249.02× |
| add | c-wasm | 2000 | undecided, 1.00×, p 0.484 | ahead, 0.97×, p 8.8e-87 | behind, 1.11× | ahead, 0.04× |
| add | cpp-wasm | 2000 | undecided, 1.00×, p 0.424 | ahead, 0.98×, p 2.1e-46 | behind, 1.11× | ahead, 0.04× |
| add | rust-wasm | 2000 | undecided, 1.00×, p 0.054 | ahead, 0.95×, p 1.4e-245 | behind, 1.11× | ahead, 0.02× |
| add | c | 40 | behind, p 0.001 | behind, 2.75×, p 7.2e-15 | behind, 3.01× | behind, 176.43× |
| add | cpp | 0 | undecided | undecided | undecided | undecided |
| add | rust | 80 | behind, p 7.7e-7 | behind, 2.47×, p 4.7e-28 | behind, 2.73× | behind, 217.77× |
| hypotenuse | c-wasm | 2000 | undecided, 1.00×, p 0.025 | ahead, 0.98×, p 2.5e-81 | behind, 1.11× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | undecided, 1.00×, p 0.019 | ahead, 0.99×, p 2.0e-59 | behind, 1.12× | ahead, 0.05× |
| hypotenuse | rust-wasm | 160 | ahead, 0.00×, p 1.0e-4 | ahead, 0.96×, p 2.6e-24 | behind, 1.11× | ahead, 0.03× |
| hypotenuse | c | 80 | behind, p 1.2e-5 | behind, 2.67×, p 4.7e-28 | behind, 3.01× | behind, 175.25× |
| hypotenuse | cpp | 0 | undecided | undecided | undecided | undecided |
| hypotenuse | rust | 80 | behind, p 8.3e-6 | behind, 2.38×, p 4.7e-28 | behind, 2.72× | behind, 215.07× |
| upcase | c-wasm | 2000 | undecided, 1.00×, p 0.376 | ahead, 1.00×, p 5.1e-6 | behind, 1.10× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | undecided, 1.00×, p 0.487 | ahead, 1.00×, p 2.0e-5 | behind, 1.10× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | undecided, 1.00×, p 0.423 | ahead, 0.97×, p 1.0e-187 | behind, 1.08× | ahead, 0.03× |
| upcase | c | 40 | behind, p 2.3e-7 | behind, 2.35×, p 7.2e-15 | behind, 2.38× | behind, 165.76× |
| upcase | cpp | 40 | behind, p 2.0e-6 | behind, 2.40×, p 7.2e-15 | behind, 2.38× | behind, 165.76× |
| upcase | rust | 40 | behind, p 2.7e-5 | behind, 2.26×, p 7.2e-15 | behind, 2.42× | behind, 209.03× |
| upcase-stdin | c-wasm | 2000 | undecided, 1.00×, p 0.161 | ahead, 0.98×, p 1.4e-70 | behind, 1.11× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | undecided, 1.00×, p 0.289 | ahead, 0.98×, p 3.6e-110 | behind, 1.11× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | undecided, 1.00×, p 0.131 | ahead, 0.98×, p 4.0e-139 | behind, 1.11× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 5.3e-5 | behind, 1.75×, p 3.4e-8 | behind, 3.00× | behind, 223.38× |
| upcase-stdin | cpp | 20 | behind, 2.00×, p 4.1e-5 | behind, 1.76×, p 3.4e-8 | behind, 3.00× | behind, 223.38× |
| upcase-stdin | rust | 40 | behind, 2.00×, p 3.0e-7 | behind, 1.60×, p 7.2e-15 | behind, 2.71× | behind, 238.75× |
| shout | c-wasm | 2000 | undecided, 1.00×, p 0.180 | behind, 1.01×, p 1.7e-20 | behind, 1.11× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | undecided, 1.00×, p 0.227 | behind, 1.01×, p 1.1e-41 | behind, 1.11× | ahead, 0.40× |
| shout | rust-wasm | 2000 | undecided, 1.00×, p 0.236 | ahead, 0.97×, p 9.1e-220 | behind, 1.09× | ahead, 0.03× |
| shout | c | 20 | behind, p 6.1e-5 | behind, 2.47×, p 3.4e-8 | behind, 2.42× | behind, 218.69× |
| shout | cpp | 20 | behind, p 8.3e-4 | behind, 2.48×, p 3.4e-8 | behind, 2.41× | behind, 218.69× |
| shout | rust | 20 | behind, p 2.1e-5 | behind, 2.29×, p 3.4e-8 | behind, 2.43× | behind, 229.28× |
| wc | c-wasm | 2000 | ahead, 1.00×, p 2.7e-4 | ahead, 0.95×, p 0.0e0 | behind, 1.11× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 8.0e-9 | ahead, 0.18×, p 3.4e-8 | behind, 1.11× | ahead, 0.11× |
| wc | rust-wasm | 2000 | undecided, 1.00×, p 0.003 | ahead, 0.93×, p 0.0e0 | behind, 1.11× | ahead, 0.04× |
| wc | c | 20 | behind, 1.00×, p 2.7e-4 | behind, 1.36×, p 3.4e-8 | behind, 2.99× | behind, 174.08× |
| wc | cpp | 640 | behind, 1.00×, p 1.2e-4 | ahead, 0.98×, p 9.6e-85 | behind, 2.99× | behind, 173.50× |
| wc | rust | 40 | behind, 1.00×, p 1.4e-5 | behind, 1.34×, p 7.2e-15 | behind, 2.69× | behind, 217.78× |
| histogram | c-wasm | 40 | ahead, 1.00×, p 1.8e-5 | ahead, 0.82×, p 7.2e-15 | behind, 1.11× | ahead, 0.09× |
| histogram | cpp-wasm | 40 | ahead, 1.00×, p 2.0e-4 | ahead, 0.82×, p 7.2e-15 | behind, 1.11× | ahead, 0.09× |
| histogram | rust-wasm | 80 | ahead, 1.00×, p 8.3e-5 | ahead, 0.86×, p 1.5e-26 | behind, 1.10× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.50×, p 2.2e-5 | behind, 1.27×, p 3.4e-8 | behind, 2.96× | behind, 175.84× |
| histogram | cpp | 20 | behind, 1.50×, p 5.1e-6 | behind, 1.27×, p 3.4e-8 | behind, 2.96× | behind, 175.84× |
| histogram | rust | 20 | behind, 1.50×, p 2.2e-4 | behind, 1.19×, p 3.5e-7 | behind, 2.69× | behind, 218.69× |
| crc32c | c-wasm | 40 | ahead, 0.75×, p 8.6e-8 | ahead, 0.81×, p 7.2e-15 | behind, 1.11× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | ahead, 0.75×, p 4.8e-6 | ahead, 0.81×, p 7.2e-15 | behind, 1.11× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.75×, p 5.6e-4 | ahead, 0.81×, p 3.4e-8 | behind, 1.11× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.00×, p 7.7e-9 | behind, 2.27×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| crc32c | cpp | 20 | behind, 2.00×, p 9.0e-9 | behind, 2.21×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| crc32c | rust | 20 | behind, 2.00×, p 2.7e-8 | behind, 2.14×, p 3.4e-8 | behind, 2.68× | behind, 232.36× |
| sort | c-wasm | 20 | ahead, 0.19×, p 2.3e-8 | ahead, 0.19×, p 3.4e-8 | behind, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.54×, p 1.9e-8 | ahead, 0.55×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.58×, p 2.1e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.41×, p 1.9e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.01× | behind, 216.89× |
| sort | cpp | 0 | undecided | undecided | undecided | undecided |
| sort | rust | 40 | behind, 1.00×, p 2.3e-5 | behind, 1.03×, p 5.9e-9 | ahead, 0.94× | behind, 215.99× |
| tally | c-wasm | 20 | ahead, 0.37×, p 1.4e-8 | ahead, 0.39×, p 3.4e-8 | behind, 1.13× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.37×, p 1.4e-8 | ahead, 0.39×, p 3.4e-8 | behind, 1.13× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.30×, p 1.5e-8 | ahead, 0.32×, p 3.4e-8 | behind, 1.14× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.50×, p 1.7e-8 | ahead, 0.52×, p 3.4e-8 | behind, 2.58× | behind, 171.23× |
| tally | cpp | 0 | undecided | undecided | undecided | undecided |
| tally | rust | 20 | ahead, 0.44×, p 2.1e-8 | ahead, 0.48×, p 3.4e-8 | behind, 2.65× | behind, 202.54× |
| server | c-wasm | 92 | undecided, 1.00×, p 0.430 | ahead, 0.71×, p 5.4e-32 | behind, 1.12× | ahead, 0.12× |
| server | cpp-wasm | 92 | undecided, 1.00×, p 0.419 | ahead, 0.74×, p 5.4e-32 | behind, 1.11× | ahead, 0.06× |
| server | rust-wasm | 92 | undecided, 1.00×, p 0.378 | ahead, 0.77×, p 5.4e-32 | behind, 1.09× | ahead, 0.03× |
| server | c | 20 | behind, 1.08×, p 2.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 2.54× | behind, 221.48× |
| server | cpp | 20 | behind, 1.08×, p 2.5e-8 | behind, 1.06×, p 3.4e-8 | behind, 2.54× | behind, 215.08× |
| server | rust | 20 | behind, 1.48×, p 2.9e-8 | behind, 2.47×, p 3.4e-8 | behind, 1.93× | behind, 183.92× |
| files | c-wasm | 80 | ahead, 0.90×, p 2.6e-4 | ahead, 0.97×, p 4.9e-14 | behind, 1.10× | ahead, 0.05× |
| files | cpp-wasm | 160 | ahead, 1.00×, p 5.3e-4 | ahead, 0.97×, p 1.4e-26 | behind, 1.10× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 0.90×, p 5.4e-4 | ahead, 0.96×, p 1.1e-13 | behind, 1.10× | ahead, 0.03× |
| files | c | 20 | behind, 1.50×, p 9.3e-9 | behind, 1.55×, p 3.4e-8 | behind, 2.90× | behind, 167.36× |
| files | cpp | 20 | behind, 1.50×, p 1.1e-8 | behind, 1.54×, p 3.4e-8 | behind, 2.90× | behind, 167.36× |
| files | rust | 20 | behind, 1.80×, p 2.0e-8 | behind, 1.68×, p 3.4e-8 | behind, 2.66× | behind, 209.03× |
| readings | c-wasm | 80 | ahead, 0.86×, p 7.7e-6 | ahead, 0.93×, p 2.7e-25 | behind, 1.11× | ahead, 0.04× |
| readings | cpp-wasm | 80 | ahead, 0.86×, p 2.1e-4 | ahead, 0.95×, p 3.4e-23 | behind, 1.11× | ahead, 0.04× |
| readings | rust-wasm | 80 | ahead, 0.86×, p 2.0e-4 | ahead, 0.92×, p 8.1e-26 | behind, 1.11× | ahead, 0.02× |
| readings | c | 20 | behind, 3.25×, p 2.1e-8 | behind, 3.98×, p 3.4e-8 | behind, 3.02× | behind, 176.43× |
| readings | cpp | 0 | undecided | undecided | undecided | undecided |
| readings | rust | 20 | behind, 3.25×, p 1.8e-8 | behind, 2.73×, p 3.4e-8 | behind, 2.72× | behind, 209.87× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 40 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 9.95 ms | 9.86 ms | 9.86 ms | 10.3 ms | 3.83 ms | 3.81 ms | 4.25 ms |
| peak memory | 10.2 MiB | 9.19 MiB | 9.19 MiB | 9.26 MiB | 3.38 MiB | 3.38 MiB | 3.74 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 157 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 0 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 9.88 ms | 10.2 ms | 10.0 ms | 10.4 ms | 3.68 ms | – | 4.12 ms |
| peak memory | 10.3 MiB | 9.29 MiB | 9.29 MiB | 9.29 MiB | 3.42 MiB | – | 3.77 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 0 B | 120 KiB |

Its rounds took 157 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\add\main.cpp: D:\a\lengua\lengua\benches\programs\add\main.cpp:10:6: error: no template named 'optional' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      | ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:10:37: error: no type named 'string_view' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      |                                ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:12:10: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |          ^~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\add\main.cpp:12:30: error: no member named 'from_chars' in namespace 'std'
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |                         ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:13:23: error: no member named 'errc' in namespace 'std'
   13 |     if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
      |                  ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:15:21: error: no member named 'nullopt' in namespace 'std'
   15 |         return std::nullopt;
      |                ~~~~~^
1 warning and 5 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 160 | 80 | 0 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 9.67 ms | 9.82 ms | 9.80 ms | 10.1 ms | 3.65 ms | – | 4.10 ms |
| peak memory | 10.3 MiB | 9.28 MiB | 9.20 MiB | 9.29 MiB | 3.42 MiB | – | 3.78 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 0 B | 121 KiB |

Its rounds took 119 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp: D:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp:13:16: error: no type named 'string_view' in namespace 'std'
   13 | int parse(std::string_view text, long long& units) {
      |           ~~~~~^
1 error generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 10.7 ms | 10.8 ms | 10.8 ms | 11.0 ms | 4.58 ms | 4.49 ms | 4.76 ms |
| peak memory | 10.7 MiB | 9.73 MiB | 9.73 MiB | 9.88 MiB | 4.50 MiB | 4.50 MiB | 4.42 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 157 KiB | 157 KiB | 124 KiB |

Its rounds took 163 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 26.9 ms | 27.3 ms | 27.4 ms | 27.5 ms | 15.2 ms | 15.1 ms | 16.6 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.31 MiB | 3.45 MiB | 3.44 MiB | 3.82 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 109 KiB |

Its rounds took 303 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 9.95 ms | 9.88 ms | 9.84 ms | 10.3 ms | 4.27 ms | 4.26 ms | 4.61 ms |
| peak memory | 10.6 MiB | 9.52 MiB | 9.52 MiB | 9.71 MiB | 4.39 MiB | 4.39 MiB | 4.36 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 155 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 20 | 2000 | 20 | 640 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 188 ms | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 34.4 ms | 36.3 ms | 196 ms | 36.8 ms | 25.6 ms | 35.1 ms | 25.8 ms |
| peak memory | 10.4 MiB | 9.31 MiB | 9.30 MiB | 9.31 MiB | 3.47 MiB | 3.47 MiB | 3.85 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 150 KiB | 150 KiB | 120 KiB |

Its rounds took 315 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 40 | 40 | 80 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 46.9 ms | 46.9 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 43.5 ms | 53.3 ms | 53.0 ms | 50.3 ms | 34.4 ms | 34.2 ms | 36.7 ms |
| peak memory | 10.3 MiB | 9.31 MiB | 9.31 MiB | 9.32 MiB | 3.48 MiB | 3.48 MiB | 3.82 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 119 KiB |

Its rounds took 18 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 62.5 ms | 62.5 ms | 15.6 ms | 23.4 ms | 23.4 ms |
| wall time | 48.6 ms | 59.9 ms | 59.9 ms | 59.7 ms | 21.4 ms | 21.9 ms | 22.7 ms |
| peak memory | 10.3 MiB | 9.31 MiB | 9.31 MiB | 9.31 MiB | 3.47 MiB | 3.47 MiB | 3.84 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 13 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 20 | 20 | 0 | 40 |
| CPU time | 219 ms | 1.14 s | 406 ms | 375 ms | 531 ms | – | 219 ms |
| wall time | 224 ms | 1.15 s | 406 ms | 378 ms | 537 ms | – | 217 ms |
| peak memory | 79.2 MiB | 78.1 MiB | 86.2 MiB | 86.3 MiB | 78.4 MiB | – | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 0 B | 120 KiB |

Its rounds took 78 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\sort\main.cpp: D:\a\lengua\lengua\benches\programs\sort\main.cpp:39:22: error: no member named 'string_view' in namespace 'std'
   39 |     std::vector<std::string_view> lines;
      |                 ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:21: error: expected ';' after expression
   40 |     std::string_view rest(input, length);
      |                     ^
      |                     ;
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:10: error: no member named 'string_view' in namespace 'std'
   40 |     std::string_view rest(input, length);
      |     ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:22: error: use of undeclared identifier 'rest'
   40 |     std::string_view rest(input, length);
      |                      ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:41:13: error: use of undeclared identifier 'rest'
   41 |     while (!rest.empty()) {
      |             ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:42:31: error: use of undeclared identifier 'rest'
   42 |         std::size_t newline = rest.find('\n');
      |                               ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:43:25: error: use of undeclared identifier 'rest'
   43 |         lines.push_back(rest.substr(0, newline));
      |                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:44:9: error: use of undeclared identifier 'rest'
   44 |         rest.remove_prefix(newline + 1);
      |         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:27: error: use of undeclared identifier 'line'; did you mean 'lines'?
   52 |     for (std::string_view line : lines) {
      |                           ^~~~
      |                           lines
D:\a\lengua\lengua\benches\programs\sort\main.cpp:39:35: note: 'lines' declared here
   39 |     std::vector<std::string_view> lines;
      |                                   ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:15: error: no member named 'string_view' in namespace 'std'
   52 |     for (std::string_view line : lines) {
      |          ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:39: error: expected ';' in 'for' statement specifier
   52 |     for (std::string_view line : lines) {
      |                                       ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:25: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:41: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:69: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                                                                     ^
14 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 0 | 20 |
| CPU time | 109 ms | 297 ms | 297 ms | 359 ms | 219 ms | – | 250 ms |
| wall time | 118 ms | 299 ms | 300 ms | 364 ms | 226 ms | – | 248 ms |
| peak memory | 10.6 MiB | 9.36 MiB | 9.37 MiB | 9.32 MiB | 4.11 MiB | – | 4.00 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 0 B | 128 KiB |

Its rounds took 37 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\tally\main.cpp: D:\a\lengua\lengua\benches\programs\tally\main.cpp:42:6: error: no type named 'string_view' in namespace 'std'
   42 | std::string_view word_of(const Entry& entry) {
      | ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:43:12: error: excess elements in scalar initializer
   43 |     return {store.data() + entry.offset, entry.length};
      |            ^                           ~~~~~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\tally\main.cpp:47:15: error: no type named 'string_view' in namespace 'std'
   47 | bool add(std::string_view word) {
      |          ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:107:27: error: no member named 'string_view' in namespace 'std'
  107 |             if (!add(std::string_view(buffer.data() + start, at - start))) {
      |                      ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:125:14: error: no type named 'string_view' in namespace 'std'
  125 |         std::string_view word = word_of(entry);
      |         ~~~~~^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8923:17: error: static_cast from 'void' to 'bool' is not allowed
 8923 |             if (_DEBUG_LT_PRED(_Pred, _Val, *_First)) { // found new earliest element, move to front
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9055:18: note: in instantiation of function template specialization 'std::_Insertion_sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9055 |             _STD _Insertion_sort_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:71:13: error: static_cast from 'void' to 'bool' is not allowed
   71 |         if (_DEBUG_LT_PRED(_Pred, *(_First + _Idx), *(_First + (_Idx - 1)))) {
      |             ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:27:25: error: static_cast from 'void' to 'bool' is not allowed
   27 |         _Top < _Hole && _DEBUG_LT_PRED(_Pred, *(_First + _Idx), _Val); _Idx = (_Hole - 1) >> 1) { // shift for codegen
      |                         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:83:10: note: in instantiation of function template specialization 'std::_Push_heap_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
   83 |     _STD _Push_heap_by_index(_First, _Hole, _Top, _STD forward<_Ty>(_Val), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:93:5: error: no matching function for call to '_Pop_heap_hole_by_index'
   93 |     _STD _Pop_heap_hole_by_index(
      |     ^~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\yvals_core.h:1906:17: note: expanded from macro '_STD'
 1906 | #define _STD    ::std::
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:103:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_unchecked<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  103 |         _STD _Pop_heap_hole_unchecked(_First, _Last, _Last, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:7816:14: note: in instantiation of function template specialization 'std::_Pop_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 7816 |         _STD _Pop_heap_unchecked(_First, _Last, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9061:18: note: in instantiation of function template specialization 'std::_Sort_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9061 |             _STD _Sort_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:57:19: note: candidate template ignored: substitution failure [with _RanIt = (anonymous namespace)::Entry *, _Ty = (anonymous namespace)::Entry, _Pr = (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)]
   57 | _CONSTEXPR20 void _Pop_heap_hole_by_index(
      |                   ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8942:9: error: static_cast from 'void' to 'bool' is not allowed
 8942 |     if (_DEBUG_LT_PRED(_Pred, *_Mid, *_First)) {
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8963:14: note: in instantiation of function template specialization 'std::_Med3_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 8963 |         _STD _Med3_unchecked(_First, _First + _Step, _First + _Two_step, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9045:10: note: in instantiation of function template specialization 'std::_Guess_median_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9045 |     _STD _Guess_median_unchecked(_First, _Mid, _STD _Prev_iter(_Last), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8946:9: error: static_cast from 'void' to 'bool' is not allowed
 8946 |     if (_DEBUG_LT_PRED(_Pred, *_Last, *_Mid)) { // swap middle and last, then test first again
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8978:33: error: static_cast from 'void' to 'bool' is not allowed
 8978 |     while (_First < _Pfirst && !_DEBUG_LT_PRED(_Pred, *_STD _Prev_iter(_Pfirst), *_Pfirst)
      |                                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9047:17: note: in instantiation of function template specialization 'std::_Partition_by_pivot_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9047 |     return _STD _Partition_by_pivot_unchecked(_First, _Mid, _Last, _Pred);
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8983:31: error: static_cast from 'void' to 'bool' is not allowed
 8983 |     while (_Plast < _Last && !_DEBUG_LT_PRED(_Pred, *_Plast, *_Pfirst) && !_Pred(*_Pfirst, *_Plast)) {
      |                               ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8992:17: error: static_cast from 'void' to 'bool' is not allowed
 8992 |             if (_DEBUG_LT_PRED(_Pred, *_Pfirst, *_Gfirst)) {
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
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
| counted runs | 92 | 92 | 92 | 92 | 20 | 20 | 20 |
| CPU time | 1.08 s | 1.08 s | 1.08 s | 1.08 s | 1.00 s | 1.00 s | 727 ms |
| wall time | 1.08 s | 1.52 s | 1.46 s | 1.40 s | 1.02 s | 1.02 s | 436 ms |
| peak memory | 10.2 MiB | 9.11 MiB | 9.16 MiB | 9.30 MiB | 4.00 MiB | 4.00 MiB | 5.27 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 121 KiB | 142 KiB |

Its rounds took 631 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 80 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 141 ms | 156 ms | 141 ms | 156 ms | 93.8 ms | 93.8 ms | 78.1 ms |
| wall time | 143 ms | 148 ms | 148 ms | 148 ms | 90.4 ms | 90.8 ms | 83.3 ms |
| peak memory | 10.4 MiB | 9.47 MiB | 9.47 MiB | 9.48 MiB | 3.60 MiB | 3.60 MiB | 3.92 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 85 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 80 | 80 | 20 | 0 | 20 |
| CPU time | 93.8 ms | 109 ms | 109 ms | 109 ms | 31.2 ms | – | 31.2 ms |
| wall time | 100 ms | 108 ms | 106 ms | 109 ms | 25.3 ms | – | 36.9 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.30 MiB | 3.41 MiB | – | 3.79 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 0 B | 124 KiB |

Its rounds took 41 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\readings\main.cpp: D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:25: error: expected ';' after expression
   14 |         std::string_view text(argv[1]);
      |                         ^
      |                         ;
D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:14: error: no member named 'string_view' in namespace 'std'
   14 |         std::string_view text(argv[1]);
      |         ~~~~~^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:26: error: use of undeclared identifier 'text'
   14 |         std::string_view text(argv[1]);
      |                          ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:14: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |              ^~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:34: error: no member named 'from_chars' in namespace 'std'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                             ~~~~~^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:45: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                             ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:58: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                          ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:72: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                                        ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:16:22: error: no member named 'errc' in namespace 'std'; did you mean 'argc'?
   16 |         if (error != std::errc{} || end != text.data() + text.size()) {
      |                      ^~~~~~~~~
      |                      argc
D:\a\lengua\lengua\benches\programs\readings\main.cpp:10:14: note: 'argc' declared here
   10 | int main(int argc, char** argv) {
      |              ^
1 warning and 8 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 6 s | means 13.3 ms against 10.5 ms: +2.73 ms (-225 µs to +5.69 ms) | 9.82 ms against 9.80 ms: +33.4 µs (-12.9 µs to +81.7 µs) | 10.2 MiB against 9.31 MiB: +952 KiB (+944 KiB to +960 KiB) |
| add | 160, 160 in 6 s | means 11.3 ms against 11.1 ms: +195 µs (-2.63 ms to +3.02 ms) | 9.60 ms against 9.59 ms: +44.6 µs (-9.70 µs to +97.4 µs) | 10.3 MiB against 9.30 MiB: +996 KiB (+992 KiB to +1000 KiB) |
| hypotenuse | 160, 160 in 6 s | means 10.7 ms against 9.86 ms: +879 µs (-1.53 ms to +3.29 ms) | 9.67 ms against 9.55 ms: +99.9 µs (+51.5 µs to +148 µs) | 10.3 MiB against 9.30 MiB: +1016 KiB (+1016 KiB to +1.00 MiB) |
| upcase | 160, 160 in 6 s | means 11.2 ms against 10.8 ms: +391 µs (-2.06 ms to +2.84 ms) | 10.7 ms against 10.5 ms: +246 µs (+188 µs to +300 µs) | 10.7 MiB against 9.62 MiB: +1.05 MiB (+1.04 MiB to +1.06 MiB) |
| upcase-stdin | 160, 160 in 12 s | means 29.9 ms against 26.6 ms: +3.32 ms (+630 µs to +6.01 ms) | 26.9 ms against 26.6 ms: +228 µs (+75.7 µs to +376 µs) | 10.3 MiB against 9.32 MiB: +1016 KiB (+1016 KiB to +1020 KiB) |
| shout | 160, 160 in 6 s | means 11.8 ms against 10.7 ms: +1.07 ms (-1.67 ms to +3.82 ms) | 9.92 ms against 9.84 ms: +58.2 µs (+8.50 µs to +109 µs) | 10.6 MiB against 9.60 MiB: +1.01 MiB (+1.00 MiB to +1.02 MiB) |
| wc | 160, 160 in 14 s | means 37.2 ms against 35.8 ms: +1.37 ms (-796 µs to +3.53 ms) | 34.3 ms against 34.4 ms: -8.00 µs (-201 µs to +198 µs) | 10.4 MiB against 9.33 MiB: +1.03 MiB (+1.03 MiB to +1.04 MiB) |
| histogram | 160, 160 in 17 s | means 46.4 ms against 46.4 ms: +0.00 ns (-2.88 ms to +2.88 ms) | 43.5 ms against 43.4 ms: +80.3 µs (-64.3 µs to +229 µs) | 10.3 MiB against 9.33 MiB: +1000 KiB (+992 KiB to +1004 KiB) |
| crc32c | 160, 160 in 19 s | means 50.8 ms against 50.8 ms: +0.00 ns (-2.05 ms to +2.05 ms) | 48.7 ms against 48.5 ms: +222 µs (+67.6 µs to +379 µs) | 10.3 MiB against 9.33 MiB: +992 KiB (+988 KiB to +996 KiB) |
| sort | 128, 128 in 60 s | means 226 ms against 223 ms: +3.66 ms (+494 µs to +6.83 ms) | 223 ms against 223 ms: -173 µs (-833 µs to +549 µs) | 79.2 MiB against 78.1 MiB: +1.08 MiB (+1.08 MiB to +1.09 MiB) |
| tally | 160, 160 in 42 s | means 121 ms against 123 ms: -1.66 ms (-4.73 ms to +1.41 ms) | 119 ms against 119 ms: +358 µs (+84.6 µs to +643 µs) | 10.6 MiB against 9.53 MiB: +1.07 MiB (+1.07 MiB to +1.07 MiB) |
| server | 25, 25 in 60 s | means 1.07 s against 1.07 s: +2.50 ms (-7.96 ms to +13.0 ms) | 1.08 s against 1.08 s: -20.2 µs (-2.21 ms to +1.91 ms) | 10.2 MiB against 9.13 MiB: +1.04 MiB (+1.02 MiB to +1.05 MiB) |
| files | 160, 160 in 49 s | means 142 ms against 142 ms: -488 µs (-3.22 ms to +2.24 ms) | 143 ms against 138 ms: +1.10 ms (+347 µs to +2.08 ms) | 10.4 MiB against 9.47 MiB: +988 KiB (+984 KiB to +992 KiB) |
| readings | 160, 160 in 36 s | means 103 ms against 101 ms: +1.86 ms (-783 µs to +4.49 ms) | 100 ms against 100 ms: +24.4 µs (-127 µs to +168 µs) | 10.3 MiB against 9.32 MiB: +988 KiB (+984 KiB to +992 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
