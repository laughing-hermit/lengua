# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 45b507bf262dab89bff38a1f9c35bdd5590c8965, in https://github.com/laughing-hermit/lengua/actions/runs/43084459708. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (4.62 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
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
| empty | ✗ ✗ ≈ ✓ | ✗ ✗ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✗ ✗ ≈ ✓ | ✗ ✗ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ≈ ✓ | ✗ ✗ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 empty c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 empty c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 empty c-wasm memory: lengua is level now, and KNOWN_BEHIND line 384 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 empty cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 empty cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 386 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 upcase c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase cpp-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 upcase-stdin c-wasm memory: lengua is level now, and KNOWN_BEHIND line 435 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 upcase-stdin cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 437 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 shout c-wasm memory: lengua is level now, and KNOWN_BEHIND line 452 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 shout cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 454 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 tally cpp-wasm memory: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 tally rust-wasm memory: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 1.92× | 1.75× | 2.44× | 287.84× |
| add | 1.97× | 1.77× | 2.46× | 143.84× |
| hypotenuse | 2.13× | 1.87× | 2.48× | 144.23× |
| upcase | 1.69× | 1.52× | 1.87× | 143.73× |
| upcase-stdin | 1.45× | 1.42× | 2.46× | 144.84× |
| shout | 2.07× | 1.89× | 1.83× | 144.46× |
| wc | 1.64× | 1.61× | 2.44× | 144.48× |
| histogram | 1.23× | 1.23× | 2.35× | 144.47× |
| crc32c | 2.40× | 2.35× | 2.42× | 144.47× |
| sort | 1.12× | 1.11× | 0.95× | 143.24× |
| tally | 0.45× | 0.45× | 2.12× | 142.86× |
| server | 1.43× | 3.23× | 2.71× | 142.51× |
| files | 1.70× | 1.69× | 2.89× | 143.32× |
| readings | 3.69× | 3.64× | 2.47× | 143.84× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 320 | behind, 1.07×, p 2.4e-4 | behind, 1.07×, p 2.5e-4 | level, 1.04× | ahead, 0.89× |
| empty | cpp-wasm | 640 | behind, 1.05×, p 1.2e-5 | behind, 1.06×, p 6.8e-6 | level, 1.04× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 0.98×, p 0.004 | level, 0.99×, p 0.014 | level, 0.97× | ahead, 0.01× |
| empty | c | 20 | behind, 1.58×, p 5.2e-5 | behind, 1.43×, p 2.5e-4 | behind, 2.44× | behind, 287.84× |
| empty | cpp | 20 | behind, 1.92×, p 5.5e-6 | behind, 1.75×, p 5.5e-6 | behind, 2.44× | behind, 287.84× |
| empty | rust | 20 | behind, 1.49×, p 4.1e-5 | behind, 1.38×, p 1.2e-4 | behind, 2.30× | behind, 16.95× |
| add | c-wasm | 2000 | level, 1.03×, p 0.012 | level, 1.03×, p 0.012 | level, 0.98× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.03×, p 0.006 | level, 1.04×, p 0.003 | level, 0.98× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, 0.97×, p 0.116 | level, 0.98×, p 0.167 | ahead, 0.95× | ahead, 0.02× |
| add | c | 20 | behind, 1.58×, p 5.8e-5 | behind, 1.51×, p 5.6e-4 | behind, 2.46× | behind, 143.84× |
| add | cpp | 40 | behind, 1.97×, p 9.4e-8 | behind, 1.77×, p 2.0e-6 | behind, 2.43× | behind, 143.37× |
| add | rust | 20 | behind, 1.77×, p 6.9e-7 | behind, 1.68×, p 2.0e-6 | behind, 2.32× | behind, 16.02× |
| hypotenuse | c-wasm | 2000 | level, 1.03×, p 3.1e-4 | level, 1.03×, p 1.1e-4 | level, 0.99× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.03×, p 5.9e-4 | level, 1.03×, p 4.9e-4 | level, 0.99× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.374 | level, 1.01×, p 0.317 | ahead, 0.95× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.03×, p 6.0e-7 | behind, 1.87×, p 3.8e-6 | behind, 2.48× | behind, 144.23× |
| hypotenuse | cpp | 20 | behind, 2.13×, p 3.0e-7 | behind, 1.86×, p 2.0e-7 | behind, 2.48× | behind, 144.02× |
| hypotenuse | rust | 20 | behind, 1.73×, p 3.3e-6 | behind, 1.58×, p 1.5e-5 | behind, 2.33× | behind, 16.02× |
| upcase | c-wasm | 320 | behind, 1.07×, p 2.3e-4 | behind, 1.09×, p 2.0e-4 | level, 0.99× | ahead, 0.06× |
| upcase | cpp-wasm | 320 | behind, 1.06×, p 2.6e-4 | behind, 1.07×, p 5.5e-4 | level, 0.99× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.01×, p 0.105 | level, 1.01×, p 0.085 | ahead, 0.93× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.65×, p 7.4e-4 | behind, 1.52×, p 7.4e-4 | behind, 1.85× | behind, 143.73× |
| upcase | cpp | 40 | behind, 1.69×, p 8.5e-8 | behind, 1.49×, p 6.6e-7 | behind, 1.87× | behind, 143.73× |
| upcase | rust | 40 | behind, 1.58×, p 5.2e-6 | behind, 1.45×, p 1.9e-5 | behind, 1.77× | behind, 16.02× |
| upcase-stdin | c-wasm | 160 | ahead, 0.94×, p 1.5e-4 | ahead, 0.94×, p 3.8e-4 | level, 1.04× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 40 | ahead, 0.88×, p 1.0e-7 | ahead, 0.89×, p 1.8e-7 | level, 1.04× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 0.99×, p 0.111 | level, 1.00×, p 0.224 | level, 0.98× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.45×, p 3.4e-8 | behind, 1.42×, p 6.2e-8 | behind, 2.46× | behind, 144.84× |
| upcase-stdin | cpp | 20 | behind, 1.33×, p 6.2e-8 | behind, 1.29×, p 1.1e-7 | behind, 2.43× | behind, 144.84× |
| upcase-stdin | rust | 20 | behind, 1.40×, p 3.5e-7 | behind, 1.38×, p 4.6e-7 | behind, 2.24× | behind, 16.02× |
| shout | c-wasm | 80 | behind, 1.10×, p 9.3e-6 | behind, 1.09×, p 2.7e-5 | level, 1.02× | ahead, 0.40× |
| shout | cpp-wasm | 40 | behind, 1.12×, p 3.1e-5 | behind, 1.10×, p 1.6e-4 | level, 1.02× | ahead, 0.40× |
| shout | rust-wasm | 640 | level, 1.04×, p 7.9e-5 | level, 1.04×, p 3.1e-4 | level, 0.97× | ahead, 0.03× |
| shout | c | 20 | behind, 2.07×, p 8.3e-8 | behind, 1.89×, p 8.3e-8 | behind, 1.83× | behind, 144.46× |
| shout | cpp | 20 | behind, 2.06×, p 3.4e-8 | behind, 1.88×, p 3.4e-8 | behind, 1.83× | behind, 144.46× |
| shout | rust | 20 | behind, 1.83×, p 2.6e-7 | behind, 1.66×, p 6.0e-7 | behind, 1.73× | behind, 16.03× |
| wc | c-wasm | 40 | behind, 1.17×, p 9.3e-7 | behind, 1.17×, p 3.4e-6 | level, 1.03× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | level, 1.03× | ahead, 0.11× |
| wc | rust-wasm | 20 | behind, 1.17×, p 3.7e-5 | behind, 1.17×, p 5.2e-5 | level, 0.96× | ahead, 0.04× |
| wc | c | 20 | behind, 1.64×, p 3.4e-8 | behind, 1.61×, p 3.4e-8 | behind, 2.44× | behind, 144.48× |
| wc | cpp | 20 | ahead, 0.17×, p 3.4e-8 | ahead, 0.17×, p 3.4e-8 | behind, 2.44× | behind, 144.48× |
| wc | rust | 20 | behind, 1.60×, p 3.4e-8 | behind, 1.58×, p 3.4e-8 | behind, 2.23× | behind, 16.03× |
| histogram | c-wasm | 20 | ahead, 0.61×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | level, 1.02× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | level, 1.02× | ahead, 0.09× |
| histogram | rust-wasm | 160 | level, 0.97×, p 2.5e-4 | level, 0.97×, p 4.7e-4 | ahead, 0.95× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.16×, p 7.2e-5 | behind, 1.15×, p 9.0e-5 | behind, 2.35× | behind, 144.47× |
| histogram | cpp | 20 | behind, 1.14×, p 3.3e-5 | behind, 1.13×, p 7.2e-5 | behind, 2.35× | behind, 144.44× |
| histogram | rust | 20 | behind, 1.23×, p 6.0e-7 | behind, 1.23×, p 6.9e-7 | behind, 2.20× | behind, 16.02× |
| crc32c | c-wasm | 20 | ahead, 0.66×, p 7.9e-7 | ahead, 0.67×, p 1.0e-6 | level, 1.02× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.66×, p 7.9e-7 | ahead, 0.66×, p 9.0e-7 | level, 1.02× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.63×, p 5.2e-7 | ahead, 0.64×, p 5.2e-7 | level, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.37×, p 3.4e-8 | behind, 2.33×, p 3.4e-8 | behind, 2.42× | behind, 144.47× |
| crc32c | cpp | 20 | behind, 2.40×, p 3.4e-8 | behind, 2.35×, p 3.4e-8 | behind, 2.39× | behind, 144.47× |
| crc32c | rust | 20 | behind, 2.20×, p 3.9e-8 | behind, 2.14×, p 7.2e-8 | behind, 2.23× | behind, 16.03× |
| sort | c-wasm | 20 | ahead, 0.16×, p 3.4e-8 | ahead, 0.16×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.61×, p 1.1e-7 | ahead, 0.60×, p 2.0e-7 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.66×, p 1.0e-6 | ahead, 0.66×, p 1.7e-6 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.66×, p 4.6e-7 | ahead, 0.65×, p 7.9e-7 | level, 0.95× | behind, 143.24× |
| sort | cpp | 20 | ahead, 0.75×, p 2.1e-5 | ahead, 0.74×, p 4.1e-5 | ahead, 0.79× | behind, 138.98× |
| sort | rust | 40 | behind, 1.12×, p 6.2e-4 | behind, 1.11×, p 9.6e-4 | ahead, 0.93× | behind, 16.03× |
| tally | c-wasm | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.07× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.06× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | behind, 1.11× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.44×, p 3.4e-8 | ahead, 0.45×, p 3.4e-8 | behind, 2.10× | behind, 142.86× |
| tally | cpp | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.45×, p 3.4e-8 | behind, 2.12× | behind, 138.42× |
| tally | rust | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.95× | behind, 15.20× |
| server | c-wasm | 34 | level, 1.08×, p 0.168 | ahead, 0.78×, p 7.4e-7 | level, 1.03× | ahead, 0.12× |
| server | cpp-wasm | 34 | level, 1.13×, p 0.200 | ahead, 0.76×, p 1.3e-8 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 34 | level, 1.03×, p 0.415 | ahead, 0.72×, p 1.9e-8 | level, 0.96× | ahead, 0.03× |
| server | c | 34 | behind, 1.33×, p 1.0e-5 | behind, 3.03×, p 3.0e-10 | behind, 2.71× | behind, 142.51× |
| server | cpp | 34 | behind, 1.43×, p 5.8e-6 | behind, 3.23×, p 4.2e-11 | behind, 2.67× | behind, 137.78× |
| server | rust | 34 | level, 1.02×, p 0.382 | behind, 3.16×, p 3.3e-11 | behind, 1.28× | behind, 15.16× |
| files | c-wasm | 80 | ahead, 0.94×, p 3.7e-5 | ahead, 0.94×, p 3.7e-5 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 160 | ahead, 0.94×, p 5.4e-5 | ahead, 0.94×, p 6.2e-5 | level, 0.99× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 0.93×, p 2.6e-4 | ahead, 0.93×, p 2.6e-4 | ahead, 0.94× | ahead, 0.03× |
| files | c | 20 | behind, 1.70×, p 5.3e-8 | behind, 1.69×, p 5.3e-8 | behind, 2.86× | behind, 143.32× |
| files | cpp | 20 | behind, 1.61×, p 9.6e-8 | behind, 1.60×, p 9.6e-8 | behind, 2.89× | behind, 143.32× |
| files | rust | 20 | behind, 1.65×, p 6.2e-8 | behind, 1.64×, p 7.2e-8 | behind, 2.53× | behind, 16.02× |
| readings | c-wasm | 80 | level, 0.96×, p 4.4e-5 | level, 0.96×, p 5.3e-5 | level, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 80 | level, 0.96×, p 6.8e-5 | level, 0.96×, p 1.1e-4 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 80 | level, 0.97×, p 7.0e-4 | level, 0.97×, p 0.001 | ahead, 0.94× | ahead, 0.02× |
| readings | c | 20 | behind, 2.97×, p 3.4e-8 | behind, 2.94×, p 3.4e-8 | behind, 2.47× | behind, 143.84× |
| readings | cpp | 20 | behind, 3.20×, p 3.4e-8 | behind, 3.17×, p 3.4e-8 | behind, 2.47× | behind, 142.53× |
| readings | rust | 20 | behind, 3.69×, p 3.4e-8 | behind, 3.64×, p 3.4e-8 | behind, 2.35× | behind, 16.02× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 320 | 640 | 2000 | 20 | 20 | 20 |
| CPU time | 4.26 ms | 4.20 ms | 3.91 ms | 4.33 ms | 2.91 ms | 2.39 ms | 3.09 ms |
| wall time | 4.93 ms | 4.82 ms | 4.52 ms | 4.99 ms | 3.60 ms | 2.95 ms | 3.73 ms |
| peak memory | 3.02 MiB | 2.91 MiB | 2.91 MiB | 3.11 MiB | 1.23 MiB | 1.23 MiB | 1.31 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 62 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 40 | 20 |
| CPU time | 4.32 ms | 4.21 ms | 4.21 ms | 4.47 ms | 2.52 ms | 1.94 ms | 2.24 ms |
| wall time | 4.97 ms | 4.83 ms | 4.78 ms | 5.07 ms | 3.15 ms | 2.56 ms | 2.81 ms |
| peak memory | 3.08 MiB | 3.12 MiB | 3.12 MiB | 3.25 MiB | 1.25 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 33.0 KiB | 295 KiB |

Its rounds took 100 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 4.56 ms | 4.42 ms | 4.43 ms | 4.54 ms | 2.47 ms | 2.35 ms | 2.91 ms |
| wall time | 5.24 ms | 5.07 ms | 5.07 ms | 5.18 ms | 3.01 ms | 3.02 ms | 3.54 ms |
| peak memory | 3.09 MiB | 3.12 MiB | 3.12 MiB | 3.27 MiB | 1.25 MiB | 1.25 MiB | 1.33 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 105 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

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
| counted runs | 2000 | 320 | 320 | 2000 | 20 | 40 | 40 |
| CPU time | 5.36 ms | 3.82 ms | 3.86 ms | 5.29 ms | 3.25 ms | 3.24 ms | 3.47 ms |
| wall time | 6.00 ms | 4.28 ms | 4.38 ms | 5.92 ms | 3.89 ms | 4.03 ms | 4.14 ms |
| peak memory | 4.17 MiB | 4.22 MiB | 4.22 MiB | 4.47 MiB | 2.25 MiB | 2.23 MiB | 2.36 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 62 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

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
| CPU time | 14.0 ms | 14.4 ms | 15.9 ms | 14.0 ms | 9.82 ms | 10.7 ms | 10.1 ms |
| wall time | 14.6 ms | 15.0 ms | 16.5 ms | 14.6 ms | 10.4 ms | 11.5 ms | 10.7 ms |
| peak memory | 3.16 MiB | 3.05 MiB | 3.05 MiB | 3.23 MiB | 1.28 MiB | 1.30 MiB | 1.41 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 92 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

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
| counted runs | 640 | 80 | 40 | 640 | 20 | 20 | 20 |
| CPU time | 3.29 ms | 2.97 ms | 2.91 ms | 3.16 ms | 1.52 ms | 1.53 ms | 1.72 ms |
| wall time | 3.73 ms | 3.37 ms | 3.32 ms | 3.59 ms | 1.87 ms | 1.88 ms | 2.13 ms |
| peak memory | 4.08 MiB | 3.98 MiB | 3.98 MiB | 4.20 MiB | 2.23 MiB | 2.23 MiB | 2.36 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 13 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

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
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 28.3 ms | 24.2 ms | 199 ms | 23.0 ms | 16.5 ms | 158 ms | 16.9 ms |
| wall time | 28.9 ms | 24.6 ms | 200 ms | 23.6 ms | 17.2 ms | 159 ms | 17.4 ms |
| peak memory | 3.20 MiB | 3.11 MiB | 3.11 MiB | 3.33 MiB | 1.31 MiB | 1.31 MiB | 1.44 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 13 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

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
| counted runs | 160 | 20 | 20 | 160 | 20 | 20 | 20 |
| CPU time | 37.9 ms | 59.7 ms | 57.5 ms | 39.1 ms | 31.3 ms | 31.9 ms | 29.3 ms |
| wall time | 38.4 ms | 60.2 ms | 58.2 ms | 39.7 ms | 32.0 ms | 32.6 ms | 29.9 ms |
| peak memory | 3.16 MiB | 3.11 MiB | 3.11 MiB | 3.33 MiB | 1.34 MiB | 1.34 MiB | 1.44 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 20 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

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
| CPU time | 39.9 ms | 60.2 ms | 60.7 ms | 62.9 ms | 16.8 ms | 16.7 ms | 18.2 ms |
| wall time | 40.6 ms | 60.8 ms | 61.2 ms | 63.5 ms | 17.4 ms | 17.2 ms | 19.0 ms |
| peak memory | 3.17 MiB | 3.11 MiB | 3.11 MiB | 3.27 MiB | 1.31 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

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
| counted runs | 40 | 20 | 20 | 20 | 20 | 20 | 40 |
| CPU time | 238 ms | 1.51 s | 398 ms | 364 ms | 368 ms | 324 ms | 214 ms |
| wall time | 239 ms | 1.51 s | 401 ms | 365 ms | 370 ms | 325 ms | 215 ms |
| peak memory | 72.7 MiB | 72.6 MiB | 80.7 MiB | 80.8 MiB | 76.3 MiB | 92.4 MiB | 78.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 93 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

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
| CPU time | 134 ms | 322 ms | 321 ms | 390 ms | 303 ms | 299 ms | 321 ms |
| wall time | 136 ms | 326 ms | 322 ms | 395 ms | 304 ms | 300 ms | 323 ms |
| peak memory | 4.11 MiB | 3.83 MiB | 3.89 MiB | 3.72 MiB | 1.95 MiB | 1.94 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 50 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

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
| counted runs | 34 | 34 | 34 | 34 | 34 | 34 | 34 |
| CPU time | 775 ms | 720 ms | 688 ms | 752 ms | 581 ms | 541 ms | 761 ms |
| wall time | 2.86 s | 3.68 s | 3.78 s | 3.96 s | 944 ms | 887 ms | 906 ms |
| peak memory | 3.34 MiB | 3.23 MiB | 3.33 MiB | 3.49 MiB | 1.23 MiB | 1.25 MiB | 2.61 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 648 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

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
| counted runs | 160 | 80 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 83.3 ms | 86.0 ms | 88.7 ms | 87.2 ms | 52.5 ms | 55.3 ms | 54.0 ms |
| wall time | 83.8 ms | 86.6 ms | 89.3 ms | 87.8 ms | 53.1 ms | 55.9 ms | 54.6 ms |
| peak memory | 3.73 MiB | 3.76 MiB | 3.77 MiB | 3.97 MiB | 1.31 MiB | 1.30 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 52 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

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
| counted runs | 80 | 80 | 80 | 80 | 20 | 20 | 20 |
| CPU time | 104 ms | 108 ms | 108 ms | 107 ms | 34.6 ms | 32.1 ms | 27.8 ms |
| wall time | 104 ms | 108 ms | 108 ms | 108 ms | 35.1 ms | 32.6 ms | 28.3 ms |
| peak memory | 3.12 MiB | 3.14 MiB | 3.14 MiB | 3.31 MiB | 1.27 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 33.2 KiB | 296 KiB |

Its rounds took 40 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 4 s | 4.08 ms against 3.88 ms: +195 µs (+69.0 µs to +319 µs) | 4.62 ms against 4.43 ms: +209 µs (+54.8 µs to +358 µs) | 3.02 MiB against 2.91 MiB: +112 KiB (+112 KiB to +112 KiB) |
| add | 160, 160 in 3 s | 4.02 ms against 3.71 ms: +288 µs (+111 µs to +455 µs) | 4.57 ms against 4.20 ms: +335 µs (+137 µs to +532 µs) | 3.08 MiB against 2.95 MiB: +128 KiB (+128 KiB to +128 KiB) |
| hypotenuse | 160, 160 in 4 s | 5.25 ms against 5.01 ms: +274 µs (-61.0 µs to +613 µs) | 6.03 ms against 5.74 ms: +342 µs (-29.9 µs to +717 µs) | 3.09 MiB against 2.97 MiB: +128 KiB (+128 KiB to +128 KiB) |
| upcase | 160, 160 in 4 s | 5.28 ms against 5.06 ms: +399 µs (+64.0 µs to +728 µs) | 5.85 ms against 5.56 ms: +403 µs (+35.0 µs to +754 µs) | 4.17 MiB against 4.03 MiB: +128 KiB (+128 KiB to +144 KiB) |
| upcase-stdin | 160, 160 in 6 s | 13.0 ms against 12.7 ms: +296 µs (+68.0 µs to +542 µs) | 13.5 ms against 13.2 ms: +320 µs (+72.6 µs to +584 µs) | 3.16 MiB against 3.03 MiB: +128 KiB (+128 KiB to +128 KiB) |
| shout | 160, 160 in 3 s | 3.56 ms against 3.34 ms: +223 µs (+113 µs to +336 µs) | 4.13 ms against 3.87 ms: +302 µs (+143 µs to +449 µs) | 4.08 MiB against 3.95 MiB: +128 KiB (+128 KiB to +128 KiB) |
| wc | 160, 160 in 12 s | 28.3 ms against 27.9 ms: +500 µs (-109 µs to +1.16 ms) | 28.9 ms against 28.6 ms: +569 µs (-83.9 µs to +1.27 ms) | 3.20 MiB against 3.05 MiB: +160 KiB (+160 KiB to +160 KiB) |
| histogram | 160, 160 in 15 s | 37.0 ms against 37.0 ms: +498 µs (-511 µs to +1.48 ms) | 38.0 ms against 37.8 ms: +471 µs (-713 µs to +1.62 ms) | 3.16 MiB against 3.03 MiB: +128 KiB (+128 KiB to +128 KiB) |
| crc32c | 160, 160 in 14 s | 36.8 ms against 36.3 ms: +414 µs (-145 µs to +1.00 ms) | 37.3 ms against 36.8 ms: +466 µs (-120 µs to +1.06 ms) | 3.17 MiB against 3.05 MiB: +128 KiB (+128 KiB to +128 KiB) |
| sort | 113, 113 in 60 s | 248 ms against 250 ms: +4.79 ms (-5.34 ms to +15.1 ms) | 249 ms against 251 ms: +5.11 ms (-5.49 ms to +15.8 ms) | 72.7 MiB against 72.5 MiB: +192 KiB (+192 KiB to +192 KiB) |
| tally | 160, 160 in 47 s | 136 ms against 132 ms: +2.65 ms (-1.34 ms to +6.69 ms) | 137 ms against 134 ms: +2.67 ms (-1.55 ms to +7.11 ms) | 4.11 MiB against 3.91 MiB: +208 KiB (+192 KiB to +208 KiB) |
| server | 20, 20 in 108 s | 767 ms against 734 ms: +34.0 ms (-49.9 ms to +106 ms) | 2.60 s against 2.71 s: -28.9 ms (-288 ms to +254 ms) | 3.34 MiB against 3.20 MiB: +144 KiB (+128 KiB to +144 KiB) |
| files | 160, 160 in 31 s | 86.3 ms against 84.9 ms: -122 µs (-2.63 ms to +2.48 ms) | 87.3 ms against 86.3 ms: -104 µs (-2.78 ms to +2.66 ms) | 3.73 MiB against 3.59 MiB: +144 KiB (+144 KiB to +160 KiB) |
| readings | 160, 160 in 41 s | 119 ms against 119 ms: -67.0 µs (-2.60 ms to +2.36 ms) | 120 ms against 120 ms: -11.6 µs (-2.44 ms to +2.27 ms) | 3.12 MiB against 2.98 MiB: +144 KiB (+144 KiB to +144 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
