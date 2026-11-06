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
milestone W10. The WebAssembly lengua is built in milestones W0 to W13 below, each merged to main
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
| 1. AI language, Agent Plugin | The native plugin: `write-idea`, `compile`, `check`, `run`, `idea.md` and `main.s`, the `Builds on` graph | W1 (idea files), W5 (the compile skill for WebAssembly), W6 (the graph) |
| 2. One product everywhere | Two native Linux targets pass the one suite (run 42178888801) | W2 to W5 on all five runners |
| 3. Tailored | CPU tiers for x86-64 and ARM64 | W2 (engine, caches), W8 |
| 4. Competitive | Native programs measured, none gated | W8 (the gate) |
| 5. Safe and proven | Native rules, proofs and Linux sandbox | W3, W4, W7 |
| 6. Truthful documents | Describe the native lengua | W11 |
| 7. Shippable | Native archives for two Linux targets | W9, W12 |

## Decisions made during the pivot, for the owner to review

The owner was away while these were made (2026-11-06: "im not going to check up on you anymore").
Each can be reversed.
- **The code is WebAssembly text** (`main.wat`), written by the agent and validated by lengua.
- **The interface is WASI preview 1**, which every WebAssembly runtime runs and Wasmtime keeps
  supporting. lengua implements the part its programs use, holding the semantics of today's 21
  operations and the broker's rules for names, so a program runs the same way on every system.
  A program that only reads and writes files and standard streams also runs under any WASI
  runtime.
- **Wasmtime is the engine** (Cranelift), embedded in `lengua-mcp`, with SIMD, wide arithmetic and
  bulk memory on, and threads, relaxed SIMD, 64-bit memory, GC and exceptions off.
- **No threads** until WebAssembly's threads are standard; the parallel template is retired.
- **Programs run inside the server**, one store per run, with memory, time, output and file
  limits from the idea; a shell's `lengua-mcp run` gets its own process. An OS sandbox as a second
  wall is deferred.
- **The clock is rounded to a microsecond**, after OpenAI's 2026 report of agents signalling to
  each other through the CPU cache's timing.
- **`idea.md` is an Open Knowledge Format `Attested Computation`**: `runtime: webassembly`,
  `computation: main.wat`, `run` its executor and `check` its attester. `check` writes
  `verified: { by: lengua/<version>, at }` when everything passes, and the lock's hash leaves the
  trust and lifecycle keys out.
- **The skill `write-idea` is now `write-idea`.**
- **The graph is also an MCP resource**, read-only, beside the two tools.
- **The performance finish line** compares lengua with C, C++ and Rust compiled to WebAssembly and
  run the same way, and publishes the gaps against their native builds (point 4).
- **Secrets**: an idea line `- Secret:` names data that must not reach a branch, an address or a
  variable-time instruction (point 5).
- **Five runners**, as above; macOS on x86-64 stays dropped.

## Roadmap: the pivot's milestones (2026-11-06)

Each merges to main when green on the runners it names; [ROADMAP.md](ROADMAP.md) has the detail.

- **W0. Record the decision.** This file, ROADMAP.md, README's opening, the tag `native-final`.
- **W1. Idea files.** `idea.md` becomes `idea.md` and `write-idea` becomes `write-idea`,
  everywhere, with the lock's format bumped.
- **W2. The WebAssembly foundation.** The engine, the build, lengua's WASI host, the runner, the
  `machine` line with the CPU's caches, the first WebAssembly fixtures, and CI on all five
  runners.
- **W3. The WebAssembly dialect's rules.** Power of 10 and the memory rules, for WebAssembly.
- **W4. The prover's WebAssembly front end.**
- **W5. Every program in WebAssembly**, and the compile skill rewritten for it.
- **W6. The knowledge graph**, in the Open Knowledge Format.
- **W7. Constant time.**
- **W8. Performance and the gate.**
- **W9. Package, `doctor` and release** on all five runners.
- **W10. The native product retired** (kept at the tag).
- **W11. The documents and the finish line.**
- **W12. The agent's test, in the interactive session.**
- **W13. The finish-line run, and the report.**

Runs, as they land:

| Milestone | Runs |
|---|---|
| W0 | 42295197937 (compile, both Linux runners); tag `native-final` at 723879e; main at fc86dd3 |
| W1 | (to come) |

## Superseded by the pivot

These were open for the native lengua and no longer apply; the native lengua's full list is in
this file at the tag `native-final`.
- **Round 2's leftovers:** the QEMU job's seven failing cases, plain runs' goldens, sha256's
  load-dependent proof bound on x86-64, upcase's retries and the skipped assertion, the stamp's
  CPU names, moving the CPU harnesses onto the edge. The WebAssembly lengua has no QEMU, no
  per-CPU code and no CPU stamp; its own goldens come with W3 and W4.
- **Round 3:** the Ubuntu package with LLVM, the install test through `setup.sh`, the kernel
  floor and Landlock, `doctor` for Linux kernels, musl or an older glibc, LLVM's version, WSL 2 on
  `windows-11-arm`, and the routes in CI. `lengua-mcp` needs no LLVM, no kernel feature and no VM
  now; W9 builds the package and `doctor` for every system.
- **Later, the image:** a container image of a Linux package isn't needed for Windows or a Mac
  any more. It can still come later for servers, if wanted.
- **The native macOS and Windows ports** (ROADMAP's deferred round 4): the WebAssembly lengua
  reaches both natively.
- **Round 5's native gaps:** the GNU toolchain A/B, `crc32c`'s wall time and `move_front`, the
  histogram's loop placement and the dialect's code alignment, the executables' size, the SSE2
  one-thread shout, and ARM64's missing bench programs. W8 measures the WebAssembly programs
  afresh; the gate and its workloads carry over.
- **Round 6's native items:** the Linux sandbox's twelve known limits (W2 and W9 set the
  WebAssembly host's limits instead), the `.equ` checks against the edge, a pointer moved by a
  register, the one-spec layout across CPU folders, the raw system-call model, the x86-64 and
  ARM64 prover's missing instructions, signed lane compares, the xmm-only harness, the
  zero-register branch, AArch64's `adc` and `sbc`, and the small items. The release check and the
  last pass over the documents carry over (W9, W11).
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
  instructions, and the row may stay undecided or tied after every fix. W8 names those rows, with
  the runs. For each, the owner then picks: keep the finish line open, or accept that row as a
  stated exception. Until they pick, it stays open.
- **The decisions above**, each the owner's to keep or reverse.

## Deferred, and why

- **A knowledge graph over more than program folders.** W6 makes the program folders a graph in
  the Open Knowledge Format, with `index.md`, `log.md` and a read-only resource. Indexing other
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
  line's scope; W11 has DESIGN say what would bring each in.

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
