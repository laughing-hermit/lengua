# lengua

Spec-driven programming in checked assembly. You and an AI agent write a plain-language spec; the
agent writes tests from it, then x86-64 assembly that must pass every test and an adapted NASA
"Power of 10" rule set. A signed lock keeps spec, tests and code in sync, and a program only runs
when all three match. See [DESIGN.md](DESIGN.md) for how and why.

lengua is an [Agent Plugin](https://agent-plugins.org): three skills and an MCP server, using only
open standards, so it works in Claude Code, Codex, Cursor and other agent harnesses.

> Status: MVP in progress (milestone 5 of 6: the whole workflow works, from spec to `run`; the
> end-to-end example and test come next). Linux x86-64 only.

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

Then ask for a program, for example: "Use lengua to start a program in /home/you/projects/add
that adds two numbers." The agent writes `spec.md` with you (the `write-spec` skill), shows it to
you, and asks you to approve it in a dialog.

## Develop

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

[Apache-2.0](LICENSE)
