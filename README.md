---
type: lengua Overview
title: lengua
description: "What lengua 3 is, and where its design, its plan, its plugin and its runner are."
---

# lengua

lengua 3 is a full-stack framework and an AI language. A person writes what they want. The agent
writes an idea file, the program's purpose, its behaviours as tests and what it may touch, and
then the code that does it: Rust for the back end, compiled to one WebAssembly component, and
TypeScript for a Next.js front end, carried and served by that same file. lengua checks both
against the idea, to the same rules, and runs the file under the idea's grants, the same bytes on
every CPU and every operating system.

The design of record is [Lengua3.md](Lengua3.md). The work and its state are in
[todo.md](todo.md). lengua 2, which used hand-written WebAssembly text, is kept at the tag
`lengua2-final`.

lengua is an [Agent Plugin](https://agent-plugins.org/): the skills and the MCP server are in
[plugin/](plugin/), and the server's source is in [crates/lengua-mcp](crates/lengua-mcp). A checked program
runs from a shell with the lean runner, [crates/lengua-run](crates/lengua-run), which loads
only the sealed code its signed `lengua.lock` names; a checked server also ships as a container
image of `lengua-run`, [deploy/Dockerfile](deploy/Dockerfile).

Everything is built and tested on GitHub's free runners for Linux, macOS and Windows, on x86-64
and ARM64.

Licensed under the Apache License 2.0 ([LICENSE](LICENSE)).
