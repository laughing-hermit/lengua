# lengua against C, C++ and Rust on windows-aarch64

Measured on windows-aarch64 (Cobalt 100, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 610aaf5c622249762f3d80df3fa0947c3f71739e, in https://github.com/laughing-hermit/lengua/actions/runs/43191148225. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records the module and lengua's idea, with the line that names an outside module added, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (5.16 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-aarch64 upcase-stdin c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 upcase-stdin cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 upcase-stdin rust cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc c wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc cpp wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc rust wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.43× | 1.54× | 52.03× |
| add | – | 1.43× | 1.54× | 45.53× |
| hypotenuse | – | 1.43× | 1.54× | 45.15× |
| upcase | – | 1.41× | 1.43× | 44.20× |
| upcase-stdin | 2.00× | 1.36× | 1.52× | 49.59× |
| shout | – | 1.39× | 1.43× | 47.58× |
| wc | 1.50× | 1.10× | 1.52× | 46.13× |
| histogram | 1.33× | 1.19× | 1.52× | 45.34× |
| crc32c | 4.00× | 2.10× | 1.52× | 48.02× |
| sort | 1.12× | 1.10× | 0.96× | 45.55× |
| tally | 0.65× | 0.65× | 1.50× | 42.28× |
| server | 1.18× | 1.60× | 1.43× | 46.75× |
| files | 1.83× | 1.82× | 1.55× | 44.39× |
| readings | 3.50× | 2.94× | 1.54× | 44.20× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.00×, p 0.116 | level, 0.99×, p 2.4e-8 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.00×, p 0.298 | level, 0.99×, p 9.7e-10 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 1.4e-4 | level, 0.97×, p 1.1e-119 | level, 0.99× | ahead, 0.01× |
| empty | c | 80 | behind, p 9.0e-7 | behind, 1.43×, p 4.7e-28 | behind, 1.54× | behind, 49.12× |
| empty | cpp | 80 | behind, p 3.0e-4 | behind, 1.43×, p 4.7e-28 | behind, 1.54× | behind, 49.12× |
| empty | rust | 80 | behind, p 1.2e-7 | behind, 1.36×, p 4.7e-28 | behind, 1.45× | behind, 52.03× |
| add | c-wasm | 2000 | level, 1.00×, p 0.019 | level, 0.98×, p 1.9e-116 | level, 0.99× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.00×, p 0.071 | level, 0.98×, p 9.1e-97 | level, 0.99× | ahead, 0.04× |
| add | rust-wasm | 1280 | level, 1.00×, p 3.3e-4 | level, 0.96×, p 4.7e-172 | level, 1.00× | ahead, 0.02× |
| add | c | 80 | behind, p 1.4e-4 | behind, 1.42×, p 4.7e-28 | behind, 1.54× | behind, 37.06× |
| add | cpp | 160 | behind, p 1.4e-7 | behind, 1.43×, p 2.9e-54 | behind, 1.54× | behind, 37.06× |
| add | rust | 80 | behind, p 4.1e-4 | behind, 1.35×, p 4.7e-28 | behind, 1.45× | behind, 45.53× |
| hypotenuse | c-wasm | 2000 | level, 1.00×, p 0.042 | level, 0.98×, p 2.8e-94 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.00×, p 0.048 | level, 0.98×, p 7.6e-105 | level, 1.00× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.003 | level, 0.96×, p 2.8e-245 | level, 1.00× | ahead, 0.03× |
| hypotenuse | c | 80 | behind, p 6.8e-5 | behind, 1.43×, p 4.7e-28 | behind, 1.54× | behind, 36.81× |
| hypotenuse | cpp | 20 | behind, p 6.2e-4 | behind, 1.43×, p 3.4e-8 | behind, 1.54× | behind, 36.68× |
| hypotenuse | rust | 20 | behind, p 6.2e-4 | behind, 1.36×, p 3.4e-8 | behind, 1.45× | behind, 45.15× |
| upcase | c-wasm | 320 | level, 1.00×, p 5.4e-5 | level, 0.98×, p 2.5e-16 | level, 0.99× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 1.00×, p 0.055 | level, 0.98×, p 2.1e-54 | level, 0.99× | ahead, 0.06× |
| upcase | rust-wasm | 320 | level, 1.00×, p 2.8e-4 | level, 0.96×, p 3.9e-42 | level, 0.97× | ahead, 0.03× |
| upcase | c | 160 | behind, p 9.9e-5 | behind, 1.41×, p 5.4e-53 | behind, 1.39× | behind, 34.64× |
| upcase | cpp | 80 | behind, p 9.2e-5 | behind, 1.40×, p 8.2e-27 | behind, 1.39× | behind, 34.64× |
| upcase | rust | 160 | behind, p 4.9e-4 | behind, 1.36×, p 3.1e-49 | behind, 1.43× | behind, 44.20× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.058 | level, 1.00×, p 0.091 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.304 | level, 1.00×, p 2.5e-6 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.001 | level, 0.98×, p 3.8e-85 | level, 0.99× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 5.2e-5 | behind, 1.36×, p 3.4e-8 | behind, 1.52× | behind, 47.37× |
| upcase-stdin | cpp | 20 | behind, 1.33×, p 0.001 | behind, 1.35×, p 3.4e-8 | behind, 1.52× | behind, 47.37× |
| upcase-stdin | rust | 20 | behind, 2.00×, p 3.0e-4 | behind, 1.28×, p 3.4e-8 | behind, 1.45× | behind, 49.59× |
| shout | c-wasm | 2000 | level, 1.00×, p 0.455 | level, 0.99×, p 3.6e-20 | level, 1.01× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.00×, p 0.389 | level, 0.99×, p 1.6e-18 | level, 1.01× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, 1.00×, p 0.473 | level, 0.97×, p 1.2e-189 | level, 0.99× | ahead, 0.03× |
| shout | c | 20 | behind, p 0.001 | behind, 1.39×, p 3.4e-8 | behind, 1.41× | behind, 46.13× |
| shout | cpp | 20 | behind, p 4.1e-4 | behind, 1.39×, p 3.4e-8 | behind, 1.41× | behind, 46.13× |
| shout | rust | 40 | behind, p 5.1e-5 | behind, 1.34×, p 7.2e-15 | behind, 1.43× | behind, 47.58× |
| wc | c-wasm | 40 | level, 1.00×, p 5.1e-4 | ahead, 0.85×, p 1.4e-13 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.23×, p 1.5e-8 | ahead, 0.22×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 40 | level, 1.00×, p 9.9e-5 | ahead, 0.84×, p 1.4e-13 | level, 1.00× | ahead, 0.04× |
| wc | c | 80 | behind, 1.50×, p 3.0e-5 | behind, 1.10×, p 1.5e-25 | behind, 1.52× | behind, 36.68× |
| wc | cpp | 160 | behind, 1.50×, p 7.5e-5 | behind, 1.08×, p 3.5e-53 | behind, 1.52× | behind, 36.68× |
| wc | rust | 320 | level, 1.00×, p 1.2e-4 | behind, 1.06×, p 6.4e-95 | behind, 1.45× | behind, 46.13× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 2.4e-7 | ahead, 0.68×, p 3.4e-8 | level, 0.99× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 5.5e-8 | ahead, 0.68×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 80 | level, 1.00×, p 2.8e-4 | level, 0.96×, p 2.0e-26 | level, 1.00× | ahead, 0.03× |
| histogram | c | 40 | behind, 1.33×, p 9.0e-7 | behind, 1.19×, p 7.2e-15 | behind, 1.52× | behind, 36.81× |
| histogram | cpp | 40 | behind, 1.33×, p 8.1e-5 | behind, 1.18×, p 7.2e-15 | behind, 1.51× | behind, 36.81× |
| histogram | rust | 40 | behind, 1.33×, p 1.7e-5 | behind, 1.14×, p 7.2e-15 | behind, 1.44× | behind, 45.34× |
| crc32c | c-wasm | 80 | level, 1.00×, p 3.1e-7 | ahead, 0.85×, p 4.7e-28 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | level, 1.00×, p 2.8e-4 | ahead, 0.85×, p 7.2e-15 | level, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 40 | level, 1.00×, p 9.4e-6 | ahead, 0.82×, p 7.2e-15 | level, 1.00× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.00×, p 1.5e-8 | behind, 2.10×, p 3.4e-8 | behind, 1.52× | behind, 36.94× |
| crc32c | cpp | 20 | behind, 4.00×, p 1.5e-8 | behind, 2.06×, p 3.4e-8 | behind, 1.52× | behind, 36.94× |
| crc32c | rust | 20 | behind, 2.00×, p 3.6e-8 | behind, 1.96×, p 3.4e-8 | behind, 1.45× | behind, 48.02× |
| sort | c-wasm | 20 | ahead, 0.20×, p 1.9e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 1.6e-8 | ahead, 0.57×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.66×, p 1.5e-8 | ahead, 0.64×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.42×, p 2.1e-8 | ahead, 0.42×, p 3.4e-8 | level, 0.96× | behind, 45.55× |
| sort | cpp | 20 | ahead, 0.43×, p 1.8e-8 | ahead, 0.43×, p 3.4e-8 | ahead, 0.85× | behind, 41.61× |
| sort | rust | 20 | behind, 1.12×, p 1.0e-7 | behind, 1.10×, p 3.4e-8 | ahead, 0.90× | behind, 44.78× |
| tally | c-wasm | 20 | ahead, 0.52×, p 1.5e-8 | ahead, 0.54×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.54×, p 1.4e-8 | ahead, 0.54×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.39×, p 6.5e-9 | ahead, 0.41×, p 3.4e-8 | level, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.65×, p 1.3e-8 | ahead, 0.65×, p 3.4e-8 | behind, 1.47× | behind, 35.95× |
| tally | cpp | 20 | ahead, 0.65×, p 7.9e-9 | ahead, 0.64×, p 3.4e-8 | behind, 1.48× | behind, 34.88× |
| tally | rust | 20 | ahead, 0.61×, p 9.3e-9 | ahead, 0.63×, p 3.4e-8 | behind, 1.50× | behind, 42.28× |
| server | c-wasm | 40 | level, 0.99×, p 4.2e-4 | ahead, 0.87×, p 7.2e-15 | level, 1.01× | ahead, 0.12× |
| server | cpp-wasm | 78 | level, 1.00×, p 0.299 | ahead, 0.93×, p 2.1e-27 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 78 | level, 1.00×, p 0.105 | ahead, 0.91×, p 2.1e-27 | level, 0.99× | ahead, 0.03× |
| server | c | 20 | behind, 1.17×, p 3.0e-8 | behind, 1.17×, p 3.4e-8 | behind, 1.43× | behind, 46.75× |
| server | cpp | 20 | behind, 1.18×, p 2.9e-8 | behind, 1.17×, p 3.4e-8 | behind, 1.42× | behind, 45.73× |
| server | rust | 20 | ahead, 0.90×, p 4.8e-6 | behind, 1.60×, p 3.4e-8 | behind, 1.17× | behind, 38.70× |
| files | c-wasm | 20 | ahead, 0.85×, p 2.6e-6 | ahead, 0.91×, p 4.0e-7 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.92×, p 5.0e-5 | ahead, 0.90×, p 1.5e-7 | level, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.88×, p 3.2e-5 | ahead, 0.92×, p 6.0e-7 | level, 0.99× | ahead, 0.03× |
| files | c | 20 | behind, 1.57×, p 1.2e-8 | behind, 1.73×, p 3.4e-8 | behind, 1.55× | behind, 34.87× |
| files | cpp | 20 | behind, 1.57×, p 1.8e-8 | behind, 1.72×, p 3.4e-8 | behind, 1.55× | behind, 34.87× |
| files | rust | 20 | behind, 1.83×, p 1.2e-8 | behind, 1.82×, p 3.4e-8 | behind, 1.47× | behind, 44.39× |
| readings | c-wasm | 160 | level, 1.00×, p 4.6e-4 | level, 0.97×, p 5.9e-44 | level, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 1280 | level, 1.00×, p 6.7e-9 | level, 0.99×, p 9.4e-291 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 80 | level, 1.00×, p 0.001 | level, 0.96×, p 4.3e-23 | level, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 3.50×, p 5.3e-9 | behind, 2.94×, p 3.4e-8 | behind, 1.54× | behind, 37.06× |
| readings | cpp | 20 | behind, 3.50×, p 4.0e-9 | behind, 2.89×, p 3.4e-8 | behind, 1.54× | behind, 37.06× |
| readings | rust | 20 | behind, 2.33×, p 5.1e-9 | behind, 2.24×, p 3.4e-8 | behind, 1.45× | behind, 44.20× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 80 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.9 ms | 17.0 ms | 17.1 ms | 17.5 ms | 11.8 ms | 11.8 ms | 12.4 ms |
| peak memory | 7.20 MiB | 7.19 MiB | 7.19 MiB | 7.23 MiB | 4.67 MiB | 4.67 MiB | 4.96 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 108 KiB | 108 KiB | 102 KiB |

Its rounds took 294 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 1280 | 80 | 160 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.3 ms | 16.7 ms | 16.6 ms | 16.9 ms | 11.3 ms | 11.2 ms | 11.9 ms |
| peak memory | 7.23 MiB | 7.27 MiB | 7.27 MiB | 7.23 MiB | 4.70 MiB | 4.70 MiB | 4.99 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 142 KiB | 142 KiB | 116 KiB |

Its rounds took 264 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.5 ms | 16.8 ms | 16.9 ms | 17.1 ms | 11.5 ms | 11.5 ms | 12.1 ms |
| peak memory | 7.23 MiB | 7.21 MiB | 7.21 MiB | 7.23 MiB | 4.70 MiB | 4.70 MiB | 4.99 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 144 KiB | 144 KiB | 117 KiB |

Its rounds took 289 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 320 | 2000 | 320 | 160 | 80 | 160 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 18.1 ms | 18.7 ms | 18.5 ms | 19.0 ms | 12.9 ms | 13.0 ms | 13.3 ms |
| peak memory | 8.09 MiB | 8.13 MiB | 8.13 MiB | 8.32 MiB | 5.81 MiB | 5.80 MiB | 5.66 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 152 KiB | 152 KiB | 120 KiB |

Its rounds took 183 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 23.4 ms | 15.6 ms |
| wall time | 41.0 ms | 41.1 ms | 41.2 ms | 41.8 ms | 29.6 ms | 29.9 ms | 31.4 ms |
| peak memory | 7.23 MiB | 7.22 MiB | 7.22 MiB | 7.27 MiB | 4.74 MiB | 4.74 MiB | 4.98 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 112 KiB | 112 KiB | 106 KiB |

Its rounds took 483 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.5 ms | 16.6 ms | 16.6 ms | 17.0 ms | 12.0 ms | 12.0 ms | 12.6 ms |
| peak memory | 8.00 MiB | 7.95 MiB | 7.95 MiB | 8.10 MiB | 5.67 MiB | 5.68 MiB | 5.60 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 114 KiB | 114 KiB | 111 KiB |

Its rounds took 279 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 40 | 20 | 40 | 80 | 160 | 320 |
| CPU time | 31.2 ms | 46.9 ms | 203 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 46.5 ms | 54.8 ms | 214 ms | 55.2 ms | 42.3 ms | 42.9 ms | 43.8 ms |
| peak memory | 7.24 MiB | 7.25 MiB | 7.28 MiB | 7.23 MiB | 4.77 MiB | 4.76 MiB | 4.99 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 144 KiB | 144 KiB | 114 KiB |

Its rounds took 67 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 20 | 80 | 40 | 40 | 40 |
| CPU time | 62.5 ms | 93.8 ms | 93.8 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 66.2 ms | 98.4 ms | 98.7 ms | 69.1 ms | 56.1 ms | 56.2 ms | 58.2 ms |
| peak memory | 7.24 MiB | 7.28 MiB | 7.27 MiB | 7.25 MiB | 4.78 MiB | 4.78 MiB | 5.02 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 144 KiB | 144 KiB | 116 KiB |

Its rounds took 30 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 40 | 40 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 62.5 ms | 62.5 ms | 62.5 ms | 31.2 ms | 15.6 ms | 31.2 ms |
| wall time | 62.7 ms | 73.5 ms | 74.3 ms | 76.9 ms | 29.9 ms | 30.4 ms | 31.9 ms |
| peak memory | 7.25 MiB | 7.27 MiB | 7.27 MiB | 7.29 MiB | 4.77 MiB | 4.77 MiB | 5.01 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 143 KiB | 143 KiB | 110 KiB |

Its rounds took 26 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 297 ms | 1.52 s | 516 ms | 453 ms | 703 ms | 688 ms | 266 ms |
| wall time | 298 ms | 1.52 s | 525 ms | 465 ms | 713 ms | 699 ms | 271 ms |
| peak memory | 76.6 MiB | 76.5 MiB | 84.6 MiB | 84.6 MiB | 79.7 MiB | 89.7 MiB | 84.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 116 KiB | 127 KiB | 118 KiB |

Its rounds took 109 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 172 ms | 328 ms | 320 ms | 438 ms | 266 ms | 266 ms | 281 ms |
| wall time | 180 ms | 332 ms | 331 ms | 443 ms | 277 ms | 280 ms | 284 ms |
| peak memory | 7.95 MiB | 7.78 MiB | 7.79 MiB | 7.60 MiB | 5.41 MiB | 5.36 MiB | 5.29 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 147 KiB | 152 KiB | 125 KiB |

Its rounds took 52 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 78 | 40 | 78 | 78 | 20 | 20 | 20 |
| CPU time | 1.61 s | 1.62 s | 1.61 s | 1.61 s | 1.36 s | 1.34 s | 1.77 s |
| wall time | 1.63 s | 1.87 s | 1.76 s | 1.79 s | 1.38 s | 1.38 s | 1.01 s |
| peak memory | 7.59 MiB | 7.54 MiB | 7.59 MiB | 7.70 MiB | 5.32 MiB | 5.33 MiB | 6.46 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 113 KiB | 116 KiB | 136 KiB |

Its rounds took 638 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 172 ms | 203 ms | 188 ms | 195 ms | 109 ms | 109 ms | 93.8 ms |
| wall time | 188 ms | 205 ms | 209 ms | 205 ms | 109 ms | 109 ms | 103 ms |
| peak memory | 7.59 MiB | 7.56 MiB | 7.56 MiB | 7.65 MiB | 4.91 MiB | 4.91 MiB | 5.17 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 152 KiB | 152 KiB | 119 KiB |

Its rounds took 30 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 160 | 1280 | 80 | 20 | 20 | 20 |
| CPU time | 109 ms | 109 ms | 109 ms | 109 ms | 31.2 ms | 31.2 ms | 46.9 ms |
| wall time | 114 ms | 118 ms | 116 ms | 120 ms | 38.9 ms | 39.6 ms | 51.0 ms |
| peak memory | 7.25 MiB | 7.29 MiB | 7.29 MiB | 7.26 MiB | 4.71 MiB | 4.72 MiB | 5.01 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 142 KiB | 142 KiB | 120 KiB |

Its rounds took 379 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 25 s | means 9.86 ms against 9.86 ms: +0.00 ns (-2.37 ms to +2.37 ms) | 15.8 ms against 15.3 ms: +537 µs (+456 µs to +618 µs) | 7.18 MiB against 7.04 MiB: +148 KiB (+148 KiB to +148 KiB) |
| add | 160, 160 in 27 s | means 9.86 ms against 10.9 ms: -1.07 ms (-3.34 ms to +1.19 ms) | 16.4 ms against 15.9 ms: +539 µs (+445 µs to +638 µs) | 7.23 MiB against 7.07 MiB: +156 KiB (+152 KiB to +156 KiB) |
| hypotenuse | 160, 160 in 26 s | means 10.1 ms against 8.79 ms: +1.27 ms (-817 µs to +3.36 ms) | 16.2 ms against 15.7 ms: +588 µs (+474 µs to +706 µs) | 7.22 MiB against 7.08 MiB: +152 KiB (+152 KiB to +152 KiB) |
| upcase | 160, 160 in 27 s | means 11.3 ms against 12.3 ms: -977 µs (-3.05 ms to +1.10 ms) | 17.8 ms against 17.0 ms: +816 µs (+694 µs to +947 µs) | 8.09 MiB against 7.94 MiB: +152 KiB (+152 KiB to +156 KiB) |
| upcase-stdin | 160, 160 in 46 s | means 34.6 ms against 34.3 ms: +293 µs (-1.55 ms to +2.13 ms) | 41.3 ms against 40.8 ms: +502 µs (+286 µs to +724 µs) | 7.23 MiB against 7.08 MiB: +148 KiB (+148 KiB to +148 KiB) |
| shout | 160, 160 in 27 s | means 11.2 ms against 10.4 ms: +879 µs (-1.36 ms to +3.12 ms) | 17.1 ms against 16.5 ms: +509 µs (+405 µs to +616 µs) | 8.00 MiB against 7.86 MiB: +144 KiB (+144 KiB to +144 KiB) |
| wc | 119, 119 in 60 s | means 39.4 ms against 40.6 ms: -1.18 ms (-3.80 ms to +1.43 ms) | 46.9 ms against 46.3 ms: +542 µs (+223 µs to +889 µs) | 7.24 MiB against 7.10 MiB: +148 KiB (+148 KiB to +148 KiB) |
| histogram | 125, 125 in 60 s | means 60.2 ms against 58.2 ms: +2.00 ms (-412 µs to +4.41 ms) | 66.2 ms against 65.6 ms: +534 µs (+289 µs to +776 µs) | 7.24 MiB against 7.10 MiB: +148 KiB (+144 KiB to +148 KiB) |
| crc32c | 137, 137 in 60 s | means 56.1 ms against 57.3 ms: -1.14 ms (-3.66 ms to +1.37 ms) | 63.5 ms against 63.0 ms: +420 µs (+145 µs to +692 µs) | 7.25 MiB against 7.10 MiB: +152 KiB (+152 KiB to +152 KiB) |
| sort | 20, 20 in 64 s | means 295 ms against 293 ms: +2.34 ms (-6.20 ms to +10.9 ms) | 300 ms against 298 ms: +1.93 ms (-1.63 ms to +5.33 ms) | 76.6 MiB against 76.4 MiB: +168 KiB (+164 KiB to +172 KiB) |
| tally | 39, 39 in 61 s | means 173 ms against 171 ms: +1.60 ms (-2.59 ms to +5.80 ms) | 180 ms against 179 ms: +1.51 ms (+298 µs to +2.77 ms) | 7.95 MiB against 7.77 MiB: +184 KiB (+184 KiB to +188 KiB) |
| server | 20, 20 in 186 s | means 1.59 s against 1.61 s: -14.1 ms (-27.6 ms to -566 µs) | 1.61 s against 1.62 s: -16.7 ms (-26.0 ms to -6.68 ms) | 7.59 MiB against 7.46 MiB: +136 KiB (+124 KiB to +144 KiB) |
| files | 55, 55 in 60 s | means 182 ms against 183 ms: -1.42 ms (-5.79 ms to +2.94 ms) | 189 ms against 190 ms: +228 µs (-1.38 ms to +2.05 ms) | 7.59 MiB against 7.46 MiB: +132 KiB (+128 KiB to +132 KiB) |
| readings | 91, 91 in 60 s | means 106 ms against 107 ms: -687 µs (-3.56 ms to +2.19 ms) | 114 ms against 114 ms: +482 µs (+249 µs to +710 µs) | 7.25 MiB against 7.11 MiB: +156 KiB (+152 KiB to +156 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 15.6 ms, 1.00× | 15.3 ms, 1.03× | 7.03 MiB, 1.02× |
| empty | cpp-wasm | 160 | 15.6 ms, 1.00× | 15.3 ms, 1.03× | 7.03 MiB, 1.02× |
| empty | rust-wasm | 160 | 15.6 ms, 1.00× | 15.5 ms, 1.02× | 7.05 MiB, 1.02× |
| add | c-wasm | 160 | 15.6 ms, 1.00× | 16.0 ms, 1.02× | 7.10 MiB, 1.02× |
| add | cpp-wasm | 160 | 15.6 ms, 1.00× | 16.1 ms, 1.02× | 7.10 MiB, 1.02× |
| add | rust-wasm | 160 | 15.6 ms, 1.00× | 16.2 ms, 1.01× | 7.08 MiB, 1.02× |
| hypotenuse | c-wasm | 160 | 7.81 ms, 2.00× | 15.8 ms, 1.03× | 7.09 MiB, 1.02× |
| hypotenuse | cpp-wasm | 160 | 15.6 ms, 1.00× | 15.8 ms, 1.03× | 7.09 MiB, 1.02× |
| hypotenuse | rust-wasm | 160 | 7.81 ms, 2.00× | 16.0 ms, 1.02× | 7.09 MiB, 1.02× |
| upcase | c-wasm | 160 | 15.6 ms, 1.00× | 17.0 ms, 1.05× | 7.96 MiB, 1.02× |
| upcase | cpp-wasm | 160 | 15.6 ms, 1.00× | 17.1 ms, 1.04× | 7.96 MiB, 1.02× |
| upcase | rust-wasm | 160 | 15.6 ms, 1.00× | 17.3 ms, 1.03× | 8.05 MiB, 1.00× |
| upcase-stdin | c-wasm | 160 | 31.2 ms, 1.00× | 40.5 ms, 1.02× | 7.08 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 31.2 ms, 1.00× | 41.0 ms, 1.01× | 7.08 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 31.2 ms, 1.00× | 41.3 ms, 1.00× | 7.10 MiB, 1.02× |
| shout | c-wasm | 160 | 15.6 ms, 1.00× | 16.5 ms, 1.04× | 7.80 MiB, 1.02× |
| shout | cpp-wasm | 160 | 15.6 ms, 1.00× | 16.5 ms, 1.03× | 7.80 MiB, 1.02× |
| shout | rust-wasm | 160 | 15.6 ms, 1.00× | 16.7 ms, 1.02× | 7.92 MiB, 1.01× |
| wc | c-wasm | 119 | 46.9 ms, 0.67× | 54.0 ms, 0.87× | 7.12 MiB, 1.02× |
| wc | cpp-wasm | 119 | 203 ms, 0.15× | 214 ms, 0.22× | 7.12 MiB, 1.02× |
| wc | rust-wasm | 119 | 46.9 ms, 0.67× | 54.7 ms, 0.86× | 7.10 MiB, 1.02× |
| histogram | c-wasm | 125 | 93.8 ms, 0.67× | 97.0 ms, 0.68× | 7.14 MiB, 1.01× |
| histogram | cpp-wasm | 125 | 93.8 ms, 0.67× | 96.9 ms, 0.68× | 7.13 MiB, 1.02× |
| histogram | rust-wasm | 125 | 62.5 ms, 1.00× | 68.1 ms, 0.97× | 7.11 MiB, 1.02× |
| crc32c | c-wasm | 137 | 62.5 ms, 1.00× | 73.4 ms, 0.87× | 7.11 MiB, 1.02× |
| crc32c | cpp-wasm | 137 | 62.5 ms, 1.00× | 73.5 ms, 0.86× | 7.11 MiB, 1.02× |
| crc32c | rust-wasm | 137 | 62.5 ms, 1.00× | 76.4 ms, 0.83× | 7.11 MiB, 1.02× |
| sort | c-wasm | 20 | 1.49 s, 0.20× | 1.51 s, 0.20× | 76.4 MiB, 1.00× |
| sort | cpp-wasm | 20 | 516 ms, 0.58× | 518 ms, 0.58× | 84.4 MiB, 0.91× |
| sort | rust-wasm | 20 | 453 ms, 0.66× | 466 ms, 0.64× | 84.4 MiB, 0.91× |
| tally | c-wasm | 39 | 328 ms, 0.52× | 332 ms, 0.54× | 7.62 MiB, 1.04× |
| tally | cpp-wasm | 39 | 328 ms, 0.52× | 331 ms, 0.54× | 7.61 MiB, 1.05× |
| tally | rust-wasm | 39 | 438 ms, 0.39× | 441 ms, 0.41× | 7.41 MiB, 1.07× |
| server | c-wasm | 20 | 1.58 s, 1.01× | 1.74 s, 0.92× | 7.38 MiB, 1.03× |
| server | cpp-wasm | 20 | 1.58 s, 1.01× | 1.75 s, 0.92× | 7.43 MiB, 1.02× |
| server | rust-wasm | 20 | 1.62 s, 0.99× | 1.84 s, 0.87× | 7.52 MiB, 1.01× |
| files | c-wasm | 55 | 203 ms, 0.92× | 206 ms, 0.92× | 7.36 MiB, 1.03× |
| files | cpp-wasm | 55 | 203 ms, 0.92× | 208 ms, 0.91× | 7.36 MiB, 1.03× |
| files | rust-wasm | 55 | 203 ms, 0.92× | 208 ms, 0.91× | 7.46 MiB, 1.02× |
| readings | c-wasm | 91 | 109 ms, 1.00× | 117 ms, 0.98× | 7.12 MiB, 1.02× |
| readings | cpp-wasm | 91 | 109 ms, 1.00× | 115 ms, 0.99× | 7.12 MiB, 1.02× |
| readings | rust-wasm | 91 | 109 ms, 1.00× | 119 ms, 0.96× | 7.11 MiB, 1.02× |
