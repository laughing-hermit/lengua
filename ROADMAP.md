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

W0 to W14 and W17a are on main, W17b is measured, and W15's finish-line run is done
(42947270993, 42947285372, 42947294489 and 42947303258, at main 33ed7fe). The finish line itself
isn't reached: the gate passed on no runner in that run, nor in the first run with its noise
margin (42964232679; W15 below). What is left goes in this order
(todo.md): W15's report to the owner (the agent's test with the final archive ran on 2026-11-11, and W16 is done);
then, after the finish line, W18 to W24.

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
runner; native programs ran only on Linux on x86-64 and Linux on ARM64, and on the other three
runners their tests said so and ended. *Verified by:* the suite natively on all five runners at
f762aa2 (42304371890); macOS's one failure there, a proof's time test that a fast machine finished
early, is fixed at e9f4700.

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
which W18 has since given. *Verified by:* every program's rules and tests on all five runners, each
run with
format, the suite and the gate green: the WebAssembly templates, wasm.md and write-idea's Attested
Computation at 50a546f (42356699038); all 29 programs in WebAssembly from the request to done at
b761dd3 (42367496557); the compile skill for main.wat, the parity ledger and 0.2.0 (42373720077).

## W10. Constant time and caches

The `- Secret:` line and the taint pass over the WebAssembly (no secret in a condition, a
`select`, an address, a length, a loop's count, division or floating point); an example; the
`machine` line's caches on every system, and the compile skill's portable guidance (blocks within
32 KiB for L1 and 256 KiB for L2, 128-byte padding, strides that avoid 4 KiB aliasing).
*Verified by:* planted leaks found on their lines, and caches shown on every runner: at df1e8ae,
format, the suite on all five runners and the gate, green (42386323947); on main, stacked with
W8's steps 3 and 4, at 2727216 (42391963513). A soundness review on 2026-11-10
(docs/reviews/secret-rule-2026-11-10.md) found five ways around the rule, each then shown by a
failing test and closed (42877251634, 42881405730, 42884257361); todo.md has what it left open.

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
passed on any runner yet, in four runs. W12b and W12c worked on that. The latest run is the
finish-line run (42947294489, W15), and its reports are in
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
were in [docs/benchmarks](docs/benchmarks/README.md) until the finish-line run's took their place
(they are in the history, at 33ed7fe); main at f4cf505, whose push run, format, the
suite on all five runners and the gate, is green (42760180544). In 42756074550 the gate passed on
no runner: besides those 208 rows, its problems were five rows the list doesn't name, which came
out behind or undecided and are the owner's (todo.md), and native C++ builds that failed on macOS
on ARM64 and on both Windows runners. One run later, at 33ed7fe (42947294489, W15), 14 of those
208 rows were behind or undecided again.

## W13. Constant time in the machine code

For an idea with `- Secret:`, `check` compiles the module for this machine as `run` and a run
from a shell will, and reads the engine's code for each operator the secret rule found to take a
secret, by the engine's address map: x86-64 with `iced-x86`, ARM64 by the architecture's encoding
index (`src/checker/wasm/machine.rs`). On the operator's line of main.wat it refuses a conditional
branch, a jump or a call through a register, a call of the engine's functions, a division, the
address of a table in the code, and bytes it can't read. `run`, a run from a shell and the
attester read the code again before they run it, so each run's code was read on its own computer.
An idea with no secret compiles nothing for it. It was stopped by the safety filter on 2026-11-07
while it was being designed, and built on 2026-11-10 (on main).

What it found: on x86-64, Cranelift's code for `i8x16.shl` and `i8x16.shr_u` by an amount that
isn't a constant reads a mask from a table at an index made from the amount, which the secret rule
over main.wat can't see. `check` refuses it there. On ARM64 the same module passes, and no
operator the secret rule passes was found whose ARM64 code branches or reads a table.

In a function that computes with a secret, it reads the code the address map gives no operator
as well, but the function's entry and its constants: the engine's optimizer makes some operators
anew with no offset, and the first build, which trusted that code, passed a lane shift it had
made anew.

What it trusts: the CPU's own timing of the instructions it leaves; a function's entry; the code
of functions and operators that take no secret, of lengua's library, and of the engine's and the
host's own functions; and, on ARM64, that a word is the instruction its group's bits say.

*Verified by:* the tests on all five runners, each of its own CPU's code: secret-equal passes;
one module with a lane shift by a secret amount is refused on its line on x86-64 and passes on
ARM64; the engine's own branch is refused only when its operator is named as taking a secret; and
both readers are pinned on every runner with code written as bytes (todo.md has the runs).

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
owner.

*The finish-line run,* all at main 33ed7fe, on all five runners:
- `test`: main's push run, format, the suite and the gate job, green (42947270993).
- `ignored`: the slow tests the suite leaves out, green (42947285372).
- `release`: the five archives built, each installed and checked on a clean runner of its system
  and CPU, green (42947303258).
- `bench`: it ran to its five reports, and the gate failed on every runner (42947294489). It
  found lengua ahead on 20 rows KNOWN_BEHIND named, which left the list at 28afe67 (1,037 rows
  now), and behind or undecided on 22 rows the list doesn't name: 3 on Linux on x86-64, 12 on
  Linux on ARM64, 2 on macOS on ARM64, 1 on Windows on x86-64 and 4 on Windows on ARM64. The
  native C++ builds failed again on macOS on ARM64 (2) and on both Windows runners (5 each).
  Against C, C++ and Rust compiled to WebAssembly, of each runner's 42 rows a measure, lengua
  was ahead on 40 for size, 34 to 39 for wall time, 21 to 36 for CPU time and 2 to 16 for peak
  memory. Against the best native build its CPU time was 0.46 to 6.56 times.

Each of the 22 rows was ahead in an earlier run, and 19 of them in the run before: rows of short
workloads and of peak memory, at 0.99× to 1.07× in this run, move between ahead and behind from
run to run, and the list may not take a row back. What to do with them is the owner's (todo.md,
"A row the gate can't decide"). The five reports were in
[docs/benchmarks](docs/benchmarks/README.md) until the next run's took their place.

*After the run, the owner's decision of 2026-11-11:* the gate gets a noise margin. A measure
within 5% either way, or one the test doesn't decide, is level, and a level row doesn't fail the
gate (`MARGIN` in `src/gate.rs`). The first run with it (42964232679, at cd54e79 on
`wip/gate-margin`, with the suite on Linux on x86-64 green in 42964220148) found 139 rows
KNOWN_BEHIND named level or ahead, which left the list (898 rows now), and 6 rows behind by more
than the margin that it doesn't name: 1 on Linux on x86-64, 2 on Linux on ARM64, 2 on macOS on
ARM64 and 1 on Windows on x86-64. Every native C++ version was built on all five runners, now
that the bench tells each compiler the files are C++17.

*Left:* the report to the owner. The agent's test with the final archive ran on 2026-11-11 (todo.md).

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
assembly left in the bench's folder and `scripts/` go; the tag keeps the site. *Verified by:* at
d0e9ea6, format, the suite on all five runners and the gate, green (42753840475); release and
install on all five runners, each archive built by `cargo xtask release` and installed on a clean
runner, green (42753851417); the golden diff against its base, differing in four proof lines only,
each one the base ran out of time on and the head proved (42753863525); the bench through the
task crate on Linux on x86-64, which measured every workload and wrote its report (42753874793),
its gate's four problems, rows of upcase and files. Merged with main's W12c at bce6cd5: the suite
on all five runners and the gate, green (42765232534), and release and install on all five, green
(42765242004); main at bce6cd5.

**W17b, the solvers as WebAssembly.** z3 runs as WebAssembly if it costs fairly: the same verdicts
on all five runners at today's limits, with the replayed questions taking at most twice as long in
all. If the bar isn't met, native z3 stays and the measurement is written in todo.md. cvc5 stays
native, since no build that Wasmtime runs exists yet; after z3, one bounded try to make one, at
most five build runs. *Measured, and the bar isn't met:* run 42919127514 on all five runners has
the WebAssembly z3 at 1.70 to 2.28 times native's time on the replayed questions, past twice on
macOS on ARM64 and Windows on ARM64, and a proof lost on three runners. Native z3 stays; the
build, the host and the measurement are kept at the tag `archive/wip/z3-wasm`. The cvc5 try
is skipped, by the owner's decision of 2026-11-11: z3 missed the bar on four of the five runners,
and cvc5 would face the same bar. To revisit when Wasmtime or the solvers change (todo.md).

## W18. Reach: everything possible, nothing implicit

After the finish line, as are W19 to W21 (the owner, 2026-11-09: "I agreee please add them").
Least privilege stays the default. An idea may grant whatever a C, C++ or Rust program can do:
each grant is declared under `# Constraints`, checked by lengua, shown to the user before the
program runs, enforced by the host, and modelled by the prover.
- Files anywhere the user names: a folder or file an argument names.
- The network chosen at run time: any computer on a declared port, over TLS, and also UDP and DNS.
- Starting named programs (``- Runs: `git` …``).
- The terminal's imports (keys as they're pressed, the window's size), so interactive tools
  work.

What it unlocks: shells, build tools, package managers, crawlers, proxies, and programs over
databases on any path.

**The network, built on 2026-11-11** (branch `wip/reach-net`). An idea grants what its program
opens by itself, a `- Network:` line for each grant: `opens TCP connections to port N on …`
(`over TLS` too), `looks up the addresses of …`, `sends UDP datagrams to port N on …` and
`receives UDP datagrams on port N`, for a named computer, any subdomain of a domain, the computer
an argument names, or any computer. Seven functions of lengua's own, in the module
`lengua_net_1`, open them, since WASI preview 1 has none. lengua spells each name one way, finds
the line that grants it, resolves it once, holds every address to the grant (never this computer;
a private network only where the line says so; never an address of many computers), and connects
to those addresses alone. A program holds 64 of its own at once, and lengua ends one that no byte
crosses for 300 seconds. The checker's import rule, the secret rule, the test format's steps (a
test's own servers by computer and port, datagrams, and names' addresses), `check`'s and `run`'s
`may_reach`, the skills, a client template and the example `follow` came with it. The prover
refuses such a program with the reason: tested, not proven. The threat cases and the test that
holds each are in docs/reviews/reach-2026-11-11.md. *Verified by:* the suite on all five
runners at ca87111, with format and the gate green (43049640420); 17 of the defences' tests
failing on the commit that took the defences out (43037211691); todo.md, "W18's network", has
every run.

**Files, programs and the terminal, built on 2026-11-11** (branch `wip/reach-2`). An idea grants
the rest of what a program reaches by itself, each grant in its own words:
- `- Files:` lines grant the file or the folder one of the program's arguments names, to read,
  to write or both (`reads the file its first argument names`, `reads the files its arguments
  name`, `reads and writes the folder its second argument names`). Two functions of
  `lengua_fs_1` open them by the argument's place, never by a path: `file_open` and
  `folder_open`. lengua resolves the argument itself, in a test beneath the test's own folder
  and in a run on the user's file system, opens nothing among its own folders or the system's,
  follows no link at the last name, and opens no file with a second name.
- `- Runs:` lines name a program and bound its arguments (``- Runs: `tar` with the arguments
  `-tf` and a value, within 60 seconds``). One function of `lengua_run_1`, `program_run`, starts
  the program, gives it input, waits, and gives back its output and exit code. A line never
  names a shell; lengua finds the program in the absolute folders of `PATH`, never where the
  starter could have written it, starts it with no environment in an empty folder of its own,
  and ends it, with every process it started, at its time or at the buffer's end. A test starts
  nothing: a step says what a command gives.
- `- Uses: the terminal` now has its functions, three in `lengua_term_1`: `term_is`,
  `term_size` and `term_raw`. lengua puts a raw terminal back however the program ends, from the
  host's own end and from a handler in lengua-os for a signal or Windows's console closing.

The checker's import rule, the constants rule, the secret rule (a command is no data; a
started program's input is), `check`'s and `run`'s `may_reach`, the skills with a reference
page, and three examples came with it: `copy`, `computer` and `key`. The prover refuses such a
program with the reason: tested, not proven. The threat cases and the test that holds each are
in docs/reviews/reach-2-2026-11-11.md. What it leaves: a started program that works in the
user's folder, pipes between started programs, an environment for one, and a step of a test for
each test alone; todo.md, "W18's second part", has each with why. *Verified by:* the suite on all
five runners at the branch's head, which the commit that lands it names (the two runs before it,
43098886771 and 43099231987, failed on a line of `examples/index.md` and on a test's count of
the variables Windows on ARM64 gives a process); 23 of the defences' tests
failing on the commit that took the defences out (43084696723); the mutation test on
`examples/follow` and the three new examples (43093511750); todo.md has every run.

## W19. Native speed: a lean runner and native builds

Most of the gap to native builds is start-up and the engine, not the program. Two steps.

**Step 1, built: a run that starts sooner and holds less.** It was measured first, on Linux on
x86-64 (42962697250), with marks along a run of add. That run took 1.85 ms of CPU time and
12.4 MiB at its peak, where a small Rust program that does nothing took 0.55 ms and 1.9 MiB:
- 0.75 ms and 4.9 MiB were gone before lengua-mcp's first line ran: the 24 MiB program itself,
  which took 0.88 ms and 5.5 MiB to do nothing. At the run's end, 10.6 MiB of its 13.3 MiB
  were pages of lengua-mcp's own file, and under 0.1 MiB its heap;
- the checks took 0.23 ms; the engine with its compiler and cache 0.10 to 0.13 ms; lengua's 46
  host functions 0.08 to 0.11 ms; the module from Wasmtime's cache 0.20 ms; the program's start
  and run 0.10 ms; and the end, with the cache's thread, 0.17 to 0.23 ms.

What was built, and what each part bought in those runs and the two after them (42967736363,
42971426958, Linux on x86-64 and ARM64):
- `check` compiles the module ahead of its runs and seals it with the user's key, and a run
  loads it in place of Wasmtime's cache: 0.34 ms less, and a file without the seal is never
  loaded (DESIGN.md, The engine);
- lengua-run, lengua built without the compiler, 5.7 MiB where lengua-mcp is 23.1 MiB: 1.16 ms
  and 5.6 MiB for add, where the same run by `lengua-mcp run` took 1.69 ms and 10.5 MiB;
- the program on the main thread: 0.18 ms less on x86-64, 0.23 ms on ARM64;
- no copy-on-write image of the module's memory: 0.11 ms less;
- only the host functions the module imports: 0.04 to 0.08 ms less on x86-64;
- the seal by ring's HMAC: 0.06 ms on ARM64 where the sha2 crate's took 0.32 ms.

Tried and left out, since they bought nothing: ring for the hashes of idea.md and main.wat,
and leaving what a run loaded to the process's end. A compiled module mapped from its file
would save 0.05 ms, and can't be held to a seal.

*Verified by:* at 45b507b, with main's W18 merged in, on all five runners: the suite, green
(43083574889); the release, each archive installed on a clean runner, where its lengua-run says
it holds no compiler and runs add from what `check` compiled, green (42975980745 at d6b871c,
43084441775 at 45b507b); and the bench (43084459708, and 43099764273 for Windows on ARM64, whose
files GitHub didn't keep the first time), whose reports are in
[docs/benchmarks](docs/benchmarks/README.md). An earlier bench (43022878824) ran into its time
limit on both Windows runners: lengua-run had left a module's first run to lengua-mcp, the bench
stopped lengua-run, and lengua-mcp went on serving on the port. Since then lengua-run on Windows
ties the lengua-mcp it starts to itself, so that both end together, and `measure` compiles a
module before the run it measures. On empty, lengua's medians in the bench before (42964232679)
and in this one:

| Runner | CPU time | Wall time | Peak memory | The program that runs it |
|---|---|---|---|---|
| Linux on x86-64 | 2.20 ms to 1.33 ms | 2.03 ms to 1.42 ms | 11.4 MiB to 5.50 MiB | 23.1 MiB to 5.87 MiB |
| Linux on ARM64 | 2.98 ms to 1.43 ms | 10.5 ms to 1.52 ms | 8.08 MiB to 4.34 MiB | 17.4 MiB to 4.88 MiB |
| macOS on ARM64 | 6.75 ms to 4.26 ms | 7.31 ms to 4.93 ms | 4.94 MiB to 3.02 MiB | 16.9 MiB to 4.62 MiB |
| Windows on x86-64 | in 15.6 ms ticks | 14.1 ms to 8.00 ms | 10.2 MiB to 5.80 MiB | 25.4 MiB to 6.03 MiB |
| Windows on ARM64 | in 15.6 ms ticks | 21.0 ms to 15.9 ms | 11.6 MiB to 7.12 MiB | 18.8 MiB to 4.91 MiB |

Against C's native build of empty, lengua took 5.49 times the CPU time and 9.42 times the
memory on Linux on x86-64 before, and takes 2.64 and 4.73 times now; on Linux on ARM64, 6.27 and
9.48 times before, 3.02 and 5.10 now; on macOS on ARM64, 3.26 and 3.98 before, 1.92 and 2.44
now. On Windows, where CPU time comes in ticks, its wall time went from 2.79 to 1.79 times C's
on x86-64 and from 1.91 to 1.44 times on ARM64, and its memory from 3.05 to 1.72 and from 2.49
to 1.53 times. C's module of empty, run by the same lengua-run, takes 1.27 ms and 5.41 MiB on
Linux on x86-64 (2.05 ms and 10.4 MiB before). A run's own cost over its module is 0.09 to
0.10 ms and 0.12 MiB there, and 0.08 to 0.11 ms on Linux on ARM64: the lock, its signature and
the hashes of idea.md and main.wat. With the other modules that much faster too, lengua's checks
now decide more of the short rows against them: the gate found 26 rows behind that
KNOWN_BEHIND doesn't name, and 137 it names level or ahead, which left it (761 rows stay).
Since 2026-11-11 the gate runs the other modules with the same checks, as any module lengua runs
for a user has them, and since W23 as outside modules (run 43191148225): the gate passed on
Linux on ARM64, no row against a WebAssembly build is behind and unnamed on any runner, and 15
rows against native builds are, each one a row the list named once (733 rows stay). Reading
an idea's grants and secrets in one pass, the one risk-free trim found in the checks, measured
no gain in run 43131768685 and is gone: a run's own cost over its module stays about 0.1 ms.

What a run still pays, on Linux on x86-64, of about 1.2 ms for add where a small Rust program
takes 0.6 ms: 0.2 ms to load lengua-run itself, whose file is 3.4 MiB of the run's 5.6 MiB; 0.2
ms for the checks; 0.12 ms for the engine and the compiled module, 0.3 ms on ARM64, where the
module is 135 KiB; and 0.2 ms to link, start, run and end the program. lengua-run still holds
TLS, for a program with connections, and the parser of main.wat, for a trap's line.

**Step 2, open:** an optional native build of the proven main.wat, through wasm2c and clang, as
Firefox sandboxes C libraries at close to native speed. The proof still holds, since it covers
WebAssembly's semantics, and main.wat stays the one shared artifact.

## W20. Parallelism

*Built on the branch `wip/threads`.* A program uses more cores by splitting its work into
pieces that run at once and share no memory, so that no rule has to show the absence of a data
race, and a result is the same on every run and every computer, which free threads in C, C++
and Rust can't promise.

The plan was WebAssembly's threads (shared memory and atomics) as structured fork-join, with a
rule that shows the parts share no writable memory. What was built instead gives each part a
memory of its own (DESIGN.md, "The host", has the two designs weighed):

- an idea's `- Splits:` line grants it and bounds it: how many pieces at most, how many bytes
  a piece gets at most, and how many bytes each gives back;
- `split`, of `lengua_split_1`, cuts the bytes into pieces by arithmetic, runs the same module
  once for each from `_start` in a store of its own, at once on as many cores as the computer
  has, and writes the answers in the pieces' order; `piece` says which run is one;
- a piece reads its share as standard input, prints its answer, reaches nothing else of what
  the idea grants, and can't split;
- `check` refuses a main.wat outside the line, and an idea with a `- Splits:` line and a
  `- Secret:` line;
- a test of a program that split runs again with the pieces one at a time, the last first;
  a test with the step `the program is a piece` runs as a piece, and the prover proves it as
  it proves any program that reads standard input. The step that joins the answers is tested,
  not proven.

`examples/histogram-split` counts the byte values of standard input 8 MiB at a time, each
buffer-full in eight pieces. Measured by lengua-run on 64 MiB, the best of seven runs on the
clock (`split_work_is_measured`, crates/lengua-run/tests/alone.rs; run 43148133679, at 92dec35), against
tests/fixtures/histogram, which does the same work on one core:

| Runner | Cores | One core (tests/fixtures/histogram) | Split, one piece at a time | Two at once | One for each core |
|---|---|---|---|---|---|
| Linux on x86-64 | 4 | 29.4 ms | 67.6 ms | 47.0 ms | 45.8 ms |
| Linux on ARM64 | 4 | 37.7 ms | 75.2 ms | 46.9 ms | 39.6 ms |
| macOS on ARM64 | 3 | 33.8 ms | 49.8 ms | 34.0 ms | 36.5 ms |
| Windows on x86-64 | 4 | 66.7 ms | 140.6 ms | 112.7 ms | 102.8 ms |
| Windows on ARM64 | 4 | 71.4 ms | 105.5 ms | 80.3 ms | 60.8 ms |

**For this work the split does not pay.** With a piece for each core the example is 1.4 to 1.9
times as fast as itself with one piece at a time, so the pieces do run at once. But against
the program that does the same work on one core it is slower on Linux on x86-64 and on Windows
on x86-64 (1.5 times the time), level on Linux on ARM64 and on macOS on ARM64 (1.05 and 1.08
times), and faster only on Windows on ARM64 (0.85 times). The run before it (43141059631, at
55443fd, where a piece still read its share 4 KiB a read) gave the same picture: 35.1 ms
against 19.0 ms on Linux on x86-64, 38.3 against 37.3 on Linux on ARM64, 78.5 against 40.2 on
macOS, 55.3 against 45.5 on Windows on x86-64, and 66.9 against 75.4 on Windows on ARM64. The
runners are shared machines of 3 or 4 cores, and a time moves by a third from run to run.

Why: the count takes about 0.3 ms a mebibyte on one core, and a piece costs more than that
before it counts: its bytes are copied twice, into lengua's memory and then into the piece's,
both into pages the system hands over for the first time, and each call starts its threads
and its stores anew. One piece at a time, the split program takes 1.5 to 2.3 times the
one-core program's time, and 3 or 4 cores, some of them two threads of one core, don't win
that back. A split pays where a piece's work on a byte costs several times a copy of it,
which none of lengua's examples yet does. What would cut the cost: threads kept through a
run, and a piece's bytes handed over without a copy.

What it leaves: pieces of another program than the program itself; a piece with arguments,
files or a clock of its own; a piece that splits; a proof of the join, and of the whole
program; a secret in a program that splits; a template in the compile skill; a workload in
the benchmark's gate, with C, C++ and Rust native builds that use threads beside it; and a
stop for a piece in a run from a shell, whose engine stops nothing. todo.md, "W20: work split
into pieces", has each with why.

*Verified by:* the suite on all five runners at 6790d4c, with main's W21 merged in (43160037821); the
example from its idea to done, with its proofs' lines (43136507348, Linux on x86-64); 7 of the
8 tests of tests/wasm_split.rs failing on the commit that took the defences out (43148079519,
at 3729579, which never reaches main); and the two measurements above, each on all five.

## W21. A library of proven parts

A shared, proven standard set (sorting, hash tables, JSON, HTTP, compression, dates) that programs
build on with `- Builds on:`. The agent assembles programs from verified pieces instead of writing
everything new, which makes them faster to build and to prove.

**The mechanism and the first five parts, built on 2026-11-11** (branch `wip/parts`). A part is
a program whose idea has a `# Contract` section: each function a program may take, with what it
reads and writes through the addresses it is lent. `check` holds the section to what the memory
rules find in the part's main.wat. A program names the part in its idea and copies each function
it takes into its own main.wat, under a `;; from:` line; at every check, `check` holds the copy
to the part word for word, with each constant it reads and each function it calls, and refuses
a part that changed, has no contract, or that a proof found wrong. main.wat stays the one text
lengua builds, checks, tests, proves and runs, so the rules, the secret rule and the proofs go
through a taken function as through any other, and the memory rules refuse a call that lends
less room than the contract states. lengua.lock records each part with its hashes, and
`check`'s reply names what a program takes from each part and what the part's own proofs cover.
The library, in the compile skill's `assets/parts` and in `examples/`: `decimal` (a whole number
read from decimal text and written as it), `lines` (an input read into memory, and where each
line ends), `crc32` (the CRC-32 of zlib, gzip and PNG), `bytes` (whether two runs of bytes are
the same) and `sort64` (up to 256 whole numbers of 64 bits, sorted where they are).
`examples/sum` and `examples/checksums` are built on them, and both skills teach an agent to
look for a part first.

*Proven, each by its own program:* decimal, lines and bytes for every input in their ranges, and
so `examples/sum`; sort64 for two numbers in order, three tested; crc32 for a text of 2
characters in five minutes, tested beyond, and so `examples/checksums`.

What it doesn't do yet: the prover proves a taken function again in each program, since a
contract states what a function reaches and not what it computes, so a program built on parts
is no faster to prove; a part's limits are fixed, 20 bytes for a number, 4096 for a text and
256 numbers for a sort; and of the set above, hash tables, SHA-256, the lines of an HTTP
request, a sort faster than by selection, JSON, compression and dates have no part (todo.md,
"W21's parts", has what is left and every run).

## W22. Scale

After W21 (the owner, 2026-11-11: "that evolution needs to function better than a programming
language. Not have stuff missing"). Every program so far is a few hundred lines of main.wat. W22
builds one of several thousand from an idea, and records what breaks first: the agent's hold of
the code, `check`'s time, the proofs, or the rules. What breaks gets its fix: ideas in parts that
build on each other, modules checked alone and linked, proofs kept for parts that didn't change.

Moved to the front on 2026-11-12, by the owner's word: it runs next, once W21's parts are on main,
before W23 and W24, as the test the direction rests on. It passes if the agent builds one real
program of that size and can then change it with `check` still green; if it fails, what broke
first chooses the next step (todo.md).

Run on 2026-11-12, on the branch `wip/scale`: **passed with conditions**. A web server of 3,638
lines of main.wat, `examples/httpd`, on four parts, was built in five check cycles and changed
three times (a feature that cuts across, a fix in the parser, a refactor into a part) with
`check` green after each; no test failed, and two rules were broken in about 4,100 lines written
by hand. The conditions: none of the server's generated tests is proven, since a server's wait
is not; the agent needed a count of operators and computed places for its literal texts, which
the product does not give; the idea was the harder text to keep right; and it was one agent in
one session. [docs/scale/README.md](docs/scale/README.md) has the measures and the verdict, and
[docs/scale/log.md](docs/scale/log.md) every check cycle.

## W23. Working with other code

A lengua program as a library other languages call (its module with a stated interface, and a
native build where the caller needs one), and a named outside WebAssembly module a lengua program
calls, held to the grants its idea gives it and marked "trusted, not proven". Each is declared in
the idea.

One part moved up beside W22 on 2026-11-12, by the owner's word: a module a compiler made from
Rust, C or another language, run as the program itself under an idea's grants and tests, held by
the host as any module is and marked "trusted, not proven" (todo.md).

That part is built and on main since 3fd8a3a (2026-11-12, run 43172439582 on all five runners):

- **The shape.** A folder holds `idea.md` and `main.wasm`. The idea says `computation:
  main.wasm` and has a line `- Module: main.wasm, built from …` in its own words. Its tests and
  grants are written as for any idea. It has no secret and builds on no part.
- **`check`** reads the module and refuses what lengua's host can't hold by itself: a feature
  the engine has off, an import that isn't a function of the host's with its type, a function
  of lengua's own modules without its grant, a memory not exported as `memory`, with no maximum
  or of more than 1 GiB, no `_start`, more than 16 MiB. Then it runs the idea's tests, stamps
  the module, and says `trusted_not_proven`, with every guarantee that does not hold in the
  reply's `outside`. It checks no rule, proves nothing, writes no entry in `verified`, and the
  attester doesn't attest the folder.
- **`run`, `lengua-mcp run` and lengua-run** run it in the same host, under the same grants,
  and say "trusted, not proven" before it starts.
- **The host stands alone.** Each guarantee that leaned on a rule of main.wat's is listed with
  what holds it now, in DESIGN.md ("An outside module") and in
  [docs/reviews/outside-2026-11-12.md](docs/reviews/outside-2026-11-12.md), with a test of a
  hostile module for each threat case and a run on a commit with the defences out.
- **Real compilers' output** is in the suite: a Rust program that reads a file its idea names,
  and a C program that is refused the network its idea doesn't grant, each built for
  `wasm32-wasip1` by CI, once, for all five runners.
- **Not held:** an outside module's loops have no bound but a run's deadline, and a run from a
  shell has none. Inside its own memory it is held to nothing. What it was built from is the
  idea's word.

The rest of W23, a lengua program as a library and an outside module that a lengua program
calls, is not started. Runs: todo.md's table.

## W24. Tools

What a programmer expects beside `check`: a run that stops at a line of main.wat and shows its
values, a profile of a run's time by function and line, and a trace of what a run asked the host
for. Each is a tool call or a flag. *Not started.*

## Before the pivot

Until 2026-11-06 lengua was native: the agent wrote `main.s`, assembly for Linux on x86-64 or on
ARM64, which lengua assembled with LLVM, checked, proved on the decoded machine code and ran in a
Linux sandbox through a broker. Its roadmap planned six targets in rounds R0 to R6: the documents;
six hosts, one workflow and the spikes; the shared OS edge and the broker, built on Linux on
2026-11-06; a Linux beside the agent, through WSL 2 and Lima; native macOS and Windows ports,
deferred that day; the performance finish line; and prover parity, the leftovers and the release.
The pivot superseded it. The tag `native-final` keeps that lengua, its roadmap included:
[ROADMAP.md at native-final](https://github.com/laughing-hermit/lengua/blob/native-final/ROADMAP.md).
