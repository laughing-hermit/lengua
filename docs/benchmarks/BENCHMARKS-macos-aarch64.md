# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 43813aef065cae8d91368d4891f6823f48ad9d82, in https://github.com/laughing-hermit/lengua/actions/runs/43145761976. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records lengua's idea and the module, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (4.75 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ✓ ✓ | ≈ ≈ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✓ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ✓ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✓ ✓ | ≈ ✓ ✓ ✓ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 empty cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 352 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 415 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 shout cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 416 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 tally c-wasm memory: lengua is level now, and KNOWN_BEHIND line 472 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 tally rust-wasm memory: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 server c cpu: lengua is level now, and KNOWN_BEHIND line 479 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- macos-aarch64 server cpp cpu: lengua is level now, and KNOWN_BEHIND line 483 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 2.67× | 2.11× | 2.43× | 295.78× |
| add | 2.23× | 2.02× | 2.49× | 147.81× |
| hypotenuse | 1.94× | 1.76× | 2.47× | 148.21× |
| upcase | 1.94× | 1.92× | 1.90× | 147.69× |
| upcase-stdin | 1.58× | 1.52× | 2.49× | 148.83× |
| shout | 1.99× | 1.79× | 1.85× | 148.45× |
| wc | 1.80× | 1.78× | 2.45× | 148.47× |
| histogram | 1.22× | 1.22× | 2.41× | 148.46× |
| crc32c | 2.39× | 2.34× | 2.41× | 148.46× |
| sort | 1.11× | 1.11× | 0.95× | 147.19× |
| tally | 0.43× | 0.43× | 2.12× | 146.79× |
| server | 1.04× | 2.61× | 2.73× | 146.44× |
| files | 1.72× | 1.71× | 2.90× | 147.27× |
| readings | 3.74× | 3.68× | 2.52× | 147.81× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 0.99×, p 0.009 | level, 0.99×, p 0.047 | level, 0.99× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 0.99×, p 0.040 | level, 0.99×, p 0.073 | level, 0.99× | ahead, 0.89× |
| empty | rust-wasm | 320 | ahead, 0.91×, p 6.9e-7 | ahead, 0.92×, p 5.9e-5 | ahead, 0.91× | ahead, 0.01× |
| empty | c | 20 | behind, 1.88×, p 2.9e-6 | behind, 1.40×, p 1.7e-4 | behind, 2.43× | behind, 295.78× |
| empty | cpp | 20 | behind, 2.67×, p 2.3e-6 | behind, 2.11×, p 1.9e-5 | behind, 2.43× | behind, 295.78× |
| empty | rust | 40 | behind, 1.29×, p 9.1e-6 | behind, 1.28×, p 3.2e-4 | behind, 2.34× | behind, 17.42× |
| add | c-wasm | 160 | level, 0.95×, p 6.4e-6 | level, 0.95×, p 7.0e-5 | ahead, 0.94× | ahead, 0.04× |
| add | cpp-wasm | 80 | ahead, 0.90×, p 6.8e-5 | ahead, 0.89×, p 1.7e-4 | ahead, 0.94× | ahead, 0.04× |
| add | rust-wasm | 80 | ahead, 0.88×, p 1.1e-6 | ahead, 0.87×, p 3.5e-6 | ahead, 0.90× | ahead, 0.02× |
| add | c | 20 | behind, 2.23×, p 1.0e-6 | behind, 2.02×, p 2.9e-6 | behind, 2.49× | behind, 147.81× |
| add | cpp | 20 | behind, 2.15×, p 2.4e-6 | behind, 1.94×, p 4.3e-6 | behind, 2.49× | behind, 147.32× |
| add | rust | 20 | behind, 1.99×, p 9.0e-7 | behind, 1.83×, p 7.9e-7 | behind, 2.31× | behind, 16.46× |
| hypotenuse | c-wasm | 1280 | level, 0.97×, p 4.9e-5 | level, 0.98×, p 0.001 | ahead, 0.95× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 1280 | level, 0.96×, p 4.9e-5 | level, 0.97×, p 9.3e-4 | ahead, 0.95× | ahead, 0.05× |
| hypotenuse | rust-wasm | 320 | ahead, 0.90×, p 1.8e-6 | ahead, 0.92×, p 4.9e-5 | ahead, 0.90× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 1.94×, p 2.9e-5 | behind, 1.76×, p 7.2e-5 | behind, 2.47× | behind, 148.21× |
| hypotenuse | cpp | 20 | behind, 1.85×, p 3.8e-6 | behind, 1.73×, p 5.5e-6 | behind, 2.47× | behind, 148.00× |
| hypotenuse | rust | 20 | behind, 1.84×, p 4.0e-7 | behind, 1.70×, p 6.0e-7 | behind, 2.35× | behind, 16.46× |
| upcase | c-wasm | 1280 | level, 0.98×, p 6.7e-6 | level, 0.99×, p 4.6e-4 | level, 0.95× | ahead, 0.06× |
| upcase | cpp-wasm | 640 | level, 0.96×, p 2.6e-5 | level, 0.96×, p 5.0e-4 | level, 0.95× | ahead, 0.06× |
| upcase | rust-wasm | 320 | ahead, 0.91×, p 1.1e-9 | ahead, 0.91×, p 3.1e-7 | ahead, 0.90× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.94×, p 3.3e-5 | behind, 1.86×, p 9.0e-5 | behind, 1.90× | behind, 147.69× |
| upcase | cpp | 40 | behind, 1.51×, p 1.4e-7 | behind, 1.61×, p 1.6e-5 | behind, 1.88× | behind, 147.69× |
| upcase | rust | 20 | behind, 1.94×, p 4.1e-5 | behind, 1.92×, p 1.9e-5 | behind, 1.81× | behind, 16.47× |
| upcase-stdin | c-wasm | 160 | ahead, 0.94×, p 3.3e-8 | ahead, 0.94×, p 2.0e-7 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 80 | ahead, 0.89×, p 2.9e-5 | ahead, 0.89×, p 5.2e-5 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 160 | level, 0.96×, p 5.3e-4 | level, 0.96×, p 0.001 | ahead, 0.93× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.44×, p 6.0e-7 | behind, 1.41×, p 7.9e-7 | behind, 2.49× | behind, 148.83× |
| upcase-stdin | cpp | 20 | behind, 1.58×, p 2.0e-7 | behind, 1.52×, p 4.0e-7 | behind, 2.46× | behind, 148.83× |
| upcase-stdin | rust | 20 | behind, 1.41×, p 1.7e-6 | behind, 1.39×, p 2.6e-6 | behind, 2.27× | behind, 16.47× |
| shout | c-wasm | 2000 | level, 1.01×, p 0.003 | level, 1.02×, p 7.3e-5 | level, 0.99× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.01×, p 0.015 | level, 1.01×, p 9.6e-4 | level, 0.99× | ahead, 0.40× |
| shout | rust-wasm | 40 | ahead, 0.93×, p 9.7e-6 | ahead, 0.94×, p 1.4e-4 | ahead, 0.93× | ahead, 0.03× |
| shout | c | 20 | behind, 1.99×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 1.84× | behind, 148.45× |
| shout | cpp | 20 | behind, 1.90×, p 8.3e-8 | behind, 1.76×, p 1.5e-7 | behind, 1.85× | behind, 148.45× |
| shout | rust | 20 | behind, 1.69×, p 6.0e-7 | behind, 1.59×, p 6.0e-7 | behind, 1.75× | behind, 16.47× |
| wc | c-wasm | 40 | behind, 1.21×, p 3.1e-8 | behind, 1.20×, p 4.5e-8 | level, 0.99× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | level, 0.99× | ahead, 0.11× |
| wc | rust-wasm | 20 | behind, 1.24×, p 4.1e-5 | behind, 1.24×, p 6.5e-5 | ahead, 0.92× | ahead, 0.04× |
| wc | c | 20 | behind, 1.71×, p 3.4e-8 | behind, 1.69×, p 3.4e-8 | behind, 2.45× | behind, 148.47× |
| wc | cpp | 20 | ahead, 0.18×, p 3.4e-8 | ahead, 0.18×, p 3.4e-8 | behind, 2.45× | behind, 148.47× |
| wc | rust | 20 | behind, 1.80×, p 3.4e-8 | behind, 1.78×, p 3.4e-8 | behind, 2.26× | behind, 16.47× |
| histogram | c-wasm | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | level, 0.97× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | level, 0.97× | ahead, 0.09× |
| histogram | rust-wasm | 40 | ahead, 0.93×, p 2.6e-5 | ahead, 0.93×, p 4.6e-5 | ahead, 0.90× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.15×, p 2.6e-7 | behind, 1.15×, p 3.0e-7 | behind, 2.41× | behind, 148.46× |
| histogram | cpp | 20 | behind, 1.17×, p 4.6e-7 | behind, 1.17×, p 4.6e-7 | behind, 2.41× | behind, 148.42× |
| histogram | rust | 20 | behind, 1.22×, p 9.6e-8 | behind, 1.22×, p 9.6e-8 | behind, 2.23× | behind, 16.46× |
| crc32c | c-wasm | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | level, 0.97× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.65×, p 3.4e-8 | level, 0.97× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.92× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.33×, p 3.4e-8 | behind, 2.30×, p 3.4e-8 | behind, 2.41× | behind, 148.46× |
| crc32c | cpp | 20 | behind, 2.39×, p 3.4e-8 | behind, 2.34×, p 3.4e-8 | behind, 2.41× | behind, 148.46× |
| crc32c | rust | 20 | behind, 2.33×, p 3.4e-8 | behind, 2.29×, p 3.4e-8 | behind, 2.25× | behind, 16.47× |
| sort | c-wasm | 20 | ahead, 0.16×, p 3.4e-8 | ahead, 0.16×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.60×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.67×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | level, 0.95× | behind, 147.19× |
| sort | cpp | 20 | ahead, 0.76×, p 3.9e-8 | ahead, 0.76×, p 5.3e-8 | ahead, 0.79× | behind, 142.81× |
| sort | rust | 20 | behind, 1.11×, p 1.4e-4 | behind, 1.11×, p 3.8e-4 | ahead, 0.93× | behind, 16.48× |
| tally | c-wasm | 20 | ahead, 0.41×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | level, 1.03× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.41×, p 3.4e-8 | ahead, 0.41×, p 3.4e-8 | level, 1.01× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.31×, p 3.4e-8 | ahead, 0.31×, p 3.4e-8 | behind, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 2.10× | behind, 146.79× |
| tally | cpp | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 2.12× | behind, 142.23× |
| tally | rust | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.96× | behind, 15.62× |
| server | c-wasm | 41 | level, 0.96×, p 0.199 | ahead, 0.52×, p 1.1e-10 | level, 0.99× | ahead, 0.12× |
| server | cpp-wasm | 41 | level, 0.91×, p 0.035 | ahead, 0.47×, p 3.8e-12 | level, 0.96× | ahead, 0.06× |
| server | rust-wasm | 41 | level, 0.96×, p 0.112 | ahead, 0.48×, p 6.9e-11 | ahead, 0.91× | ahead, 0.03× |
| server | c | 41 | level, 0.99×, p 0.125 | behind, 2.04×, p 4.8e-11 | behind, 2.73× | behind, 146.44× |
| server | cpp | 41 | level, 1.04×, p 0.041 | behind, 2.13×, p 8.3e-12 | behind, 2.73× | behind, 141.58× |
| server | rust | 41 | level, 0.83×, p 0.007 | behind, 2.61×, p 2.2e-13 | behind, 1.31× | behind, 15.57× |
| files | c-wasm | 160 | level, 0.96×, p 1.9e-5 | level, 0.95×, p 2.4e-5 | level, 0.96× | ahead, 0.05× |
| files | cpp-wasm | 80 | level, 0.95×, p 3.0e-4 | level, 0.95×, p 3.3e-4 | level, 0.96× | ahead, 0.05× |
| files | rust-wasm | 80 | level, 0.95×, p 3.9e-4 | level, 0.95×, p 4.8e-4 | ahead, 0.91× | ahead, 0.03× |
| files | c | 20 | behind, 1.60×, p 3.4e-8 | behind, 1.60×, p 3.4e-8 | behind, 2.90× | behind, 147.27× |
| files | cpp | 20 | behind, 1.68×, p 7.2e-8 | behind, 1.67×, p 7.2e-8 | behind, 2.90× | behind, 147.27× |
| files | rust | 20 | behind, 1.72×, p 7.2e-8 | behind, 1.71×, p 7.2e-8 | behind, 2.57× | behind, 16.47× |
| readings | c-wasm | 20 | ahead, 0.92×, p 5.6e-4 | ahead, 0.92×, p 6.7e-4 | ahead, 0.94× | ahead, 0.04× |
| readings | cpp-wasm | 160 | level, 0.95×, p 7.4e-9 | ahead, 0.95×, p 2.6e-8 | ahead, 0.94× | ahead, 0.04× |
| readings | rust-wasm | 160 | level, 0.96×, p 8.7e-7 | level, 0.96×, p 2.3e-6 | ahead, 0.89× | ahead, 0.02× |
| readings | c | 20 | behind, 2.96×, p 3.4e-8 | behind, 2.93×, p 3.4e-8 | behind, 2.52× | behind, 147.81× |
| readings | cpp | 20 | behind, 3.23×, p 3.4e-8 | behind, 3.20×, p 3.4e-8 | behind, 2.49× | behind, 146.45× |
| readings | rust | 20 | behind, 3.74×, p 3.4e-8 | behind, 3.68×, p 3.4e-8 | behind, 2.40× | behind, 16.46× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 40 |
| CPU time | 4.07 ms | 4.12 ms | 4.10 ms | 4.96 ms | 2.47 ms | 1.74 ms | 3.26 ms |
| wall time | 4.73 ms | 4.78 ms | 4.76 ms | 5.78 ms | 3.80 ms | 2.52 ms | 4.38 ms |
| peak memory | 3.00 MiB | 3.02 MiB | 3.02 MiB | 3.28 MiB | 1.23 MiB | 1.23 MiB | 1.28 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 78 seconds.

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
| counted runs | 160 | 160 | 80 | 80 | 20 | 20 | 20 |
| CPU time | 3.69 ms | 3.88 ms | 4.38 ms | 4.46 ms | 1.91 ms | 1.98 ms | 2.15 ms |
| wall time | 4.16 ms | 4.35 ms | 4.96 ms | 5.10 ms | 2.39 ms | 2.48 ms | 2.64 ms |
| peak memory | 3.11 MiB | 3.31 MiB | 3.31 MiB | 3.45 MiB | 1.25 MiB | 1.25 MiB | 1.34 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 33.0 KiB | 295 KiB |

Its rounds took 6 seconds.

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
| counted runs | 1280 | 1280 | 1280 | 320 | 20 | 20 | 20 |
| CPU time | 3.90 ms | 4.04 ms | 4.07 ms | 4.65 ms | 2.13 ms | 2.23 ms | 2.24 ms |
| wall time | 4.42 ms | 4.51 ms | 4.56 ms | 5.18 ms | 2.68 ms | 2.73 ms | 2.77 ms |
| peak memory | 3.12 MiB | 3.30 MiB | 3.30 MiB | 3.45 MiB | 1.27 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 46 seconds.

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
| counted runs | 1280 | 1280 | 640 | 320 | 20 | 40 | 20 |
| CPU time | 4.83 ms | 4.92 ms | 5.20 ms | 5.77 ms | 3.64 ms | 3.85 ms | 3.65 ms |
| wall time | 5.35 ms | 5.43 ms | 5.74 ms | 6.43 ms | 4.45 ms | 4.52 ms | 4.31 ms |
| peak memory | 4.23 MiB | 4.44 MiB | 4.44 MiB | 4.69 MiB | 2.23 MiB | 2.25 MiB | 2.34 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 43 seconds.

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
| counted runs | 160 | 160 | 80 | 160 | 20 | 20 | 20 |
| CPU time | 13.3 ms | 14.1 ms | 16.4 ms | 13.8 ms | 11.7 ms | 10.7 ms | 12.0 ms |
| wall time | 13.8 ms | 14.6 ms | 17.0 ms | 14.4 ms | 12.4 ms | 11.5 ms | 12.6 ms |
| peak memory | 3.19 MiB | 3.19 MiB | 3.19 MiB | 3.44 MiB | 1.28 MiB | 1.30 MiB | 1.41 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 16 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 40 | 20 | 20 | 20 |
| CPU time | 3.60 ms | 3.55 ms | 3.57 ms | 4.25 ms | 1.96 ms | 2.06 ms | 2.31 ms |
| wall time | 4.06 ms | 4.00 ms | 4.01 ms | 4.73 ms | 2.49 ms | 2.52 ms | 2.79 ms |
| peak memory | 4.11 MiB | 4.14 MiB | 4.14 MiB | 4.41 MiB | 2.23 MiB | 2.22 MiB | 2.34 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 54 seconds.

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
| CPU time | 25.5 ms | 21.2 ms | 186 ms | 21.1 ms | 15.3 ms | 147 ms | 14.5 ms |
| wall time | 26.0 ms | 21.7 ms | 186 ms | 21.7 ms | 15.9 ms | 147 ms | 15.1 ms |
| peak memory | 3.25 MiB | 3.30 MiB | 3.30 MiB | 3.55 MiB | 1.33 MiB | 1.33 MiB | 1.44 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 12 seconds.

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
| counted runs | 40 | 20 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 32.8 ms | 52.5 ms | 51.9 ms | 35.1 ms | 28.8 ms | 28.3 ms | 27.2 ms |
| wall time | 33.3 ms | 53.0 ms | 52.3 ms | 35.7 ms | 29.3 ms | 28.8 ms | 27.6 ms |
| peak memory | 3.20 MiB | 3.30 MiB | 3.30 MiB | 3.55 MiB | 1.33 MiB | 1.33 MiB | 1.44 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 9 seconds.

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
| CPU time | 37.6 ms | 59.1 ms | 58.8 ms | 62.4 ms | 16.1 ms | 15.8 ms | 16.1 ms |
| wall time | 38.4 ms | 60.2 ms | 59.4 ms | 64.6 ms | 16.7 ms | 16.4 ms | 16.8 ms |
| peak memory | 3.20 MiB | 3.30 MiB | 3.30 MiB | 3.47 MiB | 1.33 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 7 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 200 ms | 1.26 s | 334 ms | 298 ms | 311 ms | 263 ms | 180 ms |
| wall time | 201 ms | 1.26 s | 334 ms | 299 ms | 313 ms | 265 ms | 182 ms |
| peak memory | 72.8 MiB | 72.8 MiB | 80.9 MiB | 81.0 MiB | 76.3 MiB | 92.4 MiB | 78.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 69 seconds.

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
| CPU time | 109 ms | 262 ms | 268 ms | 346 ms | 254 ms | 254 ms | 270 ms |
| wall time | 111 ms | 264 ms | 271 ms | 353 ms | 258 ms | 257 ms | 272 ms |
| peak memory | 4.14 MiB | 4.03 MiB | 4.11 MiB | 3.94 MiB | 1.97 MiB | 1.95 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 42 seconds.

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
| counted runs | 41 | 41 | 41 | 41 | 41 | 41 | 41 |
| CPU time | 486 ms | 508 ms | 534 ms | 506 ms | 491 ms | 467 ms | 587 ms |
| wall time | 1.63 s | 3.15 s | 3.51 s | 3.40 s | 802 ms | 767 ms | 626 ms |
| peak memory | 3.41 MiB | 3.45 MiB | 3.56 MiB | 3.75 MiB | 1.25 MiB | 1.25 MiB | 2.61 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 656 seconds.

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
| counted runs | 160 | 160 | 80 | 80 | 20 | 20 | 20 |
| CPU time | 80.4 ms | 84.1 ms | 81.9 ms | 82.1 ms | 51.2 ms | 48.9 ms | 47.6 ms |
| wall time | 80.9 ms | 84.7 ms | 82.4 ms | 82.6 ms | 51.7 ms | 49.5 ms | 48.2 ms |
| peak memory | 3.81 MiB | 3.98 MiB | 3.98 MiB | 4.20 MiB | 1.31 MiB | 1.31 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 48 seconds.

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
| counted runs | 160 | 20 | 160 | 160 | 20 | 20 | 20 |
| CPU time | 102 ms | 117 ms | 108 ms | 107 ms | 36.3 ms | 33.3 ms | 28.8 ms |
| wall time | 103 ms | 117 ms | 109 ms | 107 ms | 36.9 ms | 33.8 ms | 29.4 ms |
| peak memory | 3.16 MiB | 3.34 MiB | 3.34 MiB | 3.53 MiB | 1.25 MiB | 1.27 MiB | 1.31 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 33.2 KiB | 296 KiB |

Its rounds took 59 seconds.

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
| empty | 160, 160 in 9 s | 4.19 ms against 3.90 ms: +188 µs (+13.0 µs to +383 µs) | 4.86 ms against 4.53 ms: +222 µs (+4.71 µs to +451 µs) | 3.02 MiB against 2.94 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |
| add | 160, 160 in 11 s | 4.54 ms against 4.74 ms: +71.0 µs (-167 µs to +323 µs) | 5.65 ms against 5.62 ms: +105 µs (-254 µs to +486 µs) | 3.11 MiB against 3.02 MiB: +80.0 KiB (+80.0 KiB to +96.0 KiB) |
| hypotenuse | 160, 160 in 8 s | 3.95 ms against 3.75 ms: +257 µs (+116 µs to +404 µs) | 4.45 ms against 4.20 ms: +285 µs (+122 µs to +456 µs) | 3.12 MiB against 3.03 MiB: +96.0 KiB (+80.0 KiB to +96.0 KiB) |
| upcase | 160, 160 in 9 s | 4.69 ms against 4.29 ms: +406 µs (+321 µs to +503 µs) | 5.21 ms against 4.78 ms: +449 µs (+352 µs to +555 µs) | 4.23 MiB against 4.12 MiB: +112 KiB (+112 KiB to +128 KiB) |
| upcase-stdin | 160, 160 in 16 s | 13.1 ms against 12.9 ms: +221 µs (+2.00 µs to +443 µs) | 13.7 ms against 13.4 ms: +269 µs (+20.3 µs to +515 µs) | 3.19 MiB against 3.11 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |
| shout | 160, 160 in 6 s | 3.25 ms against 3.02 ms: +219 µs (+138 µs to +301 µs) | 3.64 ms against 3.38 ms: +240 µs (+143 µs to +338 µs) | 4.11 MiB against 4.03 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |
| wc | 160, 160 in 52 s | 26.8 ms against 26.1 ms: +413 µs (-117 µs to +966 µs) | 27.4 ms against 26.6 ms: +422 µs (-121 µs to +1.02 ms) | 3.25 MiB against 3.12 MiB: +112 KiB (+112 KiB to +112 KiB) |
| histogram | 160, 160 in 40 s | 34.1 ms against 33.4 ms: +645 µs (+149 µs to +1.15 ms) | 34.8 ms against 34.0 ms: +695 µs (+186 µs to +1.23 ms) | 3.19 MiB against 3.11 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |
| crc32c | 160, 160 in 44 s | 36.1 ms against 35.6 ms: +426 µs (-14.0 µs to +913 µs) | 36.6 ms against 36.0 ms: +444 µs (-29.5 µs to +959 µs) | 3.20 MiB against 3.12 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |
| sort | 25, 25 in 62 s | 211 ms against 209 ms: +1.80 ms (-8.66 ms to +13.4 ms) | 211 ms against 210 ms: +1.88 ms (-8.65 ms to +13.7 ms) | 72.8 MiB against 72.6 MiB: +128 KiB (+112 KiB to +128 KiB) |
| tally | 45, 45 in 61 s | 130 ms against 131 ms: +485 µs (-9.80 ms to +9.83 ms) | 131 ms against 131 ms: +324 µs (-10.0 ms to +9.58 ms) | 4.14 MiB against 4.00 MiB: +128 KiB (+128 KiB to +144 KiB) |
| server | 20, 20 in 288 s | 545 ms against 482 ms: +39.4 ms (-23.9 ms to +107 ms) | 2.33 s against 1.89 s: +280 ms (-167 ms to +807 ms) | 3.39 MiB against 3.31 MiB: +80.0 KiB (+80.0 KiB to +96.0 KiB) |
| files | 120, 120 in 60 s | 86.0 ms against 83.7 ms: +2.09 ms (-1.28 ms to +5.74 ms) | 86.6 ms against 84.2 ms: +2.08 ms (-1.31 ms to +5.83 ms) | 3.81 MiB against 3.66 MiB: +144 KiB (+144 KiB to +160 KiB) |
| readings | 96, 96 in 60 s | 110 ms against 112 ms: -322 µs (-3.73 ms to +2.96 ms) | 111 ms against 113 ms: -351 µs (-3.87 ms to +3.09 ms) | 3.16 MiB against 3.08 MiB: +80.0 KiB (+80.0 KiB to +80.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 3.82 ms, 1.10× | 4.34 ms, 1.12× | 2.94 MiB, 1.03× |
| empty | cpp-wasm | 160 | 4.07 ms, 1.03× | 4.64 ms, 1.05× | 2.94 MiB, 1.03× |
| empty | rust-wasm | 160 | 4.03 ms, 1.04× | 4.58 ms, 1.06× | 3.16 MiB, 0.96× |
| add | c-wasm | 160 | 4.49 ms, 1.01× | 5.75 ms, 0.98× | 3.22 MiB, 0.97× |
| add | cpp-wasm | 160 | 5.19 ms, 0.87× | 6.15 ms, 0.92× | 3.22 MiB, 0.97× |
| add | rust-wasm | 160 | 4.90 ms, 0.93× | 6.26 ms, 0.90× | 3.33 MiB, 0.93× |
| hypotenuse | c-wasm | 160 | 3.87 ms, 1.02× | 4.33 ms, 1.03× | 3.20 MiB, 0.98× |
| hypotenuse | cpp-wasm | 160 | 3.77 ms, 1.05× | 4.26 ms, 1.04× | 3.20 MiB, 0.98× |
| hypotenuse | rust-wasm | 160 | 4.00 ms, 0.99× | 4.51 ms, 0.99× | 3.33 MiB, 0.94× |
| upcase | c-wasm | 160 | 4.47 ms, 1.05× | 4.96 ms, 1.05× | 4.31 MiB, 0.98× |
| upcase | cpp-wasm | 160 | 4.46 ms, 1.05× | 4.95 ms, 1.05× | 4.31 MiB, 0.98× |
| upcase | rust-wasm | 160 | 4.58 ms, 1.02× | 5.09 ms, 1.02× | 4.55 MiB, 0.93× |
| upcase-stdin | c-wasm | 160 | 14.0 ms, 0.94× | 14.6 ms, 0.93× | 3.12 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 15.0 ms, 0.88× | 15.6 ms, 0.88× | 3.12 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 13.1 ms, 1.00× | 13.7 ms, 1.00× | 3.31 MiB, 0.96× |
| shout | c-wasm | 160 | 3.05 ms, 1.07× | 3.43 ms, 1.06× | 4.06 MiB, 1.01× |
| shout | cpp-wasm | 160 | 3.06 ms, 1.06× | 3.44 ms, 1.06× | 4.06 MiB, 1.01× |
| shout | rust-wasm | 160 | 3.16 ms, 1.03× | 3.55 ms, 1.02× | 4.28 MiB, 0.96× |
| wc | c-wasm | 160 | 22.1 ms, 1.22× | 22.6 ms, 1.21× | 3.19 MiB, 1.02× |
| wc | cpp-wasm | 160 | 184 ms, 0.15× | 185 ms, 0.15× | 3.19 MiB, 1.02× |
| wc | rust-wasm | 160 | 22.1 ms, 1.22× | 22.7 ms, 1.21× | 3.42 MiB, 0.95× |
| histogram | c-wasm | 160 | 53.4 ms, 0.64× | 54.0 ms, 0.64× | 3.19 MiB, 1.00× |
| histogram | cpp-wasm | 160 | 53.5 ms, 0.64× | 54.1 ms, 0.64× | 3.19 MiB, 1.00× |
| histogram | rust-wasm | 160 | 35.3 ms, 0.97× | 35.9 ms, 0.97× | 3.42 MiB, 0.93× |
| crc32c | c-wasm | 160 | 55.4 ms, 0.65× | 55.9 ms, 0.65× | 3.19 MiB, 1.00× |
| crc32c | cpp-wasm | 160 | 55.4 ms, 0.65× | 55.9 ms, 0.65× | 3.19 MiB, 1.00× |
| crc32c | rust-wasm | 160 | 58.3 ms, 0.62× | 58.8 ms, 0.62× | 3.35 MiB, 0.96× |
| sort | c-wasm | 25 | 1.33 s, 0.16× | 1.33 s, 0.16× | 72.7 MiB, 1.00× |
| sort | cpp-wasm | 25 | 349 ms, 0.60× | 350 ms, 0.60× | 80.8 MiB, 0.90× |
| sort | rust-wasm | 25 | 301 ms, 0.70× | 302 ms, 0.70× | 80.8 MiB, 0.90× |
| tally | c-wasm | 45 | 308 ms, 0.42× | 309 ms, 0.42× | 3.92 MiB, 1.06× |
| tally | cpp-wasm | 45 | 304 ms, 0.43× | 305 ms, 0.43× | 4.00 MiB, 1.04× |
| tally | rust-wasm | 45 | 400 ms, 0.32× | 401 ms, 0.33× | 3.80 MiB, 1.09× |
| server | c-wasm | 20 | 544 ms, 1.00× | 3.11 s, 0.75× | 3.34 MiB, 1.01× |
| server | cpp-wasm | 20 | 550 ms, 0.99× | 3.27 s, 0.71× | 3.44 MiB, 0.99× |
| server | rust-wasm | 20 | 505 ms, 1.08× | 3.40 s, 0.69× | 3.61 MiB, 0.94× |
| files | c-wasm | 120 | 89.5 ms, 0.96× | 90.0 ms, 0.96× | 3.88 MiB, 0.98× |
| files | cpp-wasm | 120 | 88.1 ms, 0.98× | 88.5 ms, 0.98× | 3.88 MiB, 0.98× |
| files | rust-wasm | 120 | 90.5 ms, 0.95× | 91.1 ms, 0.95× | 4.08 MiB, 0.93× |
| readings | c-wasm | 96 | 117 ms, 0.94× | 119 ms, 0.93× | 3.25 MiB, 0.97× |
| readings | cpp-wasm | 96 | 117 ms, 0.94× | 118 ms, 0.94× | 3.25 MiB, 0.97× |
| readings | rust-wasm | 96 | 116 ms, 0.95× | 117 ms, 0.95× | 3.39 MiB, 0.93× |
