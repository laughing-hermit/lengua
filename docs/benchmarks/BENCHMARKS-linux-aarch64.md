# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 43813aef065cae8d91368d4891f6823f48ad9d82, in https://github.com/laughing-hermit/lengua/actions/runs/43131768685. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records lengua's idea and the module, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (5.01 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ≈ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-aarch64 hypotenuse c-wasm memory: lengua is level now, and KNOWN_BEHIND line 221 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 hypotenuse cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 222 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 wc c cpu: lengua is level now, and KNOWN_BEHIND line 273 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-aarch64 wc c wall: lengua is level now, and KNOWN_BEHIND line 274 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.05× | 2.74× | 5.25× | 77.76× |
| add | 2.99× | 2.73× | 3.31× | 77.78× |
| hypotenuse | 2.76× | 2.58× | 3.11× | 77.78× |
| upcase | 1.83× | 1.79× | 2.81× | 77.79× |
| upcase-stdin | 1.22× | 1.22× | 7.76× | 77.78× |
| shout | 2.09× | 2.03× | 2.77× | 77.78× |
| wc | 1.04× | 1.05× | 5.22× | 77.79× |
| histogram | 1.07× | 1.08× | 5.13× | 77.78× |
| crc32c | 3.36× | 3.34× | 5.13× | 77.78× |
| sort | 1.17× | 1.17× | 1.01× | 77.82× |
| tally | 0.59× | 0.59× | 4.22× | 77.83× |
| server | 1.29× | 1.44× | 10.90× | 77.79× |
| files | 3.05× | 3.03× | 5.98× | 77.78× |
| readings | 2.94× | 2.94× | 5.60× | 77.78× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 640 | level, 0.99×, p 1.7e-8 | level, 0.99×, p 3.8e-7 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 320 | level, 0.99×, p 2.7e-4 | level, 0.99×, p 9.0e-4 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.89×, p 4.9e-7 | ahead, 0.90×, p 2.6e-7 | level, 0.97× | ahead, 0.01× |
| empty | c | 20 | behind, 3.05×, p 3.4e-8 | behind, 2.74×, p 3.4e-8 | behind, 5.25× | behind, 77.76× |
| empty | cpp | 20 | behind, 3.00×, p 3.4e-8 | behind, 2.73×, p 3.4e-8 | behind, 5.25× | behind, 77.76× |
| empty | rust | 20 | behind, 2.28×, p 3.4e-8 | behind, 2.14×, p 3.4e-8 | behind, 3.05× | behind, 19.66× |
| add | c-wasm | 20 | level, 0.95×, p 7.4e-7 | level, 0.96×, p 3.8e-6 | level, 0.97× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.94×, p 2.4e-6 | ahead, 0.94×, p 8.0e-6 | level, 0.97× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.84×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 0.97× | ahead, 0.02× |
| add | c | 20 | behind, 2.99×, p 3.4e-8 | behind, 2.73×, p 3.4e-8 | behind, 3.31× | behind, 77.78× |
| add | cpp | 20 | behind, 2.96×, p 3.4e-8 | behind, 2.72×, p 3.4e-8 | behind, 3.31× | behind, 77.78× |
| add | rust | 20 | behind, 2.42×, p 3.4e-8 | behind, 2.26×, p 3.4e-8 | behind, 2.94× | behind, 15.79× |
| hypotenuse | c-wasm | 20 | ahead, 0.95×, p 8.5e-6 | level, 0.96×, p 1.2e-4 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | level, 0.96×, p 6.8e-5 | level, 0.96×, p 6.1e-4 | level, 1.00× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.84×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 0.99× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.76×, p 3.4e-8 | behind, 2.58×, p 3.4e-8 | behind, 3.11× | behind, 77.78× |
| hypotenuse | cpp | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.58×, p 3.4e-8 | behind, 3.11× | behind, 77.78× |
| hypotenuse | rust | 20 | behind, 2.44×, p 3.4e-8 | behind, 2.27×, p 3.4e-8 | behind, 3.03× | behind, 15.79× |
| upcase | c-wasm | 320 | level, 0.97×, p 1.6e-27 | level, 0.97×, p 4.6e-10 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 80 | level, 0.95×, p 6.6e-12 | level, 0.95×, p 1.9e-4 | level, 1.03× | ahead, 0.06× |
| upcase | rust-wasm | 80 | ahead, 0.88×, p 2.3e-25 | ahead, 0.89×, p 5.0e-8 | ahead, 0.90× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.83×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 2.81× | behind, 77.79× |
| upcase | cpp | 20 | behind, 1.79×, p 3.4e-8 | behind, 1.77×, p 3.4e-8 | behind, 2.81× | behind, 77.79× |
| upcase | rust | 20 | behind, 1.63×, p 3.4e-8 | behind, 1.60×, p 3.4e-8 | behind, 2.39× | behind, 15.79× |
| upcase-stdin | c-wasm | 160 | level, 1.01×, p 2.5e-6 | level, 1.01×, p 1.0e-4 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.003 | level, 1.00×, p 7.4e-5 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 640 | level, 1.00×, p 8.4e-7 | level, 1.00×, p 7.9e-5 | level, 0.98× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.22×, p 3.4e-8 | behind, 1.22×, p 3.4e-8 | behind, 7.76× | behind, 77.78× |
| upcase-stdin | cpp | 20 | behind, 1.20×, p 3.4e-8 | behind, 1.20×, p 3.4e-8 | behind, 7.76× | behind, 77.78× |
| upcase-stdin | rust | 20 | behind, 1.15×, p 3.4e-8 | behind, 1.14×, p 3.4e-8 | behind, 4.60× | behind, 15.78× |
| shout | c-wasm | 2000 | level, 1.00×, p 0.088 | level, 1.00×, p 0.031 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.00×, p 0.087 | level, 1.00×, p 0.025 | level, 1.00× | ahead, 0.40× |
| shout | rust-wasm | 20 | ahead, 0.88×, p 5.3e-8 | ahead, 0.89×, p 6.3e-6 | level, 0.98× | ahead, 0.03× |
| shout | c | 20 | behind, 2.05×, p 3.4e-8 | behind, 2.01×, p 3.4e-8 | behind, 2.77× | behind, 77.78× |
| shout | cpp | 20 | behind, 2.09×, p 3.4e-8 | behind, 2.03×, p 3.4e-8 | behind, 2.77× | behind, 77.78× |
| shout | rust | 20 | behind, 1.76×, p 3.4e-8 | behind, 1.72×, p 3.4e-8 | behind, 2.33× | behind, 15.78× |
| wc | c-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.68×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 1.00× | ahead, 0.04× |
| wc | c | 20 | level, 1.04×, p 3.4e-8 | level, 1.05×, p 3.4e-8 | behind, 5.22× | behind, 77.79× |
| wc | cpp | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.90×, p 3.4e-8 | behind, 5.22× | behind, 77.79× |
| wc | rust | 20 | ahead, 0.84×, p 3.4e-8 | ahead, 0.84×, p 3.4e-8 | behind, 4.67× | behind, 15.79× |
| histogram | c-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | rust-wasm | 20 | level, 0.96×, p 4.0e-7 | level, 0.96×, p 5.2e-7 | level, 0.98× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.07×, p 9.6e-8 | behind, 1.08×, p 8.3e-8 | behind, 5.13× | behind, 77.78× |
| histogram | cpp | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 5.13× | behind, 77.78× |
| histogram | rust | 20 | behind, 1.06×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 4.76× | behind, 15.79× |
| crc32c | c-wasm | 20 | ahead, 0.77×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.77×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.36×, p 3.4e-8 | behind, 2.35×, p 3.4e-8 | behind, 5.13× | behind, 77.78× |
| crc32c | cpp | 20 | behind, 2.35×, p 3.4e-8 | behind, 2.34×, p 3.4e-8 | behind, 5.13× | behind, 77.78× |
| crc32c | rust | 20 | behind, 3.36×, p 3.4e-8 | behind, 3.34×, p 3.4e-8 | behind, 4.62× | behind, 15.79× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.65×, p 3.4e-8 | ahead, 0.65×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | ahead, 0.89× | behind, 77.82× |
| sort | cpp | 20 | ahead, 0.75×, p 3.4e-8 | ahead, 0.75×, p 3.4e-8 | ahead, 0.93× | behind, 77.71× |
| sort | rust | 20 | behind, 1.17×, p 3.4e-8 | behind, 1.17×, p 3.4e-8 | level, 1.01× | behind, 15.79× |
| tally | c-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 4.02× | behind, 77.83× |
| tally | cpp | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | behind, 2.16× | behind, 77.83× |
| tally | rust | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | behind, 4.22× | behind, 15.79× |
| server | c-wasm | 269 | level, 0.99×, p 0.021 | ahead, 0.92×, p 2.0e-38 | level, 1.00× | ahead, 0.12× |
| server | cpp-wasm | 269 | level, 0.99×, p 0.004 | ahead, 0.92×, p 3.3e-42 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 80 | level, 0.97×, p 0.001 | ahead, 0.92×, p 1.5e-14 | level, 1.00× | ahead, 0.03× |
| server | c | 20 | behind, 1.29×, p 3.4e-8 | behind, 1.28×, p 3.4e-8 | behind, 10.90× | behind, 77.79× |
| server | cpp | 20 | behind, 1.28×, p 3.4e-8 | behind, 1.28×, p 3.4e-8 | behind, 3.03× | behind, 77.68× |
| server | rust | 20 | ahead, 0.72×, p 3.4e-8 | behind, 1.44×, p 3.4e-8 | behind, 3.79× | behind, 15.78× |
| files | c-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| files | c | 20 | behind, 2.51×, p 3.4e-8 | behind, 2.51×, p 3.4e-8 | behind, 5.98× | behind, 77.78× |
| files | cpp | 20 | behind, 2.52×, p 3.4e-8 | behind, 2.51×, p 3.4e-8 | behind, 5.98× | behind, 77.78× |
| files | rust | 20 | behind, 3.05×, p 3.4e-8 | behind, 3.03×, p 3.4e-8 | behind, 5.06× | behind, 15.79× |
| readings | c-wasm | 20 | level, 0.96×, p 3.4e-8 | level, 0.96×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| readings | cpp-wasm | 20 | level, 0.99×, p 4.6e-7 | level, 0.99×, p 6.3e-6 | level, 0.98× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.94×, p 3.4e-8 | level, 0.98× | ahead, 0.02× |
| readings | c | 20 | behind, 2.94×, p 3.4e-8 | behind, 2.94×, p 3.4e-8 | behind, 5.60× | behind, 77.78× |
| readings | cpp | 20 | behind, 2.85×, p 3.4e-8 | behind, 2.85×, p 3.4e-8 | behind, 2.39× | behind, 77.78× |
| readings | rust | 20 | behind, 2.88×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 5.14× | behind, 15.79× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 640 | 320 | 20 | 20 | 20 | 20 |
| CPU time | 1.53 ms | 1.54 ms | 1.55 ms | 1.79 ms | 522 µs | 530 µs | 696 µs |
| wall time | 1.63 ms | 1.65 ms | 1.66 ms | 1.89 ms | 622 µs | 623 µs | 796 µs |
| peak memory | 4.47 MiB | 4.47 MiB | 4.47 MiB | 4.59 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 8,430 | 8,592 | 8,635 | 9,503 | 1,480 | 1,514 | 2,560 |

Its rounds took 8 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 1.65 ms | 1.74 ms | 1.75 ms | 1.97 ms | 553 µs | 559 µs | 682 µs |
| wall time | 1.76 ms | 1.84 ms | 1.87 ms | 2.07 ms | 645 µs | 647 µs | 779 µs |
| peak memory | 4.47 MiB | 4.59 MiB | 4.59 MiB | 4.62 MiB | 1.35 MiB | 1.35 MiB | 1.52 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 8,936 | 9,747 | 9,710 | 10,758 | 1,665 | 1,635 | 2,636 |

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
| CPU time | 1.69 ms | 1.79 ms | 1.77 ms | 2.03 ms | 613 µs | 616 µs | 694 µs |
| wall time | 1.81 ms | 1.89 ms | 1.88 ms | 2.14 ms | 700 µs | 701 µs | 797 µs |
| peak memory | 4.59 MiB | 4.59 MiB | 4.59 MiB | 4.63 MiB | 1.48 MiB | 1.48 MiB | 1.52 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,150 | 9,731 | 9,784 | 10,880 | 2,060 | 1,978 | 2,599 |

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
| counted runs | 320 | 320 | 80 | 80 | 20 | 20 | 20 |
| CPU time | 2.56 ms | 2.65 ms | 2.66 ms | 2.86 ms | 1.39 ms | 1.42 ms | 1.56 ms |
| wall time | 2.78 ms | 2.86 ms | 2.87 ms | 3.07 ms | 1.56 ms | 1.58 ms | 1.75 ms |
| peak memory | 5.47 MiB | 5.47 MiB | 5.47 MiB | 6.27 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 10,306 | 11,043 | 10,738 | 12,004 | 2,089 | 2,151 | 3,172 |

Its rounds took 6 seconds.

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
| counted runs | 2000 | 160 | 2000 | 640 | 20 | 20 | 20 |
| CPU time | 12.6 ms | 12.4 ms | 12.6 ms | 12.6 ms | 10.2 ms | 10.4 ms | 10.9 ms |
| wall time | 12.8 ms | 12.6 ms | 12.7 ms | 12.8 ms | 10.3 ms | 10.5 ms | 11.0 ms |
| peak memory | 7.58 MiB | 7.58 MiB | 7.58 MiB | 7.71 MiB | 1000 KiB | 1000 KiB | 1.65 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 149,007 | 156,703 | 159,854 | 165,280 | 25,857 | 26,281 | 47,697 |

Its rounds took 82 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 20 | 20 | 20 | 20 |
| CPU time | 2.13 ms | 2.13 ms | 2.13 ms | 2.43 ms | 1.04 ms | 1.02 ms | 1.21 ms |
| wall time | 2.25 ms | 2.25 ms | 2.24 ms | 2.56 ms | 1.13 ms | 1.12 ms | 1.32 ms |
| peak memory | 5.47 MiB | 5.47 MiB | 5.47 MiB | 5.59 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,420 | 9,589 | 9,569 | 11,014 | 1,910 | 1,886 | 2,969 |

Its rounds took 35 seconds.

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
| CPU time | 18.8 ms | 27.8 ms | 186 ms | 27.5 ms | 18.0 ms | 20.8 ms | 22.4 ms |
| wall time | 18.9 ms | 28.0 ms | 186 ms | 27.7 ms | 18.1 ms | 20.9 ms | 22.6 ms |
| peak memory | 7.71 MiB | 7.71 MiB | 7.71 MiB | 7.73 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 126,948 | 136,640 | 123,682 | 146,112 | 25,182 | 26,749 | 42,736 |

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
| CPU time | 38.2 ms | 69.9 ms | 69.9 ms | 39.9 ms | 35.6 ms | 35.6 ms | 36.0 ms |
| wall time | 38.4 ms | 70.0 ms | 70.0 ms | 40.1 ms | 35.7 ms | 35.7 ms | 36.1 ms |
| peak memory | 7.58 MiB | 7.71 MiB | 7.71 MiB | 7.73 MiB | 1.48 MiB | 1.48 MiB | 1.59 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 181,415 | 186,596 | 185,839 | 202,686 | 86,750 | 84,368 | 106,003 |

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
| CPU time | 35.7 ms | 46.1 ms | 46.1 ms | 49.1 ms | 15.2 ms | 15.2 ms | 10.6 ms |
| wall time | 35.9 ms | 46.2 ms | 46.3 ms | 49.3 ms | 15.3 ms | 15.3 ms | 10.7 ms |
| peak memory | 7.58 MiB | 7.70 MiB | 7.71 MiB | 7.76 MiB | 1.48 MiB | 1.48 MiB | 1.64 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 302,196 | 226,402 | 233,744 | 238,812 | 27,310 | 25,925 | 27,208 |

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
| CPU time | 274 ms | 1.37 s | 468 ms | 419 ms | 382 ms | 367 ms | 234 ms |
| wall time | 274 ms | 1.37 s | 468 ms | 419 ms | 382 ms | 367 ms | 235 ms |
| peak memory | 77.1 MiB | 77.1 MiB | 85.1 MiB | 85.2 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,254,877 | 30,349,907 | 12,285,575 | 12,081,464 | 14,534,778 | 15,502,777 | 12,439,668 |

Its rounds took 83 seconds.

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
| CPU time | 139 ms | 289 ms | 288 ms | 405 ms | 235 ms | 244 ms | 251 ms |
| wall time | 139 ms | 289 ms | 288 ms | 405 ms | 236 ms | 244 ms | 251 ms |
| peak memory | 8.46 MiB | 8.33 MiB | 8.33 MiB | 8.11 MiB | 2.11 MiB | 3.92 MiB | 2.00 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,836,725 | 8,661,554 | 8,645,778 | 8,145,552 | 8,530,935 | 8,388,955 | 11,203,554 |

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
| counted runs | 269 | 269 | 269 | 80 | 20 | 20 | 20 |
| CPU time | 551 ms | 557 ms | 557 ms | 566 ms | 421 ms | 422 ms | 748 ms |
| wall time | 553 ms | 602 ms | 599 ms | 599 ms | 426 ms | 424 ms | 376 ms |
| peak memory | 7.71 MiB | 7.71 MiB | 7.71 MiB | 7.73 MiB | 724 KiB | 2.54 MiB | 2.04 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,572,778 | 2,533,664 | 2,694,198 | 2,404,222 | 150,136 | 214,959 | 534,723 |

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
| CPU time | 43.5 ms | 47.2 ms | 47.1 ms | 47.2 ms | 17.3 ms | 17.3 ms | 14.3 ms |
| wall time | 43.6 ms | 47.4 ms | 47.3 ms | 47.4 ms | 17.4 ms | 17.4 ms | 14.4 ms |
| peak memory | 8.08 MiB | 8.08 MiB | 8.08 MiB | 8.25 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,516,882 | 7,589,543 | 7,578,576 | 7,612,405 | 11,228 | 9,775 | 10,313 |

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
| CPU time | 94.4 ms | 98.3 ms | 95.6 ms | 101 ms | 32.1 ms | 33.1 ms | 32.8 ms |
| wall time | 94.6 ms | 98.5 ms | 95.7 ms | 101 ms | 32.2 ms | 33.2 ms | 32.9 ms |
| peak memory | 7.57 MiB | 7.71 MiB | 7.71 MiB | 7.75 MiB | 1.35 MiB | 3.16 MiB | 1.47 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,975 | 10,720 | 10,534 | 11,756 | 1,738 | 13,437 | 2,779 |

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
| empty | 160, 160 in 4 s | 1.55 ms against 1.46 ms: +88.0 µs (+76.0 µs to +99.0 µs) | 1.65 ms against 1.56 ms: +85.5 µs (+72.4 µs to +98.2 µs) | 4.47 MiB against 4.34 MiB: +128 KiB (+128 KiB to +128 KiB) |
| add | 160, 160 in 4 s | 1.62 ms against 1.48 ms: +137 µs (+127 µs to +147 µs) | 1.73 ms against 1.59 ms: +141 µs (+129 µs to +152 µs) | 4.47 MiB against 4.34 MiB: +128 KiB (+128 KiB to +128 KiB) |
| hypotenuse | 160, 160 in 4 s | 1.64 ms against 1.48 ms: +155 µs (+143 µs to +166 µs) | 1.74 ms against 1.59 ms: +156 µs (+143 µs to +169 µs) | 4.59 MiB against 4.34 MiB: +252 KiB (+252 KiB to +256 KiB) |
| upcase | 160, 160 in 5 s | 2.61 ms against 2.46 ms: +151 µs (+128 µs to +174 µs) | 2.86 ms against 2.65 ms: +203 µs (+166 µs to +243 µs) | 5.47 MiB against 5.22 MiB: +256 KiB (+256 KiB to +256 KiB) |
| upcase-stdin | 160, 160 in 13 s | 12.7 ms against 12.6 ms: +119 µs (+66.0 µs to +170 µs) | 12.9 ms against 12.7 ms: +149 µs (+92.4 µs to +204 µs) | 7.58 MiB against 7.58 MiB: +0 B (+0 B to +0 B) |
| shout | 160, 160 in 5 s | 2.15 ms against 2.01 ms: +143 µs (+127 µs to +159 µs) | 2.27 ms against 2.12 ms: +156 µs (+135 µs to +176 µs) | 5.47 MiB against 5.09 MiB: +384 KiB (+380 KiB to +384 KiB) |
| wc | 160, 160 in 48 s | 18.8 ms against 18.6 ms: +195 µs (+132 µs to +254 µs) | 19.0 ms against 18.8 ms: +185 µs (+117 µs to +247 µs) | 7.71 MiB against 7.58 MiB: +128 KiB (+128 KiB to +128 KiB) |
| histogram | 160, 160 in 44 s | 38.2 ms against 38.0 ms: +184 µs (+133 µs to +233 µs) | 38.3 ms against 38.2 ms: +169 µs (+116 µs to +222 µs) | 7.58 MiB against 7.58 MiB: -4.00 KiB (-4.00 KiB to -4.00 KiB) |
| crc32c | 160, 160 in 37 s | 35.7 ms against 35.6 ms: +108 µs (+58.0 µs to +156 µs) | 35.8 ms against 35.7 ms: +106 µs (+52.9 µs to +155 µs) | 7.58 MiB against 7.58 MiB: +0 B (-4.00 KiB to +0 B) |
| sort | 22, 22 in 63 s | 275 ms against 274 ms: -572 µs (-3.07 ms to +1.74 ms) | 275 ms against 275 ms: -628 µs (-3.13 ms to +1.68 ms) | 77.1 MiB against 77.1 MiB: +0 B (+0 B to +8.00 KiB) |
| tally | 47, 47 in 60 s | 138 ms against 138 ms: +75.0 µs (-468 µs to +631 µs) | 138 ms against 138 ms: +122 µs (-443 µs to +699 µs) | 8.46 MiB against 8.45 MiB: +8.00 KiB (+4.00 KiB to +8.00 KiB) |
| server | 20, 20 in 64 s | 555 ms against 550 ms: -2.48 ms (-24.1 ms to +20.8 ms) | 557 ms against 553 ms: -2.97 ms (-25.2 ms to +20.7 ms) | 7.71 MiB against 7.69 MiB: +94.0 KiB (+12.0 KiB to +128 KiB) |
| files | 160, 160 in 40 s | 43.5 ms against 43.3 ms: +156 µs (+100 µs to +212 µs) | 43.6 ms against 43.5 ms: +161 µs (+99.8 µs to +224 µs) | 8.08 MiB against 8.08 MiB: +0 B (+0 B to +0 B) |
| readings | 120, 120 in 60 s | 94.3 ms against 94.4 ms: -35.0 µs (-95.0 µs to +20.0 µs) | 94.5 ms against 94.5 ms: -42.4 µs (-106 µs to +16.6 µs) | 7.57 MiB against 7.58 MiB: -8.00 KiB (-8.00 KiB to -4.00 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 1.47 ms, 1.05× | 1.58 ms, 1.05× | 4.34 MiB, 1.03× |
| empty | cpp-wasm | 160 | 1.49 ms, 1.04× | 1.59 ms, 1.04× | 4.34 MiB, 1.03× |
| empty | rust-wasm | 160 | 1.54 ms, 1.01× | 1.65 ms, 1.00× | 4.47 MiB, 1.00× |
| add | c-wasm | 160 | 1.55 ms, 1.05× | 1.65 ms, 1.05× | 4.47 MiB, 1.00× |
| add | cpp-wasm | 160 | 1.54 ms, 1.05× | 1.65 ms, 1.05× | 4.34 MiB, 1.03× |
| add | rust-wasm | 160 | 1.71 ms, 0.95× | 1.81 ms, 0.96× | 4.45 MiB, 1.01× |
| hypotenuse | c-wasm | 160 | 1.54 ms, 1.07× | 1.64 ms, 1.06× | 4.34 MiB, 1.06× |
| hypotenuse | cpp-wasm | 160 | 1.54 ms, 1.07× | 1.64 ms, 1.06× | 4.34 MiB, 1.06× |
| hypotenuse | rust-wasm | 160 | 1.70 ms, 0.96× | 1.81 ms, 0.97× | 4.45 MiB, 1.03× |
| upcase | c-wasm | 160 | 2.47 ms, 1.06× | 2.69 ms, 1.06× | 5.22 MiB, 1.05× |
| upcase | cpp-wasm | 160 | 2.49 ms, 1.05× | 2.72 ms, 1.05× | 5.22 MiB, 1.05× |
| upcase | rust-wasm | 160 | 2.65 ms, 0.99× | 2.88 ms, 0.99× | 5.34 MiB, 1.02× |
| upcase-stdin | c-wasm | 160 | 12.5 ms, 1.02× | 12.7 ms, 1.02× | 7.58 MiB, 1.00× |
| upcase-stdin | cpp-wasm | 160 | 12.6 ms, 1.01× | 12.8 ms, 1.01× | 7.58 MiB, 1.00× |
| upcase-stdin | rust-wasm | 160 | 12.6 ms, 1.01× | 12.7 ms, 1.01× | 7.71 MiB, 0.98× |
| shout | c-wasm | 160 | 2.03 ms, 1.06× | 2.15 ms, 1.06× | 5.22 MiB, 1.05× |
| shout | cpp-wasm | 160 | 2.04 ms, 1.06× | 2.16 ms, 1.05× | 5.22 MiB, 1.05× |
| shout | rust-wasm | 160 | 2.20 ms, 0.98× | 2.33 ms, 0.98× | 5.30 MiB, 1.03× |
| wc | c-wasm | 160 | 27.7 ms, 0.68× | 27.8 ms, 0.68× | 7.57 MiB, 1.02× |
| wc | cpp-wasm | 160 | 185 ms, 0.10× | 186 ms, 0.10× | 7.57 MiB, 1.02× |
| wc | rust-wasm | 160 | 27.3 ms, 0.69× | 27.5 ms, 0.69× | 7.68 MiB, 1.00× |
| histogram | c-wasm | 160 | 69.8 ms, 0.55× | 70.0 ms, 0.55× | 7.57 MiB, 1.00× |
| histogram | cpp-wasm | 160 | 69.8 ms, 0.55× | 70.0 ms, 0.55× | 7.57 MiB, 1.00× |
| histogram | rust-wasm | 160 | 39.7 ms, 0.96× | 39.9 ms, 0.96× | 7.69 MiB, 0.99× |
| crc32c | c-wasm | 160 | 45.9 ms, 0.78× | 46.0 ms, 0.78× | 7.57 MiB, 1.00× |
| crc32c | cpp-wasm | 160 | 46.0 ms, 0.78× | 46.2 ms, 0.78× | 7.57 MiB, 1.00× |
| crc32c | rust-wasm | 160 | 48.9 ms, 0.73× | 49.0 ms, 0.73× | 7.67 MiB, 0.99× |
| sort | c-wasm | 22 | 1.38 s, 0.20× | 1.38 s, 0.20× | 77.1 MiB, 1.00× |
| sort | cpp-wasm | 22 | 473 ms, 0.58× | 473 ms, 0.58× | 85.1 MiB, 0.91× |
| sort | rust-wasm | 22 | 417 ms, 0.66× | 417 ms, 0.66× | 85.2 MiB, 0.90× |
| tally | c-wasm | 47 | 289 ms, 0.48× | 289 ms, 0.48× | 8.20 MiB, 1.03× |
| tally | cpp-wasm | 47 | 287 ms, 0.48× | 288 ms, 0.48× | 8.33 MiB, 1.02× |
| tally | rust-wasm | 47 | 404 ms, 0.34× | 405 ms, 0.34× | 8.18 MiB, 1.03× |
| server | c-wasm | 20 | 565 ms, 0.98× | 604 ms, 0.92× | 7.57 MiB, 1.02× |
| server | cpp-wasm | 20 | 562 ms, 0.99× | 598 ms, 0.93× | 7.58 MiB, 1.02× |
| server | rust-wasm | 20 | 566 ms, 0.98× | 592 ms, 0.94× | 7.68 MiB, 1.00× |
| files | c-wasm | 160 | 46.8 ms, 0.93× | 47.0 ms, 0.93× | 8.08 MiB, 1.00× |
| files | cpp-wasm | 160 | 46.8 ms, 0.93× | 47.0 ms, 0.93× | 8.08 MiB, 1.00× |
| files | rust-wasm | 160 | 46.9 ms, 0.93× | 47.1 ms, 0.93× | 8.20 MiB, 0.99× |
| readings | c-wasm | 120 | 98.3 ms, 0.96× | 98.4 ms, 0.96× | 7.71 MiB, 0.98× |
| readings | cpp-wasm | 120 | 95.3 ms, 0.99× | 95.5 ms, 0.99× | 7.71 MiB, 0.98× |
| readings | rust-wasm | 120 | 100 ms, 0.94× | 101 ms, 0.94× | 7.69 MiB, 0.98× |
