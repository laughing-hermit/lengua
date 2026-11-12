# lengua against C, C++ and Rust on windows-aarch64

Measured on windows-aarch64 (Cobalt 100, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 43813aef065cae8d91368d4891f6823f48ad9d82, in https://github.com/laughing-hermit/lengua/actions/runs/43131768685. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records lengua's idea and the module, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (5.06 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-aarch64 upcase-stdin c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 upcase-stdin cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-aarch64 wc c wall: lengua is level now, and KNOWN_BEHIND line 728 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc cpp wall: lengua is level now, and KNOWN_BEHIND line 731 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 tally rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 765 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.42× | 1.54× | 51.10× |
| add | – | 1.41× | 1.53× | 44.72× |
| hypotenuse | – | 1.42× | 1.53× | 44.34× |
| upcase | – | 1.42× | 1.42× | 43.41× |
| upcase-stdin | 2.00× | 1.32× | 1.52× | 48.71× |
| shout | – | 1.38× | 1.42× | 46.73× |
| wc | 1.00× | 1.05× | 1.52× | 45.31× |
| histogram | 1.33× | 1.15× | 1.51× | 44.53× |
| crc32c | 3.00× | 2.08× | 1.51× | 47.16× |
| sort | 1.13× | 1.13× | 0.96× | 44.74× |
| tally | 0.65× | 0.63× | 1.50× | 41.52× |
| server | 1.15× | 1.57× | 1.42× | 45.91× |
| files | 2.20× | 1.87× | 1.53× | 43.59× |
| readings | 3.50× | 2.96× | 1.53× | 43.41× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.00×, p 0.293 | level, 1.00×, p 0.077 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.00×, p 0.188 | level, 1.00×, p 0.227 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 0.376 | level, 0.98×, p 1.2e-104 | level, 1.00× | ahead, 0.01× |
| empty | c | 160 | behind, p 9.7e-10 | behind, 1.42×, p 2.9e-54 | behind, 1.53× | behind, 48.24× |
| empty | cpp | 80 | behind, p 0.001 | behind, 1.41×, p 4.7e-28 | behind, 1.54× | behind, 48.24× |
| empty | rust | 80 | behind, p 4.0e-4 | behind, 1.35×, p 4.7e-28 | behind, 1.44× | behind, 51.10× |
| add | c-wasm | 2000 | level, 1.00×, p 0.005 | level, 0.98×, p 3.3e-70 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.00×, p 0.119 | level, 0.99×, p 2.6e-49 | level, 1.00× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, 1.00×, p 0.048 | level, 0.97×, p 6.5e-165 | level, 1.00× | ahead, 0.02× |
| add | c | 20 | behind, p 2.4e-4 | behind, 1.42×, p 3.4e-8 | behind, 1.53× | behind, 36.40× |
| add | cpp | 160 | behind, p 1.8e-5 | behind, 1.41×, p 5.7e-53 | behind, 1.53× | behind, 36.40× |
| add | rust | 80 | behind, p 1.2e-4 | behind, 1.35×, p 4.7e-28 | behind, 1.44× | behind, 44.72× |
| hypotenuse | c-wasm | 2000 | level, 1.00×, p 0.120 | level, 0.99×, p 6.2e-42 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.00×, p 0.215 | level, 0.99×, p 2.6e-39 | level, 1.00× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.022 | level, 0.98×, p 4.2e-151 | level, 1.00× | ahead, 0.03× |
| hypotenuse | c | 160 | behind, p 1.4e-6 | behind, 1.42×, p 2.9e-54 | behind, 1.53× | behind, 36.15× |
| hypotenuse | cpp | 80 | behind, p 7.4e-4 | behind, 1.42×, p 4.7e-28 | behind, 1.53× | behind, 36.02× |
| hypotenuse | rust | 80 | behind, p 2.3e-4 | behind, 1.36×, p 4.7e-28 | behind, 1.44× | behind, 44.34× |
| upcase | c-wasm | 2000 | level, 1.00×, p 0.297 | level, 0.99×, p 1.0e-42 | level, 1.00× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 1.00×, p 0.047 | level, 0.99×, p 6.2e-38 | level, 1.00× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.00×, p 0.003 | level, 0.98×, p 5.4e-183 | level, 0.97× | ahead, 0.03× |
| upcase | c | 40 | behind, p 9.6e-5 | behind, 1.42×, p 1.3e-13 | behind, 1.39× | behind, 34.02× |
| upcase | cpp | 40 | behind, p 1.7e-5 | behind, 1.42×, p 8.3e-15 | behind, 1.39× | behind, 34.02× |
| upcase | rust | 80 | behind, p 3.1e-5 | behind, 1.37×, p 1.2e-25 | behind, 1.42× | behind, 43.41× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.285 | level, 1.00×, p 0.054 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.414 | level, 1.00×, p 2.1e-6 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.001 | level, 0.98×, p 6.0e-137 | level, 0.99× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 1.2e-5 | behind, 1.31×, p 3.4e-8 | behind, 1.52× | behind, 46.52× |
| upcase-stdin | cpp | 20 | behind, 2.00×, p 6.9e-5 | behind, 1.32×, p 3.4e-8 | behind, 1.52× | behind, 46.52× |
| upcase-stdin | rust | 40 | level, 1.00×, p 2.0e-5 | behind, 1.24×, p 7.2e-15 | behind, 1.44× | behind, 48.71× |
| shout | c-wasm | 2000 | level, 1.00×, p 0.121 | level, 1.00×, p 0.039 | level, 1.01× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.00×, p 0.496 | level, 1.00×, p 0.097 | level, 1.01× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, 1.00×, p 1.5e-4 | level, 0.98×, p 1.5e-136 | level, 0.99× | ahead, 0.03× |
| shout | c | 160 | behind, p 6.2e-5 | behind, 1.38×, p 2.9e-54 | behind, 1.40× | behind, 45.30× |
| shout | cpp | 160 | behind, p 8.1e-5 | behind, 1.38×, p 2.9e-54 | behind, 1.40× | behind, 45.30× |
| shout | rust | 160 | behind, p 1.1e-4 | behind, 1.33×, p 5.7e-53 | behind, 1.42× | behind, 46.73× |
| wc | c-wasm | 40 | ahead, 0.67×, p 6.1e-5 | ahead, 0.84×, p 7.2e-15 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.15×, p 5.2e-9 | ahead, 0.21×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 40 | ahead, 0.67×, p 1.3e-4 | ahead, 0.83×, p 7.2e-15 | level, 1.00× | ahead, 0.04× |
| wc | c | 160 | level, 1.00×, p 2.7e-4 | level, 1.05×, p 1.2e-45 | behind, 1.52× | behind, 36.03× |
| wc | cpp | 1280 | level, 1.00×, p 4.7e-7 | level, 1.03×, p 2.7e-242 | behind, 1.51× | behind, 36.03× |
| wc | rust | 2000 | level, 1.00×, p 0.100 | level, 1.01×, p 1.9e-62 | behind, 1.45× | behind, 45.31× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 3.1e-8 | ahead, 0.66×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 2.2e-8 | ahead, 0.67×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 640 | level, 1.00×, p 2.0e-6 | level, 0.97×, p 5.3e-129 | level, 1.00× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 7.8e-4 | behind, 1.14×, p 3.4e-8 | behind, 1.51× | behind, 36.15× |
| histogram | cpp | 20 | behind, 1.33×, p 1.7e-4 | behind, 1.15×, p 3.4e-8 | behind, 1.51× | behind, 36.15× |
| histogram | rust | 40 | behind, 1.33×, p 3.4e-5 | behind, 1.11×, p 7.2e-15 | behind, 1.44× | behind, 44.53× |
| crc32c | c-wasm | 20 | ahead, 0.75×, p 1.3e-5 | ahead, 0.85×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.75×, p 2.3e-5 | ahead, 0.84×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.75×, p 9.9e-5 | ahead, 0.80×, p 3.4e-8 | level, 1.00× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.00×, p 2.4e-8 | behind, 2.08×, p 3.4e-8 | behind, 1.51× | behind, 36.28× |
| crc32c | cpp | 20 | behind, 3.00×, p 2.4e-8 | behind, 2.06×, p 3.4e-8 | behind, 1.51× | behind, 36.28× |
| crc32c | rust | 20 | behind, 3.00×, p 3.2e-8 | behind, 1.95×, p 3.4e-8 | behind, 1.44× | behind, 47.16× |
| sort | c-wasm | 20 | ahead, 0.20×, p 2.3e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.59×, p 1.8e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.65×, p 2.0e-8 | ahead, 0.66×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.42×, p 2.2e-8 | ahead, 0.43×, p 3.4e-8 | level, 0.96× | behind, 44.74× |
| sort | cpp | 20 | ahead, 0.44×, p 2.1e-8 | ahead, 0.44×, p 3.4e-8 | ahead, 0.85× | behind, 40.86× |
| sort | rust | 20 | behind, 1.13×, p 9.7e-8 | behind, 1.13×, p 3.4e-8 | ahead, 0.90× | behind, 43.98× |
| tally | c-wasm | 20 | ahead, 0.55×, p 1.1e-8 | ahead, 0.53×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.55×, p 1.2e-8 | ahead, 0.53×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.41×, p 1.7e-8 | ahead, 0.40×, p 3.4e-8 | level, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.65×, p 1.0e-8 | ahead, 0.63×, p 3.4e-8 | behind, 1.46× | behind, 35.31× |
| tally | cpp | 20 | ahead, 0.65×, p 1.7e-8 | ahead, 0.63×, p 3.4e-8 | behind, 1.47× | behind, 34.26× |
| tally | rust | 20 | ahead, 0.61×, p 9.9e-9 | ahead, 0.62×, p 3.4e-8 | behind, 1.50× | behind, 41.52× |
| server | c-wasm | 72 | level, 0.99×, p 1.5e-5 | ahead, 0.88×, p 2.0e-25 | level, 1.01× | ahead, 0.12× |
| server | cpp-wasm | 72 | level, 1.01×, p 0.033 | ahead, 0.94×, p 3.6e-25 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 72 | level, 1.00×, p 0.001 | ahead, 0.92×, p 2.2e-25 | level, 0.99× | ahead, 0.03× |
| server | c | 20 | behind, 1.14×, p 2.8e-8 | behind, 1.15×, p 3.4e-8 | behind, 1.42× | behind, 45.91× |
| server | cpp | 20 | behind, 1.15×, p 2.9e-8 | behind, 1.14×, p 3.4e-8 | behind, 1.41× | behind, 44.92× |
| server | rust | 20 | ahead, 0.83×, p 6.4e-7 | behind, 1.57×, p 3.4e-8 | behind, 1.17× | behind, 38.01× |
| files | c-wasm | 20 | ahead, 0.92×, p 1.4e-4 | ahead, 0.92×, p 6.3e-6 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 20 | level, 0.96×, p 3.3e-4 | ahead, 0.92×, p 4.9e-6 | level, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 8.1e-4 | ahead, 0.91×, p 2.0e-6 | level, 0.99× | ahead, 0.03× |
| files | c | 20 | behind, 1.83×, p 3.9e-9 | behind, 1.72×, p 3.4e-8 | behind, 1.53× | behind, 34.24× |
| files | cpp | 20 | behind, 1.83×, p 9.2e-9 | behind, 1.72×, p 3.4e-8 | behind, 1.53× | behind, 34.24× |
| files | rust | 20 | behind, 2.20×, p 1.2e-8 | behind, 1.87×, p 3.4e-8 | behind, 1.46× | behind, 43.59× |
| readings | c-wasm | 160 | level, 1.00×, p 5.0e-4 | level, 0.98×, p 3.9e-49 | level, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 1280 | level, 1.00×, p 1.2e-4 | level, 0.99×, p 2.4e-110 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 40 | level, 1.00×, p 3.8e-4 | level, 0.95×, p 1.4e-13 | level, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 3.50×, p 3.9e-9 | behind, 2.96×, p 3.4e-8 | behind, 1.53× | behind, 36.40× |
| readings | cpp | 20 | behind, 3.50×, p 5.1e-9 | behind, 2.90×, p 3.4e-8 | behind, 1.53× | behind, 36.40× |
| readings | rust | 20 | behind, 2.33×, p 9.3e-9 | behind, 2.25×, p 3.4e-8 | behind, 1.44× | behind, 43.41× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 15.8 ms | 15.9 ms | 15.9 ms | 16.2 ms | 11.7 ms | 11.8 ms | 12.3 ms |
| peak memory | 7.14 MiB | 7.14 MiB | 7.14 MiB | 7.18 MiB | 4.66 MiB | 4.66 MiB | 4.95 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 108 KiB | 108 KiB | 102 KiB |

Its rounds took 272 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 160 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 15.7 ms | 16.0 ms | 15.9 ms | 16.1 ms | 11.2 ms | 11.1 ms | 11.7 ms |
| peak memory | 7.18 MiB | 7.20 MiB | 7.20 MiB | 7.17 MiB | 4.69 MiB | 4.70 MiB | 4.98 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 142 KiB | 142 KiB | 116 KiB |

Its rounds took 269 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.4 ms | 16.6 ms | 16.6 ms | 16.8 ms | 11.2 ms | 11.1 ms | 11.6 ms |
| peak memory | 7.18 MiB | 7.16 MiB | 7.16 MiB | 7.17 MiB | 4.69 MiB | 4.69 MiB | 4.98 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 144 KiB | 144 KiB | 117 KiB |

Its rounds took 282 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 40 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 18.5 ms | 18.7 ms | 18.7 ms | 19.0 ms | 13.3 ms | 13.3 ms | 13.7 ms |
| peak memory | 8.04 MiB | 8.05 MiB | 8.05 MiB | 8.25 MiB | 5.80 MiB | 5.80 MiB | 5.65 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 152 KiB | 152 KiB | 120 KiB |

Its rounds took 299 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 15.6 ms | 31.2 ms |
| wall time | 39.8 ms | 39.9 ms | 39.9 ms | 40.6 ms | 30.7 ms | 30.6 ms | 32.4 ms |
| peak memory | 7.18 MiB | 7.17 MiB | 7.17 MiB | 7.21 MiB | 4.73 MiB | 4.73 MiB | 4.97 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 112 KiB | 112 KiB | 106 KiB |

Its rounds took 469 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 160 | 160 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.6 ms | 16.6 ms | 16.6 ms | 17.0 ms | 12.0 ms | 11.9 ms | 12.4 ms |
| peak memory | 7.94 MiB | 7.89 MiB | 7.89 MiB | 8.04 MiB | 5.67 MiB | 5.67 MiB | 5.59 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 114 KiB | 114 KiB | 111 KiB |

Its rounds took 286 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 40 | 20 | 40 | 160 | 1280 | 2000 |
| CPU time | 31.2 ms | 46.9 ms | 203 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 44.1 ms | 52.2 ms | 212 ms | 52.9 ms | 41.8 ms | 42.5 ms | 43.7 ms |
| peak memory | 7.20 MiB | 7.22 MiB | 7.23 MiB | 7.24 MiB | 4.75 MiB | 4.76 MiB | 4.98 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 144 KiB | 144 KiB | 114 KiB |

Its rounds took 341 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 640 | 20 | 20 | 640 | 20 | 20 | 40 |
| CPU time | 62.5 ms | 93.8 ms | 93.8 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 62.8 ms | 95.6 ms | 94.8 ms | 65.0 ms | 55.8 ms | 55.2 ms | 57.1 ms |
| peak memory | 7.20 MiB | 7.21 MiB | 7.22 MiB | 7.20 MiB | 4.77 MiB | 4.77 MiB | 5.01 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 144 KiB | 144 KiB | 116 KiB |

Its rounds took 117 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 62.5 ms | 62.5 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 60.0 ms | 70.6 ms | 71.2 ms | 74.6 ms | 28.9 ms | 29.1 ms | 30.7 ms |
| peak memory | 7.20 MiB | 7.22 MiB | 7.21 MiB | 7.24 MiB | 4.77 MiB | 4.77 MiB | 5.00 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 143 KiB | 143 KiB | 110 KiB |

Its rounds took 11 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 266 ms | 1.36 s | 453 ms | 406 ms | 625 ms | 609 ms | 234 ms |
| wall time | 275 ms | 1.38 s | 467 ms | 419 ms | 637 ms | 624 ms | 244 ms |
| peak memory | 76.5 MiB | 76.5 MiB | 84.6 MiB | 84.6 MiB | 79.7 MiB | 89.7 MiB | 84.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 116 KiB | 127 KiB | 118 KiB |

Its rounds took 98 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 172 ms | 312 ms | 312 ms | 422 ms | 266 ms | 266 ms | 281 ms |
| wall time | 174 ms | 326 ms | 326 ms | 437 ms | 276 ms | 277 ms | 283 ms |
| peak memory | 7.90 MiB | 7.72 MiB | 7.73 MiB | 7.55 MiB | 5.41 MiB | 5.36 MiB | 5.28 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 147 KiB | 152 KiB | 125 KiB |

Its rounds took 51 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 72 | 72 | 72 | 72 | 20 | 20 | 20 |
| CPU time | 1.56 s | 1.58 s | 1.55 s | 1.56 s | 1.35 s | 1.34 s | 1.87 s |
| wall time | 1.57 s | 1.78 s | 1.67 s | 1.70 s | 1.37 s | 1.37 s | 997 ms |
| peak memory | 7.54 MiB | 7.48 MiB | 7.53 MiB | 7.63 MiB | 5.31 MiB | 5.33 MiB | 6.46 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 113 KiB | 116 KiB | 136 KiB |

Its rounds took 641 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 172 ms | 188 ms | 180 ms | 188 ms | 93.8 ms | 93.8 ms | 78.1 ms |
| wall time | 175 ms | 190 ms | 190 ms | 192 ms | 101 ms | 102 ms | 93.3 ms |
| peak memory | 7.52 MiB | 7.49 MiB | 7.49 MiB | 7.57 MiB | 4.91 MiB | 4.91 MiB | 5.17 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 152 KiB | 152 KiB | 119 KiB |

Its rounds took 29 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 160 | 1280 | 40 | 20 | 20 | 20 |
| CPU time | 109 ms | 109 ms | 109 ms | 109 ms | 31.2 ms | 31.2 ms | 46.9 ms |
| wall time | 114 ms | 117 ms | 115 ms | 119 ms | 38.3 ms | 39.1 ms | 50.3 ms |
| peak memory | 7.21 MiB | 7.23 MiB | 7.23 MiB | 7.20 MiB | 4.71 MiB | 4.71 MiB | 5.00 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 142 KiB | 142 KiB | 120 KiB |

Its rounds took 368 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 25 s | means 9.08 ms against 9.57 ms: -488 µs (-2.74 ms to +1.76 ms) | 15.8 ms against 15.3 ms: +464 µs (+387 µs to +542 µs) | 7.13 MiB against 7.03 MiB: +108 KiB (+104 KiB to +108 KiB) |
| add | 160, 160 in 26 s | means 9.47 ms against 9.28 ms: +195 µs (-2.13 ms to +2.52 ms) | 16.0 ms against 15.4 ms: +515 µs (+422 µs to +600 µs) | 7.18 MiB against 7.07 MiB: +116 KiB (+112 KiB to +116 KiB) |
| hypotenuse | 160, 160 in 27 s | means 9.86 ms against 8.89 ms: +977 µs (-1.18 ms to +3.14 ms) | 16.9 ms against 16.3 ms: +589 µs (+519 µs to +659 µs) | 7.18 MiB against 7.07 MiB: +120 KiB (+120 KiB to +120 KiB) |
| upcase | 160, 160 in 28 s | means 11.6 ms against 11.6 ms: +0.00 ns (-2.15 ms to +2.15 ms) | 18.4 ms against 17.7 ms: +749 µs (+668 µs to +835 µs) | 8.04 MiB against 7.92 MiB: +120 KiB (+120 KiB to +120 KiB) |
| upcase-stdin | 160, 160 in 45 s | means 33.6 ms against 31.9 ms: +1.66 ms (+18.6 µs to +3.30 ms) | 39.3 ms against 38.9 ms: +311 µs (+131 µs to +492 µs) | 7.18 MiB against 7.06 MiB: +112 KiB (+112 KiB to +116 KiB) |
| shout | 160, 160 in 26 s | means 11.5 ms against 10.1 ms: +1.46 ms (-670 µs to +3.60 ms) | 16.5 ms against 16.0 ms: +444 µs (+343 µs to +547 µs) | 7.94 MiB against 7.83 MiB: +112 KiB (+112 KiB to +112 KiB) |
| wc | 122, 122 in 60 s | means 37.0 ms against 37.8 ms: -768 µs (-3.55 ms to +2.02 ms) | 44.0 ms against 43.5 ms: +535 µs (+268 µs to +793 µs) | 7.20 MiB against 7.09 MiB: +120 KiB (+120 KiB to +120 KiB) |
| histogram | 131, 131 in 60 s | means 55.7 ms against 56.7 ms: -954 µs (-3.31 ms to +1.40 ms) | 62.7 ms against 62.1 ms: +560 µs (+346 µs to +780 µs) | 7.20 MiB against 7.09 MiB: +116 KiB (+116 KiB to +120 KiB) |
| crc32c | 145, 145 in 60 s | means 54.3 ms against 54.8 ms: -539 µs (-3.37 ms to +2.29 ms) | 60.0 ms against 59.2 ms: +642 µs (+453 µs to +834 µs) | 7.20 MiB against 7.09 MiB: +116 KiB (+116 KiB to +120 KiB) |
| sort | 21, 21 in 60 s | means 266 ms against 268 ms: -2.23 ms (-9.33 ms to +4.87 ms) | 274 ms against 275 ms: +615 µs (-3.90 ms to +4.96 ms) | 76.5 MiB against 76.4 MiB: +144 KiB (+136 KiB to +152 KiB) |
| tally | 40, 40 in 61 s | means 164 ms against 165 ms: -391 µs (-4.77 ms to +3.99 ms) | 174 ms against 173 ms: +762 µs (-225 µs to +1.82 ms) | 7.90 MiB against 7.75 MiB: +156 KiB (+152 KiB to +156 KiB) |
| server | 20, 20 in 180 s | means 1.56 s against 1.56 s: -3.12 ms (-17.7 ms to +11.4 ms) | 1.57 s against 1.58 s: -7.81 ms (-17.4 ms to +3.19 ms) | 7.52 MiB against 7.44 MiB: +96.0 KiB (+84.0 KiB to +104 KiB) |
| files | 60, 60 in 61 s | means 167 ms against 166 ms: +781 µs (-3.34 ms to +4.91 ms) | 176 ms against 176 ms: -152 µs (-1.47 ms to +1.39 ms) | 7.52 MiB against 7.43 MiB: +92.0 KiB (+92.0 KiB to +96.0 KiB) |
| readings | 92, 92 in 61 s | means 107 ms against 106 ms: +1.02 ms (-1.35 ms to +3.39 ms) | 114 ms against 113 ms: +419 µs (+93.1 µs to +746 µs) | 7.21 MiB against 7.09 MiB: +120 KiB (+120 KiB to +124 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 0.00 ns | 15.4 ms, 1.03× | 7.02 MiB, 1.02× |
| empty | cpp-wasm | 160 | 0.00 ns | 15.3 ms, 1.03× | 7.02 MiB, 1.02× |
| empty | rust-wasm | 160 | 0.00 ns | 15.6 ms, 1.01× | 7.04 MiB, 1.01× |
| add | c-wasm | 160 | 15.6 ms, 1.00× | 15.6 ms, 1.02× | 7.08 MiB, 1.01× |
| add | cpp-wasm | 160 | 0.00 ns | 15.6 ms, 1.02× | 7.08 MiB, 1.01× |
| add | rust-wasm | 160 | 15.6 ms, 1.00× | 15.7 ms, 1.02× | 7.07 MiB, 1.02× |
| hypotenuse | c-wasm | 160 | 15.6 ms, 1.00× | 16.5 ms, 1.03× | 7.08 MiB, 1.01× |
| hypotenuse | cpp-wasm | 160 | 15.6 ms, 1.00× | 16.5 ms, 1.03× | 7.07 MiB, 1.02× |
| hypotenuse | rust-wasm | 160 | 15.6 ms, 1.00× | 16.7 ms, 1.02× | 7.07 MiB, 1.02× |
| upcase | c-wasm | 160 | 15.6 ms, 1.00× | 17.8 ms, 1.03× | 7.93 MiB, 1.01× |
| upcase | cpp-wasm | 160 | 15.6 ms, 1.00× | 17.8 ms, 1.03× | 7.93 MiB, 1.01× |
| upcase | rust-wasm | 160 | 15.6 ms, 1.00× | 18.1 ms, 1.02× | 8.03 MiB, 1.00× |
| upcase-stdin | c-wasm | 160 | 31.2 ms, 1.00× | 38.8 ms, 1.01× | 7.07 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 31.2 ms, 1.00× | 39.0 ms, 1.01× | 7.06 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 31.2 ms, 1.00× | 39.5 ms, 0.99× | 7.09 MiB, 1.01× |
| shout | c-wasm | 160 | 15.6 ms, 1.00× | 16.0 ms, 1.03× | 7.78 MiB, 1.02× |
| shout | cpp-wasm | 160 | 7.81 ms, 2.00× | 16.0 ms, 1.03× | 7.78 MiB, 1.02× |
| shout | rust-wasm | 160 | 15.6 ms, 1.00× | 16.2 ms, 1.02× | 7.90 MiB, 1.00× |
| wc | c-wasm | 122 | 46.9 ms, 0.67× | 52.0 ms, 0.85× | 7.11 MiB, 1.01× |
| wc | cpp-wasm | 122 | 203 ms, 0.15× | 212 ms, 0.21× | 7.11 MiB, 1.01× |
| wc | rust-wasm | 122 | 46.9 ms, 0.67× | 52.5 ms, 0.84× | 7.09 MiB, 1.02× |
| histogram | c-wasm | 131 | 93.8 ms, 0.67× | 94.2 ms, 0.67× | 7.12 MiB, 1.01× |
| histogram | cpp-wasm | 131 | 93.8 ms, 0.67× | 94.2 ms, 0.67× | 7.12 MiB, 1.01× |
| histogram | rust-wasm | 131 | 62.5 ms, 1.00× | 64.4 ms, 0.97× | 7.10 MiB, 1.01× |
| crc32c | c-wasm | 145 | 62.5 ms, 0.75× | 70.0 ms, 0.86× | 7.10 MiB, 1.01× |
| crc32c | cpp-wasm | 145 | 62.5 ms, 0.75× | 70.0 ms, 0.86× | 7.10 MiB, 1.01× |
| crc32c | rust-wasm | 145 | 62.5 ms, 0.75× | 73.4 ms, 0.82× | 7.10 MiB, 1.01× |
| sort | c-wasm | 21 | 1.34 s, 0.20× | 1.36 s, 0.20× | 76.4 MiB, 1.00× |
| sort | cpp-wasm | 21 | 453 ms, 0.59× | 461 ms, 0.60× | 84.4 MiB, 0.91× |
| sort | rust-wasm | 21 | 406 ms, 0.65× | 411 ms, 0.67× | 84.4 MiB, 0.91× |
| tally | c-wasm | 40 | 312 ms, 0.55× | 325 ms, 0.54× | 7.60 MiB, 1.04× |
| tally | cpp-wasm | 40 | 312 ms, 0.55× | 325 ms, 0.54× | 7.59 MiB, 1.04× |
| tally | rust-wasm | 40 | 422 ms, 0.41× | 437 ms, 0.40× | 7.39 MiB, 1.07× |
| server | c-wasm | 20 | 1.56 s, 1.00× | 1.67 s, 0.94× | 7.38 MiB, 1.02× |
| server | cpp-wasm | 20 | 1.56 s, 1.00× | 1.71 s, 0.92× | 7.42 MiB, 1.01× |
| server | rust-wasm | 20 | 1.59 s, 0.98× | 1.77 s, 0.89× | 7.52 MiB, 1.00× |
| files | c-wasm | 60 | 188 ms, 0.92× | 189 ms, 0.93× | 7.34 MiB, 1.03× |
| files | cpp-wasm | 60 | 180 ms, 0.96× | 191 ms, 0.92× | 7.34 MiB, 1.03× |
| files | rust-wasm | 60 | 188 ms, 0.92× | 191 ms, 0.92× | 7.43 MiB, 1.01× |
| readings | c-wasm | 92 | 109 ms, 1.00× | 116 ms, 0.98× | 7.11 MiB, 1.02× |
| readings | cpp-wasm | 92 | 109 ms, 1.00× | 114 ms, 1.00× | 7.11 MiB, 1.01× |
| readings | rust-wasm | 92 | 109 ms, 1.00× | 118 ms, 0.96× | 7.09 MiB, 1.02× |
