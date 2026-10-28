# lengua

Spec-driven programming in checked assembly. You and an AI agent write a plain-language spec; the
agent writes tests from it, then x86-64 assembly that must pass every test and an adapted NASA
"Power of 10" rule set. A signed lock keeps spec, tests and code in sync, and a program only runs
when all three match. See [DESIGN.md](DESIGN.md) for how and why.

lengua is an [Agent Plugin](https://agent-plugins.org): three skills and an MCP server, using only
open standards, so it works in Claude Code, Codex, Cursor and other agent harnesses.

> Status: MVP, Linux x86-64 only. Next: vector instructions (dialect 2), proofs with an SMT
> solver, and more platforms.

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

## Use it in Claude Code

```sh
claude --plugin-dir /absolute/path/to/lengua/plugin
```

Then ask for a program, for example: "Use lengua to write a program in /home/you/projects/add
that adds two whole numbers given as arguments and prints the sum." The agent:

1. writes `spec.md` with you (the `write-spec` skill) and asks you to approve it in a dialog:
   "Approve /home/you/projects/add/spec.md?";
2. writes `tests.toml` from the spec and checks it (`write-tests`, `check_tests`);
3. writes `main.s` and checks it against the rules and every test (`write-assembly`,
   `check_code`), which reports the instructions, cycles and CPU time each test took;
4. runs it (`run`), which works only while spec, tests and code all match `lengua.lock`.

The first approval creates a signing key in `~/.local/share/lengua`.

## Example

[`examples/add`](examples/add) is a complete program: its spec, tests and assembly. Its
`lengua.lock` isn't committed, because a lock is signed with each machine's own key; approve the
spec on yours and run `check_tests` and `check_code` to create it.

## Develop

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

[Apache-2.0](LICENSE)
