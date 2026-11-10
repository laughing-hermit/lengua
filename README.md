# lengua

> **Moving to WebAssembly (2026-11-06).** The owner decided to move lengua's code from assembly to
> WebAssembly. A program becomes a folder of `idea.md`, the idea file, and `main.wat`, the agent's
> WebAssembly, which `lengua-mcp` checks, proves and runs the same way on Linux, Windows and macOS,
> on x86-64 and ARM64. The work lands in milestones W0 to W15 ([todo.md](todo.md),
> [ROADMAP.md](ROADMAP.md)). Until it does, this page describes the native lengua below, which the
> tag `native-final` keeps.

Natural language in, checked machine code out. lengua is an AI language, not a programming
language: you describe a program in plain words, and your AI agent is the compiler. A program is
two files. `idea.md` is the source: what the program does, as behaviors shown by examples, with
the tests that try each one, in plain markdown anyone can read and share: an idea file, in
Andrej Karpathy's words, that anyone's agent builds out for their own computer. `main.s` is the fastest
assembly the agent can compile from it for your computer, x86-64 or ARM64, and it's disposable: the spec is
what you keep and share. That assembly must pass every
test and an adapted NASA "Power of 10" rule set, and lengua validates each step of the
translation; the one step no machine can check, your request to the spec, comes back to you in
the agent's report. A signed lock keeps the two files in sync, and a program runs only while both
match. See [DESIGN.md](DESIGN.md) for how and why.

lengua is an [Agent Plugin](https://agent-plugins.org): two skills and an MCP server, using only
open standards, so it works in Claude Code, Codex, Cursor and other agent harnesses.

> Status: dialect 4 on Linux x86-64 and Linux ARM64, with memory rules that check every address has an owner and
> stays inside it. Windows and Mac will run the WebAssembly lengua natively (above; the WSL 2 and
> Lima routes below are superseded). It covers whole numbers, exact decimals, floating point in double and single
> precision with fused multiply-add, vectors from SSE2 to AVX-512 and from NEON to SVE,
> hashing and checksums, files
> and folders, environment variables, fork-join threads, waiting, the clock and random bytes
> (which tests supply, so such output is exact), the terminal, TCP clients of the places a spec
> names, or of the computer an argument names as with curl, whose connections lengua makes and
> hands over, with TLS for HTTPS done by lengua so the
> program holds no crypto, servers on the port it names, for other
> computers and many clients at once when it says so, and `switch`es
> as checked jump tables. The kernel holds every run to the system calls, files and network its
> spec names. lengua proves programs right for every input, not just
> the tested ones, over whole numbers, decimals, choices, texts beyond ASCII and text formulas,
> checksums, files and the network, servers that poll in every order their clients' steps can come
> in, threads, and floats computed in the formula's order, in integer, bit, vector and float code
> from SSE2 to AVX-512 and from NEON to SVE, and over texts as long as the spec's lengths and the proof's time reach,
> and further with `prove_further`; a filter or a count that streams its input a buffer-full at a
> time is proven for every length, by induction over its read loop. Programs that read with
> lengua's `fill` are proven however their input arrives, cut into any pieces. Each solver step is
> confirmed by two independent solvers, z3 and cvc5, and most proofs need none: the terms settle
> every step. `lengua.lock` records what's proven. Programs build on programs: a spec links to the
> specs of the parts it uses, so a folder of programs is a graph of two-file nodes, as an OKF
> bundle is, and `check` holds each program to its parts' current code. In real Claude Code
> sessions, twelve requests on each Linux target from an add to an HTTPS fetch, every program came
> from one request ([SESSIONS.md](SESSIONS.md)). Linux on ARM64 is the same product, every example
> with its AArch64 twin; [WHITEPAPER.md](WHITEPAPER.md) says what lengua is, and `site/` is a local
> site for sharing specs. What's left for people, and the work the safety filter stopped or that
> was skipped, is in [todo.md](todo.md).

## Requirements

- Linux on x86-64 or ARM64 (AArch64), with glibc 2.39 or newer: Ubuntu 24.04, the one CI tests and
  the project promises, or Debian 13 or Fedora 40 and later. On Windows that Linux is WSL 2, and on
  a Mac a Lima VM ("Windows and Mac" below).
- A kernel with Landlock on, which every run needs: Linux 5.13 or newer, and 6.12 or newer for a
  program that uses the network. lengua won't run a program without it. `cat
  /sys/kernel/security/lsm` should list `landlock`.
- Rust 1.89 or newer (to build the server)
- LLVM's assembler and linker: `sudo dnf install llvm lld` (Fedora) or `sudo apt install llvm lld`
  (Debian/Ubuntu)
- For proofs, the z3 and cvc5 solvers: `sudo dnf install z3 cvc5` or `sudo apt install z3 cvc5`.
  With only one of them installed, proofs still run, and say that one solver answered alone.

## Windows and Mac

lengua runs the same way on every system: on a real Linux kernel, next to your agent, on Ubuntu
24.04, the same Ubuntu that CI runs on GitHub's runners for both CPUs. A Linux laptop already has
that; Windows and a Mac get it once, in a few minutes:

| system | the Linux | once |
|---|---|---|
| Linux | itself (Ubuntu 24.04 is the one promised; Fedora 40+ and Debian 13+ work when the requirements above hold) | the plugin and the tools in "Requirements" |
| Windows | WSL 2 with Ubuntu 24.04, built into Windows | `wsl --install -d Ubuntu-24.04`, then your agent and lengua inside it |
| Mac | a Lima VM with Ubuntu 24.04, free | `brew install lima`, `limactl start template:ubuntu-24.04`, then your agent and lengua inside it |

**Windows: WSL 2.**

1. In an administrator PowerShell, run `wsl --install -d Ubuntu-24.04`, restart, and create a Linux
   user when Ubuntu asks.
2. Open Ubuntu (from Windows Terminal or the Start menu) and install your agent and the lengua
   plugin there, and the tools in "Requirements" above. Claude Code runs in WSL, and the plugin is
   an ordinary Linux plugin. A Claude Code installed on the Windows side keeps its own login and
   settings, so use the one inside Ubuntu.
3. Keep your projects in Ubuntu's home folder, not under `/mnt/c`, which is slow and ignores case.

Microsoft's WSL 2 kernel (6.18) has Landlock on by default, above lengua's 6.12 floor for programs
that use the network.

**Mac: a Lima VM.**

1. Install Lima and start Ubuntu 24.04: `brew install lima`, then `limactl start
   template:ubuntu-24.04`.
2. Open a shell in it (`limactl shell ubuntu-24.04`) and install your agent (Claude Code runs on
   Linux on ARM64), the lengua plugin and the tools in "Requirements".
3. Keep your projects inside the VM: Lima mounts your Mac's home folder read-only by default.

Which kernel Lima's Ubuntu 24.04 boots is not checked yet. On Ubuntu 24.04's stock 6.8 kernel,
lengua runs every program but refuses those that use the network, below the 6.12 floor, until
round 3 settles the floor ([todo.md](todo.md), "Round 3").

**These routes are for `main.s`.** They bring the Linux that a native `main.s` program needs. A
`main.wat` program, which every Attested Computation compiles to, runs on macOS and Windows
themselves: lengua's release archive for each of the five systems and CPUs it is built for holds
the plugin, its server and both solvers, and CI installs and checks each archive on a clean
runner of its own system and CPU ("Build" below). The rest of this README, written for the
native product, waits for its rewrite in W15 ([ROADMAP.md](ROADMAP.md)).

**Containers, later.** A container image of the same package may come later, for dev containers,
Codespaces or an agent that must stay on the Windows or Mac side: run by a working Docker (a
`docker` command and an engine from any provider: Docker Desktop, Rancher Desktop, Colima) with
Podman supported, wherever the provider's kernel has Landlock, which Docker Desktop's Mac VM
reportedly lacks. lengua already uses nothing a default container lacks (no io_uring; hardware
counters optional), so the image can come without changing the product.

Other Linuxes work too when they meet the requirements above, though the project doesn't test
them: a VM in UTM or Parallels, or your own image from `ubuntu:24.04` (or `debian:trixie-slim`,
which is Debian 13; `debian:12`, `ubuntu:22.04` and Alpine are too old). In any of them, `cat
/sys/kernel/security/lsm` must list `landlock`.

## Build

```sh
cargo xtask plugin
```

This builds the server and installs it at `plugin/server/lengua-mcp`, where both plugin manifests
expect it. Run it again after every change to the Rust code. `cargo xtask` runs lengua's tooling,
the task crate in `xtask/`, with the Rust that `rust-toolchain.toml` pins (rustup installs it).

`cargo xtask release` builds the release archive of the computer it runs on, one of the five lengua
is built for: `dist/lengua-<version>-<target>.tar.gz` for `linux-x86_64`, `linux-aarch64` and
`macos-aarch64`, or a `.zip` for `windows-x86_64` and `windows-aarch64`. It holds the plugin with
its server built, the z3 and cvc5 that CI pins for that system and CPU, and the licences of all
of it. Its own server installs it: `server/lengua-mcp install` refuses another system or CPU,
copies the plugin into a marketplace folder of lengua's own, adds it to Claude Code when `claude`
is installed, and ends with `lengua-mcp doctor`: one line for each thing lengua needs, and what to
do about anything missing. CI's `release` run builds each archive on its own runner, then installs
it on a clean runner of the same system and CPU and checks it there through a plain MCP client
(`tests/release_check.rs`). The archives are unsigned, and nothing is published.

## Use it

Load the plugin in your agent harness. In Claude Code:

```sh
claude --plugin-dir /absolute/path/to/lengua/plugin
```

Then describe a program, for example: "Use lengua to write a program that adds two whole numbers
given as arguments and prints the sum." That request is all it takes:

1. The agent writes `idea.md` from your request, and lengua records it with your words in
   `lengua.lock`. The agent asks you something only when your request leaves open what only you
   can decide.
2. It compiles the spec into assembly, calling lengua's one tool, `check`, after each file:
   lengua checks the spec's tests against the spec and the code against its rules, runs every
   test, and proves the program for every input it can, in the background. When a test fails,
   the agent decides from the spec whether the test or the code is wrong and fixes it, until
   everything passes. No call takes more than about 40 seconds, in any harness.
3. Its report says where the program is, what the spec promises (behaviors, limits, and the files
   and ports it may touch), what's proven, what its first example printed, and every change made
   to the spec after your request, with lengua's own marks on any test change that tests less.

If you'd rather read the spec before the agent builds, say "show me the spec first", and it
waits for your go-ahead. Later, ask the agent to run the program with other inputs, or on your
files ("run it on notes.txt in ~/docs"), or to change what it does. If you don't name a folder,
the program goes in a new folder inside the current one.

Run it yourself, too, from your terminal, scripts, pipelines and services, with the command the
agent's report gives you, then the program's arguments:

```sh
~/path/to/lengua/plugin/server/lengua-mcp run ./add -- 2 3
```

It streams your standard input and output, has no time limit, works on the files in the folder
you start it in (or `--in <folder>`), and exits with the program's own code. A server keeps
serving on its spec's port until you stop it. lengua runs only a program whose spec and code it
checked; otherwise it says why and exits with 125.

Every run is confined by the kernel: the program can use only what its spec names (standard
input, files, the clock, random bytes, environment variables), touch files only in the folder it
runs in, and hold no network of its own: lengua makes each connection its spec names before it
starts and hands it over, and binds a server's port for it.

Share a program by sharing its spec, the source of truth: anyone's agent compiles it for their own
computer, and lengua checks that copy there. Each copy is compiled afresh, so copies pass the same
checks but can differ wherever the spec leaves room: what must be the same everywhere belongs in
its behaviors and ranges.

lengua relies on nothing from your harness's permission prompts, so it is as safe with every tool
allowed or in an auto mode: what a program may touch is what its spec names, which the kernel enforces
on every run (the sandbox's known limits are in DESIGN.md's threat model), and lengua runs no
build its checks didn't pass. The first `check` creates a
signing key in `~/.local/share/lengua`.

## Examples

Complete programs, each with its spec, tests included, and its assembly:

- [`examples/add`](examples/add) adds two whole numbers;
- [`examples/mean`](examples/mean) averages two prices exactly, in whole cents, rounding half a
  cent away from zero;
- [`examples/hypotenuse`](examples/hypotenuse) computes a square root in double-precision floating
  point and prints it with three decimals;
- [`examples/upcase`](examples/upcase) writes a copy of a text file in capitals, 16 bytes at a
  time with SSE2, proven for every file as long as the proof's time reaches, and further with
  each `check` that asks to prove on;
- [`examples/absdiff`](examples/absdiff) prints how far apart two numbers are. Claude Code wrote
  it through lengua, proven for every pair of inputs;
- [`examples/sort`](examples/sort) sorts lines as `LC_ALL=C sort` does, with `-u` to drop
  duplicates, comparing 8 bytes at a time. Claude Code wrote it through lengua from one plain
  request; its errors are proven, and its sorting for every input up to 6 bytes, as far as its
  comparisons' paths go, and tested beyond;
- [`examples/sha256`](examples/sha256) prints the SHA-256 digest of its input in plain integer
  code that runs on every computer of its target, on both Linux CPUs from one spec, tested
  against the standard's vectors and at every block boundary, not proven. The forms that use the
  CPU's SHA instructions are the fast tiers `fast/sha256-ni` and `fast/sha256-sha2` in the compile
  skill's assets, from the very same spec;
- [`examples/aes`](examples/aes) encrypts hex data with a hex key, AES-128 block by block, in
  plain integer code and a table, on both Linux CPUs from one spec, tested against the standard's
  vectors, not proven; the CPU's AES instructions are the fast tiers `fast/aes-ni` and
  `fast/aes-aes`. It isn't a security product;
- [`examples/greet`](examples/greet) greets whoever the `NAME` environment variable names. It
  gets only the variables its tests, or your `run`, give it;
- [`examples/archive`](examples/archive) moves a file into an `archive` folder, making the folder
  when it's missing;
- [`examples/dice`](examples/dice) rolls a fair die from the computer's random bytes. Its tests
  supply the bytes, so every roll is checked exactly, and the proof covers every pair of first
  bytes;
- [`examples/fetch`](examples/fetch) fetches a page from the web server on this computer's port
  8080 over HTTP/1.0, through the connection lengua makes for it, and
  [`examples/echo`](examples/echo) is a line-echo server. Their tests bring their own server and
  client; both read with `fill`, so each is proven however the network cuts what it reads;
- [`examples/count`](examples/count) counts lines and words like `wc -lw`, 16 bytes at a time with
  SSE2, so it runs on every x86-64 computer. Claude Code wrote it through lengua from one plain
  request in an interactive session; that version took 128 bytes a step with AVX-512 and, on a
  4 GiB file, ran at 15.8 GB/s, ahead of hand-written AVX-512 C, 12 times faster than plain C and
  25 times faster than GNU `wc` (it is kept as
  [a benchmark program](benches/programs/count/avx512.s)). The SSE2 counting is proven right,
  with no solver question, for every input up to 3,190 bytes in four minutes of a GitHub runner,
  and further each time a `check` asks to prove on; one pipe write holds 4096.

Their `lengua.lock` files aren't committed, because a lock is signed with each machine's own key.
Ask the agent to check one with lengua: one `check` records the spec, checks its tests and the
code, and creates the lock.

## Against C, C++ and Rust

Since the pivot to WebAssembly, `cargo bench --bench against_rust_and_c` is the gate of
[todo.md](todo.md)'s point 4: on each of the five runners, lengua's WebAssembly program for each
of 14 workloads runs against C, C++ and Rust compiled to WebAssembly and run the same way, and
against their native builds, in alternating runs decided by a one-sided Mann-Whitney U test.
Each runner writes its own BENCHMARKS-<target>.md from its run's results, and
[KNOWN_BEHIND](KNOWN_BEHIND) names the rows where lengua isn't ahead yet, a list that may only
shrink. No runner has run the gate yet. What follows is the native product's, before the pivot.

[BENCHMARKS.md](BENCHMARKS.md) measures lengua programs against the same programs in C, C++ and
Rust, built with every optimization for this CPU (`cargo bench --bench against_rust_and_c`
measures again). On the owner's computer, an AMD Ryzen measured on 2026-11-02 before everything moved
to GitHub's runners, lengua's best version is ahead or level on every workload in CPU time, wall
time, peak memory and file size; that is Linux on x86-64 only, and
[BENCHMARKS-aarch64.md](BENCHMARKS-aarch64.md) is GitHub's ARM64 Linux runner. It has memory safety proven rather than checked at
run time, and uses less than half the memory. Where it isn't ahead is described below the table.
The goal is one program that is ahead on every measure at once, on every target. The table below
takes lengua's best version for each measure, which can be a different program for each; that, the
rows that read level, and the items below are the work left to reach it (round 5 of
[todo.md](todo.md)).

| Workload | Cycles | CPU time | Wall time | Peak memory |
|---|---|---|---|---|
| `add`, mostly starting and exiting | 125× fewer | 3.3× less | 2.4× less | 2.6× less |
| `hypotenuse`, a square root in doubles | 98× fewer | 1.9× less | 1.4× less | 2.9× less |
| Capitalize a 1 MiB file | 5.1× fewer | 1.3× less | 1.3× less | 2.8× less |
| Capitalize 1 MiB of standard input | 5.1× fewer | 1.7× less | 1.3× less | 2.8× less |
| Count lines and words of 64 MiB (Claude Code's version, AVX-512 tier) | 13.8× fewer | 1.6× less | 1.3× less | 3.3× less |
| CRC-32C of 64 MiB | 2.9× fewer | level | level | 3.1× less |
| Byte histogram of 64 MiB | level | level | level | 2.5× less |

The baselines are written the way a performance-minded programmer would: the CRC uses the
`crc32` instruction, the histogram four tables, and the word count a loop compilers vectorize.
lengua wins by using the CPU fully: 64 bytes a step with AVX-512 where the CPU has it (16 with
SSE2 on any x86-64 computer, the baseline form), three `crc32` chains side by side, no C library
and no runtime. The histogram is limited by one memory update per byte in
every language. On 64 MiB through a pipe, the kernel's copying is most of the wall time, so a 3×
lead in the program's own cycles shows as level on the clock. Measured 2026-11-02, with where the
cycles go (stalls, L1 misses, mispredicted branches) for each large workload; BENCHMARKS-aarch64.md
measures the same programs on GitHub's ARM64 runner. GitHub's x86-64 runners are a mixed pool, about
half without AVX-512: on six draws of them (2026-11-04, workflow runs 41893737162, 41893742012,
41893747152, 41895807100, 41895812441 and 41895817808), including two CPUs without AVX-512, the
baseline SSE2 word count took from level to 1.6 times less CPU time, and from level to 1.5 times
less wall time, than the best of C, C++ and Rust.

Where lengua isn't ahead:
- **Instructions.** It runs more instructions than the best other on the histogram (0.7×) and the
  CRC-32C (0.5×), and the CRC-32C also has more L1 misses and mispredicted branches. Vector and
  multi-chain code does more per instruction.
- **The histogram.** Its "level" is about 2% more CPU time and wall time than Rust's.
- **The shout without AVX-512.** On a CPU without AVX-512, the best shout is the SSE2 form on 16
  threads, which takes 814 µs of CPU time against plain Rust's 603 µs.
- **The CRC-32C on x86-64.** The table's "level" is the owner's computer. On six draws of GitHub's x86-64
  runners (2026-11-04) the three-chain form took 8 to 29 percent more wall time than the best
  other, with CPU time from 0.96× to 2.09× less.
- **The CRC-32C on ARM64.** On GitHub's ARM64 runner, it takes more wall time than C++ (0.7×).

The roadmap in [todo.md](todo.md) closes these gaps and gates speed on both targets.

## Develop

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo bench --bench against_rust_and_c   # the gate: writes BENCHMARKS-<target>.md
cargo test --release -- --ignored        # the slow proof of examples/add
```

## License

[Apache-2.0](LICENSE). lengua's server links only permissively licensed crates (MIT, Apache-2.0,
BSD, ISC, Zlib and Unicode's, with Mozilla's root certificates under CDLA-Permissive-2.0; r-efi's
LGPL is one of three licenses it offers, the others MIT and Apache-2.0). The tools it runs are
separate programs, also permissive: LLVM's assembler and linker (Apache-2.0 with the LLVM
exception), z3 (MIT) and cvc5 (BSD-3-Clause; its `-gpl` builds aren't used). Only tests here use
QEMU (GPL-2.0), as a separate program, and GCC builds only the benchmark's C and C++. The programs
lengua builds use no library at all. The site's Next.js brings an optional image library, sharp,
whose libvips is LGPL-3.0, which the site doesn't use.
