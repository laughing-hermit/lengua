# lengua against C, C++ and Rust on linux-x86_64

Measured on linux-x86_64 (AMD EPYC 7763 64-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 45b507bf262dab89bff38a1f9c35bdd5590c8965, in https://github.com/laughing-hermit/lengua/actions/runs/43084459708. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-linux-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (5.87 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| empty | ✗ ✗ ≈ ✓ | ✗ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ≈ ≈ ≈ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- linux-x86_64 empty c-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 empty c-wasm memory: lengua is level now, and KNOWN_BEHIND line 39 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 empty cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 empty cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 41 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 empty rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 42 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 add c-wasm memory: lengua is level now, and KNOWN_BEHIND line 55 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 add cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 56 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 add rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 57 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 hypotenuse c-wasm memory: lengua is level now, and KNOWN_BEHIND line 70 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 hypotenuse cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 71 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 hypotenuse rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 72 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 upcase c-wasm memory: lengua is level now, and KNOWN_BEHIND line 85 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 upcase cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 86 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 113 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout c-wasm memory: lengua is level now, and KNOWN_BEHIND line 114 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout cpp-wasm wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- linux-x86_64 shout cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 115 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 shout rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 116 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 sort rust cpu: lengua is level now, and KNOWN_BEHIND line 165 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 sort rust wall: lengua is level now, and KNOWN_BEHIND line 166 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 sort rust memory: lengua is level now, and KNOWN_BEHIND line 167 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 tally c-wasm memory: lengua is level now, and KNOWN_BEHIND line 169 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 tally cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 170 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- linux-x86_64 tally rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 171 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 2.64× | 2.44× | 4.73× | 429.12× |
| add | 2.31× | 2.15× | 3.73× | 425.15× |
| hypotenuse | 2.10× | 1.94× | 3.21× | 425.17× |
| upcase | 1.81× | 1.86× | 3.25× | 425.20× |
| upcase-stdin | 1.62× | 1.65× | 6.13× | 425.15× |
| shout | 1.86× | 1.81× | 2.84× | 425.15× |
| wc | 1.42× | 1.41× | 4.88× | 425.20× |
| histogram | 1.18× | 1.18× | 5.07× | 425.17× |
| crc32c | 2.93× | 2.91× | 5.08× | 425.18× |
| sort | 1.05× | 1.05× | 1.01× | 425.33× |
| tally | 0.51× | 0.51× | 4.13× | 425.37× |
| server | 1.08× | 1.88× | 7.12× | 425.19× |
| files | 3.24× | 3.22× | 5.31× | 425.18× |
| readings | 3.51× | 3.50× | 5.00× | 425.15× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 20 | behind, 1.06×, p 4.6e-4 | behind, 1.05×, p 3.4e-4 | level, 1.01× | ahead, 0.89× |
| empty | cpp-wasm | 20 | behind, 1.07×, p 2.6e-6 | behind, 1.07×, p 1.7e-6 | level, 1.02× | ahead, 0.89× |
| empty | rust-wasm | 40 | ahead, 0.89×, p 1.4e-8 | ahead, 0.90×, p 2.2e-8 | level, 1.00× | ahead, 0.01× |
| empty | c | 20 | behind, 2.60×, p 3.4e-8 | behind, 2.41×, p 3.4e-8 | behind, 4.50× | behind, 429.12× |
| empty | cpp | 20 | behind, 2.64×, p 3.4e-8 | behind, 2.44×, p 3.4e-8 | behind, 4.73× | behind, 429.12× |
| empty | rust | 20 | behind, 1.87×, p 3.4e-8 | behind, 1.78×, p 3.4e-8 | behind, 3.19× | behind, 21.63× |
| add | c-wasm | 20 | ahead, 0.85×, p 4.0e-6 | ahead, 0.85×, p 4.9e-6 | level, 1.00× | ahead, 0.04× |
| add | cpp-wasm | 20 | ahead, 0.86×, p 2.9e-6 | ahead, 0.86×, p 2.6e-6 | level, 0.99× | ahead, 0.04× |
| add | rust-wasm | 20 | ahead, 0.75×, p 3.4e-8 | ahead, 0.76×, p 3.4e-8 | level, 0.98× | ahead, 0.02× |
| add | c | 20 | behind, 2.25×, p 3.4e-8 | behind, 2.10×, p 3.4e-8 | behind, 3.44× | behind, 425.15× |
| add | cpp | 20 | behind, 2.31×, p 3.4e-8 | behind, 2.15×, p 3.4e-8 | behind, 3.73× | behind, 425.15× |
| add | rust | 20 | behind, 1.73×, p 3.4e-8 | behind, 1.66×, p 3.4e-8 | behind, 3.19× | behind, 20.99× |
| hypotenuse | c-wasm | 40 | ahead, 0.89×, p 5.3e-4 | ahead, 0.89×, p 3.1e-4 | level, 1.00× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 80 | ahead, 0.89×, p 2.0e-8 | ahead, 0.89×, p 6.5e-9 | level, 0.99× | ahead, 0.05× |
| hypotenuse | rust-wasm | 20 | ahead, 0.77×, p 1.0e-7 | ahead, 0.77×, p 9.6e-8 | level, 0.98× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.10×, p 3.4e-8 | behind, 1.94×, p 3.4e-8 | behind, 2.90× | behind, 425.17× |
| hypotenuse | cpp | 20 | behind, 2.02×, p 3.4e-8 | behind, 1.91×, p 3.4e-8 | behind, 2.94× | behind, 425.17× |
| hypotenuse | rust | 20 | behind, 1.82×, p 3.4e-8 | behind, 1.73×, p 3.4e-8 | behind, 3.21× | behind, 20.81× |
| upcase | c-wasm | 2000 | level, 0.99×, p 0.051 | level, 0.99×, p 0.003 | level, 0.98× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 0.99×, p 0.006 | level, 0.99×, p 3.2e-4 | level, 0.98× | ahead, 0.06× |
| upcase | rust-wasm | 160 | ahead, 0.92×, p 3.4e-12 | ahead, 0.93×, p 1.0e-6 | level, 0.99× | ahead, 0.03× |
| upcase | c | 20 | behind, 1.81×, p 3.4e-8 | behind, 1.86×, p 3.4e-8 | behind, 3.25× | behind, 425.20× |
| upcase | cpp | 20 | behind, 1.80×, p 3.4e-8 | behind, 1.83×, p 3.4e-8 | behind, 3.25× | behind, 425.20× |
| upcase | rust | 20 | behind, 1.37×, p 1.1e-7 | behind, 1.42×, p 1.1e-7 | behind, 2.83× | behind, 21.03× |
| upcase-stdin | c-wasm | 20 | ahead, 0.33×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 0.99× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 20 | ahead, 0.33×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 0.99× | behind, 1.32× |
| upcase-stdin | rust-wasm | 160 | ahead, 0.94×, p 5.0e-11 | ahead, 0.95×, p 3.7e-10 | level, 0.99× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.62×, p 3.4e-8 | behind, 1.65×, p 3.4e-8 | behind, 5.86× | behind, 425.15× |
| upcase-stdin | cpp | 20 | behind, 1.62×, p 3.4e-8 | behind, 1.65×, p 3.4e-8 | behind, 6.13× | behind, 425.15× |
| upcase-stdin | rust | 20 | behind, 1.56×, p 3.4e-8 | behind, 1.59×, p 3.4e-8 | behind, 4.31× | behind, 21.41× |
| shout | c-wasm | 160 | level, 1.05×, p 3.4e-7 | level, 1.04×, p 3.4e-4 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 160 | level, 1.04×, p 1.2e-7 | behind, 1.05×, p 5.0e-5 | level, 0.99× | ahead, 0.40× |
| shout | rust-wasm | 320 | ahead, 0.94×, p 2.8e-19 | ahead, 0.95×, p 0.001 | level, 0.95× | ahead, 0.03× |
| shout | c | 20 | behind, 1.85×, p 3.4e-8 | behind, 1.81×, p 2.3e-7 | behind, 2.84× | behind, 425.15× |
| shout | cpp | 20 | behind, 1.86×, p 3.4e-8 | behind, 1.81×, p 3.4e-8 | behind, 2.84× | behind, 425.15× |
| shout | rust | 20 | behind, 1.28×, p 4.5e-7 | behind, 1.27×, p 2.3e-7 | behind, 2.49× | behind, 21.15× |
| wc | c-wasm | 20 | ahead, 0.86×, p 2.9e-6 | ahead, 0.86×, p 3.3e-6 | level, 1.00× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.10×, p 3.4e-8 | ahead, 0.10×, p 3.4e-8 | level, 1.00× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.84×, p 5.3e-8 | ahead, 0.84×, p 5.3e-8 | level, 0.99× | ahead, 0.04× |
| wc | c | 20 | behind, 1.42×, p 3.4e-8 | behind, 1.41×, p 3.4e-8 | behind, 4.88× | behind, 425.20× |
| wc | cpp | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.14×, p 3.4e-8 | behind, 4.88× | behind, 425.20× |
| wc | rust | 20 | behind, 1.22×, p 3.4e-8 | behind, 1.21×, p 3.4e-8 | behind, 4.41× | behind, 20.62× |
| histogram | c-wasm | 20 | ahead, 0.71×, p 3.4e-8 | ahead, 0.71×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.72×, p 3.4e-8 | ahead, 0.72×, p 3.4e-8 | level, 1.00× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 0.78×, p 3.4e-8 | ahead, 0.77×, p 3.4e-8 | level, 0.99× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.15×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 5.07× | behind, 425.17× |
| histogram | cpp | 20 | behind, 1.15×, p 3.4e-8 | behind, 1.15×, p 3.4e-8 | behind, 5.07× | behind, 425.17× |
| histogram | rust | 20 | behind, 1.18×, p 3.4e-8 | behind, 1.18×, p 3.4e-8 | behind, 4.44× | behind, 20.65× |
| crc32c | c-wasm | 20 | ahead, 0.80×, p 3.4e-8 | ahead, 0.80×, p 3.4e-8 | level, 1.00× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.80×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.79×, p 3.4e-8 | ahead, 0.79×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.92×, p 3.4e-8 | behind, 2.91×, p 3.4e-8 | behind, 5.08× | behind, 425.18× |
| crc32c | cpp | 20 | behind, 2.93×, p 3.4e-8 | behind, 2.91×, p 3.4e-8 | behind, 5.08× | behind, 425.18× |
| crc32c | rust | 20 | behind, 2.89×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 4.43× | behind, 21.19× |
| sort | c-wasm | 20 | ahead, 0.21×, p 3.4e-8 | ahead, 0.21×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.64×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | ahead, 0.89× | behind, 425.33× |
| sort | cpp | 20 | ahead, 0.73×, p 3.4e-8 | ahead, 0.73×, p 3.4e-8 | ahead, 0.93× | behind, 329.80× |
| sort | rust | 20 | level, 1.05×, p 4.9e-6 | level, 1.05×, p 4.9e-6 | level, 1.01× | behind, 20.66× |
| tally | c-wasm | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | level, 1.03× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | level, 1.01× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.32×, p 3.4e-8 | ahead, 0.32×, p 3.4e-8 | level, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.48×, p 3.4e-8 | ahead, 0.48×, p 3.4e-8 | behind, 4.04× | behind, 425.37× |
| tally | cpp | 20 | ahead, 0.51×, p 3.4e-8 | ahead, 0.51×, p 3.4e-8 | behind, 1.98× | behind, 331.54× |
| tally | rust | 20 | ahead, 0.46×, p 3.4e-8 | ahead, 0.46×, p 3.4e-8 | behind, 4.13× | behind, 20.10× |
| server | c-wasm | 160 | level, 1.00×, p 5.5e-4 | ahead, 0.92×, p 1.1e-44 | level, 1.01× | ahead, 0.12× |
| server | cpp-wasm | 40 | level, 0.99×, p 0.001 | ahead, 0.82×, p 2.1e-13 | level, 0.99× | ahead, 0.06× |
| server | rust-wasm | 80 | level, 1.00×, p 0.001 | ahead, 0.94×, p 1.0e-22 | level, 0.98× | ahead, 0.03× |
| server | c | 20 | behind, 1.08×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 7.12× | behind, 425.19× |
| server | cpp | 20 | behind, 1.07×, p 3.4e-8 | behind, 1.07×, p 3.4e-8 | behind, 2.55× | behind, 422.39× |
| server | rust | 20 | level, 0.97×, p 3.4e-8 | behind, 1.88×, p 3.4e-8 | behind, 3.25× | behind, 19.25× |
| files | c-wasm | 20 | ahead, 0.92×, p 8.3e-8 | ahead, 0.92×, p 8.3e-8 | level, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.91×, p 7.2e-8 | ahead, 0.91×, p 7.2e-8 | level, 1.00× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.90×, p 3.4e-8 | ahead, 0.90×, p 3.4e-8 | level, 0.98× | ahead, 0.03× |
| files | c | 20 | behind, 2.57×, p 3.4e-8 | behind, 2.57×, p 3.4e-8 | behind, 5.31× | behind, 425.18× |
| files | cpp | 20 | behind, 2.57×, p 3.4e-8 | behind, 2.57×, p 3.4e-8 | behind, 5.12× | behind, 425.18× |
| files | rust | 20 | behind, 3.24×, p 3.4e-8 | behind, 3.22×, p 3.4e-8 | behind, 4.34× | behind, 20.73× |
| readings | c-wasm | 20 | ahead, 0.92×, p 3.4e-8 | ahead, 0.92×, p 3.4e-8 | level, 0.99× | ahead, 0.04× |
| readings | cpp-wasm | 20 | ahead, 0.94×, p 3.4e-8 | ahead, 0.94×, p 3.4e-8 | level, 0.98× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.93×, p 3.4e-8 | ahead, 0.93×, p 3.4e-8 | level, 0.97× | ahead, 0.02× |
| readings | c | 20 | behind, 3.51×, p 3.4e-8 | behind, 3.50×, p 3.4e-8 | behind, 5.00× | behind, 425.15× |
| readings | cpp | 20 | behind, 3.21×, p 3.4e-8 | behind, 3.21×, p 3.4e-8 | behind, 2.13× | behind, 425.15× |
| readings | rust | 20 | behind, 3.46×, p 3.4e-8 | behind, 3.45×, p 3.4e-8 | behind, 4.60× | behind, 20.64× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 1.33 ms | 1.27 ms | 1.25 ms | 1.49 ms | 516 µs | 508 µs | 719 µs |
| wall time | 1.42 ms | 1.36 ms | 1.34 ms | 1.58 ms | 596 µs | 588 µs | 806 µs |
| peak memory | 5.50 MiB | 5.41 MiB | 5.41 MiB | 5.51 MiB | 1.22 MiB | 1.16 MiB | 1.72 MiB |
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
| CPU time | 1.24 ms | 1.45 ms | 1.44 ms | 1.65 ms | 548 µs | 534 µs | 713 µs |
| wall time | 1.32 ms | 1.55 ms | 1.55 ms | 1.74 ms | 631 µs | 615 µs | 797 µs |
| peak memory | 5.50 MiB | 5.48 MiB | 5.56 MiB | 5.63 MiB | 1.60 MiB | 1.48 MiB | 1.72 MiB |
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
| counted runs | 80 | 40 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 1.26 ms | 1.42 ms | 1.41 ms | 1.69 ms | 620 µs | 644 µs | 715 µs |
| wall time | 1.35 ms | 1.52 ms | 1.52 ms | 1.80 ms | 716 µs | 726 µs | 803 µs |
| peak memory | 5.50 MiB | 5.54 MiB | 5.56 MiB | 5.61 MiB | 1.89 MiB | 1.87 MiB | 1.71 MiB |
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
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 2.83 ms | 2.86 ms | 2.86 ms | 3.09 ms | 1.57 ms | 1.57 ms | 2.08 ms |
| wall time | 3.08 ms | 3.10 ms | 3.12 ms | 3.34 ms | 1.71 ms | 1.74 ms | 2.25 ms |
| peak memory | 7.66 MiB | 7.78 MiB | 7.78 MiB | 7.76 MiB | 2.35 MiB | 2.35 MiB | 2.70 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 14.1 KiB | 14.1 KiB | 286 KiB |

Its rounds took 43 seconds.

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
| counted runs | 160 | 20 | 20 | 160 | 20 | 20 | 20 |
| CPU time | 10.3 ms | 33.3 ms | 33.3 ms | 11.0 ms | 6.85 ms | 6.86 ms | 7.11 ms |
| wall time | 10.5 ms | 33.8 ms | 33.4 ms | 11.1 ms | 6.93 ms | 6.95 ms | 7.21 ms |
| peak memory | 7.93 MiB | 8.00 MiB | 8.01 MiB | 8.01 MiB | 1.35 MiB | 1.29 MiB | 1.83 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 14.1 KiB | 14.1 KiB | 281 KiB |

Its rounds took 8 seconds.

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
| counted runs | 320 | 160 | 160 | 320 | 20 | 20 | 20 |
| CPU time | 2.19 ms | 2.10 ms | 2.11 ms | 2.32 ms | 1.18 ms | 1.17 ms | 1.71 ms |
| wall time | 2.39 ms | 2.29 ms | 2.26 ms | 2.52 ms | 1.26 ms | 1.27 ms | 1.81 ms |
| peak memory | 6.38 MiB | 6.37 MiB | 6.40 MiB | 6.69 MiB | 2.22 MiB | 2.22 MiB | 2.53 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 14.1 KiB | 14.1 KiB | 284 KiB |

Its rounds took 6 seconds.

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
| CPU time | 20.3 ms | 23.7 ms | 198 ms | 24.3 ms | 14.3 ms | 148 ms | 16.7 ms |
| wall time | 20.4 ms | 23.8 ms | 198 ms | 24.4 ms | 14.5 ms | 148 ms | 16.8 ms |
| peak memory | 8.12 MiB | 8.09 MiB | 8.08 MiB | 8.23 MiB | 1.66 MiB | 1.66 MiB | 1.84 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 14.1 KiB | 14.1 KiB | 291 KiB |

Its rounds took 11 seconds.

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
| CPU time | 32.9 ms | 46.4 ms | 45.9 ms | 42.2 ms | 28.6 ms | 28.7 ms | 27.9 ms |
| wall time | 33.0 ms | 46.6 ms | 46.1 ms | 42.7 ms | 28.8 ms | 28.8 ms | 28.0 ms |
| peak memory | 8.12 MiB | 8.10 MiB | 8.12 MiB | 8.25 MiB | 1.60 MiB | 1.60 MiB | 1.83 MiB |
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
| CPU time | 38.0 ms | 47.7 ms | 47.8 ms | 48.1 ms | 13.0 ms | 13.0 ms | 13.2 ms |
| wall time | 38.1 ms | 47.8 ms | 48.0 ms | 48.3 ms | 13.1 ms | 13.1 ms | 13.3 ms |
| peak memory | 8.13 MiB | 8.12 MiB | 8.09 MiB | 8.25 MiB | 1.60 MiB | 1.60 MiB | 1.84 MiB |
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
| CPU time | 270 ms | 1.30 s | 457 ms | 423 ms | 428 ms | 371 ms | 258 ms |
| wall time | 270 ms | 1.30 s | 457 ms | 423 ms | 428 ms | 371 ms | 258 ms |
| peak memory | 77.6 MiB | 77.6 MiB | 85.7 MiB | 85.6 MiB | 87.1 MiB | 83.2 MiB | 76.7 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 14.1 KiB | 18.2 KiB | 291 KiB |

Its rounds took 82 seconds.

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
| CPU time | 135 ms | 340 ms | 337 ms | 424 ms | 284 ms | 268 ms | 295 ms |
| wall time | 136 ms | 340 ms | 338 ms | 425 ms | 284 ms | 268 ms | 295 ms |
| peak memory | 8.99 MiB | 8.74 MiB | 8.87 MiB | 8.60 MiB | 2.23 MiB | 4.54 MiB | 2.18 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 14.1 KiB | 18.1 KiB | 299 KiB |

Its rounds took 49 seconds.

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
| counted runs | 160 | 160 | 40 | 80 | 20 | 20 | 20 |
| CPU time | 928 ms | 931 ms | 934 ms | 932 ms | 865 ms | 867 ms | 964 ms |
| wall time | 930 ms | 1.01 s | 1.13 s | 991 ms | 871 ms | 871 ms | 495 ms |
| peak memory | 8.07 MiB | 8.01 MiB | 8.13 MiB | 8.25 MiB | 1.13 MiB | 3.16 MiB | 2.48 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 14.1 KiB | 14.2 KiB | 312 KiB |

Its rounds took 545 seconds.

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
| CPU time | 62.8 ms | 68.6 ms | 68.8 ms | 69.7 ms | 24.4 ms | 24.4 ms | 19.4 ms |
| wall time | 62.9 ms | 68.7 ms | 68.9 ms | 69.9 ms | 24.5 ms | 24.5 ms | 19.5 ms |
| peak memory | 8.51 MiB | 8.50 MiB | 8.48 MiB | 8.71 MiB | 1.60 MiB | 1.66 MiB | 1.96 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 14.1 KiB | 14.1 KiB | 290 KiB |

Its rounds took 8 seconds.

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
| CPU time | 101 ms | 109 ms | 107 ms | 108 ms | 28.8 ms | 31.4 ms | 29.2 ms |
| wall time | 101 ms | 109 ms | 107 ms | 108 ms | 28.9 ms | 31.5 ms | 29.3 ms |
| peak memory | 7.99 MiB | 8.10 MiB | 8.13 MiB | 8.23 MiB | 1.60 MiB | 3.76 MiB | 1.74 MiB |
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
| empty | 160, 160 in 1 s | 1.31 ms against 1.22 ms: +93.0 µs (+80.0 µs to +104 µs) | 1.40 ms against 1.31 ms: +95.5 µs (+82.4 µs to +109 µs) | 5.50 MiB against 5.41 MiB: +116 KiB (+96.0 KiB to +136 KiB) |
| add | 160, 160 in 1 s | 1.23 ms against 1.12 ms: +104 µs (+94.0 µs to +117 µs) | 1.32 ms against 1.21 ms: +107 µs (+94.0 µs to +120 µs) | 5.51 MiB against 5.39 MiB: +120 KiB (+100 KiB to +140 KiB) |
| hypotenuse | 160, 160 in 1 s | 1.24 ms against 1.12 ms: +120 µs (+112 µs to +129 µs) | 1.34 ms against 1.21 ms: +120 µs (+110 µs to +130 µs) | 5.51 MiB against 5.43 MiB: +96.0 KiB (+76.0 KiB to +120 KiB) |
| upcase | 160, 160 in 2 s | 2.80 ms against 2.78 ms: +28.0 µs (-34.0 µs to +79.0 µs) | 3.08 ms against 3.14 ms: +49.1 µs (-80.3 µs to +136 µs) | 7.70 MiB against 7.69 MiB: +4.00 KiB (-28.0 KiB to +32.0 KiB) |
| upcase-stdin | 160, 160 in 5 s | 10.6 ms against 9.99 ms: +227 µs (+137 µs to +363 µs) | 10.8 ms against 10.1 ms: +234 µs (+141 µs to +372 µs) | 7.94 MiB against 7.93 MiB: +12.0 KiB (-4.00 KiB to +28.0 KiB) |
| shout | 160, 160 in 2 s | 2.20 ms against 2.07 ms: +124 µs (+89.0 µs to +157 µs) | 2.36 ms against 2.21 ms: +131 µs (+70.7 µs to +192 µs) | 6.39 MiB against 6.31 MiB: +92.0 KiB (+52.0 KiB to +128 KiB) |
| wc | 160, 160 in 8 s | 20.1 ms against 19.8 ms: +295 µs (+189 µs to +408 µs) | 20.3 ms against 19.9 ms: +320 µs (+209 µs to +442 µs) | 8.11 MiB against 8.12 MiB: -16.0 KiB (-32.0 KiB to -4.00 KiB) |
| histogram | 160, 160 in 12 s | 33.3 ms against 32.9 ms: +194 µs (-79.0 µs to +481 µs) | 33.5 ms against 33.1 ms: +216 µs (-57.7 µs to +508 µs) | 8.12 MiB against 8.00 MiB: +124 KiB (+112 KiB to +136 KiB) |
| crc32c | 160, 160 in 13 s | 37.7 ms against 37.7 ms: +27.0 µs (-268 µs to +305 µs) | 37.8 ms against 37.9 ms: +35.7 µs (-259 µs to +316 µs) | 8.12 MiB against 8.11 MiB: +8.00 KiB (-4.00 KiB to +20.0 KiB) |
| sort | 110, 110 in 60 s | 270 ms against 270 ms: +702 µs (-704 µs to +2.17 ms) | 271 ms against 270 ms: +706 µs (-706 µs to +2.17 ms) | 77.6 MiB against 77.6 MiB: -8.00 KiB (-32.0 KiB to +12.0 KiB) |
| tally | 160, 160 in 45 s | 135 ms against 135 ms: +159 µs (-310 µs to +596 µs) | 135 ms against 135 ms: +149 µs (-324 µs to +587 µs) | 8.99 MiB against 8.86 MiB: +136 KiB (+124 KiB to +148 KiB) |
| server | 31, 31 in 61 s | 928 ms against 924 ms: +2.07 ms (-2.57 ms to +5.75 ms) | 930 ms against 927 ms: +1.27 ms (-3.34 ms to +5.45 ms) | 8.08 MiB against 8.05 MiB: +0 B (-48.0 KiB to +60.0 KiB) |
| files | 160, 160 in 22 s | 62.0 ms against 64.5 ms: -2.47 ms (-2.80 ms to -2.13 ms) | 62.2 ms against 64.6 ms: -2.45 ms (-2.79 ms to -2.10 ms) | 8.50 MiB against 8.50 MiB: -4.00 KiB (-20.0 KiB to +4.00 KiB) |
| readings | 160, 160 in 34 s | 101 ms against 101 ms: +171 µs (-43.0 µs to +375 µs) | 101 ms against 101 ms: +175 µs (-40.5 µs to +381 µs) | 8.00 MiB against 7.99 MiB: +8.00 KiB (-4.00 KiB to +20.0 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
