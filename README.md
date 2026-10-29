# lengua

Natural language in, checked machine code out. lengua makes your AI agent the compiler: you
describe a program in plain words, and there's no programming language in between. The agent
writes a spec for you to approve, tests from the spec, and then the fastest x86-64 assembly it
can. That assembly must pass every test and an adapted NASA "Power of 10" rule set, and lengua
validates each step of the translation. A signed lock keeps spec, tests and code in sync, and a
program runs only while all three match. See [DESIGN.md](DESIGN.md) for how and why.

lengua is an [Agent Plugin](https://agent-plugins.org): three skills and an MCP server, using only
open standards, so it works in Claude Code, Codex, Cursor and other agent harnesses.

> Status: dialect 2 on Linux x86-64. It covers whole numbers, exact decimals, floating point in
> double and single precision with fused multiply-add, vectors from SSE2 to AVX-512, hashing and
> checksums, files, fork-join threads, the clock and random bytes (which tests supply, so such
> output is exact), plain TCP clients and servers on the ports a spec approves, and inputs up to
> 1 MiB. The
> kernel holds every run to what its spec approves. `prove` proves programs right for every
> input, not just the tested ones, over whole numbers, decimals, choices, short texts and files,
> with each step confirmed by two independent solvers, z3 and cvc5; `lengua.lock` records what's
> proven. Next:
> - proofs of programs on the network;
> - proofs of text formulas, vector code and decimal arithmetic written in another shape;
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
given as arguments and prints the sum."

1. The agent writes `spec.md` and tells you where it is. Read it in your editor.
2. When it says what you want, tell the agent so, for example: "I approve
   /home/you/add/spec.md". That's your one decision, and lengua records your words in
   `lengua.lock`.
3. The rest is automatic: the agent writes the tests and the assembly, lengua checks both, and the
   agent runs the program once and tells you how it went.

Later, ask the agent to run the program with other inputs, or on your files ("run it on
notes.txt in ~/docs"), or to change what it does; a change starts with a new spec for you to
approve. If you don't name a folder, the program goes in a new folder inside the current one.

Every run is confined by the kernel: the program can make only the system calls its spec
approves, and touch files only in the folder it runs in.

Your harness may ask permission the first time each lengua tool runs. Allowing the tools doesn't
skip your approval: only your words approve a spec. The first approval creates a signing key in
`~/.local/share/lengua`.

## Examples

Complete programs, each with its spec, tests and assembly:

- [`examples/add`](examples/add) adds two whole numbers;
- [`examples/mean`](examples/mean) averages two prices exactly, in whole cents, rounding half a
  cent away from zero;
- [`examples/hypotenuse`](examples/hypotenuse) computes a square root in double-precision floating
  point and prints it with three decimals;
- [`examples/upcase`](examples/upcase) writes a copy of a text file in capitals, 16 bytes at a
  time;
- [`examples/absdiff`](examples/absdiff) prints how far apart two numbers are. Claude Code wrote
  it through lengua, and `prove` proves it for every pair of inputs;
- [`examples/dice`](examples/dice) rolls a fair die from the kernel's random bytes. Its tests
  supply the bytes, so every roll is checked exactly, and `prove` covers every pair of first
  bytes;
- [`examples/fetch`](examples/fetch) fetches a page from a web server on this computer over
  HTTP/1.0, and [`examples/echo`](examples/echo) is a line-echo server. Their tests bring their
  own server and client, and the kernel keeps each to the ports its spec names.

Their `lengua.lock` files aren't committed, because a lock is signed with each machine's own key.
Ask the agent to run one with lengua: after you approve its spec, the agent checks the tests and
the code, which creates the lock.

## Against C, C++ and Rust

[BENCHMARKS.md](BENCHMARKS.md) measures lengua programs against the same programs in C, C++ and
Rust, built with every optimization for this CPU (`cargo bench --bench against_rust_and_c`
measures again). On this machine, lengua's best version is ahead or level on every workload and
every measure, and uses less than half the memory:

| Workload | Cycles | CPU time | Wall time |
|---|---|---|---|
| `add`, mostly starting and exiting | 104× fewer | 1.8× less | 1.4× less |
| Capitalize a 1 MiB file | 4.4× fewer | 1.2× less | 1.2× less |
| Capitalize 1 MiB of standard input | 4.8× fewer | 1.6× less | 1.3× less |
| Count lines and words of 64 MiB | 11.9× fewer | 1.8× less | 1.1× less |
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
