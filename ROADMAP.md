# lengua's roadmap: the pivot to WebAssembly

On 2026-11-06 the owner decided that lengua's code is WebAssembly: "we are now pivoting into web
assembly", to "build end 2 end, this new iteration of lengua", "a good product so we can compete
with c/c++ and rust", still "just a agent plugin {Skills + rust mcp}". A program becomes a folder
of `idea.md`, an idea file in the Open Knowledge Format, and `main.wat`, the agent's WebAssembly.
`lengua-mcp` embeds a WebAssembly engine (Wasmtime) and its own WASI preview 1 host, so it checks,
proves and runs every program the same way on five runners: Linux on x86-64 and ARM64, Windows on
x86-64 and ARM64, and macOS on ARM64. The finish line, the decisions made for the owner, and what
is left are in [todo.md](todo.md). The native lengua's roadmap is at the tag `native-final`, as
the last section says.

Every milestone works on its own branch, merges to main when its runs are green on the runners it
names, and records their ids in todo.md; each milestone below names them. Until W11 the native
path and the WebAssembly path lived side by side, chosen by the code file (`main.s` or
`main.wat`); W11 removed the native one. Nothing builds on the owner's computer: the format job
hands back the formatting patch and, for a new dependency, the resolved `Cargo.lock`.

What is left goes in this order (todo.md): W17b, W15 and W16, then, after the finish line, W18 to
W21.

## W0. Record the decision

todo.md's finish line, decisions, roadmap and superseded list; this part; README's opening; the
tag `native-final` at 723879e. *Verified by:* `compile` green on both Linux runners (42295197937);
main at fc86dd3.

## W1. Idea files

`spec.md` becomes `idea.md` everywhere (examples, fixtures, the skills' assets), and the skill
`write-spec` becomes `write-idea`. In Rust, `src/spec.rs` becomes `src/idea.rs` and `Spec` becomes
`Idea`; messages, next steps and docs follow. The lock's `[spec]` table becomes `[idea]`, and its
format version goes from 4 to 5, so old locks are checked again. A folder that still holds only
`spec.md` is asked to rename it. The manifests point at the Agent Plugins 1.1.0 schemas, vendored
from the specification's repository (a working draft). *Verified by:* `test` green on both Linux
runners at ff68d40, the QEMU job's known failures aside (42304173338), and the golden ledger
against 723879e showing no checker or prover verdict changed (42296519743).

## W2. CI on all five runners

The two-target cut's changes to CI are reversed: five rows in the runners' list, z3 and cvc5
pinned for each, and each system's setup. lengua builds and its tests pass natively on each
runner; native programs ran on Linux only, and elsewhere their tests noted "native programs run on
Linux only" and ended. *Verified by:* the suite natively on all five runners at f762aa2
(42304371890); macOS's one failure there, a proof's time test that a fast machine finished early,
is fixed at e9f4700.

## W3. The knowledge graph

`check` builds the graph of program folders from the folder to the bundle's root (the nearest
folder above with an `index.md` that has `okf_version`, else the program folder's parent), checks
it (both files, links, no cycles in `Builds on`, every node listed, no orphans, no idea newer than
its lock, `type` on every page), keeps `index.md` (the Open Knowledge Format's §8) and `log.md`
(§9) at the root, changing only its own entries and never writing in a home folder, a
filesystem's root, the temporary folder, or beside an `index.md` without `okf_version` that isn't
lengua's, and says how the graph stands in its reply. The scan is bounded, and writes are
serialized. *Verified by:* `tests/graph.rs` and the whole suite on all five runners at 5acdb65
(42308171081, its format patch applied at e9f4700), and `examples/index.md` reproduced byte for
byte.

## W3b. Idea files in Simplified Technical English

Idea files are written in ASD-STE100 Simplified Technical English, in the style of
behavior-driven development (the owner's words). `check` holds an idea's prose (its description,
purpose, behaviors' titles and steps) to what a computer can see without STE's dictionary: at most
25 words a sentence and six sentences a paragraph, no -ing form but technical names, and every
step in the step vocabulary's STE forms, an old phrasing refused with its rewrite. The
`write-idea` skill teaches the rest, in lengua's own words, and every idea file in the repository
is rewritten in STE. *Verified by:* `tests/ste.rs` (every idea file in the repository passes), and
format, the suite on all five runners and the gate, green at 2fb46f1 (42317583638).

## W4. The WebAssembly runtime

`src/wasm/`: the text parsed by `wast` with each instruction's line, lengua's library merged in,
validated by `wasmparser` with the engine's features; Wasmtime with Cranelift, pinned, with three
configurations (tests with fuel and epochs, runs with epochs, shells and benchmarks with neither)
and its compilation cache in the `cache` folder of the plugin's data folder when the agent's
harness gives one (`PLUGIN_DATA`, or `CLAUDE_PLUGIN_DATA`), else of lengua's own; lengua's own WASI
preview 1 host (the function table with each function's allowance, borrowed buffers and errors;
standard streams; files through `cap-std` and `src/names.rs`; the listener and connections;
`poll_oneoff`; the clock and random bytes, supplied in tests; limits); a pool of `lengua-mcp
--wasm-worker` processes, each run a fresh store, returning `runner::Run`. *Verified by:*
`tests/host.rs` (the conformance cases in `tests/host-cases/`, written apart from the host as its
check) and `tests/wasm_limits.rs` green on all five runners with the same assertions: at 63af1eb,
format, the suite on all five runners and the gate, green, with the 109 host cases and the limit
tests passing the same on every runner (42327997413). Its security and correctness review's 15
findings were fixed before that run.

## W5. The WebAssembly dialect's rules

`src/checker/wasm/`: the module's structure (imports the idea allows, exports `_start` and
`memory` only, no start function, no tables, no `memory.grow`, memory sized once), structured
control and an acyclic call graph, rules 1 to 10 in their WebAssembly meaning (an assertion is
`call $assert_fail` with its number and exits 134; loops have counters the checker follows; every
host call's error is tested; `br_table`'s default leads to an assertion or an error), the memory
rules over regions (`$x` with `$x.size` or `$x.rosize`), and determinism (no relaxed SIMD or
atomics, no NaN bits observed, saturating conversions only, division guarded). Reports keep their
format, `rule N (name) main.wat:LINE`. *Verified by:* a planted violation of every rule found on
its line, and the red team's refusals, on all five runners: at 0534a06, format, the suite on all
five runners and the gate, green, with the template passing every rule (42336485988). Its
review's 23 findings were fixed before that run.

## W6. `check` and `run` for WebAssembly ideas

`idea.md` an Open Knowledge Format `Attested Computation` (`runtime`, `computation`, `parameters`
with `from`, `executor`, `attester`); `check` from request to `done` for `main.wat` (the build, the
rules, parts, tests through the pool, the code stamp in lock format 6, `verified` written at
`done` and removed when the idea changes), "tested, not proven" until W8; the `machine` line with
the engine and its features; `lengua-mcp run` from a shell and `lengua-mcp check` for a consumer's
own attestation; the first programs ported. *Verified by:* `add` from request to `done` and run,
on all five runners: the Attested Computation form alone at 5791d73 (42329274411), then check and
run for main.wat at 1c75e7d, with format, the suite on all five runners and the gate green, add
from the request to done, run, `lengua-mcp run` and `lengua-mcp check` (42345074041). The
reviews' 15 and 17 findings were fixed before those runs.

## W7. The prover's seam

The world the prover models (standard input and output, files, the network, the clock, random
bytes, obligations) taken out of the CPU's state, and `explore` generic over a core, with the
induction keyed by slots instead of registers. *Verified by:* the golden mode's raw view, empty:
the checker's and the prover's recordings identical, byte for byte (42325471157); the seam on main
at d280fa1, with format, the suite on all five runners and the gate green (42336209846).

## W8. The WebAssembly prover

In four steps, each merged on its own: the concrete model against the engine (differential tests
on every runner, and the host's cases through the model); symbolic proofs over integers, text,
files and the network; every length, by induction over a read loop; SIMD128 and floats. The plan
also named merging at loop exits and tables. Neither is in: the WebAssembly core merges no loop
exits, and the model refuses tables, as the dialect does. *Verified by:* every prover case with
the same kind of verdict on all five runners:
- the model and proofs, z3 and cvc5 on every runner, with add, echo and greet proven and planted
  bugs found (42344545735), and stacked on W6 at dac4c7d (42348682819);
- `check` proving main.wat at c6508b4 (42362037786), its golden ledger (42357663706) differing
  only in sha256's load-dependent x86-64 line, proven neither way;
- every length by induction over a read loop, and SIMD128 and floats in the model, at 2b71184,
  with every covered operator compared with the engine's bits (42386526911); the golden ledger
  against W8's earlier base, with no native verdict changed (42388141609);
- on main with W10 at 2727216 (42391963513).

Its review's 4 findings were fixed before 42386526911. The mutation test, which W11 moved onto
main.wat, has no recorded run yet; the finish-line run (W15) runs it.

## W9. Every program in WebAssembly

The remaining examples, fixtures and templates, the fast forms (SIMD128), and the benchmark's
lengua versions, each as `idea.md` and `main.wat`, written from the skills alone; the skills
rewritten (`write-idea`'s format reference; `compile`'s references for the dialect, the host, the
rules and speed, and its WebAssembly templates); version 0.2.0; and the parity ledger: every
verdict that changed from the native lengua, with its reason
([docs/history/PARITY.md](docs/history/PARITY.md)). The server template waits with `poll_oneoff`,
which the model doesn't follow yet, so its proofs stop there. A program that reads keys from the
terminal has no template: it waits for the host to give the terminal's raw mode and window size,
which W18 plans. *Verified by:* every program's rules
and tests on all five runners, each run with format, the suite and the gate green: the
WebAssembly templates, wasm.md and write-idea's Attested Computation at 50a546f (42356699038); all
29 programs in WebAssembly from the request to done at b761dd3 (42367496557); the compile skill
for main.wat, the parity ledger and 0.2.0 (42373720077).

## W10. Constant time and caches

The `- Secret:` line and the taint pass over the WebAssembly (no secret in a condition, a
`select`, an address, a length, a loop's count, division or floating point); an example; the
`machine` line's caches on every system, and the compile skill's portable guidance (blocks within
32 KiB for L1 and 256 KiB for L2, 128-byte padding, strides that avoid 4 KiB aliasing).
*Verified by:* planted leaks found on their lines, and caches shown on every runner: at df1e8ae,
format, the suite on all five runners and the gate, green (42386323947); on main, stacked with
W8's steps 3 and 4, at 2727216 (42391963513). The rule hasn't passed a soundness review: one was
stopped by the safety filter (todo.md).

## W11. The native product retired

The assembly path removed: the CPU models and their decoders, the per-CPU checker and prover
cores, the edge's assembly, the broker, the Linux sandbox, the LLVM toolchain and QEMU, with their
tests, assets and dependencies. The tag keeps them, the decoders W13 would have used among them.
*Verified by:* the full suite on all five runners, and the golden baseline recorded for
WebAssembly:
- at 9ab14a5, format, the suite on all five runners and the gate, green (42713054140);
- release at 8d196ee: the five archives built without it, each installed on a clean runner,
  doctor and the release check green (42707398628);
- main at 3dae90d, whose push suite is green (42716434491);
- the golden baseline for WebAssembly, recorded twice at 3dae90d and the same both times
  (42716553289).

Its three reviews' 15 findings were fixed before those runs.

## W12. Performance and the gate

The bench rewritten: lengua's module against C, C++ and Rust natively and compiled to WebAssembly
(built once on Linux and shared), process against process; `lengua-mcp measure`; the workloads of
todo.md's point 4; Linux's counters (L1 data, last-level and TLB misses); alternating runs, a
one-sided Mann–Whitney U test at 1% split over eight looks, KNOWN_BEHIND and its ratchet, the gate
in `bench` mode and the weekly run; a BENCHMARKS file per runner from the artifacts. *Verified
by:* the gate green on all five runners against WebAssembly builds, and the native gaps
published. So far: the bench, its workloads and the gate at 9a549b3, with the suite on all five
runners and the gate job green (42393960971); the first gate run on all five runners, at 9a549b3
(42661776298), whose rows became each runner's lines of KNOWN_BEHIND at 2bad55a. The gate hasn't
passed on any runner yet. W12b and W12c work on that, and the latest reports are in
[docs/benchmarks](docs/benchmarks/README.md).

## W12b. lengua's run made lean

The first gate run (42661776298, all five runners) found lengua's program ahead on module size
nearly everywhere, and on time where the work is large (wc, files, readings, part of sort). It was
behind on time and memory wherever starting dominates, and on memory almost everywhere. That gap
was `lengua-mcp run`'s own work around the program (reading the whole idea, building main.wat
again) and lengua's library merged in whole. W12b cuts it and keeps every guarantee:
- a run from a shell runs the module `check` tested, which `check` keeps in the data folder by
  its hash, and builds main.wat again only when that copy is missing or differs;
- a run reads idea.md and main.wat once, takes of the idea only its grants, and reads and
  verifies lengua.lock once;
- the module that runs has no name section, and lengua's library calls main.wat's own imports of
  WASI's functions;
- the cache of compiled modules never compresses a module again;
- `lengua-mcp` is built with one codegen unit, fat link-time optimization and no symbol table.

*Verified by:* at d6496e3, format, the suite on all five runners and the gate, green
(42728146781); the gate run on all five runners at d6496e3 (42728160641), where lengua came out
ahead on 94 rows KNOWN_BEHIND named, which left the list in 59f76f9; main's push suite at eff6b98,
green (42732412732).

## W12c. Faster programs

Speed work in lengua's programs, where they trailed C, C++ and Rust compiled to WebAssembly:
- tally finds a piece's word edges from 64-byte bitmasks and counts a word of up to 8 bytes as one
  number;
- sort-large sorts 8-byte records in place with a multikey quicksort, keys compared as numbers;
- histogram takes eight bytes a step into four lent tables;
- crc32c slices by sixteen;
- count (wc) counts lines and words in byte counters, with no bitmask;
- files reads a file shorter than its buffer in one read.

And the last of the run's own cost: the engine starts while the run checks, on both paths, and
compiles nothing before the checks end; lengua.lock is read in one read, and a lock exactly as
lengua writes it is read with no TOML (todo.md's decisions say why the lock's guarantees hold).
*Verified by:* at 076bcc1, the suite on all five runners, green (42756064246; its format job's
layout fixed at f4cf505); the gate run on all five runners at 076bcc1 (42756074550), where lengua
came out ahead on 208 rows KNOWN_BEHIND named, which left the list in 5714b42, and whose reports
are in [docs/benchmarks](docs/benchmarks/README.md); main at f4cf505, whose push run, format, the
suite on all five runners and the gate, is green (42760180544). In 42756074550 the gate passed on
no runner: besides those 208 rows, its problems were five rows the gate can't decide, which are
the owner's (todo.md), and native C++ builds that failed on macOS and Windows.

## W13. Constant time in the machine code

For an idea with `- Secret:`, `check` compiles the module for this machine, decodes the engine's
code, maps each instruction to its WebAssembly offset, and refuses a conditional branch, an
indirect jump, a division or an address derived from a secret. *Verified by:* the tests on all
five runners, which covers x86-64 and ARM64 machine code. Stopped by the safety filter on
2026-11-07, while it was being designed, and left for the owner (todo.md): the engine's compiled
code is trusted for timing.

## W14. Package, `doctor`, release, and the agent's test

One archive per runner's system and CPU (the plugin, `lengua-mcp`, pinned z3 and cvc5, the
licenses), `lengua-mcp doctor`, solvers found beside the server first; `release` mode builds each,
and a clean runner of each installs and checks it through a plain MCP client. Then the agent's
test, in the interactive session: the Linux x86-64 archive connected, and an idea written,
compiled, checked, run and read back through the graph. *Verified by:* the suite on all five
runners and the gate at 83641b5 (42387284972); release at 7835d9a, the five archives built, each
installed on a clean runner of its system and CPU, doctor and the release check green
(42387777039); main at 7835d9a. The agent's test in the interactive session, on 2026-11-07 with
the Linux x86-64 archive of release run 42383680141, is in todo.md.

## W15. The documents, the finish-line run, the report

README, DESIGN (with the threat model), the white paper, this file, todo.md, the plugin README and
the skills say what the runs show. The native lengua's parity ledger, sessions and benchmarks move
to [docs/history](docs/history), and the latest bench run's reports are kept in
[docs/benchmarks](docs/benchmarks/README.md). Then `test`, `ignored`, `bench` and `release` on all
five runners at the final commit, their ids against each point in todo.md, and the report to the
owner. *Under way.*

## W16. The repository tidied and presentable on GitHub

The owner, 2026-11-07: "when finished with lengua please tidy up the repo and make it presentable
please in github". A README that presents lengua, the top level pruned, and the repository's
description and topics on GitHub. At the end only main remains (the owner, 2026-11-09), on GitHub
and on the owner's computer, `wip/routes-ci`, `wip/r2-8a-snap` and `wip/r2-base-check` included.
First, a branch whose commits main doesn't hold keeps its tip as a tag `archive/<branch>`, so
nothing is lost; the tag `native-final` stays as it is. *Left to do.*

## W17. lengua in Rust and WebAssembly only

The owner, 2026-11-09: "ok go with rust and webassembly, proceed". Rust is everything lengua is:
the server, its tools and command line, and its tooling. WebAssembly is everything lengua makes:
main.wat, lengua's library, the templates and the programs. The benchmark's C and C++ programs
stay, as the competitors point 4 measures, and so do the manifests, data and documents. todo.md's
decisions give the four choices made with it.

**W17a, the tooling in Rust.** The shell, PowerShell, sed and Python scripts become Rust: the task
crate `xtask/` (`cargo xtask`, with Rust 1.99.1 pinned in `rust-toolchain.toml`) for the release
archive, fetching the pinned tools, CI's job matrix and steps, the goldens' comparison and the
bench's WebAssembly builds; and `lengua-mcp install` in place of the install scripts. The workflow
file is glue: each step is one command, most of them `cargo xtask` calls. The site, the native
assembly left in the bench's folder and
`scripts/` go; the tag keeps the site. *Verified by:* at d0e9ea6, format, the suite on all five
runners and the gate, green (42753840475); release and install on all five runners, each archive
built by `cargo xtask release` and installed on a clean runner, green (42753851417); the golden
diff against its base, differing in four proof lines only, each one the base ran out of time on
and the head proved (42753863525); the bench through the task crate on Linux on x86-64, which
measured every workload and wrote its report (42753874793), its gate's four problems rows of
upcase and files. Merged with main's W12c at bce6cd5.

**W17b, the solvers as WebAssembly.** z3 runs as WebAssembly if it costs fairly: the same verdicts
on all five runners at today's limits, with the replayed questions taking at most twice as long in
all. If the bar isn't met, native z3 stays and the measurement is written in todo.md. cvc5 stays
native, since no build that Wasmtime runs exists yet; after z3, one bounded try to make one, at
most five build runs. *Under way.*

## W18. Reach: everything possible, nothing implicit

After the finish line, as are W19 to W21 (the owner, 2026-11-09: "I agreee please add them").
Least privilege stays the default. An idea may grant whatever a C, C++ or Rust program can do:
each grant is declared under `# Constraints`, checked by lengua, shown to the user before the
program runs, enforced by the host, and modelled by the prover.
- Files anywhere the user names: a folder or file an argument names, the home folder, or
  read-only system files.
- The network chosen at run time: any computer on a declared port, over TLS, and also UDP and DNS.
- Starting named programs (`- Runs: git`), with pipes.
- The terminal's two imports (keys as they're pressed, the window's size), so interactive tools
  work.

What it unlocks: shells, build tools, package managers, crawlers, proxies, and programs over
databases on any path.

## W19. Native speed: a lean runner and native builds

Most of the gap to native builds is start-up and the engine, not the program.
- A run-only binary: Wasmtime's runtime without its compiler loads the module `check` compiled
  ahead of time, authenticated as it is today. Start-up goes from about 2.5 ms toward C's 0.4 ms,
  and memory from about 10 MiB toward a few.
- An optional native build of the proven main.wat, through wasm2c and clang, as Firefox sandboxes
  C libraries at close to native speed. The proof still holds, since it covers WebAssembly's
  semantics, and main.wat stays the one shared artifact.

## W20. Parallelism

WebAssembly's threads (shared memory and atomics), as structured fork-join alone. The rules show
that a program has no data race, so its results stay the same on every run, which free threads in
C, C++ and Rust can't promise.

## W21. A library of proven parts

A shared, proven standard set (sorting, hash tables, JSON, HTTP, compression, dates) that programs
build on with `- Builds on:`. The agent assembles programs from verified pieces instead of writing
everything new, which makes them faster to build and to prove.

## Before the pivot

Until 2026-11-06 lengua was native: the agent wrote `main.s`, assembly for Linux on x86-64 or on
ARM64, which lengua assembled with LLVM, checked, proved on the decoded machine code and ran in a
Linux sandbox through a broker. Its roadmap planned six targets in rounds R0 to R6: the documents;
six hosts, one workflow and the spikes; the shared OS edge and the broker, built on Linux on
2026-11-06; a Linux beside the agent, through WSL 2 and Lima; native macOS and Windows ports,
deferred that day; the performance finish line; and prover parity, the leftovers and the release.
The pivot superseded it. The tag `native-final` keeps that lengua, its roadmap included:
[ROADMAP.md at native-final](https://github.com/laughing-hermit/lengua/blob/native-final/ROADMAP.md).
