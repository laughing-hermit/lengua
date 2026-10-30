# lengua

Natural language in, checked machine code out. lengua is an AI language, not a programming
language: you describe a program in plain words, and your AI agent is the compiler. A program is
two files. `spec.md` is the source: what the program does, as behaviors shown by examples, with
the tests that try each one, in plain markdown anyone can read and share. `main.s` is the fastest
x86-64 assembly the agent can compile from it for your computer, and it's disposable: the spec is
what you keep and share. That assembly must pass every
test and an adapted NASA "Power of 10" rule set, and lengua validates each step of the
translation; the one step no machine can check, your request to the spec, comes back to you in
the agent's report. A signed lock keeps the two files in sync, and a program runs only while both
match. See [DESIGN.md](DESIGN.md) for how and why.

lengua is an [Agent Plugin](https://agent-plugins.org): two skills and an MCP server, using only
open standards, so it works in Claude Code, Codex, Cursor and other agent harnesses.

> Status: dialect 3 on Linux x86-64, with memory rules that check every address has an owner and
> stays inside it. It covers whole numbers, exact decimals, floating point in double and single
> precision with fused multiply-add, vectors from SSE2 to AVX-512, hashing and checksums, files
> and folders, environment variables, fork-join threads, waiting, the clock and random bytes
> (which tests supply, so such output is exact), plain TCP clients and servers on the ports a spec
> names, and inputs up to 1 MiB. The kernel holds every run to what its spec allows. lengua
> proves programs right for every input, not just the tested ones, over whole numbers, decimals,
> choices, texts beyond ASCII and text formulas, files and the network, in integer, bit and vector code from SSE2 to AVX-512,
> and over long texts up to a stated length (4,096 bytes of standard input). Programs that read
> with the skill's `fill` are proven however their input arrives, cut into any pieces, and
> servers are tested and proven with clients that keep the connection open, as curl does. Each
> solver step is confirmed by two independent solvers, z3 and cvc5, and most proofs need none:
> the terms settle every step. `lengua.lock` records what's proven. Next:
> - proofs of arithmetic a formula writes at 128 bits and a program at 64, such as a halving;
> - more of the top CPU and OS targets, which LLVM already supports.

## Requirements

- Linux on x86-64
- Rust 1.89 or newer (to build the server)
- LLVM's assembler and linker: `sudo dnf install llvm lld` (Fedora) or `sudo apt install llvm lld`
  (Debian/Ubuntu)
- For proofs, the z3 and cvc5 solvers: `sudo dnf install z3 cvc5` or `sudo apt install z3 cvc5`.
  With only one of them installed, proofs still run, and say that one solver answered alone.

## Build

```sh
scripts/build-plugin.sh
```

This builds the server and installs it at `plugin/server/lengua-mcp`, where both plugin manifests
expect it. Run it again after every change to the Rust code.

## Use it

Load the plugin in your agent harness. In Claude Code:

```sh
claude --plugin-dir /absolute/path/to/lengua/plugin
```

Then describe a program, for example: "Use lengua to write a program that adds two whole numbers
given as arguments and prints the sum." That request is all it takes:

1. The agent writes `spec.md` from your request, and lengua records it with your words in
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

Every run is confined by the kernel: the program can use only what its spec names (standard
input, files, the clock, random bytes, environment variables, the network ports it lists), and
touch files only in the folder it runs in.

Share a program by sharing its spec, the source of truth: anyone's agent compiles it for their own
computer, and lengua checks that copy there. Each copy is compiled afresh, so copies pass the same
checks but can differ wherever the spec leaves room: what must be the same everywhere belongs in
its behaviors and ranges.

lengua relies on nothing from your harness's permission prompts, so it is as safe with every tool
allowed or in an auto mode: what a program may touch is what its spec names, which the kernel enforces
on every run, and lengua runs no build its checks didn't pass. The first `check` creates a
signing key in `~/.local/share/lengua`.

## Examples

Complete programs, each with its spec, tests included, and its assembly:

- [`examples/add`](examples/add) adds two whole numbers;
- [`examples/mean`](examples/mean) averages two prices exactly, in whole cents, rounding half a
  cent away from zero;
- [`examples/hypotenuse`](examples/hypotenuse) computes a square root in double-precision floating
  point and prints it with three decimals;
- [`examples/upcase`](examples/upcase) writes a copy of a text file in capitals, 16 bytes at a
  time with SSE2, proven for every file up to 4096 bytes;
- [`examples/absdiff`](examples/absdiff) prints how far apart two numbers are. Claude Code wrote
  it through lengua, proven for every pair of inputs;
- [`examples/greet`](examples/greet) greets whoever the `NAME` environment variable names. It
  gets only the variables its tests, or your `run`, give it;
- [`examples/archive`](examples/archive) moves a file into an `archive` folder, making the folder
  when it's missing;
- [`examples/dice`](examples/dice) rolls a fair die from the kernel's random bytes. Its tests
  supply the bytes, so every roll is checked exactly, and the proof covers every pair of first
  bytes;
- [`examples/fetch`](examples/fetch) fetches a page from a web server on this computer over
  HTTP/1.0, and [`examples/echo`](examples/echo) is a line-echo server. Their tests bring their
  own server and client, and lengua keeps each to the ports its spec names and to this computer;
  both read with `fill`, so each is proven however the network cuts what it reads;
- [`examples/count`](examples/count) counts lines and words like `wc -lw`. Claude Code wrote it
  through lengua from one plain request in an interactive session; on a 4 GiB file it ran at
  15.8 GB/s, ahead of hand-written AVX-512 C, 12 times faster than plain C and 25 times faster
  than GNU `wc`. Its AVX-512 counting is proven right for every input up to 4096 bytes.

Their `lengua.lock` files aren't committed, because a lock is signed with each machine's own key.
Ask the agent to check one with lengua: one `check` records the spec, checks its tests and the
code, and creates the lock.

## Against C, C++ and Rust

[BENCHMARKS.md](BENCHMARKS.md) measures lengua programs against the same programs in C, C++ and
Rust, built with every optimization for this CPU (`cargo bench --bench against_rust_and_c`
measures again). On this machine, lengua's best version is ahead or level on every workload and
every measure, and uses less than half the memory:

| Workload | Cycles | CPU time | Wall time |
|---|---|---|---|
| `add`, mostly starting and exiting | 106× fewer | 1.8× less | 1.4× less |
| Capitalize a 1 MiB file | 4.4× fewer | 1.2× less | 1.2× less |
| Capitalize 1 MiB of standard input | 4.8× fewer | 1.6× less | 1.3× less |
| Count lines and words of 64 MiB (Claude Code's version) | 13.7× fewer | 1.6× less | 1.3× less |
| CRC-32C of 64 MiB | 2.9× fewer | 1.1× less | 1.1× less |
| Byte histogram of 64 MiB | level | level | level |

The baselines are written the way a performance-minded programmer would: the CRC uses the
`crc32` instruction, the histogram four tables, and the word count a loop compilers vectorize.
lengua wins by using the CPU fully: 64 bytes a step with AVX-512, three `crc32` chains side by
side, no C library and no runtime. The histogram is limited by one memory update per byte in
every language. On 64 MiB through a pipe, the kernel's copying is most of the wall time, so a 3×
lead in the program's own cycles shows as 1.1× on the clock.

## Develop

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo bench --bench against_rust_and_c   # rewrites BENCHMARKS.md
cargo test --release -- --ignored        # the slow proof of examples/add
```

## License

[Apache-2.0](LICENSE)
