# lengua

lengua is an AI language. You say what a program should do, in your own words. Your AI agent
writes that down as an idea file, `idea.md`: the program's behaviors, each shown by an example,
with the tests that try them. The agent then compiles the idea into WebAssembly text, `main.wat`.
lengua doesn't trust that translation. It holds `main.wat` to its own rules, runs every test,
the generated ones with fresh inputs, proves those for every input where it can, with the z3
and cvc5 solvers, and runs the program in a host of its own that gives it only what the idea
grants. The same `main.wat` gets the same checks on the five systems and CPUs lengua is built
for: Linux on x86-64 and on ARM64, macOS on ARM64, and Windows on x86-64 and on ARM64.

lengua is an [Agent Plugin](https://agent-plugins.org): two skills and one MCP server, nothing
else. The idea is what you keep and share; the code is the agent's, checked instead of trusted.
[WHITEPAPER.md](WHITEPAPER.md) says why, and [DESIGN.md](DESIGN.md) says how.

The native lengua, which compiled ideas to assembly for Linux on x86-64 and Linux on ARM64, is
kept at the tag `native-final`; [docs/history/](docs/history/) holds the documents of its time.

## An example

[`examples/add`](examples/add) adds two whole numbers. Its idea's first behavior is an example,
and the section under it is a generated test, which tries 200 pairs on every check: the
boundary values first, then pairs drawn afresh each time.

```markdown
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |
```

Its `# Constraints` say `- Uses: nothing`, so the program gets its arguments, standard output
and standard error, and its exit, and nothing more. Here is the end of its `main.wat`'s
`_start`, after both numbers are read:

```wat
          (local.set $b (i64.load (global.get $number)))
          (br_if $not_a_number (i64.lt_s (local.get $b) (global.get $MIN_VALUE)))
          (br_if $not_a_number (i64.gt_s (local.get $b) (global.get $MAX_VALUE)))
          ;; Both are in the idea's range, so their sum fits.
          (br_if $write_failed (call $write_i64 (i64.add (local.get $a) (local.get $b))))
          (call $proc_exit (global.get $EXIT_OK))
```

Every exit code is a named constant, every loop states its bound (`;; bound: 20`), and every
assertion calls `$assert_fail` with a number of its own, at least two a function on average:
the rules ask for each.

Once every test passes, `check` waits up to 20 seconds for the proofs, then says `done`. Its
reply's `tests`, `proofs` and `memory`, with the numbers of a run left out, read:

```text
main.wat follows every rule and passed all … test runs
generated B1: proven for every input in range (… input combinations, … paths through the
WebAssembly), …
the rules (M1 to M8) hold every address they can follow to its owner, and WebAssembly holds
every access inside the program's own memory; …
```

Each proof line says what it covers: every input in range, every input up to a length, or
"tested, not proven" with the reason. Proofs that take longer go on in the background after
`done`, for up to five minutes in all, and the reply says so. At `done`, `check` adds its entry
to the idea's `verified` list: `{ by: lengua/0.2.0, at: …, idea: "sha256:…", code: "sha256:…" }`.
On any other computer, `lengua-mcp check <folder>` checks such a folder again from its two
files, and ends with `attested` or `not attested` ([docs/check.md](docs/check.md)).

The other examples are in [examples/index.md](examples/index.md): files, the environment,
random bytes, a server, a client, a sort, SHA-256, AES-128 and a comparison that keeps its
input secret.

## Install

Each release archive is lengua for one system and CPU: `linux-x86_64`, `linux-aarch64`,
`macos-aarch64`, `windows-x86_64` or `windows-aarch64`. There is none for macOS on x86-64. An
archive holds the plugin with its server built, z3 4.16.0 and cvc5 1.3.4, the licences, and
INSTALL.md. CI's release mode builds each one on its own runner with `cargo xtask release`. The
archives aren't published yet, and they aren't signed.

Unpack the archive, and in its folder run its own server's install:

```sh
server/lengua-mcp install            # Linux and macOS
.\server\lengua-mcp.exe install      # Windows, in PowerShell
```

The install:

1. checks that this computer is the archive's system and CPU, the hardware's own CPU even under
   macOS's Rosetta or Windows's x64 emulation, and on Linux that its glibc is 2.39 or later
   (Ubuntu 24.04, Debian 13, Fedora 40 and later);
2. copies the plugin into a marketplace folder of lengua's own:
   `~/.local/share/lengua/plugins/lengua` on Linux (under `$XDG_DATA_HOME` when it is set),
   `~/Library/Application Support/lengua/plugins/lengua` on macOS, and
   `%LOCALAPPDATA%\lengua\plugins\lengua` on Windows;
3. adds it to Claude Code when the `claude` command is installed, or prints the two commands
   that do;
4. runs `lengua-mcp doctor`, which checks the server, the engine, the CPU, both solvers, the
   data folder, `PLUGIN_DATA` and the clock. The install exits with doctor's code: 0 when
   nothing is wrong.

`--check` only checks. `--prefix <folder>` installs into another marketplace folder, and replaces
there only an earlier lengua install. `--link` (Linux and macOS) also puts `lengua-mcp` in
`~/.local/bin`. A browser marks what it downloads, and macOS won't start the unsigned server so
marked: if you trust the archive, clear the mark first with `xattr -dr com.apple.quarantine
<folder>`. The install says how on macOS and on Windows, and clears nothing itself.

To try the plugin without installing it, load the unpacked folder for one session:
`claude --plugin-dir <folder>`. Other agent harnesses are meant to load it from its Agent Plugins
manifests, `plugin.json` and `mcp.json`, which `tests/manifests.rs` holds to the 1.1.0 schemas; no
run has loaded it in one yet.

From the source, `cargo xtask plugin` builds the server into `plugin/server/`, with the Rust that
`rust-toolchain.toml` pins (1.99.1, which rustup installs); then `claude --plugin-dir
/absolute/path/to/lengua/plugin`. The prover finds z3 and cvc5 in a `solvers/` folder beside the
server's folder, else on `PATH`. Without them, programs are still checked and tested, and a proof
finishes only where it needs no solver: where the tests tried every value, or the terms settle
every step.

## Use

Describe a program to your agent: "Use lengua to write a program that ...". Then:

1. The agent writes `idea.md` with the `write-idea` skill and calls `check` with your words.
   `check` holds the idea to its format, its generated tests to its examples and to the ranges
   its purpose states, and its prose to Simplified Technical English, then records it in
   `lengua.lock` with your words.
2. The agent writes `main.wat` with the `compile` skill and calls `check` after each change.
   `check` builds the module, holds it to every rule, runs every test, and starts the proofs.
   It replies within about 40 seconds and says what to do next. The tests and proofs go on in
   the background, and the next call picks them up.
3. At `done`, the agent tells you where the program is, what its idea promises, that it passed
   every test, what is proven, and any change made to the idea after your words.

Run it with the `run` tool, which gives it 10 seconds by default (up to 40) and 8 MiB of each
output stream, in a fresh folder or in one you name. A server keeps serving in the background
until `run` stops it, ten minutes pass, or the session ends. From your own shell, scripts and
services, the `done` reply gives the command:

```sh
lengua-mcp run <program folder> -- <arguments>
```

It runs only a folder whose idea and code match the signed `lengua.lock`, with the module `check`
tested; otherwise it says why and exits with 125. It has no time limit, works in the current
folder (or the one `--in` names), gives the program only the environment variables `--env` names,
and exits with the program's own code, or 134 when the program trapped
([docs/run.md](docs/run.md)).

Share a program by its idea, which anyone's agent compiles and checks, or by its folder, which
`lengua-mcp check` attests on any computer with no lock and no key. A lock is signed with a key
in your own data folder, so it verifies only there.

## What lengua guarantees, and what it doesn't

Within lengua, for a program at `done`:

- **Nothing that breaks a rule runs.** A rule broken stops `check` before any test. `run` runs
  only the module `check` tested, by the SHA-256 the signed lock records, and refuses a build a
  proof found wrong.
- **It reaches only what its idea grants.** The checker refuses any import the idea's `- Uses:`
  and `- Network:` lines don't allow, and the host checks every call against those lines again.
  Every path is resolved beneath the working folder; a path that would leave it is refused. A
  program makes no connection itself: lengua makes the ones its idea names before it starts, and
  does TLS for it.
- **Its memory stays its own.** WebAssembly traps any access outside the program's own memory,
  which can't grow past what the module declares. The memory rules M1 to M8 hold every address
  they can follow to its owner. What they can't follow is left to the proofs, which keep it
  inside the program's memory but follow no region, so whether it stays inside its own region
  is tested, not proven. The reply's memory line says which is which.
- **The same bits on every computer.** NaNs are made canonical, there are no threads, and relaxed
  SIMD, whose results differ between computers, is off.
- **Every proof says what it covers.** With both solvers installed, as the archive ships them, a
  solver's "no counterexample" counts only when z3 and cvc5 both give it. Questions about floats
  that the bits don't settle go to cvc5 alone, and the line says so. Inputs a proof finds are run
  for real before lengua says the code is wrong.
- **An idea's secrets don't steer its code.** For an idea with a `- Secret:` line, `main.wat` may
  not branch on, index with, divide by or loop over those inputs. A review on 2026-11-10 found
  five ways around the rule; each has a test and is closed, and what it left open is in
  [todo.md](todo.md).
- **The clock is rounded down to a microsecond**, so it can't carry finer timing.

What it doesn't promise:

- **Nothing outside lengua.** `lengua-mcp wasm` writes the module out, a standard WASI preview 1
  module, which another runtime can run when the program reaches only standard streams and
  files, with none of these checks behind it.
- **Not every proof finishes.** Some tests stay "tested, not proven": sorts and word tallies over
  8 bytes, SHA-256 and AES-128 (the proof doesn't model them), a server that waits on its
  connections with `poll_oneoff`, and whatever runs out of time.
- **The engine is trusted.** Isolation rests on Wasmtime, lengua's host and `cap-std`, with no
  operating system sandbox around them. The secret rule checks the WebAssembly; the machine code
  Wasmtime compiles from it is trusted to keep its timing. Checking that code (W13) was stopped by
  the safety filter ([todo.md](todo.md)).
- **No security product.** The examples show algorithms, such as AES-128 and SHA-256. They
  aren't built to resist an attacker, and lengua isn't presented as covering security because
  of them.
- **Not every kind of program yet.** No threads, no GUIs, no connections a program opens itself,
  no UDP, and no starting other programs. W18 plans the network chosen at run time, UDP and
  starting programs, and W20 threads; GUIs aren't planned.

## Against C, C++ and Rust

The gate, `cargo bench --bench against_rust_and_c`, runs 14 workloads on each runner: an empty
program, add and a hypotenuse (mostly starting up); 64 MiB streams for capitals, a word count, a
byte histogram, a CRC-32C, a sort and a word tally; 1 MiB of capitals from a file and from
standard input; a server under 64 clients; 4,096 small files; and a million clock readings.
lengua's program runs through `lengua-mcp run`, with every check it makes. It runs against C,
C++ and Rust compiled to WebAssembly (wasi-sdk 34, rustc 1.99.1) and run in the same engine and
host, and against their native builds, made on the runner. The runs alternate, and a one-sided
Mann-Whitney U test decides CPU and wall time at p < 0.01; memory and size are compared as
measured. [KNOWN_BEHIND](KNOWN_BEHIND) must name every row where lengua isn't ahead, and may only
shrink: a row where lengua is behind or undecided that the list doesn't name, or one it names
where lengua is ahead, fails the gate.

The latest run is 42756074550, at commit 076bcc1, on all five runners. Its reports, as the bench
wrote them from the run's results:

| Runner | CPU, as the report names it | Report |
|---|---|---|
| Linux on x86-64 | AMD EPYC 9V74 80-Core Processor, 4 threads | [BENCHMARKS-linux-x86_64.md](docs/benchmarks/BENCHMARKS-linux-x86_64.md) |
| Linux on ARM64 | CPU implementer 0x41, part 0xd49, 4 threads | [BENCHMARKS-linux-aarch64.md](docs/benchmarks/BENCHMARKS-linux-aarch64.md) |
| macOS on ARM64 | Apple M1 (Virtual), 3 threads | [BENCHMARKS-macos-aarch64.md](docs/benchmarks/BENCHMARKS-macos-aarch64.md) |
| Windows on x86-64 | AMD EPYC 9V74 80-Core Processor, 4 threads | [BENCHMARKS-windows-x86_64.md](docs/benchmarks/BENCHMARKS-windows-x86_64.md) |
| Windows on ARM64 | Cobalt 100, 4 threads | [BENCHMARKS-windows-aarch64.md](docs/benchmarks/BENCHMARKS-windows-aarch64.md) |

Against the WebAssembly builds, the rows where lengua was ahead, of 42 on each runner (14
workloads against three languages):

| Runner | CPU time | Wall time | Peak memory | Size |
|---|---|---|---|---|
| Linux on x86-64 | 35 | 38 | 3 | 40 |
| Linux on ARM64 | 39 | 37 | 19 | 40 |
| macOS on ARM64 | 26 | 30 | 14 | 40 |
| Windows on x86-64 | 21 | 37 | 2 | 40 |
| Windows on ARM64 | 21 | 35 | 2 | 40 |

- **Size:** lengua's module is smaller on every row but one workload's, the 64 MiB stream of
  capitals (upcase-stdin), against C and C++.
- **Time:** where lengua was behind, it was on programs that mostly start up or run briefly (the
  empty program, the hypotenuse, and the 1 MiB capitals from a file and from standard input), by
  10% or less where the report gives a ratio; and on macOS on ARM64 on the word count, which took
  1.14 to 1.21 times the C and Rust modules' time. Windows counts a process's CPU time in 15.6 ms
  ticks, so on both Windows runners a short workload's median is a whole number of ticks, most
  often the same one for every module, and most of those CPU rows stay undecided. Three came out
  behind: shout against C's module on Windows on x86-64 and upcase against C's module on Windows
  on ARM64, both at 1.00×, and shout against Rust's module on Windows on x86-64, whose median was
  no ticks to lengua's one.
- **Memory:** lengua's median peak was 0.90 to 1.14 times the other module's. On Linux on x86-64,
  lengua's run adds 620 KiB to 1.02 MiB to its own module's peak (the report's last section).

Against the native builds, lengua's median over the best of C's, C++'s and Rust's (above 1,
lengua takes more), from each report's native gaps:

| Runner | CPU time | Wall time | Peak memory | Size, with lengua-mcp |
|---|---|---|---|---|
| Linux on x86-64 | 0.50× to 5.33× | 0.50× to 4.11× | 1.13× to 14.60× | 1633.33× to 1659.66× |
| Linux on ARM64 | 0.60× to 6.05× | 0.63× to 16.31× | 1.09× to 19.62× | 269.08× to 269.14× |
| macOS on ARM64 | 0.47× to 3.75× | 0.47× to 3.72× | 0.98× to 4.51× | 516.00× to 1042.45× |
| Windows on x86-64 | 0.53× to 3.00× | 0.52× to 3.93× | 1.01× to 3.03× | 199.58× to 245.38× |
| Windows on ARM64 | 0.65× to 4.00× | 0.65× to 3.25× | 1.01× to 2.49× | 153.23× to 188.66× |

The word tally is the one workload where lengua took less time than every native build, on all
five runners. Size against a native build counts the lengua-mcp that runs the module (16.7 to
25.0 MiB). On Windows the CPU ratios leave out the five shortest workloads, whose native CPU time
reads as no ticks.

The gate didn't pass on any runner in this run. Most of its problems were rows the faster
programs had put lengua ahead on, which KNOWN_BEHIND still named; they have left the list.
Five rows the list doesn't name came out behind or undecided: the server's CPU time against the
three modules on Linux on x86-64, the word count's memory against C++'s module there, and the
shout's CPU time against Rust's module on Windows on ARM64. [todo.md](todo.md) holds them for the
owner to decide. On macOS on ARM64 the system's C++ compiler couldn't build the native C++ add
and readings, and on both Windows runners also the hypotenuse, sort and tally, so their rows
against native C++ are undecided.

## The standards it follows

- **[Agent Plugins](https://agent-plugins.org) 1.1.0:** `plugin/plugin.json` and
  `plugin/mcp.json`, held to the 1.1.0 schemas (a working draft, vendored in `tests/schemas`),
  with Claude Code's mirrors of both.
- **[Agent Skills](https://agentskills.io):** `write-idea` and `compile`, each a `SKILL.md`
  with only the standard's fields.
- **The [Model Context Protocol](https://modelcontextprotocol.io)**, through the
  [Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), rmcp 3.8.0: two tools, `check` and `run`, with no resources and no prompts, tested in the
  2025-11-25 handshake and the 2026-07-28 `server/discover`.
- **The [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
  v0.2:** `idea.md` is an `Attested Computation`, and a folder of programs is a bundle whose
  `index.md` and `log.md` `check` keeps.
- **Andrej Karpathy's [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f):**
  `idea.md` is an idea file, the source each person's agent builds out, and `index.md` and
  `log.md` are the wiki's index and log.
- **ASD-STE100 Simplified Technical English** for an idea's prose: `check` holds it to 25 words
  a sentence, six sentences a paragraph, no -ing forms but technical names, and the step
  vocabulary's phrasings. The rest of STE is the `write-idea` skill's to teach.
- **WebAssembly and WASI preview 1:** the module is a standard WASI preview 1 module, and
  Wasmtime 50.0.1 runs it with a fixed set of WebAssembly 2.0's features.
- **NASA's "Power of 10" rules**, adapted for WebAssembly, with memory rules like Rust's:
  `plugin/skills/compile/references/power-of-10-webassembly.md`.

## The repository

- `src/`: `lengua-mcp`, the MCP server and its command line: the idea's parser and its tests'
  format, the checker (`src/checker/wasm`), the prover (`src/prove`), the runtime and host
  (`src/wasm`), the knowledge graph (`src/graph`), the lock, `doctor`, `install`, `measure` and
  the gate.
- `crates/lengua-os/`: the one crate allowed unsafe code, each call in a safe function.
- `plugin/`: the plugin as it ships: its manifests, the two skills with their references and
  templates, its [README](plugin/README.md) and licence.
- `examples/`: 15 programs, each a folder of `idea.md` and `main.wat`.
- `tests/`: the suite, the 28 programs of `tests/fixtures`, the host's 109 cases in
  `tests/host-cases`, and the vendored schemas.
- `benches/`: the gate, and the C, C++ and Rust versions of its workloads.
- `xtask/`: lengua's tooling, in Rust (`cargo xtask help` lists the tasks), with the runners in
  `targets.json` and the pinned tools in `tools.tsv`.
- `docs/`: [run.md](docs/run.md) and [check.md](docs/check.md), the two pages every idea names
  as its executor and attester; [benchmarks/](docs/benchmarks/), the latest gate run's reports;
  and [history/](docs/history/), the native lengua's.
- `.github/workflows/ci.yml`: CI on the five runners, as glue around `cargo xtask`.
- [KNOWN_BEHIND](KNOWN_BEHIND), [DESIGN.md](DESIGN.md), [WHITEPAPER.md](WHITEPAPER.md),
  [ROADMAP.md](ROADMAP.md) and [todo.md](todo.md).

## Status and what's next

lengua 0.2.0 is a work in progress. The WebAssembly lengua's milestones W0 to W12c, W14 and
W17a, lengua's tooling in Rust, are on main; W13 was stopped by the safety filter, and is the
owner's. At bce6cd5, which merged W17a with W12c, the suite passes natively on all five runners
(run 42765232534); among its tests, every program of `examples/` and `tests/fixtures` goes from
its request to done. There, each runner's archive was built, then installed and checked on a
clean runner of its own system and CPU (run 42765242004). In an interactive Claude Code session,
the agent followed the Linux on x86-64 archive's two skills and drove its `lengua-mcp` as the MCP
client, from a request to done; loading the plugin into Claude Code itself is left for the owner
([todo.md](todo.md), W14's test).

The finish line, seven points in [todo.md](todo.md), isn't reached. The gate doesn't pass yet on
any runner. Some proofs stay "tested, not proven". No run yet compares the proofs' verdicts across
the five runners. And lengua keeps one set of WebAssembly features on every machine, so that every
machine computes the same bits, where point 3 asks for each machine's fastest: that choice is the
owner's. This pass over the documents is W15's, before the finish-line run and the report. Next:

- **W17b, measured:** z3 as WebAssembly was to replace native z3 only if it gave the same verdicts
  on all five runners with the replayed questions taking at most twice as long. It took 1.70 to
  2.28 times as long and lost a proof on three runners (run 42919127514), so native z3 stays;
  [todo.md](todo.md) has the numbers.
- **W16:** the repository tidied, and only main left of its branches.

After the finish line, four milestones aim to make lengua a replacement for C, C++ and Rust:

- **W18, reach:** every grant a C, C++ or Rust program can have, each declared in the idea and
  shown before the program runs: files anywhere the user names, the network chosen at run time,
  UDP and DNS, starting named programs, and the terminal.
- **W19, native speed:** a runner without Wasmtime's compiler, and an optional native build of
  the proven `main.wat` through wasm2c.
- **W20, parallelism:** WebAssembly's threads, as structured fork-join that the rules show free
  of data races.
- **W21, a library of proven parts** that programs build on with `- Builds on:`.

[ROADMAP.md](ROADMAP.md) has each milestone's design and the runs that verify it.

## Licence

lengua is under the [Apache License 2.0](LICENSE). A release archive also ships z3, under the
MIT License, and cvc5, under the BSD 3-Clause License with the parts it links in, among them
LibPoly and GMP under the GNU LGPL 3 and, on Linux, glibc under the GNU LGPL 2.1. The archive's
`licenses/` holds each licence, and lists every Rust crate compiled into `lengua-mcp` with its
own.
