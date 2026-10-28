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
> checksums, and inputs up to 1 MiB. Next:
> - fork-join threads;
> - files in tests;
> - proofs with an SMT solver;
> - more of the top CPU and OS targets, which LLVM already supports.

## Requirements

- Linux on x86-64
- Rust 1.89 or newer (to build the server)
- LLVM's assembler and linker: `sudo dnf install llvm lld` (Fedora) or `sudo apt install llvm lld`
  (Debian/Ubuntu)

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

Later, ask the agent to run the program with other inputs, or to change what it does; a change
starts with a new spec for you to approve. If you don't name a folder, the program goes in a new
folder inside the current one.

Your harness may ask permission the first time each lengua tool runs. Allowing the tools doesn't
skip your approval: only your words approve a spec. The first approval creates a signing key in
`~/.local/share/lengua`.

## Examples

Complete programs, each with its spec, tests and assembly:

- [`examples/add`](examples/add) adds two whole numbers;
- [`examples/mean`](examples/mean) averages two prices exactly, in whole cents, rounding half a
  cent away from zero;
- [`examples/hypotenuse`](examples/hypotenuse) computes a square root in double-precision floating
  point and prints it with three decimals.

Their `lengua.lock` files aren't committed, because a lock is signed with each machine's own key.
Ask the agent to run one with lengua: after you approve its spec, the agent checks the tests and
the code, which creates the lock.

## Develop

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

[Apache-2.0](LICENSE)
