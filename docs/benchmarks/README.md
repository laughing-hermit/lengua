# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[43084459708](https://github.com/laughing-hermit/lengua/actions/runs/43084459708), at commit
45b507b (branch `wip/lean-runner`, the first run with lengua-run, W19). GitHub didn't keep that
run's files for Windows on ARM64, so its report is from the same commit's run
[43099764273](https://github.com/laughing-hermit/lengua/actions/runs/43099764273) of that runner
alone. The bench writes each report from the run's results alone, the
`target/bench/results-<runner>.json` it keeps in the run's artifacts, so every number in a report
is the run's. The files are copied here byte for byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 7763, 4 threads at once |
| Linux on ARM64 (`ubuntu-24.04-arm`) | [BENCHMARKS-linux-aarch64.md](BENCHMARKS-linux-aarch64.md) | CPU implementer 0x41, part 0xd49, 4 threads at once |
| macOS on ARM64 (`macos-26`) | [BENCHMARKS-macos-aarch64.md](BENCHMARKS-macos-aarch64.md) | Apple M1 (Virtual), 3 threads at once |
| Windows on x86-64 (`windows-2025`) | [BENCHMARKS-windows-x86_64.md](BENCHMARKS-windows-x86_64.md) | Intel Xeon Platinum 8573C, 4 threads at once |
| Windows on ARM64 (`windows-11-arm`) | [BENCHMARKS-windows-aarch64.md](BENCHMARKS-windows-aarch64.md) | Cobalt 100, 4 threads at once |

## The margin

Since run 42964232679 the gate has a noise margin of 5% (the owner's decision of 2026-11-11;
`MARGIN` in
`src/gate.rs`). A measure is **level** when the test doesn't decide it, or when lengua's value is
from 0.95 to 1.05 times the other version's, whatever the test decides. It is **ahead** or
**behind** only past the margin, and for CPU and wall time only when the test decides it too.
Size has no margin. KNOWN_BEHIND names the rows behind; a level row needs no line and doesn't
fail the gate. The reports mark a level row with ≈ and still print every ratio and every p.

## What changed in what is measured

Since this run every version that is WebAssembly runs by lengua-run, the small program a user
runs a checked program with, which has no compiler in it: lengua's program with every check a
run makes, and C's, C++'s and Rust's modules by the same program in the same engine. Each runs
from its compiled form, kept under the user's key. Before, each ran by lengua-mcp, from
Wasmtime's cache. Against a native build, lengua's size is now its module's and lengua-run's.
The empty program, lengua's medians in the run before (42964232679) and in this one:

| Runner | CPU time | Wall time | Peak memory | The program that runs it |
|---|---|---|---|---|
| Linux on x86-64 | 2.20 ms to 1.33 ms | 2.03 ms to 1.42 ms | 11.4 MiB to 5.50 MiB | 23.1 MiB to 5.87 MiB |
| Linux on ARM64 | 2.98 ms to 1.43 ms | 10.5 ms to 1.52 ms | 8.08 MiB to 4.34 MiB | 17.4 MiB to 4.88 MiB |
| macOS on ARM64 | 6.75 ms to 4.26 ms | 7.31 ms to 4.93 ms | 4.94 MiB to 3.02 MiB | 16.9 MiB to 4.62 MiB |
| Windows on x86-64 | in 15.6 ms ticks | 14.1 ms to 8.00 ms | 10.2 MiB to 5.80 MiB | 25.4 MiB to 6.03 MiB |
| Windows on ARM64 | in 15.6 ms ticks | 21.0 ms to 15.9 ms | 11.6 MiB to 7.12 MiB | 18.8 MiB to 4.91 MiB |

## What the gate found

The run measured the list as it was (898 rows), so the gate failed on every runner. Counted from
each report's section "Where the run and KNOWN_BEHIND disagree":

| Runner | Named rows now level | Named rows now ahead | Rows behind that the list doesn't name |
|---|---|---|---|
| Linux on x86-64 | 21 | 0 | 3 |
| Linux on ARM64 | 13 | 2 | 9 |
| macOS on ARM64 | 6 | 0 | 11 |
| Windows on x86-64 | 50 | 0 | 3 |
| Windows on ARM64 | 45 | 0 | 0 |
| All five | 135 | 2 | 26 |

- The 137 named rows that are level or ahead have left KNOWN_BEHIND, which now names 761 rows:
  154 on Linux on x86-64, 154 on Linux on ARM64, 162 on macOS on ARM64, 141 on Windows on x86-64
  and 150 on Windows on ARM64. Of them 24 are against C, C++ and Rust compiled to WebAssembly,
  and the rest against their native builds.
- The 26 rows behind that the list doesn't name stay off it, since it may not grow. All but
  five are CPU or wall time of a workload that is mostly starting up, against C's or C++'s
  module. Those modules now start in about the time lengua's does, less the 0.1 ms lengua's own
  checks take, where the run before had every version start in 2 ms or more. Each row's ratio is
  in its report, under "Every row":
  - Linux on x86-64: `empty c-wasm wall`, `empty cpp-wasm wall`, `shout cpp-wasm wall`;
  - Linux on ARM64: `empty c-wasm wall`; `hypotenuse` against c-wasm and cpp-wasm, cpu and wall;
    `shout c-wasm cpu`, `shout c-wasm wall`, `shout cpp-wasm cpu`; `upcase c-wasm cpu`;
  - macOS on ARM64: `empty c-wasm cpu`, `empty c-wasm wall`, `empty cpp-wasm wall`;
    `shout c-wasm wall`, `shout cpp-wasm wall`; `upcase` against c-wasm and cpp-wasm, cpu and
    wall; `tally cpp-wasm memory`, `tally rust-wasm memory`;
  - Windows on x86-64: `histogram rust-wasm cpu`, `server c wall`, `upcase-stdin cpp cpu`.
- Of the 6 such rows of the run before, two are level or ahead now,
  `linux-x86_64 upcase-stdin c-wasm memory` and `windows-x86_64 wc cpp wall`, and four are
  among the 26: the two `shout` CPU rows on Linux on ARM64 and the two `upcase` CPU rows on
  macOS on ARM64.
- With the list as it is now, these same results pass on Windows on ARM64 and fail on the other
  four runners, on those 26 rows alone. No run has measured the new list yet; this is counted
  from the reports.
- Every native C++ build was made, on all five runners.

The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
