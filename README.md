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
random bytes, a server, a client, a client that follows a redirect to a computer it learns
while it runs, a copy of a file the user names, a program that starts two others, a key read
from the terminal, a sort, SHA-256, AES-128 and a comparison that keeps its input secret.

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
there only an earlier lengua install. `--link` (Linux and macOS) also puts `lengua-mcp` and
`lengua-run` in
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

A program can build on parts: programs whose ideas offer some of their functions, each with a
stated contract. lengua ships five (`decimal`, `lines`, `crc32`, `bytes` and `sort64`), and a
program of your own can be one. The agent names the part in the idea (`- Builds on:`), copies
the functions it takes into `main.wat` word for word, and `check` holds each copy to the part
at every check and says what the program takes ([DESIGN.md](DESIGN.md), Parts).

Run it with the `run` tool, which gives it 10 seconds by default (up to 40) and 8 MiB of each
output stream, in a fresh folder or in one you name. A server keeps serving in the background
until `run` stops it, ten minutes pass, or the session ends. From your own shell, scripts and
services, the `done` reply gives the command:

```sh
lengua-run <program folder> -- <arguments>
```

`lengua-run` is a small program beside `lengua-mcp` with no compiler in it, so it starts sooner
and holds less memory; `lengua-mcp run` takes the same words and does the same. It runs only a
folder whose idea and code match the signed `lengua.lock`, with the module `check` tested, in
the form `check` compiled under your key; otherwise it says why and exits with 125. It has no
time limit, works in the current
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
  `- Network:`, `- Files:` and `- Runs:` lines don't allow, and the host checks every call
  against those lines again.
  Every path is resolved beneath the working folder; a path that would leave it is refused. By
  default a program makes no connection itself: lengua makes the ones its idea names before it
  starts, and does TLS for it. An idea may grant more, a line for each grant: connections the
  program opens itself, the addresses of names, UDP datagrams, each to the computers and ports
  the line names. lengua then resolves each name itself, checks every address, and never lets
  such a connection reach this computer, or a private network the line doesn't name
  ([DESIGN.md](DESIGN.md), the host). The same way, a line may grant a file or a folder that
  one of the program's arguments names, anywhere the user says, to read or to write: the
  program gives the argument's place and never a path, and lengua opens nothing in its own
  folders or the system's. A line may name a program to start, with the arguments the line
  bounds: never a shell, with no environment and no file of the program's, for a stated
  time. lengua doesn't confine a program started this way: it is the user's own. And an
  idea that uses the terminal may set its mode, which lengua puts back however the program
  ends. `check` and `run` say what an idea grants before the
  program runs.
- **Its memory stays its own.** WebAssembly traps any access outside the program's own memory,
  which can't grow past what the module declares. The memory rules M1 to M8 hold every address
  they can follow to its owner. What they can't follow is left to the proofs, which keep it
  inside the program's memory but follow no region, so whether it stays inside its own region
  is tested, not proven. The reply's memory line says which is which.
- **A function taken from a part is the part's own.** A function `main.wat` says it takes from a
  part is that part's function word for word, with the constants it reads and the functions it
  calls, or `check` refuses the program. The part is checked on this computer, states what
  each function reaches through the addresses it is lent, and the memory rules hold every call
  to that. lengua.lock records the parts with their hashes, and no run starts once a part changed.
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
  8 bytes, SHA-256 and AES-128 (the proof doesn't model them), a CRC-32 a program computes
  itself past two characters of text (the library's `crc32` among them), a server that waits on
  its connections with `poll_oneoff`, a program that opens its own connections, sends datagrams,
  looks up names, opens a file an argument names, starts a program or uses the terminal's
  functions (the model follows only what lengua gives a program before it starts), and whatever
  runs out of time. A stream is proven for every length by induction over its read loop only
  where every final piece of up to a buffer-full is proven in the proof's time; otherwise it is
  proven up to a length, which its line says.
- **A part's proof isn't the program's.** A part is proven through its own program, for the
  inputs its idea gives. The prover doesn't carry that into a program built on the part: it
  follows each taken function again, inside the program, for the program's own tests. So a
  program's proofs say what holds of it, and a part's say only what was shown of the part.
- **Not what the other end is.** A grant names computers and ports. What a granted computer
  sends back, and who answers for a name, are that computer's and its DNS's: lengua checks the
  addresses a name resolves to and, over TLS, the certificate, and nothing more of them. An
  idea that grants `any computer` lets its program reach any public computer on those ports.
- **The engine is trusted.** Isolation rests on Wasmtime, lengua's host and `cap-std`, with no
  operating system sandbox around them.
- **A secret's timing is checked in part.** The secret rule checks the WebAssembly. Then, on this
  computer's CPU, x86-64 or ARM64, `check` and every run read the machine code Wasmtime made for
  each operator that takes a secret, and refuse a branch, a table, a call or a division there
  (W13), and in the code of the same function that the engine gives to no operator. The rest is
  trusted: the CPU's own timing of the instructions left, the code of functions and operators
  that take no secret, and the engine's and the host's own functions
  ([DESIGN.md](DESIGN.md), Secrets).
- **No security product.** The examples show algorithms, such as AES-128 and SHA-256. They
  aren't built to resist an attacker, and lengua isn't presented as covering security because
  of them.
- **Not every kind of program yet.** No threads and no GUIs. W18 has given the network chosen
  at run time, DNS and UDP, files anywhere the user names, starting named programs, and the
  terminal's mode and size, each as a grant the idea states. A program that uses one of
  those grants is tested, not proven; a started program runs in an empty folder of its own,
  so it can't work on the user's folder yet. W20 plans threads; GUIs aren't planned.

## Against C, C++ and Rust

The gate, `cargo bench --bench against_rust_and_c`, runs 14 workloads on each runner: an empty
program, add and a hypotenuse (mostly starting up); 64 MiB streams for capitals, a word count, a
byte histogram, a CRC-32C, a sort and a word tally; 1 MiB of capitals from a file and from
standard input; a server under 64 clients; 4,096 small files; and a million clock readings.
lengua's program runs through lengua-run, with every check a run makes. It runs against C,
C++ and Rust compiled to WebAssembly (wasi-sdk 34, rustc 1.99.1) and run in the same engine and
host, and against their native builds, made on the runner. The runs alternate, and a one-sided
Mann-Whitney U test decides CPU and wall time at p < 0.01; memory and size are compared as
measured. The gate has a noise margin of 5% (the owner's decision of 2026-11-11): a measure the
test doesn't decide, or one where lengua's value is within 5% of the other's either way, is
level, neither ahead nor behind. Size has no margin. [KNOWN_BEHIND](KNOWN_BEHIND) names the rows
where lengua is behind by more than the margin, and may only shrink: a row behind that the list
doesn't name, or one it names where lengua is level or ahead, fails the gate. A level row
doesn't.

The latest run is 43084459708, the first with lengua-run (W19), at commit 45b507b, on all five
runners; GitHub didn't keep its files for Windows on ARM64, whose report is from run 43099764273
at the same commit. In it every WebAssembly version, lengua's and the others', runs by lengua-run
from its compiled form, where the runs before used lengua-mcp and Wasmtime's cache. The reports,
as the bench wrote them from the runs' results:

| Runner | CPU, as the report names it | Report |
|---|---|---|
| Linux on x86-64 | AMD EPYC 7763 64-Core Processor, 4 threads | [BENCHMARKS-linux-x86_64.md](docs/benchmarks/BENCHMARKS-linux-x86_64.md) |
| Linux on ARM64 | CPU implementer 0x41, part 0xd49, 4 threads | [BENCHMARKS-linux-aarch64.md](docs/benchmarks/BENCHMARKS-linux-aarch64.md) |
| macOS on ARM64 | Apple M1 (Virtual), 3 threads | [BENCHMARKS-macos-aarch64.md](docs/benchmarks/BENCHMARKS-macos-aarch64.md) |
| Windows on x86-64 | INTEL(R) XEON(R) PLATINUM 8573C, 4 threads | [BENCHMARKS-windows-x86_64.md](docs/benchmarks/BENCHMARKS-windows-x86_64.md) |
| Windows on ARM64 | Cobalt 100, 4 threads | [BENCHMARKS-windows-aarch64.md](docs/benchmarks/BENCHMARKS-windows-aarch64.md) |

Against the WebAssembly builds, the rows where lengua was ahead, level and behind, in that
order, of 42 on each runner (14 workloads against three languages):

| Runner | CPU time | Wall time | Peak memory | Size |
|---|---|---|---|---|
| Linux on x86-64 | 33, 7, 2 | 36, 3, 3 | 2, 40, 0 | 40, 0, 2 |
| Linux on ARM64 | 19, 18, 5 | 22, 16, 4 | 2, 38, 2 | 40, 0, 2 |
| macOS on ARM64 | 17, 17, 8 | 20, 14, 8 | 8, 31, 3 | 40, 0, 2 |
| Windows on x86-64 | 14, 27, 1 | 15, 27, 0 | 2, 39, 1 | 40, 0, 2 |
| Windows on ARM64 | 13, 29, 0 | 20, 22, 0 | 2, 39, 1 | 40, 0, 2 |
| All five, of 210 | 96, 98, 16 | 113, 82, 15 | 16, 187, 7 | 200, 0, 10 |

- **Size:** lengua's module is smaller on every row but one workload's, the 64 MiB stream of
  capitals (upcase-stdin), against C and C++.
- **Memory:** lengua was behind by more than the margin on 7 rows, where it was on 115 in the
  run before: a run now holds 3 to 7 MiB for a small program where it held 5 to 12, so the
  0.1 MiB that lengua's own checks add falls within the margin.
- **Time:** lengua was behind by more than the margin on 16 rows of CPU time and 15 of wall
  time, nearly all on workloads that are mostly starting up (the empty program, hypotenuse,
  upcase and shout), against C's and C++'s modules. Those modules now start in about the time
  lengua's does, less the 0.1 ms that reading and checking lengua.lock, idea.md and main.wat
  takes, which is past 5% of a run of 1.3 ms. Windows counts a process's CPU time in 15.6 ms
  ticks, so most CPU rows there are level.

Against the native builds, lengua's median over the best of C's, C++'s and Rust's (above 1,
lengua takes more), from each report's native gaps:

| Runner | CPU time | Wall time | Peak memory | Size, with lengua-run |
|---|---|---|---|---|
| Linux on x86-64 | 0.51× to 3.51× | 0.51× to 3.50× | 1.01× to 7.12× | 425.15× to 429.12× |
| Linux on ARM64 | 0.59× to 3.38× | 0.59× to 3.36× | 1.01× to 10.62× | 75.82× to 75.88× |
| macOS on ARM64 | 0.45× to 3.69× | 0.45× to 3.64× | 0.95× to 2.89× | 142.51× to 287.84× |
| Windows on x86-64 | 0.50× to 6.00× | 0.49× to 3.94× | 0.96× to 1.73× | 48.06× to 59.06× |
| Windows on ARM64 | 0.65× to 3.50× | 0.64× to 2.97× | 0.96× to 1.53× | 40.26× to 49.54× |

In the run before, the largest of these were 3.78× to 7.00× for CPU time, 3.20× to 20.51× for
wall time and 2.49× to 19.33× for peak memory, and the size counted lengua-mcp, 16.9 to
25.4 MiB, where it now counts lengua-run, 4.6 to 6.0 MiB. On Windows the CPU ratios leave out
the shortest workloads, whose native CPU time reads as no ticks. Every native C++ version was
built on all five runners.

The run measured KNOWN_BEHIND as it was, 898 rows, so the gate failed on every runner. What it
found:

- **137 rows the list named that are level or ahead now** (135 level, 2 ahead): 21 on Linux on
  x86-64, 15 on Linux on ARM64, 6 on macOS on ARM64, 50 on Windows on x86-64 and 45 on Windows
  on ARM64. They have left the list, which now names 761 rows; 24 of them are against the
  WebAssembly builds.
- **26 rows behind by more than the margin that the list doesn't name:** 3 on Linux on x86-64,
  9 on Linux on ARM64, 11 on macOS on ARM64 and 3 on Windows on x86-64, 21 of them the time of
  a workload that is mostly starting up against C's or C++'s module.
  [docs/benchmarks](docs/benchmarks/README.md) names each. Of the 6 such rows of the run
  before, 2 are level or ahead now and 4 are among the 26. The list may not grow, so they stay
  off it.
- **No build failed.**

With the list as it is now, the same results pass the gate on Windows on ARM64 and fail it on
the other four runners, on those 26 rows alone; no run has measured the new list yet. What to do
with rows the list can't take back is the owner's ([todo.md](todo.md)).

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
- `examples/`: 26 programs, each a folder of `idea.md` and `main.wat`. Five are the library's
  parts, `decimal`, `lines`, `crc32`, `bytes` and `sort64`, the same files the compile skill
  ships in its `assets/parts`, and `sum` and `checksums` are built on them.
- `tests/`: the suite, the 29 programs of `tests/fixtures`, the host's 109 cases in
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

lengua 0.2.0 is a work in progress. The WebAssembly lengua's milestones W0 to W14 and W17a,
lengua's tooling in Rust, are on main. That includes W13, the check of the engine's machine code
for an idea's secrets, and the fixes of two reviews of 2026-11-10: the secret rule's
([docs/reviews/secret-rule-2026-11-10.md](docs/reviews/secret-rule-2026-11-10.md), five
findings) and the host's ([docs/reviews/host-2026-11-10.md](docs/reviews/host-2026-11-10.md),
eleven).

The finish-line run was made at main's commit 33ed7fe, on all five runners:

- **The suite** passes natively on each (run 42947270993); among its tests, every program of
  `examples/` and `tests/fixtures` goes from its request to done.
- **The slow tests** the suite leaves out pass on each (run 42947285372).
- **The release:** each runner's archive was built, then installed and checked on a clean runner
  of its own system and CPU (run 42947303258).
- **The bench** ran on each, and the gate failed on each (run 42947294489). The run after it,
  the first with the gate's margin, is the one the section above reports (run 42964232679, at
  cd54e79 on the branch `wip/gate-margin`).

In an interactive Claude Code session, on 2026-11-07, the agent followed the Linux on x86-64
archive's two skills and drove its `lengua-mcp` as the MCP client, from a request to done. That
test hasn't been made again with the final archive, and loading the plugin into Claude Code
itself is left for the owner ([todo.md](todo.md), W14's test).

The finish line, seven points in [todo.md](todo.md), isn't reached. The gate has passed on no
runner yet. Since the owner's decision of 2026-11-11 it has a noise margin of 5%, and with it
the latest run's results leave 6 rows behind that KNOWN_BEHIND doesn't name, on four runners,
and 136 rows behind C, C++ and Rust compiled to WebAssembly that it names, most of them peak
memory. Some proofs stay "tested, not proven". No run yet compares the proofs'
verdicts across the five runners. Point 3 is settled: the owner decided on 2026-11-11 that it is
the same bits on every machine, so lengua keeps one set of WebAssembly features everywhere, its
proofs and tests hold on every machine, and Cranelift compiles each module for the machine's own
CPU. The documents now say what the finish-line run showed
(W15). Next:

- **The report to the owner** (W15). The agent's test with the final archive ran on 2026-11-11.
- **W17b, measured:** z3 as WebAssembly was to replace native z3 only if it gave the same verdicts
  on all five runners with the replayed questions taking at most twice as long. It took 1.70 to
  2.28 times as long and lost a proof on three runners (run 42919127514), so native z3 stays;
  [todo.md](todo.md) has the numbers. The bounded try to build cvc5 as WebAssembly is skipped,
  by the owner's decision of 2026-11-11, to revisit when Wasmtime or the solvers change.
- **W16, done:** only main is left of the branches; 28 tips it doesn't hold are kept as `archive/` tags.

After the finish line, four milestones aim to make lengua a replacement for C, C++ and Rust:

- **W18, reach:** every grant a C, C++ or Rust program can have, each declared in the idea and
  shown before the program runs. Built: the network chosen at run time (connections a program
  opens itself, over TLS too, DNS and UDP), files and folders anywhere the user names, starting
  named programs, and the terminal's mode and size.
- **W19, native speed:** step 1 is built: lengua-run, lengua
  without Wasmtime's compiler, runs a checked program from the module `check` compiled and
  sealed with your key. On Linux on x86-64 the empty program went from 2.20 ms and 11.4 MiB to
  1.33 ms and 5.50 MiB ([ROADMAP.md](ROADMAP.md)). Step 2 is an optional native build of
  the proven `main.wat` through wasm2c.
- **W20, parallelism:** WebAssembly's threads, as structured fork-join that the rules show free
  of data races.
- **W21, a library of proven parts** that programs build on with `- Builds on:`. Built: the
  mechanism (a part's contract, the copy `check` holds to the part, the lock's record) and five
  parts, `decimal`, `lines`, `crc32`, `bytes` and `sort64`, with two programs on them. Left:
  the prover proves a taken function again in each program, and hash tables, SHA-256, HTTP, a
  faster sort, JSON, compression and dates have no part. `decimal`, `lines` and `bytes` are
  proven for every input in their ranges; `sort64` for two numbers and `crc32` for 2
  characters of text, each tested beyond.
- **W22, scale; W23, working with other code; W24, tools:** what a programming language has
  that lengua still lacks ([todo.md](todo.md)).

[ROADMAP.md](ROADMAP.md) has each milestone's design and the runs that verify it.

## Licence

lengua is under the [Apache License 2.0](LICENSE). A release archive also ships z3, under the
MIT License, and cvc5, under the BSD 3-Clause License with the parts it links in, among them
LibPoly and GMP under the GNU LGPL 3 and, on Linux, glibc under the GNU LGPL 2.1. The archive's
`licenses/` holds each licence, and lists every Rust crate compiled into `lengua-mcp` with its
own.
