# lengua against C, C++ and Rust on macos-aarch64

Measured on macos-aarch64 (Apple M1 (Virtual), 3 threads at once) by `cargo bench --bench against_rust_and_c`, at commit 610aaf5c622249762f3d80df3fa0947c3f71739e, in https://github.com/laughing-hermit/lengua/actions/runs/43191148225. Every number here is that run's: the bench writes this file from the run's results alone, target/bench/results-macos-aarch64.json in its artifacts.

Every version of a workload does the same work in its own language's way: lengua's program in the fastest form lengua's dialect allows, with SIMD128 written by hand where the work is bytes, and C, C++ and Rust with their standard libraries and plain loops, written for their compilers to vectorize, with no SIMD written by hand. Each workload below says how each version does its work.

Every version of a workload gets the same arguments, input and files, and gives the same output, checked before anything is measured. Each runs as people run it, in a process of its own, measured by `lengua-mcp measure`: its CPU time (in user space and in the kernel) and its peak resident memory as the system counts them for the process, its wall time on the clock, and the size of what ran. lengua's program runs through `lengua-run`, with every check a run makes; C, C++ and Rust compiled to WebAssembly run in lengua's host, by the same lengua-run in the same engine (wasmtime 50.0.1), each from its compiled form, kept under the user's key as a checked program's is, and with every check lengua's run makes: each has a lengua.lock of its own, signed with the same key, which records the module and lengua's idea, with the line that names an outside module added, and its run reads and checks the three as lengua's does, since a module lengua runs for a user always pays them (the last section gives the same modules without); their native builds, made on this runner, run as a shell starts them. Linux begins a process's count of its peak memory at what the process it was started from held, so there every version starts from lengua-spawn, a small process of lengua's that forks it and counts it, and writes the counts and the version's time on the clock for `measure`: the count begins at the few hundred KiB of lengua-spawn's copy, not at lengua-mcp's megabytes.

Size is a module's bytes on the rows against other modules, which run in the same lengua-run. Against a native build, lengua's size is its module's and that of the lengua-run it needs to run (4.81 MiB), and the native build's is its executable's alone: what it loads from the system's libraries, C's C library and C++'s where they are linked dynamically, isn't counted, and Rust's standard library, linked in, is.

The runs alternate: each round runs every version once, in an order shuffled from round to round, after each version's checked run and 2 rounds that aren't counted. On each row, lengua against one other version, the gate decides CPU and wall time with a one-sided Mann-Whitney U test at p < 0.01: it looks at the row after 20, 40, 80, 160, 320, 640, 1280, 2000 runs of each version, each look deciding at p < 0.00125 (0.01 split over the looks), until both of the row's tests decide, up to 2000 runs or 600 seconds. Peak memory, the median of the runs' peaks, and size are compared as measured.

The gate has a noise margin of 5%, as the runners are shared machines and a row this close changes sides from one run to the next. A measure is level when the test doesn't decide it (its p isn't below 0.00125), or when lengua's value is from 0.95 to 1.05 times the other version's, whatever the test decides; for peak memory, when the two medians are that close. It is ahead or behind only past the margin, and for CPU and wall time only when the test decides it too. Size is the same on every run and has no margin: a byte more is behind. KNOWN_BEHIND names the rows where lengua is behind; a level row needs no line there and doesn't fail the gate. Every ratio and every p is printed below, whatever the verdict.

The versions:

- lengua: lengua-run, run in wasmtime 50.0.1
- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native
- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all
- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17
- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions
- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols
- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

## The gate

lengua against each version, on CPU time, wall time, peak memory and size, in that order: ✓ lengua is ahead by more than 5%, ✗ behind by more than 5%, ≈ level (within 5%, or not decided), – not measured.

| workload | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|
| empty | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| add | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| hypotenuse | ✓ ✓ ✓ ✓ | ✓ ≈ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| upcase-stdin | ✓ ✓ ≈ ✗ | ✓ ✓ ≈ ✗ | ≈ ≈ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| shout | ≈ ≈ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| wc | ✗ ✗ ≈ ✓ | ✓ ✓ ≈ ✓ | ✗ ✗ ✓ ✓ | ✗ ✗ ✗ ✗ | ✓ ✓ ✗ ✗ | ✗ ✗ ✗ ✗ |
| histogram | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| crc32c | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| sort | ✓ ✓ ≈ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ≈ ✗ | ✓ ✓ ✓ ✗ | ✗ ✗ ✓ ✗ |
| tally | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ≈ ✓ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ | ✓ ✓ ✗ ✗ |
| server | ≈ ✓ ≈ ✓ | ≈ ✓ ≈ ✓ | ≈ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ≈ ✗ ✗ ✗ |
| files | ✓ ✓ ≈ ✓ | ≈ ≈ ≈ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |
| readings | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✓ ✓ ✓ ✓ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ | ✗ ✗ ✗ ✗ |

Where the run and KNOWN_BEHIND disagree:

- macos-aarch64 server c cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow
- macos-aarch64 server cpp cpu: lengua is behind by more than the margin, and KNOWN_BEHIND doesn't name the row: make lengua's program or its engine faster, where the list may not grow

## The native gaps

lengua's median over that of the best native build, C's, C++'s or Rust's, on each measure, its size with lengua-run's: above 1, lengua takes that many times as much.

| workload | CPU time | wall time | peak memory | size |
|---|---|---|---|---|
| empty | 2.08× | 1.74× | 2.54× | 299.71× |
| add | 2.15× | 2.01× | 2.56× | 149.78× |
| hypotenuse | 2.35× | 1.96× | 2.58× | 150.18× |
| upcase | 1.73× | 1.64× | 1.92× | 149.65× |
| upcase-stdin | 1.41× | 1.38× | 2.56× | 150.81× |
| shout | 1.89× | 1.72× | 1.89× | 150.42× |
| wc | 1.67× | 1.66× | 2.55× | 150.44× |
| histogram | 1.23× | 1.23× | 2.51× | 150.43× |
| crc32c | 2.40× | 2.35× | 2.50× | 150.43× |
| sort | 1.11× | 1.12× | 0.95× | 149.15× |
| tally | 0.46× | 0.46× | 2.19× | 148.74× |
| server | 1.47× | 3.17× | 2.81× | 148.38× |
| files | 1.70× | 1.69× | 3.00× | 149.22× |
| readings | 3.68× | 3.64× | 2.58× | 149.78× |

## Every row

Each measure's verdict (ahead, level, behind, or not measured), lengua's median over the other version's, and the test's p.

| workload | against | runs | CPU time | wall time | peak memory | size |
|---|---|---|---|---|---|---|
| empty | c-wasm | 2000 | level, 0.99×, p 0.011 | level, 0.99×, p 0.060 | level, 0.99× | ahead, 0.89× |
| empty | cpp-wasm | 2000 | level, 0.98×, p 2.8e-4 | level, 0.99×, p 0.005 | level, 0.99× | ahead, 0.89× |
| empty | rust-wasm | 320 | ahead, 0.90×, p 3.8e-8 | ahead, 0.92×, p 2.3e-6 | ahead, 0.92× | ahead, 0.01× |
| empty | c | 20 | behind, 2.08×, p 1.3e-6 | behind, 1.74×, p 3.3e-5 | behind, 2.51× | behind, 299.71× |
| empty | cpp | 20 | behind, 1.62×, p 1.6e-5 | behind, 1.33×, p 9.0e-5 | behind, 2.54× | behind, 299.71× |
| empty | rust | 20 | behind, 1.61×, p 2.3e-5 | behind, 1.38×, p 3.7e-5 | behind, 2.39× | behind, 17.65× |
| add | c-wasm | 160 | ahead, 0.90×, p 2.5e-6 | ahead, 0.89×, p 1.6e-5 | ahead, 0.94× | ahead, 0.04× |
| add | cpp-wasm | 320 | ahead, 0.90×, p 1.2e-7 | ahead, 0.93×, p 2.3e-6 | ahead, 0.94× | ahead, 0.04× |
| add | rust-wasm | 80 | ahead, 0.83×, p 2.6e-5 | ahead, 0.87×, p 3.0e-4 | ahead, 0.90× | ahead, 0.02× |
| add | c | 20 | behind, 1.95×, p 2.0e-6 | behind, 1.84×, p 1.2e-5 | behind, 2.56× | behind, 149.78× |
| add | cpp | 20 | behind, 2.15×, p 6.0e-7 | behind, 2.01×, p 5.2e-7 | behind, 2.53× | behind, 149.28× |
| add | rust | 20 | behind, 1.52×, p 4.1e-5 | behind, 1.50×, p 1.2e-4 | behind, 2.41× | behind, 16.68× |
| hypotenuse | c-wasm | 160 | ahead, 0.92×, p 1.5e-7 | ahead, 0.93×, p 2.6e-6 | ahead, 0.94× | ahead, 0.05× |
| hypotenuse | cpp-wasm | 160 | ahead, 0.93×, p 2.7e-5 | level, 0.95×, p 5.2e-4 | ahead, 0.94× | ahead, 0.05× |
| hypotenuse | rust-wasm | 40 | ahead, 0.90×, p 3.0e-6 | ahead, 0.90×, p 2.4e-5 | ahead, 0.91× | ahead, 0.03× |
| hypotenuse | c | 20 | behind, 2.35×, p 3.4e-8 | behind, 1.96×, p 3.4e-8 | behind, 2.54× | behind, 150.18× |
| hypotenuse | cpp | 20 | behind, 2.10×, p 9.6e-8 | behind, 1.89×, p 1.5e-7 | behind, 2.58× | behind, 149.96× |
| hypotenuse | rust | 20 | behind, 1.88×, p 5.3e-8 | behind, 1.72×, p 1.1e-7 | behind, 2.42× | behind, 16.68× |
| upcase | c-wasm | 1280 | level, 0.96×, p 1.4e-5 | level, 0.96×, p 1.6e-4 | level, 0.96× | ahead, 0.06× |
| upcase | cpp-wasm | 1280 | level, 0.95×, p 1.3e-6 | level, 0.96×, p 1.5e-5 | level, 0.96× | ahead, 0.06× |
| upcase | rust-wasm | 320 | ahead, 0.89×, p 2.3e-5 | ahead, 0.91×, p 1.2e-4 | ahead, 0.90× | ahead, 0.03× |
| upcase | c | 40 | behind, 1.73×, p 1.7e-7 | behind, 1.64×, p 5.5e-7 | behind, 1.92× | behind, 149.65× |
| upcase | cpp | 40 | behind, 1.66×, p 1.5e-7 | behind, 1.51×, p 2.4e-7 | behind, 1.92× | behind, 149.65× |
| upcase | rust | 40 | behind, 1.39×, p 1.6e-5 | behind, 1.35×, p 4.8e-5 | behind, 1.84× | behind, 16.69× |
| upcase-stdin | c-wasm | 40 | ahead, 0.91×, p 9.1e-5 | ahead, 0.91×, p 1.3e-4 | level, 1.00× | behind, 1.35× |
| upcase-stdin | cpp-wasm | 40 | ahead, 0.88×, p 1.1e-5 | ahead, 0.89×, p 1.6e-5 | level, 1.00× | behind, 1.32× |
| upcase-stdin | rust-wasm | 320 | level, 0.97×, p 9.9e-5 | level, 0.97×, p 1.2e-4 | ahead, 0.93× | ahead, 0.03× |
| upcase-stdin | c | 20 | behind, 1.41×, p 2.6e-5 | behind, 1.38×, p 4.1e-5 | behind, 2.53× | behind, 150.81× |
| upcase-stdin | cpp | 20 | behind, 1.37×, p 3.3e-6 | behind, 1.34×, p 3.8e-6 | behind, 2.56× | behind, 150.81× |
| upcase-stdin | rust | 20 | behind, 1.32×, p 6.3e-6 | behind, 1.30×, p 1.0e-5 | behind, 2.33× | behind, 16.69× |
| shout | c-wasm | 2000 | level, 0.99×, p 0.047 | level, 1.00×, p 0.131 | level, 1.00× | ahead, 0.40× |
| shout | cpp-wasm | 2000 | level, 0.99×, p 0.033 | level, 0.99×, p 0.135 | level, 1.00× | ahead, 0.40× |
| shout | rust-wasm | 80 | ahead, 0.89×, p 8.6e-7 | ahead, 0.89×, p 4.6e-6 | ahead, 0.93× | ahead, 0.03× |
| shout | c | 20 | behind, 1.89×, p 2.6e-7 | behind, 1.72×, p 9.0e-7 | behind, 1.89× | behind, 150.42× |
| shout | cpp | 20 | behind, 1.84×, p 3.4e-8 | behind, 1.69×, p 5.7e-8 | behind, 1.89× | behind, 150.42× |
| shout | rust | 20 | behind, 1.57×, p 8.3e-8 | behind, 1.46×, p 7.9e-7 | behind, 1.80× | behind, 16.69× |
| wc | c-wasm | 40 | behind, 1.20×, p 5.6e-5 | behind, 1.17×, p 7.6e-5 | level, 0.98× | ahead, 0.10× |
| wc | cpp-wasm | 20 | ahead, 0.14×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | level, 0.99× | ahead, 0.11× |
| wc | rust-wasm | 20 | behind, 1.16×, p 5.2e-5 | behind, 1.15×, p 9.0e-5 | ahead, 0.91× | ahead, 0.04× |
| wc | c | 20 | behind, 1.67×, p 3.4e-8 | behind, 1.66×, p 4.6e-8 | behind, 2.55× | behind, 150.44× |
| wc | cpp | 20 | ahead, 0.19×, p 3.4e-8 | ahead, 0.19×, p 3.4e-8 | behind, 2.52× | behind, 150.44× |
| wc | rust | 20 | behind, 1.64×, p 7.2e-8 | behind, 1.62×, p 8.3e-8 | behind, 2.33× | behind, 16.69× |
| histogram | c-wasm | 20 | ahead, 0.61×, p 3.4e-8 | ahead, 0.62×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | cpp-wasm | 20 | ahead, 0.62×, p 3.4e-8 | ahead, 0.63×, p 3.4e-8 | level, 0.98× | ahead, 0.09× |
| histogram | rust-wasm | 80 | ahead, 0.95×, p 4.3e-7 | ahead, 0.95×, p 8.1e-7 | ahead, 0.91× | ahead, 0.03× |
| histogram | c | 20 | behind, 1.15×, p 1.3e-7 | behind, 1.15×, p 1.5e-7 | behind, 2.48× | behind, 150.43× |
| histogram | cpp | 20 | behind, 1.16×, p 5.8e-5 | behind, 1.16×, p 5.8e-5 | behind, 2.51× | behind, 150.39× |
| histogram | rust | 20 | behind, 1.23×, p 5.5e-6 | behind, 1.23×, p 1.0e-5 | behind, 2.34× | behind, 16.68× |
| crc32c | c-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 0.97× | ahead, 0.08× |
| crc32c | cpp-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.68×, p 3.4e-8 | level, 0.97× | ahead, 0.08× |
| crc32c | rust-wasm | 20 | ahead, 0.63×, p 3.4e-8 | ahead, 0.64×, p 3.4e-8 | ahead, 0.93× | ahead, 0.04× |
| crc32c | c | 20 | behind, 2.40×, p 3.4e-8 | behind, 2.35×, p 3.4e-8 | behind, 2.50× | behind, 150.43× |
| crc32c | cpp | 20 | behind, 2.30×, p 3.4e-8 | behind, 2.27×, p 3.4e-8 | behind, 2.50× | behind, 150.43× |
| crc32c | rust | 20 | behind, 2.37×, p 3.4e-8 | behind, 2.33×, p 3.4e-8 | behind, 2.36× | behind, 16.69× |
| sort | c-wasm | 20 | ahead, 0.15×, p 3.4e-8 | ahead, 0.15×, p 3.4e-8 | level, 1.00× | ahead, 0.27× |
| sort | cpp-wasm | 20 | ahead, 0.59×, p 3.4e-8 | ahead, 0.59×, p 3.4e-8 | ahead, 0.90× | ahead, 0.11× |
| sort | rust-wasm | 20 | ahead, 0.67×, p 3.4e-8 | ahead, 0.67×, p 3.4e-8 | ahead, 0.90× | ahead, 0.07× |
| sort | c | 20 | ahead, 0.66×, p 3.4e-8 | ahead, 0.66×, p 3.4e-8 | level, 0.95× | behind, 149.15× |
| sort | cpp | 20 | ahead, 0.77×, p 1.7e-7 | ahead, 0.77×, p 1.7e-7 | ahead, 0.79× | behind, 144.70× |
| sort | rust | 160 | behind, 1.11×, p 8.5e-6 | behind, 1.12×, p 3.0e-5 | ahead, 0.93× | behind, 16.69× |
| tally | c-wasm | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | level, 1.03× | ahead, 0.20× |
| tally | cpp-wasm | 20 | ahead, 0.42×, p 3.4e-8 | ahead, 0.42×, p 3.4e-8 | level, 1.00× | ahead, 0.12× |
| tally | rust-wasm | 20 | ahead, 0.34×, p 3.4e-8 | ahead, 0.34×, p 3.4e-8 | level, 1.04× | ahead, 0.08× |
| tally | c | 20 | ahead, 0.46×, p 3.4e-8 | ahead, 0.46×, p 3.4e-8 | behind, 2.15× | behind, 148.74× |
| tally | cpp | 20 | ahead, 0.45×, p 3.4e-8 | ahead, 0.45×, p 3.4e-8 | behind, 2.19× | behind, 144.12× |
| tally | rust | 20 | ahead, 0.43×, p 3.4e-8 | ahead, 0.43×, p 3.4e-8 | behind, 2.01× | behind, 15.83× |
| server | c-wasm | 38 | level, 1.08×, p 0.139 | ahead, 0.66×, p 9.8e-9 | level, 0.98× | ahead, 0.12× |
| server | cpp-wasm | 38 | level, 1.03×, p 0.408 | ahead, 0.63×, p 1.5e-8 | level, 0.96× | ahead, 0.06× |
| server | rust-wasm | 38 | level, 1.07×, p 0.093 | ahead, 0.68×, p 2.3e-9 | ahead, 0.91× | ahead, 0.03× |
| server | c | 20 | behind, 1.47×, p 8.0e-6 | behind, 2.92×, p 7.2e-8 | behind, 2.81× | behind, 148.38× |
| server | cpp | 20 | behind, 1.44×, p 9.0e-6 | behind, 3.02×, p 5.3e-8 | behind, 2.81× | behind, 143.46× |
| server | rust | 38 | level, 0.98×, p 0.163 | behind, 3.17×, p 5.3e-13 | behind, 1.33× | behind, 15.78× |
| files | c-wasm | 20 | ahead, 0.94×, p 8.9e-4 | ahead, 0.94×, p 0.001 | level, 0.97× | ahead, 0.05× |
| files | cpp-wasm | 80 | level, 0.96×, p 1.7e-5 | level, 0.96×, p 2.6e-5 | level, 0.97× | ahead, 0.05× |
| files | rust-wasm | 20 | ahead, 0.95×, p 4.6e-4 | ahead, 0.95×, p 6.1e-4 | ahead, 0.92× | ahead, 0.03× |
| files | c | 20 | behind, 1.62×, p 3.4e-8 | behind, 1.61×, p 3.4e-8 | behind, 3.00× | behind, 149.22× |
| files | cpp | 20 | behind, 1.62×, p 4.6e-7 | behind, 1.61×, p 4.6e-7 | behind, 3.00× | behind, 149.22× |
| files | rust | 20 | behind, 1.70×, p 3.4e-8 | behind, 1.69×, p 3.4e-8 | behind, 2.65× | behind, 16.68× |
| readings | c-wasm | 40 | ahead, 0.93×, p 2.1e-4 | ahead, 0.90×, p 9.0e-4 | ahead, 0.94× | ahead, 0.04× |
| readings | cpp-wasm | 160 | ahead, 0.93×, p 2.9e-7 | ahead, 0.94×, p 1.1e-6 | ahead, 0.94× | ahead, 0.04× |
| readings | rust-wasm | 160 | ahead, 0.92×, p 1.3e-6 | ahead, 0.92×, p 2.4e-6 | ahead, 0.89× | ahead, 0.02× |
| readings | c | 20 | behind, 2.94×, p 3.4e-8 | behind, 2.92×, p 3.4e-8 | behind, 2.58× | behind, 149.78× |
| readings | cpp | 20 | behind, 3.07×, p 3.4e-8 | behind, 3.03×, p 3.4e-8 | behind, 2.54× | behind, 148.40× |
| readings | rust | 20 | behind, 3.68×, p 3.4e-8 | behind, 3.64×, p 3.4e-8 | behind, 2.42× | behind, 16.68× |

## empty

Starts and exits with code 0: all of it is starting and ending a process, which no program avoids, the floor under every workload that starts one. lengua's program: tests/fixtures/empty.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 320 | 20 | 20 | 20 |
| CPU time | 4.24 ms | 4.29 ms | 4.32 ms | 4.75 ms | 1.88 ms | 2.40 ms | 2.42 ms |
| wall time | 4.90 ms | 4.97 ms | 4.97 ms | 5.44 ms | 2.52 ms | 3.31 ms | 3.19 ms |
| peak memory | 3.09 MiB | 3.11 MiB | 3.11 MiB | 3.36 MiB | 1.23 MiB | 1.22 MiB | 1.30 MiB |
| size | 319 B | 357 B | 357 B | 33.8 KiB | 16.4 KiB | 16.4 KiB | 279 KiB |

Its rounds took 83 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/empty

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## add

Parses two whole numbers from its arguments and prints their sum: almost all of it is starting up and exiting. lengua's program: tests/fixtures/add.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 160 | 320 | 80 | 20 | 20 | 20 |
| CPU time | 4.23 ms | 4.72 ms | 4.70 ms | 5.40 ms | 2.29 ms | 2.07 ms | 2.94 ms |
| wall time | 4.88 ms | 5.31 ms | 5.27 ms | 6.08 ms | 2.88 ms | 2.62 ms | 3.53 ms |
| peak memory | 3.20 MiB | 3.41 MiB | 3.41 MiB | 3.56 MiB | 1.25 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.22 KiB | 30.2 KiB | 29.0 KiB | 54.5 KiB | 32.9 KiB | 33.0 KiB | 295 KiB |

Its rounds took 13 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/add

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## hypotenuse

Prints the hypotenuse of two decimal sides from its arguments, rounded to three decimals: a square root, and almost all of it starting up and exiting. lengua's program: examples/hypotenuse.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 160 | 160 | 40 | 20 | 20 | 20 |
| CPU time | 3.42 ms | 3.73 ms | 3.67 ms | 3.68 ms | 1.41 ms | 1.57 ms | 1.76 ms |
| wall time | 3.90 ms | 4.20 ms | 4.10 ms | 4.13 ms | 1.91 ms | 1.99 ms | 2.19 ms |
| peak memory | 3.22 MiB | 3.42 MiB | 3.42 MiB | 3.56 MiB | 1.27 MiB | 1.25 MiB | 1.33 MiB |
| size | 1.52 KiB | 28.4 KiB | 28.9 KiB | 56.3 KiB | 32.8 KiB | 32.9 KiB | 296 KiB |

Its rounds took 6 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/hypotenuse

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase

Copies a 1 MiB text file to another with its ASCII letters in capitals: a file opened, read, made and written, and one pass over the bytes. lengua's program changes 16 bytes a step with SIMD128, written by hand; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: examples/upcase.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 1280 | 1280 | 1280 | 320 | 40 | 40 | 40 |
| CPU time | 4.90 ms | 5.11 ms | 5.14 ms | 7.00 ms | 4.60 ms | 4.81 ms | 5.73 ms |
| wall time | 5.43 ms | 5.63 ms | 5.68 ms | 7.63 ms | 5.32 ms | 5.78 ms | 6.48 ms |
| peak memory | 4.31 MiB | 4.52 MiB | 4.52 MiB | 4.78 MiB | 2.25 MiB | 2.25 MiB | 2.34 MiB |
| size | 1.93 KiB | 33.4 KiB | 33.4 KiB | 59.0 KiB | 32.9 KiB | 32.9 KiB | 296 KiB |

Its rounds took 59 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/upcase

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## upcase-stdin

Copies 64 MiB of standard input to standard output with its ASCII letters in capitals, a piece at a time. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/upcase-stdin.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 320 | 40 | 40 | 320 | 20 | 20 | 20 |
| CPU time | 12.3 ms | 14.4 ms | 14.7 ms | 12.6 ms | 8.84 ms | 9.11 ms | 9.44 ms |
| wall time | 12.8 ms | 14.8 ms | 15.2 ms | 13.2 ms | 9.39 ms | 9.67 ms | 10.0 ms |
| peak memory | 3.28 MiB | 3.28 MiB | 3.28 MiB | 3.53 MiB | 1.30 MiB | 1.28 MiB | 1.41 MiB |
| size | 1.20 KiB | 912 B | 929 B | 35.6 KiB | 32.7 KiB | 32.7 KiB | 295 KiB |

Its rounds took 17 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/upcase-stdin

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## shout

Reads 1 MiB of standard input whole, then prints it with its ASCII letters in capitals. lengua's program changes 64 bytes a step with SIMD128, written by hand, then 16; C, C++ and Rust store every byte in a plain loop, for their compilers to vectorize. lengua's program: tests/fixtures/shout.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 2000 | 2000 | 2000 | 80 | 20 | 20 | 20 |
| CPU time | 3.71 ms | 3.73 ms | 3.76 ms | 3.59 ms | 2.10 ms | 2.15 ms | 2.52 ms |
| wall time | 4.17 ms | 4.18 ms | 4.21 ms | 4.03 ms | 2.57 ms | 2.62 ms | 3.02 ms |
| peak memory | 4.22 MiB | 4.23 MiB | 4.23 MiB | 4.52 MiB | 2.23 MiB | 2.23 MiB | 2.34 MiB |
| size | 1.24 KiB | 3.07 KiB | 3.09 KiB | 40.5 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 62 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## wc

Counts the lines and words of 64 MiB of standard input, as `wc -lw` does. lengua's program compares 16 bytes a step with SIMD128, written by hand, and adds each comparison's lanes to counters of a byte, widened every 255 steps; C, C++ and Rust compare each byte and the byte before it in a plain loop, counted 255 bytes at a time in counters of a byte, for their compilers to vectorize. lengua's program: examples/count.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 40 | 40 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 28.6 ms | 23.9 ms | 211 ms | 25.9 ms | 18.1 ms | 159 ms | 18.3 ms |
| wall time | 29.1 ms | 24.9 ms | 212 ms | 26.9 ms | 18.7 ms | 160 ms | 19.2 ms |
| peak memory | 3.34 MiB | 3.41 MiB | 3.39 MiB | 3.66 MiB | 1.31 MiB | 1.33 MiB | 1.44 MiB |
| size | 1.91 KiB | 18.4 KiB | 17.9 KiB | 53.9 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 14 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, examples/count

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## histogram

Counts each byte value of 64 MiB of standard input. Every version counts in four tables, so that neighboring bytes don't wait on one counter: C, C++ and Rust four bytes a step, lengua's program eight, each byte loaded on its own. lengua's program: tests/fixtures/histogram.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 20 | 80 | 20 | 20 | 20 |
| CPU time | 32.4 ms | 55.5 ms | 54.8 ms | 34.2 ms | 29.6 ms | 29.3 ms | 27.7 ms |
| wall time | 32.9 ms | 56.0 ms | 55.3 ms | 34.7 ms | 30.1 ms | 29.9 ms | 28.2 ms |
| peak memory | 3.31 MiB | 3.41 MiB | 3.41 MiB | 3.66 MiB | 1.34 MiB | 1.33 MiB | 1.42 MiB |
| size | 1.58 KiB | 18.1 KiB | 18.1 KiB | 55.5 KiB | 32.8 KiB | 32.8 KiB | 296 KiB |

Its rounds took 12 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/histogram

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## crc32c

The CRC-32C of 64 MiB of standard input. Natively, C, C++ and Rust use the CPU's CRC instruction; WebAssembly has none, so the modules take the bytes through tables: C, C++ and Rust eight bytes a step through eight tables, lengua's program sixteen through sixteen. lengua's program: tests/fixtures/crc32c.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 38.8 ms | 57.8 ms | 57.8 ms | 61.4 ms | 16.2 ms | 16.9 ms | 16.4 ms |
| wall time | 39.5 ms | 58.3 ms | 58.3 ms | 62.0 ms | 16.8 ms | 17.4 ms | 16.9 ms |
| peak memory | 3.32 MiB | 3.41 MiB | 3.41 MiB | 3.58 MiB | 1.33 MiB | 1.33 MiB | 1.41 MiB |
| size | 1.61 KiB | 19.1 KiB | 19.1 KiB | 39.6 KiB | 32.8 KiB | 32.8 KiB | 295 KiB |

Its rounds took 8 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/crc32c

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=apple-m1

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=apple-m1 -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## sort

Sorts the lines of 64 MiB of standard input in byte order, as `LC_ALL=C sort` does. Each version sorts with its own language's sort: lengua's program finds the lines 16 bytes a step from a bitmask of the newlines, gives each a record of 8 bytes, its start below 4 bytes of the line as a key that compares as one number, and orders the records in place with a multikey quicksort without recursion, whose two splits around a median of three keys take no branch on a record, whose lines that are the same as the pivot go on with their next 4 bytes, and which sorts a part with a heap sort once it has been split below or above a pivot more than twice as many times as the lines' count has bits, as introsort does; it prints the lines 16 bytes a step with SIMD128, written by hand. C sorts with its C library's qsort (wasi-libc's in WebAssembly), C++ with std::sort and Rust with sort_unstable, each comparing lines with its library's memcmp or comparison of slices. Every version prints through a 64 KiB buffer. lengua's program: tests/fixtures/sort-large.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 20 | 20 | 20 | 20 | 20 | 160 |
| CPU time | 243 ms | 1.41 s | 370 ms | 323 ms | 331 ms | 281 ms | 219 ms |
| wall time | 247 ms | 1.44 s | 371 ms | 324 ms | 333 ms | 283 ms | 220 ms |
| peak memory | 72.9 MiB | 72.9 MiB | 81.0 MiB | 81.1 MiB | 76.3 MiB | 92.4 MiB | 78.4 MiB |
| size | 3.84 KiB | 14.4 KiB | 33.6 KiB | 53.3 KiB | 33.1 KiB | 34.1 KiB | 295 KiB |

Its rounds took 152 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/sort-large

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## tally

Counts each different word of 64 MiB of standard input, 4,096 different words from common to rare, and prints them from the most frequent. C and C++ count in a table of FNV-1a with linear probing and Rust in its HashMap with FNV-1a, each passing over the bytes and hashing them a byte at a time. lengua's program finds where each word starts and ends 64 bytes a step from SIMD128's bitmasks, written by hand, and counts in a table with linear probing whose entries hold a word's first 8 bytes as its key: a word of up to 8 bytes is hashed with one multiplication and matched as one number. lengua's program: tests/fixtures/tally.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
| CPU time | 116 ms | 268 ms | 275 ms | 342 ms | 252 ms | 257 ms | 266 ms |
| wall time | 116 ms | 269 ms | 275 ms | 342 ms | 252 ms | 258 ms | 267 ms |
| peak memory | 4.23 MiB | 4.12 MiB | 4.22 MiB | 4.06 MiB | 1.97 MiB | 1.94 MiB | 2.11 MiB |
| size | 4.38 KiB | 21.6 KiB | 37.5 KiB | 55.0 KiB | 33.2 KiB | 34.2 KiB | 312 KiB |

Its rounds took 43 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/tally

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## server

A server: 64 clients at once each send 1,000 lines of 64 bytes, one at a time, and each line comes back in capitals before the next goes. Its wall time is from the first connection to the last answer. lengua's program: tests/fixtures/shout-server.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 38 | 38 | 38 | 38 | 20 | 20 | 38 |
| CPU time | 643 ms | 596 ms | 624 ms | 601 ms | 510 ms | 520 ms | 655 ms |
| wall time | 2.42 s | 3.66 s | 3.86 s | 3.55 s | 856 ms | 827 ms | 766 ms |
| peak memory | 3.47 MiB | 3.53 MiB | 3.62 MiB | 3.83 MiB | 1.23 MiB | 1.23 MiB | 2.61 MiB |
| size | 1.82 KiB | 15.7 KiB | 31.1 KiB | 54.0 KiB | 33.2 KiB | 34.4 KiB | 312 KiB |

Its rounds took 661 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/shout-server

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## files

Reads each of the 4,096 small files of its folder, of 1 to 4,096 bytes, to its end, and prints how many files and bytes it read. A read of a file gives every byte it asks for up to the file's end, so lengua's program takes a read that gives less than its 64 KiB buffer as a file's last, one read for each of these files; C, C++ and Rust read until a read gives nothing, two reads a file. lengua's program: tests/fixtures/files.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 80 | 20 | 80 | 20 | 20 | 20 | 20 |
| CPU time | 78.4 ms | 84.1 ms | 82.0 ms | 83.6 ms | 48.9 ms | 48.9 ms | 46.5 ms |
| wall time | 79.0 ms | 84.6 ms | 82.7 ms | 84.1 ms | 49.5 ms | 49.6 ms | 47.1 ms |
| peak memory | 3.92 MiB | 4.06 MiB | 4.06 MiB | 4.30 MiB | 1.31 MiB | 1.31 MiB | 1.48 MiB |
| size | 1.71 KiB | 37.7 KiB | 37.7 KiB | 62.1 KiB | 33.0 KiB | 33.0 KiB | 296 KiB |

Its rounds took 23 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/files

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## readings

Reads the clock that only goes forward a million times. lengua's program: tests/fixtures/readings.

| | lengua | c-wasm | cpp-wasm | rust-wasm | c | cpp | rust |
|---|---|---|---|---|---|---|---|
| counted runs | 160 | 40 | 160 | 160 | 20 | 20 | 20 |
| CPU time | 111 ms | 122 ms | 118 ms | 120 ms | 37.6 ms | 36.1 ms | 30.0 ms |
| wall time | 111 ms | 127 ms | 119 ms | 121 ms | 38.2 ms | 36.8 ms | 30.7 ms |
| peak memory | 3.22 MiB | 3.41 MiB | 3.42 MiB | 3.61 MiB | 1.25 MiB | 1.27 MiB | 1.33 MiB |
| size | 1.21 KiB | 30.3 KiB | 28.8 KiB | 59.9 KiB | 32.9 KiB | 33.2 KiB | 296 KiB |

Its rounds took 72 seconds.

- lengua: lengua-run of lengua 0.2.0, wasmtime 50.0.1, tests/fixtures/readings

- c-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all

- cpp-wasm: wasi-sdk 34.0, clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project 895aa2c896ada719451be2e3673c83da8ddf1141); clang++ --target=wasm32-wasip1 --sysroot=/home/runner/wasi-sdk/share/wasi-sysroot -O3 -flto -msimd128 -Wl,--max-memory=1073741824 -Wl,-z,stack-size=1048576 -Wl,--strip-all -fno-exceptions

- rust-wasm: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 --target wasm32-wasip1 -C opt-level=3 -C codegen-units=1 -C lto=fat -C panic=abort -C target-feature=+simd128 -C strip=symbols -C link-arg=--max-memory=1073741824

- c: Apple clang version 21.0.0 (clang-2100.1.1.101); cc -O3 -flto -Wl,-x -mcpu=native

- cpp: Apple clang version 21.0.0 (clang-2100.1.1.101); c++ -O3 -flto -Wl,-x -mcpu=native -std=gnu++17

- rust: rustc 1.99.1 (58cf8f9c3 2026-10-13); rustc --edition 2024 -C opt-level=3 -C target-cpu=native -C codegen-units=1 -C lto=fat -C panic=abort -C strip=symbols

## lengua's run apart from its module

After the gate's rounds, each workload's lengua program ran against lengua-module: the very module that `lengua-run` runs, built by lengua from the same main.wat and run by `lengua-mcp measure` as the other modules are, by the same program, in the same engine, from its compiled form kept under the same key and with the words of the program's idea. The two ran alone, in rounds of their own, in an order shuffled from round to round, after 2 rounds that weren't counted, until each had 160 runs, or for 60 seconds once each had 20. Each cell gives lengua's median against lengua-module's, then the shift from lengua-module's runs to lengua's: the median of every difference of a lengua run less a lengua-module run (Hodges and Lehmann's estimate), with its 95% interval (Mann-Whitney's normal bound). The shift is what lengua's run does before and around running its module.

| workload | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|
| empty | 160, 160 in 10 s | 4.14 ms against 4.52 ms: +1.00 µs (-266 µs to +234 µs) | 4.75 ms against 5.24 ms: -13.2 µs (-323 µs to +251 µs) | 3.11 MiB against 3.00 MiB: +112 KiB (+112 KiB to +112 KiB) |
| add | 160, 160 in 6 s | 2.87 ms against 2.65 ms: +220 µs (+135 µs to +314 µs) | 3.22 ms against 2.98 ms: +242 µs (+146 µs to +347 µs) | 3.20 MiB against 3.09 MiB: +112 KiB (+112 KiB to +112 KiB) |
| hypotenuse | 160, 160 in 11 s | 4.86 ms against 4.24 ms: +530 µs (+253 µs to +811 µs) | 5.53 ms against 4.94 ms: +563 µs (+251 µs to +890 µs) | 3.23 MiB against 3.09 MiB: +128 KiB (+128 KiB to +128 KiB) |
| upcase | 160, 160 in 8 s | 4.49 ms against 4.22 ms: +344 µs (+180 µs to +505 µs) | 4.99 ms against 4.67 ms: +369 µs (+176 µs to +550 µs) | 4.31 MiB against 4.16 MiB: +176 KiB (+176 KiB to +176 KiB) |
| upcase-stdin | 160, 160 in 15 s | 13.0 ms against 12.7 ms: +234 µs (-3.00 µs to +471 µs) | 13.6 ms against 13.2 ms: +261 µs (-12.3 µs to +539 µs) | 3.28 MiB against 3.17 MiB: +112 KiB (+112 KiB to +112 KiB) |
| shout | 160, 160 in 12 s | 5.73 ms against 5.97 ms: +30.0 µs (-327 µs to +361 µs) | 6.54 ms against 6.75 ms: +22.4 µs (-377 µs to +414 µs) | 4.22 MiB against 4.09 MiB: +128 KiB (+128 KiB to +128 KiB) |
| wc | 160, 160 in 55 s | 28.0 ms against 27.3 ms: +686 µs (-9.00 µs to +1.35 ms) | 28.5 ms against 28.2 ms: +622 µs (-113 µs to +1.38 ms) | 3.34 MiB against 3.19 MiB: +160 KiB (+160 KiB to +160 KiB) |
| histogram | 160, 160 in 42 s | 35.2 ms against 34.2 ms: +394 µs (-337 µs to +1.24 ms) | 35.9 ms against 35.1 ms: +356 µs (-442 µs to +1.24 ms) | 3.31 MiB against 3.17 MiB: +144 KiB (+144 KiB to +144 KiB) |
| crc32c | 160, 160 in 46 s | 37.9 ms against 37.6 ms: -32.0 µs (-680 µs to +636 µs) | 38.4 ms against 38.1 ms: -21.4 µs (-709 µs to +666 µs) | 3.31 MiB against 3.19 MiB: +128 KiB (+128 KiB to +144 KiB) |
| sort | 25, 25 in 60 s | 199 ms against 198 ms: +1.23 ms (-6.09 ms to +8.61 ms) | 200 ms against 198 ms: +1.27 ms (-6.46 ms to +9.18 ms) | 72.9 MiB against 72.7 MiB: +176 KiB (+160 KiB to +176 KiB) |
| tally | 51, 51 in 61 s | 111 ms against 111 ms: +897 µs (-872 µs to +2.55 ms) | 112 ms against 111 ms: +926 µs (-839 µs to +2.58 ms) | 4.23 MiB against 4.06 MiB: +176 KiB (+176 KiB to +176 KiB) |
| server | 20, 20 in 336 s | 673 ms against 727 ms: -28.7 ms (-122 ms to +65.3 ms) | 2.67 s against 2.89 s: -225 ms (-581 ms to +176 ms) | 3.47 MiB against 3.33 MiB: +144 KiB (+128 KiB to +144 KiB) |
| files | 125, 125 in 60 s | 82.4 ms against 80.4 ms: -387 µs (-2.38 ms to +1.68 ms) | 83.0 ms against 81.2 ms: -276 µs (-2.40 ms to +1.83 ms) | 3.92 MiB against 3.70 MiB: +224 KiB (+208 KiB to +224 KiB) |
| readings | 89, 89 in 61 s | 117 ms against 117 ms: +254 µs (-3.83 ms to +4.50 ms) | 118 ms against 120 ms: -148 µs (-4.43 ms to +4.58 ms) | 3.22 MiB against 3.09 MiB: +128 KiB (+128 KiB to +128 KiB) |

The runs are lengua's and lengua-module's counted runs, in that order, and how long their counted rounds took.

## The other modules without lengua's checks

In the gate's rows, C's, C++'s and Rust's modules run as lengua's program does, and as any module lengua runs for a user does: each run reads lengua.lock and holds it to the user's key, reads and hashes the idea and the module, and takes what the module may reach from the idea. Here the same modules ran with none of that, by the words of lengua's idea alone, in the rounds of the section above, beside lengua's program, which made its checks. Each cell gives the module's median, then lengua's median over it: above 1, lengua took that many times as much. No verdict is drawn from these, and the gate doesn't read them.

| workload | against, unchecked | runs | CPU time | wall time | peak memory |
|---|---|---|---|---|---|
| empty | c-wasm | 160 | 4.14 ms, 1.00× | 4.76 ms, 1.00× | 3.00 MiB, 1.04× |
| empty | cpp-wasm | 160 | 4.01 ms, 1.03× | 4.67 ms, 1.02× | 3.00 MiB, 1.04× |
| empty | rust-wasm | 160 | 4.65 ms, 0.89× | 5.39 ms, 0.88× | 3.20 MiB, 0.97× |
| add | c-wasm | 160 | 2.73 ms, 1.05× | 3.08 ms, 1.05× | 3.27 MiB, 0.98× |
| add | cpp-wasm | 160 | 2.74 ms, 1.05× | 3.09 ms, 1.04× | 3.27 MiB, 0.98× |
| add | rust-wasm | 160 | 2.80 ms, 1.02× | 3.17 ms, 1.02× | 3.39 MiB, 0.94× |
| hypotenuse | c-wasm | 160 | 4.37 ms, 1.11× | 4.97 ms, 1.11× | 3.25 MiB, 1.00× |
| hypotenuse | cpp-wasm | 160 | 4.73 ms, 1.03× | 5.33 ms, 1.04× | 3.25 MiB, 1.00× |
| hypotenuse | rust-wasm | 160 | 4.60 ms, 1.06× | 5.32 ms, 1.04× | 3.39 MiB, 0.95× |
| upcase | c-wasm | 160 | 4.21 ms, 1.07× | 4.65 ms, 1.07× | 4.33 MiB, 1.00× |
| upcase | cpp-wasm | 160 | 4.23 ms, 1.06× | 4.66 ms, 1.07× | 4.33 MiB, 1.00× |
| upcase | rust-wasm | 160 | 4.39 ms, 1.02× | 4.86 ms, 1.03× | 4.56 MiB, 0.95× |
| upcase-stdin | c-wasm | 160 | 13.6 ms, 0.95× | 14.1 ms, 0.96× | 3.19 MiB, 1.03× |
| upcase-stdin | cpp-wasm | 160 | 14.7 ms, 0.88× | 15.3 ms, 0.89× | 3.19 MiB, 1.03× |
| upcase-stdin | rust-wasm | 160 | 12.8 ms, 1.01× | 13.4 ms, 1.01× | 3.38 MiB, 0.97× |
| shout | c-wasm | 160 | 5.46 ms, 1.05× | 6.14 ms, 1.06× | 4.12 MiB, 1.02× |
| shout | cpp-wasm | 160 | 5.72 ms, 1.00× | 6.37 ms, 1.03× | 4.12 MiB, 1.02× |
| shout | rust-wasm | 160 | 5.80 ms, 0.99× | 6.61 ms, 0.99× | 4.34 MiB, 0.97× |
| wc | c-wasm | 160 | 23.3 ms, 1.20× | 23.9 ms, 1.19× | 3.25 MiB, 1.03× |
| wc | cpp-wasm | 160 | 195 ms, 0.14× | 196 ms, 0.15× | 3.25 MiB, 1.03× |
| wc | rust-wasm | 160 | 22.6 ms, 1.24× | 23.2 ms, 1.23× | 3.48 MiB, 0.96× |
| histogram | c-wasm | 160 | 54.6 ms, 0.64× | 55.5 ms, 0.65× | 3.25 MiB, 1.02× |
| histogram | cpp-wasm | 160 | 56.2 ms, 0.63× | 56.6 ms, 0.63× | 3.25 MiB, 1.02× |
| histogram | rust-wasm | 160 | 35.6 ms, 0.99× | 36.1 ms, 0.99× | 3.48 MiB, 0.95× |
| crc32c | c-wasm | 160 | 58.4 ms, 0.65× | 58.8 ms, 0.65× | 3.25 MiB, 1.02× |
| crc32c | cpp-wasm | 160 | 58.0 ms, 0.65× | 58.5 ms, 0.66× | 3.25 MiB, 1.02× |
| crc32c | rust-wasm | 160 | 60.9 ms, 0.62× | 61.5 ms, 0.62× | 3.42 MiB, 0.97× |
| sort | c-wasm | 25 | 1.26 s, 0.16× | 1.26 s, 0.16× | 72.7 MiB, 1.00× |
| sort | cpp-wasm | 25 | 336 ms, 0.59× | 336 ms, 0.59× | 80.8 MiB, 0.90× |
| sort | rust-wasm | 25 | 309 ms, 0.65× | 309 ms, 0.65× | 80.9 MiB, 0.90× |
| tally | c-wasm | 51 | 270 ms, 0.41× | 271 ms, 0.41× | 3.97 MiB, 1.07× |
| tally | cpp-wasm | 51 | 282 ms, 0.40× | 283 ms, 0.40× | 4.05 MiB, 1.05× |
| tally | rust-wasm | 51 | 343 ms, 0.32× | 344 ms, 0.33× | 3.88 MiB, 1.09× |
| server | c-wasm | 20 | 704 ms, 0.96× | 3.82 s, 0.70× | 3.38 MiB, 1.03× |
| server | cpp-wasm | 20 | 713 ms, 0.94× | 3.68 s, 0.73× | 3.47 MiB, 1.00× |
| server | rust-wasm | 20 | 735 ms, 0.92× | 3.61 s, 0.74× | 3.64 MiB, 0.95× |
| files | c-wasm | 125 | 84.8 ms, 0.97× | 85.6 ms, 0.97× | 3.91 MiB, 1.00× |
| files | cpp-wasm | 125 | 84.7 ms, 0.97× | 85.2 ms, 0.97× | 3.91 MiB, 1.00× |
| files | rust-wasm | 125 | 85.2 ms, 0.97× | 86.1 ms, 0.96× | 4.11 MiB, 0.95× |
| readings | c-wasm | 89 | 125 ms, 0.94× | 126 ms, 0.93× | 3.28 MiB, 0.98× |
| readings | cpp-wasm | 89 | 123 ms, 0.95× | 124 ms, 0.95× | 3.28 MiB, 0.98× |
| readings | rust-wasm | 89 | 125 ms, 0.94× | 126 ms, 0.94× | 3.44 MiB, 0.94× |
