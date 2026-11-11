# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 33ed7fe1f45158efe1a2695d8accd88e1dfb18a5, in https://github.com/laughing-hermit/lengua/actions/runs/42947294489. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (16.9 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ✗ ? ✗ ✓ | ✗ ? ✗ ✓ | ? ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✗ ✓ ✗ ✓ | ✗ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| hypotenuse | ✗ ✓ ✗ ✓ | ✗ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ? ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ✗ ✓ | ✗ ? ✗ ✓ | ? ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✓ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✓ ✓ | ? ✗ ✗ ✗ | ? ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 empty rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 add c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 437 names the row: take it out, as the list only shrinks
- macos-aarch64 add cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 440 names the row: take it out, as the list only shrinks
- macos-aarch64 hypotenuse c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 455 names the row: take it out, as the list only shrinks
- macos-aarch64 hypotenuse cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 458 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 472 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 473 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 475 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 476 names the row: take it out, as the list only shrinks
- macos-aarch64 shout rust-wasm cpu: lengua is undecided, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 server rust cpu: lengua is ahead now, and KNOWN_BEHIND line 597 names the row: take it out, as the list only shrinks
- macos-aarch64 files c-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 601 names the row: take it out, as the list only shrinks
- macos-aarch64 files cpp-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 602 names the row: take it out, as the list only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.28× | 2.50× | 4.00× | 1049.36× |
| add | 2.53× | 1.99× | 4.10× | 524.33× |
| hypotenuse | 2.90× | 2.27× | 4.11× | 525.72× |
| upcase | 2.55× | 1.94× | 2.78× | 523.86× |
| upcase-stdin | 1.55× | 1.46× | 4.02× | 527.97× |
| shout | 2.55× | 2.01× | 2.76× | 526.58× |
| wc | 1.81× | 1.72× | 3.99× | 526.60× |
| histogram | 1.35× | 1.26× | 3.88× | 526.59× |
| crc32c | 2.71× | 2.58× | 3.90× | 526.60× |
| sort | 1.12× | 1.10× | 0.98× | 521.94× |
| tally | 0.46× | 0.46× | 3.14× | 520.48× |
| server | 1.06× | 2.03× | 4.28× | 519.42× |
| files | 1.75× | 1.72× | 4.42× | 522.36× |
| readings | 3.72× | 3.64× | 4.06× | 524.33× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | behind, 1.07×, p 3.3e-10 | undecided, 1.02×, p 0.040 | behind, 1.05× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | behind, 1.08×, p 1.8e-10 | undecided, 1.03×, p 0.036 | behind, 1.05× | ahead, 0.89× |
| empty | rust-wasm | 2000 | undecided, 1.00×, p 0.252 | ahead, 0.96×, p 2.4e-6 | ahead, 0.99× | ahead, 0.01× |
| empty | c | 20 | behind, 2.82×, p 4.6e-8 | behind, 2.28×, p 2.3e-7 | behind, 4.00× | behind, 1049.36× |
| empty | cpp | 20 | behind, 3.28×, p 1.7e-7 | behind, 2.50×, p 2.3e-6 | behind, 4.00× | behind, 1049.36× |
| empty | rust | 20 | behind, 2.38×, p 7.9e-7 | behind, 1.84×, p 1.6e-5 | behind, 3.85× | behind, 61.81× |
| add | c-wasm | 2000 | behind, 1.02×, p 2.6e-4 | ahead, 0.96×, p 2.5e-8 | behind, 1.02× | ahead, 0.04× |
| add | cpp-wasm | 2000 | behind, 1.02×, p 5.0e-4 | ahead, 0.96×, p 8.4e-9 | behind, 1.02× | ahead, 0.04× |
| add | rust-wasm | 640 | ahead, 0.95×, p 6.9e-4 | ahead, 0.90×, p 1.2e-9 | ahead, 0.95× | ahead, 0.02× |
| add | c | 20 | behind, 2.53×, p 1.1e-7 | behind, 1.99×, p 3.0e-7 | behind, 4.10× | behind, 524.33× |
| add | cpp | 0 | undecided | undecided | undecided | undecided |
| add | rust | 20 | behind, 2.37×, p 3.4e-8 | behind, 1.94×, p 1.1e-7 | behind, 3.81× | behind, 58.41× |
| hypotenuse | c-wasm | 40 | behind, 1.05×, p 1.1e-8 | ahead, 0.97×, p 2.8e-5 | behind, 1.02× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | behind, 1.05×, p 2.7e-4 | ahead, 0.96×, p 0.001 | behind, 1.02× | ahead, 0.05× |
| hypotenuse | rust-wasm | 40 | ahead, 0.98×, p 6.8e-4 | ahead, 0.91×, p 7.1e-14 | ahead, 0.94× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.90×, p 3.4e-8 | behind, 2.27×, p 3.4e-8 | behind, 4.06× | behind, 525.72× |
| hypotenuse | cpp | 20 | behind, 2.81×, p 3.4e-8 | behind, 2.20×, p 3.4e-8 | behind, 4.11× | behind, 524.97× |
| hypotenuse | rust | 20 | behind, 2.31×, p 3.4e-8 | behind, 1.87×, p 3.4e-8 | behind, 3.82× | behind, 58.40× |
| upcase | c-wasm | 2000 | ahead, 0.98×, p 1.9e-6 | ahead, 0.99×, p 1.4e-4 | behind, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 1280 | ahead, 0.98×, p 3.0e-5 | ahead, 0.98×, p 1.6e-4 | behind, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 640 | ahead, 0.96×, p 3.3e-8 | ahead, 0.93×, p 1.6e-16 | ahead, 0.94× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.50×, p 3.4e-8 | behind, 1.92×, p 3.4e-8 | behind, 2.78× | behind, 523.86× |
| upcase | cpp | 20 | behind, 2.55×, p 3.4e-8 | behind, 1.94×, p 3.4e-8 | behind, 2.78× | behind, 523.86× |
| upcase | rust | 20 | behind, 2.14×, p 3.4e-8 | behind, 1.67×, p 3.4e-8 | behind, 2.65× | behind, 58.40× |
| upcase-stdin | c-wasm | 20 | ahead, 0.95×, p 0.001 | ahead, 0.93×, p 1.3e-5 | behind, 1.07× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.90×, p 7.9e-7 | ahead, 0.88×, p 2.6e-7 | behind, 1.07× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | undecided, 1.00×, p 0.147 | ahead, 0.98×, p 3.2e-4 | ahead, 0.99× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.55×, p 3.4e-8 | behind, 1.45×, p 3.4e-8 | behind, 4.02× | behind, 527.97× |
| upcase-stdin | cpp | 20 | behind, 1.55×, p 3.4e-8 | behind, 1.46×, p 3.4e-8 | behind, 4.02× | behind, 527.97× |
| upcase-stdin | rust | 20 | behind, 1.50×, p 3.4e-8 | behind, 1.40×, p 3.4e-8 | behind, 3.62× | behind, 58.41× |
| shout | c-wasm | 2000 | behind, 1.10×, p 7.4e-22 | behind, 1.04×, p 5.2e-4 | behind, 1.05× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | behind, 1.09×, p 1.8e-18 | undecided, 1.03×, p 0.004 | behind, 1.05× | ahead, 0.40× |
| shout | rust-wasm | 2000 | undecided, 1.01×, p 0.294 | ahead, 0.96×, p 4.5e-7 | ahead, 0.98× | ahead, 0.03× |
| shout | c | 20 | behind, 2.55×, p 3.4e-8 | behind, 2.01×, p 3.4e-8 | behind, 2.76× | behind, 526.58× |
| shout | cpp | 20 | behind, 2.49×, p 6.0e-7 | behind, 1.97×, p 6.9e-7 | behind, 2.74× | behind, 526.58× |
| shout | rust | 20 | behind, 2.11×, p 5.2e-7 | behind, 1.66×, p 1.3e-5 | behind, 2.60× | behind, 58.41× |
| wc | c-wasm | 40 | behind, 1.22×, p 8.0e-6 | behind, 1.24×, p 8.3e-6 | behind, 1.04× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.15×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | behind, 1.04× | ahead, 0.11× |
| wc | rust-wasm | 20 | behind, 1.20×, p 3.8e-6 | behind, 1.19×, p 1.5e-5 | ahead, 0.96× | ahead, 0.04× |
| wc | c | 20 | behind, 1.78×, p 3.4e-8 | behind, 1.70×, p 5.3e-8 | behind, 3.94× | behind, 526.60× |
| wc | cpp | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | behind, 3.99× | behind, 526.60× |
| wc | rust | 20 | behind, 1.81×, p 5.3e-8 | behind, 1.72×, p 5.3e-8 | behind, 3.68× | behind, 58.41× |
| histogram | c-wasm | 20 | ahead, 0.70×, p 3.4e-8 | ahead, 0.67×, p 3.9e-8 | behind, 1.02× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.65×, p 3.4e-8 | behind, 1.02× | ahead, 0.09× |
| histogram | rust-wasm | 320 | ahead, 0.95×, p 2.1e-5 | ahead, 0.94×, p 5.9e-6 | ahead, 0.94× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.30×, p 1.9e-5 | behind, 1.21×, p 9.0e-5 | behind, 3.83× | behind, 526.59× |
| histogram | cpp | 40 | behind, 1.25×, p 6.6e-7 | behind, 1.17×, p 1.1e-5 | behind, 3.88× | behind, 526.47× |
| histogram | rust | 20 | behind, 1.35×, p 7.1e-6 | behind, 1.26×, p 3.3e-5 | behind, 3.58× | behind, 58.40× |
| crc32c | c-wasm | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | behind, 1.03× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | behind, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.65×, p 3.4e-8 | behind, 2.51×, p 3.4e-8 | behind, 3.90× | behind, 526.60× |
| crc32c | cpp | 20 | behind, 2.60×, p 3.4e-8 | behind, 2.49×, p 3.4e-8 | behind, 3.90× | behind, 526.60× |
| crc32c | rust | 20 | behind, 2.71×, p 3.4e-8 | behind, 2.58×, p 3.4e-8 | behind, 3.64× | behind, 58.41× |
| sort | c-wasm | 20 | ahead, 0.16×, p 3.4e-8 | ahead, 0.16×, p 3.4e-8 | behind, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.68×, p 3.9e-8 | ahead, 0.68×, p 3.9e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.98× | behind, 521.94× |
| sort | cpp | 20 | ahead, 0.77×, p 6.2e-8 | ahead, 0.76×, p 1.1e-7 | ahead, 0.81× | behind, 506.38× |
| sort | rust | 40 | behind, 1.12×, p 2.4e-4 | behind, 1.10×, p 7.6e-4 | ahead, 0.95× | behind, 58.42× |
| tally | c-wasm | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 1.07× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.03× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.33×, p 3.4e-8 | behind, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.46×, p 3.4e-8 | ahead, 0.46×, p 3.4e-8 | behind, 3.11× | behind, 520.48× |
| tally | cpp | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.44×, p 3.4e-8 | behind, 3.14× | behind, 504.32× |
| tally | rust | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 2.90× | behind, 55.38× |
| server | c-wasm | 43 | undecided, 0.97×, p 0.099 | ahead, 0.33×, p 7.8e-10 | behind, 1.05× | ahead, 0.12× |
| server | cpp-wasm | 43 | undecided, 0.92×, p 0.018 | ahead, 0.34×, p 3.0e-10 | behind, 1.02× | ahead, 0.06× |
| server | rust-wasm | 43 | undecided, 0.88×, p 0.002 | ahead, 0.35×, p 9.2e-10 | ahead, 0.96× | ahead, 0.03× |
| server | c | 43 | undecided, 1.02×, p 0.081 | behind, 1.78×, p 7.2e-8 | behind, 4.28× | behind, 519.42× |
| server | cpp | 43 | undecided, 1.06×, p 0.029 | behind, 1.76×, p 8.3e-8 | behind, 4.28× | behind, 502.18× |
| server | rust | 40 | ahead, 0.83×, p 2.8e-5 | behind, 2.03×, p 1.8e-9 | behind, 2.06× | behind, 55.24× |
| files | c-wasm | 160 | ahead, 0.93×, p 1.2e-4 | ahead, 0.94×, p 7.3e-5 | ahead, 0.99× | ahead, 0.05× |
| files | cpp-wasm | 160 | ahead, 0.94×, p 2.4e-4 | ahead, 0.93×, p 1.1e-4 | ahead, 0.99× | ahead, 0.05× |
| files | rust-wasm | 160 | ahead, 0.92×, p 6.4e-6 | ahead, 0.92×, p 4.2e-6 | ahead, 0.92× | ahead, 0.03× |
| files | c | 20 | behind, 1.62×, p 8.3e-8 | behind, 1.60×, p 1.1e-7 | behind, 4.42× | behind, 522.36× |
| files | cpp | 20 | behind, 1.60×, p 9.0e-7 | behind, 1.57×, p 1.3e-6 | behind, 4.36× | behind, 522.36× |
| files | rust | 20 | behind, 1.75×, p 1.1e-7 | behind, 1.72×, p 1.3e-7 | behind, 3.86× | behind, 58.40× |
| readings | c-wasm | 80 | ahead, 0.94×, p 4.8e-6 | ahead, 0.92×, p 3.9e-6 | behind, 1.02× | ahead, 0.04× |
| readings | cpp-wasm | 160 | ahead, 0.97×, p 5.6e-5 | ahead, 0.97×, p 4.1e-5 | behind, 1.03× | ahead, 0.04× |
| readings | rust-wasm | 80 | ahead, 0.95×, p 4.8e-5 | ahead, 0.93×, p 3.1e-5 | ahead, 0.94× | ahead, 0.02× |
| readings | c | 20 | behind, 3.00×, p 3.4e-8 | behind, 2.94×, p 3.4e-8 | behind, 4.06× | behind, 524.33× |
| readings | cpp | 0 | undecided | undecided | undecided | undecided |
| readings | rust | 20 | behind, 3.72×, p 3.4e-8 | behind, 3.64×, p 3.4e-8 | behind, 3.92× | behind, 58.40× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 4.77 ms | 4.44 ms | 4.40 ms | 4.77 ms | 2.62 ms | 2.25 ms | 3.10 ms |
| wall time | 4.86 ms | 4.77 ms | 4.72 ms | 5.08 ms | 3.43 ms | 3.12 ms | 4.25 ms |
| peak memory | 4.95 MiB | 4.70 MiB | 4.70 MiB | 5.00 MiB | 1.23 MiB | 1.23 MiB | 1.28 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 85 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 0 | 20 |
| CPU time | 3.99 ms | 3.90 ms | 3.91 ms | 5.03 ms | 2.22 ms | – | 2.37 ms |
| wall time | 3.95 ms | 4.11 ms | 4.12 ms | 5.29 ms | 2.75 ms | – | 2.83 ms |
| peak memory | 5.12 MiB | 5.03 MiB | 5.03 MiB | 5.39 MiB | 1.25 MiB | – | 1.34 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 0 B | 295 KiB |

Its rounds took 58 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp wasn't measured to the end: c++ couldn't build /Users/runner/work/lengua/lengua/benches/programs/add/main.cpp: /Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:10:6: error: no template named 'optional' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      | ~~~~~^
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:12:10: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |          ^~~~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:12:30: error: no member named 'from_chars' in namespace 'std'
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |                              ^~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:13:23: error: no member named 'errc' in namespace 'std'; did you mean 'erfc'?
   13 |     if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
      |                       ^~~~
      |                       erfc
/Applications/Xcode_26.6.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/usr/include/c++/v1/cmath:438:9: note: 'erfc' declared here
  438 | using ::erfc _LIBCPP_USING_IF_EXISTS;
      |         ^
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:15:21: error: no member named 'nullopt' in namespace 'std'
   15 |         return std::nullopt;
      |                     ^~~~~~~
1 warning and 4 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 3.39 ms | 3.22 ms | 3.25 ms | 3.47 ms | 1.17 ms | 1.21 ms | 1.47 ms |
| wall time | 3.29 ms | 3.39 ms | 3.43 ms | 3.63 ms | 1.45 ms | 1.50 ms | 1.76 ms |
| peak memory | 5.12 MiB | 5.00 MiB | 5.03 MiB | 5.44 MiB | 1.27 MiB | 1.25 MiB | 1.34 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 1 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 1280 | 640 | 20 | 20 | 20 |
| CPU time | 4.27 ms | 4.33 ms | 4.38 ms | 4.67 ms | 1.90 ms | 1.86 ms | 2.22 ms |
| wall time | 4.19 ms | 4.24 ms | 4.27 ms | 4.67 ms | 2.26 ms | 2.24 ms | 2.61 ms |
| peak memory | 6.25 MiB | 6.23 MiB | 6.23 MiB | 6.67 MiB | 2.25 MiB | 2.25 MiB | 2.36 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 50 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 20 | 20 | 2000 | 20 | 20 | 20 |
| CPU time | 16.1 ms | 13.0 ms | 13.8 ms | 16.1 ms | 8.02 ms | 8.02 ms | 8.28 ms |
| wall time | 15.5 ms | 13.0 ms | 13.8 ms | 15.8 ms | 8.35 ms | 8.32 ms | 8.69 ms |
| peak memory | 5.16 MiB | 4.83 MiB | 4.83 MiB | 5.23 MiB | 1.28 MiB | 1.28 MiB | 1.42 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 91 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 5.35 ms | 4.86 ms | 4.89 ms | 5.29 ms | 1.97 ms | 2.01 ms | 2.38 ms |
| wall time | 5.33 ms | 5.11 ms | 5.17 ms | 5.55 ms | 2.31 ms | 2.35 ms | 2.79 ms |
| peak memory | 6.12 MiB | 5.81 MiB | 5.81 MiB | 6.25 MiB | 2.22 MiB | 2.23 MiB | 2.36 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 88 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 29.8 ms | 24.4 ms | 207 ms | 25.2 ms | 16.9 ms | 148 ms | 16.6 ms |
| wall time | 29.5 ms | 23.8 ms | 207 ms | 25.0 ms | 17.5 ms | 149 ms | 17.3 ms |
| peak memory | 5.23 MiB | 5.03 MiB | 5.03 MiB | 5.45 MiB | 1.33 MiB | 1.31 MiB | 1.42 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 14 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 20 | 20 | 320 | 20 | 40 | 20 |
| CPU time | 43.2 ms | 62.8 ms | 65.4 ms | 45.5 ms | 33.8 ms | 33.5 ms | 32.5 ms |
| wall time | 42.6 ms | 62.4 ms | 65.0 ms | 45.5 ms | 34.6 ms | 34.2 ms | 33.4 ms |
| peak memory | 5.16 MiB | 5.04 MiB | 5.02 MiB | 5.48 MiB | 1.34 MiB | 1.33 MiB | 1.44 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 41 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 48.9 ms | 67.9 ms | 67.2 ms | 73.1 ms | 18.4 ms | 18.8 ms | 18.0 ms |
| wall time | 48.4 ms | 68.0 ms | 66.9 ms | 72.9 ms | 19.3 ms | 19.4 ms | 18.8 ms |
| peak memory | 5.18 MiB | 5.03 MiB | 5.03 MiB | 5.33 MiB | 1.33 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 8 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=apple-m1

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=apple-m1

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 20 | 20 | 20 | 40 |
| CPU time | 229 ms | 1.37 s | 366 ms | 318 ms | 323 ms | 281 ms | 204 ms |
| wall time | 228 ms | 1.37 s | 370 ms | 318 ms | 324 ms | 282 ms | 207 ms |
| peak memory | 74.8 MiB | 74.5 MiB | 82.6 MiB | 82.8 MiB | 76.3 MiB | 92.4 MiB | 78.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 88 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 125 ms | 290 ms | 296 ms | 371 ms | 271 ms | 280 ms | 290 ms |
| wall time | 124 ms | 289 ms | 297 ms | 371 ms | 272 ms | 281 ms | 290 ms |
| peak memory | 6.12 MiB | 5.73 MiB | 5.92 MiB | 5.88 MiB | 1.97 MiB | 1.95 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 45 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 43 | 43 | 43 | 43 | 43 | 43 | 40 |
| CPU time | 434 ms | 449 ms | 473 ms | 491 ms | 426 ms | 409 ms | 522 ms |
| wall time | 1.10 s | 3.29 s | 3.24 s | 3.16 s | 614 ms | 622 ms | 527 ms |
| peak memory | 5.34 MiB | 5.08 MiB | 5.22 MiB | 5.56 MiB | 1.25 MiB | 1.25 MiB | 2.59 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 654 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 160 | 160 | 20 | 20 | 20 |
| CPU time | 84.5 ms | 90.6 ms | 89.9 ms | 91.8 ms | 57.7 ms | 58.3 ms | 53.5 ms |
| wall time | 84.4 ms | 90.2 ms | 90.3 ms | 91.6 ms | 58.2 ms | 59.2 ms | 54.1 ms |
| peak memory | 5.73 MiB | 5.80 MiB | 5.80 MiB | 6.22 MiB | 1.30 MiB | 1.31 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 69 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 80 | 160 | 80 | 20 | 0 | 20 |
| CPU time | 103 ms | 113 ms | 106 ms | 111 ms | 35.4 ms | – | 28.5 ms |
| wall time | 103 ms | 115 ms | 106 ms | 114 ms | 35.8 ms | – | 29.0 ms |
| peak memory | 5.14 MiB | 5.05 MiB | 5.00 MiB | 5.45 MiB | 1.27 MiB | – | 1.31 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 0 B | 296 KiB |

Its rounds took 58 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp wasn't measured to the end: c++ couldn't build /Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp: /Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp:15:14: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |              ^~~~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp:15:34: error: no member named 'from_chars' in namespace 'std'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                  ^~~~~~~~~~
1 warning and 1 error generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 4 s | 5.23 ms against 4.74 ms: +550 µs (+280 µs to +844 µs) | 5.25 ms against 5.05 ms: +268 µs (-5.13 µs to +581 µs) | 4.96 MiB against 4.64 MiB: +352 KiB (+336 KiB to +352 KiB) |
| add | 160, 160 in 2 s | 3.35 ms against 3.04 ms: +284 µs (+241 µs to +326 µs) | 3.29 ms against 3.24 ms: +54.0 µs (+6.50 µs to +97.0 µs) | 5.12 MiB against 4.78 MiB: +352 KiB (+352 KiB to +368 KiB) |
| hypotenuse | 160, 160 in 2 s | 3.41 ms against 3.08 ms: +311 µs (+251 µs to +368 µs) | 3.34 ms against 3.27 ms: +46.2 µs (-17.1 µs to +102 µs) | 5.13 MiB against 4.78 MiB: +352 KiB (+352 KiB to +368 KiB) |
| upcase | 160, 160 in 3 s | 4.86 ms against 4.35 ms: +473 µs (+341 µs to +645 µs) | 4.86 ms against 4.58 ms: +167 µs (+15.6 µs to +336 µs) | 6.25 MiB against 5.91 MiB: +352 KiB (+352 KiB to +368 KiB) |
| upcase-stdin | 160, 160 in 6 s | 13.8 ms against 13.7 ms: +315 µs (-22.0 µs to +634 µs) | 13.7 ms against 13.7 ms: +50.5 µs (-282 µs to +362 µs) | 5.16 MiB against 4.80 MiB: +368 KiB (+368 KiB to +368 KiB) |
| shout | 160, 160 in 4 s | 6.95 ms against 5.98 ms: +728 µs (+418 µs to +1.08 ms) | 7.05 ms against 6.34 ms: +392 µs (+76.6 µs to +761 µs) | 6.12 MiB against 5.77 MiB: +368 KiB (+368 KiB to +384 KiB) |
| wc | 160, 160 in 11 s | 27.7 ms against 27.1 ms: +495 µs (+104 µs to +876 µs) | 27.2 ms against 27.0 ms: +135 µs (-254 µs to +502 µs) | 5.23 MiB against 4.88 MiB: +384 KiB (+368 KiB to +384 KiB) |
| histogram | 160, 160 in 16 s | 41.9 ms against 41.4 ms: +661 µs (-680 µs to +1.97 ms) | 41.6 ms against 41.5 ms: +360 µs (-967 µs to +1.68 ms) | 5.16 MiB against 4.81 MiB: +352 KiB (+352 KiB to +352 KiB) |
| crc32c | 160, 160 in 14 s | 39.6 ms against 39.0 ms: +324 µs (-293 µs to +917 µs) | 39.3 ms against 39.0 ms: +22.4 µs (-574 µs to +605 µs) | 5.17 MiB against 4.81 MiB: +368 KiB (+368 KiB to +384 KiB) |
| sort | 110, 110 in 60 s | 255 ms against 257 ms: +427 µs (-14.1 ms to +15.3 ms) | 255 ms against 262 ms: +183 µs (-14.8 ms to +14.7 ms) | 74.8 MiB against 74.4 MiB: +400 KiB (+400 KiB to +400 KiB) |
| tally | 160, 160 in 49 s | 143 ms against 145 ms: -238 µs (-5.98 ms to +5.33 ms) | 142 ms against 145 ms: -589 µs (-6.56 ms to +5.15 ms) | 6.14 MiB against 5.73 MiB: +400 KiB (+400 KiB to +416 KiB) |
| server | 20, 20 in 93 s | 593 ms against 545 ms: +12.6 ms (-63.9 ms to +91.0 ms) | 1.82 s against 1.94 s: -255 ms (-721 ms to +200 ms) | 5.34 MiB against 4.97 MiB: +384 KiB (+368 KiB to +400 KiB) |
| files | 160, 160 in 27 s | 79.0 ms against 78.1 ms: +206 µs (-1.40 ms to +1.96 ms) | 78.4 ms against 77.4 ms: -151 µs (-1.74 ms to +1.54 ms) | 5.73 MiB against 5.36 MiB: +384 KiB (+384 KiB to +400 KiB) |
| readings | 160, 160 in 35 s | 102 ms against 101 ms: +83.0 µs (-1.64 ms to +1.85 ms) | 102 ms against 101 ms: -120 µs (-1.88 ms to +1.68 ms) | 5.14 MiB against 4.78 MiB: +368 KiB (+368 KiB to +368 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
