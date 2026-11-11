# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit cd54e79fc07fe745f5fcad008bd2a7da4a18ae9e, in https://github.com/laughing-hermit/lengua/actions/runs/42964232679. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (16.9 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ✗ ✓ | ✗ ≈ ✗ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✗ ≈ ≈ ✓ | ✗ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ≈ ✗ ✓ | ✗ ≈ ✗ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ≈ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 empty c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 418 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 empty c-wasm wall: lengua is level now, and KNOWN_BEHIND line 419 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 empty cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 422 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 add c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 436 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 add c-wasm memory: lengua is level now, and KNOWN_BEHIND line 437 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 add cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 438 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 add cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 439 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 hypotenuse c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 452 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 hypotenuse c-wasm memory: lengua is level now, and KNOWN_BEHIND line 453 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 hypotenuse cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 454 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 hypotenuse cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 455 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 upcase c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase c-wasm memory: lengua is level now, and KNOWN_BEHIND line 468 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 upcase cpp-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 469 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 upcase-stdin rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 486 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout c-wasm wall: lengua is level now, and KNOWN_BEHIND line 500 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 503 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 wc c-wasm memory: lengua is level now, and KNOWN_BEHIND line 519 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 wc cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 520 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 histogram c-wasm memory: lengua is level now, and KNOWN_BEHIND line 533 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 histogram cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 534 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 crc32c c-wasm memory: lengua is level now, and KNOWN_BEHIND line 547 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 crc32c cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 548 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 sort c-wasm memory: lengua is level now, and KNOWN_BEHIND line 561 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 tally cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 568 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 tally rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 569 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 576 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server c-wasm memory: lengua is level now, and KNOWN_BEHIND line 577 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 578 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 579 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 580 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 readings c-wasm memory: lengua is level now, and KNOWN_BEHIND line 604 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 readings cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 605 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.26× | 2.20× | 3.98× | 1049.36× |
| add | 3.21× | 2.46× | 4.05× | 524.33× |
| hypotenuse | 3.57× | 2.84× | 4.06× | 525.72× |
| upcase | 2.10× | 1.68× | 2.81× | 523.86× |
| upcase-stdin | 1.64× | 1.52× | 3.98× | 527.97× |
| shout | 2.38× | 1.91× | 2.76× | 526.58× |
| wc | 1.90× | 1.77× | 4.02× | 526.60× |
| histogram | 1.31× | 1.27× | 3.86× | 526.59× |
| crc32c | 2.77× | 2.62× | 3.91× | 526.60× |
| sort | 1.11× | 1.10× | 0.98× | 521.94× |
| tally | 0.54× | 0.54× | 3.19× | 520.48× |
| server | 1.36× | 3.16× | 4.32× | 519.42× |
| files | 1.70× | 1.67× | 4.42× | 522.36× |
| readings | 3.78× | 3.66× | 4.12× | 524.33× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.05×, p 1.1e-11 | level, 1.00×, p 0.008 | behind, 1.06× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | behind, 1.07×, p 4.2e-14 | level, 1.02×, p 0.001 | behind, 1.06× | ahead, 0.89× |
| empty | rust-wasm | 640 | ahead, 0.93×, p 6.6e-5 | ahead, 0.93×, p 1.3e-6 | level, 0.98× | ahead, 0.01× |
| empty | c | 20 | behind, 2.08×, p 5.2e-7 | behind, 1.55×, p 3.7e-5 | behind, 3.98× | behind, 1049.36× |
| empty | cpp | 20 | behind, 3.26×, p 3.9e-8 | behind, 2.20×, p 8.3e-8 | behind, 3.98× | behind, 1049.36× |
| empty | rust | 20 | behind, 1.85×, p 7.1e-6 | behind, 1.67×, p 8.0e-5 | behind, 3.79× | behind, 61.81× |
| add | c-wasm | 2000 | level, 1.00×, p 0.290 | level, 0.97×, p 6.2e-4 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.01×, p 0.215 | level, 0.97×, p 0.002 | level, 1.00× | ahead, 0.04× |
| add | rust-wasm | 160 | ahead, 0.87×, p 4.7e-4 | ahead, 0.83×, p 1.8e-6 | ahead, 0.94× | ahead, 0.02× |
| add | c | 20 | behind, 3.13×, p 3.4e-8 | behind, 2.37×, p 3.4e-8 | behind, 4.05× | behind, 524.33× |
| add | cpp | 20 | behind, 3.21×, p 3.4e-8 | behind, 2.46×, p 3.4e-8 | behind, 4.00× | behind, 522.60× |
| add | rust | 20 | behind, 2.76×, p 6.2e-8 | behind, 2.16×, p 2.0e-7 | behind, 3.81× | behind, 58.41× |
| hypotenuse | c-wasm | 2000 | level, 1.03×, p 2.9e-4 | level, 1.00×, p 0.024 | level, 1.01× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.04×, p 5.6e-6 | level, 0.98×, p 0.008 | level, 1.02× | ahead, 0.05× |
| hypotenuse | rust-wasm | 640 | level, 0.95×, p 3.1e-4 | ahead, 0.92×, p 1.6e-9 | ahead, 0.94× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 3.50×, p 3.4e-8 | behind, 2.78×, p 3.9e-8 | behind, 4.01× | behind, 525.72× |
| hypotenuse | cpp | 20 | behind, 3.57×, p 7.1e-8 | behind, 2.84×, p 5.2e-7 | behind, 4.06× | behind, 524.97× |
| hypotenuse | rust | 20 | behind, 3.18×, p 3.4e-8 | behind, 2.51×, p 3.4e-8 | behind, 3.78× | behind, 58.40× |
| upcase | c-wasm | 2000 | behind, 1.06×, p 2.2e-25 | level, 1.01×, p 0.028 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | behind, 1.06×, p 1.6e-24 | level, 1.01×, p 0.045 | level, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.01×, p 0.034 | level, 0.96×, p 1.6e-9 | ahead, 0.94× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.98×, p 4.6e-8 | behind, 1.49×, p 4.9e-6 | behind, 2.79× | behind, 523.86× |
| upcase | cpp | 20 | behind, 2.10×, p 1.1e-7 | behind, 1.60×, p 5.2e-7 | behind, 2.81× | behind, 523.86× |
| upcase | rust | 20 | behind, 2.10×, p 4.0e-7 | behind, 1.68×, p 3.7e-5 | behind, 2.66× | behind, 58.40× |
| upcase-stdin | c-wasm | 160 | level, 0.96×, p 2.3e-4 | ahead, 0.94×, p 2.5e-6 | behind, 1.07× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 40 | ahead, 0.92×, p 8.4e-4 | ahead, 0.90×, p 1.4e-4 | behind, 1.07× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.148 | level, 0.98×, p 1.4e-5 | level, 0.98× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.64×, p 6.2e-8 | behind, 1.52×, p 2.3e-7 | behind, 3.98× | behind, 527.97× |
| upcase-stdin | cpp | 20 | behind, 1.64×, p 6.2e-8 | behind, 1.51×, p 1.5e-7 | behind, 3.98× | behind, 527.97× |
| upcase-stdin | rust | 20 | behind, 1.51×, p 3.4e-8 | behind, 1.40×, p 8.3e-8 | behind, 3.67× | behind, 58.41× |
| shout | c-wasm | 2000 | behind, 1.05×, p 1.9e-8 | level, 1.00×, p 0.273 | behind, 1.05× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | behind, 1.07×, p 5.0e-13 | level, 1.02×, p 0.010 | behind, 1.05× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, 0.98×, p 0.106 | ahead, 0.94×, p 6.7e-8 | level, 0.98× | ahead, 0.03× |
| shout | c | 20 | behind, 2.07×, p 2.3e-6 | behind, 1.76×, p 5.2e-5 | behind, 2.76× | behind, 526.58× |
| shout | cpp | 20 | behind, 2.38×, p 1.3e-7 | behind, 1.91×, p 1.0e-6 | behind, 2.74× | behind, 526.58× |
| shout | rust | 40 | behind, 2.07×, p 7.6e-11 | behind, 1.83×, p 3.8e-8 | behind, 2.61× | behind, 58.41× |
| wc | c-wasm | 160 | behind, 1.08×, p 3.4e-5 | behind, 1.05×, p 7.1e-4 | level, 1.05× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.15×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | level, 1.05× | ahead, 0.11× |
| wc | rust-wasm | 320 | behind, 1.09×, p 5.1e-12 | behind, 1.08×, p 1.6e-9 | level, 0.97× | ahead, 0.04× |
| wc | c | 20 | behind, 1.67×, p 3.4e-8 | behind, 1.56×, p 3.4e-8 | behind, 3.98× | behind, 526.60× |
| wc | cpp | 20 | ahead, 0.19×, p 3.4e-8 | ahead, 0.18×, p 3.4e-8 | behind, 4.02× | behind, 526.60× |
| wc | rust | 20 | behind, 1.90×, p 3.4e-8 | behind, 1.77×, p 3.9e-8 | behind, 3.67× | behind, 58.41× |
| histogram | c-wasm | 20 | ahead, 0.66×, p 8.3e-8 | ahead, 0.65×, p 5.3e-8 | level, 1.03× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.65×, p 2.6e-7 | ahead, 0.64×, p 2.3e-7 | level, 1.03× | ahead, 0.09× |
| histogram | rust-wasm | 320 | ahead, 0.94×, p 5.4e-5 | ahead, 0.93×, p 6.5e-6 | ahead, 0.94× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.22×, p 6.5e-5 | behind, 1.18×, p 2.3e-4 | behind, 3.86× | behind, 526.59× |
| histogram | cpp | 40 | behind, 1.27×, p 1.3e-7 | behind, 1.23×, p 2.1e-6 | behind, 3.86× | behind, 526.47× |
| histogram | rust | 20 | behind, 1.31×, p 5.8e-5 | behind, 1.27×, p 2.1e-4 | behind, 3.61× | behind, 58.40× |
| crc32c | c-wasm | 20 | ahead, 0.69×, p 2.0e-6 | ahead, 0.68×, p 4.3e-6 | level, 1.03× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.69×, p 1.3e-6 | ahead, 0.68×, p 2.3e-6 | level, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.65×, p 3.0e-7 | ahead, 0.64×, p 3.0e-7 | level, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.77×, p 3.4e-8 | behind, 2.62×, p 3.4e-8 | behind, 3.91× | behind, 526.60× |
| crc32c | cpp | 20 | behind, 2.49×, p 3.4e-8 | behind, 2.35×, p 3.4e-8 | behind, 3.91× | behind, 526.60× |
| crc32c | rust | 20 | behind, 2.55×, p 3.4e-8 | behind, 2.42×, p 3.4e-8 | behind, 3.65× | behind, 58.41× |
| sort | c-wasm | 20 | ahead, 0.16×, p 3.4e-8 | ahead, 0.16×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.60×, p 3.9e-8 | ahead, 0.60×, p 3.9e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.76×, p 6.9e-7 | ahead, 0.76×, p 1.3e-6 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.72×, p 2.3e-7 | ahead, 0.71×, p 2.3e-7 | level, 0.98× | behind, 521.94× |
| sort | cpp | 20 | ahead, 0.78×, p 1.5e-5 | ahead, 0.77×, p 1.5e-5 | ahead, 0.81× | behind, 506.38× |
| sort | rust | 80 | behind, 1.11×, p 7.5e-5 | behind, 1.10×, p 5.3e-4 | level, 0.95× | behind, 58.42× |
| tally | c-wasm | 20 | ahead, 0.47×, p 3.4e-8 | ahead, 0.47×, p 3.4e-8 | behind, 1.07× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.47×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.04× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.37×, p 3.4e-8 | ahead, 0.37×, p 3.4e-8 | level, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.54×, p 3.4e-8 | ahead, 0.54×, p 3.4e-8 | behind, 3.13× | behind, 520.48× |
| tally | cpp | 20 | ahead, 0.52×, p 3.4e-8 | ahead, 0.51×, p 3.4e-8 | behind, 3.19× | behind, 504.32× |
| tally | rust | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | behind, 2.93× | behind, 55.38× |
| server | c-wasm | 34 | level, 1.03×, p 0.019 | ahead, 0.77×, p 6.9e-8 | level, 1.05× | ahead, 0.12× |
| server | cpp-wasm | 34 | level, 0.97×, p 0.449 | ahead, 0.76×, p 3.5e-7 | level, 1.02× | ahead, 0.06× |
| server | rust-wasm | 34 | level, 0.98×, p 0.314 | ahead, 0.79×, p 1.4e-8 | level, 0.96× | ahead, 0.03× |
| server | c | 20 | behind, 1.36×, p 9.6e-8 | behind, 3.02×, p 3.4e-8 | behind, 4.32× | behind, 519.42× |
| server | cpp | 20 | behind, 1.34×, p 6.0e-7 | behind, 3.16×, p 3.0e-7 | behind, 4.26× | behind, 502.18× |
| server | rust | 34 | level, 0.96×, p 0.237 | behind, 2.97×, p 7.0e-13 | behind, 2.04× | behind, 55.24× |
| files | c-wasm | 160 | level, 0.95×, p 1.5e-5 | ahead, 0.95×, p 7.9e-6 | level, 0.99× | ahead, 0.05× |
| files | cpp-wasm | 160 | ahead, 0.94×, p 1.3e-5 | ahead, 0.95×, p 6.6e-6 | level, 0.99× | ahead, 0.05× |
| files | rust-wasm | 160 | level, 0.95×, p 2.0e-5 | ahead, 0.95×, p 1.1e-5 | ahead, 0.92× | ahead, 0.03× |
| files | c | 20 | behind, 1.55×, p 3.4e-8 | behind, 1.53×, p 3.4e-8 | behind, 4.42× | behind, 522.36× |
| files | cpp | 20 | behind, 1.59×, p 1.2e-6 | behind, 1.57×, p 1.5e-6 | behind, 4.37× | behind, 522.36× |
| files | rust | 20 | behind, 1.70×, p 3.4e-8 | behind, 1.67×, p 3.4e-8 | behind, 3.86× | behind, 58.40× |
| readings | c-wasm | 80 | ahead, 0.93×, p 1.9e-6 | ahead, 0.93×, p 2.1e-6 | level, 1.02× | ahead, 0.04× |
| readings | cpp-wasm | 640 | level, 0.98×, p 8.0e-5 | level, 0.98×, p 2.0e-5 | level, 1.02× | ahead, 0.04× |
| readings | rust-wasm | 80 | level, 0.97×, p 8.0e-4 | level, 0.96×, p 6.0e-4 | ahead, 0.94× | ahead, 0.02× |
| readings | c | 20 | behind, 3.01×, p 3.4e-8 | behind, 2.93×, p 3.4e-8 | behind, 4.07× | behind, 524.33× |
| readings | cpp | 20 | behind, 3.13×, p 3.4e-8 | behind, 3.06×, p 3.4e-8 | behind, 4.12× | behind, 519.52× |
| readings | rust | 20 | behind, 3.78×, p 3.4e-8 | behind, 3.66×, p 3.4e-8 | behind, 3.92× | behind, 58.40× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 20 | 20 |
| CPU time | 6.75 ms | 6.46 ms | 6.32 ms | 7.21 ms | 3.58 ms | 2.29 ms | 4.01 ms |
| wall time | 7.31 ms | 7.28 ms | 7.17 ms | 7.97 ms | 5.26 ms | 3.72 ms | 4.89 ms |
| peak memory | 4.94 MiB | 4.66 MiB | 4.66 MiB | 5.02 MiB | 1.25 MiB | 1.25 MiB | 1.31 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 101 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 6.67 ms | 6.65 ms | 6.63 ms | 7.52 ms | 1.98 ms | 1.93 ms | 2.24 ms |
| wall time | 6.82 ms | 7.05 ms | 7.03 ms | 8.04 ms | 2.52 ms | 2.42 ms | 2.76 ms |
| peak memory | 5.08 MiB | 5.06 MiB | 5.06 MiB | 5.38 MiB | 1.25 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 33.0 KiB | 295 KiB |

Its rounds took 86 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 20 | 20 |
| CPU time | 6.18 ms | 6.01 ms | 5.97 ms | 6.88 ms | 2.15 ms | 2.10 ms | 2.36 ms |
| wall time | 6.31 ms | 6.34 ms | 6.40 ms | 7.35 ms | 2.76 ms | 2.70 ms | 3.06 ms |
| peak memory | 5.08 MiB | 5.03 MiB | 5.00 MiB | 5.41 MiB | 1.27 MiB | 1.25 MiB | 1.34 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 88 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 6.39 ms | 6.04 ms | 6.05 ms | 6.34 ms | 4.25 ms | 4.02 ms | 4.01 ms |
| wall time | 6.43 ms | 6.34 ms | 6.37 ms | 6.66 ms | 5.26 ms | 4.90 ms | 4.68 ms |
| peak memory | 6.28 MiB | 6.25 MiB | 6.25 MiB | 6.70 MiB | 2.25 MiB | 2.23 MiB | 2.36 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 100 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 160 | 40 | 2000 | 20 | 20 | 20 |
| CPU time | 16.9 ms | 17.4 ms | 20.1 ms | 17.0 ms | 11.4 ms | 11.3 ms | 12.3 ms |
| wall time | 16.4 ms | 17.0 ms | 19.8 ms | 16.7 ms | 12.0 ms | 12.0 ms | 13.0 ms |
| peak memory | 5.16 MiB | 4.83 MiB | 4.83 MiB | 5.25 MiB | 1.30 MiB | 1.30 MiB | 1.41 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 97 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 7.59 ms | 7.23 ms | 7.11 ms | 7.77 ms | 3.55 ms | 3.09 ms | 3.48 ms |
| wall time | 7.73 ms | 7.75 ms | 7.57 ms | 8.21 ms | 4.44 ms | 4.10 ms | 4.33 ms |
| peak memory | 6.14 MiB | 5.83 MiB | 5.84 MiB | 6.27 MiB | 2.22 MiB | 2.23 MiB | 2.34 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 123 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 160 | 20 | 320 | 20 | 20 | 20 |
| CPU time | 35.6 ms | 33.1 ms | 218 ms | 32.6 ms | 19.8 ms | 170 ms | 17.4 ms |
| wall time | 35.4 ms | 33.7 ms | 219 ms | 32.7 ms | 20.6 ms | 175 ms | 18.3 ms |
| peak memory | 5.28 MiB | 5.05 MiB | 5.05 MiB | 5.47 MiB | 1.33 MiB | 1.31 MiB | 1.44 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 46 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 20 | 20 | 320 | 20 | 40 | 20 |
| CPU time | 44.2 ms | 68.7 ms | 69.3 ms | 46.8 ms | 37.0 ms | 36.7 ms | 34.3 ms |
| wall time | 43.9 ms | 68.4 ms | 69.8 ms | 47.1 ms | 37.6 ms | 37.6 ms | 35.1 ms |
| peak memory | 5.19 MiB | 5.05 MiB | 5.05 MiB | 5.50 MiB | 1.34 MiB | 1.34 MiB | 1.44 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 43 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 48.1 ms | 69.5 ms | 69.3 ms | 74.0 ms | 17.4 ms | 19.3 ms | 18.9 ms |
| wall time | 47.2 ms | 69.3 ms | 69.5 ms | 73.8 ms | 18.0 ms | 20.1 ms | 19.5 ms |
| peak memory | 5.19 MiB | 5.05 MiB | 5.05 MiB | 5.34 MiB | 1.33 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 9 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=apple-m1

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=apple-m1 -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 20 | 20 | 20 | 20 | 80 |
| CPU time | 304 ms | 2.07 s | 552 ms | 437 ms | 461 ms | 426 ms | 275 ms |
| wall time | 308 ms | 2.08 s | 551 ms | 436 ms | 464 ms | 429 ms | 281 ms |
| peak memory | 74.8 MiB | 74.5 MiB | 82.7 MiB | 82.8 MiB | 76.3 MiB | 92.4 MiB | 78.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 145 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 167 ms | 354 ms | 353 ms | 449 ms | 309 ms | 325 ms | 349 ms |
| wall time | 168 ms | 354 ms | 353 ms | 448 ms | 310 ms | 326 ms | 350 ms |
| peak memory | 6.17 MiB | 5.75 MiB | 5.95 MiB | 5.89 MiB | 1.97 MiB | 1.94 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 56 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 34 | 34 | 34 | 34 | 20 | 20 | 34 |
| CPU time | 809 ms | 782 ms | 833 ms | 823 ms | 614 ms | 622 ms | 843 ms |
| wall time | 3.05 s | 3.96 s | 3.99 s | 3.88 s | 1.01 s | 961 ms | 1.03 s |
| peak memory | 5.33 MiB | 5.08 MiB | 5.23 MiB | 5.56 MiB | 1.23 MiB | 1.25 MiB | 2.61 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 671 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 160 | 160 | 20 | 20 | 20 |
| CPU time | 81.9 ms | 86.0 ms | 86.9 ms | 86.1 ms | 54.7 ms | 53.3 ms | 50.1 ms |
| wall time | 81.7 ms | 86.0 ms | 86.4 ms | 86.1 ms | 55.2 ms | 53.9 ms | 50.6 ms |
| peak memory | 5.73 MiB | 5.80 MiB | 5.80 MiB | 6.20 MiB | 1.30 MiB | 1.31 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 65 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 80 | 640 | 80 | 20 | 20 | 20 |
| CPU time | 108 ms | 122 ms | 111 ms | 118 ms | 37.1 ms | 35.7 ms | 29.6 ms |
| wall time | 108 ms | 122 ms | 111 ms | 118 ms | 37.9 ms | 36.3 ms | 30.3 ms |
| peak memory | 5.16 MiB | 5.08 MiB | 5.03 MiB | 5.48 MiB | 1.27 MiB | 1.25 MiB | 1.31 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 33.2 KiB | 296 KiB |

Its rounds took 173 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 4 s | 6.44 ms against 5.94 ms: +562 µs (+305 µs to +822 µs) | 6.63 ms against 6.50 ms: +203 µs (-88.5 µs to +491 µs) | 4.97 MiB against 4.59 MiB: +352 KiB (+352 KiB to +368 KiB) |
| add | 160, 160 in 4 s | 6.27 ms against 5.57 ms: +653 µs (+376 µs to +936 µs) | 6.34 ms against 5.94 ms: +321 µs (+49.9 µs to +605 µs) | 5.06 MiB against 4.72 MiB: +368 KiB (+352 KiB to +368 KiB) |
| hypotenuse | 160, 160 in 5 s | 6.73 ms against 6.31 ms: +374 µs (+81.0 µs to +663 µs) | 6.74 ms against 6.77 ms: +1.38 µs (-300 µs to +311 µs) | 5.08 MiB against 4.72 MiB: +352 KiB (+352 KiB to +368 KiB) |
| upcase | 160, 160 in 4 s | 6.11 ms against 5.47 ms: +593 µs (+438 µs to +744 µs) | 6.03 ms against 5.77 ms: +262 µs (+96.0 µs to +427 µs) | 6.28 MiB against 5.93 MiB: +352 KiB (+352 KiB to +352 KiB) |
| upcase-stdin | 160, 160 in 7 s | 16.9 ms against 16.1 ms: +721 µs (+329 µs to +1.12 ms) | 16.4 ms against 15.8 ms: +431 µs (+12.1 µs to +838 µs) | 5.17 MiB against 4.80 MiB: +368 KiB (+368 KiB to +384 KiB) |
| shout | 160, 160 in 4 s | 6.22 ms against 5.53 ms: +462 µs (+122 µs to +806 µs) | 6.26 ms against 5.83 ms: +166 µs (-167 µs to +515 µs) | 6.14 MiB against 5.77 MiB: +384 KiB (+368 KiB to +384 KiB) |
| wc | 160, 160 in 13 s | 33.4 ms against 32.7 ms: +755 µs (-93.0 µs to +1.60 ms) | 32.9 ms against 32.8 ms: +259 µs (-576 µs to +1.20 ms) | 5.28 MiB against 4.89 MiB: +384 KiB (+384 KiB to +400 KiB) |
| histogram | 160, 160 in 15 s | 41.3 ms against 41.0 ms: -81.0 µs (-1.30 ms to +1.04 ms) | 40.8 ms against 40.8 ms: -345 µs (-1.56 ms to +801 µs) | 5.19 MiB against 4.81 MiB: +384 KiB (+384 KiB to +384 KiB) |
| crc32c | 160, 160 in 17 s | 44.8 ms against 44.1 ms: +1.29 ms (+158 µs to +2.43 ms) | 44.1 ms against 44.0 ms: +939 µs (-152 µs to +2.08 ms) | 5.20 MiB against 4.81 MiB: +384 KiB (+384 KiB to +400 KiB) |
| sort | 96, 96 in 61 s | 299 ms against 300 ms: +4.31 ms (-13.1 ms to +21.1 ms) | 299 ms against 301 ms: +3.69 ms (-13.7 ms to +20.7 ms) | 74.8 MiB against 74.4 MiB: +400 KiB (+400 KiB to +416 KiB) |
| tally | 160, 160 in 58 s | 163 ms against 166 ms: -2.61 ms (-9.49 ms to +4.26 ms) | 164 ms against 167 ms: -2.98 ms (-10.5 ms to +4.29 ms) | 6.17 MiB against 5.77 MiB: +416 KiB (+416 KiB to +416 KiB) |
| server | 20, 20 in 81 s | 524 ms against 538 ms: -23.9 ms (-95.2 ms to +39.9 ms) | 2.06 s against 1.77 s: +122 ms (-346 ms to +602 ms) | 5.33 MiB against 4.95 MiB: +384 KiB (+368 KiB to +400 KiB) |
| files | 160, 160 in 28 s | 80.2 ms against 79.9 ms: +250 µs (-1.17 ms to +1.60 ms) | 79.6 ms against 79.6 ms: -65.2 µs (-1.47 ms to +1.31 ms) | 5.73 MiB against 5.36 MiB: +384 KiB (+384 KiB to +384 KiB) |
| readings | 160, 160 in 36 s | 109 ms against 106 ms: +1.12 ms (-607 µs to +2.95 ms) | 109 ms against 106 ms: +881 µs (-833 µs to +2.75 ms) | 5.16 MiB against 4.78 MiB: +368 KiB (+368 KiB to +368 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
