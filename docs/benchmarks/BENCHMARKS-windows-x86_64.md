# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 610aaf5c622249762f3d80df3fa0947c3f71739e, in https://github.com/laughing-hermit/lengua/actions/runs/43191148225. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records the module and lengua's idea, with the line that names an outside module added, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (6.32 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| add | ✓ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| wc | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ≈ ✓ ✗ ✗ | ≈ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ≈ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 upcase c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 wc c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 histogram rust cpu: lengua is level now, and KNOWN_BEHIND line 579 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort rust cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 server c wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.83× | 1.74× | 61.92× |
| add | – | 1.85× | 1.73× | 54.16× |
| hypotenuse | – | 1.81× | 1.73× | 53.49× |
| upcase | – | 1.70× | 1.55× | 51.99× |
| upcase-stdin | 1.00× | 1.54× | 1.71× | 59.37× |
| shout | – | 1.69× | 1.53× | 57.02× |
| wc | 2.00× | 1.22× | 1.71× | 54.16× |
| histogram | 1.00× | 1.17× | 1.70× | 54.39× |
| crc32c | 3.00× | 2.12× | 1.70× | 57.79× |
| sort | 1.08× | 1.02× | 0.96× | 53.95× |
| tally | 0.50× | 0.53× | 1.65× | 50.39× |
| server | 1.40× | 2.41× | 1.58× | 55.08× |
| files | 1.80× | 1.68× | 1.75× | 51.99× |
| readings | 3.00× | 3.55× | 1.73× | 52.19× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, p 0.130 | level, 0.99×, p 9.6e-18 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, p 0.179 | level, 0.99×, p 5.6e-18 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, p 0.012 | ahead, 0.95×, p 5.5e-318 | level, 1.00× | ahead, 0.01× |
| empty | c | 160 | level, p 2.9e-4 | behind, 1.83×, p 2.9e-54 | behind, 1.74× | behind, 57.78× |
| empty | cpp | 80 | level, p 4.2e-4 | behind, 1.83×, p 4.7e-28 | behind, 1.74× | behind, 57.78× |
| empty | rust | 160 | level, p 4.5e-6 | behind, 1.65×, p 1.2e-51 | behind, 1.57× | behind, 61.92× |
| add | c-wasm | 40 | ahead, 0.00×, p 0.001 | level, 0.96×, p 2.5e-7 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, p 0.027 | level, 0.96×, p 4.7e-99 | level, 1.00× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, p 0.006 | ahead, 0.94×, p 1.6e-173 | level, 0.99× | ahead, 0.02× |
| add | c | 160 | level, p 5.1e-4 | behind, 1.85×, p 2.9e-54 | behind, 1.73× | behind, 43.88× |
| add | cpp | 160 | level, p 1.3e-6 | behind, 1.84×, p 2.9e-54 | behind, 1.73× | behind, 44.03× |
| add | rust | 320 | level, p 1.0e-6 | behind, 1.66×, p 1.6e-106 | behind, 1.57× | behind, 54.16× |
| hypotenuse | c-wasm | 2000 | level, p 0.414 | level, 0.97×, p 1.6e-190 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, p 0.191 | level, 0.96×, p 2.8e-197 | level, 1.00× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, p 0.285 | ahead, 0.94×, p 0.0e0 | level, 0.98× | ahead, 0.03× |
| hypotenuse | c | 160 | level, p 2.0e-7 | behind, 1.81×, p 2.9e-54 | behind, 1.73× | behind, 43.58× |
| hypotenuse | cpp | 160 | level, p 5.3e-7 | behind, 1.80×, p 2.9e-54 | behind, 1.73× | behind, 43.44× |
| hypotenuse | rust | 160 | level, p 1.3e-5 | behind, 1.62×, p 2.9e-54 | behind, 1.57× | behind, 53.49× |
| upcase | c-wasm | 2000 | level, p 0.481 | level, 0.97×, p 7.7e-143 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, p 0.178 | level, 0.97×, p 2.3e-151 | level, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, p 0.341 | level, 0.95×, p 3.9e-319 | level, 0.98× | ahead, 0.03× |
| upcase | c | 40 | behind, p 0.001 | behind, 1.70×, p 7.2e-15 | behind, 1.52× | behind, 41.23× |
| upcase | cpp | 80 | level, p 2.7e-4 | behind, 1.70×, p 4.7e-28 | behind, 1.52× | behind, 41.23× |
| upcase | rust | 80 | level, p 7.1e-4 | behind, 1.62×, p 8.8e-27 | behind, 1.55× | behind, 51.99× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.004 | level, 0.98×, p 0.0e0 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 1280 | level, 1.00×, p 6.1e-4 | level, 0.97×, p 1.4e-309 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.004 | level, 0.97×, p 0.0e0 | level, 1.00× | ahead, 0.03× |
| upcase-stdin | c | 20 | level, 1.00×, p 8.7e-4 | behind, 1.54×, p 3.4e-8 | behind, 1.71× | behind, 55.55× |
| upcase-stdin | cpp | 40 | level, 1.00×, p 5.9e-6 | behind, 1.53×, p 7.2e-15 | behind, 1.71× | behind, 55.55× |
| upcase-stdin | rust | 40 | level, 1.00×, p 3.0e-5 | behind, 1.40×, p 7.2e-15 | behind, 1.55× | behind, 59.37× |
| shout | c-wasm | 2000 | level, p 0.377 | level, 0.99×, p 5.2e-98 | level, 1.01× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, p 0.290 | level, 0.99×, p 1.2e-86 | level, 1.01× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, p 0.155 | level, 0.95×, p 0.0e0 | level, 0.97× | ahead, 0.03× |
| shout | c | 160 | level, p 4.1e-4 | behind, 1.69×, p 2.9e-54 | behind, 1.52× | behind, 54.38× |
| shout | cpp | 80 | level, p 3.5e-4 | behind, 1.69×, p 4.7e-28 | behind, 1.52× | behind, 54.38× |
| shout | rust | 320 | level, p 5.8e-7 | behind, 1.56×, p 1.6e-106 | behind, 1.53× | behind, 57.02× |
| wc | c-wasm | 160 | level, 1.00×, p 4.9e-6 | ahead, 0.94×, p 2.6e-47 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 4.9e-9 | ahead, 0.16×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 160 | level, 1.00×, p 8.1e-6 | ahead, 0.92×, p 7.3e-48 | level, 0.98× | ahead, 0.04× |
| wc | c | 40 | behind, 2.00×, p 2.8e-5 | behind, 1.22×, p 7.2e-15 | behind, 1.71× | behind, 43.29× |
| wc | cpp | 80 | level, 1.00×, p 3.9e-4 | ahead, 0.87×, p 7.1e-27 | behind, 1.71× | behind, 43.15× |
| wc | rust | 80 | level, 1.00×, p 1.1e-5 | behind, 1.20×, p 4.7e-28 | behind, 1.54× | behind, 54.16× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 3.2e-4 | ahead, 0.81×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 2.7e-4 | ahead, 0.80×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 40 | ahead, 0.67×, p 3.3e-5 | ahead, 0.85×, p 7.2e-15 | level, 0.97× | ahead, 0.03× |
| histogram | c | 40 | level, 1.00×, p 0.001 | behind, 1.17×, p 7.2e-15 | behind, 1.70× | behind, 43.73× |
| histogram | cpp | 80 | level, 1.00×, p 1.7e-5 | behind, 1.17×, p 3.9e-26 | behind, 1.70× | behind, 43.73× |
| histogram | rust | 160 | level, 1.00×, p 6.3e-5 | behind, 1.10×, p 2.7e-47 | behind, 1.55× | behind, 54.39× |
| crc32c | c-wasm | 40 | ahead, 0.86×, p 7.7e-6 | ahead, 0.80×, p 7.2e-15 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | ahead, 0.75×, p 3.4e-6 | ahead, 0.80×, p 7.2e-15 | level, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 40 | level, 1.00×, p 1.3e-5 | ahead, 0.80×, p 7.2e-15 | level, 0.99× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.00×, p 6.3e-8 | behind, 2.12×, p 3.4e-8 | behind, 1.70× | behind, 43.73× |
| crc32c | cpp | 20 | behind, 3.00×, p 1.5e-8 | behind, 2.09×, p 3.4e-8 | behind, 1.70× | behind, 43.73× |
| crc32c | rust | 20 | behind, 3.00×, p 1.5e-7 | behind, 1.98×, p 3.4e-8 | behind, 1.54× | behind, 57.79× |
| sort | c-wasm | 20 | ahead, 0.20×, p 2.2e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.54×, p 1.2e-8 | ahead, 0.54×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.59×, p 8.7e-9 | ahead, 0.59×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.41×, p 1.4e-8 | ahead, 0.41×, p 3.4e-8 | level, 0.96× | behind, 53.95× |
| sort | cpp | 20 | ahead, 0.45×, p 1.6e-8 | ahead, 0.46×, p 3.4e-8 | ahead, 0.83× | behind, 47.61× |
| sort | rust | 160 | behind, 1.08×, p 0.001 | level, 1.02×, p 4.3e-27 | ahead, 0.89× | behind, 53.73× |
| tally | c-wasm | 20 | ahead, 0.37×, p 7.2e-9 | ahead, 0.39×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.37×, p 1.2e-8 | ahead, 0.39×, p 3.4e-8 | level, 1.01× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.30×, p 6.1e-9 | ahead, 0.32×, p 3.4e-8 | level, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.50×, p 1.3e-8 | ahead, 0.51×, p 3.4e-8 | behind, 1.61× | behind, 42.60× |
| tally | cpp | 20 | ahead, 0.50×, p 8.0e-9 | ahead, 0.53×, p 3.4e-8 | behind, 1.63× | behind, 41.11× |
| tally | rust | 20 | ahead, 0.44×, p 7.1e-9 | ahead, 0.46×, p 3.4e-8 | behind, 1.65× | behind, 50.39× |
| server | c-wasm | 98 | level, 1.00×, p 0.017 | ahead, 0.79×, p 5.8e-34 | level, 1.01× | ahead, 0.12× |
| server | cpp-wasm | 98 | level, 1.00×, p 0.381 | ahead, 0.80×, p 5.8e-34 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 98 | level, 1.00×, p 0.067 | ahead, 0.80×, p 5.8e-34 | level, 0.98× | ahead, 0.03× |
| server | c | 20 | behind, 1.07×, p 3.3e-8 | behind, 1.06×, p 3.4e-8 | behind, 1.58× | behind, 55.08× |
| server | cpp | 20 | behind, 1.07×, p 3.1e-8 | behind, 1.06×, p 3.4e-8 | behind, 1.58× | behind, 53.49× |
| server | rust | 20 | behind, 1.40×, p 3.0e-8 | behind, 2.41×, p 3.4e-8 | behind, 1.20× | behind, 45.74× |
| files | c-wasm | 40 | ahead, 0.90×, p 1.9e-5 | ahead, 0.94×, p 5.5e-7 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 40 | ahead, 0.90×, p 2.0e-6 | ahead, 0.94×, p 5.9e-8 | level, 0.99× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.90×, p 2.3e-5 | ahead, 0.91×, p 3.3e-5 | level, 0.98× | ahead, 0.03× |
| files | c | 20 | behind, 1.50×, p 9.3e-9 | behind, 1.58×, p 3.4e-8 | behind, 1.75× | behind, 41.62× |
| files | cpp | 20 | behind, 1.50×, p 1.3e-8 | behind, 1.56×, p 3.4e-8 | behind, 1.75× | behind, 41.62× |
| files | rust | 20 | behind, 1.80×, p 9.7e-9 | behind, 1.68×, p 3.4e-8 | behind, 1.61× | behind, 51.99× |
| readings | c-wasm | 40 | level, 1.00×, p 7.6e-5 | level, 0.95×, p 9.4e-14 | level, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 40 | level, 1.00×, p 3.8e-4 | level, 0.96×, p 1.2e-13 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 160 | level, 1.00×, p 6.4e-7 | level, 0.96×, p 3.1e-51 | level, 0.98× | ahead, 0.02× |
| readings | c | 20 | behind, 3.00×, p 6.5e-9 | behind, 3.55×, p 3.4e-8 | behind, 1.73× | behind, 43.88× |
| readings | cpp | 20 | behind, 3.00×, p 8.1e-9 | behind, 3.36×, p 3.4e-8 | behind, 1.73× | behind, 44.03× |
| readings | rust | 20 | behind, 3.00×, p 1.3e-8 | behind, 2.42×, p 3.4e-8 | behind, 1.56× | behind, 52.19× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 160 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 7.10 ms | 7.17 ms | 7.16 ms | 7.49 ms | 3.94 ms | 3.94 ms | 4.38 ms |
| peak memory | 5.86 MiB | 5.85 MiB | 5.85 MiB | 5.88 MiB | 3.37 MiB | 3.37 MiB | 3.73 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 155 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 40 | 2000 | 2000 | 160 | 160 | 320 |
| CPU time | 0.00 ns | 7.81 ms | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 6.97 ms | 7.44 ms | 7.23 ms | 7.37 ms | 3.87 ms | 3.87 ms | 4.29 ms |
| peak memory | 5.91 MiB | 5.91 MiB | 5.91 MiB | 5.99 MiB | 3.41 MiB | 3.41 MiB | 3.77 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 147 KiB | 120 KiB |

Its rounds took 121 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 160 | 160 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 6.63 ms | 6.87 ms | 6.88 ms | 7.04 ms | 3.59 ms | 3.59 ms | 4.00 ms |
| peak memory | 5.91 MiB | 5.90 MiB | 5.89 MiB | 6.01 MiB | 3.41 MiB | 3.41 MiB | 3.77 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 149 KiB | 121 KiB |

Its rounds took 149 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 80 | 80 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 7.65 ms | 7.86 ms | 7.87 ms | 8.04 ms | 4.48 ms | 4.48 ms | 4.71 ms |
| peak memory | 6.85 MiB | 6.84 MiB | 6.84 MiB | 6.98 MiB | 4.50 MiB | 4.50 MiB | 4.41 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 157 KiB | 157 KiB | 124 KiB |

Its rounds took 156 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 1280 | 2000 | 20 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 23.0 ms | 23.6 ms | 23.7 ms | 23.7 ms | 15.1 ms | 15.1 ms | 16.5 ms |
| peak memory | 5.89 MiB | 5.89 MiB | 5.89 MiB | 5.90 MiB | 3.44 MiB | 3.44 MiB | 3.81 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 109 KiB |

Its rounds took 255 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 320 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 6.72 ms | 6.79 ms | 6.79 ms | 7.04 ms | 3.99 ms | 4.00 ms | 4.32 ms |
| peak memory | 6.66 MiB | 6.62 MiB | 6.62 MiB | 6.89 MiB | 4.39 MiB | 4.39 MiB | 4.36 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 143 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 20 | 160 | 40 | 80 | 80 |
| CPU time | 31.2 ms | 31.2 ms | 188 ms | 31.2 ms | 15.6 ms | 31.2 ms | 31.2 ms |
| wall time | 30.2 ms | 32.3 ms | 191 ms | 32.7 ms | 24.8 ms | 34.7 ms | 25.2 ms |
| peak memory | 5.91 MiB | 5.91 MiB | 5.92 MiB | 6.04 MiB | 3.47 MiB | 3.47 MiB | 3.84 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 150 KiB | 150 KiB | 120 KiB |

Its rounds took 34 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 20 | 20 | 40 | 40 | 80 | 160 |
| CPU time | 31.2 ms | 46.9 ms | 46.9 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 39.4 ms | 48.7 ms | 48.9 ms | 46.2 ms | 33.5 ms | 33.6 ms | 35.9 ms |
| peak memory | 5.90 MiB | 5.91 MiB | 5.92 MiB | 6.07 MiB | 3.47 MiB | 3.47 MiB | 3.81 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 119 KiB |

Its rounds took 27 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 40 | 40 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 54.7 ms | 62.5 ms | 46.9 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 44.5 ms | 55.9 ms | 55.9 ms | 55.9 ms | 21.0 ms | 21.3 ms | 22.5 ms |
| peak memory | 5.89 MiB | 5.91 MiB | 5.91 MiB | 5.95 MiB | 3.47 MiB | 3.47 MiB | 3.84 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 13 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 20 | 20 | 20 | 20 | 20 | 160 |
| CPU time | 219 ms | 1.06 s | 391 ms | 359 ms | 516 ms | 469 ms | 203 ms |
| wall time | 214 ms | 1.07 s | 392 ms | 362 ms | 521 ms | 464 ms | 209 ms |
| peak memory | 75.2 MiB | 75.2 MiB | 83.3 MiB | 83.5 MiB | 78.4 MiB | 90.4 MiB | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 136 KiB | 120 KiB |

Its rounds took 140 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 109 ms | 297 ms | 297 ms | 359 ms | 219 ms | 219 ms | 250 ms |
| wall time | 116 ms | 295 ms | 297 ms | 360 ms | 226 ms | 217 ms | 250 ms |
| peak memory | 6.60 MiB | 6.50 MiB | 6.54 MiB | 6.43 MiB | 4.10 MiB | 4.05 MiB | 4.00 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 158 KiB | 128 KiB |

Its rounds took 43 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 98 | 98 | 98 | 98 | 20 | 20 | 20 |
| CPU time | 1.06 s | 1.06 s | 1.06 s | 1.06 s | 1.00 s | 1.00 s | 766 ms |
| wall time | 1.07 s | 1.35 s | 1.35 s | 1.33 s | 1.01 s | 1.01 s | 445 ms |
| peak memory | 6.32 MiB | 6.28 MiB | 6.34 MiB | 6.46 MiB | 4.00 MiB | 4.00 MiB | 5.27 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 121 KiB | 142 KiB |

Its rounds took 629 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 141 ms | 156 ms | 156 ms | 156 ms | 93.8 ms | 93.8 ms | 78.1 ms |
| wall time | 143 ms | 152 ms | 153 ms | 156 ms | 90.5 ms | 91.4 ms | 85.2 ms |
| peak memory | 6.29 MiB | 6.30 MiB | 6.34 MiB | 6.41 MiB | 3.60 MiB | 3.60 MiB | 3.91 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 32 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 40 | 40 | 160 | 20 | 20 | 20 |
| CPU time | 93.8 ms | 93.8 ms | 93.8 ms | 93.8 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 89.2 ms | 93.6 ms | 93.5 ms | 93.4 ms | 25.1 ms | 26.5 ms | 36.9 ms |
| peak memory | 5.91 MiB | 5.91 MiB | 5.91 MiB | 6.03 MiB | 3.41 MiB | 3.41 MiB | 3.79 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 147 KiB | 124 KiB |

Its rounds took 46 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 14 s | means 5.57 ms against 6.05 ms: -488 µs (-2.56 ms to +1.59 ms) | 7.05 ms against 6.61 ms: +440 µs (+400 µs to +481 µs) | 5.86 MiB against 5.74 MiB: +120 KiB (+120 KiB to +120 KiB) |
| add | 160, 160 in 12 s | means 6.45 ms against 5.08 ms: +1.37 ms (-717 µs to +3.45 ms) | 6.47 ms against 6.04 ms: +427 µs (+404 µs to +450 µs) | 5.90 MiB against 5.77 MiB: +132 KiB (+132 KiB to +132 KiB) |
| hypotenuse | 160, 160 in 13 s | means 6.15 ms against 5.86 ms: +293 µs (-1.81 ms to +2.39 ms) | 6.54 ms against 6.10 ms: +440 µs (+414 µs to +466 µs) | 5.91 MiB against 5.77 MiB: +136 KiB (+136 KiB to +136 KiB) |
| upcase | 160, 160 in 13 s | means 7.71 ms against 6.35 ms: +1.37 ms (-909 µs to +3.64 ms) | 7.55 ms against 6.95 ms: +594 µs (+569 µs to +620 µs) | 6.85 MiB against 6.73 MiB: +120 KiB (+116 KiB to +120 KiB) |
| upcase-stdin | 160, 160 in 26 s | means 23.4 ms against 22.8 ms: +684 µs (-1.68 ms to +3.05 ms) | 23.1 ms against 22.7 ms: +440 µs (+379 µs to +498 µs) | 5.89 MiB against 5.77 MiB: +124 KiB (+120 KiB to +124 KiB) |
| shout | 160, 160 in 13 s | means 7.03 ms against 5.86 ms: +1.17 ms (-815 µs to +3.16 ms) | 6.75 ms against 6.35 ms: +401 µs (+377 µs to +423 µs) | 6.66 MiB against 6.55 MiB: +120 KiB (+116 KiB to +120 KiB) |
| wc | 160, 160 in 59 s | means 28.8 ms against 30.2 ms: -1.37 ms (-3.08 ms to +343 µs) | 30.3 ms against 29.8 ms: +489 µs (+395 µs to +576 µs) | 5.92 MiB against 5.79 MiB: +132 KiB (+128 KiB to +132 KiB) |
| histogram | 160, 160 in 44 s | means 38.9 ms against 39.0 ms: -97.7 µs (-2.30 ms to +2.11 ms) | 39.4 ms against 38.9 ms: +471 µs (+380 µs to +554 µs) | 5.89 MiB against 5.77 MiB: +124 KiB (+124 KiB to +128 KiB) |
| crc32c | 160, 160 in 49 s | means 44.1 ms against 44.6 ms: -488 µs (-2.81 ms to +1.84 ms) | 44.5 ms against 44.0 ms: +493 µs (+402 µs to +576 µs) | 5.89 MiB against 5.77 MiB: +128 KiB (+128 KiB to +132 KiB) |
| sort | 26, 26 in 60 s | means 215 ms against 212 ms: +3.00 ms (-2.31 ms to +8.32 ms) | 213 ms against 212 ms: +817 µs (+63.4 µs to +1.75 ms) | 75.2 MiB against 75.1 MiB: +148 KiB (+144 KiB to +156 KiB) |
| tally | 49, 49 in 61 s | means 117 ms against 113 ms: +4.46 ms (+532 µs to +8.40 ms) | 115 ms against 115 ms: +501 µs (+99.9 µs to +956 µs) | 6.60 MiB against 6.45 MiB: +156 KiB (+156 KiB to +160 KiB) |
| server | 20, 20 in 137 s | means 1.07 s against 1.07 s: +3.12 ms (-9.98 ms to +16.2 ms) | 1.08 s against 1.08 s: +1.72 ms (-1.88 ms to +5.09 ms) | 6.29 MiB against 6.20 MiB: +120 KiB (+88.0 KiB to +132 KiB) |
| files | 75, 75 in 61 s | means 143 ms against 144 ms: -833 µs (-4.09 ms to +2.42 ms) | 143 ms against 143 ms: +450 µs (-549 µs to +1.38 ms) | 6.29 MiB against 6.18 MiB: +116 KiB (+116 KiB to +120 KiB) |
| readings | 117, 117 in 60 s | means 89.9 ms against 90.0 ms: -134 µs (-2.75 ms to +2.48 ms) | 89.3 ms against 88.9 ms: +380 µs (+267 µs to +493 µs) | 5.91 MiB against 5.79 MiB: +124 KiB (+124 KiB to +128 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 0.00 ns | 6.64 ms, 1.06× | 5.72 MiB, 1.02× |
| empty | cpp-wasm | 160 | 0.00 ns | 6.64 ms, 1.06× | 5.72 MiB, 1.02× |
| empty | rust-wasm | 160 | 0.00 ns | 6.85 ms, 1.03× | 5.74 MiB, 1.02× |
| add | c-wasm | 160 | 0.00 ns | 6.19 ms, 1.04× | 5.78 MiB, 1.02× |
| add | cpp-wasm | 160 | 0.00 ns | 6.19 ms, 1.04× | 5.77 MiB, 1.02× |
| add | rust-wasm | 160 | 0.00 ns | 6.29 ms, 1.03× | 5.77 MiB, 1.02× |
| hypotenuse | c-wasm | 160 | 0.00 ns | 6.21 ms, 1.05× | 5.77 MiB, 1.02× |
| hypotenuse | cpp-wasm | 160 | 0.00 ns | 6.23 ms, 1.05× | 5.77 MiB, 1.02× |
| hypotenuse | rust-wasm | 160 | 0.00 ns | 6.35 ms, 1.03× | 5.83 MiB, 1.01× |
| upcase | c-wasm | 160 | 0.00 ns | 7.08 ms, 1.07× | 6.70 MiB, 1.02× |
| upcase | cpp-wasm | 160 | 0.00 ns | 7.08 ms, 1.07× | 6.75 MiB, 1.02× |
| upcase | rust-wasm | 160 | 0.00 ns | 7.18 ms, 1.05× | 6.87 MiB, 1.00× |
| upcase-stdin | c-wasm | 160 | 15.6 ms, 1.00× | 23.2 ms, 1.00× | 5.77 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 15.6 ms, 1.00× | 23.3 ms, 0.99× | 5.77 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 15.6 ms, 1.00× | 23.3 ms, 0.99× | 5.78 MiB, 1.02× |
| shout | c-wasm | 160 | 0.00 ns | 6.36 ms, 1.06× | 6.51 MiB, 1.02× |
| shout | cpp-wasm | 160 | 0.00 ns | 6.36 ms, 1.06× | 6.51 MiB, 1.02× |
| shout | rust-wasm | 160 | 0.00 ns | 6.54 ms, 1.03× | 6.73 MiB, 0.99× |
| wc | c-wasm | 160 | 31.2 ms, 1.00× | 31.9 ms, 0.95× | 5.82 MiB, 1.02× |
| wc | cpp-wasm | 160 | 188 ms, 0.17× | 191 ms, 0.16× | 5.82 MiB, 1.02× |
| wc | rust-wasm | 160 | 31.2 ms, 1.00× | 32.1 ms, 0.94× | 5.87 MiB, 1.01× |
| histogram | c-wasm | 160 | 46.9 ms, 0.67× | 48.2 ms, 0.82× | 5.82 MiB, 1.01× |
| histogram | cpp-wasm | 160 | 46.9 ms, 0.67× | 48.2 ms, 0.82× | 5.82 MiB, 1.01× |
| histogram | rust-wasm | 160 | 46.9 ms, 0.67× | 45.4 ms, 0.87× | 5.78 MiB, 1.02× |
| crc32c | c-wasm | 160 | 46.9 ms, 1.00× | 55.3 ms, 0.80× | 5.82 MiB, 1.01× |
| crc32c | cpp-wasm | 160 | 46.9 ms, 1.00× | 55.5 ms, 0.80× | 5.82 MiB, 1.01× |
| crc32c | rust-wasm | 160 | 46.9 ms, 1.00× | 55.4 ms, 0.80× | 5.80 MiB, 1.02× |
| sort | c-wasm | 26 | 1.07 s, 0.20× | 1.07 s, 0.20× | 75.1 MiB, 1.00× |
| sort | cpp-wasm | 26 | 391 ms, 0.56× | 391 ms, 0.54× | 83.1 MiB, 0.90× |
| sort | rust-wasm | 26 | 359 ms, 0.61× | 362 ms, 0.59× | 83.3 MiB, 0.90× |
| tally | c-wasm | 49 | 297 ms, 0.37× | 295 ms, 0.39× | 6.32 MiB, 1.04× |
| tally | cpp-wasm | 49 | 297 ms, 0.37× | 296 ms, 0.39× | 6.30 MiB, 1.05× |
| tally | rust-wasm | 49 | 359 ms, 0.30× | 360 ms, 0.32× | 6.12 MiB, 1.08× |
| server | c-wasm | 20 | 1.06 s, 1.01× | 1.37 s, 0.78× | 6.14 MiB, 1.02× |
| server | cpp-wasm | 20 | 1.07 s, 1.00× | 1.40 s, 0.77× | 6.18 MiB, 1.02× |
| server | rust-wasm | 20 | 1.08 s, 0.99× | 1.34 s, 0.80× | 6.30 MiB, 1.00× |
| files | c-wasm | 75 | 156 ms, 0.90× | 152 ms, 0.95× | 6.13 MiB, 1.03× |
| files | cpp-wasm | 75 | 156 ms, 0.90× | 152 ms, 0.94× | 6.13 MiB, 1.03× |
| files | rust-wasm | 75 | 156 ms, 0.90× | 154 ms, 0.93× | 6.32 MiB, 1.00× |
| readings | c-wasm | 117 | 93.8 ms, 1.00× | 93.0 ms, 0.96× | 5.79 MiB, 1.02× |
| readings | cpp-wasm | 117 | 93.8 ms, 1.00× | 92.9 ms, 0.96× | 5.79 MiB, 1.02× |
| readings | rust-wasm | 117 | 93.8 ms, 1.00× | 92.9 ms, 0.96× | 5.79 MiB, 1.02× |
