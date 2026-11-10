# The benchmark reports

The five reports here are the latest bench run's, one for each runner lengua-mcp is built for. The
bench, `cargo bench --bench against_rust_and_c`, wrote them in GitHub Actions run
[42756074550](https://github.com/laughing-hermit/lengua/actions/runs/42756074550), at commit
076bcc1 (W12c, with main merged in). It writes each report from the run's results alone, the
`target/bench/results-<runner>.json` it keeps in the run's artifacts, so every number in a report
is the run's. The files are copied here byte for byte, as a run's artifacts expire after 7 days.

| Runner | Its report | The CPU the run had |
|---|---|---|
| Linux on x86-64 (`ubuntu-24.04`) | [BENCHMARKS-linux-x86_64.md](BENCHMARKS-linux-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
| Linux on ARM64 (`ubuntu-24.04-arm`) | [BENCHMARKS-linux-aarch64.md](BENCHMARKS-linux-aarch64.md) | CPU implementer 0x41, part 0xd49, 4 threads at once |
| macOS on ARM64 (`macos-26`) | [BENCHMARKS-macos-aarch64.md](BENCHMARKS-macos-aarch64.md) | Apple M1 (Virtual), 3 threads at once |
| Windows on x86-64 (`windows-2025`) | [BENCHMARKS-windows-x86_64.md](BENCHMARKS-windows-x86_64.md) | AMD EPYC 9V74, 4 threads at once |
| Windows on ARM64 (`windows-11-arm`) | [BENCHMARKS-windows-aarch64.md](BENCHMARKS-windows-aarch64.md) | Cobalt 100, 4 threads at once |

In this run the gate passed on no runner. Most of its problems are rows where lengua came out
ahead while KNOWN_BEHIND still named them, which commit 5714b42 took out of the list (208 rows).
Five rows the list doesn't name came out behind or undecided; todo.md lists them under "The
owner's", as rows the gate can't decide so far. The rest are native C++ builds that failed, so
those versions weren't measured: add and readings on macOS on ARM64, and add, hypotenuse, sort,
tally and readings on both Windows runners. Each report's section "Where the run and KNOWN_BEHIND
disagree" and its workloads' sections say which rows these are.

The finish-line run ([ROADMAP.md](../../ROADMAP.md), W15) replaces these five files with its own
reports. The native lengua's numbers, from before the move to WebAssembly, are in
[docs/history](../history/BENCHMARKS.md).
