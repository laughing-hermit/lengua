# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[42964232679](https://github.com/laughing-hermit/lengua/actions/runs/42964232679), at commit
cd54e79 (branch `wip/gate-margin`, the first run with the gate's noise margin). It writes each
report from the run's results alone, the `target/bench/results-<runner>.json` it keeps in the
run's artifacts, so every number in a report is the run's. The files are copied here byte for
byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
| Linux on ARM64 (`ubuntu-24.04-arm`) | [BENCHMARKS-linux-aarch64.md](BENCHMARKS-linux-aarch64.md) | CPU implementer 0x41, part 0xd49, 4 threads at once |
| macOS on ARM64 (`macos-26`) | [BENCHMARKS-macos-aarch64.md](BENCHMARKS-macos-aarch64.md) | Apple M1 (Virtual), 3 threads at once |
| Windows on x86-64 (`windows-2025`) | [BENCHMARKS-windows-x86_64.md](BENCHMARKS-windows-x86_64.md) | Intel Xeon Platinum 8370C, 4 threads at once |
| Windows on ARM64 (`windows-11-arm`) | [BENCHMARKS-windows-aarch64.md](BENCHMARKS-windows-aarch64.md) | Cobalt 100, 4 threads at once |

## The margin

Since this run the gate has a noise margin of 5% (the owner's decision of 2026-11-11; `MARGIN` in
`src/gate.rs`). A measure is **level** when the test doesn't decide it, or when lengua's value is
from 0.95 to 1.05 times the other version's, whatever the test decides. It is **ahead** or
**behind** only past the margin, and for CPU and wall time only when the test decides it too.
Size has no margin. KNOWN_BEHIND names the rows behind; a level row needs no line and doesn't
fail the gate. The reports mark a level row with ≈ and still print every ratio and every p.

## What the gate found

The run measured the list as it was before the margin (1,037 rows), so the gate failed on every
runner: most of what it found is rows the list named that are level now. Counted from each
report's section "Where the run and KNOWN_BEHIND disagree":

| Runner | Named rows now level | Named rows now ahead | Rows behind that the list doesn't name | Native C++ builds that failed |
|---|---|---|---|---|
| Linux on x86-64 | 25 | 0 | 1 | 0 |
| Linux on ARM64 | 16 | 0 | 2 | 0 |
| macOS on ARM64 | 32 | 0 | 2 | 0 |
| Windows on x86-64 | 28 | 7 | 1 | 0 |
| Windows on ARM64 | 26 | 5 | 0 | 0 |
| All five | 127 | 12 | 6 | 0 |

- The 139 named rows that are level or ahead have left KNOWN_BEHIND, which now names 898 rows
  (1,037 before the run): 175 on Linux on x86-64, 169 on Linux on ARM64, 168 on macOS on ARM64,
  191 on Windows on x86-64 and 195 on Windows on ARM64. Each of the 898 was behind by more than
  the margin in this run.
- The 6 rows behind that the list doesn't name stay off it, since it may not grow:
  - `linux-x86_64 upcase-stdin c-wasm memory`, 1.05×;
  - `linux-aarch64 shout c-wasm cpu`, 1.11×, and `linux-aarch64 shout cpp-wasm cpu`, 1.09×;
  - `macos-aarch64 upcase c-wasm cpu` and `macos-aarch64 upcase cpp-wasm cpu`, 1.06× each;
  - `windows-x86_64 wc cpp wall`, 1.08×.
- With the list as it is now, these same results pass on Windows on ARM64 and fail on the other
  four runners, on those 6 rows alone. No run has measured the new list yet; this is counted
  from the reports.
- Every native C++ build was made, on all five runners, for the first time. Before this run,
  add and readings didn't build on macOS on ARM64, and add, hypotenuse, sort, tally and readings
  didn't on both Windows runners: the files are C++17, and Apple's clang and LLVM's clang on
  Windows take an older standard unless told. The bench now passes `-std=gnu++17` to every
  native C++ build.

The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
