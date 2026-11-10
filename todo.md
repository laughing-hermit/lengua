# lengua's agenda: what's left for people

**What lengua is.** An AI language: an LLM is the compiler, and lengua validates its translations
instead of trusting them. A person's request becomes an idea file, `idea.md`: behaviors in
Given/When/Then with their tests inside, an Open Knowledge Format page naming no CPU or OS. The
agent compiles it to `main.wat`, WebAssembly text; lengua checks the code against its rules, tests
it, proves it with two solvers that must agree, and runs it confined. The program folders form a
knowledge graph that `check` keeps. The product is an Agent Plugin, two skills and one Rust MCP
server only, so it runs in any agent harness.

**Where lengua stands (2026-11-06).** The owner decided this day to move lengua from assembly to
WebAssembly ("we are now pivoting into web assembly"). The native lengua, Linux on x86-64 and
Linux on ARM64 with one suite and the mutation test passing on both (run 42178888801), is tagged
`native-final` (commit 723879e), and stays in the repository beside the WebAssembly path until
milestone W11. The WebAssembly lengua is built in milestones W0 to W15 below, each merged to main
once it is green on every runner. With it, `lengua-mcp` runs natively on Linux, Windows and macOS,
so the WSL 2 and Lima routes are superseded (branch `wip/routes-ci`, left unmerged).

**How the work is done.**
- Everything is built, tested, proven and measured on GitHub's free runners, which cost nothing
  since the repository is public. The owner's computer only holds the files, with one exception:
  the agent-level test runs in the live interactive session (the owner, 2026-11-06: "Don't use
  headless mode. Test in this interactive session."), never in headless `claude -p` and never in
  a spawned session.
- The pivot works on the branch `pivot/wasm`; each milestone merges to main when its runs are
  green, and its run ids are written below.
- An item leaves this file when it's done, so what's here is what's left.
- [ROADMAP.md](ROADMAP.md) holds each milestone's design, steps, and the runs that verify it.

## The finish line (set 2026-11-06, with the pivot)

lengua is finished when all seven of these are true. Each is shown by green runs on GitHub's free
runners, whose ids are written in the table below, and by nothing else. The runners are five:
Linux on x86-64 (`ubuntu-24.04`), Linux on ARM64 (`ubuntu-24.04-arm`), Windows on x86-64
(`windows-2025`), Windows on ARM64 (`windows-11-arm`) and macOS on ARM64 (`macos-15`). macOS on
x86-64 stays dropped (the owner: Apple makes no Intel Macs).

1. **An AI language, as an Agent Plugin and nothing else.**
   - Two skills (`write-idea`, `compile`) and one Rust MCP server (`check`, `run`), conforming to
     Agent Plugins 1.1.0 and Agent Skills; no hooks, no subagents.
   - A program is a folder of two files. `idea.md` is an idea file, as in Karpathy's LLM Wiki, and
     an Open Knowledge Format v0.2 page of type `Attested Computation`: behaviors in
     Given/When/Then, its tests inside, naming no CPU or OS. `main.wat` is the agent's WebAssembly,
     which lengua validates instead of trusting.
   - The folders are the nodes of a knowledge graph, their `Builds on` links and links between
     ideas its edges. `check` keeps it: `index.md` and `log.md` in the Open Knowledge Format's
     layouts, the links, the graph's health, and each idea's `verified` trust.
2. **One product everywhere.** The same `main.wat`, the same checks and the same verdicts on all
   five runners, each running the one suite natively. No runner finds a counterexample another
   doesn't, and each proof line has the same kind of verdict on every runner: proven, or bounded
   to a stated length.
3. **Tailored to each machine for speed.** `check` compiles each module ahead of time for the
   machine's CPU, with the fastest WebAssembly features the engine supports there, and tells the
   agent the CPU's caches. The program, and what it does, are the same everywhere.
4. **Competitive with C, C++ and Rust.**
   - On every workload and on each runner, the one program lengua ships beats C, C++ and Rust
     compiled to WebAssembly and run the same way: less CPU time, less wall time, less peak memory
     and a smaller module.
   - A gate shows it: the runs alternate between the languages, and lengua's median must be lower
     with 99% confidence (one-sided Mann–Whitney U); memory and size are compared as measured.
   - Against the same languages' native builds, every gap is measured and published per runner,
     in a list of what's behind that may only shrink (KNOWN_BEHIND).
   - Workloads: startup-bound (an empty program, add, hypotenuse), throughput (64 MiB streams,
     files, sort, a word tally, a checksum, a histogram), a server with 64 clients, many files,
     and the clock.
5. **Safe, proven, and a replacement for command-line and server programs.**
   - Every shipped program is proven to do what its idea says and never to trap, or bounded to a
     stated length, with z3 and cvc5 agreeing.
   - It reaches only what its idea allows: the checker refuses any other import, and `run` grants
     nothing else.
   - Data the idea names as secret reaches no branch, no memory address and no variable-time
     instruction.
   - DESIGN and the white paper say what's out of scope or excluded by design (threads until
     WebAssembly has them, GUIs, connections a program opens itself, UDP), and "Deferred" says
     what would bring each in.
6. **Truthful documents.** README, DESIGN, the BENCHMARKS files (generated from runner artifacts),
   the white paper and the skills say what the runs show, per runner, and nothing more.
7. **Shippable.** One plugin archive per runner's OS and CPU, holding `lengua-mcp`, the skills,
   the manifests and pinned z3 and cvc5, installed and checked on clean runners of all five through
   a plain MCP client, and checked once more by an agent in the interactive session. The archives
   are unsigned, and the owner publishes them.

## Where each stands

| Point | Now | Left |
|---|---|---|
| 1. AI language, Agent Plugin | The native plugin: `write-idea`, `compile`, `check`, `run`, `idea.md` and `main.s`, the `Builds on` graph | W1 (idea files), W3 (the graph), W6 (Attested Computation), W9 (the skills) |
| 2. One product everywhere | Two native Linux targets pass the one suite (run 42178888801) | W2, then W4 to W9 on all five runners |
| 3. Tailored | CPU tiers for x86-64 and ARM64 | W4 (the engine), W10 (caches), W12 |
| 4. Competitive | The gate built (W12): `lengua-mcp measure`, 14 workloads with lengua's programs and C, C++ and Rust in WebAssembly and native, the Mann-Whitney U looks, KNOWN_BEHIND's ratchet, the bench on all five runners and weekly; no runner has run it yet | The gate's first run on each runner, KNOWN_BEHIND from it, and the speed work it shows |
| 5. Safe and proven | Native rules, proofs and Linux sandbox | W5, W8, W10, W13 |
| 6. Truthful documents | Describe the native lengua | W15 |
| 7. Shippable | Native archives for two Linux targets | W14 |

## Decisions made during the pivot, for the owner to review

The owner was away while these were made (2026-11-06: "im not going to check up on you anymore").
Each can be reversed.
- **lengua is written in Rust and WebAssembly only** (the owner, 2026-11-09: "ok go with rust
  and webassembly, proceed"):
  - Rust is everything lengua is: the server, its tools and command line, and its tooling.
    lengua's scripts become Rust (W17), and GitHub's workflow file stays only as glue.
  - WebAssembly is everything lengua makes: main.wat, lengua's library, the templates and the
    programs. They stay WebAssembly text, because the checker, the prover and the agent read
    them line by line, so not 100% Rust.
  - The benchmark's C and C++ programs stay, being the competitors point 4 measures, and so do
    the manifests the Agent Plugins standard requires, data, and documents.
  - Four choices made with it, after research (each with its sources in W17's brief):
    - "Only" covers lengua's own code and tooling. Third-party programs and crates stay, pinned
      and checked, and DESIGN names the C they carry: the solvers, ring for TLS, zstd, and a
      file of Wasmtime's.
    - z3 runs as WebAssembly if it costs fairly. That means the same verdicts on all five
      runners at today's limits, with the replayed questions taking at most twice as long in
      all. Another project already runs z3 under Wasmtime, with 413 of 414 answers the same
      as native z3. If the bar isn't met, native z3 stays and the measurement is written here.
    - cvc5 stays native, since no build that Wasmtime runs exists yet. After z3, one bounded try
      to make one: at most five build runs.
    - The site (`site/`) goes, and the tag keeps it. Static pages built by Rust can come back
      when the owner wants to publish one.
- **The code is WebAssembly text** (`main.wat`), written by the agent and validated by lengua.
- **The interface is WASI preview 1**, which every WebAssembly runtime runs and Wasmtime keeps
  supporting. lengua implements the part its programs use, holding the semantics of today's 21
  operations and the broker's rules for names, so a program runs the same way on every system.
  A program that only reads and writes files and standard streams also runs under any WASI
  runtime.
- **lengua's own host, not Wasmtime's WASI**: lengua implements its part of WASI preview 1 itself,
  on the engine's linker, so its semantics are its own (one closed set of errors, the rules for
  names, test-supplied clocks and random bytes, connections made before the start) and the prover
  models one implementation. Folders are reached through `cap-std`, which resolves every name
  beneath the folder.
- **Wasmtime is the engine** (Cranelift), pinned exactly, embedded in `lengua-mcp`: WebAssembly
  2.0 with SIMD128, bulk memory, multi-value, sign extension and saturating conversions on;
  threads, relaxed SIMD, wide arithmetic, 64-bit memory, multiple memories, tail calls, GC and
  exceptions off (wide arithmetic and relaxed SIMD aren't finished standards). NaN results are
  made canonical, so every machine computes the same bits. Where Cranelift fails on a runner
  (Windows on ARM64 is the engine's third tier), its portable interpreter (Pulley) runs instead,
  and the gap is published. The pinned version is Wasmtime 50.0.1. Reference types stay on in
  the engine, as today's C, C++ and Rust compilers encode their modules with them (which the
  comparison of point 4 runs); the dialect's rules refuse tables in `main.wat`. A module's one
  table holds at most 1,048,576 entries, whatever it declares or grows to, so no module takes
  the worker's memory through a table.
- **The host's answers** follow the compile skill's host.md: a name that would leave the working
  folder gives `NOTCAPABLE`, a link on the way `ACCES`; closing a standard stream needs files or
  the network in the idea. The terminal's two non-WASI imports (raw mode, window size) wait for
  W9, which ports the terminal template or leaves it out.
- **The text is parsed with `wast`**, keeping each instruction's line, and validated by
  `wasmparser` with the engine's own features. lengua's library (`$assert_fail`, `$fill`,
  `$write_all`) is merged into the module, so a program is one standard module. `main.wat` has no
  lengua header line; the lock and `verified` record what it was checked as.
- **Programs run in child processes**: a pool of reused `lengua-mcp --wasm-worker` processes runs
  the tests, each run in a fresh store; `run` and a background server each get a fresh child;
  a worker ends when the server does. Limits: memory up to what the module declares, time by the
  engine's epochs, 8 MiB of output, 64 MiB a file and a total for all files. Instruction counts
  become the engine's fuel, WebAssembly operations executed, the same on every system. An OS
  sandbox as a second wall is deferred.
- **Descriptors by role**: 0 to 2 the standard streams, 3 the working folder (as `.`), 4 the
  listener, 5 on the connections lengua made. Files live in the working folder, as before.
- **The clock is rounded to a microsecond**, after OpenAI's 2026 report of agents signalling to
  each other through the CPU cache's timing.
- **No threads** until WebAssembly's threads are standard, and no fused multiply-add (core
  WebAssembly has none): an idea with `- Threads:` or `fma` is refused. The parallel template, and
  the tiers that need CPU instructions (AES, SHA, CRC, AVX-512, SVE), are retired.
- **`idea.md` is an Open Knowledge Format `Attested Computation`**: `runtime: webassembly`,
  `computation: main.wat`, `parameters` with a lengua key `from` (an argument, standard input, an
  environment variable or a file), `run` its executor and `check` its attester. OKF §6.2 makes
  `executor.resource` and `attester.resource` paths or URLs, so an idea names two pages of this
  repository, [docs/run.md](docs/run.md) and [docs/check.md](docs/check.md). The format says an
  agent must not author a computation; lengua reads that as: the agent writes `main.wat` only while
  the idea is unverified, and any edit starts a new, unverified definition. `check` writes
  `verified: [{ by: lengua/<version>, at, idea: sha256:…, code: sha256:… }]` at `done` (a
  line-level edit, never re-serializing the YAML), removes its own entry when a changed idea is
  recorded, and keeps anyone else's. The lock's hash of the idea leaves out `verified` and
  `generated`. The lock's format is 5 after the rename and 6 with the WebAssembly code stamp.
- **The skill `write-spec` is now `write-idea`.**
- **Idea files are in ASD-STE100 Simplified Technical English** (the owner's words), in the
  style of behavior-driven development. `check` holds an idea's prose to what a computer can see
  without STE's dictionary: 25 words a sentence, six sentences a paragraph, no -ing form but
  technical names, and the steps in the step vocabulary's STE forms, each old phrasing refused
  with its rewrite. The rest of STE is the `write-idea` skill's to teach. lengua states the rules
  in its own words and copies nothing of STE's dictionary.
- **The knowledge graph's root** is the nearest folder above whose `index.md` has `okf_version`,
  else the program folder's parent; lengua never writes `index.md` or `log.md` in a home folder,
  a filesystem's root or the system's temporary folder; it changes only its own entries; `log.md`
  never quotes a request's words. An `index.md` in the parent without `okf_version`, such as a
  site's home page, isn't lengua's: lengua leaves it as it is and writes no `log.md` beside it.
  **No MCP resource**: the files are the graph.
- **The install scripts don't clear a download's mark** (macOS's quarantine, Windows's mark of the
  web): the archives are unsigned, so trusting one is the user's choice, and the scripts and
  doctor only say how. Signing and notarizing the archives are the owner's, with publishing.
- **The signing key stays in the per-user data folder**, so a lock made in one agent harness
  verifies in another; `PLUGIN_DATA` holds only caches.
- **Memory regions** in `main.wat`: a region `$x` is an immutable global holding its address,
  with `$x.size` (writable) or `$x.rosize` (read-only); addresses 0 to 1023 are lengua's. Rule 4,
  a function's size, starts at 180 operators, to be calibrated on the templates.
- **The performance finish line** compares lengua with C, C++ and Rust compiled to WebAssembly
  (built once, on Linux, and shared) and run the same way, process against process, and publishes
  the gaps against their native builds (point 4).
- **The gate's looks** (W12): a row's runs grow until both of its tests decide, so the gate looks
  at 20, 40, 80, 160, 320, 640, 1,280 and 2,000 runs of each version, and each look decides at
  p < 0.00125, the 1% split over the eight looks (Bonferroni's bound): a row looked at again and
  again is still decided wrongly with a chance below 1%. A row also stops at ten minutes; one the
  looks don't decide is undecided, and KNOWN_BEHIND must name it.
- **What the gate runs** (W12): lengua's program through `lengua-mcp run`, with every check it
  makes on each run (the lock, its signature, the module check kept held to the lock's hash of
  it, or main.wat built again where none is kept); C, C++ and Rust's modules in
  lengua's host through `lengua-mcp measure`, with the same engine and the same cache of compiled
  modules, the policy from the words of lengua's program's idea, and memory that may grow to
  4 GiB, as lengua's host holds a module to the maximum it declares. The native builds use each
  system's own compilers: GCC on Linux, Apple's clang on macOS, LLVM's clang on Windows, rustc
  everywhere, each for this CPU with link-time optimization, falling back to a CPU flag every
  runner of its kind has where the compiler doesn't take `native`.
- **KNOWN_BEHIND's first state** (W12) names every row of each runner, as none is measured yet.
  Each runner's first gate run fails wherever lengua is ahead, and its KNOWN_BEHIND-<target>.txt
  (in the run's artifacts) takes the place of that runner's line, naming only the rows where
  lengua isn't ahead.
- **KNOWN_BEHIND only shrinks** (W12): the bench holds each change to the list in the history it
  measures to the list before it, and fails where a change named a row the list before it didn't
  (a runner, workload, version or measure new to the gate excepted), so a bench leg checks out
  the whole history.
- **Peak memory on Linux** (W12): Linux begins a process's count of its peak at what the process
  it was started from held, so `measure` starts each program from lengua-spawn, a small process
  beside lengua-mcp that forks it and counts it; without lengua-spawn, a peak no higher than
  lengua-mcp's own is left out, undecided.
- **Size against native builds** (W12): lengua's size on those rows is its module's and the
  lengua-mcp executable's that runs it; against the other modules, the module's alone.
- **The other versions' forms** (W12): C, C++ and Rust use their own libraries' sorts and plain
  loops their compilers vectorize, with no SIMD by hand, where lengua's programs use SIMD128 by
  hand; each workload's description in the report says so. Their programs carry no handicap the
  others don't: C stores every byte, as C++ and Rust do; wc counts in byte counters in all three;
  C++'s tally uses C's table; Rust writes large outputs straight to its descriptor, as C does.
  The gate's test corrects for continuity by half of U's step between runs of tied values.
- **Secrets**: an idea line `- Secret:` names data that must not reach a branch, a `select`, an
  address, a length, a loop's count, division or floating point (point 5), checked in the
  WebAssembly. Checking the machine code the engine makes as well (W13) was stopped by the safety
  filter and is the owner's, so that code is trusted for timing, as DESIGN says.
- **A generated test may repeat fixed text** (W12, b878286): `` `\n` `4194304` times then
  `{line}` `` is written out in its template, at most 8 MiB as an example's repeat is, and a
  placeholder is never repeated. A limit no generator reaches, such as sort-large's 4194305
  lines, gets its generated test, as the write-idea skill gives every limit a behavior.
- **z3's library ships only where z3 loads it** (W14, 7835d9a): Linux's and macOS's z3 have it
  built in, and z3's .NET binding is left out everywhere. The archives shrank by about a quarter
  on Linux and macOS (Linux on x86-64: 53.7 MB to 39.2 MB), release and install green on all
  five (42387777039).
- **Five runners**, as above; macOS on x86-64 stays dropped (the design pass would have kept it;
  the owner's earlier word stands).

## Roadmap: the pivot's milestones (2026-11-06)

Each merges to main when green on the runners it names; [ROADMAP.md](ROADMAP.md) has the detail.

- **W0. Record the decision.** This file, ROADMAP.md, README's opening, the tag `native-final`.
- **W1. Idea files.** `spec.md` becomes `idea.md` and `write-spec` becomes `write-idea`,
  everywhere, with the lock's format bumped, and the manifests on Agent Plugins 1.1.0.
- **W2. CI on all five runners** again: lengua builds and its tests pass natively on each (native
  programs on Linux only).
- **W3. The knowledge graph**, in the Open Knowledge Format, on today's program folders.
- **W3b. Idea files in Simplified Technical English**: `check`'s STE checks, and every idea file
  in the repository rewritten in STE.
- **W4. The WebAssembly runtime**: lengua's WASI host, the engine, the worker pool.
- **W5. The WebAssembly dialect's rules.**
- **W6. `check` and `run` for WebAssembly ideas**, end to end, with `idea.md` an Attested
  Computation.
- **W7. The prover's seam**: the world it models taken out of the CPU's state, with no verdict
  changed.
- **W8. The WebAssembly prover**, in four steps: the concrete model, symbolic proofs, every length,
  SIMD and floats.
- **W9. Every program in WebAssembly**, the skills rewritten, and the parity ledger.
- **W10. Constant time and caches.**
- **W11. The native product retired** (kept at the tag).
- **W12. Performance and the gate.**
- **W13. Constant time in the machine code.** Stopped by the safety filter on 2026-11-07, and the
  owner's (below).
- **W14. Package, `doctor` and release** on all five runners, and the agent's test in the
  interactive session.
- **W15. The documents, the finish-line run, and the report.**
- **W16. The repository tidied and presentable on GitHub** (the owner, 2026-11-07: "when
  finished with lengua please tidy up the repo and make it presentable please in github"): a
  README that presents lengua, the top level pruned, and the repository's description and
  topics on GitHub.
  - **Only main remains** (the owner, 2026-11-09: "remember at the end only main branch shoudl
    remain"). Every other branch goes, on GitHub and on the owner's computer, including
    `wip/routes-ci`, `wip/r2-8a-snap` and `wip/r2-base-check`.
  - First, a branch whose commits main doesn't hold keeps its tip as a tag `archive/<branch>`,
    so nothing is lost. A tag isn't a branch, and the tag `native-final` stays as it is.
- **W12b. lengua's run made lean.** The first gate run (42661776298, all five runners) found
  lengua's program ahead on module size nearly everywhere, and on time where the work is large
  (wc, files, readings, part of sort). It is behind on time and memory wherever starting dominates,
  and on memory almost everywhere. That gap is `lengua-mcp run`'s own work around the program
  (reading the whole idea, building main.wat again) and lengua's library merged in whole. W12b
  cuts it and keeps every guarantee. KNOWN_BEHIND holds each runner's rows from that run, and
  may only shrink.
- **W17. lengua in Rust and WebAssembly only** (the owner, 2026-11-09):
  - The shell, PowerShell, sed and Python scripts become Rust: `lengua-mcp install` for the
    install scripts, and a Rust task crate for release, CI setup and the goldens.
  - Each step of the workflow file calls into Rust.
  - The site is rebuilt in Rust or left out.
  - The solvers run as WebAssembly if the research shows they can at a fair cost.

Runs, as they land:

| Milestone | Runs |
|---|---|
| W0 | 42295197937 (compile, both Linux runners); tag `native-final` at 723879e; main at fc86dd3 |
| W1 | 42304173338 (test, both Linux runners, at ff68d40; the QEMU job's known failures aside); 42296519743 (the golden ledger against `native-final`: no checker or prover verdict changed) |
| W2 | 42304371890 (the suite natively on all five runners at f762aa2; macOS's one failure, a proof's time test that a fast machine finished early, is fixed at e9f4700) |
| W3 | 42308171081 (the suite on all five runners at 5acdb65; its format patch is applied at e9f4700) |
| W3b | 42317583638 (at 2fb46f1, the chain's tip: format, the suite on all five runners and the gate, green); main fast-forwarded to it |
| W4 | 42327997413 (the runtime at 63af1eb: format, the suite on all five runners and the gate, green, with the 109 host cases and the limit tests passing the same on every runner); its security and correctness review's 15 findings fixed before it; main fast-forwarded to the same code |
| W5 | 42336485988 (the rules at 0534a06, on the seam: format, the suite on all five runners and the gate, green; the red team's refusals and the template passing every rule); its review's 23 findings fixed before it |
| W6 | 42329274411 (the Attested Computation form alone, at 5791d73); 42345074041 (check and run for main.wat at 1c75e7d: format, the suite on all five runners and the gate, green, add from the request to done, run, `lengua-mcp run` and `lengua-mcp check`); the reviews' 15 and 17 findings fixed before them |
| W7 | 42325471157 (the golden ledger, raw: the checker's and the prover's recordings identical, byte for byte); 42336209846 (the seam on main at d280fa1: format, the suite on all five runners and the gate, green) |
| W8 | 42344545735 (the WebAssembly model and proofs, z3 and cvc5 on every runner, with add, echo and greet proven and planted bugs found); 42348682819 (stacked on W6 at dac4c7d: format, the suite on all five runners and the gate, green); the review's 11 findings fixed before them; 42362037786 (check proves main.wat, at c6508b4: format, the suite on all five runners and the gate, green; the golden ledger 42357663706 differs only in sha256's load-dependent x86-64 line, not proven either way); 42386526911 (every length by induction over a read loop, step 3, and SIMD128 and floats in the model, step 4, at 2b71184: format, the suite on all five runners and the gate, green, with every covered operator compared with the engine's bits); its review's 4 findings fixed before it (a read before the loop's head, a time round that changes the working folder, count's flag, the layout); 42388141609 (the golden ledger against W8's earlier base: no native verdict changed); on main with W10 at 2727216 (42391963513) |
| W10 | 42386323947 (the secret rule over main.wat and the caches on every system, at df1e8ae: format, the suite on all five runners and the gate, green); on main, stacked with W8's steps 3 and 4, at 2727216 (42391963513: the suite on all five runners and the gate, green) |
| W11 | 42713054140 (the native product retired, at 9ab14a5: format, the suite on all five runners and the gate, green); 42707398628 (release at 8d196ee: the five archives built without it, each installed on a clean runner, doctor and the release check, green); main at 3dae90d, whose push suite 42716434491 is green; 42716553289 (the golden baseline for WebAssembly, recorded twice at 3dae90d and the same both times); its three reviews' 15 findings fixed before them |
| W12 | 42393960971 (the benchmark, its workloads and the gate, at 9a549b3: the suite on all five runners and the gate, green); 42661776298 (the first gate run on all five runners: each runner's KNOWN_BEHIND, from its own KNOWN_BEHIND-<runner>.txt, at 2bad55a); on main with W11 |
| W13 | Stopped by the safety filter, left for the owner (below) |
| W14 | 42387284972 (the suite on all five runners and the gate at 83641b5); 42387777039 (release at 7835d9a: the five archives built, each installed on a clean runner of its system and CPU, doctor and the release check, green); main at 7835d9a. The agent's test in this session, 2026-11-07, with that Linux x86-64 archive: below |
| W9 | 42356699038 (the WebAssembly templates, wasm.md and write-idea's Attested Computation, at 50a546f); 42367496557 (all 29 programs in WebAssembly from the request to done, at b761dd3); 42373720077 (the compile skill for main.wat, [PARITY.md](PARITY.md) and 0.2.0); each with format, the suite on all five runners and the gate, green |

## Superseded by the pivot

These were open for the native lengua and no longer apply; the native lengua's full list is in
this file at the tag `native-final`.
- **Round 2's leftovers:** the QEMU job's seven failing cases, plain runs' goldens, sha256's
  load-dependent proof bound on x86-64, upcase's retries and the skipped assertion, the stamp's
  CPU names, moving the CPU harnesses onto the edge. The WebAssembly lengua has no QEMU, no
  per-CPU code and no CPU stamp; its own goldens come with W5 and W8. The QEMU job now runs only
  when a dispatch's `targets` names `qemu`.
- **Round 3:** the Ubuntu package with LLVM, the install test through `setup.sh`, the kernel
  floor and Landlock, `doctor` for Linux kernels, musl or an older glibc, LLVM's version, WSL 2 on
  `windows-11-arm`, and the routes in CI. `lengua-mcp` needs no LLVM, no kernel feature and no VM
  now; W14 builds the package and `doctor` for every system.
- **Later, the image:** a container image of a Linux package isn't needed for Windows or a Mac
  any more. It can still come later for servers, if wanted.
- **The native macOS and Windows ports** (ROADMAP's deferred round 4): the WebAssembly lengua
  reaches both natively.
- **Round 5's native gaps:** the GNU toolchain A/B, `crc32c`'s wall time and `move_front`, the
  histogram's loop placement and the dialect's code alignment, the executables' size, the SSE2
  one-thread shout, and ARM64's missing bench programs. W12 measures the WebAssembly programs
  afresh; the gate and its workloads carry over.
- **Round 6's native items:** the Linux sandbox's twelve known limits (W4 and W14 set the
  WebAssembly host's limits instead), the `.equ` checks against the edge, a pointer moved by a
  register, the one-spec layout across CPU folders, the raw system-call model, the x86-64 and
  ARM64 prover's missing instructions, signed lane compares, the xmm-only harness, the
  zero-register branch, AArch64's `adc` and `sbc`, and the small items. The release check and the
  last pass over the documents carry over (W14, W15).
- **Fast paths on Linux** (the vDSO clock, `epoll`, `splice`): the host's own I/O replaces them.

## The owner's

- **A `gate` job as a required check**, with branch protection.
- **Publishing** the release archives, the plugin listing where agent plugins are found, and the
  site (`site/`). The repository has been public since 2026-11-02, a work in progress under the
  Apache License 2.0.
- **Signing.** Code signing costs money, so the archives are unsigned.
- **Launcher services for compression and a GUI.** Each would be one definition that every system
  must provide outside the program, as lengua does TLS; a GUI is a product of its own. Not
  designed yet.
- **A row the gate can't decide.** Some costs every language pays alike: copying through a pipe,
  an open, the clock. Where one of those dominates a row, lengua's lead is only its own few
  instructions, and the row may stay undecided or tied after every fix. W12 names those rows, with
  the runs. For each, the owner then picks: keep the finish line open, or accept that row as a
  stated exception. Until they pick, it stays open.
- **The decisions above**, each the owner's to keep or reverse.

## Deferred, and why

- **A knowledge graph over more than program folders.** W3 makes the program folders a graph in
  the Open Knowledge Format, with `index.md` and `log.md`. Indexing other
  files, or answering questions over the graph, needs a design the owner hasn't asked for.
- **Connections made on demand.** A program that opens a connection itself, as a proxy does,
  needs WASI's newer sockets (preview 2 or 3) and spec syntax. Today a program gets the
  connections its idea names, made before it starts, and that carries over.
- **Threads**, until WebAssembly's threads are standard and Wasmtime runs them.
- **An OS sandbox around the engine** as a second wall: Linux's seccomp and Landlock from the
  native lengua, and each other system's own. It would bring back per-OS work.
- **Native kernels**: assembly a WebAssembly program calls for its busiest loops, for speed. Only
  if each is proven for every input and runs inside an OS sandbox, since it runs outside
  WebAssembly's.
- **Checking the engine's machine code for constant time**, with the native lengua's x86-64 and
  ARM64 decoders, so the timing guarantee doesn't rest on the engine's compiler.
- **Proofs past the finish line.** Research: sorting at every length, word tallies past a few
  bytes, byte loops at 64 KiB, and long chains of mixing rounds.
- **A pointer read from memory.** The memory rules can't follow it to an owner, so a program that
  uses one is "tested, not proven" for memory.
- **UDP, any URL, processes, GUIs, libraries for other languages to call:** outside the finish
  line's scope; W15 has DESIGN say what would bring each in.

## Stopped by the safety filter

Claude's safety filter has stopped some replies. What a stop hits is listed here, and never
routed around through other models or subagents.
- **2026-10-31 to 2026-11-02.** The items those stops left were done on 2026-11-04 by Sonnet 5.5,
  with the owner's permission, on GitHub's runners and on both Linux targets: two examples, the
  hardening pass's red team (done defensively), and two ARM64 twins. The record is in the git
  history and DESIGN.md. The examples show algorithms; they aren't a product that resists an
  attacker. lengua isn't to be presented as covering security because of them, or because of
  its checksums.
- **2026-11-04.** A planning reply (Opus 5.5) was stopped while this file's sections were being
  read for the six-target plan. Nothing was lost. Its content isn't produced again, and the topic
  near it is left alone.
- **2026-11-05.** A reply (Opus 5.5) was stopped while round 1's workflow and the macOS and Windows
  spikes were being planned. Nothing was lost, and its content isn't produced again. With three
  stops near the same ground, the tests that try to get past the sandbox on macOS and Windows are
  left for the owner or another engineer.
- **2026-11-07.** A review of W10's secret rule for soundness (src/checker/wasm/secret.rs on
  `wip/secret`) was stopped partway, when it had only read files. It confirmed no finding and
  showed nothing about the rule, so the rule hasn't passed a soundness review. Its content isn't
  produced again, and the owner decides how the review goes on.
- **2026-11-07, W13.** A reply (Opus 5.5) was stopped while W13, checking the engine's machine
  code for an idea's secrets, was being designed, before anything was written. It is the second
  stop on that ground in a day, so W13 is left for the owner: not retried, reworded or passed to
  another model. The secret rule over main.wat (W10) stands, and the engine's compiled code is
  trusted for timing. W11 removes the native decoders W13 would have used; the tag
  `native-final` keeps them.

## The agent's test in this session (W14, 2026-11-07)

The owner's rule is that an agent-level test runs only in the live interactive session. Claude
Code can't load a plugin into a session already running (`claude --plugin-dir` starts a new one),
so the session's agent was also the MCP client: it started the archive's own `lengua-mcp` over
standard input and output, read the archive's two skills, and followed them as written. Loading
the plugin in Claude Code is the one step left for the owner (`/plugin`, or `claude
--plugin-dir`).
- **The archive:** lengua 0.2.0 for Linux on x86-64, from release run 42383680141. Its SHA-256
  matched its `.sha256`. `lengua-mcp doctor` said nothing is wrong.
- **The program:** vowels, which counts the vowels in standard input of any length. The request
  was the test's own words, since the owner asked for no such program.
- **The idea:**
  - The first `check` listed its three problems at once, each with the fix.
  - Once they were fixed, the idea was recorded.
- **The code:**
  - The first main.wat, from the stream template, went to `done` on its first code check, after
    one STE problem in the idea was fixed. It followed every rule and passed 172 test runs.
  - One generated test was proven for every input in range. The others were proven up to 93 and
    89 bytes and tested beyond, as that build's prover went one length at a time (W8's steps 3
    and 4 came after it).
- **Running it:**
  - `run` printed 21 for a 66-byte text, and `tr` agrees.
  - From a shell, `lengua-mcp run` counted 625,299 vowels in 4 MB in 35 ms, the same count as
    `tr`. With its output closed, it exited with code 2.
- **The attester:** `lengua-mcp check` said `attested` after five minutes and wrote nothing.
- **The graph:** index.md and log.md are in the Open Knowledge Format's layouts.
- **Speed:** the compile skill's speed pass rewrote main.wat to take 16 bytes a step with
  SIMD128. It passed every rule and test on its first check. A 1 MiB input went from 1,729 µs to
  197 µs (8.8 times faster), and its proofs were then "tested, not proven" at `I8x16Splat`, as
  the skill says for that build.
- **What it found:**
  - The archive shipped 35 MB of z3 library that z3 doesn't load; fixed in 7835d9a.
  - Several native words remain: plugin.json's description, the server's instructions, the
    native fields of `run`'s reply, and the native examples in the test format's reference (W11).
  - The first recording's reply both lists the program in index.md and calls it an orphan.

## Where the work broke its own ground rules

The rule is that this computer only edits, reads and runs git and gh. These broke it, each once,
none changed a result, and each is recorded here for the owner:
- W5's implementer ran `rustfmt` on files here.
- W12's implementer, the session itself on W14's scripts/release.sh, and W11's part C each ran
  `bash -n`, a shell's syntax check that runs nothing.
