# lengua

lengua 3 is an AI language. A person writes what they want. The agent writes an idea file, the
program's purpose, its behaviours as tests and what it may touch, and then the Rust that does
it. lengua checks the Rust against the idea, compiles it to one WebAssembly component, and runs
it under the idea's grants, the same bytes on every CPU and every operating system.

The design of record is [Lengua3.md](Lengua3.md). The work and its state are in
[todo.md](todo.md). lengua 2, which used hand-written WebAssembly text, is kept at the tag
`lengua2-final`.

lengua is an [Agent Plugin](https://agent-plugins.org/): the skills and the MCP server are in
[plugin/](plugin/), and the server's source is in [crates/lengua-mcp](crates/lengua-mcp).

Everything is built and tested on GitHub's free runners for Linux, macOS and Windows, on x86-64
and ARM64.

Licensed under the Apache License 2.0 ([LICENSE](LICENSE)).
