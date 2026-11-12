# lengua against C, C++ and Rust on linux-aarch64

Measured on linux-aarch64 (CPU implementer 0x41, part 0xd49, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 610aaf5c622249762f3d80df3fa0947c3f71739e, in https://github.com/laughing-hermit/lengua/actions/runs/43191148225. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records the module and lengua's idea, with the line that names an outside module added, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (5.07 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ≈ ≈ ✓ | ✓ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
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

The run agrees with KNOWN_BEHIND.

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.00× | 2.70× | 5.10× | 78.73× |
| add | 3.02× | 2.72× | 3.21× | 78.75× |
| hypotenuse | 2.75× | 2.52× | 2.94× | 78.75× |
| upcase | 1.83× | 1.79× | 2.93× | 78.76× |
| upcase-stdin | 1.20× | 1.20× | 7.32× | 78.75× |
| shout | 2.02× | 1.95× | 2.70× | 78.75× |
| wc | 1.03× | 1.04× | 4.84× | 78.76× |
| histogram | 1.08× | 1.08× | 4.84× | 78.75× |
| crc32c | 3.29× | 3.26× | 4.84× | 78.75× |
| sort | 1.15× | 1.15× | 1.00× | 78.79× |
| tally | 0.59× | 0.59× | 4.07× | 78.80× |
| server | 1.25× | 1.43× | 10.28× | 78.76× |
| files | 3.22× | 3.20× | 5.61× | 78.76× |
| readings | 2.93× | 2.92× | 5.28× | 78.75× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 40 | level, 0.96×, p 1.2e-4 | level, 0.97×, p 7.6e-4 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 20 | level, 0.96×, p 9.5e-5 | level, 0.97×, p 2.1e-4 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.86×, p 1.5e-7 | ahead, 0.87×, p 2.3e-7 | level, 0.97× | ahead, 0.01× |
| empty | c | 20 | behind, 3.00×, p 3.4e-8 | behind, 2.70×, p 3.4e-8 | behind, 5.10× | behind, 78.73× |
| empty | cpp | 20 | behind, 2.99×, p 3.4e-8 | behind, 2.70×, p 3.4e-8 | behind, 5.10× | behind, 78.73× |
| empty | rust | 20 | behind, 2.26×, p 3.4e-8 | behind, 2.13×, p 3.4e-8 | behind, 2.97× | behind, 19.90× |
| add | c-wasm | 20 | ahead, 0.92×, p 1.7e-7 | ahead, 0.92×, p 3.5e-7 | level, 0.97× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.92×, p 2.0e-5 | ahead, 0.92×, p 2.6e-5 | level, 0.97× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.83×, p 3.4e-8 | level, 0.97× | ahead, 0.02× |
| add | c | 20 | behind, 2.97×, p 3.4e-8 | behind, 2.71×, p 3.4e-8 | behind, 3.21× | behind, 78.75× |
| add | cpp | 20 | behind, 3.02×, p 3.4e-8 | behind, 2.72×, p 3.4e-8 | behind, 3.21× | behind, 78.75× |
| add | rust | 20 | behind, 2.36×, p 3.4e-8 | behind, 2.19×, p 3.4e-8 | behind, 2.84× | behind, 15.98× |
| hypotenuse | c-wasm | 20 | ahead, 0.93×, p 1.5e-7 | ahead, 0.94×, p 2.3e-7 | level, 0.97× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | ahead, 0.92×, p 2.8e-7 | ahead, 0.93×, p 6.9e-7 | level, 0.97× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.83×, p 3.4e-8 | level, 0.96× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.73×, p 3.4e-8 | behind, 2.51×, p 3.4e-8 | behind, 2.94× | behind, 78.75× |
| hypotenuse | cpp | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.52×, p 3.4e-8 | behind, 2.85× | behind, 78.75× |
| hypotenuse | rust | 20 | behind, 2.43×, p 3.4e-8 | behind, 2.23×, p 3.4e-8 | behind, 2.83× | behind, 15.99× |
| upcase | c-wasm | 80 | ahead, 0.95×, p 2.0e-14 | level, 0.97×, p 2.2e-4 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 80 | ahead, 0.94×, p 2.6e-16 | level, 0.96×, p 4.6e-6 | level, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 20 | ahead, 0.90×, p 4.6e-8 | ahead, 0.92×, p 5.2e-5 | level, 0.96× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.83×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 2.93× | behind, 78.76× |
| upcase | cpp | 20 | behind, 1.83×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 2.93× | behind, 78.76× |
| upcase | rust | 20 | behind, 1.64×, p 3.4e-8 | behind, 1.61×, p 3.4e-8 | behind, 2.49× | behind, 15.99× |
| upcase-stdin | c-wasm | 40 | level, 1.02×, p 5.6e-4 | level, 1.02×, p 7.4e-4 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 80 | level, 1.02×, p 1.3e-5 | level, 1.02×, p 9.0e-5 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.051 | level, 1.00×, p 0.160 | level, 0.98× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.19×, p 3.4e-8 | behind, 1.19×, p 3.4e-8 | behind, 7.32× | behind, 78.75× |
| upcase-stdin | cpp | 20 | behind, 1.20×, p 3.4e-8 | behind, 1.20×, p 3.4e-8 | behind, 7.32× | behind, 78.75× |
| upcase-stdin | rust | 20 | behind, 1.15×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 4.34× | behind, 15.98× |
| shout | c-wasm | 640 | level, 0.99×, p 1.2e-4 | level, 0.98×, p 3.7e-5 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 320 | level, 0.98×, p 3.4e-6 | level, 0.98×, p 7.0e-6 | level, 1.00× | ahead, 0.40× |
| shout | rust-wasm | 20 | ahead, 0.88×, p 1.3e-6 | ahead, 0.85×, p 7.2e-5 | level, 0.98× | ahead, 0.03× |
| shout | c | 20 | behind, 2.02×, p 3.4e-8 | behind, 1.95×, p 3.5e-7 | behind, 2.70× | behind, 78.75× |
| shout | cpp | 20 | behind, 2.01×, p 3.4e-8 | behind, 1.93×, p 3.4e-8 | behind, 2.70× | behind, 78.75× |
| shout | rust | 20 | behind, 1.78×, p 3.4e-8 | behind, 1.73×, p 3.4e-8 | behind, 2.28× | behind, 15.98× |
| wc | c-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 0.98× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 0.98× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| wc | c | 20 | level, 1.03×, p 2.2e-5 | level, 1.04×, p 9.0e-6 | behind, 4.84× | behind, 78.76× |
| wc | cpp | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.90×, p 3.4e-8 | behind, 4.84× | behind, 78.76× |
| wc | rust | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.83×, p 3.4e-8 | behind, 4.32× | behind, 15.98× |
| histogram | c-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | rust-wasm | 20 | level, 0.95×, p 3.4e-8 | level, 0.95×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 4.84× | behind, 78.75× |
| histogram | cpp | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.08×, p 3.4e-8 | behind, 4.84× | behind, 78.75× |
| histogram | rust | 20 | behind, 1.06×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 4.31× | behind, 15.98× |
| crc32c | c-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.33×, p 3.4e-8 | behind, 2.32×, p 3.4e-8 | behind, 4.84× | behind, 78.75× |
| crc32c | cpp | 20 | behind, 2.31×, p 3.4e-8 | behind, 2.31×, p 3.4e-8 | behind, 4.84× | behind, 78.75× |
| crc32c | rust | 20 | behind, 3.29×, p 3.4e-8 | behind, 3.26×, p 3.4e-8 | behind, 4.35× | behind, 15.98× |
| sort | c-wasm | 20 | ahead, 0.19×, p 3.4e-8 | ahead, 0.19×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.67×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | ahead, 0.88× | behind, 78.79× |
| sort | cpp | 20 | ahead, 0.75×, p 3.4e-8 | ahead, 0.75×, p 3.4e-8 | ahead, 0.93× | behind, 78.68× |
| sort | rust | 20 | behind, 1.15×, p 1.5e-6 | behind, 1.15×, p 1.5e-6 | level, 1.00× | behind, 15.99× |
| tally | c-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | level, 1.03× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.49×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | behind, 3.81× | behind, 78.80× |
| tally | cpp | 20 | ahead, 0.57×, p 3.4e-8 | ahead, 0.57×, p 3.4e-8 | behind, 2.05× | behind, 78.80× |
| tally | rust | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | behind, 4.07× | behind, 15.99× |
| server | c-wasm | 40 | level, 0.96×, p 0.001 | ahead, 0.89×, p 3.0e-10 | level, 1.03× | ahead, 0.12× |
| server | cpp-wasm | 320 | level, 0.98×, p 2.8e-4 | ahead, 0.93×, p 7.4e-56 | level, 1.01× | ahead, 0.06× |
| server | rust-wasm | 40 | level, 0.96×, p 5.8e-5 | ahead, 0.90×, p 2.0e-11 | level, 1.00× | ahead, 0.03× |
| server | c | 20 | behind, 1.25×, p 3.4e-8 | behind, 1.25×, p 3.4e-8 | behind, 10.28× | behind, 78.76× |
| server | cpp | 20 | behind, 1.24×, p 3.4e-8 | behind, 1.22×, p 3.4e-8 | behind, 2.86× | behind, 78.65× |
| server | rust | 20 | ahead, 0.72×, p 3.4e-8 | behind, 1.43×, p 3.4e-8 | behind, 3.49× | behind, 15.98× |
| files | c-wasm | 20 | ahead, 0.93×, p 2.3e-7 | ahead, 0.93×, p 2.0e-7 | level, 0.99× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.93×, p 1.7e-7 | ahead, 0.93×, p 1.5e-7 | level, 0.99× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 7.2e-8 | ahead, 0.92×, p 6.2e-8 | level, 0.97× | ahead, 0.03× |
| files | c | 20 | behind, 2.61×, p 3.4e-8 | behind, 2.60×, p 3.4e-8 | behind, 5.61× | behind, 78.76× |
| files | cpp | 20 | behind, 2.58×, p 3.4e-8 | behind, 2.57×, p 3.4e-8 | behind, 5.61× | behind, 78.76× |
| files | rust | 20 | behind, 3.22×, p 3.4e-8 | behind, 3.20×, p 3.4e-8 | behind, 4.75× | behind, 15.99× |
| readings | c-wasm | 20 | level, 0.95×, p 3.4e-8 | level, 0.95×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| readings | cpp-wasm | 20 | level, 0.98×, p 3.4e-8 | level, 0.98×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.4e-8 | level, 0.98× | ahead, 0.02× |
| readings | c | 20 | behind, 2.93×, p 3.4e-8 | behind, 2.92×, p 3.4e-8 | behind, 5.28× | behind, 78.75× |
| readings | cpp | 20 | behind, 2.84×, p 3.4e-8 | behind, 2.83×, p 3.4e-8 | behind, 2.26× | behind, 78.75× |
| readings | rust | 20 | behind, 2.87×, p 3.4e-8 | behind, 2.86×, p 3.4e-8 | behind, 4.85× | behind, 15.98× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 1.74 ms | 1.82 ms | 1.81 ms | 2.01 ms | 578 µs | 580 µs | 768 µs |
| wall time | 1.88 ms | 1.94 ms | 1.93 ms | 2.15 ms | 691 µs | 692 µs | 876 µs |
| peak memory | 4.34 MiB | 4.34 MiB | 4.34 MiB | 4.47 MiB | 872 KiB | 872 KiB | 1.46 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 65.9 KiB | 65.9 KiB | 261 KiB |
| L1 data misses | 8,130 | 8,272 | 8,365 | 9,318 | 1,500 | 1,449 | 2,574 |

Its rounds took 1 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 1.87 ms | 2.04 ms | 2.03 ms | 2.27 ms | 628 µs | 618 µs | 790 µs |
| wall time | 1.99 ms | 2.17 ms | 2.16 ms | 2.40 ms | 732 µs | 731 µs | 905 µs |
| peak memory | 4.34 MiB | 4.47 MiB | 4.47 MiB | 4.50 MiB | 1.35 MiB | 1.35 MiB | 1.53 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 8,816 | 9,517 | 9,457 | 10,410 | 1,674 | 1,617 | 2,577 |

Its rounds took 1 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 1.93 ms | 2.07 ms | 2.09 ms | 2.35 ms | 708 µs | 704 µs | 797 µs |
| wall time | 2.06 ms | 2.20 ms | 2.23 ms | 2.48 ms | 821 µs | 817 µs | 922 µs |
| peak memory | 4.34 MiB | 4.47 MiB | 4.47 MiB | 4.50 MiB | 1.48 MiB | 1.52 MiB | 1.53 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 8,800 | 9,561 | 9,537 | 10,572 | 2,056 | 2,029 | 2,610 |

Its rounds took 1 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 3.02 ms | 3.19 ms | 3.20 ms | 3.44 ms | 1.68 ms | 1.69 ms | 1.88 ms |
| wall time | 3.36 ms | 3.47 ms | 3.49 ms | 3.69 ms | 1.90 ms | 1.89 ms | 2.10 ms |
| peak memory | 6.21 MiB | 6.22 MiB | 6.22 MiB | 6.38 MiB | 2.10 MiB | 2.10 MiB | 2.47 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 10,018 | 11,221 | 10,859 | 12,023 | 2,255 | 2,128 | 3,143 |

Its rounds took 3 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 40 | 80 | 2000 | 20 | 20 | 20 |
| CPU time | 13.0 ms | 13.9 ms | 13.9 ms | 13.0 ms | 11.9 ms | 11.7 ms | 12.2 ms |
| wall time | 13.2 ms | 14.1 ms | 14.0 ms | 13.1 ms | 12.0 ms | 11.8 ms | 12.3 ms |
| peak memory | 7.15 MiB | 7.15 MiB | 7.15 MiB | 7.27 MiB | 1000 KiB | 1000 KiB | 1.65 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 149,810 | 146,497 | 151,101 | 155,289 | 25,661 | 26,887 | 47,388 |

Its rounds took 77 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 640 | 320 | 20 | 20 | 20 | 20 |
| CPU time | 2.19 ms | 2.22 ms | 2.19 ms | 2.31 ms | 1.00 ms | 1.01 ms | 1.14 ms |
| wall time | 2.30 ms | 2.35 ms | 2.31 ms | 2.53 ms | 1.10 ms | 1.11 ms | 1.24 ms |
| peak memory | 5.34 MiB | 5.34 MiB | 5.34 MiB | 5.46 MiB | 1.98 MiB | 1.98 MiB | 2.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,264 | 9,377 | 9,274 | 10,595 | 1,918 | 1,897 | 3,080 |

Its rounds took 11 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 18.6 ms | 27.7 ms | 186 ms | 27.6 ms | 18.0 ms | 20.7 ms | 22.6 ms |
| wall time | 18.9 ms | 27.9 ms | 186 ms | 27.7 ms | 18.1 ms | 20.9 ms | 22.7 ms |
| peak memory | 7.14 MiB | 7.27 MiB | 7.27 MiB | 7.30 MiB | 1.48 MiB | 1.48 MiB | 1.65 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 126,832 | 133,806 | 127,488 | 140,763 | 23,208 | 27,539 | 28,463 |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 38.3 ms | 70.0 ms | 70.0 ms | 40.1 ms | 35.8 ms | 35.7 ms | 36.2 ms |
| wall time | 38.5 ms | 70.1 ms | 70.2 ms | 40.3 ms | 35.9 ms | 35.8 ms | 36.3 ms |
| peak memory | 7.14 MiB | 7.27 MiB | 7.27 MiB | 7.30 MiB | 1.48 MiB | 1.48 MiB | 1.66 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 174,673 | 180,770 | 181,330 | 200,249 | 83,620 | 84,331 | 100,155 |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 36.4 ms | 46.4 ms | 46.7 ms | 49.7 ms | 15.6 ms | 15.7 ms | 11.1 ms |
| wall time | 36.6 ms | 46.6 ms | 46.9 ms | 49.9 ms | 15.7 ms | 15.8 ms | 11.2 ms |
| peak memory | 7.15 MiB | 7.27 MiB | 7.27 MiB | 7.27 MiB | 1.48 MiB | 1.48 MiB | 1.64 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 299,559 | 220,746 | 232,292 | 234,364 | 24,212 | 25,433 | 27,780 |

Its rounds took 6 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 270 ms | 1.39 s | 467 ms | 406 ms | 373 ms | 360 ms | 235 ms |
| wall time | 270 ms | 1.40 s | 467 ms | 406 ms | 373 ms | 360 ms | 236 ms |
| peak memory | 76.6 MiB | 76.6 MiB | 84.6 MiB | 84.8 MiB | 86.9 MiB | 82.7 MiB | 76.5 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 6,310,045 | 30,343,348 | 12,245,244 | 12,090,112 | 14,522,365 | 15,505,512 | 12,441,327 |

Its rounds took 84 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 139 ms | 289 ms | 288 ms | 405 ms | 236 ms | 244 ms | 252 ms |
| wall time | 140 ms | 290 ms | 288 ms | 406 ms | 237 ms | 245 ms | 252 ms |
| peak memory | 8.02 MiB | 7.77 MiB | 7.90 MiB | 7.79 MiB | 2.11 MiB | 3.92 MiB | 1.97 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 6,807,822 | 8,673,065 | 8,638,431 | 8,139,718 | 8,514,544 | 8,395,310 | 11,254,160 |

Its rounds took 44 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 40 | 320 | 40 | 20 | 20 | 20 |
| CPU time | 562 ms | 577 ms | 570 ms | 574 ms | 441 ms | 446 ms | 770 ms |
| wall time | 565 ms | 628 ms | 609 ms | 618 ms | 445 ms | 453 ms | 387 ms |
| peak memory | 7.27 MiB | 7.09 MiB | 7.21 MiB | 7.23 MiB | 724 KiB | 2.54 MiB | 2.09 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 65.9 KiB | 66.0 KiB | 325 KiB |
| L1 data misses | 2,288,636 | 2,477,308 | 2,591,248 | 2,741,625 | 118,711 | 191,269 | 529,534 |

Its rounds took 510 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 43.7 ms | 47.1 ms | 47.0 ms | 47.6 ms | 16.7 ms | 16.9 ms | 13.6 ms |
| wall time | 43.8 ms | 47.2 ms | 47.2 ms | 47.8 ms | 16.8 ms | 17.1 ms | 13.7 ms |
| peak memory | 7.58 MiB | 7.65 MiB | 7.65 MiB | 7.82 MiB | 1.35 MiB | 1.35 MiB | 1.60 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 7,468,065 | 7,675,067 | 7,665,479 | 7,613,353 | 10,039 | 10,093 | 11,890 |

Its rounds took 6 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 94.0 ms | 98.9 ms | 95.7 ms | 101 ms | 32.1 ms | 33.1 ms | 32.8 ms |
| wall time | 94.2 ms | 99.1 ms | 95.8 ms | 101 ms | 32.2 ms | 33.3 ms | 32.9 ms |
| peak memory | 7.14 MiB | 7.27 MiB | 7.27 MiB | 7.31 MiB | 1.35 MiB | 3.16 MiB | 1.47 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 65.9 KiB | 65.9 KiB | 325 KiB |
| L1 data misses | 9,389 | 10,074 | 10,209 | 11,682 | 1,776 | 13,602 | 2,749 |

Its rounds took 12 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; gcc -O3 -flto -s -march=native -lm

- cpp: g++ (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; g++ -O3 -flto -s -march=native -std=gnu++17 -lm

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 4 s | 1.74 ms against 1.63 ms: +115 µs (+100 µs to +130 µs) | 1.87 ms against 1.75 ms: +120 µs (+102 µs to +137 µs) | 4.34 MiB against 4.09 MiB: +256 KiB (+256 KiB to +256 KiB) |
| add | 160, 160 in 5 s | 1.82 ms against 1.66 ms: +157 µs (+143 µs to +172 µs) | 1.95 ms against 1.79 ms: +158 µs (+142 µs to +174 µs) | 4.34 MiB against 4.22 MiB: +128 KiB (+128 KiB to +128 KiB) |
| hypotenuse | 160, 160 in 5 s | 1.84 ms against 1.65 ms: +188 µs (+176 µs to +200 µs) | 1.97 ms against 1.78 ms: +190 µs (+177 µs to +204 µs) | 4.34 MiB against 4.22 MiB: +120 KiB (+120 KiB to +124 KiB) |
| upcase | 160, 160 in 6 s | 3.04 ms against 2.84 ms: +204 µs (+178 µs to +229 µs) | 3.33 ms against 3.17 ms: +180 µs (+132 µs to +225 µs) | 6.09 MiB against 5.21 MiB: +388 KiB (+372 KiB to +628 KiB) |
| upcase-stdin | 160, 160 in 14 s | 13.2 ms against 13.1 ms: +135 µs (+43.0 µs to +226 µs) | 13.4 ms against 13.3 ms: +116 µs (+12.9 µs to +214 µs) | 7.15 MiB against 7.02 MiB: +128 KiB (+128 KiB to +128 KiB) |
| shout | 160, 160 in 5 s | 2.22 ms against 2.06 ms: +155 µs (+138 µs to +170 µs) | 2.34 ms against 2.18 ms: +158 µs (+139 µs to +178 µs) | 5.34 MiB against 4.97 MiB: +384 KiB (+380 KiB to +384 KiB) |
| wc | 160, 160 in 48 s | 19.1 ms against 18.9 ms: +202 µs (+88.0 µs to +315 µs) | 19.2 ms against 19.1 ms: +164 µs (+39.8 µs to +288 µs) | 7.14 MiB against 7.02 MiB: +128 KiB (+124 KiB to +128 KiB) |
| histogram | 160, 160 in 45 s | 39.0 ms against 38.9 ms: +160 µs (+76.0 µs to +241 µs) | 39.2 ms against 39.0 ms: +163 µs (+77.6 µs to +248 µs) | 7.14 MiB against 7.02 MiB: +124 KiB (+120 KiB to +128 KiB) |
| crc32c | 160, 160 in 38 s | 36.3 ms against 36.1 ms: +216 µs (+106 µs to +324 µs) | 36.5 ms against 36.3 ms: +207 µs (+93.3 µs to +323 µs) | 7.15 MiB against 7.02 MiB: +128 KiB (+128 KiB to +128 KiB) |
| sort | 20, 20 in 61 s | 289 ms against 289 ms: +2.27 ms (-7.15 ms to +11.7 ms) | 289 ms against 290 ms: +2.23 ms (-7.16 ms to +11.6 ms) | 76.6 MiB against 76.5 MiB: +128 KiB (+128 KiB to +128 KiB) |
| tally | 47, 47 in 60 s | 139 ms against 138 ms: +754 µs (-16.0 µs to +1.51 ms) | 139 ms against 138 ms: +763 µs (+3.84 µs to +1.49 ms) | 8.02 MiB against 7.90 MiB: +128 KiB (+128 KiB to +132 KiB) |
| server | 20, 20 in 64 s | 544 ms against 551 ms: -3.59 ms (-16.4 ms to +11.1 ms) | 548 ms against 551 ms: -1.35 ms (-15.0 ms to +12.6 ms) | 7.27 MiB against 7.02 MiB: +240 KiB (+128 KiB to +256 KiB) |
| files | 160, 160 in 41 s | 44.9 ms against 44.6 ms: +249 µs (+123 µs to +371 µs) | 45.1 ms against 44.9 ms: +243 µs (+109 µs to +379 µs) | 7.58 MiB against 7.59 MiB: -4.00 KiB (-4.00 KiB to +0 B) |
| readings | 119, 119 in 60 s | 94.1 ms against 94.0 ms: +190 µs (+112 µs to +266 µs) | 94.3 ms against 94.2 ms: +202 µs (+115 µs to +289 µs) | 7.14 MiB against 7.02 MiB: +120 KiB (+120 KiB to +128 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 1.65 ms, 1.05× | 1.78 ms, 1.05× | 4.22 MiB, 1.03× |
| empty | cpp-wasm | 160 | 1.65 ms, 1.06× | 1.78 ms, 1.05× | 4.22 MiB, 1.03× |
| empty | rust-wasm | 160 | 1.73 ms, 1.00× | 1.86 ms, 1.01× | 4.34 MiB, 1.00× |
| add | c-wasm | 160 | 1.73 ms, 1.05× | 1.85 ms, 1.05× | 4.21 MiB, 1.03× |
| add | cpp-wasm | 160 | 1.74 ms, 1.05× | 1.87 ms, 1.04× | 4.21 MiB, 1.03× |
| add | rust-wasm | 160 | 1.88 ms, 0.97× | 2.01 ms, 0.97× | 4.31 MiB, 1.01× |
| hypotenuse | c-wasm | 160 | 1.71 ms, 1.07× | 1.84 ms, 1.07× | 4.21 MiB, 1.03× |
| hypotenuse | cpp-wasm | 160 | 1.72 ms, 1.07× | 1.85 ms, 1.06× | 4.21 MiB, 1.03× |
| hypotenuse | rust-wasm | 160 | 1.89 ms, 0.97× | 2.02 ms, 0.98× | 4.32 MiB, 1.00× |
| upcase | c-wasm | 160 | 2.87 ms, 1.06× | 3.17 ms, 1.05× | 5.46 MiB, 1.11× |
| upcase | cpp-wasm | 160 | 2.89 ms, 1.05× | 3.19 ms, 1.04× | 5.34 MiB, 1.14× |
| upcase | rust-wasm | 160 | 3.07 ms, 0.99× | 3.36 ms, 0.99× | 6.07 MiB, 1.00× |
| upcase-stdin | c-wasm | 160 | 12.9 ms, 1.02× | 13.1 ms, 1.02× | 7.02 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 13.1 ms, 1.01× | 13.3 ms, 1.01× | 7.02 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 13.1 ms, 1.01× | 13.3 ms, 1.01× | 7.15 MiB, 1.00× |
| shout | c-wasm | 160 | 2.07 ms, 1.07× | 2.19 ms, 1.07× | 5.09 MiB, 1.05× |
| shout | cpp-wasm | 160 | 2.09 ms, 1.06× | 2.20 ms, 1.06× | 5.09 MiB, 1.05× |
| shout | rust-wasm | 160 | 2.26 ms, 0.98× | 2.37 ms, 0.99× | 5.17 MiB, 1.03× |
| wc | c-wasm | 160 | 27.9 ms, 0.68× | 28.1 ms, 0.68× | 7.01 MiB, 1.02× |
| wc | cpp-wasm | 160 | 186 ms, 0.10× | 186 ms, 0.10× | 7.01 MiB, 1.02× |
| wc | rust-wasm | 160 | 27.5 ms, 0.69× | 27.7 ms, 0.69× | 7.12 MiB, 1.00× |
| histogram | c-wasm | 160 | 70.5 ms, 0.55× | 70.7 ms, 0.55× | 7.01 MiB, 1.02× |
| histogram | cpp-wasm | 160 | 70.5 ms, 0.55× | 70.7 ms, 0.55× | 7.01 MiB, 1.02× |
| histogram | rust-wasm | 160 | 40.5 ms, 0.96× | 40.6 ms, 0.96× | 7.12 MiB, 1.00× |
| crc32c | c-wasm | 160 | 46.5 ms, 0.78× | 46.6 ms, 0.78× | 7.01 MiB, 1.02× |
| crc32c | cpp-wasm | 160 | 46.6 ms, 0.78× | 46.8 ms, 0.78× | 7.01 MiB, 1.02× |
| crc32c | rust-wasm | 160 | 49.4 ms, 0.73× | 49.6 ms, 0.73× | 7.10 MiB, 1.01× |
| sort | c-wasm | 20 | 1.48 s, 0.19× | 1.48 s, 0.19× | 76.5 MiB, 1.00× |
| sort | cpp-wasm | 20 | 512 ms, 0.56× | 512 ms, 0.56× | 84.5 MiB, 0.91× |
| sort | rust-wasm | 20 | 460 ms, 0.63× | 460 ms, 0.63× | 84.6 MiB, 0.91× |
| tally | c-wasm | 47 | 288 ms, 0.48× | 289 ms, 0.48× | 7.64 MiB, 1.05× |
| tally | cpp-wasm | 47 | 287 ms, 0.48× | 288 ms, 0.48× | 7.64 MiB, 1.05× |
| tally | rust-wasm | 47 | 404 ms, 0.34× | 405 ms, 0.34× | 7.61 MiB, 1.05× |
| server | c-wasm | 20 | 560 ms, 0.97× | 618 ms, 0.89× | 7.07 MiB, 1.03× |
| server | cpp-wasm | 20 | 566 ms, 0.96× | 604 ms, 0.91× | 7.08 MiB, 1.03× |
| server | rust-wasm | 20 | 556 ms, 0.98× | 588 ms, 0.93× | 7.18 MiB, 1.01× |
| files | c-wasm | 160 | 47.7 ms, 0.94× | 47.9 ms, 0.94× | 7.52 MiB, 1.01× |
| files | cpp-wasm | 160 | 47.7 ms, 0.94× | 47.9 ms, 0.94× | 7.52 MiB, 1.01× |
| files | rust-wasm | 160 | 48.1 ms, 0.93× | 48.3 ms, 0.93× | 7.63 MiB, 0.99× |
| readings | c-wasm | 119 | 98.8 ms, 0.95× | 99.0 ms, 0.95× | 7.02 MiB, 1.02× |
| readings | cpp-wasm | 119 | 95.2 ms, 0.99× | 95.4 ms, 0.99× | 7.02 MiB, 1.02× |
| readings | rust-wasm | 119 | 101 ms, 0.93× | 101 ms, 0.94× | 7.12 MiB, 1.00× |
