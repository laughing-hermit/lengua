# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 43813aef065cae8d91368d4891f6823f48ad9d82, in https://github.com/laughing-hermit/lengua/actions/runs/43131768685. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records lengua's idea and the module, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (6.20 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| add | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ✓ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| wc | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✓ ✗ ✗ | ≈ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ≈ ≈ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 hypotenuse c cpu: lengua is level now, and KNOWN_BEHIND line 533 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse cpp cpu: lengua is level now, and KNOWN_BEHIND line 537 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse rust cpu: lengua is level now, and KNOWN_BEHIND line 541 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 upcase cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 upcase-stdin c cpu: lengua is level now, and KNOWN_BEHIND line 556 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin rust cpu: lengua is level now, and KNOWN_BEHIND line 563 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout c cpu: lengua is level now, and KNOWN_BEHIND line 567 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout cpp cpu: lengua is level now, and KNOWN_BEHIND line 571 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout rust cpu: lengua is level now, and KNOWN_BEHIND line 575 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 histogram c cpu: lengua is level now, and KNOWN_BEHIND line 587 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 histogram cpp cpu: lengua is level now, and KNOWN_BEHIND line 591 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 tally rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 614 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server c wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.85× | 1.74× | 60.79× |
| add | – | 1.86× | 1.73× | 53.16× |
| hypotenuse | – | 1.81× | 1.73× | 52.51× |
| upcase | – | 1.67× | 1.55× | 51.04× |
| upcase-stdin | 1.00× | 1.51× | 1.71× | 58.29× |
| shout | – | 1.71× | 1.53× | 55.98× |
| wc | 1.00× | 1.24× | 1.71× | 53.17× |
| histogram | 1.00× | 1.17× | 1.69× | 53.39× |
| crc32c | 3.00× | 2.09× | 1.70× | 56.73× |
| sort | 1.00× | 1.02× | 0.96× | 52.97× |
| tally | 0.54× | 0.53× | 1.66× | 49.47× |
| server | 1.43× | 2.43× | 1.59× | 54.08× |
| files | 1.80× | 1.69× | 1.74× | 51.03× |
| readings | 3.00× | 3.54× | 1.73× | 51.24× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, p 0.196 | level, 1.01×, p 3.1e-4 | level, 1.00× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, p 0.235 | level, 1.00×, p 0.011 | level, 1.00× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, p 0.070 | level, 0.97×, p 8.7e-103 | level, 1.00× | ahead, 0.01× |
| empty | c | 160 | level, p 5.8e-7 | behind, 1.85×, p 2.9e-54 | behind, 1.74× | behind, 56.72× |
| empty | cpp | 160 | level, p 5.6e-5 | behind, 1.84×, p 5.6e-53 | behind, 1.74× | behind, 56.72× |
| empty | rust | 160 | level, p 0.001 | behind, 1.65×, p 2.9e-54 | behind, 1.57× | behind, 60.79× |
| add | c-wasm | 2000 | level, p 0.274 | level, 0.98×, p 6.9e-44 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, p 0.275 | level, 0.98×, p 1.3e-47 | level, 1.00× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, p 0.129 | level, 0.96×, p 5.6e-144 | level, 0.99× | ahead, 0.02× |
| add | c | 80 | level, p 3.1e-4 | behind, 1.83×, p 4.4e-27 | behind, 1.73× | behind, 43.07× |
| add | cpp | 80 | level, p 2.4e-4 | behind, 1.86×, p 4.7e-28 | behind, 1.73× | behind, 43.22× |
| add | rust | 80 | level, p 2.4e-4 | behind, 1.64×, p 4.7e-28 | behind, 1.57× | behind, 53.16× |
| hypotenuse | c-wasm | 2000 | level, p 0.295 | level, 0.98×, p 6.0e-30 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, p 0.488 | level, 0.98×, p 1.2e-29 | level, 1.00× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, p 0.228 | level, 0.96×, p 7.2e-141 | level, 0.98× | ahead, 0.03× |
| hypotenuse | c | 160 | level, p 6.5e-4 | behind, 1.81×, p 2.9e-54 | behind, 1.73× | behind, 42.78× |
| hypotenuse | cpp | 160 | level, p 0.001 | behind, 1.80×, p 2.9e-54 | behind, 1.73× | behind, 42.64× |
| hypotenuse | rust | 80 | level, p 9.9e-5 | behind, 1.62×, p 4.7e-28 | behind, 1.56× | behind, 52.51× |
| upcase | c-wasm | 2000 | level, p 0.098 | level, 0.99×, p 1.8e-14 | level, 1.01× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, p 0.086 | level, 0.99×, p 2.0e-16 | level, 1.01× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, p 0.272 | level, 0.97×, p 1.7e-92 | level, 0.99× | ahead, 0.03× |
| upcase | c | 80 | behind, p 9.6e-5 | behind, 1.67×, p 4.7e-28 | behind, 1.52× | behind, 40.47× |
| upcase | cpp | 80 | behind, p 1.4e-4 | behind, 1.67×, p 4.1e-27 | behind, 1.52× | behind, 40.47× |
| upcase | rust | 640 | level, p 7.1e-9 | behind, 1.62×, p 4.7e-209 | behind, 1.55× | behind, 51.04× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.479 | level, 0.98×, p 1.5e-115 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 0.50×, p 0.006 | level, 0.98×, p 5.7e-170 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 320 | ahead, 0.50×, p 2.0e-4 | level, 0.98×, p 2.5e-24 | level, 1.00× | ahead, 0.03× |
| upcase-stdin | c | 40 | level, 1.00×, p 4.1e-5 | behind, 1.51×, p 7.2e-15 | behind, 1.71× | behind, 54.53× |
| upcase-stdin | cpp | 40 | level, 1.00×, p 7.6e-6 | behind, 1.50×, p 7.2e-15 | behind, 1.71× | behind, 54.53× |
| upcase-stdin | rust | 40 | level, 1.00×, p 4.1e-5 | behind, 1.37×, p 7.2e-15 | behind, 1.54× | behind, 58.29× |
| shout | c-wasm | 2000 | level, p 0.234 | level, 1.00×, p 0.108 | level, 1.01× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, p 0.325 | level, 1.00×, p 0.199 | level, 1.01× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, p 0.005 | level, 0.97×, p 6.5e-102 | level, 0.97× | ahead, 0.03× |
| shout | c | 160 | level, p 4.1e-5 | behind, 1.71×, p 2.9e-54 | behind, 1.52× | behind, 53.39× |
| shout | cpp | 160 | level, p 4.2e-6 | behind, 1.71×, p 2.9e-54 | behind, 1.52× | behind, 53.39× |
| shout | rust | 160 | level, p 8.7e-5 | behind, 1.59×, p 2.9e-54 | behind, 1.53× | behind, 55.98× |
| wc | c-wasm | 80 | level, 1.00×, p 0.001 | level, 0.95×, p 1.6e-16 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 2.3e-9 | ahead, 0.16×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 160 | level, 1.00×, p 5.8e-6 | ahead, 0.93×, p 2.6e-42 | level, 0.98× | ahead, 0.04× |
| wc | c | 40 | level, 1.00×, p 6.3e-5 | behind, 1.24×, p 2.2e-13 | behind, 1.70× | behind, 42.50× |
| wc | cpp | 80 | level, 1.00×, p 5.3e-4 | ahead, 0.90×, p 2.1e-24 | behind, 1.71× | behind, 42.36× |
| wc | rust | 40 | level, 1.00×, p 6.3e-5 | behind, 1.20×, p 4.5e-13 | behind, 1.54× | behind, 53.17× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 9.6e-5 | ahead, 0.82×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 4.4e-4 | ahead, 0.81×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.67×, p 9.6e-5 | ahead, 0.86×, p 3.9e-8 | level, 0.97× | ahead, 0.03× |
| histogram | c | 40 | level, 1.00×, p 2.7e-4 | behind, 1.17×, p 1.2e-13 | behind, 1.69× | behind, 42.93× |
| histogram | cpp | 20 | level, 1.00×, p 0.001 | behind, 1.17×, p 3.4e-8 | behind, 1.69× | behind, 42.93× |
| histogram | rust | 160 | behind, 1.25×, p 8.6e-4 | behind, 1.10×, p 6.4e-50 | behind, 1.54× | behind, 53.39× |
| crc32c | c-wasm | 20 | ahead, 0.75×, p 3.1e-5 | ahead, 0.80×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | ahead, 0.75×, p 3.8e-7 | ahead, 0.80×, p 7.2e-15 | level, 1.00× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.75×, p 1.0e-4 | ahead, 0.80×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.00×, p 2.6e-8 | behind, 2.09×, p 3.4e-8 | behind, 1.70× | behind, 42.93× |
| crc32c | cpp | 20 | behind, 3.00×, p 3.8e-8 | behind, 2.07×, p 3.4e-8 | behind, 1.70× | behind, 42.93× |
| crc32c | rust | 20 | behind, 3.00×, p 3.8e-8 | behind, 1.96×, p 3.4e-8 | behind, 1.53× | behind, 56.73× |
| sort | c-wasm | 20 | ahead, 0.20×, p 1.2e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.56×, p 1.1e-8 | ahead, 0.55×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.61×, p 9.2e-9 | ahead, 0.59×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.41×, p 9.8e-9 | ahead, 0.41×, p 3.4e-8 | level, 0.96× | behind, 52.97× |
| sort | cpp | 20 | ahead, 0.47×, p 6.8e-9 | ahead, 0.46×, p 3.4e-8 | ahead, 0.83× | behind, 46.73× |
| sort | rust | 80 | level, 1.00×, p 2.4e-4 | level, 1.02×, p 1.4e-9 | ahead, 0.89× | behind, 52.75× |
| tally | c-wasm | 20 | ahead, 0.39×, p 1.5e-8 | ahead, 0.39×, p 3.4e-8 | level, 1.02× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.39×, p 1.3e-8 | ahead, 0.39×, p 3.4e-8 | level, 1.02× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.33×, p 9.8e-9 | ahead, 0.32×, p 3.4e-8 | level, 1.03× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.50×, p 1.6e-8 | ahead, 0.51×, p 3.4e-8 | behind, 1.61× | behind, 41.82× |
| tally | cpp | 20 | ahead, 0.54×, p 9.7e-9 | ahead, 0.53×, p 3.4e-8 | behind, 1.63× | behind, 40.36× |
| tally | rust | 20 | ahead, 0.47×, p 9.8e-9 | ahead, 0.46×, p 3.4e-8 | behind, 1.66× | behind, 49.47× |
| server | c-wasm | 97 | level, 1.00×, p 0.209 | ahead, 0.80×, p 2.1e-33 | level, 1.01× | ahead, 0.12× |
| server | cpp-wasm | 97 | level, 1.00×, p 0.242 | ahead, 0.79×, p 2.0e-33 | level, 1.00× | ahead, 0.06× |
| server | rust-wasm | 97 | level, 1.00×, p 0.368 | ahead, 0.81×, p 1.4e-33 | level, 0.98× | ahead, 0.03× |
| server | c | 20 | behind, 1.07×, p 6.0e-7 | behind, 1.06×, p 6.0e-7 | behind, 1.58× | behind, 54.08× |
| server | cpp | 20 | behind, 1.05×, p 2.9e-8 | behind, 1.06×, p 3.4e-8 | behind, 1.59× | behind, 52.51× |
| server | rust | 20 | behind, 1.43×, p 2.8e-8 | behind, 2.43×, p 3.4e-8 | behind, 1.21× | behind, 44.90× |
| files | c-wasm | 80 | ahead, 0.90×, p 1.7e-5 | ahead, 0.94×, p 5.4e-14 | level, 1.01× | ahead, 0.05× |
| files | cpp-wasm | 40 | ahead, 0.90×, p 5.6e-5 | ahead, 0.95×, p 4.7e-6 | level, 1.01× | ahead, 0.05× |
| files | rust-wasm | 40 | ahead, 0.90×, p 7.7e-9 | ahead, 0.93×, p 5.7e-7 | level, 0.98× | ahead, 0.03× |
| files | c | 20 | behind, 1.50×, p 5.2e-9 | behind, 1.56×, p 3.4e-8 | behind, 1.74× | behind, 40.86× |
| files | cpp | 20 | behind, 1.50×, p 1.1e-8 | behind, 1.56×, p 3.4e-8 | behind, 1.74× | behind, 40.86× |
| files | rust | 20 | behind, 1.80×, p 1.4e-8 | behind, 1.69×, p 3.4e-8 | behind, 1.60× | behind, 51.03× |
| readings | c-wasm | 80 | level, 1.00×, p 5.4e-4 | ahead, 0.94×, p 9.1e-27 | level, 1.00× | ahead, 0.04× |
| readings | cpp-wasm | 320 | level, 1.00×, p 1.3e-6 | level, 0.96×, p 3.8e-76 | level, 1.00× | ahead, 0.04× |
| readings | rust-wasm | 160 | level, 1.00×, p 1.6e-4 | level, 0.96×, p 5.1e-42 | level, 0.98× | ahead, 0.02× |
| readings | c | 20 | behind, 3.00×, p 1.9e-8 | behind, 3.54×, p 3.4e-8 | behind, 1.73× | behind, 43.07× |
| readings | cpp | 20 | behind, 3.00×, p 1.8e-8 | behind, 3.35×, p 3.4e-8 | behind, 1.73× | behind, 43.22× |
| readings | rust | 20 | behind, 3.00×, p 1.0e-8 | behind, 2.42×, p 3.4e-8 | behind, 1.56× | behind, 51.24× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 160 | 160 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 7.04 ms | 7.00 ms | 7.02 ms | 7.29 ms | 3.82 ms | 3.83 ms | 4.27 ms |
| peak memory | 5.88 MiB | 5.86 MiB | 5.86 MiB | 5.88 MiB | 3.38 MiB | 3.38 MiB | 3.75 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 149 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 80 | 80 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 6.98 ms | 7.12 ms | 7.13 ms | 7.28 ms | 3.90 ms | 3.85 ms | 4.35 ms |
| peak memory | 5.91 MiB | 5.90 MiB | 5.90 MiB | 5.99 MiB | 3.43 MiB | 3.42 MiB | 3.78 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 147 KiB | 120 KiB |

Its rounds took 146 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 160 | 80 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 7.06 ms | 7.17 ms | 7.17 ms | 7.33 ms | 3.98 ms | 4.00 ms | 4.43 ms |
| peak memory | 5.91 MiB | 5.89 MiB | 5.89 MiB | 6.00 MiB | 3.43 MiB | 3.43 MiB | 3.79 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 149 KiB | 121 KiB |

Its rounds took 150 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 80 | 640 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 8.16 ms | 8.25 ms | 8.25 ms | 8.41 ms | 4.67 ms | 4.65 ms | 5.04 ms |
| peak memory | 6.86 MiB | 6.82 MiB | 6.82 MiB | 6.95 MiB | 4.51 MiB | 4.51 MiB | 4.42 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 157 KiB | 157 KiB | 124 KiB |

Its rounds took 165 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 40 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 31.2 ms | 31.2 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 24.7 ms | 25.2 ms | 25.4 ms | 25.3 ms | 16.4 ms | 16.5 ms | 18.0 ms |
| peak memory | 5.90 MiB | 5.89 MiB | 5.89 MiB | 5.89 MiB | 3.45 MiB | 3.45 MiB | 3.82 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 109 KiB |

Its rounds took 237 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 160 | 160 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 7.32 ms | 7.31 ms | 7.31 ms | 7.53 ms | 4.32 ms | 4.33 ms | 4.64 ms |
| peak memory | 6.68 MiB | 6.63 MiB | 6.63 MiB | 6.89 MiB | 4.39 MiB | 4.40 MiB | 4.37 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 152 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 80 | 20 | 160 | 40 | 80 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 188 ms | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 31.8 ms | 33.9 ms | 192 ms | 34.1 ms | 26.1 ms | 35.9 ms | 27.0 ms |
| peak memory | 5.93 MiB | 5.91 MiB | 5.91 MiB | 6.04 MiB | 3.48 MiB | 3.48 MiB | 3.86 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 150 KiB | 150 KiB | 120 KiB |

Its rounds took 30 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 20 | 20 | 20 | 40 | 20 | 160 |
| CPU time | 39.1 ms | 46.9 ms | 46.9 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 40.9 ms | 49.8 ms | 50.2 ms | 47.3 ms | 34.8 ms | 34.8 ms | 37.3 ms |
| peak memory | 5.91 MiB | 5.91 MiB | 5.91 MiB | 6.07 MiB | 3.49 MiB | 3.49 MiB | 3.82 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 119 KiB |

Its rounds took 24 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 62.5 ms | 62.5 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 45.4 ms | 56.5 ms | 56.6 ms | 56.5 ms | 21.7 ms | 21.9 ms | 23.2 ms |
| peak memory | 5.90 MiB | 5.91 MiB | 5.91 MiB | 5.95 MiB | 3.48 MiB | 3.48 MiB | 3.85 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 11 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 20 | 20 | 20 | 20 | 80 |
| CPU time | 219 ms | 1.08 s | 391 ms | 359 ms | 531 ms | 469 ms | 219 ms |
| wall time | 218 ms | 1.08 s | 395 ms | 365 ms | 529 ms | 471 ms | 213 ms |
| peak memory | 75.2 MiB | 75.2 MiB | 83.3 MiB | 83.5 MiB | 78.4 MiB | 90.4 MiB | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 136 KiB | 120 KiB |

Its rounds took 106 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 117 ms | 297 ms | 297 ms | 359 ms | 234 ms | 219 ms | 250 ms |
| wall time | 116 ms | 296 ms | 296 ms | 360 ms | 227 ms | 219 ms | 250 ms |
| peak memory | 6.62 MiB | 6.51 MiB | 6.47 MiB | 6.44 MiB | 4.11 MiB | 4.05 MiB | 4.00 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 158 KiB | 128 KiB |

Its rounds took 43 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 97 | 97 | 97 | 97 | 20 | 20 | 20 |
| CPU time | 1.08 s | 1.08 s | 1.08 s | 1.08 s | 1.00 s | 1.02 s | 750 ms |
| wall time | 1.09 s | 1.36 s | 1.37 s | 1.33 s | 1.02 s | 1.02 s | 444 ms |
| peak memory | 6.35 MiB | 6.30 MiB | 6.35 MiB | 6.48 MiB | 4.01 MiB | 4.01 MiB | 5.27 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 121 KiB | 142 KiB |

Its rounds took 629 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 40 | 40 | 20 | 20 | 20 |
| CPU time | 141 ms | 156 ms | 156 ms | 156 ms | 93.8 ms | 93.8 ms | 78.1 ms |
| wall time | 143 ms | 152 ms | 151 ms | 153 ms | 91.3 ms | 91.6 ms | 84.6 ms |
| peak memory | 6.29 MiB | 6.23 MiB | 6.23 MiB | 6.43 MiB | 3.61 MiB | 3.61 MiB | 3.93 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 48 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 80 | 320 | 160 | 20 | 20 | 20 |
| CPU time | 93.8 ms | 93.8 ms | 93.8 ms | 93.8 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 89.3 ms | 95.4 ms | 93.2 ms | 93.5 ms | 25.2 ms | 26.6 ms | 36.9 ms |
| peak memory | 5.91 MiB | 5.90 MiB | 5.90 MiB | 6.03 MiB | 3.42 MiB | 3.42 MiB | 3.80 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 147 KiB | 124 KiB |

Its rounds took 96 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 14 s | means 7.62 ms against 6.84 ms: +781 µs (-1.61 ms to +3.17 ms) | 7.09 ms against 6.68 ms: +415 µs (+360 µs to +472 µs) | 5.88 MiB against 5.76 MiB: +120 KiB (+120 KiB to +120 KiB) |
| add | 160, 160 in 14 s | means 7.32 ms against 5.47 ms: +1.86 ms (-215 µs to +3.93 ms) | 7.05 ms against 6.62 ms: +451 µs (+376 µs to +525 µs) | 5.90 MiB against 5.78 MiB: +128 KiB (+124 KiB to +128 KiB) |
| hypotenuse | 160, 160 in 14 s | means 5.86 ms against 7.52 ms: -1.66 ms (-3.76 ms to +438 µs) | 7.06 ms against 6.65 ms: +432 µs (+355 µs to +506 µs) | 5.91 MiB against 5.79 MiB: +132 KiB (+128 KiB to +132 KiB) |
| upcase | 160, 160 in 15 s | means 7.23 ms against 6.84 ms: +391 µs (-1.63 ms to +2.41 ms) | 8.21 ms against 7.51 ms: +682 µs (+625 µs to +739 µs) | 6.86 MiB against 6.74 MiB: +116 KiB (+112 KiB to +116 KiB) |
| upcase-stdin | 160, 160 in 29 s | means 23.2 ms against 23.7 ms: -488 µs (-2.64 ms to +1.67 ms) | 24.7 ms against 24.4 ms: +389 µs (+248 µs to +530 µs) | 5.90 MiB against 5.78 MiB: +124 KiB (+124 KiB to +128 KiB) |
| shout | 160, 160 in 14 s | means 7.81 ms against 4.59 ms: +3.22 ms (+1.20 ms to +5.25 ms) | 7.61 ms against 7.20 ms: +404 µs (+319 µs to +485 µs) | 6.68 MiB against 6.57 MiB: +116 KiB (+112 KiB to +116 KiB) |
| wc | 158, 158 in 60 s | means 31.5 ms against 31.9 ms: -396 µs (-1.99 ms to +1.20 ms) | 32.1 ms against 31.6 ms: +522 µs (+408 µs to +638 µs) | 5.93 MiB against 5.80 MiB: +136 KiB (+132 KiB to +136 KiB) |
| histogram | 160, 160 in 46 s | means 39.9 ms against 39.3 ms: +684 µs (-1.58 ms to +2.95 ms) | 41.0 ms against 40.5 ms: +501 µs (+384 µs to +626 µs) | 5.91 MiB against 5.78 MiB: +128 KiB (+128 KiB to +128 KiB) |
| crc32c | 160, 160 in 50 s | means 43.3 ms against 43.4 ms: -97.7 µs (-1.96 ms to +1.77 ms) | 45.0 ms against 44.4 ms: +492 µs (+342 µs to +641 µs) | 5.91 MiB against 5.78 MiB: +132 KiB (+132 KiB to +132 KiB) |
| sort | 25, 25 in 61 s | means 226 ms against 221 ms: +5.62 ms (-8.12 ms to +19.4 ms) | 223 ms against 220 ms: +364 µs (-3.24 ms to +4.86 ms) | 75.2 MiB against 75.1 MiB: +148 KiB (+144 KiB to +148 KiB) |
| tally | 49, 49 in 61 s | means 114 ms against 115 ms: -1.59 ms (-5.95 ms to +2.76 ms) | 115 ms against 114 ms: +596 µs (+260 µs to +893 µs) | 6.62 MiB against 6.47 MiB: +156 KiB (+152 KiB to +160 KiB) |
| server | 20, 20 in 138 s | means 1.09 s against 1.09 s: +781 µs (-17.7 ms to +19.3 ms) | 1.09 s against 1.09 s: +701 µs (-7.62 ms to +8.85 ms) | 6.36 MiB against 6.24 MiB: +124 KiB (+108 KiB to +136 KiB) |
| files | 75, 75 in 61 s | means 144 ms against 142 ms: +2.29 ms (-1.31 ms to +5.89 ms) | 143 ms against 142 ms: +821 µs (-223 µs to +1.75 ms) | 6.29 MiB against 6.18 MiB: +108 KiB (+104 KiB to +112 KiB) |
| readings | 112, 112 in 60 s | means 94.3 ms against 91.8 ms: +2.51 ms (-318 µs to +5.34 ms) | 90.6 ms against 90.2 ms: +510 µs (+84.3 µs to +982 µs) | 5.91 MiB against 5.79 MiB: +128 KiB (+124 KiB to +128 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 0.00 ns | 6.65 ms, 1.07× | 5.74 MiB, 1.02× |
| empty | cpp-wasm | 160 | 0.00 ns | 6.70 ms, 1.06× | 5.74 MiB, 1.02× |
| empty | rust-wasm | 160 | 0.00 ns | 6.92 ms, 1.03× | 5.76 MiB, 1.02× |
| add | c-wasm | 160 | 0.00 ns | 6.74 ms, 1.05× | 5.78 MiB, 1.02× |
| add | cpp-wasm | 160 | 0.00 ns | 6.76 ms, 1.04× | 5.78 MiB, 1.02× |
| add | rust-wasm | 160 | 0.00 ns | 6.85 ms, 1.03× | 5.78 MiB, 1.02× |
| hypotenuse | c-wasm | 160 | 0.00 ns | 6.73 ms, 1.05× | 5.78 MiB, 1.02× |
| hypotenuse | cpp-wasm | 160 | 0.00 ns | 6.79 ms, 1.04× | 5.78 MiB, 1.02× |
| hypotenuse | rust-wasm | 160 | 0.00 ns | 6.91 ms, 1.02× | 5.84 MiB, 1.01× |
| upcase | c-wasm | 160 | 0.00 ns | 7.67 ms, 1.07× | 6.75 MiB, 1.02× |
| upcase | cpp-wasm | 160 | 0.00 ns | 7.69 ms, 1.07× | 6.75 MiB, 1.02× |
| upcase | rust-wasm | 160 | 0.00 ns | 7.84 ms, 1.05× | 6.87 MiB, 1.00× |
| upcase-stdin | c-wasm | 160 | 15.6 ms, 1.00× | 24.9 ms, 0.99× | 5.78 MiB, 1.02× |
| upcase-stdin | cpp-wasm | 160 | 15.6 ms, 1.00× | 25.1 ms, 0.99× | 5.77 MiB, 1.02× |
| upcase-stdin | rust-wasm | 160 | 31.2 ms, 0.50× | 24.9 ms, 0.99× | 5.78 MiB, 1.02× |
| shout | c-wasm | 160 | 0.00 ns | 7.23 ms, 1.05× | 6.53 MiB, 1.02× |
| shout | cpp-wasm | 160 | 0.00 ns | 7.21 ms, 1.06× | 6.53 MiB, 1.02× |
| shout | rust-wasm | 160 | 0.00 ns | 7.35 ms, 1.04× | 6.75 MiB, 0.99× |
| wc | c-wasm | 158 | 31.2 ms, 1.00× | 33.6 ms, 0.96× | 5.83 MiB, 1.02× |
| wc | cpp-wasm | 158 | 188 ms, 0.17× | 193 ms, 0.17× | 5.83 MiB, 1.02× |
| wc | rust-wasm | 158 | 31.2 ms, 1.00× | 33.9 ms, 0.95× | 5.83 MiB, 1.02× |
| histogram | c-wasm | 160 | 46.9 ms, 0.67× | 49.8 ms, 0.82× | 5.83 MiB, 1.01× |
| histogram | cpp-wasm | 160 | 46.9 ms, 0.67× | 49.8 ms, 0.82× | 5.83 MiB, 1.01× |
| histogram | rust-wasm | 160 | 46.9 ms, 0.67× | 47.0 ms, 0.87× | 5.85 MiB, 1.01× |
| crc32c | c-wasm | 160 | 46.9 ms, 1.00× | 55.7 ms, 0.81× | 5.83 MiB, 1.01× |
| crc32c | cpp-wasm | 160 | 62.5 ms, 0.75× | 55.9 ms, 0.80× | 5.83 MiB, 1.01× |
| crc32c | rust-wasm | 160 | 62.5 ms, 0.75× | 55.8 ms, 0.81× | 5.82 MiB, 1.02× |
| sort | c-wasm | 25 | 1.14 s, 0.19× | 1.15 s, 0.19× | 75.1 MiB, 1.00× |
| sort | cpp-wasm | 25 | 391 ms, 0.56× | 400 ms, 0.56× | 83.2 MiB, 0.90× |
| sort | rust-wasm | 25 | 375 ms, 0.58× | 375 ms, 0.59× | 83.2 MiB, 0.90× |
| tally | c-wasm | 49 | 297 ms, 0.37× | 295 ms, 0.39× | 6.33 MiB, 1.05× |
| tally | cpp-wasm | 49 | 297 ms, 0.37× | 296 ms, 0.39× | 6.32 MiB, 1.05× |
| tally | rust-wasm | 49 | 359 ms, 0.30× | 359 ms, 0.32× | 6.28 MiB, 1.05× |
| server | c-wasm | 20 | 1.08 s, 1.00× | 1.39 s, 0.78× | 6.19 MiB, 1.03× |
| server | cpp-wasm | 20 | 1.08 s, 1.00× | 1.34 s, 0.81× | 6.23 MiB, 1.02× |
| server | rust-wasm | 20 | 1.09 s, 0.99× | 1.37 s, 0.80× | 6.34 MiB, 1.00× |
| files | c-wasm | 75 | 156 ms, 0.90× | 150 ms, 0.95× | 6.12 MiB, 1.03× |
| files | cpp-wasm | 75 | 156 ms, 0.90× | 151 ms, 0.94× | 6.12 MiB, 1.03× |
| files | rust-wasm | 75 | 156 ms, 0.90× | 152 ms, 0.94× | 6.32 MiB, 1.00× |
| readings | c-wasm | 112 | 93.8 ms, 1.00× | 96.6 ms, 0.94× | 5.78 MiB, 1.02× |
| readings | cpp-wasm | 112 | 93.8 ms, 1.00× | 93.9 ms, 0.97× | 5.79 MiB, 1.02× |
| readings | rust-wasm | 112 | 93.8 ms, 1.00× | 94.8 ms, 0.96× | 5.86 MiB, 1.01× |
