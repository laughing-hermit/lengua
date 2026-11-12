# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 7763 64-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 610aaf5c622249762f3d80df3fa0947c3f71739e, in https://github.com/laughing-hermit/lengua/actions/runs/43191148225. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records the module and lengua's idea, with the line that names an outside module added, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (6.06 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ≈ ≈ ≈ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 empty c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 47 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 empty cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 48 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 server c wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 2.47× | 2.25× | 4.41× | 443.49× |
| add | 2.16× | 2.00× | 3.44× | 439.38× |
| hypotenuse | 1.91× | 1.82× | 3.09× | 439.40× |
| upcase | 1.82× | 1.83× | 3.31× | 439.43× |
| upcase-stdin | 1.55× | 1.55× | 5.92× | 439.38× |
| shout | 1.81× | 1.79× | 2.75× | 439.39× |
| wc | 1.40× | 1.40× | 4.85× | 439.43× |
| histogram | 1.19× | 1.19× | 4.83× | 439.41× |
| crc32c | 2.88× | 2.87× | 4.85× | 439.41× |
| sort | 1.04× | 1.04× | 1.01× | 439.57× |
| tally | 0.51× | 0.51× | 3.99× | 439.61× |
| server | 1.08× | 1.87× | 7.07× | 439.43× |
| files | 3.28× | 3.30× | 5.18× | 439.42× |
| readings | 3.44× | 3.43× | 4.87× | 439.38× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.00×, p 0.067 | level, 1.00×, p 9.0e-4 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.04×, p 1.5e-5 | level, 1.04×, p 1.4e-9 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.82×, p 3.4e-8 | ahead, 0.82×, p 3.4e-8 | ahead, 0.95× | ahead, 0.01× |
| empty | c | 20 | behind, 2.47×, p 3.4e-8 | behind, 2.25×, p 3.4e-8 | behind, 4.41× | behind, 443.49× |
| empty | cpp | 20 | behind, 2.44×, p 3.4e-8 | behind, 2.25×, p 3.4e-8 | behind, 4.29× | behind, 443.49× |
| empty | rust | 20 | behind, 1.73×, p 3.4e-8 | behind, 1.65×, p 3.4e-8 | behind, 3.07× | behind, 22.36× |
| add | c-wasm | 20 | ahead, 0.78×, p 7.1e-8 | ahead, 0.78×, p 9.6e-8 | level, 0.98× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.79×, p 7.1e-8 | ahead, 0.80×, p 1.7e-7 | level, 0.98× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.69×, p 3.4e-8 | level, 0.99× | ahead, 0.02× |
| add | c | 20 | behind, 2.12×, p 3.4e-8 | behind, 2.00×, p 3.4e-8 | behind, 3.31× | behind, 439.38× |
| add | cpp | 20 | behind, 2.16×, p 3.4e-8 | behind, 1.98×, p 3.4e-8 | behind, 3.44× | behind, 439.38× |
| add | rust | 20 | behind, 1.64×, p 3.3e-8 | behind, 1.57×, p 3.4e-8 | behind, 3.09× | behind, 21.69× |
| hypotenuse | c-wasm | 20 | ahead, 0.81×, p 6.0e-7 | ahead, 0.82×, p 6.0e-7 | level, 0.98× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | ahead, 0.86×, p 1.1e-4 | ahead, 0.87×, p 1.5e-4 | level, 0.99× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.68×, p 3.4e-8 | ahead, 0.69×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 1.90×, p 3.4e-8 | behind, 1.82×, p 3.4e-8 | behind, 2.96× | behind, 439.40× |
| hypotenuse | cpp | 20 | behind, 1.91×, p 3.4e-8 | behind, 1.82×, p 3.4e-8 | behind, 2.92× | behind, 439.40× |
| hypotenuse | rust | 20 | behind, 1.70×, p 3.4e-8 | behind, 1.63×, p 3.4e-8 | behind, 3.09× | behind, 21.51× |
| upcase | c-wasm | 640 | level, 0.96×, p 9.3e-22 | level, 0.96×, p 4.5e-7 | level, 0.99× | ahead, 0.06× |
| upcase | cpp-wasm | 640 | level, 0.97×, p 2.2e-17 | level, 0.97×, p 5.3e-5 | level, 0.99× | ahead, 0.06× |
| upcase | rust-wasm | 80 | ahead, 0.89×, p 1.7e-17 | ahead, 0.90×, p 2.4e-4 | level, 0.98× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.82×, p 3.4e-8 | behind, 1.83×, p 3.4e-8 | behind, 3.22× | behind, 439.43× |
| upcase | cpp | 20 | behind, 1.77×, p 3.4e-8 | behind, 1.79×, p 3.4e-8 | behind, 3.31× | behind, 439.43× |
| upcase | rust | 20 | behind, 1.36×, p 2.0e-7 | behind, 1.41×, p 1.5e-7 | behind, 2.80× | behind, 21.74× |
| upcase-stdin | c-wasm | 20 | ahead, 0.32×, p 3.4e-8 | ahead, 0.31×, p 3.4e-8 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.33×, p 3.4e-8 | ahead, 0.33×, p 3.4e-8 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 40 | ahead, 0.92×, p 4.9e-4 | ahead, 0.92×, p 4.9e-4 | level, 0.97× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.54×, p 3.4e-8 | behind, 1.54×, p 3.4e-8 | behind, 5.67× | behind, 439.38× |
| upcase-stdin | cpp | 20 | behind, 1.55×, p 3.4e-8 | behind, 1.55×, p 3.4e-8 | behind, 5.92× | behind, 439.38× |
| upcase-stdin | rust | 20 | behind, 1.52×, p 3.4e-8 | behind, 1.51×, p 3.4e-8 | behind, 4.14× | behind, 22.13× |
| shout | c-wasm | 320 | level, 0.98×, p 1.4e-4 | level, 0.96×, p 9.8e-4 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 1280 | level, 0.98×, p 6.4e-15 | level, 0.97×, p 1.2e-6 | level, 1.00× | ahead, 0.40× |
| shout | rust-wasm | 80 | ahead, 0.93×, p 8.6e-14 | ahead, 0.94×, p 5.1e-5 | level, 0.97× | ahead, 0.03× |
| shout | c | 20 | behind, 1.81×, p 3.4e-8 | behind, 1.79×, p 4.0e-7 | behind, 2.75× | behind, 439.39× |
| shout | cpp | 20 | behind, 1.80×, p 3.4e-8 | behind, 1.78×, p 1.7e-6 | behind, 2.75× | behind, 439.39× |
| shout | rust | 20 | behind, 1.28×, p 3.0e-7 | behind, 1.28×, p 6.0e-7 | behind, 2.38× | behind, 21.86× |
| wc | c-wasm | 20 | ahead, 0.86×, p 3.4e-8 | ahead, 0.86×, p 3.4e-8 | level, 0.99× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.85×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| wc | c | 20 | behind, 1.40×, p 3.4e-8 | behind, 1.40×, p 3.4e-8 | behind, 4.67× | behind, 439.43× |
| wc | cpp | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | behind, 4.85× | behind, 439.43× |
| wc | rust | 20 | behind, 1.21×, p 3.4e-8 | behind, 1.22×, p 3.4e-8 | behind, 4.21× | behind, 21.31× |
| histogram | c-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.70×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.78×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.16×, p 3.4e-8 | behind, 1.16×, p 3.4e-8 | behind, 4.65× | behind, 439.41× |
| histogram | cpp | 20 | behind, 1.15×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 4.83× | behind, 439.41× |
| histogram | rust | 20 | behind, 1.19×, p 3.4e-8 | behind, 1.19×, p 3.4e-8 | behind, 4.10× | behind, 21.35× |
| crc32c | c-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | level, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.88×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 4.85× | behind, 439.41× |
| crc32c | cpp | 20 | behind, 2.88×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 4.85× | behind, 439.41× |
| crc32c | rust | 20 | behind, 2.83×, p 3.4e-8 | behind, 2.82×, p 3.4e-8 | behind, 4.24× | behind, 21.90× |
| sort | c-wasm | 20 | ahead, 0.22×, p 3.4e-8 | ahead, 0.22×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 3.4e-8 | ahead, 0.58×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | ahead, 0.89× | behind, 439.57× |
| sort | cpp | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | ahead, 0.93× | behind, 340.84× |
| sort | rust | 20 | level, 1.04×, p 1.4e-4 | level, 1.04×, p 1.4e-4 | level, 1.01× | behind, 21.36× |
| tally | c-wasm | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.32×, p 3.4e-8 | ahead, 0.32×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | behind, 3.89× | behind, 439.61× |
| tally | cpp | 20 | ahead, 0.51×, p 3.4e-8 | ahead, 0.51×, p 3.4e-8 | behind, 1.92× | behind, 342.63× |
| tally | rust | 20 | ahead, 0.46×, p 3.4e-8 | ahead, 0.46×, p 3.4e-8 | behind, 3.99× | behind, 20.77× |
| server | c-wasm | 160 | level, 1.00×, p 5.7e-5 | ahead, 0.92×, p 6.6e-41 | level, 0.99× | ahead, 0.12× |
| server | cpp-wasm | 80 | level, 0.99×, p 1.3e-4 | ahead, 0.94×, p 1.8e-23 | level, 0.99× | ahead, 0.06× |
| server | rust-wasm | 40 | level, 0.99×, p 5.1e-4 | ahead, 0.87×, p 6.9e-12 | level, 0.97× | ahead, 0.03× |
| server | c | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.08×, p 3.4e-8 | behind, 7.07× | behind, 439.43× |
| server | cpp | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 2.48× | behind, 436.53× |
| server | rust | 20 | level, 0.96×, p 3.4e-8 | behind, 1.87×, p 3.4e-8 | behind, 3.19× | behind, 19.89× |
| files | c-wasm | 20 | ahead, 0.88×, p 3.4e-8 | ahead, 0.88×, p 3.4e-8 | level, 0.99× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.87×, p 3.4e-8 | ahead, 0.87×, p 3.4e-8 | level, 0.98× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 1.7e-7 | ahead, 0.93×, p 1.7e-7 | level, 0.97× | ahead, 0.03× |
| files | c | 20 | behind, 2.58×, p 3.4e-8 | behind, 2.59×, p 3.4e-8 | behind, 5.18× | behind, 439.42× |
| files | cpp | 20 | behind, 2.57×, p 3.4e-8 | behind, 2.58×, p 3.4e-8 | behind, 4.98× | behind, 439.42× |
| files | rust | 20 | behind, 3.28×, p 3.4e-8 | behind, 3.30×, p 3.4e-8 | behind, 4.23× | behind, 21.42× |
| readings | c-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.94×, p 1.1e-7 | ahead, 0.94×, p 1.1e-7 | level, 0.99× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 0.96× | ahead, 0.02× |
| readings | c | 20 | behind, 3.44×, p 3.4e-8 | behind, 3.43×, p 3.4e-8 | behind, 4.87× | behind, 439.38× |
| readings | cpp | 20 | behind, 3.15×, p 3.4e-8 | behind, 3.14×, p 3.4e-8 | behind, 2.03× | behind, 439.38× |
| readings | rust | 20 | behind, 3.38×, p 3.4e-8 | behind, 3.38×, p 3.4e-8 | behind, 4.46× | behind, 21.33× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 20 | 20 | 20 | 20 |
| CPU time | 1.24 ms | 1.24 ms | 1.20 ms | 1.51 ms | 500 µs | 506 µs | 714 µs |
| wall time | 1.34 ms | 1.33 ms | 1.29 ms | 1.61 ms | 589 µs | 588 µs | 804 µs |
| peak memory | 5.27 MiB | 5.26 MiB | 5.25 MiB | 5.53 MiB | 1.19 MiB | 1.22 MiB | 1.71 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 14.0 KiB | 14.0 KiB | 278 KiB |

Its rounds took 28 seconds.

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
| CPU time | 1.18 ms | 1.51 ms | 1.49 ms | 1.76 ms | 556 µs | 546 µs | 720 µs |
| wall time | 1.27 ms | 1.62 ms | 1.58 ms | 1.85 ms | 635 µs | 640 µs | 806 µs |
| peak memory | 5.29 MiB | 5.40 MiB | 5.39 MiB | 5.35 MiB | 1.60 MiB | 1.54 MiB | 1.71 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

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
| CPU time | 1.22 ms | 1.51 ms | 1.41 ms | 1.79 ms | 641 µs | 638 µs | 716 µs |
| wall time | 1.31 ms | 1.60 ms | 1.50 ms | 1.88 ms | 721 µs | 719 µs | 803 µs |
| peak memory | 5.30 MiB | 5.38 MiB | 5.33 MiB | 5.38 MiB | 1.79 MiB | 1.82 MiB | 1.72 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 14.1 KiB | 14.1 KiB | 289 KiB |

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
| counted runs | 640 | 640 | 640 | 80 | 20 | 20 | 20 |
| CPU time | 2.79 ms | 2.89 ms | 2.87 ms | 3.15 ms | 1.55 ms | 1.59 ms | 2.06 ms |
| wall time | 3.04 ms | 3.16 ms | 3.12 ms | 3.42 ms | 1.70 ms | 1.74 ms | 2.21 ms |
| peak memory | 7.58 MiB | 7.66 MiB | 7.66 MiB | 7.78 MiB | 2.35 MiB | 2.29 MiB | 2.70 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 17 seconds.

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
| counted runs | 40 | 20 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 10.7 ms | 34.2 ms | 32.8 ms | 11.7 ms | 7.04 ms | 7.00 ms | 7.16 ms |
| wall time | 10.9 ms | 34.9 ms | 32.9 ms | 11.9 ms | 7.14 ms | 7.09 ms | 7.25 ms |
| peak memory | 7.65 MiB | 7.66 MiB | 7.61 MiB | 7.89 MiB | 1.35 MiB | 1.29 MiB | 1.85 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 281 KiB |

Its rounds took 5 seconds.

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
| counted runs | 1280 | 320 | 1280 | 80 | 20 | 20 | 20 |
| CPU time | 2.13 ms | 2.21 ms | 2.18 ms | 2.31 ms | 1.18 ms | 1.18 ms | 1.66 ms |
| wall time | 2.27 ms | 2.38 ms | 2.33 ms | 2.41 ms | 1.26 ms | 1.27 ms | 1.77 ms |
| peak memory | 6.16 MiB | 6.17 MiB | 6.18 MiB | 6.39 MiB | 2.22 MiB | 2.22 MiB | 2.56 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

Its rounds took 20 seconds.

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
| CPU time | 20.0 ms | 23.1 ms | 197 ms | 23.5 ms | 14.2 ms | 148 ms | 16.4 ms |
| wall time | 20.1 ms | 23.3 ms | 198 ms | 23.6 ms | 14.3 ms | 148 ms | 16.5 ms |
| peak memory | 7.77 MiB | 7.81 MiB | 7.73 MiB | 7.90 MiB | 1.66 MiB | 1.60 MiB | 1.85 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 11 seconds.

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
| CPU time | 33.2 ms | 46.8 ms | 46.5 ms | 42.4 ms | 28.7 ms | 28.8 ms | 27.9 ms |
| wall time | 33.3 ms | 47.0 ms | 47.2 ms | 42.9 ms | 28.8 ms | 28.9 ms | 28.0 ms |
| peak memory | 7.74 MiB | 7.75 MiB | 7.74 MiB | 7.93 MiB | 1.66 MiB | 1.60 MiB | 1.89 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 7 seconds.

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
| CPU time | 37.7 ms | 47.8 ms | 48.0 ms | 48.0 ms | 13.1 ms | 13.1 ms | 13.3 ms |
| wall time | 37.8 ms | 48.0 ms | 48.1 ms | 48.2 ms | 13.2 ms | 13.2 ms | 13.4 ms |
| peak memory | 7.77 MiB | 7.76 MiB | 7.73 MiB | 7.97 MiB | 1.60 MiB | 1.60 MiB | 1.83 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

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
| CPU time | 266 ms | 1.22 s | 455 ms | 423 ms | 423 ms | 365 ms | 257 ms |
| wall time | 266 ms | 1.22 s | 455 ms | 423 ms | 423 ms | 365 ms | 257 ms |
| peak memory | 77.3 MiB | 77.2 MiB | 85.4 MiB | 85.3 MiB | 87.0 MiB | 83.2 MiB | 76.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 291 KiB |

Its rounds took 81 seconds.

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
| CPU time | 136 ms | 341 ms | 341 ms | 423 ms | 284 ms | 268 ms | 296 ms |
| wall time | 136 ms | 341 ms | 341 ms | 423 ms | 284 ms | 268 ms | 296 ms |
| peak memory | 8.65 MiB | 8.52 MiB | 8.49 MiB | 8.33 MiB | 2.23 MiB | 4.52 MiB | 2.17 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 299 KiB |

Its rounds took 49 seconds.

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
| counted runs | 160 | 160 | 80 | 40 | 20 | 20 | 20 |
| CPU time | 930 ms | 934 ms | 935 ms | 937 ms | 860 ms | 867 ms | 969 ms |
| wall time | 932 ms | 1.02 s | 989 ms | 1.07 s | 864 ms | 871 ms | 497 ms |
| peak memory | 7.79 MiB | 7.86 MiB | 7.88 MiB | 8.01 MiB | 1.10 MiB | 3.12 MiB | 2.43 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 14.1 KiB | 14.2 KiB | 312 KiB |

Its rounds took 541 seconds.

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
| CPU time | 62.2 ms | 71.0 ms | 71.7 ms | 67.6 ms | 24.1 ms | 24.3 ms | 19.0 ms |
| wall time | 62.8 ms | 71.1 ms | 71.8 ms | 67.8 ms | 24.3 ms | 24.4 ms | 19.1 ms |
| peak memory | 8.29 MiB | 8.41 MiB | 8.42 MiB | 8.55 MiB | 1.60 MiB | 1.66 MiB | 1.96 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 9 seconds.

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
| CPU time | 98.9 ms | 107 ms | 105 ms | 107 ms | 28.8 ms | 31.4 ms | 29.2 ms |
| wall time | 99.0 ms | 107 ms | 105 ms | 108 ms | 28.9 ms | 31.5 ms | 29.3 ms |
| peak memory | 7.64 MiB | 7.72 MiB | 7.75 MiB | 7.99 MiB | 1.57 MiB | 3.77 MiB | 1.71 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

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
| empty | 160, 160 in 3 s | 1.24 ms against 1.14 ms: +94.0 µs (+86.0 µs to +101 µs) | 1.33 ms against 1.23 ms: +94.9 µs (+85.5 µs to +104 µs) | 5.27 MiB against 5.12 MiB: +132 KiB (+108 KiB to +156 KiB) |
| add | 160, 160 in 4 s | 1.18 ms against 1.06 ms: +112 µs (+101 µs to +123 µs) | 1.26 ms against 1.14 ms: +112 µs (+101 µs to +123 µs) | 5.27 MiB against 5.16 MiB: +112 KiB (+88.0 KiB to +132 KiB) |
| hypotenuse | 160, 160 in 4 s | 1.20 ms against 1.07 ms: +135 µs (+124 µs to +145 µs) | 1.29 ms against 1.15 ms: +136 µs (+124 µs to +147 µs) | 5.27 MiB against 5.14 MiB: +120 KiB (+96.0 KiB to +144 KiB) |
| upcase | 160, 160 in 6 s | 2.83 ms against 2.66 ms: +148 µs (+104 µs to +191 µs) | 3.04 ms against 2.96 ms: +120 µs (+20.9 µs to +209 µs) | 7.56 MiB against 7.39 MiB: +132 KiB (+40.0 KiB to +228 KiB) |
| upcase-stdin | 160, 160 in 19 s | 10.5 ms against 10.4 ms: +142 µs (+28.0 µs to +252 µs) | 10.8 ms against 10.5 ms: +140 µs (+19.6 µs to +255 µs) | 7.64 MiB against 7.74 MiB: -68.0 KiB (-88.0 KiB to -44.0 KiB) |
| shout | 160, 160 in 5 s | 2.10 ms against 1.97 ms: +136 µs (+104 µs to +171 µs) | 2.24 ms against 2.10 ms: +137 µs (+88.5 µs to +193 µs) | 6.14 MiB against 6.09 MiB: +28.0 KiB (-8.00 KiB to +64.0 KiB) |
| wc | 160, 160 in 49 s | 20.2 ms against 20.2 ms: +120 µs (-28.0 µs to +263 µs) | 20.3 ms against 20.3 ms: +122 µs (-33.1 µs to +267 µs) | 7.75 MiB against 7.78 MiB: -24.0 KiB (-52.0 KiB to -4.00 KiB) |
| histogram | 160, 160 in 35 s | 32.9 ms against 32.7 ms: +219 µs (-35.0 µs to +472 µs) | 33.1 ms against 32.8 ms: +218 µs (-45.7 µs to +487 µs) | 7.77 MiB against 7.73 MiB: +64.0 KiB (+40.0 KiB to +84.0 KiB) |
| crc32c | 160, 160 in 39 s | 38.5 ms against 37.9 ms: +549 µs (+203 µs to +928 µs) | 38.6 ms against 38.0 ms: +577 µs (+215 µs to +953 µs) | 7.77 MiB against 7.74 MiB: +36.0 KiB (+16.0 KiB to +60.0 KiB) |
| sort | 23, 23 in 61 s | 267 ms against 268 ms: -474 µs (-3.26 ms to +2.61 ms) | 268 ms against 268 ms: -485 µs (-3.29 ms to +2.59 ms) | 77.2 MiB against 77.3 MiB: -48.0 KiB (-108 KiB to +0 B) |
| tally | 43, 43 in 60 s | 136 ms against 136 ms: +240 µs (-738 µs to +1.35 ms) | 136 ms against 136 ms: +192 µs (-798 µs to +1.33 ms) | 8.62 MiB against 8.59 MiB: +52.0 KiB (+0 B to +100 KiB) |
| server | 20, 20 in 109 s | 927 ms against 932 ms: -1.65 ms (-9.02 ms to +6.12 ms) | 930 ms against 933 ms: -2.70 ms (-9.55 ms to +4.35 ms) | 7.75 MiB against 7.83 MiB: -40.0 KiB (-112 KiB to +12.0 KiB) |
| files | 160, 160 in 56 s | 62.1 ms against 61.2 ms: +859 µs (+549 µs to +1.15 ms) | 62.3 ms against 61.3 ms: +883 µs (+571 µs to +1.19 ms) | 8.30 MiB against 8.31 MiB: -8.00 KiB (-20.0 KiB to +4.00 KiB) |
| readings | 111, 111 in 60 s | 99.5 ms against 99.8 ms: -188 µs (-442 µs to +71.0 µs) | 99.6 ms against 99.9 ms: -189 µs (-446 µs to +72.1 µs) | 7.64 MiB against 7.70 MiB: -32.0 KiB (-56.0 KiB to -12.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 1.05 ms, 1.17× | 1.14 ms, 1.16× | 5.13 MiB, 1.03× |
| empty | cpp-wasm | 160 | 1.05 ms, 1.18× | 1.14 ms, 1.17× | 5.13 MiB, 1.03× |
| empty | rust-wasm | 160 | 1.25 ms, 0.99× | 1.33 ms, 1.00× | 5.26 MiB, 1.00× |
| add | c-wasm | 160 | 1.33 ms, 0.88× | 1.43 ms, 0.88× | 5.27 MiB, 1.00× |
| add | cpp-wasm | 160 | 1.33 ms, 0.89× | 1.42 ms, 0.89× | 5.25 MiB, 1.00× |
| add | rust-wasm | 160 | 1.55 ms, 0.76× | 1.65 ms, 0.76× | 5.27 MiB, 1.00× |
| hypotenuse | c-wasm | 160 | 1.31 ms, 0.92× | 1.40 ms, 0.92× | 5.27 MiB, 1.00× |
| hypotenuse | cpp-wasm | 160 | 1.30 ms, 0.92× | 1.39 ms, 0.93× | 5.25 MiB, 1.00× |
| hypotenuse | rust-wasm | 160 | 1.58 ms, 0.76× | 1.67 ms, 0.77× | 5.27 MiB, 1.00× |
| upcase | c-wasm | 160 | 2.78 ms, 1.02× | 2.99 ms, 1.02× | 7.70 MiB, 0.98× |
| upcase | cpp-wasm | 160 | 2.78 ms, 1.02× | 3.00 ms, 1.01× | 7.63 MiB, 0.99× |
| upcase | rust-wasm | 160 | 3.05 ms, 0.93× | 3.35 ms, 0.91× | 7.74 MiB, 0.98× |
| upcase-stdin | c-wasm | 160 | 32.3 ms, 0.33× | 32.5 ms, 0.33× | 7.71 MiB, 0.99× |
| upcase-stdin | cpp-wasm | 160 | 32.0 ms, 0.33× | 32.1 ms, 0.33× | 7.71 MiB, 0.99× |
| upcase-stdin | rust-wasm | 160 | 11.4 ms, 0.92× | 11.7 ms, 0.92× | 7.95 MiB, 0.96× |
| shout | c-wasm | 160 | 1.98 ms, 1.06× | 2.10 ms, 1.07× | 6.09 MiB, 1.01× |
| shout | cpp-wasm | 160 | 2.01 ms, 1.04× | 2.21 ms, 1.01× | 6.06 MiB, 1.01× |
| shout | rust-wasm | 160 | 2.23 ms, 0.94× | 2.39 ms, 0.94× | 6.39 MiB, 0.96× |
| wc | c-wasm | 160 | 23.1 ms, 0.87× | 23.2 ms, 0.87× | 7.83 MiB, 0.99× |
| wc | cpp-wasm | 160 | 197 ms, 0.10× | 198 ms, 0.10× | 7.85 MiB, 0.99× |
| wc | rust-wasm | 160 | 23.5 ms, 0.86× | 23.8 ms, 0.85× | 7.96 MiB, 0.97× |
| histogram | c-wasm | 160 | 46.2 ms, 0.71× | 46.3 ms, 0.71× | 7.85 MiB, 0.99× |
| histogram | cpp-wasm | 160 | 46.1 ms, 0.72× | 46.2 ms, 0.72× | 7.85 MiB, 0.99× |
| histogram | rust-wasm | 160 | 41.3 ms, 0.80× | 41.4 ms, 0.80× | 7.97 MiB, 0.97× |
| crc32c | c-wasm | 160 | 47.8 ms, 0.80× | 48.0 ms, 0.80× | 7.85 MiB, 0.99× |
| crc32c | cpp-wasm | 160 | 47.9 ms, 0.80× | 48.1 ms, 0.80× | 7.82 MiB, 0.99× |
| crc32c | rust-wasm | 160 | 48.7 ms, 0.79× | 48.8 ms, 0.79× | 7.96 MiB, 0.98× |
| sort | c-wasm | 23 | 1.22 s, 0.22× | 1.22 s, 0.22× | 77.2 MiB, 1.00× |
| sort | cpp-wasm | 23 | 452 ms, 0.59× | 452 ms, 0.59× | 85.4 MiB, 0.90× |
| sort | rust-wasm | 23 | 421 ms, 0.64× | 421 ms, 0.64× | 85.4 MiB, 0.90× |
| tally | c-wasm | 43 | 341 ms, 0.40× | 341 ms, 0.40× | 8.44 MiB, 1.02× |
| tally | cpp-wasm | 43 | 340 ms, 0.40× | 341 ms, 0.40× | 8.57 MiB, 1.01× |
| tally | rust-wasm | 43 | 423 ms, 0.32× | 423 ms, 0.32× | 8.31 MiB, 1.04× |
| server | c-wasm | 20 | 934 ms, 0.99× | 1.20 s, 0.77× | 7.74 MiB, 1.00× |
| server | cpp-wasm | 20 | 936 ms, 0.99× | 1.04 s, 0.89× | 7.86 MiB, 0.99× |
| server | rust-wasm | 20 | 933 ms, 0.99× | 1.01 s, 0.92× | 7.99 MiB, 0.97× |
| files | c-wasm | 160 | 67.5 ms, 0.92× | 67.7 ms, 0.92× | 8.30 MiB, 1.00× |
| files | cpp-wasm | 160 | 67.7 ms, 0.92× | 67.8 ms, 0.92× | 8.31 MiB, 1.00× |
| files | rust-wasm | 160 | 67.7 ms, 0.92× | 67.9 ms, 0.92× | 8.51 MiB, 0.98× |
| readings | c-wasm | 111 | 107 ms, 0.93× | 107 ms, 0.93× | 7.82 MiB, 0.98× |
| readings | cpp-wasm | 111 | 105 ms, 0.94× | 105 ms, 0.95× | 7.81 MiB, 0.98× |
| readings | rust-wasm | 111 | 107 ms, 0.93× | 108 ms, 0.93× | 7.98 MiB, 0.96× |
