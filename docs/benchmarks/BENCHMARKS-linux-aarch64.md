# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit cd54e79fc07fe745f5fcad008bd2a7da4a18ae9e, in https://github.com/laughing-hermit/lengua/actions/runs/42964232679. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (17.4 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ✗ ≈ ✗ ✓ | ✗ ≈ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✓ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✓ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ≈ ✗ ✓ | ✗ ≈ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-aarch64 upcase c-wasm memory: lengua is level now, and KNOWN_BEHIND line 279 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 280 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 281 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase-stdin c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 294 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase-stdin c-wasm wall: lengua is level now, and KNOWN_BEHIND line 295 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase-stdin c-wasm memory: lengua is level now, and KNOWN_BEHIND line 296 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase-stdin cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 298 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 shout c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout c-wasm wall: lengua is level now, and KNOWN_BEHIND line 312 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 shout cpp-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 314 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 sort c-wasm memory: lengua is level now, and KNOWN_BEHIND line 363 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 sort cpp memory: lengua is level now, and KNOWN_BEHIND line 365 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 tally c-wasm memory: lengua is level now, and KNOWN_BEHIND line 371 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 tally cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 372 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 tally rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 373 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 files c-wasm memory: lengua is level now, and KNOWN_BEHIND line 391 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 files cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 392 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 6.27× | 20.51× | 9.48× | 271.02× |
| add | 6.29× | 19.23× | 6.03× | 271.04× |
| hypotenuse | 5.34× | 16.34× | 5.43× | 271.04× |
| upcase | 3.01× | 6.65× | 5.51× | 271.05× |
| upcase-stdin | 1.43× | 2.12× | 13.31× | 271.04× |
| shout | 3.84× | 9.12× | 4.63× | 271.04× |
| wc | 1.19× | 1.57× | 8.86× | 271.05× |
| histogram | 1.13× | 1.32× | 9.29× | 271.04× |
| crc32c | 3.49× | 4.03× | 9.35× | 271.04× |
| sort | 1.20× | 1.23× | 1.09× | 271.08× |
| tally | 0.60× | 0.63× | 7.45× | 271.09× |
| server | 1.36× | 1.56× | 19.33× | 271.05× |
| files | 3.04× | 3.56× | 10.49× | 271.04× |
| readings | 3.17× | 3.35× | 10.24× | 271.04× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 1280 | behind, 1.12×, p 1.6e-264 | level, 1.04×, p 1.5e-4 | behind, 1.08× | ahead, 0.89× |
| empty | cpp-wasm | 1280 | behind, 1.12×, p 2.5e-263 | level, 1.04×, p 4.5e-6 | behind, 1.08× | ahead, 0.89× |
| empty | rust-wasm | 160 | level, 0.96×, p 2.5e-12 | ahead, 0.87×, p 1.4e-7 | behind, 1.06× | ahead, 0.01× |
| empty | c | 20 | behind, 6.05×, p 3.4e-8 | behind, 19.83×, p 3.4e-8 | behind, 9.48× | behind, 271.02× |
| empty | cpp | 20 | behind, 6.27×, p 3.4e-8 | behind, 20.51×, p 3.4e-8 | behind, 9.48× | behind, 271.02× |
| empty | rust | 20 | behind, 4.81×, p 3.4e-8 | behind, 16.00×, p 3.4e-8 | behind, 5.51× | behind, 68.52× |
| add | c-wasm | 2000 | level, 0.98×, p 2.3e-41 | level, 0.99×, p 0.002 | behind, 1.08× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 0.98×, p 9.5e-27 | level, 0.98×, p 3.1e-4 | behind, 1.08× | ahead, 0.04× |
| add | rust-wasm | 1280 | ahead, 0.90×, p 2.3e-293 | level, 0.97×, p 4.5e-5 | behind, 1.05× | ahead, 0.02× |
| add | c | 20 | behind, 6.20×, p 3.4e-8 | behind, 19.23×, p 3.4e-8 | behind, 6.03× | behind, 271.04× |
| add | cpp | 20 | behind, 6.29×, p 3.4e-8 | behind, 19.21×, p 3.4e-8 | behind, 6.03× | behind, 271.04× |
| add | rust | 20 | behind, 4.93×, p 3.4e-8 | behind, 15.70×, p 3.4e-8 | behind, 5.35× | behind, 55.02× |
| hypotenuse | c-wasm | 2000 | level, 0.98×, p 4.9e-19 | level, 0.99×, p 0.001 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 0.99×, p 8.6e-8 | level, 0.99×, p 0.003 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | rust-wasm | 320 | ahead, 0.89×, p 1.5e-71 | level, 0.97×, p 5.8e-4 | behind, 1.06× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 5.33×, p 3.4e-8 | behind, 16.34×, p 3.4e-8 | behind, 5.43× | behind, 271.04× |
| hypotenuse | cpp | 20 | behind, 5.34×, p 3.4e-8 | behind, 16.34×, p 3.4e-8 | behind, 5.36× | behind, 271.04× |
| hypotenuse | rust | 20 | behind, 4.81×, p 3.4e-8 | behind, 14.71×, p 3.4e-8 | behind, 5.34× | behind, 55.02× |
| upcase | c-wasm | 640 | level, 0.98×, p 1.3e-7 | level, 0.96×, p 1.2e-7 | level, 1.04× | ahead, 0.06× |
| upcase | cpp-wasm | 640 | level, 0.98×, p 4.8e-10 | level, 0.99×, p 2.7e-5 | level, 1.04× | ahead, 0.06× |
| upcase | rust-wasm | 320 | ahead, 0.93×, p 2.5e-55 | level, 0.96×, p 1.3e-4 | level, 1.05× | ahead, 0.03× |
| upcase | c | 20 | behind, 3.01×, p 3.4e-8 | behind, 6.65×, p 3.4e-8 | behind, 5.51× | behind, 271.05× |
| upcase | cpp | 20 | behind, 2.95×, p 3.4e-8 | behind, 6.51×, p 3.4e-8 | behind, 5.51× | behind, 271.05× |
| upcase | rust | 20 | behind, 2.62×, p 3.4e-8 | behind, 5.85×, p 3.4e-8 | behind, 4.68× | behind, 55.02× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 9.0e-10 | level, 0.99×, p 0.031 | level, 1.02× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.069 | level, 0.99×, p 1.1e-5 | level, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 320 | level, 0.97×, p 9.2e-39 | level, 0.97×, p 3.5e-5 | level, 1.01× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.43×, p 3.4e-8 | behind, 2.12×, p 3.4e-8 | behind, 13.31× | behind, 271.04× |
| upcase-stdin | cpp | 20 | behind, 1.42×, p 3.4e-8 | behind, 2.10×, p 3.4e-8 | behind, 13.31× | behind, 271.04× |
| upcase-stdin | rust | 20 | behind, 1.34×, p 3.4e-8 | behind, 1.98×, p 3.4e-8 | behind, 7.87× | behind, 55.01× |
| shout | c-wasm | 2000 | behind, 1.11×, p 0.0e0 | level, 1.02×, p 0.418 | behind, 1.09× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | behind, 1.09×, p 0.0e0 | level, 1.01×, p 0.366 | behind, 1.09× | ahead, 0.40× |
| shout | rust-wasm | 160 | ahead, 0.94×, p 1.2e-30 | ahead, 0.91×, p 5.8e-5 | behind, 1.09× | ahead, 0.03× |
| shout | c | 20 | behind, 3.84×, p 3.4e-8 | behind, 9.12×, p 3.4e-8 | behind, 4.63× | behind, 271.04× |
| shout | cpp | 20 | behind, 3.78×, p 3.4e-8 | behind, 9.01×, p 3.4e-8 | behind, 4.63× | behind, 271.04× |
| shout | rust | 20 | behind, 3.24×, p 3.4e-8 | behind, 7.78×, p 3.4e-8 | behind, 3.90× | behind, 55.01× |
| wc | c-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.76×, p 5.3e-8 | level, 0.95× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.11×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | level, 0.95× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.78×, p 1.1e-7 | ahead, 0.95× | ahead, 0.04× |
| wc | c | 20 | behind, 1.19×, p 3.4e-8 | behind, 1.57×, p 3.4e-8 | behind, 8.86× | behind, 271.05× |
| wc | cpp | 20 | level, 1.02×, p 3.7e-7 | behind, 1.36×, p 3.4e-8 | behind, 8.86× | behind, 271.05× |
| wc | rust | 20 | level, 0.95×, p 7.2e-8 | behind, 1.26×, p 3.4e-8 | behind, 7.95× | behind, 55.01× |
| histogram | c-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 40 | ahead, 0.95×, p 7.2e-15 | level, 0.97×, p 2.0e-4 | level, 0.99× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.13×, p 3.4e-8 | behind, 1.32×, p 3.4e-8 | behind, 9.29× | behind, 271.04× |
| histogram | cpp | 20 | behind, 1.13×, p 3.4e-8 | behind, 1.32×, p 3.4e-8 | behind, 9.29× | behind, 271.04× |
| histogram | rust | 20 | behind, 1.11×, p 3.4e-8 | behind, 1.30×, p 3.4e-8 | behind, 8.31× | behind, 55.01× |
| crc32c | c-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.81×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.82×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.74×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | level, 1.00× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.46×, p 3.4e-8 | behind, 2.85×, p 3.4e-8 | behind, 9.35× | behind, 271.04× |
| crc32c | cpp | 20 | behind, 2.45×, p 3.4e-8 | behind, 2.83×, p 3.4e-8 | behind, 9.35× | behind, 271.04× |
| crc32c | rust | 20 | behind, 3.49×, p 3.4e-8 | behind, 4.03×, p 3.4e-8 | behind, 8.33× | behind, 55.01× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.21×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.60×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.74×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 0.96× | behind, 271.08× |
| sort | cpp | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.81×, p 3.4e-8 | level, 1.01× | behind, 270.69× |
| sort | rust | 20 | behind, 1.20×, p 3.4e-8 | behind, 1.23×, p 3.4e-8 | behind, 1.09× | behind, 55.01× |
| tally | c-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | level, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | behind, 6.98× | behind, 271.09× |
| tally | cpp | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | behind, 3.75× | behind, 271.09× |
| tally | rust | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 7.45× | behind, 55.02× |
| server | c-wasm | 20 | level, 0.98×, p 0.001 | ahead, 0.93×, p 1.0e-6 | level, 1.00× | ahead, 0.12× |
| server | cpp-wasm | 20 | level, 0.96×, p 8.9e-4 | ahead, 0.92×, p 4.3e-6 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 160 | level, 0.97×, p 1.4e-7 | ahead, 0.93×, p 1.1e-28 | level, 0.99× | ahead, 0.03× |
| server | c | 20 | behind, 1.36×, p 3.4e-8 | behind, 1.35×, p 3.4e-8 | behind, 19.33× | behind, 271.05× |
| server | cpp | 20 | behind, 1.36×, p 3.4e-8 | behind, 1.34×, p 3.4e-8 | behind, 5.38× | behind, 270.66× |
| server | rust | 20 | ahead, 0.78×, p 3.4e-8 | behind, 1.56×, p 3.4e-8 | behind, 6.93× | behind, 55.00× |
| files | c-wasm | 40 | ahead, 0.92×, p 7.2e-15 | ahead, 0.93×, p 6.2e-8 | level, 1.01× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.94×, p 3.1e-4 | level, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.91×, p 3.4e-8 | ahead, 0.93×, p 5.2e-5 | level, 0.99× | ahead, 0.03× |
| files | c | 20 | behind, 2.53×, p 3.4e-8 | behind, 2.97×, p 3.4e-8 | behind, 10.49× | behind, 271.04× |
| files | cpp | 20 | behind, 2.52×, p 3.4e-8 | behind, 2.96×, p 3.4e-8 | behind, 10.49× | behind, 271.04× |
| files | rust | 20 | behind, 3.04×, p 3.4e-8 | behind, 3.56×, p 3.4e-8 | behind, 8.88× | behind, 55.02× |
| readings | c-wasm | 20 | level, 0.97×, p 3.4e-8 | level, 0.96×, p 2.6e-6 | level, 1.01× | ahead, 0.04× |
| readings | cpp-wasm | 20 | level, 0.98×, p 3.4e-8 | level, 0.97×, p 9.0e-5 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.94×, p 9.6e-8 | level, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 3.17×, p 3.4e-8 | behind, 3.35×, p 3.4e-8 | behind, 10.24× | behind, 271.04× |
| readings | cpp | 20 | behind, 3.07×, p 3.4e-8 | behind, 3.26×, p 3.4e-8 | behind, 4.37× | behind, 271.04× |
| readings | rust | 20 | behind, 3.10×, p 3.4e-8 | behind, 3.29×, p 3.4e-8 | behind, 9.48× | behind, 55.02× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 1280 | 1280 | 160 | 20 | 20 | 20 |
| CPU time | 2.98 ms | 2.67 ms | 2.67 ms | 3.12 ms | 498 µs | 480 µs | 626 µs |
| wall time | 10.5 ms | 10.1 ms | 10.1 ms | 11.4 ms | 580 µs | 561 µs | 719 µs |
| peak memory | 8.08 MiB | 7.45 MiB | 7.45 MiB | 7.61 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 21,912 | 21,229 | 21,893 | 26,622 | 1,527 | 1,507 | 2,648 |

Its rounds took 56 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 1280 | 20 | 20 | 20 |
| CPU time | 3.05 ms | 3.12 ms | 3.11 ms | 3.40 ms | 514 µs | 506 µs | 646 µs |
| wall time | 10.5 ms | 10.7 ms | 10.7 ms | 10.8 ms | 595 µs | 595 µs | 728 µs |
| peak memory | 8.14 MiB | 7.57 MiB | 7.57 MiB | 7.75 MiB | 1.35 MiB | 1.35 MiB | 1.52 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 23,567 | 26,186 | 25,445 | 30,256 | 1,661 | 1,646 | 2,580 |

Its rounds took 107 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 20 |
| CPU time | 3.06 ms | 3.11 ms | 3.09 ms | 3.46 ms | 572 µs | 571 µs | 634 µs |
| wall time | 10.5 ms | 10.6 ms | 10.5 ms | 10.8 ms | 657 µs | 657 µs | 729 µs |
| peak memory | 8.14 MiB | 7.57 MiB | 7.57 MiB | 7.69 MiB | 1.51 MiB | 1.53 MiB | 1.53 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,087 | 25,214 | 25,240 | 30,934 | 2,035 | 1,971 | 2,650 |

Its rounds took 92 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 640 | 640 | 320 | 20 | 20 | 20 |
| CPU time | 4.22 ms | 4.29 ms | 4.30 ms | 4.56 ms | 1.40 ms | 1.43 ms | 1.60 ms |
| wall time | 11.4 ms | 11.9 ms | 11.6 ms | 12.0 ms | 1.71 ms | 1.74 ms | 1.94 ms |
| peak memory | 11.2 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 25,581 | 27,371 | 27,880 | 31,964 | 2,152 | 2,130 | 3,051 |

Its rounds took 34 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 20 |
| CPU time | 14.6 ms | 14.6 ms | 14.7 ms | 14.3 ms | 9.67 ms | 9.78 ms | 10.4 ms |
| wall time | 21.7 ms | 21.9 ms | 22.0 ms | 21.7 ms | 9.75 ms | 9.88 ms | 10.5 ms |
| peak memory | 13.0 MiB | 12.8 MiB | 12.8 MiB | 12.9 MiB | 1000 KiB | 1000 KiB | 1.65 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 161,194 | 165,567 | 165,320 | 178,905 | 22,895 | 24,417 | 46,243 |

Its rounds took 176 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 4.19 ms | 3.76 ms | 3.83 ms | 4.49 ms | 1.12 ms | 1.13 ms | 1.32 ms |
| wall time | 11.5 ms | 11.3 ms | 11.4 ms | 12.0 ms | 1.22 ms | 1.24 ms | 1.43 ms |
| peak memory | 9.14 MiB | 8.39 MiB | 8.39 MiB | 8.41 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 23,913 | 24,290 | 24,040 | 29,607 | 1,807 | 1,925 | 2,986 |

Its rounds took 104 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 21.6 ms | 30.4 ms | 188 ms | 30.3 ms | 18.2 ms | 21.1 ms | 22.7 ms |
| wall time | 28.8 ms | 37.9 ms | 195 ms | 37.1 ms | 18.3 ms | 21.2 ms | 22.8 ms |
| peak memory | 13.1 MiB | 13.7 MiB | 13.7 MiB | 13.8 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 134,432 | 150,142 | 141,689 | 158,626 | 24,685 | 26,499 | 34,150 |

Its rounds took 10 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 40.5 ms | 72.5 ms | 72.8 ms | 42.7 ms | 35.9 ms | 35.8 ms | 36.4 ms |
| wall time | 47.9 ms | 80.1 ms | 82.0 ms | 49.3 ms | 36.0 ms | 36.0 ms | 36.5 ms |
| peak memory | 13.7 MiB | 13.7 MiB | 13.7 MiB | 13.8 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 188,365 | 198,268 | 195,539 | 222,491 | 82,943 | 84,606 | 100,388 |

Its rounds took 12 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 37.9 ms | 48.2 ms | 48.4 ms | 51.5 ms | 15.4 ms | 15.5 ms | 10.9 ms |
| wall time | 44.3 ms | 54.8 ms | 54.3 ms | 57.8 ms | 15.5 ms | 15.6 ms | 11.0 ms |
| peak memory | 13.8 MiB | 13.8 MiB | 13.7 MiB | 13.8 MiB | 1.48 MiB | 1.48 MiB | 1.66 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 312,109 | 246,764 | 252,943 | 254,196 | 24,106 | 24,266 | 27,029 |

Its rounds took 7 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 271 ms | 1.33 s | 452 ms | 403 ms | 365 ms | 343 ms | 226 ms |
| wall time | 278 ms | 1.34 s | 462 ms | 411 ms | 365 ms | 343 ms | 226 ms |
| peak memory | 83.3 MiB | 83.2 MiB | 91.2 MiB | 91.2 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,274,606 | 30,374,362 | 12,265,544 | 12,111,645 | 14,513,017 | 15,493,787 | 12,427,885 |

Its rounds took 82 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 140 ms | 292 ms | 291 ms | 407 ms | 235 ms | 244 ms | 252 ms |
| wall time | 148 ms | 298 ms | 297 ms | 414 ms | 236 ms | 244 ms | 252 ms |
| peak memory | 14.7 MiB | 14.4 MiB | 14.4 MiB | 14.2 MiB | 2.11 MiB | 3.92 MiB | 1.97 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,850,077 | 8,661,847 | 8,655,455 | 8,135,064 | 8,520,830 | 8,397,815 | 11,231,456 |

Its rounds took 44 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 20 | 20 | 160 | 20 | 20 | 20 |
| CPU time | 586 ms | 596 ms | 605 ms | 602 ms | 427 ms | 428 ms | 747 ms |
| wall time | 588 ms | 628 ms | 634 ms | 634 ms | 433 ms | 436 ms | 374 ms |
| peak memory | 13.7 MiB | 13.7 MiB | 13.7 MiB | 13.8 MiB | 724 KiB | 2.54 MiB | 1.97 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,588,477 | 2,470,632 | 2,580,387 | 2,473,865 | 56,245 | 181,835 | 511,856 |

Its rounds took 282 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 44.1 ms | 47.7 ms | 47.6 ms | 48.3 ms | 17.4 ms | 17.5 ms | 14.5 ms |
| wall time | 51.5 ms | 55.1 ms | 55.4 ms | 56.3 ms | 17.6 ms | 17.6 ms | 14.6 ms |
| peak memory | 14.2 MiB | 14.1 MiB | 14.1 MiB | 14.3 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,449,305 | 7,558,682 | 7,558,433 | 7,586,154 | 9,188 | 11,344 | 9,211 |

Its rounds took 10 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 102 ms | 105 ms | 104 ms | 108 ms | 32.2 ms | 33.1 ms | 32.8 ms |
| wall time | 108 ms | 113 ms | 111 ms | 115 ms | 32.3 ms | 33.3 ms | 33.0 ms |
| peak memory | 13.8 MiB | 13.8 MiB | 13.8 MiB | 13.9 MiB | 1.35 MiB | 3.16 MiB | 1.46 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 24,582 | 27,472 | 27,063 | 31,907 | 1,698 | 13,457 | 2,693 |

Its rounds took 13 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 5 s | 2.99 ms against 2.81 ms: +174 µs (+133 µs to +215 µs) | 10.8 ms against 10.5 ms: +82.7 µs (-401 µs to +673 µs) | 8.08 MiB against 7.45 MiB: +640 KiB (+640 KiB to +644 KiB) |
| add | 160, 160 in 5 s | 3.00 ms against 2.84 ms: +166 µs (+140 µs to +192 µs) | 10.5 ms against 10.5 ms: -50.9 µs (-559 µs to +437 µs) | 8.14 MiB against 7.52 MiB: +632 KiB (+580 KiB to +640 KiB) |
| hypotenuse | 160, 160 in 5 s | 3.15 ms against 2.91 ms: +228 µs (+196 µs to +259 µs) | 10.5 ms against 10.4 ms: +145 µs (-287 µs to +573 µs) | 8.14 MiB against 7.52 MiB: +620 KiB (+576 KiB to +640 KiB) |
| upcase | 160, 160 in 5 s | 4.20 ms against 4.00 ms: +220 µs (+165 µs to +276 µs) | 11.1 ms against 10.7 ms: +121 µs (-357 µs to +635 µs) | 11.3 MiB against 10.9 MiB: +640 KiB (+384 KiB to +896 KiB) |
| upcase-stdin | 160, 160 in 9 s | 16.0 ms against 15.9 ms: +133 µs (+37.0 µs to +231 µs) | 22.8 ms against 23.0 ms: -173 µs (-698 µs to +349 µs) | 13.0 MiB against 12.8 MiB: +192 KiB (+192 KiB to +196 KiB) |
| shout | 160, 160 in 5 s | 3.92 ms against 3.75 ms: +172 µs (+120 µs to +223 µs) | 11.1 ms against 11.4 ms: -250 µs (-798 µs to +307 µs) | 9.14 MiB against 8.39 MiB: +768 KiB (+764 KiB to +784 KiB) |
| wc | 160, 160 in 11 s | 21.5 ms against 21.3 ms: +174 µs (+90.0 µs to +263 µs) | 28.3 ms against 28.4 ms: -280 µs (-851 µs to +245 µs) | 13.0 MiB against 12.9 MiB: +136 KiB (+128 KiB to +176 KiB) |
| histogram | 160, 160 in 17 s | 40.4 ms against 40.2 ms: +164 µs (+90.0 µs to +240 µs) | 47.2 ms against 47.2 ms: -102 µs (-572 µs to +347 µs) | 13.7 MiB against 13.7 MiB: +36.0 KiB (+8.00 KiB to +60.0 KiB) |
| crc32c | 160, 160 in 17 s | 37.9 ms against 37.7 ms: +147 µs (+54.0 µs to +234 µs) | 44.6 ms against 45.0 ms: -585 µs (-1.09 ms to -68.8 µs) | 13.8 MiB against 13.7 MiB: +104 KiB (+72.0 KiB to +120 KiB) |
| sort | 110, 110 in 61 s | 263 ms against 263 ms: +275 µs (-1.35 ms to +1.89 ms) | 270 ms against 270 ms: +140 µs (-1.64 ms to +1.93 ms) | 83.3 MiB against 83.2 MiB: +100 KiB (+68.0 KiB to +128 KiB) |
| tally | 160, 160 in 49 s | 141 ms against 140 ms: +547 µs (+162 µs to +941 µs) | 148 ms against 148 ms: +172 µs (-432 µs to +822 µs) | 14.7 MiB against 14.6 MiB: +124 KiB (+80.0 KiB to +128 KiB) |
| server | 46, 46 in 61 s | 595 ms against 600 ms: -2.21 ms (-14.5 ms to +7.86 ms) | 596 ms against 603 ms: -2.82 ms (-15.7 ms to +8.40 ms) | 13.7 MiB against 13.6 MiB: +160 KiB (+124 KiB to +192 KiB) |
| files | 160, 160 in 18 s | 44.0 ms against 43.7 ms: +252 µs (+200 µs to +305 µs) | 50.6 ms against 50.6 ms: +64.2 µs (-415 µs to +518 µs) | 14.2 MiB against 14.1 MiB: +64.0 KiB (+64.0 KiB to +96.0 KiB) |
| readings | 160, 160 in 36 s | 102 ms against 101 ms: +771 µs (+743 µs to +799 µs) | 109 ms against 108 ms: +496 µs (-45.3 µs to +853 µs) | 13.8 MiB against 13.7 MiB: +76.0 KiB (+64.0 KiB to +96.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
