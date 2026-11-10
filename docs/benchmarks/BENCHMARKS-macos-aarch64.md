# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 076bcc1eee81607c6236fee34fc52418049e4a69, in https://github.com/laughing-hermit/lengua/actions/runs/42756074550. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (16.7 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ✗ ? ✗ ✓ | ✗ ? ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ? ? ✗ ✓ | ? ? ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| hypotenuse | ✗ ? ✗ ✓ | ? ? ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ? ? ✗ ✓ | ? ? ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ? ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ? ✗ ✓ | ✗ ? ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✓ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✓ ✓ | ? ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ? ✗ ✗ ✗ |
| files | ✓ ✓ ? ✓ | ✓ ✓ ? ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 empty rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 516 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase-stdin rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 588 names the row: take it out, as the list only shrinks
- macos-aarch64 upcase-stdin rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 589 names the row: take it out, as the list only shrinks
- macos-aarch64 shout rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 608 names the row: take it out, as the list only shrinks
- macos-aarch64 shout rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 609 names the row: take it out, as the list only shrinks
- macos-aarch64 wc rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 628 names the row: take it out, as the list only shrinks
- macos-aarch64 histogram rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 641 names the row: take it out, as the list only shrinks
- macos-aarch64 histogram rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 642 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 655 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 656 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 658 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 659 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 661 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 662 names the row: take it out, as the list only shrinks
- macos-aarch64 crc32c rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 663 names the row: take it out, as the list only shrinks
- macos-aarch64 sort cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 677 names the row: take it out, as the list only shrinks
- macos-aarch64 sort cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 678 names the row: take it out, as the list only shrinks
- macos-aarch64 sort rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 679 names the row: take it out, as the list only shrinks
- macos-aarch64 sort rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 680 names the row: take it out, as the list only shrinks
- macos-aarch64 sort c cpu: lengua is ahead now, and KNOWN_BEHIND line 681 names the row: take it out, as the list only shrinks
- macos-aarch64 sort c wall: lengua is ahead now, and KNOWN_BEHIND line 682 names the row: take it out, as the list only shrinks
- macos-aarch64 sort cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 684 names the row: take it out, as the list only shrinks
- macos-aarch64 sort cpp wall: lengua is ahead now, and KNOWN_BEHIND line 685 names the row: take it out, as the list only shrinks
- macos-aarch64 tally c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 690 names the row: take it out, as the list only shrinks
- macos-aarch64 tally c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 691 names the row: take it out, as the list only shrinks
- macos-aarch64 tally cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 693 names the row: take it out, as the list only shrinks
- macos-aarch64 tally cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 694 names the row: take it out, as the list only shrinks
- macos-aarch64 tally rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 696 names the row: take it out, as the list only shrinks
- macos-aarch64 tally rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 697 names the row: take it out, as the list only shrinks
- macos-aarch64 tally c cpu: lengua is ahead now, and KNOWN_BEHIND line 699 names the row: take it out, as the list only shrinks
- macos-aarch64 tally c wall: lengua is ahead now, and KNOWN_BEHIND line 700 names the row: take it out, as the list only shrinks
- macos-aarch64 tally cpp cpu: lengua is ahead now, and KNOWN_BEHIND line 703 names the row: take it out, as the list only shrinks
- macos-aarch64 tally cpp wall: lengua is ahead now, and KNOWN_BEHIND line 704 names the row: take it out, as the list only shrinks
- macos-aarch64 tally rust cpu: lengua is ahead now, and KNOWN_BEHIND line 707 names the row: take it out, as the list only shrinks
- macos-aarch64 tally rust wall: lengua is ahead now, and KNOWN_BEHIND line 708 names the row: take it out, as the list only shrinks
- macos-aarch64 server rust-wasm memory: lengua is ahead now, and KNOWN_BEHIND line 716 names the row: take it out, as the list only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 3.32× | 2.78× | 4.09× | 1042.45× |
| add | 2.34× | 1.95× | 4.00× | 520.88× |
| hypotenuse | 2.94× | 2.33× | 4.01× | 522.25× |
| upcase | 2.60× | 1.80× | 2.81× | 520.41× |
| upcase-stdin | 1.73× | 1.58× | 4.01× | 524.49× |
| shout | 2.45× | 1.92× | 2.76× | 523.12× |
| wc | 1.99× | 1.90× | 3.99× | 523.14× |
| histogram | 1.30× | 1.22× | 3.83× | 523.13× |
| crc32c | 2.48× | 2.38× | 3.89× | 523.13× |
| sort | 1.14× | 1.13× | 0.98× | 518.50× |
| tally | 0.47× | 0.47× | 3.14× | 517.05× |
| server | 1.27× | 3.03× | 4.30× | 516.00× |
| files | 1.58× | 1.56× | 4.51× | 518.92× |
| readings | 3.75× | 3.72× | 4.11× | 520.88× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | behind, 1.05×, p 7.4e-11 | undecided, 1.02×, p 0.010 | behind, 1.07× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | behind, 1.05×, p 3.2e-10 | undecided, 1.02×, p 0.009 | behind, 1.07× | ahead, 0.89× |
| empty | rust-wasm | 640 | ahead, 0.94×, p 2.9e-4 | ahead, 0.92×, p 1.6e-5 | ahead, 0.99× | ahead, 0.01× |
| empty | c | 20 | behind, 3.08×, p 6.2e-8 | behind, 2.60×, p 3.5e-7 | behind, 4.09× | behind, 1042.45× |
| empty | cpp | 20 | behind, 3.32×, p 1.3e-7 | behind, 2.78×, p 1.7e-7 | behind, 4.09× | behind, 1042.45× |
| empty | rust | 20 | behind, 2.48×, p 4.6e-8 | behind, 2.14×, p 1.5e-7 | behind, 3.89× | behind, 61.40× |
| add | c-wasm | 2000 | undecided, 1.00×, p 0.358 | undecided, 0.98×, p 0.002 | behind, 1.01× | ahead, 0.04× |
| add | cpp-wasm | 2000 | undecided, 1.03×, p 0.002 | undecided, 0.99×, p 0.016 | behind, 1.02× | ahead, 0.04× |
| add | rust-wasm | 640 | ahead, 0.95×, p 2.2e-4 | ahead, 0.91×, p 1.7e-10 | ahead, 0.96× | ahead, 0.02× |
| add | c | 20 | behind, 2.34×, p 3.9e-8 | behind, 1.95×, p 9.6e-8 | behind, 4.00× | behind, 520.88× |
| add | cpp | 0 | undecided | undecided | undecided | undecided |
| add | rust | 20 | behind, 1.65×, p 1.5e-7 | behind, 1.41×, p 5.5e-6 | behind, 3.77× | behind, 58.02× |
| hypotenuse | c-wasm | 2000 | behind, 1.02×, p 4.0e-6 | undecided, 0.98×, p 0.109 | behind, 1.03× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | undecided, 1.01×, p 0.053 | undecided, 0.98×, p 0.008 | behind, 1.01× | ahead, 0.05× |
| hypotenuse | rust-wasm | 640 | ahead, 0.94×, p 1.5e-4 | ahead, 0.92×, p 8.3e-7 | ahead, 0.94× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.94×, p 9.6e-8 | behind, 2.33×, p 2.3e-7 | behind, 4.01× | behind, 522.25× |
| hypotenuse | cpp | 20 | behind, 2.68×, p 3.4e-8 | behind, 2.05×, p 3.9e-8 | behind, 4.01× | behind, 521.51× |
| hypotenuse | rust | 20 | behind, 2.12×, p 9.6e-8 | behind, 1.75×, p 1.0e-6 | behind, 3.78× | behind, 58.02× |
| upcase | c-wasm | 2000 | undecided, 1.01×, p 0.167 | undecided, 0.99×, p 0.139 | behind, 1.01× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | undecided, 1.02×, p 0.016 | undecided, 1.00×, p 0.311 | behind, 1.01× | ahead, 0.06× |
| upcase | rust-wasm | 160 | ahead, 0.87×, p 2.1e-8 | ahead, 0.88×, p 1.7e-5 | ahead, 0.95× | ahead, 0.03× |
| upcase | c | 20 | behind, 2.46×, p 3.0e-7 | behind, 1.80×, p 1.3e-6 | behind, 2.79× | behind, 520.41× |
| upcase | cpp | 20 | behind, 2.60×, p 3.3e-6 | behind, 1.74×, p 1.2e-5 | behind, 2.81× | behind, 520.41× |
| upcase | rust | 20 | behind, 1.52×, p 1.9e-5 | behind, 1.25×, p 3.1e-4 | behind, 2.66× | behind, 58.02× |
| upcase-stdin | c-wasm | 640 | ahead, 0.96×, p 5.1e-6 | ahead, 0.94×, p 1.4e-9 | behind, 1.07× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 80 | ahead, 0.90×, p 3.6e-5 | ahead, 0.89×, p 5.3e-6 | behind, 1.07× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | undecided, 0.99×, p 0.140 | ahead, 0.98×, p 1.5e-4 | ahead, 0.99× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.73×, p 2.3e-7 | behind, 1.58×, p 3.5e-7 | behind, 3.96× | behind, 524.49× |
| upcase-stdin | cpp | 20 | behind, 1.71×, p 2.0e-6 | behind, 1.56×, p 1.0e-5 | behind, 4.01× | behind, 524.49× |
| upcase-stdin | rust | 20 | behind, 1.66×, p 3.4e-8 | behind, 1.49×, p 1.1e-7 | behind, 3.66× | behind, 58.03× |
| shout | c-wasm | 2000 | behind, 1.07×, p 4.3e-29 | undecided, 1.01×, p 0.016 | behind, 1.06× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | behind, 1.07×, p 1.6e-31 | undecided, 1.01×, p 0.005 | behind, 1.06× | ahead, 0.40× |
| shout | rust-wasm | 80 | ahead, 0.91×, p 1.0e-5 | ahead, 0.94×, p 3.1e-4 | ahead, 0.98× | ahead, 0.03× |
| shout | c | 20 | behind, 2.45×, p 1.7e-6 | behind, 1.92×, p 4.9e-6 | behind, 2.74× | behind, 523.12× |
| shout | cpp | 20 | behind, 2.32×, p 9.0e-7 | behind, 1.78×, p 1.5e-5 | behind, 2.76× | behind, 523.12× |
| shout | rust | 20 | behind, 1.82×, p 6.9e-7 | behind, 1.51×, p 1.6e-5 | behind, 2.61× | behind, 58.03× |
| wc | c-wasm | 40 | behind, 1.18×, p 1.7e-4 | behind, 1.14×, p 1.8e-4 | behind, 1.05× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 3.4e-8 | ahead, 0.16×, p 3.4e-8 | behind, 1.05× | ahead, 0.11× |
| wc | rust-wasm | 40 | behind, 1.21×, p 2.2e-7 | behind, 1.19×, p 4.5e-7 | ahead, 0.97× | ahead, 0.04× |
| wc | c | 20 | behind, 1.95×, p 3.9e-8 | behind, 1.85×, p 8.3e-8 | behind, 3.99× | behind, 523.14× |
| wc | cpp | 20 | ahead, 0.21×, p 3.4e-8 | ahead, 0.20×, p 3.4e-8 | behind, 3.99× | behind, 523.14× |
| wc | rust | 20 | behind, 1.99×, p 6.2e-8 | behind, 1.90×, p 8.3e-8 | behind, 3.68× | behind, 58.03× |
| histogram | c-wasm | 20 | ahead, 0.66×, p 3.3e-6 | ahead, 0.63×, p 1.3e-6 | behind, 1.03× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 2.9e-6 | ahead, 0.64×, p 1.3e-6 | behind, 1.03× | ahead, 0.09× |
| histogram | rust-wasm | 320 | ahead, 0.97×, p 3.9e-5 | ahead, 0.96×, p 5.5e-6 | ahead, 0.95× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.24×, p 1.3e-6 | behind, 1.19×, p 2.3e-5 | behind, 3.83× | behind, 523.13× |
| histogram | cpp | 20 | behind, 1.26×, p 1.6e-5 | behind, 1.22×, p 6.5e-5 | behind, 3.83× | behind, 523.00× |
| histogram | rust | 20 | behind, 1.30×, p 4.3e-6 | behind, 1.21×, p 9.0e-5 | behind, 3.62× | behind, 58.02× |
| crc32c | c-wasm | 20 | ahead, 0.69×, p 2.6e-5 | ahead, 0.70×, p 2.6e-5 | behind, 1.03× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.68×, p 1.5e-5 | ahead, 0.68×, p 1.2e-5 | behind, 1.03× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.67×, p 7.1e-6 | ahead, 0.68×, p 7.1e-6 | ahead, 0.97× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.40×, p 3.4e-8 | behind, 2.30×, p 3.4e-8 | behind, 3.89× | behind, 523.13× |
| crc32c | cpp | 20 | behind, 2.48×, p 3.4e-8 | behind, 2.38×, p 3.4e-8 | behind, 3.89× | behind, 523.13× |
| crc32c | rust | 20 | behind, 2.39×, p 3.4e-8 | behind, 2.29×, p 3.4e-8 | behind, 3.64× | behind, 58.03× |
| sort | c-wasm | 20 | ahead, 0.15×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | behind, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.56×, p 3.4e-8 | ahead, 0.56×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.63×, p 8.3e-8 | ahead, 0.63×, p 1.1e-7 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.63×, p 6.2e-8 | ahead, 0.62×, p 6.2e-8 | ahead, 0.98× | behind, 518.50× |
| sort | cpp | 20 | ahead, 0.72×, p 1.2e-6 | ahead, 0.72×, p 1.3e-6 | ahead, 0.81× | behind, 503.05× |
| sort | rust | 40 | behind, 1.14×, p 7.1e-5 | behind, 1.13×, p 1.6e-4 | ahead, 0.95× | behind, 58.04× |
| tally | c-wasm | 20 | ahead, 0.40×, p 3.4e-8 | ahead, 0.40×, p 3.4e-8 | behind, 1.08× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 1.04× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.33×, p 3.4e-8 | ahead, 0.33×, p 3.4e-8 | behind, 1.05× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.47×, p 3.4e-8 | ahead, 0.47×, p 3.4e-8 | behind, 3.14× | behind, 517.05× |
| tally | cpp | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.44×, p 3.4e-8 | behind, 3.14× | behind, 501.00× |
| tally | rust | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 2.91× | behind, 55.02× |
| server | c-wasm | 38 | undecided, 1.03×, p 0.404 | ahead, 0.56×, p 4.0e-10 | behind, 1.05× | ahead, 0.12× |
| server | cpp-wasm | 38 | undecided, 1.03×, p 0.356 | ahead, 0.60×, p 6.3e-10 | behind, 1.03× | ahead, 0.06× |
| server | rust-wasm | 38 | undecided, 1.05×, p 0.452 | ahead, 0.65×, p 1.8e-7 | ahead, 0.97× | ahead, 0.03× |
| server | c | 38 | undecided, 1.23×, p 0.002 | behind, 2.64×, p 1.4e-10 | behind, 4.30× | behind, 516.00× |
| server | cpp | 38 | behind, 1.27×, p 1.7e-4 | behind, 2.71×, p 6.6e-11 | behind, 4.25× | behind, 498.87× |
| server | rust | 38 | undecided, 0.91×, p 0.044 | behind, 3.03×, p 5.4e-11 | behind, 2.04× | behind, 54.88× |
| files | c-wasm | 80 | ahead, 0.95×, p 2.0e-4 | ahead, 0.95×, p 8.8e-5 | undecided, 1.00× | ahead, 0.05× |
| files | cpp-wasm | 80 | ahead, 0.96×, p 8.9e-4 | ahead, 0.96×, p 4.9e-4 | undecided, 1.00× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 0.94×, p 6.3e-6 | ahead, 0.94×, p 3.0e-6 | ahead, 0.94× | ahead, 0.03× |
| files | c | 20 | behind, 1.53×, p 1.7e-7 | behind, 1.51×, p 1.7e-7 | behind, 4.45× | behind, 518.92× |
| files | cpp | 20 | behind, 1.53×, p 4.6e-7 | behind, 1.51×, p 7.9e-7 | behind, 4.51× | behind, 518.92× |
| files | rust | 20 | behind, 1.58×, p 1.1e-7 | behind, 1.56×, p 1.5e-7 | behind, 3.94× | behind, 58.02× |
| readings | c-wasm | 40 | ahead, 0.92×, p 6.4e-5 | ahead, 0.93×, p 1.9e-4 | behind, 1.02× | ahead, 0.04× |
| readings | cpp-wasm | 160 | ahead, 0.96×, p 8.9e-5 | ahead, 0.96×, p 6.5e-5 | behind, 1.03× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.93×, p 1.5e-5 | ahead, 0.92×, p 4.6e-4 | ahead, 0.95× | ahead, 0.02× |
| readings | c | 20 | behind, 2.89×, p 3.4e-8 | behind, 2.87×, p 3.4e-8 | behind, 4.11× | behind, 520.88× |
| readings | cpp | 0 | undecided | undecided | undecided | undecided |
| readings | rust | 20 | behind, 3.75×, p 3.4e-8 | behind, 3.72×, p 3.4e-8 | behind, 3.87× | behind, 58.01× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 20 | 20 |
| CPU time | 6.31 ms | 6.00 ms | 6.02 ms | 7.60 ms | 2.21 ms | 2.04 ms | 2.74 ms |
| wall time | 6.70 ms | 6.56 ms | 6.54 ms | 8.38 ms | 2.80 ms | 2.62 ms | 3.41 ms |
| peak memory | 5.00 MiB | 4.67 MiB | 4.67 MiB | 5.03 MiB | 1.22 MiB | 1.22 MiB | 1.28 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 94 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 0 | 20 |
| CPU time | 6.43 ms | 6.43 ms | 6.26 ms | 6.18 ms | 2.73 ms | – | 3.88 ms |
| wall time | 6.63 ms | 6.77 ms | 6.72 ms | 6.54 ms | 3.35 ms | – | 4.64 ms |
| peak memory | 5.08 MiB | 5.02 MiB | 4.97 MiB | 5.30 MiB | 1.27 MiB | – | 1.34 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 0 B | 295 KiB |

Its rounds took 90 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp wasn't measured to the end: c++ couldn't build /Users/runner/work/lengua/lengua/benches/programs/add/main.cpp: /Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:10:6: error: no template named 'optional' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      | ~~~~~^
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:12:10: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |          ^~~~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:12:30: error: no member named 'from_chars' in namespace 'std'
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |                              ^~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:13:23: error: no member named 'errc' in namespace 'std'; did you mean 'erfc'?
   13 |     if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
      |                       ^~~~
      |                       erfc
/Applications/Xcode_26.6.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/usr/include/c++/v1/cmath:438:9: note: 'erfc' declared here
  438 | using ::erfc _LIBCPP_USING_IF_EXISTS;
      |         ^
/Users/runner/work/lengua/lengua/benches/programs/add/main.cpp:15:21: error: no member named 'nullopt' in namespace 'std'
   15 |         return std::nullopt;
      |                     ^~~~~~~
1 warning and 4 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 640 | 20 | 20 | 20 |
| CPU time | 6.96 ms | 6.79 ms | 6.87 ms | 7.67 ms | 2.20 ms | 2.41 ms | 3.04 ms |
| wall time | 7.10 ms | 7.28 ms | 7.24 ms | 8.11 ms | 2.78 ms | 3.15 ms | 3.70 ms |
| peak memory | 5.08 MiB | 4.94 MiB | 5.02 MiB | 5.39 MiB | 1.27 MiB | 1.27 MiB | 1.34 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 96 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 160 | 20 | 20 | 20 |
| CPU time | 9.88 ms | 9.83 ms | 9.65 ms | 11.9 ms | 4.47 ms | 4.22 ms | 7.25 ms |
| wall time | 10.0 ms | 10.1 ms | 10.0 ms | 11.8 ms | 5.85 ms | 6.05 ms | 8.40 ms |
| peak memory | 6.28 MiB | 6.20 MiB | 6.20 MiB | 6.62 MiB | 2.25 MiB | 2.23 MiB | 2.36 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 114 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 640 | 80 | 2000 | 20 | 20 | 20 |
| CPU time | 16.6 ms | 18.5 ms | 21.1 ms | 16.7 ms | 11.5 ms | 11.6 ms | 12.0 ms |
| wall time | 16.0 ms | 18.2 ms | 21.4 ms | 16.4 ms | 12.3 ms | 12.4 ms | 13.0 ms |
| peak memory | 5.14 MiB | 4.80 MiB | 4.80 MiB | 5.22 MiB | 1.30 MiB | 1.28 MiB | 1.41 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 110 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 80 | 20 | 20 | 20 |
| CPU time | 4.50 ms | 4.20 ms | 4.19 ms | 4.27 ms | 3.17 ms | 3.35 ms | 4.27 ms |
| wall time | 4.54 ms | 4.48 ms | 4.48 ms | 4.19 ms | 3.91 ms | 4.23 ms | 4.98 ms |
| peak memory | 6.14 MiB | 5.78 MiB | 5.78 MiB | 6.23 MiB | 2.23 MiB | 2.22 MiB | 2.34 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 57 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 40 | 20 | 20 | 20 |
| CPU time | 30.8 ms | 26.1 ms | 212 ms | 25.4 ms | 18.0 ms | 168 ms | 17.6 ms |
| wall time | 29.7 ms | 26.0 ms | 212 ms | 24.9 ms | 18.7 ms | 169 ms | 18.2 ms |
| peak memory | 5.23 MiB | 5.00 MiB | 5.00 MiB | 5.42 MiB | 1.31 MiB | 1.31 MiB | 1.42 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 15 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 20 | 20 | 320 | 20 | 20 | 20 |
| CPU time | 36.0 ms | 58.9 ms | 58.3 ms | 37.1 ms | 31.3 ms | 30.7 ms | 29.8 ms |
| wall time | 35.6 ms | 61.6 ms | 60.4 ms | 37.1 ms | 32.5 ms | 31.7 ms | 32.0 ms |
| peak memory | 5.16 MiB | 5.00 MiB | 5.00 MiB | 5.45 MiB | 1.34 MiB | 1.34 MiB | 1.42 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 35 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 52.8 ms | 77.0 ms | 77.6 ms | 78.3 ms | 22.0 ms | 21.3 ms | 22.1 ms |
| wall time | 52.6 ms | 75.3 ms | 77.6 ms | 77.8 ms | 22.8 ms | 22.1 ms | 23.0 ms |
| peak memory | 5.17 MiB | 5.00 MiB | 5.00 MiB | 5.31 MiB | 1.33 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 10 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=apple-m1

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=apple-m1

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 20 | 20 | 20 | 40 |
| CPU time | 211 ms | 1.46 s | 395 ms | 350 ms | 351 ms | 305 ms | 185 ms |
| wall time | 211 ms | 1.46 s | 395 ms | 350 ms | 352 ms | 306 ms | 186 ms |
| peak memory | 74.8 MiB | 74.4 MiB | 82.6 MiB | 82.8 MiB | 76.3 MiB | 92.4 MiB | 78.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 90 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 148 ms | 368 ms | 348 ms | 453 ms | 317 ms | 330 ms | 341 ms |
| wall time | 148 ms | 370 ms | 348 ms | 453 ms | 318 ms | 337 ms | 343 ms |
| peak memory | 6.14 MiB | 5.70 MiB | 5.90 MiB | 5.84 MiB | 1.95 MiB | 1.95 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 53 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 38 | 38 | 38 | 38 | 38 | 38 | 38 |
| CPU time | 602 ms | 587 ms | 584 ms | 575 ms | 490 ms | 473 ms | 665 ms |
| wall time | 2.13 s | 3.80 s | 3.53 s | 3.28 s | 806 ms | 787 ms | 702 ms |
| peak memory | 5.31 MiB | 5.06 MiB | 5.17 MiB | 5.48 MiB | 1.23 MiB | 1.25 MiB | 2.61 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 645 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 80 | 80 | 80 | 20 | 20 | 20 |
| CPU time | 74.6 ms | 78.6 ms | 77.8 ms | 79.0 ms | 48.6 ms | 48.5 ms | 47.0 ms |
| wall time | 74.4 ms | 78.5 ms | 77.8 ms | 79.1 ms | 49.1 ms | 49.0 ms | 47.5 ms |
| peak memory | 5.83 MiB | 5.83 MiB | 5.83 MiB | 6.23 MiB | 1.31 MiB | 1.30 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 32 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 40 | 160 | 20 | 20 | 0 | 20 |
| CPU time | 104 ms | 111 ms | 108 ms | 108 ms | 34.8 ms | – | 26.9 ms |
| wall time | 105 ms | 113 ms | 109 ms | 110 ms | 35.3 ms | – | 27.3 ms |
| peak memory | 5.14 MiB | 5.02 MiB | 4.97 MiB | 5.41 MiB | 1.25 MiB | – | 1.33 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 0 B | 296 KiB |

Its rounds took 46 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp wasn't measured to the end: c++ couldn't build /Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp: /Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp:15:14: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |              ^~~~~~~~~~~~
/Users/runner/work/lengua/lengua/benches/programs/readings/main.cpp:15:34: error: no member named 'from_chars' in namespace 'std'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                  ^~~~~~~~~~
1 warning and 1 error generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 5 s | 6.85 ms against 6.19 ms: +554 µs (+246 µs to +874 µs) | 7.23 ms against 6.94 ms: +331 µs (-42.1 µs to +702 µs) | 5.02 MiB against 4.67 MiB: +336 KiB (+336 KiB to +352 KiB) |
| add | 160, 160 in 5 s | 7.71 ms against 7.45 ms: +404 µs (-47.0 µs to +833 µs) | 7.77 ms against 7.94 ms: +38.9 µs (-442 µs to +520 µs) | 5.08 MiB against 4.72 MiB: +368 KiB (+352 KiB to +368 KiB) |
| hypotenuse | 160, 160 in 5 s | 6.64 ms against 6.57 ms: +402 µs (+7.00 µs to +755 µs) | 6.79 ms against 7.01 ms: +35.1 µs (-430 µs to +426 µs) | 5.08 MiB against 4.72 MiB: +352 KiB (+352 KiB to +368 KiB) |
| upcase | 160, 160 in 5 s | 9.14 ms against 8.32 ms: +871 µs (+379 µs to +1.38 ms) | 9.05 ms against 8.75 ms: +444 µs (-28.2 µs to +961 µs) | 6.28 MiB against 5.92 MiB: +384 KiB (+368 KiB to +384 KiB) |
| upcase-stdin | 160, 160 in 8 s | 18.8 ms against 18.6 ms: +895 µs (+150 µs to +1.59 ms) | 18.3 ms against 18.4 ms: +600 µs (-128 µs to +1.33 ms) | 5.14 MiB against 4.78 MiB: +368 KiB (+368 KiB to +368 KiB) |
| shout | 160, 160 in 5 s | 8.76 ms against 8.53 ms: +557 µs (-14.0 µs to +1.12 ms) | 9.02 ms against 8.96 ms: +182 µs (-396 µs to +763 µs) | 6.14 MiB against 5.77 MiB: +384 KiB (+384 KiB to +384 KiB) |
| wc | 160, 160 in 11 s | 28.1 ms against 27.4 ms: +356 µs (-298 µs to +972 µs) | 27.5 ms against 27.3 ms: +49.8 µs (-580 µs to +655 µs) | 5.23 MiB against 4.84 MiB: +400 KiB (+400 KiB to +400 KiB) |
| histogram | 160, 160 in 15 s | 41.8 ms against 40.9 ms: +590 µs (-678 µs to +1.79 ms) | 41.4 ms against 40.4 ms: +388 µs (-901 µs to +1.63 ms) | 5.16 MiB against 4.78 MiB: +384 KiB (+368 KiB to +384 KiB) |
| crc32c | 160, 160 in 18 s | 48.0 ms against 48.2 ms: -679 µs (-2.37 ms to +1.03 ms) | 48.5 ms against 49.4 ms: -1.23 ms (-3.01 ms to +540 µs) | 5.17 MiB against 4.80 MiB: +384 KiB (+384 KiB to +384 KiB) |
| sort | 132, 132 in 60 s | 219 ms against 221 ms: -1.85 ms (-8.15 ms to +3.91 ms) | 217 ms against 220 ms: -2.10 ms (-8.54 ms to +3.91 ms) | 74.8 MiB against 74.4 MiB: +400 KiB (+400 KiB to +416 KiB) |
| tally | 160, 160 in 43 s | 127 ms against 126 ms: +818 µs (-2.62 ms to +4.21 ms) | 127 ms against 126 ms: +441 µs (-3.02 ms to +3.91 ms) | 6.14 MiB against 5.73 MiB: +432 KiB (+416 KiB to +432 KiB) |
| server | 20, 20 in 74 s | 496 ms against 512 ms: -3.77 ms (-66.7 ms to +52.4 ms) | 1.74 s against 1.87 s: -102 ms (-634 ms to +334 ms) | 5.31 MiB against 4.94 MiB: +384 KiB (+384 KiB to +400 KiB) |
| files | 160, 160 in 24 s | 70.6 ms against 70.4 ms: +334 µs (-57.0 µs to +730 µs) | 70.3 ms against 70.4 ms: +70.1 µs (-324 µs to +464 µs) | 5.83 MiB against 5.47 MiB: +368 KiB (+368 KiB to +368 KiB) |
| readings | 160, 160 in 37 s | 105 ms against 106 ms: -230 µs (-1.75 ms to +1.27 ms) | 105 ms against 106 ms: -542 µs (-2.12 ms to +1.01 ms) | 5.14 MiB against 4.77 MiB: +368 KiB (+368 KiB to +368 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
