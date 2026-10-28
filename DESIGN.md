# lengua design

This file records every design decision, so any session or agent harness can pick up the project.
Where the code and this file disagree, fix one of them.

## Thesis

Programming languages and compilers were made as human middlemen to machine code. With AI, the
natural-language **spec is the source code**, the AI translates it, and tools check the result.
lengua's AI writes **assembly**: readable for review, and turned into machine code by a small,
deterministic assembler. The guarantees a compiler used to give come from rules and tests instead.

## A lengua program

A program is a folder with four files, created in this order:

| File | Written by | Purpose |
|---|---|---|
| `spec.md` | the person with the AI | The single source of truth: natural language, Given/When/Then behaviors with IDs, in [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf) (OKF) |
| `tests.toml` | the AI, from the spec only | One example test per behavior, plus generated tests with fresh random inputs on every run |
| `main.s` | the AI | x86-64 assembly in the lengua dialect |
| `lengua.lock` | only the lengua server | SHA-256 of all three files, a combined fingerprint, and a signature |

**Workflow:** spec → approve (the person confirms in a dialog) → tests (stamped) → code (stamped
once every rule and test passes) → optimize (tests stay green) → run. A program only runs when the
lock matches all three files, so documentation, tests and code can't drift from the spec.

| What changed | Result |
|---|---|
| `spec.md` | Tests and code are stale; re-approve, then re-check tests and code |
| `tests.toml` only | Refused: tests are frozen until the spec changes (the stamped copy can be restored) |
| `main.s` only | Re-run `check_code`; if everything passes, the new code is stamped |
| nothing | `run` works |

## What ships

The plugin folder `plugin/` contains:

- **Skills:** `write-spec`, `write-tests`, `write-assembly`. Each teaches one file and ends with
  that file's check tool.
- **MCP server** `lengua-mcp` (Rust, official `rmcp` SDK) with five tools:

| Tool | Does |
|---|---|
| `status` | Machine (OS, CPU, features, LLVM tools), client support, a program's state, and the next step |
| `approve_spec` | Validates `spec.md`, asks the person to approve it, stamps it |
| `check_tests` | Validates `tests.toml` (coverage, cross-checks, red check, freeze) and stamps it |
| `check_code` | Assembles, links, checks the rules, runs every test, then stamps and signs |
| `run` | Verifies the lock against every file, then runs the program |

Only open standards are used: MCP (tools, server instructions, elicitation), Agent Skills and Agent
Plugins. No hooks and no subagents, so the plugin works the same in Claude Code, Codex, Cursor and
other harnesses. Every guarantee lives in the server, which detects changes and refuses to stamp
or run.

## Packaging

| File | For |
|---|---|
| `plugin/plugin.json`, `plugin/mcp.json` | The [Agent Plugins](https://agent-plugins.org) standard (Codex, Cursor, Copilot, Kiro, VS Code) |
| `plugin/.claude-plugin/plugin.json`, `plugin/.mcp.json` | Claude Code, which ignores the standard's two files |

Both MCP configs start the same binary, `plugin/server/lengua-mcp` (built by
`scripts/build-plugin.sh`, not committed). The server keeps its private data in
`~/.local/share/lengua` (or `$XDG_DATA_HOME/lengua`): one signing key per user, shared by every
harness, so a lock approved in one harness verifies in all of them.

## The lengua dialect

AI-written assembly takes the best of C (transparency, no macros, the standard calling convention,
named constants, read-only data), C++ (one exit and cleanup path per function, assembler-computed
constants, local labels, specialized code instead of templates) and Rust (one owner per buffer, no
allocation, single-threaded, pointer + length bounds checks, `Result`-style errors, exhaustive
branches, marked `unsafe` blocks, dialect versions), checked against NASA's "Power of 10" rules
adapted for assembly. The full rule text lives in
`plugin/skills/write-assembly/references/power-of-10.md` (milestone 4). In short:

1. Simple control flow: jumps stay in their function, no recursion, compare chains end in an assertion.
2. Every loop counts down from a named maximum.
3. Fixed resources: no allocation, a system-call allowlist (read, write, exit by default), a computed maximum stack size.
4. Functions of at most 60 instructions, standard calling convention, one exit.
5. At least two assertions per function on average; a failed assertion exits with code 134.
6. Smallest scope: one owning function per buffer, only `_start` exported, read-only data stays read-only.
7. Every result is checked (`rax` < 0 means error).
8. No macros; named constants for system calls, exit codes, sizes and loop bounds.
9. No function pointers; indexed accesses are bounds-checked, except inside a reported `unsafe` block.
10. Warnings are errors, every change is checked, and the dialect is versioned.

The checker analyzes the **linked executable** together with the source, and nothing AI-written
runs before it passes the rules.

## Platforms

The MVP builds for Linux x86-64 (ELF, direct system calls), assembled by LLVM's `llvm-mc` and
linked by `ld.lld`. LLVM is the toolchain on every OS. Later, in this order: Linux ARM64 (testable
here with QEMU), macOS ARM64 (on a Mac, through Apple's LLVM), Windows x86-64 and Windows ARM64
(import libraries generated by `llvm-dlltool`, no Windows SDK). The server is organized around
per-(OS, CPU) platform profiles so each platform is an added profile, not a rewrite.

The plugin's own binary will ship for x86_64/aarch64 Linux (musl), aarch64 macOS and x86_64/aarch64
Windows.

## Code conventions

- The official Rust Style Guide and Rust API Guidelines, plus the parts of Google's Rust style guide
  that apply outside Google's own codebases.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: zero warnings, like rule 10.
- No `unsafe`. `Result` for recoverable errors, `expect("why")` instead of `unwrap()`, newtypes for
  distinct values, doc comments on public items.

## Threat model and known limits

- The lock's signature detects edits not made by the server. It is not a security boundary: any
  process running as the user, including the AI's shell, can read the key.
- Locks are machine-bound: on another machine the spec has to be approved again.
- Without hooks, the server detects and refuses rather than prevents. Someone could still assemble
  `main.s` by hand outside lengua, but that isn't a lengua run, and nothing lengua stamps or runs is
  affected.
- Clients without MCP elicitation approve specs through their tool-permission prompt; the lock
  records `approved_via = "tool-permission"`.
- Pointer depth (NASA rule 9) isn't checked; bounds checks and compare-chain detection are
  pattern-based.

## Status

MVP milestones: 1 foundation · 2 spec and lock · 3 tests · 4 rule checker · 5 code and run · 6 end
to end. Each milestone is committed after the project owner approves it.
