# lengua's agenda: what's left for people

**What lengua is.** An AI language: an LLM is the compiler, and lengua validates its translations
instead of trusting them. A person's request becomes a `spec.md`, Behavior-Driven and in the Open
Knowledge Format, naming no CPU or OS; the agent compiles it to `main.s` for the machine; lengua
checks the code against the rules, tests it, proves it, and runs it confined. The product is an
Agent Plugin, skills and one Rust MCP server only, so it runs in any agent harness.

**Where lengua stands (2026-11-05).** Two of the six targets are done: Linux on x86-64 and Linux on
ARM64, one product held by one suite, and the mutation test passes on both (run 42084426990).
The other four, macOS and Windows each on x86-64 and on
ARM64, build with clippy clean and pass every test that runs no program, on their own runners, and
run no program yet (the table under "Where each stands"). The record of the finished work is in
the git history, DESIGN.md, README.md, SESSIONS.md and the BENCHMARKS files.

**How the work is done.**
- Everything is built, tested, proven and measured on GitHub's free runners, which cost nothing
  since the repository is public. Nothing runs on the owner's computer, which only holds the
  files.
- Each round works on a `wip/<round>` branch, merges to main when its runs are green, and records
  their ids here.
- An item leaves this file when it's done, so what's here is what's left.
- [ROADMAP.md](ROADMAP.md) holds what each round follows: the decisions it rests on, the design,
  its steps, and the runs that verify it.

## The finish line: "perfected" (set 2026-11-05)

lengua is finished when all seven of these are true. Each is shown by a green run on GitHub's
free runners, whose id is written in the table below, and by nothing else.

1. **It is an AI language, delivered as an Agent Plugin and nothing else.**
   - The plugin is two skills (`write-spec`, `compile`) and one Rust MCP server (`check`, `run`).
     No hooks, no subagents, nothing that needs one agent harness's features: any harness that
     loads skills and MCP servers runs it.
   - A program is a folder of two files. `spec.md` is the source of truth: Behavior-Driven
     (Given/When/Then, its tests inside), an Open Knowledge Format page in the manner of an LLM
     Wiki, naming no CPU or OS. `main.s` is the LLM's translation of it for one machine, which
     lengua validates (rules, tests, proofs) instead of trusting.
   - Program folders are the nodes of a knowledge graph and a spec's `Builds on` links are its
     edges, with an `index.md` listing a folder of nodes; `check` holds the graph to the code.
     Indexing other files in the graph is not designed yet ("Deferred" below).
2. **Six targets, one product.** Linux, macOS and Windows on x86-64 and ARM64 each pass the one
   conformance suite natively, on their own runner. The suite covers:
   - the rules and memory rules, and the sandbox's limits;
   - the edge's conformance table;
   - every example, fixture and template, through the server;
   - the network and terminal flows, and the server's session flow;
   - the prover's tests and the mutation test.

   `tests/parity_inventory.rs` requires a native `cargo test` job on all six runners, and refuses
   any test file made for one target. Every shipped program has one text per CPU, the same
   main.s on Linux, macOS and Windows, from one byte-identical spec, with a baseline form (no
   optional CPU extension) and its named fast tiers. Across the targets:
   - no target finds a counterexample another doesn't;
   - each proof line has the same verdict on the three targets of its CPU. Where a line's time
     budget cuts it short, the verdict is the same kind: proven, or bounded to a length.
3. **Tailored to each CPU and OS for speed.** main.s is written for the CPU: the widest vectors it
   has (AVX-512, AVX2, NEON, SVE) as named fast tiers, each beside a baseline form that runs on
   every machine of that CPU. The edge, which main.s calls for everything that reaches the OS, is
   written for the OS: raw system calls on Linux, libSystem's thinnest wrappers on macOS,
   kernel32's on Windows. The spec, the checks and the behavior are the same on every target; only
   the instructions differ.
4. **Faster and leaner than C, C++ and Rust, everywhere.** On every workload and every target, one
   lengua program, the one lengua ships for that machine, takes less CPU time and less wall time,
   and uses less peak memory and a smaller executable, than the best C, C++ or Rust does on each of
   those measures. A gate on each target's runner fails on any measure where that lead isn't
   shown. (The owner, 2026-11-05: "I want faster and more memory efficient evrywhere". A person
   runs one program, so one program must win all four at once: never a threaded build for time and
   a single-thread one for memory.)
   - Workloads:
     - *Throughput* (64 MiB streams, files, sort, a word tally).
     - *Startup-bound programs* (an empty program, add, hypotenuse).
     - *Servers* (64 clients, and 4,096): wall time and the server's CPU time.
     - Many files, and the clock.
     - Peak memory and size count on every one.
   - A lead in time counts only when the runs show it: they alternate between the languages, and
     lengua's median must be lower with 99% confidence ([ROADMAP.md](ROADMAP.md) §7 says how). A
     difference within the noise fails.
   - Memory and size are compared as measured, and lengua's must be smaller.
   - The others are shown at their best on each measure, which can be a different program for
     each (Rust with threads for time, without for memory). lengua's side is one program: the
     fastest tier the machine's CPU supports, which the compile skill picks. If that tier loses a
     measure, the row is behind until it doesn't; choosing another tier for that measure doesn't
     count.
   - Nothing is excused at the finish line. Round 5's list of what's still behind may only shrink,
     and it must end empty.
   - Instructions and cycles are reported, not gated. Programs are measured as people run them,
     on their own.
   - `run`'s own cost (the sandbox's start, the broker's round trips) is reported per OS. On
     Linux it is no worse than the cost of the supervisor it replaced.
5. **A replacement for systems, command-line and server programs**, with lengua's safety rules
   kept (memory sized from the spec, explicit stacks for recursion, checked tables for dispatch).
   DESIGN and the white paper say plainly what's out of that scope or excluded by design, and
   "Deferred" below says what would bring each in.
6. **Truthful documents.** README, DESIGN, BENCHMARKS and the white paper say what the runs show,
   per target, and nothing more.
7. **Shippable.** A release archive for each of the six hosts builds, installs and passes its
   check in CI, driven through a plain MCP client. The archives are unsigned, and the owner
   publishes them.

**Where each stands.** Updated as rounds merge, with the run that shows it. First by target, from
run 42085247387 (2026-11-05, commit 43c63ea; libtest counts a test that notes why it did nothing
as passed):

| target | runner | suite | programs run | against C, C++ and Rust | archive |
|---|---|---|---|---|---|
| Linux x86-64 | ubuntu-24.04 | 535 passed | yes | BENCHMARKS.md (the owner's computer, 2026-11-02) | built |
| Linux ARM64 | ubuntu-24.04-arm | 531 passed | yes | BENCHMARKS-aarch64.md (the runner) | built |
| macOS x86-64 | macos-26-intel | 527 passed, 122 of them did nothing: no program runs yet | round 3 | round 5 | round 6 |
| macOS ARM64 | macos-26 | 525 passed, 120 of them did nothing | round 3 | round 5 | round 6 |
| Windows x86-64 | windows-2025 | 522 passed, 120 of them did nothing | round 4 | round 5 | round 6 |
| Windows ARM64 | windows-11-arm | 520 passed, 118 of them did nothing | round 4 | round 5 | round 6 |

Then by finish-line point:

| | now | what's left |
|---|---|---|
| 1. AI language, Agent Plugin | On Linux x86-64 and Linux ARM64: both skills, `check` and `run`, the spec format, the `Builds on` graph. On the other four the server builds and the tests that run no program pass, but no program runs. | The release check through a plain MCP client on every target (round 6). Knowledge-graph indexing past program folders: not designed. |
| 2. Six targets | Linux x86-64 and Linux ARM64 pass the whole suite natively. The other four pass every test that runs no program (the table above). | The edge and broker (round 2); programs running on macOS (round 3) and Windows (round 4). |
| 3. Tailored | CPU tiers for x86-64 and ARM64, each with a baseline form beside it. For the OS, Linux programs call the kernel directly; no edge exists yet for any OS. | One edge per OS (rounds 2, 3 and 4). |
| 4. Faster, leaner | Measured on Linux x86-64 and Linux ARM64 (the table above): most rows ahead, some level, none gated. The other four are not measured. | The gate on all six runners (round 5). |
| 5. A replacement | Scope written in DESIGN. | Prover reach (round 6). |
| 6. Truthful documents | README and DESIGN state the stricter bar and name the targets each claim covers. | A last pass (round 6). |
| 7. Shippable | Archives for Linux x86-64 and Linux ARM64. | The other four hosts (round 6). |

## Roadmap (2026-11-05)

0. **Documents: done 2026-11-05.** This file, README's benchmark claim, DESIGN's performance bar,
   and ROADMAP.md.
1. **Six hosts, one workflow: done 2026-11-05** (run 42085247387, commit 43c63ea).
   - lengua-mcp builds and runs `cargo test --workspace` natively on all six runners, from one
     `ci.yml` (whose suites take 35 to 62 minutes, so it isn't sharded).
   - The OS seams are backends keyed by target (`src/os/`: Linux's, and one for the systems that
     can't run a program yet), and the tables are keyed by CPU (`PerCpu`, `PerTarget`; `PerOs`
     comes with round 3).
   - The spikes' first answers (what the toolchains make, names, pipes) are in ROADMAP.md's
     round 1; handed-over descriptors and handles are asked again in rounds 3 and 4. What a
     sandbox refuses is the owner's to try.
   - The Linux soundness items are fixed, each with a test.
2. **The edge and the broker, on Linux, for both CPUs.**
   - Programs reach the OS only through lengua's linked edge: 21 operations defined once
     (`src/edge/mod.rs`) and one calling convention per CPU.
   - assert_fail, fill and parallel_for move into lengua's library instead of being copied into
     every main.s.
   - A broker (`src/broker.rs`) replaces the ptrace supervisor for names, and for the tests'
     clock, random bytes, sleeps and waits.
   - The benchmarks are level with the pre-edge build.
3. **macOS, on x86-64 and ARM64:** libSystem through the GOT, Mach-O, Seatbelt, tested on the
   newest macOS images.
4. **Windows, on x86-64 and ARM64:** kernel32 through the IAT, PE, an LPAC AppContainer and a
   job.
5. **Performance:**
   - the bench on six runners, with a gate that passes only where lengua is ahead on every
     workload and measure;
   - sort, a word tally, servers, many files and the clock as workloads;
   - the known gaps below.
6. **The rest:** prover parity across CPUs, the small items, the documents, and release
   archives for six hosts.

[ROADMAP.md](ROADMAP.md) lists the decisions the rounds rest on:
- the owner's of 2026-11-04 (a shared edge per OS; unsafe code only in one small binding crate;
  the session suites theirs);
- fifteen taken on 2026-11-05 under the owner's rule, "use whichever is best to maintain speed
  and efficiency per target", and approved with the plan. Each is the owner's to reverse.

## What's left, by round

### Round 2

- **The QEMU job's seven failing cases** (run 42085247387: 528 passed, 7 failed). In
  `tests/programs.rs`: `a_program_gets_only_the_environment_its_test_or_run_gives`,
  `a_program_lists_its_folder_and_deletes_inside_it`,
  `a_program_makes_folders_and_moves_files_inside_its_own`, `small_ranges_run_every_value`,
  `the_parallel_template_runs_on_several_cores` and
  `the_vector_template_passes_its_tests_and_is_proven`; in `tests/template.rs`:
  `the_template_passes_its_tests_and_is_stamped`. They expect what a native run reports, and an
  emulated report differs (it says "under qemu-aarch64", and has no counters or timings). Make
  each cell say what an emulated run says, then take `continue-on-error` off the job in `ci.yml`
  so that the gate requires it.
- **The goldens compare nothing on a runner.** `checker::golden` and `prove::golden` compare a
  run with a recording made at an earlier commit, and a fresh runner has none, so they record
  and note that they compared nothing: the four "passed" that way on both Linux targets (run
  42084426990). Before the edge changes what the checker reads, a job records them at the
  merge-base and at the head, both on the runner, and diffs the two. The prover's two take about
  30 minutes each. Filter `explains_a_mutant` out: it is a helper that needs
  `LENGUA_GOLDEN_MUTANT`.
- `upcase` retries writes 4096 times on x86-64 and 64 times on ARM64. The ARM64 main template's
  assertions skip number 3.
- The stamp lengua writes into `main.s` and `lengua.lock` (`needs SSE4.2, …`) lists SSE3 to
  SSE4.2 and POPCNT as extensions, because the checker's own floor is SSE2 (`Facts.features`).
  The edge rewrites the header to name the CPU and leave the baseline out.
- The static system call summary is missing `ppoll`'s timeout and signal-mask pointers. This
  goes away with the edge's `os_poll`.
- The model-against-CPU harnesses differ between the CPUs in feature gating and host. They move
  onto the edge, so they run on every runner with one gating.

### Round 3

- **macOS's free memory.** The memory guard in `src/prove/watch.rs` reads Linux's `/proc`, so
  one test skips on macOS; it needs `vm_stat` or `sysctl`.
- **A write to a shut stream** fails with EPIPE on Linux's sockets, and the macOS test of a TLS
  stream cut off skips until the connect path is ported.
- **`PerOs` and `Profile::os()`**, and the shared Unix half of `src/os/` (descriptor placement,
  rlimits, `execve`, `wait4`), come when macOS shows what it needs.
- What the spikes left open on macOS: a socketpair end at descriptor 4, a descriptor passed by
  SCM_RIGHTS, and a pseudo-terminal (ROADMAP.md, round 1). They need a program that runs.

### Round 4

- **Windows's free memory.** The same guard needs `GlobalMemoryStatusEx`; one test skips.
- **The stand-in solver** (`src/prove/smt.rs`) is a shell script, which Windows lacks, so two
  tests skip there. It becomes the test binary in a stand-in mode.
- What the spikes left open on Windows: the handed-over handles, `lpReserved2`, a deep frame
  chain, an empty environment with `WSAStartup`, cycle time against ticks, and a large `.bss`'s
  commit (ROADMAP.md, round 1). They need a program that runs.

### Round 5

- **The bench takes one program.** Its table (`ratio` in `benches/against_rust_and_c.rs`) now takes
  lengua's best value for each measure, which can be a different version for each. It takes the
  four measures of the one program the compile skill picks for the machine, and reports the other
  tiers beside it, not instead of it.
- **Where the fastest tier isn't the leanest.** A threaded or wide-vector tier that wins time but
  costs memory (stacks, per-thread slots, tables) is a row to fix, not to excuse: the 16-thread
  shout against Rust's single-thread memory is the first to measure.
- **`crc32c`'s wall time.**
  - On x86-64, the three-chain form, now `fast/crc32c-pclmul`, took 0.96× to 2.09× less CPU time
    than the best other on six runner draws, but 8 to 29 percent more wall time on all six.
  - The old AVX-512 copy shows the same gap, so the copy isn't the cause; the cause is not known.
  - On ARM64 its wall time is 0.7× the best other's (BENCHMARKS-aarch64.md).
  - Its buffer is 1 MiB on x86-64 against 64 KiB on ARM64. The hypothesis to test, A/B in one
    run: fill waits for a full buffer, so process each read as it comes.
- **`move_front`** could copy 16 bytes a step with `movdqu`. That halves the iterations; the
  scalar copy is a few percent of the CPU time.
- **The histogram** on x86-64 takes about 2% more CPU time and wall time than Rust's
  (BENCHMARKS.md). Try more tables or unrolling, A/B.
- **The shout.** On a CPU without AVX-512, lengua's best shout is the SSE2 form on 16 threads.
  Against plain Rust it takes 814 µs of CPU time to 603 µs (0.74×), and 1100 µs of wall time to
  989 µs (BENCHMARKS.md). A one-thread SSE2 shout would match ARM64's `shout/neon-one.s`.
- **ARM64's benchmarks.**
  - `count/sve.s` isn't measured.
  - There is no threaded shout with wider vectors, where x86-64 has `shout/avx512.s`.
  - The workloads leave out programs x86-64 measures, and they say AArch64 has no threads.

### Round 6

- **Prover reach on x86-64.** x86-64's prover can't follow dozens of vector instructions that the
  ARM64 prover models: `palignr`, `pblendw`, `punpck*`, `pack*`, saturating and widening
  arithmetic, string compares, `pextr*`/`pinsr*`, `pmovzx*`, `gf2p8*` and more
  (`src/checker/x86_64.rs` against `src/prove/exec/vector.rs`). So the same spec ends "tested,
  not proven" for different reasons on the two CPUs.
- **Prover reach on ARM64.** `plan_a64` doesn't follow pointers through `csel`, `and`-immediate or
  shifted `add`, where x86-64's `plan` follows `cmov`, `and` to a boundary and scaled `lea`. So
  those accesses are held to the weaker "object the value points into" rule.
- **A signed compare.** `pcmpgtb` of the bytes left against the lanes' places comes out as a
  signed comparison, but `Pool::holds_where`/`reading` follow only unsigned ones. A prover that
  read signed compares of byte lanes would let more programs mask the natural way.
- **CI coverage.** A baseline variant of the model-against-CPU vector test that saves `xmm`/`ymm`
  only, so its SSE2, AVX2, AES, SHA and PCLMUL probes meet real silicon on every draw.
- **A zero-register branch after another branch.** The "assertion always fails" check reads a
  jump that follows any branch as conditional, because the loop-exhaustion and chain-default
  idioms 167 shipped programs use rely on it. So a `cbz xzr` right after another branch passes
  that check, though the dead-code check catches it. No shipped program has one.
- **The AArch64 dialect** has no add-with-carry (`adc`, `sbc`).
- **Small items.**
  - AArch64 refusal texts say "no floating point on vectors", and keep x86-64's wording in a few
    layout messages.
  - A hexadecimal number past `i64::MAX` is "not a number" to the source reader both CPUs share:
    `primary` in `src/checker/source.rs` reads hex with `i64::from_str_radix`. `#-16` for the
    same mask is read.
  - An emulated session always gets AArch64's CPU features.
  - The one-folder check's 60 s proof budget can't finish the 64 KiB proof of `fast/wc` (231 s
    on a runner), so its `check` says "tested, not proven" until asked to prove on.
- **Documents.** DESIGN lists the x86-64 instructions the prover models, and its CPU harness's
  probe counts, but has no AArch64 list. `test-format.md` explains the float functions by x86-64
  instructions alone.
- **Recorded as by design.** A register of the wrong width is refused at different stages: by
  lengua's own AArch64 reader before the assembler is asked, but by the assembler on x86-64. It's
  the same kind of refusal in other words, since lengua decodes a new CPU's dialect itself.

## The owner's

- **Session suites** in real interactive Claude Code, which cost Claude usage: macOS and Windows,
  and ARM64's run natively. SESSIONS.md ran ARM64's under QEMU without the kernel's filter, and
  its thread and terminal rows prove less than x86-64's.
- **AVX-512 as a hard gate.** An "attempts" job would retry until a draw has AVX-512, about 94%
  likely to hit. Also: whether to read the licence of Intel's SDE.
- **A `gate` job as a required check**, with branch protection.
- **Publishing** the release archives, the plugin listing where agent plugins are found, and the
  site (`site/`). The repository has been public since 2026-11-02, a work in progress under the
  Apache License 2.0.
- **Signing.** Apple's notarization and Windows code signing both cost money, so the archives
  are unsigned.
- **Launcher services for compression and a GUI.** Each would be one definition that every OS
  must provide outside the program, as lengua does TLS; a GUI is a product of its own. Not
  designed yet.
- **macOS on x86-64 once GitHub's Intel runners end.**
  - In September 2025, GitHub said x86-64 macOS support ends after the macOS 15 image retires in
    fall 2027.
  - In February 2026 it added `macos-26-intel`.
  - macOS 26 is Apple's last release for Intel Macs.
  - Rosetta 2 on the ARM64 runner is the option to weigh.
- **A row the gate can't decide.** The finish line is "ahead everywhere", with nothing excused.
  Some costs every language pays alike: macOS's dyld and libSystem start, the copy through a
  pipe, an open, libSystem's clock, and executable sizes that round to a page. Where one of those
  dominates a row, lengua's lead is only its own few instructions, and the row may stay undecided
  or tied after every fix. Round 5 will name those rows, with the runs. For each, the owner then
  picks: keep the finish line open, or accept that row as a stated exception. Until they pick, it
  stays open.

## Deferred, and why

- **A knowledge graph over more than program folders.** The owner's idea: lengua indexes files
  and anything else in a knowledge graph, each program folder a node. Program folders and their
  `Builds on` links already are one, and `check` holds it to the code. Indexing other files, or
  answering questions over the graph, needs a design first (new tools or MCP resources, which the
  plugin's two tools don't offer), and the owner decides what it should do. Not designed.
- **A direct path for existing names under `run`:** openat2 under Landlock, and on macOS too if
  its spike can read a name's stored bytes. Worth it only if round 5's many-files line under
  `run` shows the broker's round trip matters.
- **Connections made on demand.** These are for a program that opens a connection for each client
  it serves, as a proxy does, or several to one place.
  - The way to do it is an `os_connect` through the broker, which is cheap once the broker
    exists. It needs spec syntax, though, so it comes after the finish line.
  - Today a program gets the connections its spec names, made before it starts.
- **SVE on Windows ARM64:** it needs a vector-length probe through the edge, and Windows's SVE
  support is unverified.
- **macOS's sandbox without Seatbelt:** the App Sandbox, with entitlements on the ad-hoc-signed
  program. Needed only if Apple removes Seatbelt or changes it past what the profile can keep;
  the gate on the newest macOS image would show that.
- **Testing another CPU's build** under Rosetta 2 and Windows's x64 emulation, as QEMU does on
  Linux.
- **Proofs past the finish line.** Research:
  - sorting at every length, and word tallies past a few bytes, which need quantified facts over
    memory;
  - byte loops at 64 KiB, whose counts keep a copy of every part at every byte;
  - a routine for the last partial vector at a place the length chooses;
  - raw `read`s of any size;
  - long chains of mixing rounds, where `Pool::extract` pushes extractions through shared sums
    and the pool grows fast.
- **A pointer read from memory.** The memory rules can't follow it to an owner (a table of
  pointers, `argv`).
  - So a program that uses one is "tested, not proven" for memory, never proven, and a wrong one
    is killed by the sandbox.
  - Following a loaded pointer back to a table whose entries are all owned addresses would
    change how M1 and M2 follow owners: a design change, with the checker golden re-recorded.
- **UDP, any URL, processes, GUIs, libraries for other languages to call:** outside the finish
  line's scope. Round 6 has DESIGN say what would bring each in.

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
  left for the owner or another engineer: ROADMAP.md's bypass cells, and the spikes' checks of
  what the sandbox refuses. The rest of round 1 goes on.
