# lengua against C, C++ and Rust on windows-x86_64

Measured on windows-x86_64 (INTEL(R) XEON(R) PLATINUM 8573C, 4 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 45b507bf262dab89bff38a1f9c35bdd5590c8965, in https://github.com/laughing-hermit/lengua/actions/runs/43084459708. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-windows-x86_64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is; their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (6.03 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

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
| hypotenuse | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| upcase-stdin | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✗ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ≈ ✗ ✗ ✗ | ≈ ✓ ✗ ✗ | ≈ ✗ ✗ ✗ |
| histogram | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✗ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✓ ✓ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| files | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ≈ ≈ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- windows-x86_64 empty c-wasm memory: lengua is level now, and KNOWN_BEHIND line 553 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 554 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 555 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty c cpu: lengua is level now, and KNOWN_BEHIND line 556 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty cpp cpu: lengua is level now, and KNOWN_BEHIND line 560 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 empty rust cpu: lengua is level now, and KNOWN_BEHIND line 564 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add c-wasm memory: lengua is level now, and KNOWN_BEHIND line 568 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 569 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 570 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add c cpu: lengua is level now, and KNOWN_BEHIND line 571 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add cpp cpu: lengua is level now, and KNOWN_BEHIND line 575 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 add rust cpu: lengua is level now, and KNOWN_BEHIND line 579 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse c-wasm memory: lengua is level now, and KNOWN_BEHIND line 583 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 584 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 hypotenuse rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 585 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase c-wasm memory: lengua is level now, and KNOWN_BEHIND line 598 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 599 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 600 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase c cpu: lengua is level now, and KNOWN_BEHIND line 601 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase cpp cpu: lengua is level now, and KNOWN_BEHIND line 605 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase rust cpu: lengua is level now, and KNOWN_BEHIND line 609 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin c-wasm memory: lengua is level now, and KNOWN_BEHIND line 613 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 615 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 617 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 upcase-stdin cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 shout c-wasm memory: lengua is level now, and KNOWN_BEHIND line 629 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 630 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 shout rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 631 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc c-wasm memory: lengua is level now, and KNOWN_BEHIND line 644 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 645 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 646 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc c cpu: lengua is level now, and KNOWN_BEHIND line 647 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc cpp cpu: lengua is level now, and KNOWN_BEHIND line 651 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 wc rust cpu: lengua is level now, and KNOWN_BEHIND line 654 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 histogram c-wasm memory: lengua is level now, and KNOWN_BEHIND line 658 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 histogram cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 659 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 histogram rust-wasm cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 histogram rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 660 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 crc32c c-wasm memory: lengua is level now, and KNOWN_BEHIND line 673 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 crc32c cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 674 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 crc32c rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 675 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 tally c-wasm memory: lengua is level now, and KNOWN_BEHIND line 691 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 tally cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 692 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server c-wasm memory: lengua is level now, and KNOWN_BEHIND line 700 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 701 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 702 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 server c wall: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- windows-x86_64 files c-wasm memory: lengua is level now, and KNOWN_BEHIND line 714 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 files cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 715 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 files rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 716 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 readings c-wasm memory: lengua is level now, and KNOWN_BEHIND line 729 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 readings cpp-wasm memory: lengua is level now, and KNOWN_BEHIND line 730 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks
- windows-x86_64 readings rust-wasm memory: lengua is level now, and KNOWN_BEHIND line 731 names the row: take it out, as the list names only rows behind by more than the margin, and only shrinks

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | – | 1.79× | 1.72× | 59.06× |
| add | – | 1.81× | 1.72× | 51.87× |
| hypotenuse | – | 1.81× | 1.71× | 51.02× |
| upcase | – | 1.65× | 1.54× | 49.79× |
| upcase-stdin | 2.00× | 1.41× | 1.70× | 56.89× |
| shout | – | 1.65× | 1.52× | 54.39× |
| wc | 1.00× | 1.10× | 1.70× | 51.66× |
| histogram | 1.33× | 1.28× | 1.69× | 52.09× |
| crc32c | 1.50× | 1.85× | 1.69× | 55.12× |
| sort | 0.62× | 0.61× | 0.96× | 51.46× |
| tally | 0.50× | 0.49× | 1.65× | 48.06× |
| server | 1.31× | 2.01× | 1.57× | 52.54× |
| files | 1.75× | 1.79× | 1.73× | 49.58× |
| readings | 6.00× | 3.94× | 1.72× | 49.78× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, p 0.066 | level, 1.05×, p 5.6e-203 | level, 1.02× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, p 0.048 | level, 1.05×, p 3.3e-199 | level, 1.02× | ahead, 0.89× |
| empty | rust-wasm | 2000 | level, p 0.287 | level, 1.01×, p 3.2e-20 | level, 1.02× | ahead, 0.01× |
| empty | c | 40 | level, p 0.001 | behind, 1.77×, p 7.2e-15 | behind, 1.72× | behind, 55.11× |
| empty | cpp | 160 | level, p 0.001 | behind, 1.79×, p 2.9e-54 | behind, 1.72× | behind, 55.11× |
| empty | rust | 320 | level, p 0.001 | behind, 1.59×, p 1.6e-106 | behind, 1.55× | behind, 59.06× |
| add | c-wasm | 2000 | level, p 0.338 | level, 1.02×, p 3.6e-92 | level, 1.02× | ahead, 0.04× |
| add | cpp-wasm | 2000 | level, p 0.209 | level, 1.03×, p 5.2e-93 | level, 1.02× | ahead, 0.04× |
| add | rust-wasm | 2000 | level, p 0.306 | level, 1.01×, p 2.1e-7 | level, 1.01× | ahead, 0.02× |
| add | c | 40 | level, p 1.3e-4 | behind, 1.81×, p 7.2e-15 | behind, 1.72× | behind, 41.85× |
| add | cpp | 80 | level, p 6.8e-6 | behind, 1.83×, p 9.1e-27 | behind, 1.72× | behind, 41.99× |
| add | rust | 80 | level, p 7.1e-4 | behind, 1.60×, p 4.7e-28 | behind, 1.55× | behind, 51.87× |
| hypotenuse | c-wasm | 2000 | level, p 0.043 | level, 1.03×, p 3.7e-190 | level, 1.02× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 2000 | level, p 0.414 | level, 1.03×, p 1.8e-191 | level, 1.02× | ahead, 0.05× |
| hypotenuse | rust-wasm | 2000 | level, p 0.150 | level, 1.01×, p 4.5e-19 | level, 1.01× | ahead, 0.03× |
| hypotenuse | c | 40 | behind, p 3.2e-4 | behind, 1.81×, p 7.2e-15 | behind, 1.71× | behind, 41.57× |
| hypotenuse | cpp | 40 | behind, p 7.9e-4 | behind, 1.80×, p 7.2e-15 | behind, 1.71× | behind, 41.43× |
| hypotenuse | rust | 40 | behind, p 1.2e-4 | behind, 1.61×, p 7.2e-15 | behind, 1.55× | behind, 51.02× |
| upcase | c-wasm | 2000 | level, 1.00×, p 0.407 | level, 1.04×, p 0.0e0 | level, 1.01× | ahead, 0.06× |
| upcase | cpp-wasm | 2000 | level, 1.00×, p 0.356 | level, 1.04×, p 0.0e0 | level, 1.01× | ahead, 0.06× |
| upcase | rust-wasm | 2000 | level, 1.00×, p 0.495 | level, 1.02×, p 1.3e-150 | level, 0.99× | ahead, 0.03× |
| upcase | c | 160 | level, p 4.0e-6 | behind, 1.65×, p 2.9e-54 | behind, 1.51× | behind, 39.45× |
| upcase | cpp | 80 | level, p 7.6e-5 | behind, 1.65×, p 4.7e-28 | behind, 1.51× | behind, 39.45× |
| upcase | rust | 160 | level, p 2.8e-4 | behind, 1.55×, p 2.9e-54 | behind, 1.54× | behind, 49.79× |
| upcase-stdin | c-wasm | 2000 | level, 1.00×, p 0.028 | level, 1.01×, p 7.9e-26 | level, 1.02× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 2000 | level, 1.00×, p 0.204 | level, 1.01×, p 2.3e-14 | level, 1.02× | behind, 1.32× |
| upcase-stdin | rust-wasm | 2000 | level, 1.00×, p 0.282 | level, 1.01×, p 3.0e-8 | level, 1.02× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 2.00×, p 2.3e-6 | behind, 1.41×, p 3.4e-8 | behind, 1.70× | behind, 53.21× |
| upcase-stdin | cpp | 20 | behind, 2.00×, p 1.4e-4 | behind, 1.40×, p 3.4e-8 | behind, 1.70× | behind, 53.21× |
| upcase-stdin | rust | 40 | behind, 2.00×, p 6.8e-7 | behind, 1.31×, p 7.2e-15 | behind, 1.53× | behind, 56.89× |
| shout | c-wasm | 2000 | level, p 0.218 | level, 1.04×, p 0.0e0 | level, 1.02× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, p 0.387 | level, 1.04×, p 0.0e0 | level, 1.02× | ahead, 0.40× |
| shout | rust-wasm | 2000 | level, p 0.268 | level, 1.01×, p 4.1e-71 | level, 0.99× | ahead, 0.03× |
| shout | c | 80 | behind, p 2.9e-5 | behind, 1.63×, p 4.7e-28 | behind, 1.51× | behind, 51.87× |
| shout | cpp | 80 | behind, p 8.8e-5 | behind, 1.65×, p 4.7e-28 | behind, 1.51× | behind, 51.87× |
| shout | rust | 80 | behind, p 1.1e-4 | behind, 1.51×, p 4.7e-28 | behind, 1.52× | behind, 54.39× |
| wc | c-wasm | 80 | ahead, 0.67×, p 8.2e-6 | ahead, 0.80×, p 4.7e-28 | level, 1.01× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.14×, p 8.7e-9 | ahead, 0.16×, p 3.4e-8 | level, 1.01× | ahead, 0.11× |
| wc | rust-wasm | 20 | ahead, 0.67×, p 3.0e-4 | ahead, 0.79×, p 3.4e-8 | level, 1.01× | ahead, 0.04× |
| wc | c | 80 | level, 1.00×, p 2.6e-4 | behind, 1.10×, p 1.0e-19 | behind, 1.69× | behind, 41.43× |
| wc | cpp | 640 | level, 1.00×, p 4.7e-8 | ahead, 0.92×, p 1.1e-175 | behind, 1.70× | behind, 41.29× |
| wc | rust | 160 | level, 1.00×, p 2.4e-5 | behind, 1.06×, p 3.1e-36 | behind, 1.53× | behind, 51.66× |
| histogram | c-wasm | 320 | level, 1.00×, p 1.8e-4 | level, 0.95×, p 1.6e-56 | level, 1.01× | ahead, 0.09× |
| histogram | cpp-wasm | 160 | level, 1.00×, p 8.3e-5 | level, 0.95×, p 3.4e-27 | level, 1.01× | ahead, 0.09× |
| histogram | rust-wasm | 320 | behind, 1.33×, p 9.7e-5 | level, 1.04×, p 1.4e-41 | level, 1.02× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.33×, p 5.0e-7 | behind, 1.27×, p 3.4e-8 | behind, 1.68× | behind, 41.71× |
| histogram | cpp | 20 | behind, 1.33×, p 5.0e-7 | behind, 1.28×, p 3.4e-8 | behind, 1.69× | behind, 41.71× |
| histogram | rust | 20 | behind, 1.33×, p 2.7e-6 | behind, 1.19×, p 3.4e-8 | behind, 1.53× | behind, 52.09× |
| crc32c | c-wasm | 20 | ahead, 0.75×, p 8.5e-4 | ahead, 0.77×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.75×, p 3.4e-4 | ahead, 0.77×, p 3.4e-8 | level, 1.01× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.60×, p 1.9e-4 | ahead, 0.73×, p 3.4e-8 | level, 1.02× | ahead, 0.04× |
| crc32c | c | 20 | behind, 1.50×, p 9.1e-9 | behind, 1.85×, p 3.4e-8 | behind, 1.69× | behind, 41.71× |
| crc32c | cpp | 20 | behind, 1.50×, p 7.5e-9 | behind, 1.84×, p 3.4e-8 | behind, 1.69× | behind, 41.71× |
| crc32c | rust | 20 | behind, 1.50×, p 2.1e-8 | behind, 1.76×, p 3.4e-8 | behind, 1.54× | behind, 55.12× |
| sort | c-wasm | 20 | ahead, 0.18×, p 2.3e-8 | ahead, 0.18×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.50×, p 1.8e-8 | ahead, 0.49×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.55×, p 2.1e-8 | ahead, 0.55×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.39×, p 1.9e-8 | ahead, 0.39×, p 3.4e-8 | level, 0.96× | behind, 51.46× |
| sort | cpp | 20 | ahead, 0.44×, p 1.9e-8 | ahead, 0.43×, p 3.4e-8 | ahead, 0.83× | behind, 45.41× |
| sort | rust | 20 | ahead, 0.62×, p 1.8e-8 | ahead, 0.61×, p 3.4e-8 | ahead, 0.89× | behind, 51.25× |
| tally | c-wasm | 20 | ahead, 0.36×, p 1.9e-8 | ahead, 0.35×, p 3.4e-8 | level, 1.04× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.36×, p 1.7e-8 | ahead, 0.35×, p 3.4e-8 | level, 1.05× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.30×, p 1.9e-8 | ahead, 0.29×, p 3.4e-8 | behind, 1.08× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.47×, p 1.3e-8 | ahead, 0.46×, p 3.4e-8 | behind, 1.60× | behind, 40.63× |
| tally | cpp | 20 | ahead, 0.50×, p 2.2e-8 | ahead, 0.49×, p 3.4e-8 | behind, 1.63× | behind, 39.34× |
| tally | rust | 20 | ahead, 0.41×, p 2.3e-8 | ahead, 0.40×, p 3.4e-8 | behind, 1.65× | behind, 48.06× |
| server | c-wasm | 91 | level, 1.00×, p 0.213 | ahead, 0.88×, p 1.1e-31 | level, 1.03× | ahead, 0.12× |
| server | cpp-wasm | 91 | level, 1.00×, p 0.229 | ahead, 0.86×, p 1.1e-31 | level, 1.02× | ahead, 0.06× |
| server | rust-wasm | 91 | level, 0.99×, p 0.095 | ahead, 0.87×, p 1.1e-31 | level, 1.00× | ahead, 0.03× |
| server | c | 20 | behind, 1.06×, p 9.2e-8 | behind, 1.08×, p 4.6e-8 | behind, 1.57× | behind, 52.54× |
| server | cpp | 20 | behind, 1.06×, p 8.5e-8 | behind, 1.07×, p 3.4e-8 | behind, 1.57× | behind, 51.44× |
| server | rust | 20 | behind, 1.31×, p 2.9e-8 | behind, 2.01×, p 3.4e-8 | behind, 1.19× | behind, 43.78× |
| files | c-wasm | 160 | level, 1.00×, p 0.001 | level, 0.97×, p 2.6e-10 | level, 1.03× | ahead, 0.05× |
| files | cpp-wasm | 160 | level, 1.00×, p 1.7e-4 | level, 0.97×, p 7.3e-12 | level, 1.03× | ahead, 0.05× |
| files | rust-wasm | 80 | ahead, 0.91×, p 2.7e-4 | level, 0.97×, p 1.0e-6 | level, 1.01× | ahead, 0.03× |
| files | c | 20 | behind, 1.75×, p 2.0e-8 | behind, 1.79×, p 3.4e-8 | behind, 1.73× | behind, 39.70× |
| files | cpp | 20 | behind, 1.75×, p 2.2e-8 | behind, 1.73×, p 3.4e-8 | behind, 1.73× | behind, 39.70× |
| files | rust | 20 | behind, 1.75×, p 2.3e-8 | behind, 1.74×, p 3.4e-8 | behind, 1.59× | behind, 49.58× |
| readings | c-wasm | 160 | level, 1.00×, p 1.5e-5 | level, 0.97×, p 2.5e-16 | level, 1.02× | ahead, 0.04× |
| readings | cpp-wasm | 160 | level, 1.00×, p 2.0e-6 | level, 0.98×, p 2.2e-11 | level, 1.02× | ahead, 0.04× |
| readings | rust-wasm | 80 | ahead, 0.86×, p 6.2e-5 | level, 0.96×, p 5.8e-18 | level, 1.00× | ahead, 0.02× |
| readings | c | 20 | behind, 6.00×, p 1.0e-8 | behind, 3.94×, p 3.4e-8 | behind, 1.72× | behind, 41.85× |
| readings | cpp | 20 | behind, 3.00×, p 7.8e-9 | behind, 3.71×, p 3.4e-8 | behind, 1.71× | behind, 41.99× |
| readings | rust | 20 | behind, 2.00×, p 9.1e-9 | behind, 2.38×, p 3.4e-8 | behind, 1.54× | behind, 49.78× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 160 | 320 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 8.00 ms | 7.63 ms | 7.63 ms | 7.91 ms | 4.18 ms | 4.17 ms | 4.76 ms |
| peak memory | 5.80 MiB | 5.70 MiB | 5.70 MiB | 5.71 MiB | 3.36 MiB | 3.36 MiB | 3.73 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 112 KiB | 112 KiB | 104 KiB |

Its rounds took 162 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 80 | 80 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 8.41 ms | 8.21 ms | 8.20 ms | 8.36 ms | 4.43 ms | 4.44 ms | 5.09 ms |
| peak memory | 5.84 MiB | 5.75 MiB | 5.75 MiB | 5.77 MiB | 3.41 MiB | 3.40 MiB | 3.77 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 148 KiB | 147 KiB | 119 KiB |

Its rounds took 168 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 40 | 40 | 40 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 8.30 ms | 8.04 ms | 8.05 ms | 8.22 ms | 4.69 ms | 4.71 ms | 5.27 ms |
| peak memory | 5.84 MiB | 5.74 MiB | 5.74 MiB | 5.79 MiB | 3.41 MiB | 3.41 MiB | 3.78 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 148 KiB | 149 KiB | 121 KiB |

Its rounds took 164 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 160 | 80 | 160 |
| CPU time | 15.6 ms | 15.6 ms | 15.6 ms | 15.6 ms | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 10.1 ms | 9.68 ms | 9.68 ms | 9.88 ms | 6.23 ms | 6.24 ms | 6.61 ms |
| peak memory | 6.79 MiB | 6.70 MiB | 6.70 MiB | 6.82 MiB | 4.49 MiB | 4.49 MiB | 4.41 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 186 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 20 | 20 | 40 |
| CPU time | 31.2 ms | 31.2 ms | 31.2 ms | 31.2 ms | 15.6 ms | 15.6 ms | 15.6 ms |
| wall time | 31.6 ms | 31.2 ms | 31.3 ms | 31.3 ms | 21.3 ms | 21.4 ms | 23.1 ms |
| peak memory | 5.84 MiB | 5.74 MiB | 5.74 MiB | 5.74 MiB | 3.44 MiB | 3.44 MiB | 3.81 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 116 KiB | 116 KiB | 108 KiB |

Its rounds took 357 seconds.

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
| counted runs | 2000 | 2000 | 2000 | 2000 | 80 | 80 | 80 |
| CPU time | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns | 0.00 ns |
| wall time | 8.65 ms | 8.28 ms | 8.28 ms | 8.52 ms | 5.35 ms | 5.27 ms | 5.76 ms |
| peak memory | 6.62 MiB | 6.49 MiB | 6.49 MiB | 6.70 MiB | 4.38 MiB | 4.38 MiB | 4.36 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 119 KiB | 119 KiB | 114 KiB |

Its rounds took 165 seconds.

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
| counted runs | 640 | 80 | 20 | 20 | 80 | 640 | 160 |
| CPU time | 31.2 ms | 46.9 ms | 219 ms | 46.9 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 35.1 ms | 44.3 ms | 228 ms | 44.7 ms | 32.0 ms | 37.9 ms | 33.4 ms |
| peak memory | 5.87 MiB | 5.79 MiB | 5.79 MiB | 5.80 MiB | 3.46 MiB | 3.46 MiB | 3.84 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 149 KiB | 150 KiB | 120 KiB |

Its rounds took 86 seconds.

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
| counted runs | 320 | 320 | 160 | 320 | 20 | 20 | 20 |
| CPU time | 62.5 ms | 62.5 ms | 62.5 ms | 46.9 ms | 46.9 ms | 46.9 ms | 46.9 ms |
| wall time | 58.4 ms | 61.2 ms | 61.5 ms | 56.2 ms | 45.8 ms | 45.5 ms | 49.1 ms |
| peak memory | 5.85 MiB | 5.79 MiB | 5.79 MiB | 5.75 MiB | 3.47 MiB | 3.47 MiB | 3.82 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 148 KiB | 148 KiB | 118 KiB |

Its rounds took 87 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 46.9 ms | 62.5 ms | 62.5 ms | 78.1 ms | 31.2 ms | 31.2 ms | 31.2 ms |
| wall time | 53.5 ms | 69.6 ms | 69.8 ms | 72.9 ms | 28.9 ms | 29.1 ms | 30.4 ms |
| peak memory | 5.85 MiB | 5.79 MiB | 5.79 MiB | 5.76 MiB | 3.46 MiB | 3.46 MiB | 3.79 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 148 KiB | 148 KiB | 112 KiB |

Its rounds took 10 seconds.

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
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 242 ms | 1.33 s | 484 ms | 438 ms | 625 ms | 547 ms | 391 ms |
| wall time | 240 ms | 1.33 s | 486 ms | 435 ms | 623 ms | 555 ms | 392 ms |
| peak memory | 75.2 MiB | 75.1 MiB | 83.1 MiB | 83.3 MiB | 78.4 MiB | 90.4 MiB | 84.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 120 KiB | 136 KiB | 120 KiB |

Its rounds took 97 seconds.

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
| CPU time | 141 ms | 391 ms | 391 ms | 469 ms | 297 ms | 281 ms | 344 ms |
| wall time | 139 ms | 394 ms | 392 ms | 480 ms | 299 ms | 284 ms | 348 ms |
| peak memory | 6.56 MiB | 6.30 MiB | 6.28 MiB | 6.10 MiB | 4.10 MiB | 4.04 MiB | 3.99 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 152 KiB | 157 KiB | 128 KiB |

Its rounds took 56 seconds.

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
| counted runs | 91 | 91 | 91 | 91 | 20 | 20 | 20 |
| CPU time | 1.19 s | 1.19 s | 1.19 s | 1.20 s | 1.11 s | 1.11 s | 898 ms |
| wall time | 1.21 s | 1.38 s | 1.40 s | 1.39 s | 1.11 s | 1.12 s | 594 ms |
| peak memory | 6.29 MiB | 6.13 MiB | 6.17 MiB | 6.29 MiB | 4.00 MiB | 3.99 MiB | 5.27 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 118 KiB | 120 KiB | 141 KiB |

Its rounds took 628 seconds.

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
| counted runs | 160 | 160 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 156 ms | 156 ms | 156 ms | 172 ms | 93.8 ms | 93.8 ms | 93.8 ms |
| wall time | 158 ms | 163 ms | 163 ms | 166 ms | 92.1 ms | 95.2 ms | 94.4 ms |
| peak memory | 6.23 MiB | 6.07 MiB | 6.07 MiB | 6.19 MiB | 3.60 MiB | 3.60 MiB | 3.92 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 156 KiB | 156 KiB | 124 KiB |

Its rounds took 111 seconds.

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
| counted runs | 160 | 160 | 160 | 80 | 20 | 20 | 20 |
| CPU time | 93.8 ms | 93.8 ms | 93.8 ms | 109 ms | 15.6 ms | 31.2 ms | 46.9 ms |
| wall time | 98.4 ms | 101 ms | 101 ms | 104 ms | 25.4 ms | 27.0 ms | 42.0 ms |
| peak memory | 5.84 MiB | 5.75 MiB | 5.75 MiB | 5.82 MiB | 3.40 MiB | 3.41 MiB | 3.79 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 148 KiB | 147 KiB | 124 KiB |

Its rounds took 68 seconds.

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
| empty | 160, 160 in 6 s | means 8.20 ms against 6.93 ms: +1.27 ms (-1.11 ms to +3.65 ms) | 8.00 ms against 7.69 ms: +330 µs (+280 µs to +383 µs) | 5.80 MiB against 5.71 MiB: +96.0 KiB (+92.0 KiB to +96.0 KiB) |
| add | 160, 160 in 7 s | means 8.01 ms against 7.42 ms: +586 µs (-1.80 ms to +2.97 ms) | 8.44 ms against 8.00 ms: +422 µs (+381 µs to +462 µs) | 5.83 MiB against 5.73 MiB: +104 KiB (+104 KiB to +108 KiB) |
| hypotenuse | 160, 160 in 7 s | means 7.32 ms against 7.71 ms: -391 µs (-2.61 ms to +1.83 ms) | 8.47 ms against 8.04 ms: +411 µs (+376 µs to +450 µs) | 5.84 MiB against 5.73 MiB: +112 KiB (+108 KiB to +112 KiB) |
| upcase | 160, 160 in 7 s | means 9.67 ms against 8.89 ms: +781 µs (-1.64 ms to +3.20 ms) | 9.65 ms against 9.08 ms: +561 µs (+514 µs to +608 µs) | 6.79 MiB against 6.69 MiB: +100 KiB (+100 KiB to +104 KiB) |
| upcase-stdin | 160, 160 in 14 s | means 31.2 ms against 31.3 ms: -97.7 µs (-2.08 ms to +1.89 ms) | 31.5 ms against 31.1 ms: +360 µs (+236 µs to +480 µs) | 5.84 MiB against 5.74 MiB: +104 KiB (+104 KiB to +104 KiB) |
| shout | 160, 160 in 7 s | means 8.79 ms against 7.91 ms: +879 µs (-1.56 ms to +3.32 ms) | 8.79 ms against 8.41 ms: +370 µs (+321 µs to +420 µs) | 6.63 MiB against 6.52 MiB: +104 KiB (+104 KiB to +108 KiB) |
| wc | 160, 160 in 15 s | means 33.5 ms against 34.2 ms: -684 µs (-2.46 ms to +1.10 ms) | 34.5 ms against 34.1 ms: +435 µs (+314 µs to +558 µs) | 5.87 MiB against 5.75 MiB: +116 KiB (+116 KiB to +120 KiB) |
| histogram | 160, 160 in 23 s | means 59.8 ms against 57.9 ms: +1.86 ms (-679 µs to +4.39 ms) | 58.7 ms against 58.3 ms: +344 µs (-72.4 µs to +781 µs) | 5.85 MiB against 5.74 MiB: +112 KiB (+108 KiB to +112 KiB) |
| crc32c | 160, 160 in 21 s | means 50.8 ms against 53.3 ms: -2.54 ms (-4.93 ms to -150 µs) | 52.8 ms against 52.4 ms: +433 µs (+250 µs to +615 µs) | 5.85 MiB against 5.74 MiB: +116 KiB (+112 KiB to +116 KiB) |
| sort | 111, 111 in 61 s | means 257 ms against 257 ms: -141 µs (-3.78 ms to +3.50 ms) | 257 ms against 258 ms: -3.60 µs (-1.65 ms to +1.54 ms) | 75.2 MiB against 75.0 MiB: +140 KiB (+136 KiB to +140 KiB) |
| tally | 160, 160 in 49 s | means 138 ms against 138 ms: +195 µs (-2.18 ms to +2.57 ms) | 138 ms against 137 ms: +988 µs (+492 µs to +1.49 ms) | 6.57 MiB against 6.43 MiB: +144 KiB (+144 KiB to +148 KiB) |
| server | 21, 21 in 61 s | means 1.24 s against 1.23 s: +8.93 ms (-10.3 ms to +28.1 ms) | 1.30 s against 1.30 s: +449 µs (-18.1 ms to +17.9 ms) | 6.29 MiB against 6.19 MiB: +96.0 KiB (+88.0 KiB to +104 KiB) |
| files | 160, 160 in 56 s | means 156 ms against 157 ms: -1.27 ms (-3.86 ms to +1.32 ms) | 157 ms against 156 ms: +353 µs (-541 µs to +1.20 ms) | 6.22 MiB against 6.13 MiB: +96.0 KiB (+92.0 KiB to +96.0 KiB) |
| readings | 160, 160 in 33 s | means 91.6 ms against 90.2 ms: +1.37 ms (-1.02 ms to +3.76 ms) | 91.5 ms against 91.2 ms: +329 µs (-448 µs to +1.04 ms) | 5.84 MiB against 5.74 MiB: +104 KiB (+104 KiB to +104 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.
