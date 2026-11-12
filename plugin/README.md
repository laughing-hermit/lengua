# lengua

An AI language, as an [Agent Plugins](https://agent-plugins.org) 1.1.0 plugin: two skills and
one MCP server, nothing else. The agent writes a program's idea in plain words, `idea.md`, an
Open Knowledge Format Attested Computation in Simplified Technical English, with the program's
behaviors and their tests inside. It then compiles the idea into WebAssembly text, `main.wat`.
lengua checks that code against an adapted NASA Power of 10 with memory rules like Rust's, runs
every test the idea holds, proves those tests for every input it can, and runs the program
holding only what its idea names. The same `main.wat` runs the same way on every system lengua
is built for.

## What this folder holds

- `plugin.json`, the Agent Plugins manifest, and `mcp.json`, which starts `server/lengua-mcp`
  over standard input and output. `.claude-plugin/plugin.json` and `.mcp.json` are Claude Code's
  copies of the two.
- `skills/write-idea`: writes or updates `idea.md` from the user's request, and ends when `check`
  records the idea with the user's own words. Its references teach Simplified Technical English
  and the test format, and its asset is an idea's template.
- `skills/compile`: compiles `idea.md` into `main.wat`, and ends when `check` says done. Its
  references give the dialect's rules, lengua's host, WebAssembly text, constant time and the
  CPU's caches, and its assets are templates of whole programs, each with its idea. When the
  user asks for code in another language, it also says how a module that a compiler made from
  Rust or C runs as the program, `main.wasm`: under the idea's grants and tests, held to no
  rule and no proof, and marked "trusted, not proven".
- `server/lengua-mcp` (`lengua-mcp.exe` on Windows): the MCP server, with two tools. `check`
  records the idea, builds `main.wat`, holds it to every rule, runs every test and proves the
  tests in the background. `run` runs a program `check` checked.
- `server/lengua-run` (`lengua-run.exe` on Windows): runs a program `check` checked, from a
  shell. It has no compiler in it, and loads what `check` compiled, so it starts sooner and
  holds less memory than `lengua-mcp run`, which does the same. `cargo xtask plugin` builds both
  here from the repository, and a release archive ships them built.
- `LICENSE`: the Apache License 2.0.

## The release archive

lengua's release archive for each system and CPU it is built for, Linux on x86-64 or ARM64,
macOS on ARM64, and Windows on x86-64 or ARM64, is this folder with the server built, and with:

- `solvers/`: the z3 4.16.0 and cvc5 1.3.4 its proofs need;
- `licenses/`: the licences of everything it ships;
- `TARGET`, the system and CPU it is for, and `INSTALL.md`, which says how to install it.

In the archive's folder, `server/lengua-mcp install` checks that this computer is that system
and CPU, and on Linux that its glibc is 2.39 or later, copies the plugin where Claude Code finds
plugins, and ends with `lengua-mcp doctor`, which says whether lengua has what it needs here. A
program needs nothing more. To load the folder for one session instead: `claude --plugin-dir
<this folder>`.

## From a shell

- `lengua-run <program folder> -- <arguments>` runs a program `check` checked on this
  computer, and so does `lengua-mcp run`
  ([run.md](https://github.com/laughing-hermit/lengua/blob/main/docs/run.md)).
- `lengua-mcp check <program folder>` checks a folder from its two files alone, on any computer,
  and says `attested` or `not attested` ([check.md](https://github.com/laughing-hermit/lengua/blob/main/docs/check.md)).
- `lengua-mcp doctor` checks what lengua needs on this computer.
