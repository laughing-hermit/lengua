# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (AMD EPYC 9V74 80-Core Processor, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 076bcc1eee81607c6236fee34fc52418049e4a69, in https://github.com/laughing-hermit/lengua/actions/runs/42756074550. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-mcp run`, with every check it makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, in the same engine (wasmtime 50.0.1) with the same cache of compiled modules; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-mcp. Against a native build, lengua's size is its module's and that of the lengua-mcp it needs to run (25.0 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured. A measure the runs don't decide is undecided, never level, and KNOWN_BEHIND must name its row.

The versions:

- lengua: lengua-mcp, run in wasmtime 50.0.1
- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead, ✗ behind, ? undecided, – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ? ✗ ✗ ✓ | ? ✗ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| hypotenuse | ? ✓ ✗ ✓ | ? ? ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |
| upcase | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ? ✓ ✗ ✗ | ? ✓ ✗ ✗ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ✗ ✗ ✗ ✓ | ? ✗ ✗ ✓ | ✗ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ✗ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ? ? ? ? | ✓ ✓ ✗ ✗ |
| server | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ? ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✓ | ✗ ✗ ✗ ✗ | ? ? ? ? | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 empty rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 765 names the row: take it out, as the list only shrinks
- windows-x86_64 add c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 780 names the row: take it out, as the list only shrinks
- windows-x86_64 add cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 783 names the row: take it out, as the list only shrinks
- windows-x86_64 add rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 786 names the row: take it out, as the list only shrinks
- windows-x86_64 hypotenuse c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 801 names the row: take it out, as the list only shrinks
- windows-x86_64 hypotenuse rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 807 names the row: take it out, as the list only shrinks
- windows-x86_64 upcase c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 822 names the row: take it out, as the list only shrinks
- windows-x86_64 upcase cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 825 names the row: take it out, as the list only shrinks
- windows-x86_64 upcase rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 828 names the row: take it out, as the list only shrinks
- windows-x86_64 upcase-stdin c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 843 names the row: take it out, as the list only shrinks
- windows-x86_64 upcase-stdin cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 847 names the row: take it out, as the list only shrinks
- windows-x86_64 shout rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 871 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 899 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 900 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 902 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 903 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 905 names the row: take it out, as the list only shrinks
- windows-x86_64 histogram rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 906 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 920 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 921 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 923 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 924 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 926 names the row: take it out, as the list only shrinks
- windows-x86_64 crc32c rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 927 names the row: take it out, as the list only shrinks
- windows-x86_64 sort cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 942 names the row: take it out, as the list only shrinks
- windows-x86_64 sort cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 943 names the row: take it out, as the list only shrinks
- windows-x86_64 sort rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 944 names the row: take it out, as the list only shrinks
- windows-x86_64 sort rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 945 names the row: take it out, as the list only shrinks
- windows-x86_64 sort c cpu: lengua is ahead now, and KNOWN_BEHIND line 946 names the row: take it out, as the list only shrinks
- windows-x86_64 sort c wall: lengua is ahead now, and KNOWN_BEHIND line 947 names the row: take it out, as the list only shrinks
- windows-x86_64 tally c-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 957 names the row: take it out, as the list only shrinks
- windows-x86_64 tally c-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 958 names the row: take it out, as the list only shrinks
- windows-x86_64 tally cpp-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 960 names the row: take it out, as the list only shrinks
- windows-x86_64 tally cpp-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 961 names the row: take it out, as the list only shrinks
- windows-x86_64 tally rust-wasm cpu: lengua is ahead now, and KNOWN_BEHIND line 963 names the row: take it out, as the list only shrinks
- windows-x86_64 tally rust-wasm wall: lengua is ahead now, and KNOWN_BEHIND line 964 names the row: take it out, as the list only shrinks
- windows-x86_64 tally c cpu: lengua is ahead now, and KNOWN_BEHIND line 966 names the row: take it out, as the list only shrinks
- windows-x86_64 tally c wall: lengua is ahead now, and KNOWN_BEHIND line 967 names the row: take it out, as the list only shrinks
- windows-x86_64 tally rust cpu: lengua is ahead now, and KNOWN_BEHIND line 974 names the row: take it out, as the list only shrinks
- windows-x86_64 tally rust wall: lengua is ahead now, and KNOWN_BEHIND line 975 names the row: take it out, as the list only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-mcp's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 2.76× | 3.03× | 245.38× |
| add | – | 2.70× | 3.00× | 214.59× |
| hypotenuse | – | 2.68× | 3.00× | 211.93× |
| upcase | – | 2.35× | 2.41× | 205.98× |
| upcase-stdin | 2.00× | 1.69× | 2.99× | 235.26× |
| shout | – | 2.46× | 2.43× | 225.93× |
| wc | 1.33× | 1.32× | 2.98× | 214.59× |
| histogram | 1.50× | 1.26× | 2.96× | 215.49× |
| crc32c | 3.00× | 2.22× | 2.96× | 228.96× |
| sort | 1.04× | 1.04× | 1.01× | 213.72× |
| tally | 0.53× | 0.52× | 2.65× | 199.58× |
| server | 1.36× | 2.42× | 2.54× | 218.25× |
| files | 1.80× | 1.65× | 2.91× | 205.97× |
| readings | 3.00× | 3.93× | 3.01× | 206.80× |

## Every row

Each measure's verdict, lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | undecided, 1.00×, p 0.144 | behind, 1.01×, p 9.3e-6 | behind, 1.12× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | undecided, 1.00×, p 0.316 | behind, 1.01×, p 1.9e-5 | behind, 1.12× | ahead, 0.89× |
| empty | rust-wasm | 2000 | undecided, 1.00×, p 0.213 | ahead, 0.98×, p 2.5e-16 | behind, 1.11× | ahead, 0.01× |
| empty | c | 20 | behind, p 5.2e-4 | behind, 2.66×, p 3.4e-8 | behind, 3.03× | behind, 228.95× |
| empty | cpp | 20 | behind, p 1.7e-4 | behind, 2.76×, p 3.4e-8 | behind, 3.03× | behind, 228.95× |
| empty | rust | 40 | behind, p 1.6e-4 | behind, 2.44×, p 7.2e-15 | behind, 2.73× | behind, 245.38× |
| add | c-wasm | 2000 | undecided, 1.00×, p 0.303 | ahead, 0.98×, p 4.3e-16 | behind, 1.11× | ahead, 0.04× |
| add | cpp-wasm | 2000 | undecided, 1.00×, p 0.473 | ahead, 0.99×, p 3.0e-8 | behind, 1.11× | ahead, 0.04× |
| add | rust-wasm | 2000 | undecided, 1.00×, p 0.130 | ahead, 0.96×, p 1.4e-76 | behind, 1.11× | ahead, 0.02× |
| add | c | 20 | behind, p 2.3e-4 | behind, 2.70×, p 3.4e-8 | behind, 3.00× | behind, 173.85× |
| add | cpp | 0 | undecided | undecided | undecided | undecided |
| add | rust | 20 | behind, p 0.001 | behind, 2.42×, p 3.4e-8 | behind, 2.72× | behind, 214.59× |
| hypotenuse | c-wasm | 2000 | undecided, 1.00×, p 0.365 | ahead, 0.99×, p 1.7e-4 | behind, 1.11× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | undecided, 1.00×, p 0.484 | undecided, 1.00×, p 0.143 | behind, 1.12× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | undecided, 1.00×, p 0.255 | ahead, 0.97×, p 4.4e-73 | behind, 1.11× | ahead, 0.03× |
| hypotenuse | c | 40 | behind, p 2.6e-5 | behind, 2.68×, p 7.2e-15 | behind, 3.00× | behind, 172.68× |
| hypotenuse | cpp | 0 | undecided | undecided | undecided | undecided |
| hypotenuse | rust | 40 | behind, p 1.1e-5 | behind, 2.44×, p 7.2e-15 | behind, 2.71× | behind, 211.93× |
| upcase | c-wasm | 2000 | undecided, 1.00×, p 0.209 | ahead, 0.99×, p 1.0e-9 | behind, 1.09× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | undecided, 1.00×, p 0.059 | ahead, 0.99×, p 1.9e-9 | behind, 1.09× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | undecided, 1.00×, p 0.019 | ahead, 0.97×, p 4.8e-36 | behind, 1.08× | ahead, 0.03× |
| upcase | c | 20 | behind, p 4.5e-4 | behind, 2.32×, p 3.4e-8 | behind, 2.37× | behind, 163.34× |
| upcase | cpp | 40 | behind, p 2.4e-4 | behind, 2.35×, p 7.2e-15 | behind, 2.37× | behind, 163.34× |
| upcase | rust | 20 | behind, p 4.5e-4 | behind, 2.21×, p 3.4e-8 | behind, 2.41× | behind, 205.98× |
| upcase-stdin | c-wasm | 2000 | undecided, 1.00×, p 0.043 | ahead, 0.98×, p 1.4e-61 | behind, 1.11× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | undecided, 1.00×, p 0.330 | ahead, 0.98×, p 1.9e-106 | behind, 1.11× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | undecided, 1.00×, p 0.089 | ahead, 0.98×, p 3.6e-123 | behind, 1.11× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 4.5e-7 | behind, 1.68×, p 3.4e-8 | behind, 2.99× | behind, 220.11× |
| upcase-stdin | cpp | 20 | behind, 2.00×, p 2.1e-8 | behind, 1.69×, p 3.4e-8 | behind, 2.99× | behind, 220.11× |
| upcase-stdin | rust | 20 | behind, 2.00×, p 1.7e-6 | behind, 1.54×, p 3.4e-8 | behind, 2.70× | behind, 235.26× |
| shout | c-wasm | 1280 | behind, 1.00×, p 9.3e-4 | behind, 1.01×, p 1.7e-9 | behind, 1.11× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | undecided, 1.00×, p 0.022 | behind, 1.01×, p 1.4e-14 | behind, 1.11× | ahead, 0.40× |
| shout | rust-wasm | 320 | behind, p 1.6e-4 | ahead, 0.98×, p 1.1e-20 | behind, 1.09× | ahead, 0.03× |
| shout | c | 40 | behind, p 7.8e-6 | behind, 2.46×, p 7.2e-15 | behind, 2.41× | behind, 215.49× |
| shout | cpp | 40 | behind, p 2.3e-5 | behind, 2.46×, p 7.2e-15 | behind, 2.41× | behind, 215.49× |
| shout | rust | 40 | behind, p 1.6e-4 | behind, 2.26×, p 7.2e-15 | behind, 2.43× | behind, 225.93× |
| wc | c-wasm | 640 | ahead, 1.00×, p 6.8e-5 | ahead, 0.95×, p 7.8e-173 | behind, 1.11× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.17×, p 1.2e-8 | ahead, 0.18×, p 3.4e-8 | behind, 1.11× | ahead, 0.11× |
| wc | rust-wasm | 320 | ahead, 1.00×, p 4.4e-4 | ahead, 0.93×, p 4.6e-94 | behind, 1.11× | ahead, 0.04× |
| wc | c | 20 | behind, 1.00×, p 2.2e-4 | behind, 1.32×, p 3.4e-8 | behind, 2.98× | behind, 171.53× |
| wc | cpp | 2000 | undecided, 1.00×, p 0.003 | ahead, 0.98×, p 6.8e-215 | behind, 2.98× | behind, 170.96× |
| wc | rust | 20 | behind, 1.33×, p 1.5e-4 | behind, 1.31×, p 5.2e-7 | behind, 2.69× | behind, 214.59× |
| histogram | c-wasm | 20 | ahead, 1.00×, p 7.3e-5 | ahead, 0.82×, p 3.4e-8 | behind, 1.11× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 1.00×, p 3.4e-4 | ahead, 0.83×, p 3.4e-8 | behind, 1.11× | ahead, 0.09× |
| histogram | rust-wasm | 20 | ahead, 1.00×, p 3.5e-4 | ahead, 0.87×, p 3.4e-8 | behind, 1.10× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.50×, p 7.9e-4 | behind, 1.26×, p 3.4e-8 | behind, 2.96× | behind, 173.27× |
| histogram | cpp | 20 | behind, 1.50×, p 9.9e-4 | behind, 1.26×, p 6.0e-7 | behind, 2.96× | behind, 173.27× |
| histogram | rust | 40 | behind, 1.50×, p 4.6e-4 | behind, 1.18×, p 7.2e-15 | behind, 2.69× | behind, 215.49× |
| crc32c | c-wasm | 40 | ahead, 0.75×, p 7.4e-6 | ahead, 0.81×, p 1.2e-13 | behind, 1.11× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | ahead, 0.75×, p 2.4e-4 | ahead, 0.81×, p 1.3e-13 | behind, 1.11× | ahead, 0.08× |
| crc32c | rust-wasm | 40 | ahead, 0.75×, p 1.2e-5 | ahead, 0.81×, p 1.2e-13 | behind, 1.10× | ahead, 0.04× |
| crc32c | c | 20 | behind, 3.00×, p 1.2e-8 | behind, 2.22×, p 3.4e-8 | behind, 2.96× | behind, 173.27× |
| crc32c | cpp | 20 | behind, 3.00×, p 1.1e-8 | behind, 2.22×, p 3.4e-8 | behind, 2.96× | behind, 173.27× |
| crc32c | rust | 20 | behind, 3.00×, p 1.1e-8 | behind, 2.09×, p 3.4e-8 | behind, 2.68× | behind, 228.96× |
| sort | c-wasm | 20 | ahead, 0.20×, p 2.1e-8 | ahead, 0.20×, p 3.4e-8 | behind, 1.01× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.54×, p 2.3e-8 | ahead, 0.56×, p 3.4e-8 | ahead, 0.92× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.58×, p 1.9e-8 | ahead, 0.60×, p 3.4e-8 | ahead, 0.92× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.42×, p 2.1e-8 | ahead, 0.42×, p 3.4e-8 | behind, 1.01× | behind, 213.72× |
| sort | cpp | 0 | undecided | undecided | undecided | undecided |
| sort | rust | 40 | behind, 1.04×, p 1.6e-4 | behind, 1.04×, p 1.1e-12 | ahead, 0.94× | behind, 212.83× |
| tally | c-wasm | 20 | ahead, 0.42×, p 1.0e-8 | ahead, 0.39×, p 3.4e-8 | behind, 1.13× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.42×, p 1.4e-8 | ahead, 0.39×, p 3.4e-8 | behind, 1.13× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.35×, p 1.7e-8 | ahead, 0.32×, p 3.4e-8 | behind, 1.14× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.53×, p 1.3e-8 | ahead, 0.52×, p 3.4e-8 | behind, 2.58× | behind, 168.73× |
| tally | cpp | 0 | undecided | undecided | undecided | undecided |
| tally | rust | 20 | ahead, 0.50×, p 1.4e-8 | ahead, 0.47×, p 3.4e-8 | behind, 2.65× | behind, 199.58× |
| server | c-wasm | 97 | undecided, 0.99×, p 0.044 | ahead, 0.78×, p 1.2e-33 | behind, 1.11× | ahead, 0.12× |
| server | cpp-wasm | 97 | undecided, 1.00×, p 0.438 | ahead, 0.79×, p 1.2e-33 | behind, 1.11× | ahead, 0.06× |
| server | rust-wasm | 97 | undecided, 0.99×, p 0.091 | ahead, 0.83×, p 1.2e-33 | behind, 1.09× | ahead, 0.03× |
| server | c | 20 | behind, 1.07×, p 2.1e-8 | behind, 1.06×, p 3.4e-8 | behind, 2.54× | behind, 218.25× |
| server | cpp | 20 | behind, 1.07×, p 2.4e-8 | behind, 1.06×, p 3.4e-8 | behind, 2.54× | behind, 211.93× |
| server | rust | 20 | behind, 1.36×, p 2.4e-8 | behind, 2.42×, p 3.4e-8 | behind, 1.93× | behind, 181.23× |
| files | c-wasm | 80 | ahead, 1.00×, p 4.3e-4 | ahead, 0.94×, p 1.6e-16 | behind, 1.11× | ahead, 0.05× |
| files | cpp-wasm | 160 | ahead, 1.00×, p 8.1e-6 | ahead, 0.94×, p 1.0e-23 | behind, 1.11× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 1.00×, p 4.7e-5 | ahead, 0.93×, p 3.4e-17 | behind, 1.10× | ahead, 0.03× |
| files | c | 20 | behind, 1.50×, p 8.5e-9 | behind, 1.52×, p 3.4e-8 | behind, 2.91× | behind, 164.91× |
| files | cpp | 20 | behind, 1.50×, p 1.2e-8 | behind, 1.50×, p 3.4e-8 | behind, 2.91× | behind, 164.91× |
| files | rust | 20 | behind, 1.80×, p 1.3e-8 | behind, 1.65×, p 3.4e-8 | behind, 2.68× | behind, 205.97× |
| readings | c-wasm | 20 | ahead, 0.86×, p 1.7e-5 | ahead, 0.90×, p 5.5e-6 | behind, 1.11× | ahead, 0.04× |
| readings | cpp-wasm | 40 | ahead, 0.86×, p 1.1e-4 | ahead, 0.93×, p 2.6e-11 | behind, 1.10× | ahead, 0.04× |
| readings | rust-wasm | 20 | ahead, 0.86×, p 9.8e-4 | ahead, 0.91×, p 6.3e-6 | behind, 1.10× | ahead, 0.02× |
| readings | c | 20 | behind, 3.00×, p 7.1e-9 | behind, 3.93×, p 3.4e-8 | behind, 3.01× | behind, 173.85× |
| readings | cpp | 0 | undecided | undecided | undecided | undecided |
| readings | rust | 20 | behind, 3.00×, p 4.0e-9 | behind, 2.68×, p 3.4e-8 | behind, 2.71× | behind, 206.80× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 11.7 ms | 11.6 ms | 11.6 ms | 12.0 ms | 3.99 ms | 3.85 ms | 4.43 ms |
| peak memory | 10.2 MiB | 9.18 MiB | 9.18 MiB | 9.26 MiB | 3.38 MiB | 3.38 MiB | 3.74 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 181 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 0 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 12.3 ms | 12.5 ms | 12.4 ms | 12.7 ms | 4.59 ms | – | 5.12 ms |
| peak memory | 10.3 MiB | 9.29 MiB | 9.29 MiB | 9.29 MiB | 3.42 MiB | – | 3.78 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 0 B | 120 KiB |

Its rounds took 193 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\add\main.cpp: D:\a\lengua\lengua\benches\programs\add\main.cpp:10:6: error: no template named 'optional' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      | ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:10:37: error: no type named 'string_view' in namespace 'std'
   10 | std::optional<long long> parse(std::string_view text) {
      |                                ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:12:10: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |          ^~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\add\main.cpp:12:30: error: no member named 'from_chars' in namespace 'std'
   12 |     auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), number);
      |                         ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:13:23: error: no member named 'errc' in namespace 'std'
   13 |     if (error != std::errc{} || end != text.data() + text.size() || number < -1000000000LL ||
      |                  ~~~~~^
D:\a\lengua\lengua\benches\programs\add\main.cpp:15:21: error: no member named 'nullopt' in namespace 'std'
   15 |         return std::nullopt;
      |                ~~~~~^
1 warning and 5 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 0 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | – | 0.00 ns |
| wall time | 11.9 ms | 12.0 ms | 11.9 ms | 12.3 ms | 4.70 ms | – | 5.15 ms |
| peak memory | 10.3 MiB | 9.28 MiB | 9.19 MiB | 9.28 MiB | 3.42 MiB | – | 3.78 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 0 B | 121 KiB |

Its rounds took 187 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp: D:\a\lengua\lengua\benches\programs\hypotenuse\main.cpp:13:16: error: no type named 'string_view' in namespace 'std'
   13 | int parse(std::string_view text, long long& units) {
      |           ~~~~~^
1 error generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 40 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 12.1 ms | 12.2 ms | 12.2 ms | 12.5 ms | 5.53 ms | 5.41 ms | 5.79 ms |
| peak memory | 10.7 MiB | 9.75 MiB | 9.75 MiB | 9.90 MiB | 4.50 MiB | 4.50 MiB | 4.42 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 157 KiB | 157 KiB | 124 KiB |

Its rounds took 181 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 20 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 27.3 ms | 27.8 ms | 27.9 ms | 28.0 ms | 16.5 ms | 16.4 ms | 18.0 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.31 MiB | 3.45 MiB | 3.45 MiB | 3.82 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 109 KiB |

Its rounds took 305 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 1280 | 2000 | 320 | 40 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 10.0 ms | 9.95 ms | 9.95 ms | 10.2 ms | 3.99 ms | 3.99 ms | 4.34 ms |
| peak memory | 10.6 MiB | 9.53 MiB | 9.53 MiB | 9.71 MiB | 4.39 MiB | 4.39 MiB | 4.37 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 111 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 640 | 20 | 320 | 20 | 2000 | 20 |
| CPU time | 31.2 ms | 31.2 ms | 188 ms | 31.2 ms | 31.2 ms | 31.2 ms | 23.4 ms |
| wall time | 34.9 ms | 37.0 ms | 196 ms | 37.5 ms | 26.2 ms | 35.7 ms | 26.5 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.32 MiB | 3.48 MiB | 3.47 MiB | 3.85 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 150 KiB | 150 KiB | 120 KiB |

Its rounds took 249 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 20 | 20 | 20 | 40 |
| CPU time | 46.9 ms | 46.9 ms | 46.9 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 44.2 ms | 54.0 ms | 53.6 ms | 51.0 ms | 35.0 ms | 35.1 ms | 37.4 ms |
| peak memory | 10.3 MiB | 9.31 MiB | 9.31 MiB | 9.32 MiB | 3.48 MiB | 3.48 MiB | 3.82 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 119 KiB |

Its rounds took 11 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 40 | 40 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 62.5 ms | 62.5 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 48.7 ms | 59.8 ms | 59.9 ms | 60.1 ms | 21.9 ms | 21.9 ms | 23.3 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.31 MiB | 3.47 MiB | 3.47 MiB | 3.84 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 14 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 20 | 20 | 20 | 0 | 40 |
| CPU time | 219 ms | 1.09 s | 406 ms | 375 ms | 523 ms | – | 211 ms |
| wall time | 221 ms | 1.09 s | 399 ms | 369 ms | 527 ms | – | 212 ms |
| peak memory | 79.2 MiB | 78.1 MiB | 86.2 MiB | 86.2 MiB | 78.4 MiB | – | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 0 B | 120 KiB |

Its rounds took 77 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\sort\main.cpp: D:\a\lengua\lengua\benches\programs\sort\main.cpp:39:22: error: no member named 'string_view' in namespace 'std'
   39 |     std::vector<std::string_view> lines;
      |                 ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:21: error: expected ';' after expression
   40 |     std::string_view rest(input, length);
      |                     ^
      |                     ;
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:10: error: no member named 'string_view' in namespace 'std'
   40 |     std::string_view rest(input, length);
      |     ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:40:22: error: use of undeclared identifier 'rest'
   40 |     std::string_view rest(input, length);
      |                      ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:41:13: error: use of undeclared identifier 'rest'
   41 |     while (!rest.empty()) {
      |             ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:42:31: error: use of undeclared identifier 'rest'
   42 |         std::size_t newline = rest.find('\n');
      |                               ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:43:25: error: use of undeclared identifier 'rest'
   43 |         lines.push_back(rest.substr(0, newline));
      |                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:44:9: error: use of undeclared identifier 'rest'
   44 |         rest.remove_prefix(newline + 1);
      |         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:27: error: use of undeclared identifier 'line'; did you mean 'lines'?
   52 |     for (std::string_view line : lines) {
      |                           ^~~~
      |                           lines
D:\a\lengua\lengua\benches\programs\sort\main.cpp:39:35: note: 'lines' declared here
   39 |     std::vector<std::string_view> lines;
      |                                   ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:15: error: no member named 'string_view' in namespace 'std'
   52 |     for (std::string_view line : lines) {
      |          ~~~~~^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:52:39: error: expected ';' in 'for' statement specifier
   52 |     for (std::string_view line : lines) {
      |                                       ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:25: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:41: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                                         ^
D:\a\lengua\lengua\benches\programs\sort\main.cpp:53:69: error: use of undeclared identifier 'line'
   53 |         if (std::fwrite(line.data(), 1, line.size() + 1, stdout) != line.size() + 1) {
      |                                                                     ^
14 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 0 | 20 |
| CPU time | 125 ms | 297 ms | 297 ms | 359 ms | 234 ms | – | 250 ms |
| wall time | 118 ms | 298 ms | 300 ms | 362 ms | 227 ms | – | 249 ms |
| peak memory | 10.6 MiB | 9.37 MiB | 9.42 MiB | 9.31 MiB | 4.11 MiB | – | 4.00 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 0 B | 128 KiB |

Its rounds took 37 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\tally\main.cpp: D:\a\lengua\lengua\benches\programs\tally\main.cpp:42:6: error: no type named 'string_view' in namespace 'std'
   42 | std::string_view word_of(const Entry& entry) {
      | ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:43:12: error: excess elements in scalar initializer
   43 |     return {store.data() + entry.offset, entry.length};
      |            ^                           ~~~~~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\tally\main.cpp:47:15: error: no type named 'string_view' in namespace 'std'
   47 | bool add(std::string_view word) {
      |          ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:107:27: error: no member named 'string_view' in namespace 'std'
  107 |             if (!add(std::string_view(buffer.data() + start, at - start))) {
      |                      ~~~~~^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:125:14: error: no type named 'string_view' in namespace 'std'
  125 |         std::string_view word = word_of(entry);
      |         ~~~~~^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8923:17: error: static_cast from 'void' to 'bool' is not allowed
 8923 |             if (_DEBUG_LT_PRED(_Pred, _Val, *_First)) { // found new earliest element, move to front
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9055:18: note: in instantiation of function template specialization 'std::_Insertion_sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9055 |             _STD _Insertion_sort_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:71:13: error: static_cast from 'void' to 'bool' is not allowed
   71 |         if (_DEBUG_LT_PRED(_Pred, *(_First + _Idx), *(_First + (_Idx - 1)))) {
      |             ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:27:25: error: static_cast from 'void' to 'bool' is not allowed
   27 |         _Top < _Hole && _DEBUG_LT_PRED(_Pred, *(_First + _Idx), _Val); _Idx = (_Hole - 1) >> 1) { // shift for codegen
      |                         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:83:10: note: in instantiation of function template specialization 'std::_Push_heap_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
   83 |     _STD _Push_heap_by_index(_First, _Hole, _Top, _STD forward<_Ty>(_Val), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:129:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_by_index<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  129 |         _STD _Pop_heap_hole_by_index(_First, _Hole, _Bottom, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9060:18: note: in instantiation of function template specialization 'std::_Make_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9060 |             _STD _Make_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
In file included from C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:10:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:93:5: error: no matching function for call to '_Pop_heap_hole_by_index'
   93 |     _STD _Pop_heap_hole_by_index(
      |     ^~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\yvals_core.h:1906:17: note: expanded from macro '_STD'
 1906 | #define _STD    ::std::
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:103:14: note: in instantiation of function template specialization 'std::_Pop_heap_hole_unchecked<(anonymous namespace)::Entry *, (anonymous namespace)::Entry, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  103 |         _STD _Pop_heap_hole_unchecked(_First, _Last, _Last, _STD move(_Val), _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:7816:14: note: in instantiation of function template specialization 'std::_Pop_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 7816 |         _STD _Pop_heap_unchecked(_First, _Last, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9061:18: note: in instantiation of function template specialization 'std::_Sort_heap_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9061 |             _STD _Sort_heap_unchecked(_First, _Last, _Pred);
      |                  ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\__msvc_heap_algorithms.hpp:57:19: note: candidate template ignored: substitution failure [with _RanIt = (anonymous namespace)::Entry *, _Ty = (anonymous namespace)::Entry, _Pr = (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)]
   57 | _CONSTEXPR20 void _Pop_heap_hole_by_index(
      |                   ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8942:9: error: static_cast from 'void' to 'bool' is not allowed
 8942 |     if (_DEBUG_LT_PRED(_Pred, *_Mid, *_First)) {
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8963:14: note: in instantiation of function template specialization 'std::_Med3_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 8963 |         _STD _Med3_unchecked(_First, _First + _Step, _First + _Two_step, _Pred);
      |              ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9045:10: note: in instantiation of function template specialization 'std::_Guess_median_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9045 |     _STD _Guess_median_unchecked(_First, _Mid, _STD _Prev_iter(_Last), _Pred);
      |          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8946:9: error: static_cast from 'void' to 'bool' is not allowed
 8946 |     if (_DEBUG_LT_PRED(_Pred, *_Last, *_Mid)) { // swap middle and last, then test first again
      |         ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8978:33: error: static_cast from 'void' to 'bool' is not allowed
 8978 |     while (_First < _Pfirst && !_DEBUG_LT_PRED(_Pred, *_STD _Prev_iter(_Pfirst), *_Pfirst)
      |                                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9047:17: note: in instantiation of function template specialization 'std::_Partition_by_pivot_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9047 |     return _STD _Partition_by_pivot_unchecked(_First, _Mid, _Last, _Pred);
      |                 ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9066:26: note: in instantiation of function template specialization 'std::_Partition_by_median_guess_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9066 |         auto _Mid = _STD _Partition_by_median_guess_unchecked(_First, _Last, _Pred);
      |                          ^
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9085:10: note: in instantiation of function template specialization 'std::_Sort_unchecked<(anonymous namespace)::Entry *, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
 9085 |     _STD _Sort_unchecked(_UFirst, _ULast, _ULast - _UFirst, _STD _Pass_fn(_Pred));
      |          ^
D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:10: note: in instantiation of function template specialization 'std::sort<std::_Vector_iterator<std::_Vector_val<std::_Simple_types<(anonymous namespace)::Entry>>>, (lambda at D:\a\lengua\lengua\benches\programs\tally\main.cpp:117:47)>' requested here
  117 |     std::sort(entries.begin(), entries.end(), [](const Entry& x, const Entry& y) {
      |          ^
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8983:31: error: static_cast from 'void' to 'bool' is not allowed
 8983 |     while (_Plast < _Last && !_DEBUG_LT_PRED(_Pred, *_Plast, *_Pfirst) && !_Pred(*_Pfirst, *_Plast)) {
      |                               ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:8992:17: error: static_cast from 'void' to 'bool' is not allowed
 8992 |             if (_DEBUG_LT_PRED(_Pred, *_Pfirst, *_Gfirst)) {
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
In file included from D:\a\lengua\lengua\benches\programs\tally\main.cpp:7:
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\algorithm:9006:17: error: static_cast from 'void' to 'bool' is not allowed
 9006 |             if (_DEBUG_LT_PRED(_Pred, *_Glast_prev, *_Pfirst)) {
      |                 ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\include\xutility:1774:36: note: expanded from macro '_DEBUG_LT_PRED'
 1774 | #define _DEBUG_LT_PRED(pred, x, y) static_cast<bool>(pred(x, y))
      |                                    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~
15 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 97 | 97 | 97 | 97 | 20 | 20 | 20 |
| CPU time | 1.06 s | 1.08 s | 1.06 s | 1.08 s | 992 ms | 992 ms | 781 ms |
| wall time | 1.07 s | 1.38 s | 1.36 s | 1.30 s | 1.01 s | 1.01 s | 441 ms |
| peak memory | 10.2 MiB | 9.12 MiB | 9.17 MiB | 9.32 MiB | 4.01 MiB | 4.00 MiB | 5.28 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 121 KiB | 142 KiB |

Its rounds took 630 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 80 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 141 ms | 141 ms | 141 ms | 141 ms | 93.8 ms | 93.8 ms | 78.1 ms |
| wall time | 139 ms | 148 ms | 147 ms | 149 ms | 92.6 ms | 93.7 ms | 85.1 ms |
| peak memory | 10.5 MiB | 9.46 MiB | 9.46 MiB | 9.52 MiB | 3.61 MiB | 3.61 MiB | 3.92 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 85 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp: clang version 20.1.8; clang++ -O3 -flto -fuse-ld=lld -march=native -lws2_32

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 20 | 40 | 20 | 20 | 0 | 20 |
| CPU time | 93.8 ms | 109 ms | 109 ms | 109 ms | 31.2 ms | – | 31.2 ms |
| wall time | 98.9 ms | 110 ms | 107 ms | 109 ms | 25.1 ms | – | 36.8 ms |
| peak memory | 10.3 MiB | 9.30 MiB | 9.30 MiB | 9.31 MiB | 3.41 MiB | – | 3.79 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 0 B | 124 KiB |

Its rounds took 17 seconds.

- lengua: lengua-mcp 0.2.0 run, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=4294967296 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=4294967296

- c: clang version 20.1.8; clang -O3 -flto -fuse-ld=lld -march=native -lws2_32

- cpp wasn't measured to the end: clang++ couldn't build D:\a\lengua\lengua\benches\programs\readings\main.cpp: D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:25: error: expected ';' after expression
   14 |         std::string_view text(argv[1]);
      |                         ^
      |                         ;
D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:14: error: no member named 'string_view' in namespace 'std'
   14 |         std::string_view text(argv[1]);
      |         ~~~~~^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:14:26: error: use of undeclared identifier 'text'
   14 |         std::string_view text(argv[1]);
      |                          ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:14: warning: decomposition declarations are a C++17 extension [-Wc++17-extensions]
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |              ^~~~~~~~~~~~
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:34: error: no member named 'from_chars' in namespace 'std'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                             ~~~~~^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:45: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                             ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:58: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                          ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:15:72: error: use of undeclared identifier 'text'
   15 |         auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), count);
      |                                                                        ^
D:\a\lengua\lengua\benches\programs\readings\main.cpp:16:22: error: no member named 'errc' in namespace 'std'; did you mean 'argc'?
   16 |         if (error != std::errc{} || end != text.data() + text.size()) {
      |                      ^~~~~~~~~
      |                      argc
D:\a\lengua\lengua\benches\programs\readings\main.cpp:10:14: note: 'argc' declared here
   10 | int main(int argc, char** argv) {
      |              ^
1 warning and 8 errors generated.

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-mcp run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, in the same engine with the same cache of compiled modules and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

Windows counts CPU time in ticks of 15.6 ms, so on a short run each version's median is a whole number of ticks, and a shift of medians is none or a whole tick. The CPU time cells give means instead: each run's ticks sample its CPU time, so a mean converges on it. A cell gives lengua's mean against lengua-module's, then the difference of the means, with its 95% interval (1.96 standard errors either side, Welch's). Wall time and peak memory give medians, as on every runner.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 8 s | means 12.4 ms against 11.8 ms: +586 µs (-1.83 ms to +3.00 ms) | 12.2 ms against 12.2 ms: +69.2 µs (-71.0 µs to +217 µs) | 10.2 MiB against 9.29 MiB: +960 KiB (+960 KiB to +964 KiB) |
| add | 160, 160 in 7 s | means 14.6 ms against 13.6 ms: +1.07 ms (-1.75 ms to +3.89 ms) | 11.9 ms against 11.8 ms: +107 µs (-23.2 µs to +235 µs) | 10.3 MiB against 9.28 MiB: +1004 KiB (+1000 KiB to +1004 KiB) |
| hypotenuse | 160, 160 in 7 s | means 12.9 ms against 11.9 ms: +977 µs (-1.77 ms to +3.72 ms) | 11.8 ms against 11.5 ms: +148 µs (+18.4 µs to +278 µs) | 10.3 MiB against 9.28 MiB: +1016 KiB (+1012 KiB to +1016 KiB) |
| upcase | 160, 160 in 6 s | means 12.7 ms against 11.4 ms: +1.27 ms (-1.55 ms to +4.09 ms) | 10.5 ms against 10.5 ms: +103 µs (+28.8 µs to +176 µs) | 10.7 MiB against 9.62 MiB: +1.04 MiB (+1.03 MiB to +1.04 MiB) |
| upcase-stdin | 160, 160 in 12 s | means 29.9 ms against 26.9 ms: +3.03 ms (+401 µs to +5.65 ms) | 27.0 ms against 27.0 ms: +3.40 µs (-225 µs to +234 µs) | 10.3 MiB against 9.30 MiB: +1.01 MiB (+1.00 MiB to +1.01 MiB) |
| shout | 160, 160 in 7 s | means 13.6 ms against 10.4 ms: +3.22 ms (+414 µs to +6.03 ms) | 10.5 ms against 10.4 ms: +95.3 µs (+500 ns to +191 µs) | 10.6 MiB against 9.62 MiB: +1.01 MiB (+1.00 MiB to +1.02 MiB) |
| wc | 160, 160 in 15 s | means 36.8 ms against 35.3 ms: +1.56 ms (-653 µs to +3.78 ms) | 34.8 ms against 34.7 ms: +128 µs (-44.4 µs to +308 µs) | 10.3 MiB against 9.31 MiB: +1.04 MiB (+1.04 MiB to +1.04 MiB) |
| histogram | 160, 160 in 18 s | means 46.1 ms against 45.4 ms: +684 µs (-1.92 ms to +3.28 ms) | 44.0 ms against 44.0 ms: +54.8 µs (-76.6 µs to +182 µs) | 10.3 MiB against 9.31 MiB: +1000 KiB (+996 KiB to +1004 KiB) |
| crc32c | 160, 160 in 19 s | means 51.2 ms against 49.3 ms: +1.86 ms (-149 µs to +3.86 ms) | 48.6 ms against 48.6 ms: +32.6 µs (-135 µs to +201 µs) | 10.3 MiB against 9.31 MiB: +1000 KiB (+996 KiB to +1004 KiB) |
| sort | 128, 128 in 60 s | means 224 ms against 224 ms: +0.00 ns (-3.03 ms to +3.03 ms) | 222 ms against 222 ms: +250 ns (-1.20 ms to +1.07 ms) | 79.2 MiB against 78.1 MiB: +1.09 MiB (+1.09 MiB to +1.10 MiB) |
| tally | 160, 160 in 42 s | means 120 ms against 121 ms: -977 µs (-3.93 ms to +1.98 ms) | 119 ms against 118 ms: +83.1 µs (-169 µs to +316 µs) | 10.6 MiB against 9.53 MiB: +1.08 MiB (+1.08 MiB to +1.08 MiB) |
| server | 25, 25 in 60 s | means 1.09 s against 1.07 s: +16.2 ms (+6.06 ms to +26.4 ms) | 1.08 s against 1.08 s: +597 µs (-3.63 ms to +4.91 ms) | 10.2 MiB against 9.13 MiB: +1.02 MiB (+1.00 MiB to +1.05 MiB) |
| files | 160, 160 in 48 s | means 138 ms against 138 ms: +195 µs (-2.57 ms to +2.96 ms) | 136 ms against 137 ms: -24.4 µs (-488 µs to +411 µs) | 10.5 MiB against 9.54 MiB: +996 KiB (+984 KiB to +1008 KiB) |
| readings | 160, 160 in 35 s | means 100 ms against 100 ms: -97.7 µs (-2.78 ms to +2.59 ms) | 99.9 ms against 100 ms: -239 µs (-558 µs to +87.9 µs) | 10.3 MiB against 9.30 MiB: +1004 KiB (+1000 KiB to +1008 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
