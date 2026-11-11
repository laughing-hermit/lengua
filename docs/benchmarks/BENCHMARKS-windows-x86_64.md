# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit cd54e79fc07fe745f5fcad008bd2a7da4a18ae9e, in https://github.com/laughing-hermit/lengua/actions/runs/42964232679. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (25.4 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
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
| empty | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✗ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ≈ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ≈ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ≈ ✓ ✗ ✓ | ≈ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ✓ ✗ ✓ | ≈ ≈ ✗ ✓ | ≈ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 empty c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 619 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty c-wasm wall: lengua is level now, and KNOWN_BEHIND line 620 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 622 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 623 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 625 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 639 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 641 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 643 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 657 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 659 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 674 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 676 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 678 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 692 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 695 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 698 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin cpp cpu: lengua is level now, and KNOWN_BEHIND line 704 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 712 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout c-wasm wall: lengua is level now, and KNOWN_BEHIND line 713 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 715 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout cpp-wasm wall: lengua is level now, and KNOWN_BEHIND line 716 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 718 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc cpp wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 sort c-wasm memory: lengua is level now, and KNOWN_BEHIND line 776 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort c memory: lengua is level now, and KNOWN_BEHIND line 777 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 779 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort cpp wall: lengua is ahead now, and KNOWN_BEHIND line 780 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort cpp memory: lengua is ahead now, and KNOWN_BEHIND line 781 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort rust cpu: lengua is ahead now, and KNOWN_BEHIND line 783 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 sort rust wall: lengua is ahead now, and KNOWN_BEHIND line 784 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 tally cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 791 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 tally cpp wall: lengua is ahead now, and KNOWN_BEHIND line 792 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server c-wasm cpu: lengua is level now, and KNOWN_BEHIND line 797 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server cpp-wasm cpu: lengua is level now, and KNOWN_BEHIND line 799 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server rust-wasm cpu: lengua is level now, and KNOWN_BEHIND line 801 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server c wall: lengua is level now, and KNOWN_BEHIND line 804 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 2.79× | 3.05× | 249.02× |
| add | – | 2.66× | 3.02× | 218.69× |
| hypotenuse | – | 2.70× | 3.02× | 215.07× |
| upcase | – | 2.27× | 2.45× | 209.87× |
| upcase-stdin | 2.00× | 1.64× | 3.01× | 239.85× |
| shout | – | 2.42× | 2.43× | 229.28× |
| wc | 1.25× | 1.23× | 2.99× | 217.78× |
| histogram | 1.33× | 1.31× | 2.97× | 219.61× |
| crc32c | 2.00× | 2.20× | 2.97× | 232.36× |
| sort | 0.69× | 0.69× | 1.01× | 216.89× |
| tally | 0.56× | 0.58× | 2.66× | 202.54× |
| server | 1.43× | 2.26× | 2.54× | 221.48× |
| files | 2.00× | 1.90× | 2.90× | 209.87× |
| readings | 7.00× | 5.15× | 3.02× | 209.87× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 1280 | level, 1.00×, p 7.2e-4 | level, 1.01×, p 3.3e-26 | behind, 1.12× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.00×, p 2.9e-4 | level, 1.02×, p 3.1e-40 | behind, 1.12× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 0.154 | level, 0.97×, p 7.5e-95 | behind, 1.11× | ahead, 0.01× |
| empty | c | 20 | behind, p 1.1e-4 | behind, 2.79×, p 3.4e-8 | behind, 3.04× | behind, 232.35× |
| empty | cpp | 20 | behind, p 1.1e-4 | behind, 2.74×, p 3.4e-8 | behind, 3.05× | behind, 232.35× |
| empty | rust | 20 | behind, p 7.4e-4 | behind, 2.47×, p 3.4e-8 | behind, 2.74× | behind, 249.02× |
| add | c-wasm | 2000 | level, 1.00×, p 0.015 | level, 0.99×, p 7.5e-26 | behind, 1.11× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, 1.00×, p 0.003 | level, 0.99×, p 1.0e-22 | behind, 1.11× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, 1.00×, p 0.335 | level, 0.96×, p 2.2e-143 | behind, 1.11× | ahead, 0.02× |
| add | c | 20 | behind, p 2.1e-4 | behind, 2.66×, p 3.4e-8 | behind, 3.02× | behind, 176.43× |
| add | cpp | 20 | behind, p 2.1e-4 | behind, 2.62×, p 3.4e-8 | behind, 3.02× | behind, 177.03× |
| add | rust | 40 | behind, p 1.0e-6 | behind, 2.41×, p 1.2e-13 | behind, 2.73× | behind, 218.69× |
| hypotenuse | c-wasm | 2000 | level, 1.00×, p 0.226 | level, 0.99×, p 3.8e-13 | behind, 1.11× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.00×, p 0.009 | level, 0.99×, p 1.0e-7 | behind, 1.12× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.128 | level, 0.96×, p 3.7e-113 | behind, 1.11× | ahead, 0.03× |
| hypotenuse | c | 40 | behind, p 1.5e-5 | behind, 2.66×, p 7.2e-15 | behind, 3.02× | behind, 175.25× |
| hypotenuse | cpp | 40 | behind, p 4.7e-5 | behind, 2.70×, p 7.2e-15 | behind, 3.02× | behind, 174.66× |
| hypotenuse | rust | 40 | behind, p 1.5e-5 | behind, 2.42×, p 7.2e-15 | behind, 2.73× | behind, 215.07× |
| upcase | c-wasm | 2000 | level, 1.00×, p 1.0e-7 | level, 1.00×, p 0.014 | behind, 1.11× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 1.00×, p 0.042 | level, 1.00×, p 0.009 | behind, 1.11× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.00×, p 4.3e-5 | level, 0.97×, p 2.4e-95 | behind, 1.09× | ahead, 0.03× |
| upcase | c | 20 | behind, p 2.9e-5 | behind, 2.26×, p 3.4e-8 | behind, 2.40× | behind, 166.29× |
| upcase | cpp | 20 | behind, p 2.7e-6 | behind, 2.27×, p 4.6e-7 | behind, 2.40× | behind, 166.29× |
| upcase | rust | 20 | behind, p 1.4e-5 | behind, 2.15×, p 3.4e-8 | behind, 2.45× | behind, 209.87× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.262 | level, 0.99×, p 2.1e-105 | behind, 1.11× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.404 | level, 0.98×, p 1.9e-247 | behind, 1.11× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.094 | level, 0.98×, p 2.8e-220 | behind, 1.11× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 4.0e-6 | behind, 1.63×, p 3.4e-8 | behind, 3.01× | behind, 224.34× |
| upcase-stdin | cpp | 20 | level, 1.00×, p 2.8e-4 | behind, 1.64×, p 3.4e-8 | behind, 3.01× | behind, 224.34× |
| upcase-stdin | rust | 20 | behind, 2.00×, p 9.2e-5 | behind, 1.51×, p 3.4e-8 | behind, 2.71× | behind, 239.85× |
| shout | c-wasm | 2000 | level, 1.00×, p 0.212 | level, 1.01×, p 1.9e-21 | behind, 1.11× | ahead, 0.40× |
| shout | cpp-wasm | 320 | level, 1.00×, p 2.0e-4 | level, 1.01×, p 8.7e-7 | behind, 1.11× | ahead, 0.40× |
| shout | rust-wasm | 80 | level, 1.00×, p 0.001 | level, 0.98×, p 2.8e-6 | behind, 1.11× | ahead, 0.03× |
| shout | c | 20 | behind, p 4.6e-6 | behind, 2.40×, p 3.4e-8 | behind, 2.43× | behind, 218.69× |
| shout | cpp | 20 | behind, p 1.8e-6 | behind, 2.42×, p 3.4e-8 | behind, 2.42× | behind, 218.69× |
| shout | rust | 20 | behind, p 1.8e-6 | behind, 2.29×, p 3.4e-8 | behind, 2.43× | behind, 229.28× |
| wc | c-wasm | 20 | ahead, 0.83×, p 9.6e-4 | ahead, 0.84×, p 3.0e-7 | behind, 1.11× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 2.0e-8 | ahead, 0.19×, p 3.4e-8 | behind, 1.11× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.83×, p 7.0e-4 | ahead, 0.84×, p 2.6e-7 | behind, 1.11× | ahead, 0.04× |
| wc | c | 20 | behind, 1.25×, p 0.001 | behind, 1.23×, p 3.4e-8 | behind, 2.99× | behind, 174.66× |
| wc | cpp | 160 | behind, 1.50×, p 1.2e-6 | behind, 1.08×, p 4.3e-34 | behind, 2.99× | behind, 174.08× |
| wc | rust | 40 | behind, 1.50×, p 2.6e-6 | behind, 1.22×, p 7.1e-14 | behind, 2.70× | behind, 217.78× |
| histogram | c-wasm | 320 | level, 1.00×, p 3.8e-5 | ahead, 0.92×, p 2.1e-81 | behind, 1.11× | ahead, 0.09× |
| histogram | cpp-wasm | 80 | level, 1.00×, p 1.3e-4 | ahead, 0.92×, p 6.5e-22 | behind, 1.11× | ahead, 0.09× |
| histogram | rust-wasm | 320 | level, 1.00×, p 2.8e-4 | ahead, 0.95×, p 4.4e-73 | behind, 1.11× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 1.4e-7 | behind, 1.31×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| histogram | cpp | 20 | behind, 1.33×, p 1.4e-7 | behind, 1.31×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| histogram | rust | 20 | behind, 1.33×, p 2.2e-5 | behind, 1.27×, p 3.4e-8 | behind, 2.70× | behind, 219.61× |
| crc32c | c-wasm | 80 | ahead, 0.80×, p 1.4e-9 | ahead, 0.87×, p 3.4e-25 | behind, 1.11× | ahead, 0.08× |
| crc32c | cpp-wasm | 80 | ahead, 0.80×, p 9.3e-8 | ahead, 0.87×, p 1.2e-24 | behind, 1.11× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.80×, p 1.3e-4 | ahead, 0.84×, p 4.6e-7 | behind, 1.11× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.00×, p 7.7e-9 | behind, 2.20×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| crc32c | cpp | 20 | behind, 2.00×, p 7.7e-9 | behind, 2.19×, p 3.4e-8 | behind, 2.97× | behind, 175.84× |
| crc32c | rust | 20 | behind, 2.00×, p 7.7e-9 | behind, 2.07×, p 3.4e-8 | behind, 2.68× | behind, 232.36× |
| sort | c-wasm | 20 | ahead, 0.18×, p 2.8e-8 | ahead, 0.18×, p 3.4e-8 | level, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.53×, p 2.4e-8 | ahead, 0.54×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.60×, p 1.0e-7 | ahead, 0.59×, p 2.3e-7 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.43×, p 2.3e-8 | ahead, 0.43×, p 3.4e-8 | level, 1.01× | behind, 216.89× |
| sort | cpp | 20 | ahead, 0.46×, p 2.1e-8 | ahead, 0.46×, p 3.4e-8 | ahead, 0.88× | behind, 191.37× |
| sort | rust | 20 | ahead, 0.69×, p 3.0e-7 | ahead, 0.69×, p 6.0e-7 | ahead, 0.94× | behind, 215.99× |
| tally | c-wasm | 20 | ahead, 0.40×, p 2.2e-8 | ahead, 0.41×, p 3.4e-8 | behind, 1.13× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.40×, p 2.2e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.13× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.30×, p 2.1e-8 | ahead, 0.31×, p 3.4e-8 | behind, 1.14× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.56×, p 2.3e-8 | ahead, 0.58×, p 3.4e-8 | behind, 2.59× | behind, 171.23× |
| tally | cpp | 20 | ahead, 0.53×, p 2.1e-8 | ahead, 0.54×, p 3.4e-8 | behind, 2.62× | behind, 165.78× |
| tally | rust | 20 | ahead, 0.45×, p 2.0e-8 | ahead, 0.48×, p 3.4e-8 | behind, 2.66× | behind, 202.54× |
| server | c-wasm | 53 | level, 0.99×, p 0.243 | ahead, 0.71×, p 3.6e-19 | behind, 1.12× | ahead, 0.12× |
| server | cpp-wasm | 53 | level, 0.99×, p 0.089 | ahead, 0.72×, p 3.6e-19 | behind, 1.11× | ahead, 0.06× |
| server | rust-wasm | 53 | level, 0.99×, p 0.210 | ahead, 0.71×, p 3.6e-19 | behind, 1.09× | ahead, 0.03× |
| server | c | 53 | behind, 1.06×, p 1.1e-5 | level, 1.05×, p 1.4e-4 | behind, 2.54× | behind, 221.48× |
| server | cpp | 53 | behind, 1.06×, p 2.1e-5 | behind, 1.06×, p 7.9e-5 | behind, 2.54× | behind, 216.87× |
| server | rust | 20 | behind, 1.43×, p 3.3e-8 | behind, 2.26×, p 3.4e-8 | behind, 1.93× | behind, 184.57× |
| files | c-wasm | 80 | level, 0.95×, p 1.4e-4 | ahead, 0.95×, p 5.8e-10 | behind, 1.10× | ahead, 0.05× |
| files | cpp-wasm | 160 | level, 1.00×, p 7.4e-9 | ahead, 0.94×, p 3.3e-23 | behind, 1.10× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 0.91×, p 4.6e-6 | ahead, 0.94×, p 3.1e-11 | behind, 1.10× | ahead, 0.03× |
| files | c | 20 | behind, 1.67×, p 2.0e-8 | behind, 1.76×, p 3.4e-8 | behind, 2.90× | behind, 167.36× |
| files | cpp | 20 | behind, 2.00×, p 1.8e-8 | behind, 1.75×, p 3.4e-8 | behind, 2.90× | behind, 167.36× |
| files | rust | 20 | behind, 2.00×, p 1.7e-8 | behind, 1.90×, p 3.4e-8 | behind, 2.66× | behind, 209.87× |
| readings | c-wasm | 160 | level, 1.00×, p 1.1e-6 | ahead, 0.94×, p 8.7e-16 | behind, 1.11× | ahead, 0.04× |
| readings | cpp-wasm | 320 | level, 1.00×, p 0.001 | level, 0.96×, p 7.8e-31 | behind, 1.11× | ahead, 0.04× |
| readings | rust-wasm | 160 | level, 1.00×, p 2.9e-5 | ahead, 0.93×, p 2.9e-17 | behind, 1.11× | ahead, 0.02× |
| readings | c | 20 | behind, 7.00×, p 1.6e-8 | behind, 5.15×, p 3.4e-8 | behind, 3.02× | behind, 176.43× |
| readings | cpp | 20 | behind, 7.00×, p 2.4e-8 | behind, 4.46×, p 3.4e-8 | behind, 3.01× | behind, 177.03× |
| readings | rust | 20 | behind, 3.50×, p 1.9e-8 | behind, 2.71×, p 3.4e-8 | behind, 2.72× | behind, 209.87× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 1280 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 14.1 ms | 13.9 ms | 13.8 ms | 14.4 ms | 5.20 ms | 5.30 ms | 5.87 ms |
| peak memory | 10.2 MiB | 9.17 MiB | 9.17 MiB | 9.24 MiB | 3.36 MiB | 3.36 MiB | 3.73 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 201 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 14.1 ms | 14.3 ms | 14.3 ms | 14.7 ms | 5.30 ms | 5.39 ms | 5.85 ms |
| peak memory | 10.3 MiB | 9.28 MiB | 9.28 MiB | 9.27 MiB | 3.40 MiB | 3.40 MiB | 3.76 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 147 KiB | 119 KiB |

Its rounds took 224 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 14.0 ms | 14.2 ms | 14.1 ms | 14.6 ms | 5.20 ms | 5.13 ms | 5.73 ms |
| peak memory | 10.3 MiB | 9.26 MiB | 9.18 MiB | 9.25 MiB | 3.40 MiB | 3.40 MiB | 3.77 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 149 KiB | 121 KiB |

Its rounds took 225 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 15.4 ms | 15.5 ms | 15.5 ms | 15.8 ms | 7.02 ms | 7.00 ms | 7.37 ms |
| peak memory | 10.7 MiB | 9.71 MiB | 9.71 MiB | 9.86 MiB | 4.48 MiB | 4.48 MiB | 4.40 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 232 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 31.2 ms | 15.6 ms |
| wall time | 37.5 ms | 38.0 ms | 38.4 ms | 38.3 ms | 22.9 ms | 22.9 ms | 24.8 ms |
| peak memory | 10.3 MiB | 9.28 MiB | 9.29 MiB | 9.29 MiB | 3.43 MiB | 3.43 MiB | 3.81 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 108 KiB |

Its rounds took 415 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

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
| counted runs | 2000 | 2000 | 320 | 80 | 20 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 14.1 ms | 14.0 ms | 13.9 ms | 14.2 ms | 5.84 ms | 5.79 ms | 6.14 ms |
| peak memory | 10.6 MiB | 9.50 MiB | 9.50 MiB | 9.58 MiB | 4.37 MiB | 4.37 MiB | 4.35 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 121 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

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
| counted runs | 160 | 20 | 20 | 20 | 20 | 160 | 40 |
| CPU time | 46.9 ms | 46.9 ms | 234 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 42.4 ms | 50.7 ms | 228 ms | 50.9 ms | 34.7 ms | 39.4 ms | 35.1 ms |
| peak memory | 10.3 MiB | 9.29 MiB | 9.28 MiB | 9.29 MiB | 3.46 MiB | 3.45 MiB | 3.84 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 149 KiB | 150 KiB | 120 KiB |

Its rounds took 30 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

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
| counted runs | 320 | 320 | 80 | 320 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 62.5 ms | 62.5 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 63.1 ms | 68.3 ms | 68.5 ms | 66.5 ms | 48.1 ms | 48.2 ms | 49.6 ms |
| peak memory | 10.3 MiB | 9.29 MiB | 9.29 MiB | 9.30 MiB | 3.46 MiB | 3.46 MiB | 3.80 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 118 KiB |

Its rounds took 89 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

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
| counted runs | 80 | 80 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 78.1 ms | 78.1 ms | 78.1 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 64.3 ms | 74.3 ms | 74.0 ms | 75.5 ms | 29.0 ms | 29.1 ms | 30.8 ms |
| peak memory | 10.3 MiB | 9.29 MiB | 9.29 MiB | 9.28 MiB | 3.45 MiB | 3.46 MiB | 3.83 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 26 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 289 ms | 1.59 s | 547 ms | 484 ms | 672 ms | 625 ms | 422 ms |
| wall time | 290 ms | 1.60 s | 540 ms | 488 ms | 676 ms | 629 ms | 421 ms |
| peak memory | 79.2 MiB | 78.1 MiB | 86.2 MiB | 86.2 MiB | 78.4 MiB | 90.4 MiB | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 136 KiB | 120 KiB |

Its rounds took 111 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

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
| CPU time | 156 ms | 391 ms | 391 ms | 516 ms | 281 ms | 297 ms | 344 ms |
| wall time | 163 ms | 399 ms | 391 ms | 521 ms | 280 ms | 300 ms | 338 ms |
| peak memory | 10.6 MiB | 9.32 MiB | 9.33 MiB | 9.29 MiB | 4.09 MiB | 4.04 MiB | 3.98 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 157 KiB | 128 KiB |

Its rounds took 58 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

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
| counted runs | 53 | 53 | 53 | 53 | 53 | 53 | 20 |
| CPU time | 1.41 s | 1.42 s | 1.42 s | 1.42 s | 1.33 s | 1.33 s | 1.09 s |
| wall time | 1.42 s | 1.99 s | 1.99 s | 2.00 s | 1.36 s | 1.34 s | 696 ms |
| peak memory | 10.1 MiB | 9.07 MiB | 9.13 MiB | 9.28 MiB | 3.99 MiB | 3.98 MiB | 5.26 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 120 KiB | 141 KiB |

Its rounds took 642 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

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
| counted runs | 160 | 80 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 156 ms | 164 ms | 156 ms | 172 ms | 93.8 ms | 78.1 ms | 78.1 ms |
| wall time | 154 ms | 162 ms | 164 ms | 163 ms | 88.2 ms | 88.6 ms | 81.5 ms |
| peak memory | 10.4 MiB | 9.45 MiB | 9.45 MiB | 9.46 MiB | 3.59 MiB | 3.59 MiB | 3.91 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 95 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

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
| counted runs | 320 | 160 | 320 | 160 | 20 | 20 | 20 |
| CPU time | 109 ms | 109 ms | 109 ms | 109 ms | 15.6 ms | 15.6 ms | 31.2 ms |
| wall time | 104 ms | 112 ms | 108 ms | 113 ms | 21.0 ms | 24.2 ms | 39.9 ms |
| peak memory | 10.3 MiB | 9.28 MiB | 9.28 MiB | 9.27 MiB | 3.39 MiB | 3.40 MiB | 3.78 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 147 KiB | 124 KiB |

Its rounds took 125 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -std=gnu++17 -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 9 s | means 17.2 ms against 15.4 ms: +1.76 ms (-960 µs to +4.48 ms) | 14.0 ms against 13.9 ms: +198 µs (+91.7 µs to +313 µs) | 10.2 MiB against 9.28 MiB: +952 KiB (+948 KiB to +960 KiB) |
| add | 160, 160 in 9 s | means 16.4 ms against 14.7 ms: +1.66 ms (-1.12 ms to +4.44 ms) | 13.7 ms against 13.5 ms: +196 µs (+76.0 µs to +315 µs) | 10.2 MiB against 9.28 MiB: +984 KiB (+984 KiB to +988 KiB) |
| hypotenuse | 160, 160 in 9 s | means 16.0 ms against 14.7 ms: +1.27 ms (-1.28 ms to +3.82 ms) | 14.0 ms against 14.0 ms: +99.8 µs (-59.0 µs to +258 µs) | 10.3 MiB against 9.28 MiB: +1000 KiB (+1000 KiB to +1004 KiB) |
| upcase | 160, 160 in 9 s | means 17.1 ms against 15.5 ms: +1.56 ms (-665 µs to +3.79 ms) | 15.1 ms against 14.8 ms: +334 µs (+264 µs to +407 µs) | 10.7 MiB against 9.69 MiB: +1.04 MiB (+1.04 MiB to +1.05 MiB) |
| upcase-stdin | 160, 160 in 16 s | means 38.8 ms against 39.0 ms: -195 µs (-2.83 ms to +2.44 ms) | 37.0 ms against 37.0 ms: +75.0 µs (-38.3 µs to +188 µs) | 10.3 MiB against 9.31 MiB: +1016 KiB (+1012 KiB to +1020 KiB) |
| shout | 160, 160 in 8 s | means 14.3 ms against 14.5 ms: -195 µs (-2.67 ms to +2.28 ms) | 13.8 ms against 13.6 ms: +194 µs (+121 µs to +265 µs) | 10.6 MiB against 9.61 MiB: +1020 KiB (+1016 KiB to +1.00 MiB) |
| wc | 160, 160 in 18 s | means 44.8 ms against 46.2 ms: -1.37 ms (-4.26 ms to +1.53 ms) | 42.3 ms against 42.1 ms: +194 µs (+32.8 µs to +360 µs) | 10.3 MiB against 9.31 MiB: +1.03 MiB (+1.02 MiB to +1.03 MiB) |
| histogram | 160, 160 in 25 s | means 66.4 ms against 64.8 ms: +1.56 ms (-1.06 ms to +4.18 ms) | 62.9 ms against 62.6 ms: +224 µs (+38.4 µs to +421 µs) | 10.3 MiB against 9.32 MiB: +980 KiB (+976 KiB to +988 KiB) |
| crc32c | 160, 160 in 26 s | means 68.9 ms against 67.3 ms: +1.66 ms (-864 µs to +4.18 ms) | 64.2 ms against 64.1 ms: +165 µs (-130 µs to +472 µs) | 10.3 MiB against 9.31 MiB: +984 KiB (+980 KiB to +984 KiB) |
| sort | 96, 96 in 60 s | means 298 ms against 299 ms: -488 µs (-5.83 ms to +4.85 ms) | 295 ms against 294 ms: +1.08 ms (-671 µs to +2.93 ms) | 79.2 MiB against 78.1 MiB: +1.08 MiB (+1.07 MiB to +1.09 MiB) |
| tally | 158, 158 in 60 s | means 175 ms against 172 ms: +3.16 ms (-756 µs to +7.09 ms) | 169 ms against 168 ms: +622 µs (-1.46 ms to +2.82 ms) | 10.6 MiB against 9.50 MiB: +1.08 MiB (+1.08 MiB to +1.09 MiB) |
| server | 21, 21 in 61 s | means 1.32 s against 1.32 s: +3.72 ms (-10.1 ms to +17.6 ms) | 1.33 s against 1.33 s: -387 µs (-10.2 ms to +10.8 ms) | 10.1 MiB against 9.12 MiB: +1.03 MiB (+1.01 MiB to +1.05 MiB) |
| files | 160, 160 in 56 s | means 162 ms against 159 ms: +2.44 ms (-1.39 ms to +6.27 ms) | 159 ms against 155 ms: +1.74 ms (+340 µs to +3.86 ms) | 10.4 MiB against 9.45 MiB: +988 KiB (+984 KiB to +992 KiB) |
| readings | 160, 160 in 37 s | means 105 ms against 104 ms: +586 µs (-2.19 ms to +3.36 ms) | 102 ms against 102 ms: +258 µs (-158 µs to +669 µs) | 10.3 MiB against 9.30 MiB: +984 KiB (+984 KiB to +988 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
