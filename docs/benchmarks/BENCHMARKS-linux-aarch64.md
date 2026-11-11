# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 45b507bf262dab89bff38a1f9c35bdd5590c8965, in https://github.com/laughing-hermit/lengua/actions/runs/43084459708. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (4.88 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
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
| empty | ≈ ✗ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✗ ✗ ✗ ✓ | ✗ ✗ ✗ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✗ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ≈ ✓ | ✗ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ≈ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-aarch64 empty c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 214 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 empty c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 empty c-wasm memory: lengua is level now, and KNOWN_BEHIND line 215 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 empty cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 216 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 empty cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 217 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 empty rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 218 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 add c-wasm memory: lengua is level now, and KNOWN_BEHIND line 231 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 add cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 232 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 add rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 233 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 hypotenuse c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 hypotenuse c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 hypotenuse cpp-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 hypotenuse cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 hypotenuse rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 248 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 upcase c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout c-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout c-wasm memory: lengua is level now, and KNOWN_BEHIND line 287 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 shout cpp-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-aarch64 shout cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 288 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 shout rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 289 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 wc cpp wall: lengua is ahead now, and KNOWN_BEHIND line 306 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 wc rust wall: lengua is ahead now, and KNOWN_BEHIND line 309 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 sort rust memory: lengua is level now, and KNOWN_BEHIND line 340 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.02× | 2.78× | 5.10× | 75.82× |
| add | 2.87× | 2.67× | 3.21× | 75.83× |
| hypotenuse | 2.68× | 2.52× | 3.02× | 75.84× |
| upcase | 1.81× | 1.82× | 2.90× | 75.85× |
| upcase-stdin | 1.23× | 1.23× | 7.56× | 75.83× |
| shout | 2.12× | 2.01× | 2.64× | 75.83× |
| wc | 1.05× | 1.05× | 5.09× | 75.85× |
| histogram | 1.07× | 1.07× | 5.00× | 75.84× |
| crc32c | 3.38× | 3.36× | 5.00× | 75.84× |
| sort | 1.16× | 1.16× | 1.01× | 75.87× |
| tally | 0.59× | 0.59× | 4.19× | 75.88× |
| server | 1.26× | 1.48× | 10.62× | 75.84× |
| files | 3.10× | 3.08× | 5.84× | 75.84× |
| readings | 2.95× | 2.95× | 5.46× | 75.83× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 20 | level, 1.05×, p 1.8e-4 | behind, 1.06×, p 5.2e-5 | level, 1.03× | ahead, 0.89× |
| empty | cpp-wasm | 20 | level, 1.04×, p 4.5e-6 | level, 1.05×, p 1.9e-5 | level, 1.03× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 1.3e-5 | level, 1.00×, p 3.2e-5 | level, 1.00× | ahead, 0.01× |
| empty | c | 20 | behind, 3.02×, p 3.4e-8 | behind, 2.78×, p 3.4e-8 | behind, 5.10× | behind, 75.82× |
| empty | cpp | 20 | behind, 2.92×, p 3.4e-8 | behind, 2.71×, p 3.4e-8 | behind, 5.10× | behind, 75.82× |
| empty | rust | 20 | behind, 2.21×, p 3.4e-8 | behind, 2.12×, p 3.4e-8 | behind, 2.97× | behind, 19.17× |
| add | c-wasm | 80 | level, 1.03×, p 4.0e-6 | level, 1.02×, p 1.2e-5 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 40 | level, 1.03×, p 8.6e-5 | level, 1.04×, p 2.3e-4 | level, 1.03× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.93×, p 3.8e-4 | ahead, 0.95×, p 0.001 | level, 1.01× | ahead, 0.02× |
| add | c | 20 | behind, 2.87×, p 3.4e-8 | behind, 2.64×, p 3.4e-8 | behind, 3.21× | behind, 75.83× |
| add | cpp | 20 | behind, 2.87×, p 3.4e-8 | behind, 2.67×, p 3.4e-8 | behind, 3.21× | behind, 75.83× |
| add | rust | 20 | behind, 2.31×, p 3.4e-8 | behind, 2.19×, p 3.4e-8 | behind, 3.08× | behind, 15.39× |
| hypotenuse | c-wasm | 20 | behind, 1.08×, p 5.2e-6 | behind, 1.07×, p 4.3e-6 | behind, 1.06× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | behind, 1.07×, p 3.8e-6 | behind, 1.07×, p 1.5e-5 | behind, 1.06× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | level, 0.97×, p 4.6e-4 | level, 0.98×, p 0.001 | level, 1.03× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.67×, p 3.4e-8 | behind, 2.52×, p 3.4e-8 | behind, 3.02× | behind, 75.84× |
| hypotenuse | cpp | 20 | behind, 2.68×, p 3.4e-8 | behind, 2.48×, p 3.4e-8 | behind, 3.02× | behind, 75.84× |
| hypotenuse | rust | 20 | behind, 2.40×, p 3.4e-8 | behind, 2.22×, p 3.4e-8 | behind, 2.92× | behind, 15.39× |
| upcase | c-wasm | 160 | behind, 1.06×, p 2.4e-18 | level, 1.04×, p 1.0e-6 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 160 | level, 1.04×, p 6.3e-14 | level, 1.02×, p 4.6e-4 | level, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 320 | level, 0.98×, p 8.6e-8 | level, 0.98×, p 9.6e-4 | level, 0.98× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.81×, p 3.4e-8 | behind, 1.82×, p 3.4e-8 | behind, 2.90× | behind, 75.85× |
| upcase | cpp | 20 | behind, 1.80×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 2.90× | behind, 75.85× |
| upcase | rust | 20 | behind, 1.63×, p 3.4e-8 | behind, 1.63×, p 3.4e-8 | behind, 2.46× | behind, 15.40× |
| upcase-stdin | c-wasm | 80 | level, 1.01×, p 1.3e-4 | level, 1.01×, p 1.5e-4 | level, 0.98× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 640 | level, 1.01×, p 1.1e-5 | level, 1.01×, p 4.3e-5 | level, 0.98× | behind, 1.32× |
| upcase-stdin | rust-wasm | 640 | level, 1.01×, p 9.8e-6 | level, 1.01×, p 8.6e-6 | level, 0.97× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.21×, p 3.4e-8 | behind, 1.21×, p 3.4e-8 | behind, 7.56× | behind, 75.83× |
| upcase-stdin | cpp | 20 | behind, 1.23×, p 3.4e-8 | behind, 1.23×, p 3.4e-8 | behind, 7.56× | behind, 75.83× |
| upcase-stdin | rust | 20 | behind, 1.13×, p 3.4e-8 | behind, 1.13×, p 3.4e-8 | behind, 4.50× | behind, 15.39× |
| shout | c-wasm | 20 | behind, 1.09×, p 1.2e-5 | behind, 1.08×, p 7.4e-4 | level, 1.02× | ahead, 0.40× |
| shout | cpp-wasm | 80 | behind, 1.06×, p 7.7e-12 | level, 1.04×, p 1.8e-7 | level, 1.02× | ahead, 0.40× |
| shout | rust-wasm | 160 | level, 0.97×, p 1.9e-8 | level, 0.97×, p 3.8e-7 | level, 1.01× | ahead, 0.03× |
| shout | c | 20 | behind, 2.09×, p 3.4e-8 | behind, 2.00×, p 3.4e-8 | behind, 2.64× | behind, 75.83× |
| shout | cpp | 20 | behind, 2.12×, p 3.4e-8 | behind, 2.01×, p 3.4e-8 | behind, 2.64× | behind, 75.83× |
| shout | rust | 20 | behind, 1.81×, p 3.4e-8 | behind, 1.74×, p 3.4e-8 | behind, 2.22× | behind, 15.39× |
| wc | c-wasm | 20 | ahead, 0.68×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.69×, p 3.4e-8 | ahead, 0.69×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| wc | c | 20 | behind, 1.05×, p 3.4e-8 | behind, 1.05×, p 3.4e-8 | behind, 5.09× | behind, 75.85× |
| wc | cpp | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.90×, p 6.0e-7 | behind, 5.09× | behind, 75.85× |
| wc | rust | 20 | ahead, 0.84×, p 3.4e-8 | ahead, 0.84×, p 3.4e-8 | behind, 4.54× | behind, 15.39× |
| histogram | c-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.54×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | rust-wasm | 20 | level, 0.96×, p 3.4e-8 | level, 0.96×, p 4.6e-8 | level, 0.97× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 5.00× | behind, 75.84× |
| histogram | cpp | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 5.00× | behind, 75.84× |
| histogram | rust | 20 | behind, 1.06×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 4.46× | behind, 15.39× |
| crc32c | c-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.36×, p 3.4e-8 | behind, 2.36×, p 3.4e-8 | behind, 5.00× | behind, 75.84× |
| crc32c | cpp | 20 | behind, 2.36×, p 3.4e-8 | behind, 2.36×, p 3.4e-8 | behind, 5.00× | behind, 75.84× |
| crc32c | rust | 20 | behind, 3.38×, p 3.4e-8 | behind, 3.36×, p 3.4e-8 | behind, 4.46× | behind, 15.39× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.66×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | ahead, 0.89× | behind, 75.87× |
| sort | cpp | 20 | ahead, 0.77×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | ahead, 0.93× | behind, 75.77× |
| sort | rust | 20 | behind, 1.16×, p 3.4e-8 | behind, 1.16×, p 3.4e-8 | level, 1.01× | behind, 15.40× |
| tally | c-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 3.92× | behind, 75.88× |
| tally | cpp | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | behind, 2.11× | behind, 75.88× |
| tally | rust | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | behind, 4.19× | behind, 15.40× |
| server | c-wasm | 272 | level, 0.98×, p 6.7e-7 | ahead, 0.94×, p 1.3e-46 | level, 1.00× | ahead, 0.12× |
| server | cpp-wasm | 272 | level, 0.99×, p 3.5e-5 | ahead, 0.95×, p 9.1e-47 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 80 | level, 0.97×, p 2.0e-5 | ahead, 0.93×, p 1.5e-20 | level, 0.99× | ahead, 0.03× |
| server | c | 20 | behind, 1.26×, p 3.4e-8 | behind, 1.25×, p 3.4e-8 | behind, 10.62× | behind, 75.84× |
| server | cpp | 20 | behind, 1.25×, p 3.4e-8 | behind, 1.24×, p 3.4e-8 | behind, 2.95× | behind, 75.74× |
| server | rust | 20 | ahead, 0.74×, p 3.4e-8 | behind, 1.48×, p 3.4e-8 | behind, 3.69× | behind, 15.39× |
| files | c-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 0.98× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.4e-8 | level, 0.98× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.4e-8 | level, 0.97× | ahead, 0.03× |
| files | c | 20 | behind, 2.54×, p 3.4e-8 | behind, 2.53×, p 3.4e-8 | behind, 5.84× | behind, 75.84× |
| files | cpp | 20 | behind, 2.53×, p 3.4e-8 | behind, 2.53×, p 3.4e-8 | behind, 5.84× | behind, 75.84× |
| files | rust | 20 | behind, 3.10×, p 3.4e-8 | behind, 3.08×, p 3.4e-8 | behind, 4.94× | behind, 15.39× |
| readings | c-wasm | 20 | level, 0.96×, p 5.3e-8 | level, 0.96×, p 5.3e-8 | level, 0.97× | ahead, 0.04× |
| readings | cpp-wasm | 20 | level, 0.99×, p 9.0e-7 | level, 0.99×, p 4.1e-5 | level, 0.97× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.94×, p 3.4e-8 | level, 0.97× | ahead, 0.02× |
| readings | c | 20 | behind, 2.95×, p 3.4e-8 | behind, 2.95×, p 3.4e-8 | behind, 5.46× | behind, 75.83× |
| readings | cpp | 20 | behind, 2.87×, p 3.4e-8 | behind, 2.86×, p 3.4e-8 | behind, 2.33× | behind, 75.83× |
| readings | rust | 20 | behind, 2.90×, p 3.4e-8 | behind, 2.89×, p 3.4e-8 | behind, 5.04× | behind, 15.39× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 20 | 20 | 2000 | 20 | 20 | 20 |
| CPU time | 1.43 ms | 1.39 ms | 1.39 ms | 1.44 ms | 482 µs | 497 µs | 657 µs |
| wall time | 1.52 ms | 1.48 ms | 1.49 ms | 1.53 ms | 563 µs | 579 µs | 737 µs |
| peak memory | 4.34 MiB | 4.22 MiB | 4.22 MiB | 4.34 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 8,302 | 8,215 | 8,248 | 8,927 | 1,539 | 1,491 | 2,625 |

Its rounds took 19 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

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
| counted runs | 80 | 80 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 1.49 ms | 1.45 ms | 1.43 ms | 1.62 ms | 528 µs | 528 µs | 658 µs |
| wall time | 1.58 ms | 1.55 ms | 1.52 ms | 1.72 ms | 615 µs | 607 µs | 741 µs |
| peak memory | 4.34 MiB | 4.34 MiB | 4.22 MiB | 4.32 MiB | 1.35 MiB | 1.35 MiB | 1.41 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 8,665 | 8,992 | 9,131 | 10,069 | 1,725 | 1,652 | 2,644 |

Its rounds took 1 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 1.57 ms | 1.45 ms | 1.46 ms | 1.61 ms | 587 µs | 584 µs | 652 µs |
| wall time | 1.66 ms | 1.55 ms | 1.55 ms | 1.70 ms | 660 µs | 671 µs | 749 µs |
| peak memory | 4.46 MiB | 4.22 MiB | 4.22 MiB | 4.32 MiB | 1.48 MiB | 1.48 MiB | 1.53 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 8,988 | 8,886 | 8,884 | 10,035 | 2,037 | 2,083 | 2,662 |

Its rounds took 1 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

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
| counted runs | 320 | 160 | 160 | 320 | 20 | 20 | 20 |
| CPU time | 2.41 ms | 2.30 ms | 2.34 ms | 2.46 ms | 1.38 ms | 1.38 ms | 1.53 ms |
| wall time | 2.65 ms | 2.55 ms | 2.59 ms | 2.70 ms | 1.53 ms | 1.56 ms | 1.71 ms |
| peak memory | 6.34 MiB | 6.33 MiB | 6.34 MiB | 6.46 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 10,084 | 10,172 | 10,004 | 11,114 | 2,194 | 2,173 | 3,002 |

Its rounds took 7 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

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
| counted runs | 640 | 80 | 640 | 640 | 20 | 20 | 20 |
| CPU time | 12.4 ms | 12.2 ms | 12.3 ms | 12.3 ms | 10.2 ms | 10.0 ms | 10.9 ms |
| wall time | 12.5 ms | 12.3 ms | 12.4 ms | 12.4 ms | 10.3 ms | 10.1 ms | 11.0 ms |
| peak memory | 7.39 MiB | 7.51 MiB | 7.51 MiB | 7.64 MiB | 1000 KiB | 1000 KiB | 1.64 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 138,155 | 142,678 | 144,425 | 151,621 | 24,382 | 23,518 | 47,553 |

Its rounds took 35 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

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
| counted runs | 160 | 20 | 80 | 160 | 20 | 20 | 20 |
| CPU time | 1.94 ms | 1.86 ms | 1.86 ms | 2.00 ms | 972 µs | 960 µs | 1.12 ms |
| wall time | 2.03 ms | 1.96 ms | 1.98 ms | 2.10 ms | 1.06 ms | 1.05 ms | 1.22 ms |
| peak memory | 5.22 MiB | 5.09 MiB | 5.09 MiB | 5.18 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,282 | 8,948 | 8,972 | 10,129 | 1,945 | 1,927 | 3,095 |

Its rounds took 3 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

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
| CPU time | 18.7 ms | 27.5 ms | 185 ms | 27.0 ms | 17.7 ms | 20.7 ms | 22.3 ms |
| wall time | 18.8 ms | 27.7 ms | 186 ms | 27.2 ms | 17.9 ms | 20.8 ms | 22.5 ms |
| peak memory | 7.51 MiB | 7.50 MiB | 7.50 MiB | 7.61 MiB | 1.48 MiB | 1.48 MiB | 1.66 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 118,614 | 128,044 | 119,280 | 135,457 | 23,461 | 25,389 | 33,093 |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 38.0 ms | 69.6 ms | 69.7 ms | 39.6 ms | 35.5 ms | 35.5 ms | 35.9 ms |
| wall time | 38.1 ms | 69.8 ms | 69.9 ms | 39.8 ms | 35.6 ms | 35.6 ms | 36.0 ms |
| peak memory | 7.38 MiB | 7.50 MiB | 7.50 MiB | 7.62 MiB | 1.48 MiB | 1.48 MiB | 1.66 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 171,095 | 176,521 | 176,841 | 193,302 | 82,401 | 82,944 | 100,799 |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

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
| CPU time | 35.6 ms | 45.8 ms | 45.9 ms | 48.8 ms | 15.1 ms | 15.1 ms | 10.5 ms |
| wall time | 35.8 ms | 46.0 ms | 46.0 ms | 49.0 ms | 15.2 ms | 15.2 ms | 10.7 ms |
| peak memory | 7.38 MiB | 7.50 MiB | 7.50 MiB | 7.59 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 294,511 | 218,665 | 225,043 | 229,582 | 23,411 | 24,604 | 28,413 |

Its rounds took 6 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

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
| CPU time | 255 ms | 1.29 s | 443 ms | 386 ms | 359 ms | 330 ms | 220 ms |
| wall time | 255 ms | 1.29 s | 443 ms | 387 ms | 359 ms | 331 ms | 220 ms |
| peak memory | 76.9 MiB | 77.0 MiB | 85.0 MiB | 85.1 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,268,147 | 30,354,114 | 12,270,085 | 12,088,330 | 14,490,279 | 15,487,888 | 12,432,658 |

Its rounds took 78 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

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
| CPU time | 139 ms | 289 ms | 288 ms | 405 ms | 235 ms | 244 ms | 252 ms |
| wall time | 139 ms | 289 ms | 289 ms | 405 ms | 236 ms | 244 ms | 252 ms |
| peak memory | 8.26 MiB | 8.13 MiB | 8.14 MiB | 7.98 MiB | 2.11 MiB | 3.92 MiB | 1.97 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,806,534 | 8,649,593 | 8,654,914 | 8,127,913 | 8,536,104 | 8,391,147 | 11,220,128 |

Its rounds took 43 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

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
| counted runs | 272 | 272 | 272 | 80 | 20 | 20 | 20 |
| CPU time | 554 ms | 565 ms | 561 ms | 564 ms | 432 ms | 435 ms | 739 ms |
| wall time | 557 ms | 593 ms | 589 ms | 596 ms | 438 ms | 441 ms | 371 ms |
| peak memory | 7.51 MiB | 7.50 MiB | 7.51 MiB | 7.61 MiB | 724 KiB | 2.54 MiB | 2.04 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,370,827 | 2,815,903 | 2,540,939 | 2,514,327 | 55,601 | 219,000 | 481,749 |

Its rounds took 612 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 43.5 ms | 47.1 ms | 46.7 ms | 46.9 ms | 17.1 ms | 17.2 ms | 14.0 ms |
| wall time | 43.6 ms | 47.2 ms | 46.9 ms | 47.1 ms | 17.2 ms | 17.3 ms | 14.2 ms |
| peak memory | 7.89 MiB | 8.01 MiB | 8.01 MiB | 8.13 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,436,007 | 7,595,184 | 7,590,319 | 7,634,790 | 11,759 | 8,792 | 9,338 |

Its rounds took 6 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

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
| CPU time | 94.8 ms | 98.5 ms | 95.9 ms | 101 ms | 32.1 ms | 33.1 ms | 32.7 ms |
| wall time | 95.0 ms | 98.6 ms | 96.0 ms | 101 ms | 32.2 ms | 33.2 ms | 32.9 ms |
| peak memory | 7.38 MiB | 7.64 MiB | 7.64 MiB | 7.62 MiB | 1.35 MiB | 3.16 MiB | 1.46 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,941 | 10,100 | 10,007 | 10,823 | 1,775 | 13,495 | 2,737 |

Its rounds took 12 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 1 s | 1.47 ms against 1.38 ms: +84.0 µs (+73.0 µs to +95.0 µs) | 1.56 ms against 1.47 ms: +83.3 µs (+71.3 µs to +94.9 µs) | 4.34 MiB against 4.22 MiB: +128 KiB (+128 KiB to +128 KiB) |
| add | 160, 160 in 1 s | 1.48 ms against 1.38 ms: +107 µs (+96.0 µs to +117 µs) | 1.57 ms against 1.47 ms: +105 µs (+93.6 µs to +117 µs) | 4.34 MiB against 4.22 MiB: +128 KiB (+128 KiB to +128 KiB) |
| hypotenuse | 160, 160 in 2 s | 1.52 ms against 1.38 ms: +142 µs (+131 µs to +152 µs) | 1.62 ms against 1.48 ms: +142 µs (+131 µs to +154 µs) | 4.46 MiB against 4.22 MiB: +252 KiB (+252 KiB to +256 KiB) |
| upcase | 160, 160 in 2 s | 2.38 ms against 2.20 ms: +179 µs (+158 µs to +202 µs) | 2.61 ms against 2.43 ms: +177 µs (+136 µs to +219 µs) | 6.34 MiB against 6.33 MiB: +8.00 KiB (+0 B to +124 KiB) |
| upcase-stdin | 160, 160 in 5 s | 12.4 ms against 12.3 ms: +100 µs (+26.0 µs to +174 µs) | 12.5 ms against 12.4 ms: +89.3 µs (+13.3 µs to +171 µs) | 7.39 MiB against 7.51 MiB: -128 KiB (-128 KiB to -128 KiB) |
| shout | 160, 160 in 2 s | 1.95 ms against 1.82 ms: +136 µs (+116 µs to +156 µs) | 2.08 ms against 1.92 ms: +149 µs (+120 µs to +175 µs) | 5.22 MiB against 4.97 MiB: +256 KiB (+256 KiB to +256 KiB) |
| wc | 160, 160 in 7 s | 18.4 ms against 18.2 ms: +172 µs (+92.0 µs to +250 µs) | 18.5 ms against 18.4 ms: +165 µs (+78.3 µs to +247 µs) | 7.51 MiB against 7.51 MiB: +0 B (+0 B to +0 B) |
| histogram | 160, 160 in 13 s | 38.1 ms against 37.9 ms: +203 µs (+150 µs to +257 µs) | 38.2 ms against 38.0 ms: +203 µs (+146 µs to +261 µs) | 7.38 MiB against 7.51 MiB: -132 KiB (-132 KiB to -128 KiB) |
| crc32c | 160, 160 in 13 s | 35.5 ms against 35.3 ms: +180 µs (+116 µs to +245 µs) | 35.6 ms against 35.4 ms: +166 µs (+103 µs to +234 µs) | 7.38 MiB against 7.51 MiB: -132 KiB (-132 KiB to -128 KiB) |
| sort | 108, 108 in 60 s | 273 ms against 272 ms: +948 µs (-911 µs to +2.57 ms) | 273 ms against 272 ms: +951 µs (-925 µs to +2.56 ms) | 76.9 MiB against 77.0 MiB: -128 KiB (-128 KiB to -128 KiB) |
| tally | 160, 160 in 46 s | 138 ms against 139 ms: +22.0 µs (-292 µs to +341 µs) | 139 ms against 139 ms: +1.66 µs (-320 µs to +320 µs) | 8.26 MiB against 8.38 MiB: -120 KiB (-120 KiB to -120 KiB) |
| server | 49, 49 in 60 s | 549 ms against 559 ms: -2.25 ms (-11.4 ms to +6.90 ms) | 552 ms against 562 ms: -1.39 ms (-11.2 ms to +8.10 ms) | 7.51 MiB against 7.51 MiB: +0 B (-4.00 KiB to +0 B) |
| files | 160, 160 in 15 s | 43.6 ms against 43.5 ms: +57.0 µs (-25.0 µs to +137 µs) | 43.7 ms against 43.7 ms: +46.6 µs (-45.5 µs to +135 µs) | 7.89 MiB against 8.01 MiB: -128 KiB (-128 KiB to -128 KiB) |
| readings | 160, 160 in 32 s | 94.8 ms against 94.6 ms: +228 µs (+128 µs to +327 µs) | 95.0 ms against 94.8 ms: +225 µs (+124 µs to +331 µs) | 7.38 MiB against 7.51 MiB: -132 KiB (-136 KiB to -132 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
