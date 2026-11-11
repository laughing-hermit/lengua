# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[42947294489](https://github.com/laughing-hermit/lengua/actions/runs/42947294489), at commit
33ed7fe (main, the finish-line run, W15). It writes each report from the run's results alone, the
`target/bench/results-<runner>.json` it keeps in the run's artifacts, so every number in a report
is the run's. The files are copied here byte for byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 7763, 4 threads at once |
| Linux on ARM64 (`ubuntu-24.04-arm`) | [BENCHMARKS-linux-aarch64.md](BENCHMARKS-linux-aarch64.md) | CPU implementer 0x41, part 0xd49, 4 threads at once |
| macOS on ARM64 (`macos-26`) | [BENCHMARKS-macos-aarch64.md](BENCHMARKS-macos-aarch64.md) | Apple M1 (Virtual), 3 threads at once |
| Windows on x86-64 (`windows-2025`) | [BENCHMARKS-windows-x86_64.md](BENCHMARKS-windows-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
| Windows on ARM64 (`windows-11-arm`) | [BENCHMARKS-windows-aarch64.md](BENCHMARKS-windows-aarch64.md) | Cobalt 100, 4 threads at once |

In this run the gate passed on no runner. What it found, counted from each report's section
"Where the run and KNOWN_BEHIND disagree" and its workloads' sections:

| Runner | Ahead on a row KNOWN_BEHIND named | Behind or undecided on a row it doesn't name | Native C++ builds that failed |
|---|---|---|---|
| Linux on x86-64 | 2 | 3 | 0 |
| Linux on ARM64 | 2 | 12 | 0 |
| macOS on ARM64 | 11 | 2 | 2 |
| Windows on x86-64 | 2 | 1 | 5 |
| Windows on ARM64 | 3 | 4 | 5 |
| All five | 20 | 22 | 12 |

- The 20 rows where lengua came out ahead have left KNOWN_BEHIND, which now names 1,037 rows
  (1,057 before the run).
- The 22 rows the list doesn't name stay off it, since it may not grow. Each was ahead in an
  earlier run and left the list then. todo.md lists them under "The owner's", as rows the gate
  can't decide.
- The native C++ builds that failed are add and readings on macOS on ARM64, and add, hypotenuse,
  sort, tally and readings on both Windows runners. Those versions weren't measured, and their
  rows are undecided.

The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
