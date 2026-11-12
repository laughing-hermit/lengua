# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 43813aef065cae8d91368d4891f6823f48ad9d82, in https://github.com/laughing-hermit/lengua/actions/runs/43131768685. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records lengua's idea and the module, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (6.01 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ✗ ✗ ≈ ✓ | ✗ ✗ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ✓ ≈ ≈ ✓ | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ≈ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ≈ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 empty c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 empty cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 sort rust cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 sort rust wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 server c wall: lengua is level now, and KNOWN_BEHIND line 162 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 2.70× | 2.47× | 4.50× | 439.79× |
| add | 2.30× | 2.11× | 3.66× | 435.72× |
| hypotenuse | 2.03× | 1.94× | 3.26× | 435.75× |
| upcase | 1.82× | 1.80× | 3.34× | 435.77× |
| upcase-stdin | 1.60× | 1.60× | 6.27× | 435.72× |
| shout | 1.83× | 1.86× | 3.09× | 435.73× |
| wc | 1.41× | 1.41× | 5.07× | 435.77× |
| histogram | 1.09× | 1.09× | 5.16× | 435.75× |
| crc32c | 2.81× | 2.80× | 5.17× | 435.75× |
| sort | 1.08× | 1.08× | 1.01× | 435.91× |
| tally | 0.44× | 0.44× | 4.21× | 435.95× |
| server | 1.06× | 1.81× | 7.50× | 435.77× |
| files | 3.29× | 3.28× | 5.44× | 435.76× |
| readings | 3.15× | 3.14× | 5.10× | 435.72× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 40 | behind, 1.13×, p 5.4e-5 | behind, 1.12×, p 4.8e-5 | level, 1.01× | ahead, 0.89× |
| empty | cpp-wasm | 40 | behind, 1.12×, p 3.7e-4 | behind, 1.12×, p 4.3e-4 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.91×, p 1.5e-7 | ahead, 0.93× | ahead, 0.01× |
| empty | c | 20 | behind, 2.70×, p 3.4e-8 | behind, 2.47×, p 3.4e-8 | behind, 4.50× | behind, 439.79× |
| empty | cpp | 20 | behind, 2.67×, p 3.4e-8 | behind, 2.47×, p 3.4e-8 | behind, 4.50× | behind, 439.79× |
| empty | rust | 20 | behind, 1.89×, p 3.4e-8 | behind, 1.82×, p 3.4e-8 | behind, 3.23× | behind, 22.17× |
| add | c-wasm | 20 | ahead, 0.78×, p 6.1e-8 | ahead, 0.79×, p 3.0e-7 | level, 0.96× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.80×, p 1.4e-7 | ahead, 0.80×, p 6.0e-7 | level, 0.96× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.69×, p 3.4e-8 | ahead, 0.70×, p 3.4e-8 | level, 0.96× | ahead, 0.02× |
| add | c | 20 | behind, 2.26×, p 3.4e-8 | behind, 2.09×, p 3.4e-8 | behind, 3.46× | behind, 435.72× |
| add | cpp | 20 | behind, 2.30×, p 3.4e-8 | behind, 2.11×, p 3.4e-8 | behind, 3.66× | behind, 435.72× |
| add | rust | 20 | behind, 1.74×, p 3.4e-8 | behind, 1.66×, p 3.4e-8 | behind, 3.21× | behind, 21.51× |
| hypotenuse | c-wasm | 20 | ahead, 0.80×, p 2.0e-7 | ahead, 0.82×, p 2.6e-7 | level, 0.98× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 20 | ahead, 0.80×, p 1.2e-6 | ahead, 0.82×, p 9.0e-7 | level, 0.97× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.03×, p 3.4e-8 | behind, 1.94×, p 3.4e-8 | behind, 3.07× | behind, 435.75× |
| hypotenuse | cpp | 20 | behind, 2.00×, p 3.4e-8 | behind, 1.92×, p 3.4e-8 | behind, 3.00× | behind, 435.75× |
| hypotenuse | rust | 20 | behind, 1.78×, p 3.4e-8 | behind, 1.71×, p 3.4e-8 | behind, 3.26× | behind, 21.33× |
| upcase | c-wasm | 40 | ahead, 0.94×, p 6.8e-6 | level, 0.95×, p 0.001 | level, 0.99× | ahead, 0.06× |
| upcase | cpp-wasm | 80 | level, 0.96×, p 1.1e-6 | ahead, 0.95×, p 5.4e-5 | level, 0.99× | ahead, 0.06× |
| upcase | rust-wasm | 40 | ahead, 0.89×, p 6.3e-11 | ahead, 0.91×, p 2.5e-6 | level, 0.98× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.82×, p 3.4e-8 | behind, 1.80×, p 3.4e-8 | behind, 3.34× | behind, 435.77× |
| upcase | cpp | 20 | behind, 1.82×, p 3.4e-8 | behind, 1.79×, p 5.3e-8 | behind, 3.34× | behind, 435.77× |
| upcase | rust | 20 | behind, 1.61×, p 3.4e-8 | behind, 1.60×, p 3.4e-8 | behind, 2.90× | behind, 21.56× |
| upcase-stdin | c-wasm | 40 | ahead, 0.92×, p 0.001 | ahead, 0.92×, p 0.001 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.35×, p 3.4e-8 | ahead, 0.35×, p 3.4e-8 | level, 0.99× | behind, 1.32× |
| upcase-stdin | rust-wasm | 40 | ahead, 0.91×, p 8.6e-5 | ahead, 0.90×, p 8.6e-5 | level, 0.97× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.49×, p 3.4e-8 | behind, 1.49×, p 3.4e-8 | behind, 6.16× | behind, 435.72× |
| upcase-stdin | cpp | 20 | behind, 1.49×, p 3.4e-8 | behind, 1.49×, p 3.4e-8 | behind, 6.27× | behind, 435.72× |
| upcase-stdin | rust | 20 | behind, 1.60×, p 3.4e-8 | behind, 1.60×, p 3.4e-8 | behind, 4.39× | behind, 21.94× |
| shout | c-wasm | 2000 | level, 0.99×, p 7.5e-10 | level, 0.98×, p 0.092 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 0.99×, p 6.4e-9 | level, 0.98×, p 0.173 | level, 1.00× | ahead, 0.40× |
| shout | rust-wasm | 40 | ahead, 0.86×, p 5.3e-11 | ahead, 0.85×, p 3.9e-5 | level, 0.96× | ahead, 0.03× |
| shout | c | 20 | behind, 1.83×, p 3.4e-8 | behind, 1.86×, p 3.4e-8 | behind, 3.09× | behind, 435.73× |
| shout | cpp | 20 | behind, 1.83×, p 3.4e-8 | behind, 1.86×, p 3.4e-8 | behind, 3.04× | behind, 435.73× |
| shout | rust | 20 | behind, 1.54×, p 3.4e-8 | behind, 1.56×, p 3.4e-8 | behind, 2.61× | behind, 21.68× |
| wc | c-wasm | 20 | ahead, 0.91×, p 3.4e-8 | ahead, 0.91×, p 3.4e-8 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.86×, p 3.4e-8 | ahead, 0.86×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| wc | c | 20 | behind, 1.41×, p 3.4e-8 | behind, 1.41×, p 3.4e-8 | behind, 4.97× | behind, 435.77× |
| wc | cpp | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | behind, 5.07× | behind, 435.77× |
| wc | rust | 20 | behind, 1.26×, p 3.4e-8 | behind, 1.26×, p 3.4e-8 | behind, 4.48× | behind, 21.13× |
| histogram | c-wasm | 20 | ahead, 0.74×, p 3.4e-8 | ahead, 0.74×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.80×, p 3.4e-8 | ahead, 0.80×, p 3.4e-8 | level, 0.99× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.09×, p 8.0e-5 | behind, 1.09×, p 8.0e-5 | behind, 5.16× | behind, 435.75× |
| histogram | cpp | 20 | behind, 1.09×, p 9.6e-8 | behind, 1.09×, p 9.6e-8 | behind, 5.16× | behind, 435.75× |
| histogram | rust | 20 | behind, 1.09×, p 3.3e-6 | behind, 1.09×, p 3.3e-6 | behind, 4.47× | behind, 21.17× |
| crc32c | c-wasm | 20 | ahead, 0.76×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.77×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.76×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.75×, p 3.4e-8 | behind, 2.73×, p 3.4e-8 | behind, 5.17× | behind, 435.75× |
| crc32c | cpp | 20 | behind, 2.74×, p 3.4e-8 | behind, 2.73×, p 3.4e-8 | behind, 5.17× | behind, 435.75× |
| crc32c | rust | 20 | behind, 2.81×, p 3.4e-8 | behind, 2.80×, p 3.4e-8 | behind, 4.51× | behind, 21.71× |
| sort | c-wasm | 20 | ahead, 0.20×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.55×, p 3.4e-8 | ahead, 0.55×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.60×, p 3.4e-8 | ahead, 0.60×, p 3.4e-8 | ahead, 0.89× | behind, 435.91× |
| sort | cpp | 20 | ahead, 0.69×, p 3.4e-8 | ahead, 0.69×, p 3.4e-8 | ahead, 0.93× | behind, 338.00× |
| sort | rust | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.08×, p 3.4e-8 | level, 1.01× | behind, 21.17× |
| tally | c-wasm | 20 | ahead, 0.36×, p 3.4e-8 | ahead, 0.36×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.35×, p 3.4e-8 | ahead, 0.35×, p 3.4e-8 | level, 1.01× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.29×, p 3.4e-8 | ahead, 0.29×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | behind, 4.09× | behind, 435.95× |
| tally | cpp | 20 | ahead, 0.44×, p 3.4e-8 | ahead, 0.44×, p 3.4e-8 | behind, 2.01× | behind, 339.78× |
| tally | rust | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | behind, 4.21× | behind, 20.60× |
| server | c-wasm | 145 | level, 1.00×, p 0.219 | level, 0.97×, p 1.1e-35 | level, 0.99× | ahead, 0.12× |
| server | cpp-wasm | 145 | level, 1.00×, p 0.064 | level, 0.97×, p 1.7e-35 | level, 0.99× | ahead, 0.06× |
| server | rust-wasm | 145 | level, 1.00×, p 0.008 | level, 0.98×, p 8.6e-36 | level, 0.97× | ahead, 0.03× |
| server | c | 20 | behind, 1.05×, p 3.4e-8 | level, 1.05×, p 3.4e-8 | behind, 7.50× | behind, 435.77× |
| server | cpp | 20 | behind, 1.06×, p 3.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 2.60× | behind, 432.90× |
| server | rust | 20 | ahead, 0.93×, p 3.4e-8 | behind, 1.81×, p 3.4e-8 | behind, 3.40× | behind, 19.73× |
| files | c-wasm | 20 | ahead, 0.95×, p 1.6e-5 | ahead, 0.95×, p 1.6e-5 | level, 0.99× | ahead, 0.05× |
| files | cpp-wasm | 20 | level, 0.95×, p 5.5e-6 | level, 0.95×, p 5.5e-6 | level, 0.99× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.94×, p 4.0e-7 | ahead, 0.94×, p 4.0e-7 | level, 0.98× | ahead, 0.03× |
| files | c | 20 | behind, 2.47×, p 3.4e-8 | behind, 2.47×, p 3.4e-8 | behind, 5.44× | behind, 435.76× |
| files | cpp | 20 | behind, 2.48×, p 3.4e-8 | behind, 2.47×, p 3.4e-8 | behind, 5.24× | behind, 435.76× |
| files | rust | 20 | behind, 3.29×, p 3.4e-8 | behind, 3.28×, p 3.4e-8 | behind, 4.56× | behind, 21.24× |
| readings | c-wasm | 20 | ahead, 0.85×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.95×, p 4.6e-7 | ahead, 0.95×, p 4.6e-7 | level, 0.99× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.85×, p 3.4e-8 | ahead, 0.85×, p 3.4e-8 | level, 0.96× | ahead, 0.02× |
| readings | c | 20 | behind, 3.15×, p 3.4e-8 | behind, 3.14×, p 3.4e-8 | behind, 5.10× | behind, 435.72× |
| readings | cpp | 20 | behind, 2.87×, p 3.4e-8 | behind, 2.86×, p 3.4e-8 | behind, 2.17× | behind, 435.72× |
| readings | rust | 20 | behind, 3.13×, p 3.4e-8 | behind, 3.12×, p 3.4e-8 | behind, 4.77× | behind, 21.16× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 1.40 ms | 1.24 ms | 1.25 ms | 1.56 ms | 522 µs | 526 µs | 742 µs |
| wall time | 1.49 ms | 1.33 ms | 1.34 ms | 1.64 ms | 607 µs | 608 µs | 825 µs |
| peak memory | 5.56 MiB | 5.53 MiB | 5.56 MiB | 5.88 MiB | 1.22 MiB | 1.22 MiB | 1.70 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 14.0 KiB | 14.0 KiB | 278 KiB |

Its rounds took 1 seconds.

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
| CPU time | 1.30 ms | 1.67 ms | 1.64 ms | 1.88 ms | 578 µs | 568 µs | 750 µs |
| wall time | 1.39 ms | 1.76 ms | 1.74 ms | 1.97 ms | 662 µs | 658 µs | 837 µs |
| peak memory | 5.52 MiB | 5.75 MiB | 5.76 MiB | 5.75 MiB | 1.60 MiB | 1.51 MiB | 1.72 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

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
| CPU time | 1.33 ms | 1.66 ms | 1.66 ms | 1.89 ms | 655 µs | 665 µs | 750 µs |
| wall time | 1.43 ms | 1.75 ms | 1.75 ms | 1.98 ms | 738 µs | 746 µs | 836 µs |
| peak memory | 5.60 MiB | 5.70 MiB | 5.75 MiB | 5.74 MiB | 1.82 MiB | 1.87 MiB | 1.72 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 14.1 KiB | 14.1 KiB | 289 KiB |

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
| counted runs | 80 | 40 | 80 | 40 | 20 | 20 | 20 |
| CPU time | 2.85 ms | 3.05 ms | 2.97 ms | 3.20 ms | 1.58 ms | 1.58 ms | 1.78 ms |
| wall time | 3.06 ms | 3.23 ms | 3.23 ms | 3.40 ms | 1.72 ms | 1.73 ms | 1.94 ms |
| peak memory | 7.90 MiB | 7.98 MiB | 7.94 MiB | 8.06 MiB | 2.35 MiB | 2.35 MiB | 2.71 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 2 seconds.

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
| counted runs | 40 | 40 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 12.4 ms | 13.5 ms | 35.5 ms | 13.6 ms | 8.28 ms | 8.29 ms | 7.71 ms |
| wall time | 12.5 ms | 13.6 ms | 35.6 ms | 13.9 ms | 8.37 ms | 8.39 ms | 7.81 ms |
| peak memory | 8.11 MiB | 8.11 MiB | 8.13 MiB | 8.34 MiB | 1.31 MiB | 1.29 MiB | 1.84 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 281 KiB |

Its rounds took 5 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 40 | 20 | 20 | 20 |
| CPU time | 2.26 ms | 2.28 ms | 2.28 ms | 2.53 ms | 1.20 ms | 1.19 ms | 1.42 ms |
| wall time | 2.41 ms | 2.45 ms | 2.45 ms | 2.72 ms | 1.28 ms | 1.28 ms | 1.52 ms |
| peak memory | 6.98 MiB | 7.01 MiB | 7.01 MiB | 7.15 MiB | 2.19 MiB | 2.22 MiB | 2.59 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

Its rounds took 38 seconds.

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
| CPU time | 22.7 ms | 25.1 ms | 227 ms | 26.4 ms | 16.1 ms | 168 ms | 18.0 ms |
| wall time | 22.8 ms | 25.2 ms | 227 ms | 26.5 ms | 16.2 ms | 168 ms | 18.1 ms |
| peak memory | 8.27 MiB | 8.26 MiB | 8.25 MiB | 8.37 MiB | 1.66 MiB | 1.63 MiB | 1.85 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 12 seconds.

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
| CPU time | 34.2 ms | 46.3 ms | 47.1 ms | 42.6 ms | 31.3 ms | 31.2 ms | 31.4 ms |
| wall time | 34.3 ms | 46.5 ms | 47.2 ms | 42.7 ms | 31.4 ms | 31.3 ms | 31.5 ms |
| peak memory | 8.26 MiB | 8.22 MiB | 8.25 MiB | 8.37 MiB | 1.60 MiB | 1.60 MiB | 1.85 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 7 seconds.

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
| CPU time | 41.5 ms | 54.8 ms | 54.0 ms | 54.9 ms | 15.1 ms | 15.1 ms | 14.8 ms |
| wall time | 41.6 ms | 54.9 ms | 54.2 ms | 55.0 ms | 15.2 ms | 15.2 ms | 14.9 ms |
| peak memory | 8.28 MiB | 8.24 MiB | 8.28 MiB | 8.54 MiB | 1.60 MiB | 1.60 MiB | 1.84 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

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
| CPU time | 264 ms | 1.31 s | 477 ms | 447 ms | 442 ms | 380 ms | 244 ms |
| wall time | 264 ms | 1.31 s | 477 ms | 447 ms | 442 ms | 380 ms | 244 ms |
| peak memory | 77.7 MiB | 77.8 MiB | 85.8 MiB | 85.8 MiB | 87.0 MiB | 83.2 MiB | 76.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 291 KiB |

Its rounds took 84 seconds.

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
| CPU time | 127 ms | 357 ms | 360 ms | 440 ms | 304 ms | 291 ms | 318 ms |
| wall time | 127 ms | 357 ms | 361 ms | 440 ms | 305 ms | 291 ms | 318 ms |
| peak memory | 9.11 MiB | 8.97 MiB | 9.03 MiB | 8.75 MiB | 2.23 MiB | 4.54 MiB | 2.16 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 299 KiB |

Its rounds took 51 seconds.

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
| counted runs | 145 | 145 | 145 | 145 | 20 | 20 | 20 |
| CPU time | 861 ms | 861 ms | 862 ms | 863 ms | 817 ms | 809 ms | 926 ms |
| wall time | 862 ms | 885 ms | 886 ms | 883 ms | 821 ms | 811 ms | 475 ms |
| peak memory | 8.18 MiB | 8.29 MiB | 8.30 MiB | 8.41 MiB | 1.10 MiB | 3.16 MiB | 2.42 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 14.1 KiB | 14.2 KiB | 312 KiB |

Its rounds took 621 seconds.

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
| CPU time | 72.8 ms | 76.8 ms | 76.4 ms | 77.5 ms | 29.4 ms | 29.4 ms | 22.1 ms |
| wall time | 72.9 ms | 76.9 ms | 76.6 ms | 77.6 ms | 29.5 ms | 29.5 ms | 22.2 ms |
| peak memory | 8.72 MiB | 8.80 MiB | 8.85 MiB | 8.92 MiB | 1.60 MiB | 1.66 MiB | 1.91 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 10 seconds.

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
| CPU time | 96.5 ms | 113 ms | 102 ms | 114 ms | 30.7 ms | 33.6 ms | 30.9 ms |
| wall time | 96.7 ms | 113 ms | 102 ms | 114 ms | 30.8 ms | 33.8 ms | 31.0 ms |
| peak memory | 8.18 MiB | 8.30 MiB | 8.29 MiB | 8.51 MiB | 1.60 MiB | 3.76 MiB | 1.71 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 13 seconds.

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
| empty | 160, 160 in 4 s | 1.40 ms against 1.30 ms: +105 µs (+97.0 µs to +113 µs) | 1.50 ms against 1.39 ms: +105 µs (+96.2 µs to +114 µs) | 5.55 MiB against 5.47 MiB: +88.0 KiB (+64.0 KiB to +108 KiB) |
| add | 160, 160 in 4 s | 1.29 ms against 1.18 ms: +112 µs (+103 µs to +123 µs) | 1.38 ms against 1.27 ms: +115 µs (+105 µs to +125 µs) | 5.59 MiB against 5.48 MiB: +120 KiB (+92.0 KiB to +148 KiB) |
| hypotenuse | 160, 160 in 4 s | 1.32 ms against 1.19 ms: +134 µs (+125 µs to +142 µs) | 1.41 ms against 1.28 ms: +134 µs (+125 µs to +143 µs) | 5.59 MiB against 5.48 MiB: +104 KiB (+80.0 KiB to +136 KiB) |
| upcase | 160, 160 in 5 s | 2.86 ms against 2.77 ms: +125 µs (+70.0 µs to +173 µs) | 3.07 ms against 2.96 ms: +118 µs (+33.8 µs to +192 µs) | 7.98 MiB against 7.92 MiB: +88.0 KiB (+4.00 KiB to +180 KiB) |
| upcase-stdin | 160, 160 in 17 s | 12.3 ms against 12.0 ms: +171 µs (+74.0 µs to +298 µs) | 12.4 ms against 12.1 ms: +172 µs (+60.3 µs to +306 µs) | 8.10 MiB against 8.11 MiB: +0 B (-16.0 KiB to +24.0 KiB) |
| shout | 160, 160 in 5 s | 2.28 ms against 2.13 ms: +134 µs (+108 µs to +163 µs) | 2.43 ms against 2.30 ms: +158 µs (+102 µs to +271 µs) | 7.02 MiB against 7.01 MiB: +12.0 KiB (-24.0 KiB to +52.0 KiB) |
| wc | 160, 160 in 55 s | 22.6 ms against 22.5 ms: +140 µs (+48.0 µs to +222 µs) | 22.7 ms against 22.6 ms: +149 µs (+58.7 µs to +237 µs) | 8.28 MiB against 8.26 MiB: +8.00 KiB (-4.00 KiB to +24.0 KiB) |
| histogram | 160, 160 in 36 s | 34.5 ms against 34.1 ms: +164 µs (-118 µs to +460 µs) | 34.6 ms against 34.3 ms: +177 µs (-114 µs to +482 µs) | 8.24 MiB against 8.14 MiB: +104 KiB (+88.0 KiB to +120 KiB) |
| crc32c | 160, 160 in 43 s | 40.7 ms against 40.5 ms: +240 µs (+11.0 µs to +517 µs) | 40.9 ms against 40.6 ms: +252 µs (+15.5 µs to +540 µs) | 8.25 MiB against 8.26 MiB: -8.00 KiB (-24.0 KiB to +0 B) |
| sort | 22, 22 in 61 s | 263 ms against 261 ms: +1.96 ms (+136 µs to +3.61 ms) | 264 ms against 261 ms: +1.89 ms (+5.60 µs to +3.54 ms) | 77.7 MiB against 77.7 MiB: +8.00 KiB (-52.0 KiB to +64.0 KiB) |
| tally | 42, 42 in 61 s | 128 ms against 127 ms: +428 µs (-363 µs to +1.19 ms) | 128 ms against 127 ms: +434 µs (-357 µs to +1.18 ms) | 9.12 MiB against 9.02 MiB: +112 KiB (+76.0 KiB to +136 KiB) |
| server | 20, 20 in 95 s | 858 ms against 855 ms: +3.22 ms (-3.71 ms to +9.82 ms) | 863 ms against 856 ms: +4.90 ms (-2.26 ms to +10.6 ms) | 8.17 MiB against 8.24 MiB: -44.0 KiB (-120 KiB to +40.0 KiB) |
| files | 153, 153 in 60 s | 72.4 ms against 70.5 ms: +2.13 ms (+1.76 ms to +2.55 ms) | 72.6 ms against 70.7 ms: +2.13 ms (+1.77 ms to +2.55 ms) | 8.72 MiB against 8.69 MiB: +16.0 KiB (+4.00 KiB to +32.0 KiB) |
| readings | 110, 110 in 60 s | 96.5 ms against 96.7 ms: -70.0 µs (-339 µs to +200 µs) | 96.7 ms against 96.9 ms: -59.4 µs (-334 µs to +215 µs) | 8.17 MiB against 8.16 MiB: +12.0 KiB (+0 B to +28.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 1.17 ms, 1.20× | 1.26 ms, 1.19× | 5.48 MiB, 1.01× |
| empty | cpp-wasm | 160 | 1.17 ms, 1.19× | 1.26 ms, 1.19× | 5.48 MiB, 1.01× |
| empty | rust-wasm | 160 | 1.45 ms, 0.96× | 1.54 ms, 0.97× | 5.58 MiB, 0.99× |
| add | c-wasm | 160 | 1.51 ms, 0.85× | 1.61 ms, 0.86× | 5.62 MiB, 0.99× |
| add | cpp-wasm | 160 | 1.50 ms, 0.86× | 1.59 ms, 0.86× | 5.62 MiB, 0.99× |
| add | rust-wasm | 160 | 1.69 ms, 0.76× | 1.79 ms, 0.77× | 5.67 MiB, 0.99× |
| hypotenuse | c-wasm | 160 | 1.49 ms, 0.89× | 1.59 ms, 0.89× | 5.62 MiB, 0.99× |
| hypotenuse | cpp-wasm | 160 | 1.49 ms, 0.89× | 1.58 ms, 0.89× | 5.61 MiB, 1.00× |
| hypotenuse | rust-wasm | 160 | 1.71 ms, 0.77× | 1.80 ms, 0.78× | 5.65 MiB, 0.99× |
| upcase | c-wasm | 160 | 2.91 ms, 0.98× | 3.13 ms, 0.98× | 8.02 MiB, 0.99× |
| upcase | cpp-wasm | 160 | 2.87 ms, 1.00× | 3.13 ms, 0.98× | 7.94 MiB, 1.01× |
| upcase | rust-wasm | 160 | 3.10 ms, 0.92× | 3.34 ms, 0.92× | 8.04 MiB, 0.99× |
| upcase-stdin | c-wasm | 160 | 13.3 ms, 0.92× | 13.5 ms, 0.92× | 8.11 MiB, 1.00× |
| upcase-stdin | cpp-wasm | 160 | 35.3 ms, 0.35× | 35.5 ms, 0.35× | 8.13 MiB, 1.00× |
| upcase-stdin | rust-wasm | 160 | 13.0 ms, 0.94× | 13.1 ms, 0.95× | 8.33 MiB, 0.97× |
| shout | c-wasm | 160 | 2.17 ms, 1.05× | 2.35 ms, 1.03× | 7.04 MiB, 1.00× |
| shout | cpp-wasm | 160 | 2.16 ms, 1.05× | 2.36 ms, 1.03× | 7.02 MiB, 1.00× |
| shout | rust-wasm | 160 | 2.40 ms, 0.95× | 2.58 ms, 0.94× | 7.30 MiB, 0.96× |
| wc | c-wasm | 160 | 25.1 ms, 0.90× | 25.3 ms, 0.90× | 8.26 MiB, 1.00× |
| wc | cpp-wasm | 160 | 227 ms, 0.10× | 227 ms, 0.10× | 8.26 MiB, 1.00× |
| wc | rust-wasm | 160 | 25.4 ms, 0.89× | 25.6 ms, 0.89× | 8.39 MiB, 0.99× |
| histogram | c-wasm | 160 | 46.4 ms, 0.74× | 46.6 ms, 0.74× | 8.26 MiB, 1.00× |
| histogram | cpp-wasm | 160 | 46.2 ms, 0.75× | 46.5 ms, 0.75× | 8.27 MiB, 1.00× |
| histogram | rust-wasm | 160 | 42.8 ms, 0.81× | 43.0 ms, 0.81× | 8.38 MiB, 0.98× |
| crc32c | c-wasm | 160 | 54.4 ms, 0.75× | 54.5 ms, 0.75× | 8.27 MiB, 1.00× |
| crc32c | cpp-wasm | 160 | 53.8 ms, 0.76× | 53.9 ms, 0.76× | 8.27 MiB, 1.00× |
| crc32c | rust-wasm | 160 | 54.8 ms, 0.74× | 55.0 ms, 0.74× | 8.38 MiB, 0.98× |
| sort | c-wasm | 22 | 1.31 s, 0.20× | 1.31 s, 0.20× | 77.7 MiB, 1.00× |
| sort | cpp-wasm | 22 | 475 ms, 0.55× | 475 ms, 0.55× | 85.9 MiB, 0.90× |
| sort | rust-wasm | 22 | 444 ms, 0.59× | 444 ms, 0.59× | 85.7 MiB, 0.91× |
| tally | c-wasm | 42 | 356 ms, 0.36× | 356 ms, 0.36× | 8.89 MiB, 1.03× |
| tally | cpp-wasm | 42 | 364 ms, 0.35× | 364 ms, 0.35× | 8.99 MiB, 1.01× |
| tally | rust-wasm | 42 | 441 ms, 0.29× | 441 ms, 0.29× | 8.75 MiB, 1.04× |
| server | c-wasm | 20 | 858 ms, 1.00× | 887 ms, 0.97× | 8.21 MiB, 0.99× |
| server | cpp-wasm | 20 | 860 ms, 1.00× | 902 ms, 0.96× | 8.28 MiB, 0.99× |
| server | rust-wasm | 20 | 860 ms, 1.00× | 881 ms, 0.98× | 8.38 MiB, 0.97× |
| files | c-wasm | 153 | 76.5 ms, 0.95× | 76.7 ms, 0.95× | 8.69 MiB, 1.00× |
| files | cpp-wasm | 153 | 76.8 ms, 0.94× | 77.0 ms, 0.94× | 8.69 MiB, 1.00× |
| files | rust-wasm | 153 | 75.8 ms, 0.96× | 75.9 ms, 0.96× | 8.90 MiB, 0.98× |
| readings | c-wasm | 110 | 114 ms, 0.85× | 114 ms, 0.85× | 8.29 MiB, 0.99× |
| readings | cpp-wasm | 110 | 102 ms, 0.95× | 102 ms, 0.95× | 8.26 MiB, 0.99× |
| readings | rust-wasm | 110 | 114 ms, 0.85× | 114 ms, 0.85× | 8.40 MiB, 0.97× |
