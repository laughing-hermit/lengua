# lengua against C, C++ and Rust on windows-aarch64

Measured on windows-aarch64 (Cobalt 100, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 45b507bf262dab89bff38a1f9c35bdd5590c8965, in https://github.com/laughing-hermit/lengua/actions/runs/43099764273. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (4.91 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ≈ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ≈ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✓ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-aarch64 empty c-wasm memory: lengua is level now, and KNOWN_BEHIND line 745 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 746 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 empty rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 747 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add c-wasm memory: lengua is level now, and KNOWN_BEHIND line 760 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 761 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 add rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 762 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse c-wasm memory: lengua is level now, and KNOWN_BEHIND line 775 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 776 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 hypotenuse rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 777 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase c-wasm memory: lengua is level now, and KNOWN_BEHIND line 790 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 791 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 792 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin c-wasm memory: lengua is level now, and KNOWN_BEHIND line 805 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 807 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 809 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin c cpu: lengua is level now, and KNOWN_BEHIND line 810 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin cpp cpu: lengua is level now, and KNOWN_BEHIND line 814 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 upcase-stdin rust cpu: lengua is level now, and KNOWN_BEHIND line 818 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout c-wasm memory: lengua is level now, and KNOWN_BEHIND line 822 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 823 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 shout rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 824 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc c-wasm memory: lengua is level now, and KNOWN_BEHIND line 837 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 838 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 839 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc c cpu: lengua is level now, and KNOWN_BEHIND line 840 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc cpp cpu: lengua is level now, and KNOWN_BEHIND line 844 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc rust cpu: lengua is level now, and KNOWN_BEHIND line 848 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 wc rust wall: lengua is level now, and KNOWN_BEHIND line 849 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 histogram c-wasm memory: lengua is level now, and KNOWN_BEHIND line 852 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 histogram cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 853 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 histogram rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 854 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 crc32c c-wasm memory: lengua is level now, and KNOWN_BEHIND line 867 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 crc32c cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 868 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 crc32c rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 869 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 tally c-wasm memory: lengua is level now, and KNOWN_BEHIND line 887 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 tally cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 888 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 server c-wasm memory: lengua is level now, and KNOWN_BEHIND line 896 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 server cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 897 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 server rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 898 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 files c-wasm memory: lengua is level now, and KNOWN_BEHIND line 910 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 files cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 911 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 files rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 912 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 readings c-wasm memory: lengua is level now, and KNOWN_BEHIND line 925 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 readings cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 926 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-aarch64 readings rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 927 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.44× | 1.53× | 49.54× |
| add | – | 1.41× | 1.52× | 43.36× |
| hypotenuse | – | 1.41× | 1.53× | 42.99× |
| upcase | – | 1.41× | 1.42× | 42.09× |
| upcase-stdin | 1.00× | 1.32× | 1.51× | 47.22× |
| shout | – | 1.40× | 1.42× | 45.31× |
| wc | 1.00× | 1.07× | 1.51× | 43.93× |
| histogram | 1.33× | 1.15× | 1.50× | 43.17× |
| crc32c | 2.00× | 2.02× | 1.51× | 45.72× |
| sort | 1.12× | 1.09× | 0.96× | 43.38× |
| tally | 0.65× | 0.64× | 1.49× | 40.26× |
| server | 1.17× | 1.57× | 1.41× | 44.51× |
| files | 2.00× | 1.83× | 1.53× | 42.27× |
| readings | 3.50× | 2.97× | 1.53× | 42.09× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 1.00×, p 0.042 | level, 1.03×, p 2.0e-143 | level, 1.02× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 1.00×, p 0.133 | level, 1.03×, p 3.9e-157 | level, 1.02× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, 1.00×, p 0.090 | level, 1.02×, p 7.9e-51 | level, 1.01× | ahead, 0.01× |
| empty | c | 80 | behind, p 2.9e-5 | behind, 1.43×, p 4.7e-28 | behind, 1.53× | behind, 46.77× |
| empty | cpp | 80 | behind, p 6.8e-4 | behind, 1.44×, p 4.7e-28 | behind, 1.53× | behind, 46.77× |
| empty | rust | 160 | behind, p 3.0e-5 | behind, 1.36×, p 2.9e-54 | behind, 1.44× | behind, 49.54× |
| add | c-wasm | 2000 | level, 2.00×, p 0.058 | level, 1.02×, p 3.5e-92 | level, 1.01× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, p 0.032 | level, 1.02×, p 3.6e-102 | level, 1.01× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, 2.00×, p 0.110 | level, 1.02×, p 1.2e-50 | level, 1.01× | ahead, 0.02× |
| add | c | 160 | behind, p 6.0e-7 | behind, 1.41×, p 5.7e-53 | behind, 1.52× | behind, 35.29× |
| add | cpp | 80 | behind, p 6.9e-5 | behind, 1.41×, p 4.7e-28 | behind, 1.52× | behind, 35.29× |
| add | rust | 160 | behind, p 6.6e-4 | behind, 1.34×, p 2.9e-54 | behind, 1.43× | behind, 43.36× |
| hypotenuse | c-wasm | 2000 | level, 1.00×, p 0.207 | level, 1.03×, p 3.8e-111 | level, 1.01× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, 1.00×, p 0.015 | level, 1.03×, p 2.4e-118 | level, 1.01× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, 1.00×, p 0.293 | level, 1.01×, p 4.9e-35 | level, 1.01× | ahead, 0.03× |
| hypotenuse | c | 40 | behind, p 0.001 | behind, 1.41×, p 7.2e-15 | behind, 1.53× | behind, 35.05× |
| hypotenuse | cpp | 20 | behind, p 0.001 | behind, 1.42×, p 3.4e-8 | behind, 1.52× | behind, 34.93× |
| hypotenuse | rust | 20 | behind, p 0.001 | behind, 1.33×, p 3.4e-8 | behind, 1.43× | behind, 42.99× |
| upcase | c-wasm | 1280 | level, 1.00×, p 1.5e-4 | level, 1.04×, p 2.8e-113 | level, 1.01× | ahead, 0.06× |
| upcase | cpp-wasm | 1280 | level, 1.00×, p 1.7e-4 | level, 1.03×, p 4.2e-103 | level, 1.01× | ahead, 0.06× |
| upcase | rust-wasm | 640 | level, 1.00×, p 2.0e-5 | level, 1.02×, p 7.5e-25 | level, 1.00× | ahead, 0.03× |
| upcase | c | 40 | behind, p 8.1e-5 | behind, 1.40×, p 7.2e-15 | behind, 1.38× | behind, 32.98× |
| upcase | cpp | 40 | behind, p 1.6e-4 | behind, 1.41×, p 1.4e-13 | behind, 1.38× | behind, 32.98× |
| upcase | rust | 40 | behind, p 1.7e-4 | behind, 1.35×, p 7.2e-15 | behind, 1.42× | behind, 42.09× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 2.4e-4 | level, 1.01×, p 1.2e-65 | level, 1.02× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.015 | level, 1.01×, p 2.8e-34 | level, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.001 | level, 1.01×, p 5.6e-33 | level, 1.01× | ahead, 0.03× |
| upcase-stdin | c | 40 | level, 1.00×, p 4.3e-6 | behind, 1.32×, p 7.2e-15 | behind, 1.51× | behind, 45.10× |
| upcase-stdin | cpp | 20 | level, 1.00×, p 0.001 | behind, 1.31×, p 3.4e-8 | behind, 1.51× | behind, 45.10× |
| upcase-stdin | rust | 40 | level, 1.00×, p 4.3e-6 | behind, 1.26×, p 7.2e-15 | behind, 1.44× | behind, 47.22× |
| shout | c-wasm | 2000 | level, 1.00×, p 9.1e-4 | level, 1.03×, p 1.1e-163 | level, 1.02× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 1.00×, p 0.027 | level, 1.03×, p 9.4e-154 | level, 1.02× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, 1.00×, p 0.080 | level, 1.01×, p 1.6e-50 | level, 1.01× | ahead, 0.03× |
| shout | c | 80 | behind, p 1.2e-5 | behind, 1.40×, p 4.7e-28 | behind, 1.40× | behind, 43.92× |
| shout | cpp | 160 | behind, p 6.1e-5 | behind, 1.39×, p 2.9e-54 | behind, 1.40× | behind, 43.92× |
| shout | rust | 80 | behind, p 3.3e-5 | behind, 1.32×, p 4.7e-28 | behind, 1.42× | behind, 45.31× |
| wc | c-wasm | 80 | level, 1.00×, p 5.7e-6 | ahead, 0.85×, p 4.7e-28 | level, 1.01× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.19×, p 1.1e-8 | ahead, 0.21×, p 3.4e-8 | level, 1.01× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.83×, p 3.9e-4 | ahead, 0.85×, p 3.4e-8 | level, 1.02× | ahead, 0.04× |
| wc | c | 160 | level, 1.00×, p 6.1e-5 | behind, 1.07×, p 5.1e-54 | behind, 1.51× | behind, 34.93× |
| wc | cpp | 160 | level, 1.00×, p 5.2e-5 | behind, 1.06×, p 1.6e-49 | behind, 1.51× | behind, 34.93× |
| wc | rust | 640 | level, 1.00×, p 6.8e-4 | level, 1.04×, p 1.6e-136 | behind, 1.44× | behind, 43.93× |
| histogram | c-wasm | 20 | ahead, 0.67×, p 5.6e-8 | ahead, 0.67×, p 3.4e-8 | level, 1.01× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.67×, p 7.9e-8 | ahead, 0.67×, p 3.4e-8 | level, 1.01× | ahead, 0.09× |
| histogram | rust-wasm | 1280 | level, 1.00×, p 9.6e-7 | level, 0.98×, p 7.1e-181 | level, 1.02× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 2.5e-5 | behind, 1.15×, p 3.4e-8 | behind, 1.50× | behind, 35.05× |
| histogram | cpp | 20 | behind, 1.33×, p 2.9e-4 | behind, 1.15×, p 3.4e-8 | behind, 1.50× | behind, 35.05× |
| histogram | rust | 40 | behind, 1.33×, p 2.4e-4 | behind, 1.11×, p 7.2e-15 | behind, 1.43× | behind, 43.17× |
| crc32c | c-wasm | 20 | level, 1.00×, p 9.2e-4 | ahead, 0.87×, p 3.4e-8 | level, 1.02× | ahead, 0.08× |
| crc32c | cpp-wasm | 40 | level, 1.00×, p 2.2e-6 | ahead, 0.86×, p 7.2e-15 | level, 1.02× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | level, 1.00×, p 3.4e-4 | ahead, 0.83×, p 3.4e-8 | level, 1.02× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.00×, p 8.9e-8 | behind, 2.02×, p 3.4e-8 | behind, 1.51× | behind, 35.17× |
| crc32c | cpp | 20 | behind, 2.00×, p 8.9e-8 | behind, 2.02×, p 3.4e-8 | behind, 1.51× | behind, 35.17× |
| crc32c | rust | 20 | behind, 2.00×, p 9.9e-8 | behind, 1.96×, p 3.4e-8 | behind, 1.44× | behind, 45.72× |
| sort | c-wasm | 20 | ahead, 0.20×, p 1.9e-8 | ahead, 0.20×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.58×, p 1.2e-8 | ahead, 0.57×, p 3.4e-8 | ahead, 0.91× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.66×, p 1.6e-8 | ahead, 0.64×, p 3.4e-8 | ahead, 0.91× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.42×, p 1.6e-8 | ahead, 0.41×, p 3.4e-8 | level, 0.96× | behind, 43.38× |
| sort | cpp | 20 | ahead, 0.43×, p 2.1e-8 | ahead, 0.42×, p 3.4e-8 | ahead, 0.85× | behind, 39.62× |
| sort | rust | 20 | behind, 1.12×, p 9.1e-8 | behind, 1.09×, p 3.4e-8 | ahead, 0.90× | behind, 42.64× |
| tally | c-wasm | 20 | ahead, 0.55×, p 5.0e-9 | ahead, 0.54×, p 3.4e-8 | level, 1.04× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.55×, p 1.2e-8 | ahead, 0.54×, p 3.4e-8 | level, 1.04× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.39×, p 1.2e-8 | ahead, 0.40×, p 3.4e-8 | behind, 1.07× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.65×, p 1.1e-8 | ahead, 0.64×, p 3.4e-8 | behind, 1.46× | behind, 34.23× |
| tally | cpp | 20 | ahead, 0.65×, p 7.8e-9 | ahead, 0.63×, p 3.4e-8 | behind, 1.47× | behind, 33.22× |
| tally | rust | 20 | ahead, 0.61×, p 9.3e-9 | ahead, 0.62×, p 3.4e-8 | behind, 1.49× | behind, 40.26× |
| server | c-wasm | 73 | level, 1.01×, p 1.5e-4 | ahead, 0.94×, p 1.4e-25 | level, 1.02× | ahead, 0.12× |
| server | cpp-wasm | 73 | level, 1.01×, p 0.009 | ahead, 0.94×, p 9.4e-26 | level, 1.02× | ahead, 0.06× |
| server | rust-wasm | 73 | level, 1.00×, p 0.080 | ahead, 0.88×, p 9.4e-26 | level, 1.01× | ahead, 0.03× |
| server | c | 20 | behind, 1.16×, p 2.9e-8 | behind, 1.15×, p 3.4e-8 | behind, 1.41× | behind, 44.51× |
| server | cpp | 20 | behind, 1.17×, p 3.0e-8 | behind, 1.15×, p 3.4e-8 | behind, 1.41× | behind, 43.55× |
| server | rust | 20 | ahead, 0.88×, p 6.8e-7 | behind, 1.57×, p 3.4e-8 | behind, 1.16× | behind, 36.85× |
| files | c-wasm | 20 | ahead, 0.92×, p 5.7e-5 | ahead, 0.91×, p 3.9e-8 | level, 1.03× | ahead, 0.05× |
| files | cpp-wasm | 20 | ahead, 0.92×, p 1.2e-4 | ahead, 0.91×, p 3.4e-8 | level, 1.03× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.92×, p 2.3e-5 | ahead, 0.89×, p 3.9e-8 | level, 1.02× | ahead, 0.03× |
| files | c | 20 | behind, 2.00×, p 1.1e-8 | behind, 1.78×, p 3.4e-8 | behind, 1.53× | behind, 33.20× |
| files | cpp | 20 | behind, 2.00×, p 1.3e-8 | behind, 1.77×, p 3.4e-8 | behind, 1.53× | behind, 33.20× |
| files | rust | 20 | behind, 2.00×, p 7.7e-9 | behind, 1.83×, p 3.4e-8 | behind, 1.45× | behind, 42.27× |
| readings | c-wasm | 320 | level, 1.00×, p 1.2e-6 | level, 0.98×, p 1.8e-98 | level, 1.02× | ahead, 0.04× |
| readings | cpp-wasm | 2000 | level, 1.00×, p 4.8e-4 | level, 0.99×, p 9.2e-219 | level, 1.02× | ahead, 0.04× |
| readings | rust-wasm | 40 | level, 1.00×, p 1.9e-4 | level, 0.96×, p 7.2e-15 | level, 1.02× | ahead, 0.02× |
| readings | c | 20 | behind, 3.50×, p 1.7e-9 | behind, 2.97×, p 3.4e-8 | behind, 1.53× | behind, 35.29× |
| readings | cpp | 20 | behind, 3.50×, p 3.6e-9 | behind, 2.91×, p 3.4e-8 | behind, 1.53× | behind, 35.29× |
| readings | rust | 20 | behind, 2.33×, p 3.3e-9 | behind, 2.26×, p 3.4e-8 | behind, 1.44× | behind, 42.09× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 80 | 160 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 15.9 ms | 15.4 ms | 15.4 ms | 15.6 ms | 11.0 ms | 11.0 ms | 11.5 ms |
| peak memory | 7.12 MiB | 7.01 MiB | 7.01 MiB | 7.02 MiB | 4.67 MiB | 4.67 MiB | 4.96 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 108 KiB | 108 KiB | 102 KiB |

Its rounds took 264 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 160 |
| CPU time | 15.6 ms | 7.81 ms | 0.00 ns | 7.81 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 15.4 ms | 15.1 ms | 15.0 ms | 15.1 ms | 10.9 ms | 10.9 ms | 11.5 ms |
| peak memory | 7.16 MiB | 7.06 MiB | 7.06 MiB | 7.05 MiB | 4.70 MiB | 4.70 MiB | 4.99 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 142 KiB | 142 KiB | 116 KiB |

Its rounds took 259 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 20 | 20 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.0 ms | 15.6 ms | 15.6 ms | 15.8 ms | 11.2 ms | 11.2 ms | 11.9 ms |
| peak memory | 7.16 MiB | 7.06 MiB | 7.06 MiB | 7.05 MiB | 4.70 MiB | 4.70 MiB | 4.99 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 144 KiB | 144 KiB | 117 KiB |

Its rounds took 262 seconds.

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
| counted runs | 1280 | 1280 | 1280 | 640 | 40 | 40 | 40 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 18.2 ms | 17.6 ms | 17.6 ms | 17.7 ms | 12.7 ms | 12.7 ms | 13.2 ms |
| peak memory | 8.03 MiB | 7.92 MiB | 7.92 MiB | 8.02 MiB | 5.81 MiB | 5.81 MiB | 5.66 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 152 KiB | 152 KiB | 120 KiB |

Its rounds took 161 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 20 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 40.5 ms | 39.9 ms | 40.0 ms | 40.1 ms | 30.4 ms | 30.6 ms | 31.8 ms |
| peak memory | 7.16 MiB | 7.05 MiB | 7.05 MiB | 7.07 MiB | 4.73 MiB | 4.74 MiB | 4.98 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 112 KiB | 112 KiB | 106 KiB |

Its rounds took 464 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 160 | 80 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 16.9 ms | 16.4 ms | 16.4 ms | 16.7 ms | 12.1 ms | 12.4 ms | 12.8 ms |
| peak memory | 7.93 MiB | 7.76 MiB | 7.76 MiB | 7.88 MiB | 5.67 MiB | 5.68 MiB | 5.60 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 114 KiB | 114 KiB | 111 KiB |

Its rounds took 281 seconds.

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
| counted runs | 640 | 80 | 20 | 20 | 160 | 160 | 640 |
| CPU time | 31.2 ms | 46.9 ms | 203 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 44.9 ms | 52.4 ms | 213 ms | 53.1 ms | 41.8 ms | 42.2 ms | 43.2 ms |
| peak memory | 7.18 MiB | 7.09 MiB | 7.09 MiB | 7.06 MiB | 4.77 MiB | 4.77 MiB | 4.99 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 144 KiB | 144 KiB | 114 KiB |

Its rounds took 109 seconds.

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
| counted runs | 1280 | 20 | 20 | 1280 | 20 | 20 | 40 |
| CPU time | 62.5 ms | 93.8 ms | 93.8 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 64.7 ms | 94.7 ms | 95.2 ms | 66.0 ms | 55.6 ms | 55.4 ms | 57.5 ms |
| peak memory | 7.17 MiB | 7.09 MiB | 7.09 MiB | 7.06 MiB | 4.77 MiB | 4.77 MiB | 5.01 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 144 KiB | 144 KiB | 116 KiB |

Its rounds took 225 seconds.

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
| counted runs | 40 | 20 | 40 | 20 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 62.5 ms | 62.5 ms | 62.5 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 63.2 ms | 72.6 ms | 73.1 ms | 76.1 ms | 31.2 ms | 31.2 ms | 32.1 ms |
| peak memory | 7.18 MiB | 7.06 MiB | 7.06 MiB | 7.06 MiB | 4.76 MiB | 4.76 MiB | 5.00 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 143 KiB | 143 KiB | 110 KiB |

Its rounds took 15 seconds.

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
| CPU time | 297 ms | 1.48 s | 516 ms | 453 ms | 703 ms | 688 ms | 266 ms |
| wall time | 295 ms | 1.49 s | 521 ms | 460 ms | 716 ms | 703 ms | 270 ms |
| peak memory | 76.5 MiB | 76.3 MiB | 84.4 MiB | 84.4 MiB | 79.7 MiB | 89.7 MiB | 84.6 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 116 KiB | 127 KiB | 118 KiB |

Its rounds took 107 seconds.

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
| CPU time | 172 ms | 312 ms | 312 ms | 438 ms | 266 ms | 266 ms | 281 ms |
| wall time | 175 ms | 325 ms | 325 ms | 438 ms | 276 ms | 278 ms | 282 ms |
| peak memory | 7.88 MiB | 7.57 MiB | 7.55 MiB | 7.36 MiB | 5.40 MiB | 5.35 MiB | 5.28 MiB |
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
| counted runs | 73 | 73 | 73 | 73 | 20 | 20 | 20 |
| CPU time | 1.53 s | 1.52 s | 1.52 s | 1.53 s | 1.32 s | 1.31 s | 1.73 s |
| wall time | 1.54 s | 1.63 s | 1.65 s | 1.76 s | 1.34 s | 1.34 s | 985 ms |
| peak memory | 7.55 MiB | 7.36 MiB | 7.41 MiB | 7.50 MiB | 5.32 MiB | 5.33 MiB | 6.47 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 113 KiB | 116 KiB | 136 KiB |

Its rounds took 638 seconds.

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
| CPU time | 188 ms | 203 ms | 203 ms | 203 ms | 93.8 ms | 93.8 ms | 93.8 ms |
| wall time | 188 ms | 208 ms | 207 ms | 211 ms | 106 ms | 106 ms | 103 ms |
| peak memory | 7.52 MiB | 7.32 MiB | 7.32 MiB | 7.41 MiB | 4.91 MiB | 4.91 MiB | 5.17 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 152 KiB | 152 KiB | 119 KiB |

Its rounds took 30 seconds.

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
| counted runs | 2000 | 320 | 2000 | 40 | 20 | 20 | 20 |
| CPU time | 109 ms | 109 ms | 109 ms | 109 ms | 31.2 ms | 31.2 ms | 46.9 ms |
| wall time | 114 ms | 116 ms | 115 ms | 119 ms | 38.5 ms | 39.3 ms | 50.7 ms |
| peak memory | 7.20 MiB | 7.09 MiB | 7.09 MiB | 7.07 MiB | 4.71 MiB | 4.72 MiB | 5.01 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 142 KiB | 142 KiB | 120 KiB |

Its rounds took 577 seconds.

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
| empty | 160, 160 in 10 s | means 10.7 ms against 9.08 ms: +1.66 ms (-540 µs to +3.86 ms) | 15.5 ms against 15.1 ms: +444 µs (+346 µs to +536 µs) | 7.11 MiB against 7.01 MiB: +104 KiB (+104 KiB to +104 KiB) |
| add | 160, 160 in 10 s | means 9.86 ms against 10.8 ms: -977 µs (-3.23 ms to +1.27 ms) | 15.9 ms against 15.3 ms: +518 µs (+442 µs to +599 µs) | 7.16 MiB against 7.05 MiB: +108 KiB (+108 KiB to +108 KiB) |
| hypotenuse | 160, 160 in 11 s | means 11.7 ms against 8.40 ms: +3.32 ms (+1.08 ms to +5.56 ms) | 16.6 ms against 16.0 ms: +615 µs (+478 µs to +756 µs) | 7.16 MiB against 7.05 MiB: +112 KiB (+108 KiB to +112 KiB) |
| upcase | 160, 160 in 11 s | means 11.8 ms against 11.5 ms: +293 µs (-1.92 ms to +2.50 ms) | 18.1 ms against 17.4 ms: +776 µs (+661 µs to +897 µs) | 8.03 MiB against 7.91 MiB: +124 KiB (+124 KiB to +124 KiB) |
| upcase-stdin | 160, 160 in 19 s | means 35.1 ms against 34.1 ms: +977 µs (-837 µs to +2.79 ms) | 40.9 ms against 40.3 ms: +464 µs (+235 µs to +688 µs) | 7.16 MiB against 7.05 MiB: +116 KiB (+116 KiB to +116 KiB) |
| shout | 160, 160 in 11 s | means 10.4 ms against 10.2 ms: +195 µs (-2.16 ms to +2.55 ms) | 16.5 ms against 16.1 ms: +389 µs (+307 µs to +468 µs) | 7.93 MiB against 7.82 MiB: +120 KiB (+120 KiB to +120 KiB) |
| wc | 160, 160 in 20 s | means 38.3 ms against 38.0 ms: +293 µs (-1.94 ms to +2.52 ms) | 45.2 ms against 44.7 ms: +591 µs (+389 µs to +785 µs) | 7.18 MiB against 7.07 MiB: +116 KiB (+116 KiB to +116 KiB) |
| histogram | 160, 160 in 26 s | means 58.0 ms against 57.3 ms: +684 µs (-1.51 ms to +2.87 ms) | 65.1 ms against 64.5 ms: +529 µs (+343 µs to +722 µs) | 7.17 MiB against 7.06 MiB: +116 KiB (+112 KiB to +116 KiB) |
| crc32c | 160, 160 in 25 s | means 56.2 ms against 56.1 ms: +97.7 µs (-2.34 ms to +2.54 ms) | 62.4 ms against 61.9 ms: +557 µs (+392 µs to +729 µs) | 7.18 MiB against 7.06 MiB: +120 KiB (+120 KiB to +124 KiB) |
| sort | 96, 96 in 60 s | means 288 ms against 286 ms: +2.44 ms (-717 µs to +5.60 ms) | 297 ms against 296 ms: +576 µs (-152 µs to +1.34 ms) | 76.5 MiB against 76.3 MiB: +152 KiB (+148 KiB to +156 KiB) |
| tally | 156, 156 in 60 s | means 168 ms against 167 ms: +1.60 ms (-860 µs to +4.07 ms) | 175 ms against 174 ms: +799 µs (+225 µs to +1.37 ms) | 7.87 MiB against 7.71 MiB: +160 KiB (+160 KiB to +160 KiB) |
| server | 20, 20 in 67 s | means 1.53 s against 1.54 s: -10.2 ms (-24.8 ms to +4.48 ms) | 1.54 s against 1.55 s: -6.62 ms (-12.6 ms to +1.08 ms) | 7.55 MiB against 7.42 MiB: +116 KiB (+100 KiB to +140 KiB) |
| files | 144, 144 in 60 s | means 182 ms against 184 ms: -2.17 ms (-4.74 ms to +395 µs) | 190 ms against 191 ms: -1.14 ms (-2.05 ms to -193 µs) | 7.52 MiB against 7.42 MiB: +100 KiB (+100 KiB to +100 KiB) |
| readings | 160, 160 in 42 s | means 108 ms against 106 ms: +1.46 ms (-377 µs to +3.31 ms) | 114 ms against 114 ms: +497 µs (+370 µs to +624 µs) | 7.20 MiB against 7.08 MiB: +120 KiB (+120 KiB to +124 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
