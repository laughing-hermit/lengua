# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[43131768685](https://github.com/laughing-hermit/lengua/actions/runs/43131768685), at commit
43813ae (branch `wip/gate-same-way`, the first run where the other modules make lengua's
checks). In that run the macOS runner lost its connection to GitHub while it measured and left
no report, so macOS's is from the same commit's run
[43145761976](https://github.com/laughing-hermit/lengua/actions/runs/43145761976) of that runner
alone. The bench writes each report from the run's results alone, the
`target/bench/results-<runner>.json` it keeps in the run's artifacts, so every number in a report
is the run's. The files are copied here byte for byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
| Linux on ARM64 (`ubuntu-24.04-arm`) | [BENCHMARKS-linux-aarch64.md](BENCHMARKS-linux-aarch64.md) | CPU implementer 0x41, part 0xd49, 4 threads at once |
| macOS on ARM64 (`macos-26`) | [BENCHMARKS-macos-aarch64.md](BENCHMARKS-macos-aarch64.md) | Apple M1 (Virtual), 3 threads at once |
| Windows on x86-64 (`windows-2025`) | [BENCHMARKS-windows-x86_64.md](BENCHMARKS-windows-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
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

Decided on 2026-11-11 (todo.md has the reason): on the rows against C, C++ and Rust compiled to
WebAssembly, the other modules now run as lengua runs a module for a user, with lengua's checks.
`lengua-mcp measure --idea` keeps a folder for each module, with lengua's idea for the workload,
the module as main.wasm and a lengua.lock the user's key signs, and the measured run is
`lengua-run <folder> --module`: it reads the lock and checks its signature, hashes the idea and
the module and holds them to the lock, and takes the module's grants from the idea, by the code
that runs a checked program. The finish line sets lengua against the other languages under the
same runtime, and no module runs in lengua's runtime without these checks. In the run before
(43084459708) the other modules ran with no lock, and lengua's own checks, about 0.1 ms, decided
the rows of the workloads that are mostly starting up.

Nothing is hidden: each report's last section, "The other modules without lengua's checks", gives
the same modules run with no lock, beside lengua's program, with lengua's median over each. No
verdict is drawn from it. The rows against the native builds are measured as before.

What lengua's own checks cost, lengua's run against its own module run with no lock, on the
empty program (each report's section "lengua's run apart from its module"):

| Runner | CPU time, this run | CPU time, the run before | Peak memory, this run |
|---|---|---|---|
| Linux on x86-64 | +105 µs (+97 to +113) | +93 µs (+80 to +104) | +88 KiB |
| Linux on ARM64 | +88 µs (+76 to +99) | +84 µs (+73 to +95) | +128 KiB |
| macOS on ARM64 | +188 µs (+13 to +383) | +195 µs (+69 to +319) | +80 KiB |

This run's lengua reads an idea's grants and its secrets in one reading of its lines, where it
read them twice. The table shows no gain from it: the two runs' intervals overlap on every
runner. On Windows CPU time comes in 15.6 ms ticks; the wall time's difference there is +415 µs
on x86-64 and +464 µs on ARM64.

## What the gate found

The run measured the list as it was (761 rows), so the gate failed on every runner. Counted from
each report's section "Where the run and KNOWN_BEHIND disagree":

| Runner | Named rows now level | Named rows now ahead | Rows behind that the list doesn't name |
|---|---|---|---|
| Linux on x86-64 | 1 | 0 | 4 |
| Linux on ARM64 | 4 | 0 | 0 |
| macOS on ARM64 | 6 | 0 | 1 |
| Windows on x86-64 | 11 | 0 | 3 |
| Windows on ARM64 | 3 | 0 | 2 |
| All five | 25 | 0 | 10 |

- The 25 named rows that are level have left KNOWN_BEHIND, which now names 736 rows: 153 on
  Linux on x86-64, 150 on Linux on ARM64, 156 on macOS on ARM64, 130 on Windows on x86-64 and
  147 on Windows on ARM64. Of them 16 are against C, C++ and Rust compiled to WebAssembly (24
  before), and the rest against their native builds.
- Against the WebAssembly builds, of the five runners' 840 rows a measure, lengua is ahead on
  470, level on 351 and behind on 19. Of the 19, 10 are the size of upcase-stdin against C's
  and C++'s modules, on every runner; 6 are named time rows (empty's CPU time against C's and
  C++'s modules on Linux on x86-64, and wc's CPU and wall time against C's and Rust's modules
  on macOS on ARM64); 3 are not named, below.
- The 10 rows behind that the list doesn't name stay off it, since it may not grow. Each row's
  ratio and p are in its report, under "Every row":
  - against the WebAssembly builds, 3 rows:
    - Linux on x86-64: `empty c-wasm wall` and `empty cpp-wasm wall`, each 1.12×. Both modules
      make lengua's checks in these rows, and lengua's run takes 1.49 ms where theirs take
      1.33 ms and 1.34 ms. With no lock, in the report's last sections, lengua's own module
      takes 1.39 ms and C's 1.26 ms: on this runner lengua's module of the empty program
      starts 0.13 ms behind C's by itself. On Linux on ARM64 the two are level (1.56 ms and
      1.58 ms). Why x86-64 differs isn't found yet. The CPU rows of the same two are behind
      too, and named.
    - macOS on ARM64: `tally rust-wasm memory`, 1.05×, where the time is 0.31×.
  - against the native builds, 7 rows:
    - Linux on x86-64: `sort rust cpu` and `sort rust wall`, each 1.08×;
    - Windows on x86-64: `upcase c cpu` and `upcase cpp cpu`, counted in 15.6 ms ticks, and
      `server c wall`, 1.06×;
    - Windows on ARM64: `upcase-stdin c cpu` and `upcase-stdin cpp cpu`, each 2.00× in ticks.
- Of the 26 such rows of the run before, 22 are level or ahead now, and 4 are among the 10:
  the two `empty` wall rows on Linux on x86-64, `tally rust-wasm memory` on macOS on ARM64 and
  `server c wall` on Windows on x86-64. The other 6 are new, all against native builds.
- With the list as it is now, these same results pass the gate on Linux on ARM64 and fail it on
  the other four runners, on those 10 rows alone. No run has measured the new list yet; this is
  counted from the reports.
- Every native C++ build was made, on all five runners.

The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
