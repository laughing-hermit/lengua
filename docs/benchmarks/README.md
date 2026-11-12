# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[43191148225](https://github.com/laughing-hermit/lengua/actions/runs/43191148225), at commit
610aaf5 (branch `wip/gate-same-way`, the first run where the other modules run as outside
modules, with lengua's checks). The bench writes each report from the run's results alone, the
`target/bench/results-<runner>.json` it keeps in the run's artifacts, so every number in a report
is the run's. The files are copied here byte for byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 7763, 4 threads at once |
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
Since W23 lengua has a way of its own to run a module made elsewhere, an outside module: an
idea whose `- Module:` line names main.wasm. The bench uses that way and no other. It adds the
line to lengua's idea for the workload, and `lengua-mcp measure --idea` reads the module as
`check` reads an outside module, refuses what `check` refuses, and keeps a folder with the
idea, the module as main.wasm and a lengua.lock the user's key signs, with an outside module's
stamp: trusted, not proven. The measured run is `lengua-run <folder>`: it reads the lock and
checks its signature, hashes the idea and the module and holds them to the lock, takes the
module's grants from the idea, and says on standard error that the module is trusted, not
proven. The finish line sets lengua against the other languages under the same runtime, and
no module runs in lengua's runtime without these checks. In run 43084459708 the other modules
ran with no lock, and lengua's own checks, about 0.1 ms, decided the rows of the workloads
that are mostly starting up.

Two things changed with it:

- The other modules' memory may grow to 1 GiB, where the bench built them with 4 GiB before:
  1 GiB is the most lengua takes of an outside module, and of its own. No workload holds more
  than 86 MiB.
- `measure` runs none of the idea's tests against a module, as `check` does before it stamps
  one. The bench checks every version's output itself before it measures.

Nothing is hidden: each report's last section, "The other modules without lengua's checks", gives
the same modules run with no lock, beside lengua's program, with lengua's median over each. No
verdict is drawn from it. The rows against the native builds are measured as before.

What the checks cost, on the empty program on Linux on x86-64 in this run: lengua's run takes
1.24 ms of CPU time where its own module with no lock takes 1.14 ms (+94 µs, +86 to +101; the
section "lengua's run apart from its module"); C's module takes 1.24 ms as an outside module
and 1.05 ms with no lock. So an outside module's run pays about twice what lengua's own does.
What the difference goes to wasn't measured apart; one part is the line an outside module's
run writes to standard error, which lengua's own run doesn't. With no lock on either side
lengua's module of the empty program is still behind C's
there (1.14 ms against 1.05 ms), for a reason not found yet. On Linux on ARM64 lengua's
checks cost +115 µs (+100 to +130). A run before this one (43131768685) tried reading an
idea's grants and secrets in one pass instead of two: it measured no gain, and it is gone.

## What the gate found

The run measured the list as it was (736 rows). **The gate passed on Linux on ARM64**, the first
pass on any runner, and failed on the other four. Counted from each report's section "Where the
run and KNOWN_BEHIND disagree":

| Runner | Named rows now level | Named rows now ahead | Rows behind that the list doesn't name |
|---|---|---|---|
| Linux on x86-64 | 2 | 0 | 1 |
| Linux on ARM64 | 0 | 0 | 0 |
| macOS on ARM64 | 0 | 0 | 2 |
| Windows on x86-64 | 1 | 0 | 4 |
| Windows on ARM64 | 0 | 0 | 8 |
| All five | 3 | 0 | 15 |

- The 3 named rows that are level have left KNOWN_BEHIND, which now names 733 rows: 151 on
  Linux on x86-64, 150 on Linux on ARM64, 156 on macOS on ARM64, 129 on Windows on x86-64 and
  147 on Windows on ARM64. Of them 14 are against C, C++ and Rust compiled to WebAssembly, and
  the rest against their native builds.
- Against the WebAssembly builds, of the five runners' 840 rows a measure, lengua is ahead on
  489, level on 337 and behind on 14, and the list names all 14: the size of upcase-stdin
  against C's and C++'s modules on every runner (10 rows), and wc's CPU and wall time against
  C's and Rust's modules on macOS on ARM64 (4 rows, 1.15× to 1.20×). **No row against a
  WebAssembly build is behind and unnamed, on any runner.**
- The 15 rows behind that the list doesn't name are all against native builds, and stay off
  the list, since it may not grow. Each row's ratio and p are in its report, under "Every row":
  - Linux on x86-64: `server c wall`, 1.08×;
  - macOS on ARM64: `server c cpu`, 1.47×, and `server cpp cpu`, 1.44×;
  - Windows on x86-64: `upcase c cpu` and `wc c cpu`, counted in 15.6 ms ticks, `sort rust
    cpu`, 1.08×, and `server c wall`, 1.06×;
  - Windows on ARM64: `upcase-stdin` CPU time against C, C++ and Rust, and `wc` CPU time
    against C and C++, in ticks; `wc c wall`, 1.10×, `wc cpp wall`, 1.08×, and `wc rust wall`,
    1.06×.
- Every one of the 15 is a row the list named once, until a run found it level and it was
  taken out, and it is behind again now: the list can't take a row back. Five left only after
  the run before this one (43131768685): `server c wall` on Linux on x86-64, `server c cpu` and
  `server cpp cpu` on macOS on ARM64, and `wc c wall` and `wc cpp wall` on Windows on ARM64.
  Four more were already among that run's 10 unnamed rows: `upcase c cpu` and `server c wall`
  on Windows on x86-64, and `upcase-stdin` CPU time against C and C++ on Windows on ARM64. The
  other 6 of those 10, the 3 against WebAssembly builds among them, are level or ahead now.
- With the list as it is now, these same results pass the gate on Linux on ARM64 and fail it on
  the other four runners, on those 15 rows alone. No run has measured the new list yet on
  those four; this is counted from the reports.
- Every native C++ build was made, on all five runners.

The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
