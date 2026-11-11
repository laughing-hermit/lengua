# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit cd54e79fc07fe745f5fcad008bd2a7da4a18ae9e, in https://github.com/laughing-hermit/lengua/actions/runs/42964232679. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (23.1 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ✗ ≈ ✗ ✓ | ✗ ≈ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ✗ ✓ | ✓ ≈ ✗ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ✗ ✗ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ✗ ✗ ✗ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 empty c-wasm wall: lengua is level now, and KNOWN_BEHIND line 32 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 empty cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 35 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 upcase-stdin c-wasm memory: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 upcase-stdin rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 96 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout c-wasm wall: lengua is level now, and KNOWN_BEHIND line 110 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 112 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 113 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 wc c-wasm memory: lengua is level now, and KNOWN_BEHIND line 128 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 wc rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 129 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 histogram c-wasm memory: lengua is level now, and KNOWN_BEHIND line 140 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 histogram cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 141 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 histogram rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 142 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 crc32c c-wasm memory: lengua is level now, and KNOWN_BEHIND line 155 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 crc32c cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 156 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 crc32c rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 157 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 sort c-wasm memory: lengua is level now, and KNOWN_BEHIND line 170 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 sort cpp memory: lengua is level now, and KNOWN_BEHIND line 172 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 server c-wasm memory: lengua is level now, and KNOWN_BEHIND line 187 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 server cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 188 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 server rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 189 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 files c-wasm memory: lengua is level now, and KNOWN_BEHIND line 201 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 files cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 202 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 files rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 203 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 readings c-wasm memory: lengua is level now, and KNOWN_BEHIND line 216 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 readings cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 217 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 readings rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 218 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 5.49× | 4.20× | 9.42× | 1689.17× |
| add | 5.18× | 3.94× | 7.13× | 1673.36× |
| hypotenuse | 4.42× | 3.35× | 6.14× | 1673.38× |
| upcase | 2.68× | 2.23× | 5.66× | 1673.41× |
| upcase-stdin | 1.81× | 1.74× | 12.84× | 1673.35× |
| shout | 3.23× | 3.11× | 5.59× | 1673.36× |
| wc | 1.37× | 1.33× | 10.03× | 1673.40× |
| histogram | 1.18× | 1.16× | 10.26× | 1673.38× |
| crc32c | 2.85× | 2.80× | 10.11× | 1673.38× |
| sort | 1.18× | 1.17× | 1.14× | 1673.54× |
| tally | 0.51× | 0.50× | 8.03× | 1673.58× |
| server | 1.07× | 1.84× | 15.28× | 1662.37× |
| files | 2.75× | 2.72× | 10.72× | 1673.39× |
| readings | 3.40× | 3.38× | 10.64× | 1673.36× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 320 | behind, 1.08×, p 2.6e-29 | level, 1.01×, p 5.5e-4 | behind, 1.09× | ahead, 0.89× |
| empty | cpp-wasm | 320 | behind, 1.07×, p 5.7e-29 | level, 1.01×, p 6.1e-4 | behind, 1.09× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.89×, p 4.0e-7 | ahead, 0.84×, p 2.6e-6 | behind, 1.09× | ahead, 0.01× |
| empty | c | 20 | behind, 5.49×, p 3.4e-8 | behind, 4.20×, p 3.4e-8 | behind, 9.42× | behind, 1689.17× |
| empty | cpp | 20 | behind, 5.43×, p 3.4e-8 | behind, 4.16×, p 3.4e-8 | behind, 9.42× | behind, 1689.17× |
| empty | rust | 20 | behind, 3.88×, p 3.4e-8 | behind, 3.11×, p 3.4e-8 | behind, 6.30× | behind, 84.73× |
| add | c-wasm | 20 | ahead, 0.94×, p 3.9e-5 | ahead, 0.89×, p 1.5e-5 | behind, 1.08× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.94×, p 7.2e-5 | ahead, 0.89×, p 1.3e-5 | behind, 1.07× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.86×, p 7.2e-8 | ahead, 0.81×, p 2.0e-6 | behind, 1.07× | ahead, 0.02× |
| add | c | 20 | behind, 5.03×, p 3.4e-8 | behind, 3.88×, p 3.4e-8 | behind, 6.64× | behind, 1673.36× |
| add | cpp | 20 | behind, 5.18×, p 3.4e-8 | behind, 3.94×, p 3.4e-8 | behind, 7.13× | behind, 1673.36× |
| add | rust | 20 | behind, 3.87×, p 3.4e-8 | behind, 3.07×, p 3.4e-8 | behind, 6.16× | behind, 82.21× |
| hypotenuse | c-wasm | 20 | ahead, 0.94×, p 5.3e-4 | ahead, 0.87×, p 9.6e-8 | behind, 1.07× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | ahead, 0.93×, p 5.6e-4 | ahead, 0.87×, p 8.3e-8 | behind, 1.07× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.83×, p 3.9e-8 | ahead, 0.77×, p 3.4e-8 | behind, 1.05× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 4.42×, p 3.4e-8 | behind, 3.35×, p 3.4e-8 | behind, 5.64× | behind, 1673.38× |
| hypotenuse | cpp | 20 | behind, 4.37×, p 3.4e-8 | behind, 3.32×, p 3.4e-8 | behind, 5.87× | behind, 1673.38× |
| hypotenuse | rust | 20 | behind, 3.92×, p 3.4e-8 | behind, 3.01×, p 3.4e-8 | behind, 6.14× | behind, 81.52× |
| upcase | c-wasm | 320 | level, 0.96×, p 5.1e-7 | level, 1.00×, p 1.8e-4 | behind, 1.15× | ahead, 0.06× |
| upcase | cpp-wasm | 320 | ahead, 0.95×, p 1.6e-9 | level, 0.98×, p 7.6e-5 | behind, 1.16× | ahead, 0.06× |
| upcase | rust-wasm | 40 | ahead, 0.88×, p 2.2e-8 | ahead, 0.78×, p 3.1e-4 | level, 1.00× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.65×, p 3.4e-8 | behind, 2.20×, p 3.4e-8 | behind, 5.66× | behind, 1673.41× |
| upcase | cpp | 20 | behind, 2.68×, p 3.4e-8 | behind, 2.23×, p 3.4e-8 | behind, 5.59× | behind, 1673.41× |
| upcase | rust | 20 | behind, 2.39×, p 3.4e-8 | behind, 2.00×, p 6.0e-7 | behind, 4.70× | behind, 82.30× |
| upcase-stdin | c-wasm | 20 | ahead, 0.83×, p 8.3e-8 | ahead, 0.82×, p 1.0e-6 | behind, 1.05× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.32×, p 3.4e-8 | ahead, 0.31×, p 6.0e-7 | level, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 20 | ahead, 0.94×, p 9.0e-5 | ahead, 0.92×, p 3.7e-5 | level, 1.04× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.77×, p 3.4e-8 | behind, 1.70×, p 3.4e-8 | behind, 12.84× | behind, 1673.35× |
| upcase-stdin | cpp | 20 | behind, 1.77×, p 3.4e-8 | behind, 1.69×, p 3.4e-8 | behind, 12.84× | behind, 1673.35× |
| upcase-stdin | rust | 20 | behind, 1.81×, p 3.4e-8 | behind, 1.74×, p 3.4e-8 | behind, 8.79× | behind, 83.77× |
| shout | c-wasm | 640 | behind, 1.06×, p 1.4e-38 | level, 1.03×, p 4.7e-5 | behind, 1.11× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.03×, p 8.8e-36 | level, 1.02×, p 3.6e-5 | behind, 1.11× | ahead, 0.40× |
| shout | rust-wasm | 80 | ahead, 0.95×, p 2.0e-10 | ahead, 0.85×, p 8.5e-9 | behind, 1.07× | ahead, 0.03× |
| shout | c | 20 | behind, 3.23×, p 3.4e-8 | behind, 3.11×, p 3.4e-8 | behind, 5.51× | behind, 1673.36× |
| shout | cpp | 20 | behind, 3.23×, p 3.4e-8 | behind, 3.10×, p 3.4e-8 | behind, 5.59× | behind, 1673.36× |
| shout | rust | 20 | behind, 2.69×, p 3.4e-8 | behind, 2.59×, p 3.4e-8 | behind, 4.60× | behind, 82.77× |
| wc | c-wasm | 20 | ahead, 0.76×, p 3.4e-8 | ahead, 0.74×, p 3.4e-8 | level, 1.04× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.11×, p 3.4e-8 | ahead, 0.11×, p 3.4e-8 | level, 1.01× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.70×, p 3.4e-8 | level, 1.03× | ahead, 0.04× |
| wc | c | 20 | behind, 1.10×, p 3.4e-8 | behind, 1.08×, p 3.4e-8 | behind, 10.03× | behind, 1673.40× |
| wc | cpp | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.80×, p 3.4e-8 | behind, 10.03× | behind, 1673.40× |
| wc | rust | 20 | behind, 1.37×, p 3.4e-8 | behind, 1.33×, p 3.4e-8 | behind, 8.74× | behind, 80.95× |
| histogram | c-wasm | 20 | ahead, 0.65×, p 3.4e-8 | ahead, 0.65×, p 3.4e-8 | level, 1.03× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.76×, p 3.4e-8 | ahead, 0.75×, p 3.4e-8 | level, 1.03× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.72×, p 1.3e-7 | ahead, 0.71×, p 1.3e-7 | level, 1.02× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.18×, p 3.4e-8 | behind, 1.16×, p 3.4e-8 | behind, 10.26× | behind, 1673.38× |
| histogram | cpp | 20 | behind, 1.17×, p 3.9e-8 | behind, 1.15×, p 6.2e-8 | behind, 10.26× | behind, 1673.38× |
| histogram | rust | 20 | behind, 1.18×, p 3.4e-8 | behind, 1.16×, p 3.4e-8 | behind, 8.97× | behind, 80.92× |
| crc32c | c-wasm | 20 | ahead, 0.77×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.76×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.68×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 1.03× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.70×, p 3.4e-8 | behind, 10.11× | behind, 1673.38× |
| crc32c | cpp | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.70×, p 3.4e-8 | behind, 10.11× | behind, 1673.38× |
| crc32c | rust | 20 | behind, 2.85×, p 3.4e-8 | behind, 2.80×, p 3.4e-8 | behind, 9.00× | behind, 82.99× |
| sort | c-wasm | 20 | ahead, 0.17×, p 3.4e-8 | ahead, 0.17×, p 3.4e-8 | level, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.56×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.61×, p 3.4e-8 | ahead, 0.61×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | level, 1.00× | behind, 1673.54× |
| sort | cpp | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 1.05× | behind, 1297.66× |
| sort | rust | 20 | behind, 1.18×, p 3.4e-8 | behind, 1.17×, p 3.4e-8 | behind, 1.14× | behind, 80.86× |
| tally | c-wasm | 20 | ahead, 0.36×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.05× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.36×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | behind, 1.05× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.29×, p 3.4e-8 | ahead, 0.29×, p 3.4e-8 | behind, 1.06× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.49×, p 3.4e-8 | ahead, 0.49×, p 3.4e-8 | behind, 7.91× | behind, 1673.58× |
| tally | cpp | 20 | ahead, 0.51×, p 3.4e-8 | ahead, 0.50×, p 3.4e-8 | behind, 4.00× | behind, 1304.40× |
| tally | rust | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.45×, p 3.4e-8 | behind, 8.03× | behind, 78.71× |
| server | c-wasm | 177 | level, 1.00×, p 0.066 | ahead, 0.94×, p 1.0e-40 | level, 1.04× | ahead, 0.12× |
| server | cpp-wasm | 160 | level, 1.00×, p 0.001 | ahead, 0.93×, p 3.3e-38 | level, 1.03× | ahead, 0.06× |
| server | rust-wasm | 177 | level, 1.00×, p 0.086 | ahead, 0.95×, p 2.4e-37 | level, 1.03× | ahead, 0.03× |
| server | c | 20 | behind, 1.07×, p 3.9e-8 | behind, 1.07×, p 6.2e-8 | behind, 15.28× | behind, 1304.26× |
| server | cpp | 20 | behind, 1.06×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 5.52× | behind, 1662.37× |
| server | rust | 20 | ahead, 0.94×, p 3.4e-8 | behind, 1.84×, p 3.4e-8 | behind, 7.07× | behind, 75.51× |
| files | c-wasm | 20 | ahead, 0.85×, p 6.0e-7 | ahead, 0.85×, p 2.0e-6 | level, 1.02× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.90×, p 6.0e-7 | ahead, 0.90×, p 3.8e-6 | level, 1.03× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.84×, p 6.9e-7 | ahead, 0.84×, p 1.5e-6 | level, 1.02× | ahead, 0.03× |
| files | c | 20 | behind, 2.11×, p 3.4e-8 | behind, 2.08×, p 3.4e-8 | behind, 10.72× | behind, 1673.39× |
| files | cpp | 20 | behind, 2.09×, p 3.4e-8 | behind, 2.07×, p 3.4e-8 | behind, 10.72× | behind, 1673.39× |
| files | rust | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.72×, p 3.4e-8 | behind, 8.86× | behind, 81.20× |
| readings | c-wasm | 20 | ahead, 0.85×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 1.03× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.89×, p 3.4e-8 | ahead, 0.89×, p 3.4e-8 | level, 1.02× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.86×, p 3.4e-8 | ahead, 0.86×, p 3.4e-8 | level, 1.01× | ahead, 0.02× |
| readings | c | 20 | behind, 3.40×, p 3.4e-8 | behind, 3.38×, p 3.4e-8 | behind, 10.64× | behind, 1673.36× |
| readings | cpp | 20 | behind, 3.09×, p 3.4e-8 | behind, 3.07×, p 3.4e-8 | behind, 4.60× | behind, 1673.36× |
| readings | rust | 20 | behind, 3.38×, p 3.4e-8 | behind, 3.36×, p 3.4e-8 | behind, 9.54× | behind, 80.87× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 320 | 320 | 20 | 20 | 20 | 20 |
| CPU time | 2.20 ms | 2.05 ms | 2.06 ms | 2.64 ms | 426 µs | 431 µs | 604 µs |
| wall time | 2.03 ms | 2.00 ms | 2.02 ms | 2.53 ms | 507 µs | 512 µs | 685 µs |
| peak memory | 11.4 MiB | 10.4 MiB | 10.5 MiB | 10.6 MiB | 1.22 MiB | 1.22 MiB | 1.83 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 14.0 KiB | 14.0 KiB | 279 KiB |

Its rounds took 8 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 2.42 ms | 2.57 ms | 2.58 ms | 2.82 ms | 480 µs | 467 µs | 624 µs |
| wall time | 2.15 ms | 2.41 ms | 2.42 ms | 2.67 ms | 555 µs | 545 µs | 701 µs |
| peak memory | 11.4 MiB | 10.6 MiB | 10.7 MiB | 10.7 MiB | 1.72 MiB | 1.60 MiB | 1.85 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 14.1 KiB | 14.1 KiB | 288 KiB |

Its rounds took 1 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 2.43 ms | 2.58 ms | 2.60 ms | 2.93 ms | 550 µs | 555 µs | 619 µs |
| wall time | 2.12 ms | 2.42 ms | 2.43 ms | 2.74 ms | 632 µs | 637 µs | 703 µs |
| peak memory | 11.4 MiB | 10.7 MiB | 10.6 MiB | 10.9 MiB | 2.02 MiB | 1.94 MiB | 1.86 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 3 seconds.

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
| counted runs | 320 | 320 | 320 | 40 | 20 | 20 | 20 |
| CPU time | 3.80 ms | 3.94 ms | 4.00 ms | 4.30 ms | 1.40 ms | 1.38 ms | 1.55 ms |
| wall time | 4.03 ms | 4.04 ms | 4.10 ms | 4.74 ms | 1.63 ms | 1.61 ms | 1.80 ms |
| peak memory | 13.9 MiB | 12.1 MiB | 12.0 MiB | 13.6 MiB | 2.35 MiB | 2.38 MiB | 2.83 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 287 KiB |

Its rounds took 14 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 12.0 ms | 14.4 ms | 37.5 ms | 12.8 ms | 6.77 ms | 6.79 ms | 6.62 ms |
| wall time | 11.7 ms | 14.3 ms | 37.6 ms | 12.7 ms | 6.87 ms | 6.90 ms | 6.71 ms |
| peak memory | 17.3 MiB | 16.4 MiB | 17.0 MiB | 16.7 MiB | 1.35 MiB | 1.35 MiB | 1.97 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 282 KiB |

Its rounds took 5 seconds.

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
| counted runs | 2000 | 640 | 2000 | 80 | 20 | 20 | 20 |
| CPU time | 2.98 ms | 2.88 ms | 2.90 ms | 3.27 ms | 1000 µs | 1000 µs | 1.20 ms |
| wall time | 2.93 ms | 2.83 ms | 2.88 ms | 3.32 ms | 1.08 ms | 1.08 ms | 1.30 ms |
| peak memory | 12.2 MiB | 11.0 MiB | 11.0 MiB | 11.3 MiB | 2.25 MiB | 2.22 MiB | 2.70 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 45 seconds.

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
| CPU time | 20.1 ms | 26.6 ms | 179 ms | 25.6 ms | 18.2 ms | 24.6 ms | 14.7 ms |
| wall time | 19.8 ms | 26.7 ms | 179 ms | 28.1 ms | 18.3 ms | 24.6 ms | 14.8 ms |
| peak memory | 17.3 MiB | 16.6 MiB | 17.2 MiB | 16.8 MiB | 1.73 MiB | 1.73 MiB | 1.98 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 9 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 29.6 ms | 45.3 ms | 39.1 ms | 41.3 ms | 25.0 ms | 25.2 ms | 25.1 ms |
| wall time | 29.2 ms | 45.2 ms | 38.9 ms | 41.1 ms | 25.1 ms | 25.3 ms | 25.2 ms |
| peak memory | 17.7 MiB | 17.1 MiB | 17.2 MiB | 17.4 MiB | 1.73 MiB | 1.73 MiB | 1.97 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 7 seconds.

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
| CPU time | 34.6 ms | 45.2 ms | 45.2 ms | 50.9 ms | 12.6 ms | 12.6 ms | 12.1 ms |
| wall time | 34.3 ms | 45.0 ms | 45.1 ms | 50.7 ms | 12.7 ms | 12.7 ms | 12.2 ms |
| peak memory | 17.8 MiB | 17.2 MiB | 17.2 MiB | 17.3 MiB | 1.76 MiB | 1.76 MiB | 1.97 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 14.1 KiB | 14.1 KiB | 285 KiB |

Its rounds took 6 seconds.

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
| CPU time | 257 ms | 1.54 s | 458 ms | 422 ms | 404 ms | 351 ms | 219 ms |
| wall time | 257 ms | 1.54 s | 458 ms | 422 ms | 404 ms | 352 ms | 219 ms |
| peak memory | 87.3 MiB | 86.4 MiB | 94.6 MiB | 94.6 MiB | 87.2 MiB | 83.3 MiB | 76.8 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 292 KiB |

Its rounds took 87 seconds.

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
| CPU time | 103 ms | 282 ms | 287 ms | 349 ms | 210 ms | 203 ms | 229 ms |
| wall time | 102 ms | 282 ms | 287 ms | 350 ms | 210 ms | 203 ms | 229 ms |
| peak memory | 18.6 MiB | 17.7 MiB | 17.7 MiB | 17.6 MiB | 2.35 MiB | 4.65 MiB | 2.32 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 300 KiB |

Its rounds took 40 seconds.

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
| counted runs | 177 | 177 | 160 | 177 | 20 | 20 | 20 |
| CPU time | 684 ms | 686 ms | 687 ms | 685 ms | 638 ms | 643 ms | 724 ms |
| wall time | 686 ms | 726 ms | 740 ms | 725 ms | 640 ms | 645 ms | 371 ms |
| peak memory | 17.8 MiB | 17.0 MiB | 17.2 MiB | 17.3 MiB | 1.16 MiB | 3.22 MiB | 2.52 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 18.1 KiB | 14.2 KiB | 313 KiB |

Its rounds took 618 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 51.5 ms | 60.4 ms | 57.2 ms | 61.3 ms | 24.5 ms | 24.6 ms | 18.7 ms |
| wall time | 51.1 ms | 60.4 ms | 57.0 ms | 61.1 ms | 24.6 ms | 24.7 ms | 18.8 ms |
| peak memory | 18.5 MiB | 18.1 MiB | 18.0 MiB | 18.1 MiB | 1.73 MiB | 1.73 MiB | 2.09 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 8 seconds.

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
| CPU time | 81.1 ms | 95.1 ms | 91.1 ms | 93.9 ms | 23.8 ms | 26.2 ms | 24.0 ms |
| wall time | 80.9 ms | 94.9 ms | 90.9 ms | 93.7 ms | 23.9 ms | 26.3 ms | 24.1 ms |
| peak memory | 17.7 MiB | 17.2 MiB | 17.3 MiB | 17.5 MiB | 1.66 MiB | 3.85 MiB | 1.86 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 14.1 KiB | 14.1 KiB | 292 KiB |

Its rounds took 11 seconds.

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
| empty | 160, 160 in 2 s | 2.34 ms against 2.13 ms: +192 µs (+152 µs to +231 µs) | 2.17 ms against 2.08 ms: +54.5 µs (+21.9 µs to +93.2 µs) | 11.4 MiB against 10.5 MiB: +924 KiB (+872 KiB to +980 KiB) |
| add | 160, 160 in 2 s | 2.34 ms against 2.15 ms: +178 µs (+144 µs to +212 µs) | 2.12 ms against 2.08 ms: +42.6 µs (+7.63 µs to +79.6 µs) | 11.4 MiB against 10.6 MiB: +784 KiB (+728 KiB to +844 KiB) |
| hypotenuse | 160, 160 in 4 s | 2.34 ms against 2.15 ms: +192 µs (+162 µs to +224 µs) | 2.10 ms against 2.07 ms: +28.6 µs (-2.67 µs to +61.0 µs) | 11.4 MiB against 10.6 MiB: +820 KiB (+768 KiB to +872 KiB) |
| upcase | 160, 160 in 3 s | 3.84 ms against 3.59 ms: +273 µs (+205 µs to +344 µs) | 3.71 ms against 3.62 ms: +77.2 µs (-38.9 µs to +196 µs) | 12.8 MiB against 11.8 MiB: +944 KiB (+808 KiB to +1.07 MiB) |
| upcase-stdin | 160, 160 in 6 s | 12.0 ms against 13.8 ms: -786 µs (-1.74 ms to -384 µs) | 11.7 ms against 13.7 ms: -1.10 ms (-1.90 ms to -577 µs) | 17.2 MiB against 16.6 MiB: +624 KiB (+584 KiB to +668 KiB) |
| shout | 160, 160 in 3 s | 2.96 ms against 2.71 ms: +270 µs (+232 µs to +311 µs) | 2.95 ms against 2.79 ms: +133 µs (+50.2 µs to +221 µs) | 12.2 MiB against 11.1 MiB: +1.14 MiB (+1.05 MiB to +1.23 MiB) |
| wc | 160, 160 in 9 s | 19.9 ms against 19.9 ms: -11.0 µs (-176 µs to +127 µs) | 19.5 ms against 19.7 ms: -211 µs (-397 µs to -57.8 µs) | 17.2 MiB against 16.6 MiB: +644 KiB (+604 KiB to +680 KiB) |
| histogram | 160, 160 in 12 s | 29.4 ms against 29.4 ms: -331 µs (-700 µs to -35.0 µs) | 29.1 ms against 29.3 ms: -532 µs (-950 µs to -207 µs) | 17.7 MiB against 17.1 MiB: +640 KiB (+592 KiB to +688 KiB) |
| crc32c | 160, 160 in 14 s | 34.8 ms against 35.4 ms: -999 µs (-1.89 ms to -538 µs) | 34.5 ms against 35.2 ms: -1.12 ms (-1.96 ms to -666 µs) | 17.8 MiB against 17.1 MiB: +676 KiB (+636 KiB to +708 KiB) |
| sort | 112, 112 in 60 s | 258 ms against 260 ms: -1.38 ms (-3.81 ms to +907 µs) | 257 ms against 260 ms: -1.72 ms (-4.20 ms to +546 µs) | 87.2 MiB against 86.5 MiB: +768 KiB (+720 KiB to +812 KiB) |
| tally | 160, 160 in 37 s | 103 ms against 103 ms: -741 µs (-1.41 ms to -277 µs) | 102 ms against 103 ms: -1.06 ms (-1.73 ms to -581 µs) | 18.7 MiB against 17.8 MiB: +848 KiB (+804 KiB to +892 KiB) |
| server | 41, 41 in 61 s | 685 ms against 686 ms: +113 µs (-3.62 ms to +3.46 ms) | 685 ms against 688 ms: -1.30 ms (-5.00 ms to +2.65 ms) | 17.8 MiB against 16.9 MiB: +772 KiB (+692 KiB to +868 KiB) |
| files | 160, 160 in 19 s | 51.5 ms against 52.0 ms: -593 µs (-947 µs to -316 µs) | 51.1 ms against 51.9 ms: -789 µs (-1.13 ms to -502 µs) | 18.6 MiB against 18.1 MiB: +480 KiB (+432 KiB to +520 KiB) |
| readings | 160, 160 in 28 s | 81.2 ms against 82.7 ms: -1.79 ms (-2.12 ms to -1.52 ms) | 80.8 ms against 82.8 ms: -2.01 ms (-2.35 ms to -1.73 ms) | 17.8 MiB against 17.1 MiB: +644 KiB (+608 KiB to +684 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
