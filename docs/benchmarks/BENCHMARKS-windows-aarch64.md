# lengua against C, C++ and Rust on windows-aarch64

Measured on windows-aarch64 (Cobalt 100, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit cd54e79fc07fe745f5fcad008bd2a7da4a18ae9e, in https://github.com/laughing-hermit/lengua/actions/runs/42964232679. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (18.8 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
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
| empty | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ≈ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ≈ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ≈ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-aarch64 empty c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 846 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty c-wasm wall: lengua is level now, and KNOWN_BEHIND line 847 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 849 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 850 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 852 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 866 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 868 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 870 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 884 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 886 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 888 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 902 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase c-wasm wall: lengua is level now, and KNOWN_BEHIND line 903 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 919 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin c-wasm wall: lengua is level now, and KNOWN_BEHIND line 920 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 923 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 924 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 927 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 941 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout c-wasm wall: lengua is level now, and KNOWN_BEHIND line 942 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 944 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 945 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 sort c-wasm memory: lengua is level now, and KNOWN_BEHIND line 1005 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 sort c memory: lengua is level now, and KNOWN_BEHIND line 1006 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 sort cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 1008 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 sort cpp wall: lengua is ahead now, and KNOWN_BEHIND line 1009 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 sort cpp memory: lengua is ahead now, and KNOWN_BEHIND line 1010 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 tally cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 1020 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 tally cpp wall: lengua is ahead now, and KNOWN_BEHIND line 1021 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 server c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 1026 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 server rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 1029 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.91× | 2.49× | 189.99× |
| add | – | 1.85× | 2.49× | 166.25× |
| hypotenuse | – | 1.86× | 2.49× | 164.83× |
| upcase | – | 1.79× | 2.15× | 161.38× |
| upcase-stdin | 3.00× | 1.52× | 2.48× | 181.08× |
| shout | – | 1.78× | 2.14× | 173.74× |
| wc | 1.50× | 1.17× | 2.48× | 168.43× |
| histogram | 1.33× | 1.24× | 2.46× | 165.54× |
| crc32c | 4.00× | 2.24× | 2.46× | 175.32× |
| sort | 1.19× | 1.17× | 1.01× | 166.27× |
| tally | 0.68× | 0.66× | 2.27× | 154.30× |
| server | 1.17× | 1.54× | 2.16× | 170.67× |
| files | 2.18× | 2.13× | 2.43× | 162.06× |
| readings | 4.00× | 3.20× | 2.49× | 161.38× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.00×, p 0.055 | level, 1.01×, p 2.4e-4 | behind, 1.08× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.00×, p 0.137 | level, 1.01×, p 6.7e-6 | behind, 1.08× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 0.354 | level, 0.99×, p 6.0e-20 | behind, 1.08× | ahead, 0.01× |
| empty | c | 20 | behind, p 6.4e-6 | behind, 1.91×, p 3.4e-8 | behind, 2.49× | behind, 179.38× |
| empty | cpp | 20 | behind, p 8.0e-7 | behind, 1.87×, p 3.4e-8 | behind, 2.49× | behind, 179.38× |
| empty | rust | 20 | behind, p 8.0e-7 | behind, 1.80×, p 3.4e-8 | behind, 2.34× | behind, 189.99× |
| add | c-wasm | 2000 | level, 1.00×, p 0.148 | level, 0.99×, p 6.3e-10 | behind, 1.08× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.00×, p 0.349 | level, 0.99×, p 2.6e-7 | behind, 1.08× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, 1.00×, p 0.498 | level, 0.98×, p 2.3e-60 | behind, 1.08× | ahead, 0.02× |
| add | c | 40 | behind, p 6.0e-6 | behind, 1.83×, p 7.2e-15 | behind, 2.49× | behind, 135.33× |
| add | cpp | 20 | behind, p 1.4e-4 | behind, 1.85×, p 3.4e-8 | behind, 2.48× | behind, 135.33× |
| add | rust | 20 | behind, p 3.6e-5 | behind, 1.76×, p 3.4e-8 | behind, 2.34× | behind, 166.25× |
| hypotenuse | c-wasm | 2000 | level, 1.00×, p 0.100 | level, 1.00×, p 2.5e-4 | behind, 1.08× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.00×, p 0.007 | level, 0.99×, p 3.4e-5 | behind, 1.09× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.192 | level, 0.98×, p 1.8e-56 | behind, 1.09× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, p 1.1e-4 | behind, 1.85×, p 3.4e-8 | behind, 2.49× | behind, 134.39× |
| hypotenuse | cpp | 20 | behind, p 2.9e-4 | behind, 1.86×, p 3.4e-8 | behind, 2.49× | behind, 133.92× |
| hypotenuse | rust | 40 | behind, p 1.1e-5 | behind, 1.75×, p 7.2e-15 | behind, 2.34× | behind, 164.83× |
| upcase | c-wasm | 2000 | level, 1.00×, p 0.189 | level, 1.00×, p 0.011 | behind, 1.08× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 1.00×, p 0.202 | level, 1.00×, p 0.009 | behind, 1.08× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.00×, p 0.235 | level, 0.99×, p 1.0e-14 | behind, 1.07× | ahead, 0.03× |
| upcase | c | 40 | behind, p 2.8e-7 | behind, 1.79×, p 7.2e-15 | behind, 2.09× | behind, 126.46× |
| upcase | cpp | 20 | behind, p 0.001 | behind, 1.75×, p 3.4e-8 | behind, 2.09× | behind, 126.46× |
| upcase | rust | 40 | behind, p 1.0e-6 | behind, 1.73×, p 7.2e-15 | behind, 2.15× | behind, 161.38× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.002 | level, 1.01×, p 3.5e-6 | behind, 1.09× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.025 | level, 1.00×, p 0.013 | behind, 1.09× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.191 | level, 0.99×, p 5.1e-28 | behind, 1.09× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 3.00×, p 9.6e-7 | behind, 1.51×, p 3.4e-8 | behind, 2.48× | behind, 172.96× |
| upcase-stdin | cpp | 20 | behind, 3.00×, p 1.8e-6 | behind, 1.52×, p 3.4e-8 | behind, 2.48× | behind, 172.96× |
| upcase-stdin | rust | 20 | behind, 3.00×, p 9.6e-7 | behind, 1.43×, p 3.4e-8 | behind, 2.36× | behind, 181.08× |
| shout | c-wasm | 640 | level, 1.00×, p 4.3e-4 | level, 1.01×, p 3.8e-5 | behind, 1.09× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.00×, p 0.033 | level, 1.01×, p 1.5e-6 | behind, 1.09× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, 1.00×, p 0.031 | level, 0.99×, p 6.8e-17 | behind, 1.08× | ahead, 0.03× |
| shout | c | 20 | behind, p 2.8e-4 | behind, 1.78×, p 3.4e-8 | behind, 2.12× | behind, 168.43× |
| shout | cpp | 20 | behind, p 1.2e-5 | behind, 1.77×, p 3.4e-8 | behind, 2.11× | behind, 168.43× |
| shout | rust | 20 | behind, p 1.2e-5 | behind, 1.71×, p 3.4e-8 | behind, 2.14× | behind, 173.74× |
| wc | c-wasm | 160 | level, 1.00×, p 1.7e-5 | ahead, 0.85×, p 8.9e-52 | behind, 1.09× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.23×, p 1.8e-8 | ahead, 0.22×, p 3.4e-8 | behind, 1.09× | ahead, 0.11× |
| wc | rust-wasm | 80 | level, 1.00×, p 8.6e-4 | ahead, 0.85×, p 2.5e-27 | behind, 1.08× | ahead, 0.04× |
| wc | c | 20 | behind, 1.50×, p 4.4e-4 | behind, 1.17×, p 3.4e-8 | behind, 2.48× | behind, 133.93× |
| wc | cpp | 20 | behind, 1.50×, p 2.8e-5 | behind, 1.14×, p 3.4e-8 | behind, 2.48× | behind, 133.93× |
| wc | rust | 20 | behind, 1.50×, p 0.001 | behind, 1.12×, p 3.4e-8 | behind, 2.37× | behind, 168.43× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 5.9e-8 | ahead, 0.68×, p 3.4e-8 | behind, 1.08× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 3.1e-8 | ahead, 0.69×, p 3.4e-8 | behind, 1.08× | ahead, 0.09× |
| histogram | rust-wasm | 640 | level, 1.00×, p 3.0e-5 | level, 0.97×, p 1.8e-136 | behind, 1.07× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 3.5e-8 | behind, 1.24×, p 3.4e-8 | behind, 2.45× | behind, 134.39× |
| histogram | cpp | 20 | behind, 1.33×, p 1.3e-6 | behind, 1.24×, p 3.4e-8 | behind, 2.46× | behind, 134.39× |
| histogram | rust | 20 | behind, 1.33×, p 1.0e-4 | behind, 1.20×, p 3.4e-8 | behind, 2.34× | behind, 165.54× |
| crc32c | c-wasm | 40 | level, 1.00×, p 4.2e-4 | ahead, 0.86×, p 9.4e-14 | behind, 1.08× | ahead, 0.08× |
| crc32c | cpp-wasm | 80 | ahead, 0.89×, p 2.2e-7 | ahead, 0.86×, p 7.6e-27 | behind, 1.08× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.80×, p 5.1e-4 | ahead, 0.83×, p 3.4e-8 | behind, 1.08× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.67×, p 2.5e-8 | behind, 2.22×, p 3.4e-8 | behind, 2.46× | behind, 134.86× |
| crc32c | cpp | 20 | behind, 4.00×, p 2.2e-8 | behind, 2.24×, p 3.4e-8 | behind, 2.46× | behind, 134.86× |
| crc32c | rust | 20 | behind, 4.00×, p 2.5e-8 | behind, 2.11×, p 3.4e-8 | behind, 2.34× | behind, 175.32× |
| sort | c-wasm | 20 | ahead, 0.21×, p 2.7e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.60×, p 2.5e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.68×, p 2.5e-8 | ahead, 0.67×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.45×, p 2.8e-8 | ahead, 0.43×, p 3.4e-8 | level, 1.01× | behind, 166.27× |
| sort | cpp | 20 | ahead, 0.46×, p 2.6e-8 | ahead, 0.45×, p 3.4e-8 | ahead, 0.90× | behind, 151.87× |
| sort | rust | 20 | behind, 1.19×, p 6.2e-8 | behind, 1.17×, p 3.9e-8 | level, 0.95× | behind, 163.45× |
| tally | c-wasm | 20 | ahead, 0.55×, p 1.7e-8 | ahead, 0.55×, p 3.4e-8 | behind, 1.11× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.55×, p 1.7e-8 | ahead, 0.55×, p 3.4e-8 | behind, 1.11× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.41×, p 1.6e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.10× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.68×, p 6.5e-9 | ahead, 0.66×, p 3.4e-8 | behind, 2.22× | behind, 131.21× |
| tally | cpp | 20 | ahead, 0.68×, p 2.0e-8 | ahead, 0.65×, p 3.4e-8 | behind, 2.24× | behind, 127.31× |
| tally | rust | 20 | ahead, 0.64×, p 1.8e-8 | ahead, 0.64×, p 3.4e-8 | behind, 2.27× | behind, 154.30× |
| server | c-wasm | 71 | level, 1.01×, p 0.060 | ahead, 0.93×, p 1.8e-19 | behind, 1.09× | ahead, 0.12× |
| server | cpp-wasm | 71 | level, 1.01×, p 0.080 | ahead, 0.93×, p 2.9e-19 | behind, 1.09× | ahead, 0.06× |
| server | rust-wasm | 71 | level, 1.00×, p 0.361 | ahead, 0.93×, p 1.7e-20 | behind, 1.07× | ahead, 0.03× |
| server | c | 20 | behind, 1.17×, p 2.9e-8 | behind, 1.17×, p 3.4e-8 | behind, 2.16× | behind, 170.67× |
| server | cpp | 20 | behind, 1.17×, p 3.1e-8 | behind, 1.16×, p 3.5e-7 | behind, 2.15× | behind, 166.97× |
| server | rust | 20 | ahead, 0.88×, p 4.7e-8 | behind, 1.54×, p 3.4e-8 | behind, 1.78× | behind, 141.28× |
| files | c-wasm | 20 | ahead, 0.86×, p 1.4e-5 | ahead, 0.91×, p 5.3e-8 | behind, 1.08× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.92×, p 1.8e-4 | ahead, 0.90×, p 6.2e-8 | behind, 1.08× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 9.2e-6 | ahead, 0.90×, p 5.3e-8 | behind, 1.08× | ahead, 0.03× |
| files | c | 20 | behind, 2.00×, p 1.0e-8 | behind, 1.96×, p 3.4e-8 | behind, 2.43× | behind, 127.30× |
| files | cpp | 20 | behind, 2.00×, p 8.7e-9 | behind, 1.97×, p 3.4e-8 | behind, 2.43× | behind, 127.30× |
| files | rust | 20 | behind, 2.18×, p 1.7e-8 | behind, 2.13×, p 3.4e-8 | behind, 2.31× | behind, 162.06× |
| readings | c-wasm | 640 | level, 1.00×, p 2.9e-7 | level, 0.97×, p 2.0e-186 | behind, 1.08× | ahead, 0.04× |
| readings | cpp-wasm | 1734 | level, 1.00×, p 0.013 | level, 0.99×, p 1.0e-299 | behind, 1.08× | ahead, 0.04× |
| readings | rust-wasm | 80 | level, 1.00×, p 5.5e-4 | level, 0.95×, p 8.2e-27 | behind, 1.08× | ahead, 0.02× |
| readings | c | 20 | behind, 4.00×, p 4.9e-9 | behind, 3.20×, p 3.4e-8 | behind, 2.49× | behind, 135.33× |
| readings | cpp | 20 | behind, 4.00×, p 4.9e-9 | behind, 3.13×, p 3.4e-8 | behind, 2.48× | behind, 135.33× |
| readings | rust | 20 | behind, 2.67×, p 2.0e-8 | behind, 2.44×, p 3.4e-8 | behind, 2.34× | behind, 161.38× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 21.0 ms | 20.9 ms | 20.9 ms | 21.3 ms | 11.3 ms | 11.6 ms | 12.0 ms |
| peak memory | 11.6 MiB | 10.7 MiB | 10.7 MiB | 10.7 MiB | 4.66 MiB | 4.66 MiB | 4.95 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 108 KiB | 108 KiB | 102 KiB |

Its rounds took 299 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 21.8 ms | 21.9 ms | 21.9 ms | 22.3 ms | 11.2 ms | 11.2 ms | 11.8 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.69 MiB | 4.70 MiB | 4.98 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 142 KiB | 142 KiB | 116 KiB |

Its rounds took 312 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 21.1 ms | 21.2 ms | 21.2 ms | 21.6 ms | 11.2 ms | 11.1 ms | 11.7 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.7 MiB | 10.8 MiB | 4.69 MiB | 4.69 MiB | 4.98 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 144 KiB | 144 KiB | 117 KiB |

Its rounds took 300 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 20 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 22.6 ms | 22.5 ms | 22.5 ms | 22.9 ms | 12.4 ms | 12.8 ms | 12.8 ms |
| peak memory | 12.2 MiB | 11.2 MiB | 11.2 MiB | 11.4 MiB | 5.80 MiB | 5.80 MiB | 5.65 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 152 KiB | 152 KiB | 120 KiB |

Its rounds took 311 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 46.9 ms | 46.9 ms | 46.9 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 45.4 ms | 45.2 ms | 45.3 ms | 46.0 ms | 30.7 ms | 30.4 ms | 32.3 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.73 MiB | 4.73 MiB | 4.97 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 112 KiB | 112 KiB | 106 KiB |

Its rounds took 498 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

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
| counted runs | 2000 | 640 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 21.0 ms | 20.5 ms | 20.9 ms | 21.3 ms | 11.7 ms | 11.7 ms | 12.2 ms |
| peak memory | 12.0 MiB | 11.0 MiB | 11.0 MiB | 11.1 MiB | 5.66 MiB | 5.67 MiB | 5.59 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 114 KiB | 114 KiB | 111 KiB |

Its rounds took 246 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

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
| counted runs | 160 | 160 | 20 | 80 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 46.9 ms | 203 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 47.0 ms | 55.4 ms | 215 ms | 56.6 ms | 40.4 ms | 41.2 ms | 41.9 ms |
| peak memory | 11.8 MiB | 10.8 MiB | 10.8 MiB | 10.9 MiB | 4.76 MiB | 4.76 MiB | 4.98 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 144 KiB | 144 KiB | 114 KiB |

Its rounds took 37 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

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
| counted runs | 640 | 20 | 20 | 640 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 93.8 ms | 93.8 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 68.4 ms | 101 ms | 101 ms | 70.7 ms | 56.0 ms | 55.8 ms | 57.7 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.9 MiB | 4.76 MiB | 4.76 MiB | 5.00 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 144 KiB | 144 KiB | 116 KiB |

Its rounds took 121 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

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
| counted runs | 80 | 40 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 62.5 ms | 70.3 ms | 78.1 ms | 23.4 ms | 15.6 ms | 15.6 ms |
| wall time | 65.4 ms | 76.1 ms | 75.9 ms | 79.2 ms | 29.5 ms | 29.3 ms | 31.1 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.75 MiB | 4.75 MiB | 4.99 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 143 KiB | 143 KiB | 110 KiB |

Its rounds took 24 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

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
| CPU time | 297 ms | 1.43 s | 492 ms | 438 ms | 656 ms | 641 ms | 250 ms |
| wall time | 293 ms | 1.44 s | 500 ms | 440 ms | 680 ms | 657 ms | 249 ms |
| peak memory | 80.6 MiB | 79.6 MiB | 87.6 MiB | 87.7 MiB | 79.7 MiB | 89.7 MiB | 84.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 116 KiB | 127 KiB | 118 KiB |

Its rounds took 103 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

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
| CPU time | 180 ms | 328 ms | 328 ms | 438 ms | 266 ms | 266 ms | 281 ms |
| wall time | 181 ms | 332 ms | 332 ms | 443 ms | 276 ms | 279 ms | 282 ms |
| peak memory | 12.0 MiB | 10.8 MiB | 10.8 MiB | 10.9 MiB | 5.39 MiB | 5.34 MiB | 5.27 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 147 KiB | 152 KiB | 125 KiB |

Its rounds took 51 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

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
| counted runs | 71 | 71 | 71 | 71 | 20 | 20 | 20 |
| CPU time | 1.59 s | 1.58 s | 1.58 s | 1.59 s | 1.36 s | 1.36 s | 1.81 s |
| wall time | 1.60 s | 1.71 s | 1.71 s | 1.73 s | 1.38 s | 1.39 s | 1.04 s |
| peak memory | 11.5 MiB | 10.5 MiB | 10.6 MiB | 10.7 MiB | 5.30 MiB | 5.31 MiB | 6.45 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 113 KiB | 116 KiB | 136 KiB |

Its rounds took 639 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

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
| CPU time | 188 ms | 219 ms | 203 ms | 203 ms | 93.8 ms | 93.8 ms | 85.9 ms |
| wall time | 198 ms | 218 ms | 219 ms | 220 ms | 101 ms | 100 ms | 92.9 ms |
| peak memory | 11.9 MiB | 11.1 MiB | 11.1 MiB | 11.1 MiB | 4.90 MiB | 4.90 MiB | 5.16 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 152 KiB | 152 KiB | 119 KiB |

Its rounds took 30 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

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
| counted runs | 1734 | 640 | 1734 | 80 | 20 | 20 | 20 |
| CPU time | 125 ms | 125 ms | 125 ms | 125 ms | 31.2 ms | 31.2 ms | 46.9 ms |
| wall time | 124 ms | 128 ms | 126 ms | 131 ms | 39.1 ms | 40.0 ms | 51.3 ms |
| peak memory | 11.7 MiB | 10.8 MiB | 10.8 MiB | 10.8 MiB | 4.70 MiB | 4.71 MiB | 5.00 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 142 KiB | 142 KiB | 120 KiB |

Its rounds took 603 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 22.1.8 (https://github.com/llvm/llvm-project ca7933e47d3a3451d81e72ac174dcb5aa28b59d1); clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 12 s | means 15.6 ms against 15.7 ms: -97.7 µs (-2.34 ms to +2.15 ms) | 20.5 ms against 20.4 ms: +93.3 µs (-30.7 µs to +222 µs) | 11.6 MiB against 10.7 MiB: +888 KiB (+884 KiB to +888 KiB) |
| add | 160, 160 in 11 s | means 16.0 ms against 15.6 ms: +391 µs (-1.71 ms to +2.49 ms) | 20.3 ms against 20.1 ms: +184 µs (+49.7 µs to +319 µs) | 11.7 MiB against 10.8 MiB: +928 KiB (+924 KiB to +928 KiB) |
| hypotenuse | 160, 160 in 12 s | means 17.9 ms against 15.3 ms: +2.54 ms (+230 µs to +4.85 ms) | 20.5 ms against 20.4 ms: +91.7 µs (-72.1 µs to +263 µs) | 11.7 MiB against 10.8 MiB: +944 KiB (+940 KiB to +944 KiB) |
| upcase | 160, 160 in 13 s | means 19.3 ms against 18.2 ms: +1.17 ms (-903 µs to +3.25 ms) | 23.3 ms against 23.1 ms: +292 µs (+113 µs to +471 µs) | 12.1 MiB against 11.2 MiB: +984 KiB (+980 KiB to +988 KiB) |
| upcase-stdin | 160, 160 in 19 s | means 41.8 ms against 41.4 ms: +391 µs (-2.58 ms to +3.36 ms) | 44.5 ms against 44.9 ms: -238 µs (-561 µs to +74.6 µs) | 11.7 MiB against 10.8 MiB: +960 KiB (+956 KiB to +960 KiB) |
| shout | 160, 160 in 12 s | means 16.1 ms against 16.4 ms: -293 µs (-2.18 ms to +1.59 ms) | 21.4 ms against 21.2 ms: +70.8 µs (-151 µs to +289 µs) | 12.0 MiB against 11.0 MiB: +980 KiB (+980 KiB to +984 KiB) |
| wc | 160, 160 in 21 s | means 46.7 ms against 44.6 ms: +2.05 ms (-676 µs to +4.78 ms) | 48.4 ms against 48.2 ms: +246 µs (-86.4 µs to +581 µs) | 11.8 MiB against 10.8 MiB: +1.00 MiB (+1020 KiB to +1.00 MiB) |
| histogram | 160, 160 in 27 s | means 65.6 ms against 64.9 ms: +684 µs (-1.43 ms to +2.79 ms) | 68.7 ms against 68.6 ms: +80.8 µs (-288 µs to +426 µs) | 11.7 MiB against 10.8 MiB: +924 KiB (+924 KiB to +924 KiB) |
| crc32c | 160, 160 in 27 s | means 63.9 ms against 63.0 ms: +879 µs (-2.26 ms to +4.02 ms) | 66.1 ms against 66.1 ms: -24.1 µs (-297 µs to +249 µs) | 11.7 MiB against 10.8 MiB: +928 KiB (+924 KiB to +928 KiB) |
| sort | 97, 97 in 60 s | means 289 ms against 288 ms: +1.29 ms (-2.94 ms to +5.52 ms) | 296 ms against 296 ms: +7.80 µs (-2.98 ms to +2.77 ms) | 80.6 MiB against 79.6 MiB: +1.03 MiB (+1.03 MiB to +1.04 MiB) |
| tally | 153, 153 in 60 s | means 175 ms against 175 ms: +817 µs (-1.47 ms to +3.11 ms) | 180 ms against 180 ms: +190 µs (-421 µs to +800 µs) | 12.0 MiB against 10.9 MiB: +1.04 MiB (+1.03 MiB to +1.04 MiB) |
| server | 20, 20 in 70 s | means 1.60 s against 1.61 s: -1.56 ms (-17.5 ms to +14.3 ms) | 1.61 s against 1.61 s: +3.59 ms (-6.78 ms to +16.9 ms) | 11.5 MiB against 10.5 MiB: +956 KiB (+944 KiB to +988 KiB) |
| files | 139, 139 in 60 s | means 192 ms against 192 ms: -112 µs (-3.23 ms to +3.01 ms) | 198 ms against 198 ms: +277 µs (-904 µs to +1.47 ms) | 11.9 MiB against 11.0 MiB: +944 KiB (+940 KiB to +948 KiB) |
| readings | 160, 160 in 45 s | means 122 ms against 121 ms: +977 µs (-1.83 ms to +3.78 ms) | 124 ms against 124 ms: +169 µs (-29.9 µs to +368 µs) | 11.7 MiB against 10.8 MiB: +928 KiB (+924 KiB to +928 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
