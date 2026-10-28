# lengua design

This file records every design decision, so any session or agent harness can pick up the project.
Where the code and this file disagree, fix one of them.

## Thesis

Programming languages and compilers were made as human middlemen to machine code. With AI, the
natural-language **spec is the source code**, the AI translates it, and tools check the result.
In short: turn the spec with its tests into machine code, using Claude Code (or any agent) as the
compiler. lengua's AI writes **assembly**: readable for review, and turned into machine code by a
deterministic assembler that only encodes each line as one instruction and makes no choices of its
own. The guarantees a compiler used to give come from rules and tests instead.

## A lengua program

A program is a folder with four files, created in this order:

| File | Written by | Purpose |
|---|---|---|
| `spec.md` | the person with the AI | The single source of truth: natural language, Given/When/Then behaviors with IDs, in [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf) (OKF) |
| `tests.toml` | the AI, from the spec only | One example test per behavior, plus generated tests with fresh random inputs on every run |
| `main.s` | the AI | x86-64 assembly in the lengua dialect |
| `lengua.lock` | only the lengua server | SHA-256 of all three files and of the checked executable, a combined fingerprint, and a signature |

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
| `check_code` | Assembles, links, checks the rules, runs every test and counts the instructions executed, then stamps and signs |
| `run` | Verifies the lock against every file, rebuilds, confirms the build is byte-identical to the checked one, then runs it |

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

## Tests

`tests.toml` has one example per behavior, holding that behavior's backtick values, and
generated blocks that draw new inputs on every run (boundary values first, then seeded random
ones) and compute the expected results with formulas. The formula language has checked 128-bit
whole numbers, strings, comparisons, `&& || !` and the functions `len`, `abs`, `min`, `max`,
`if`, `upper`, `lower` and `reverse`. It has no loops, so every formula finishes.

`check_tests` ties the tests to the approved spec: example exit codes equal the spec's, generator
limits are numbers the spec states, and args and stdin hold bare variables so each example can be
read back into variable values and every formula must reproduce it. A dry run evaluates every
run's expected results, so overflow and division by zero are caught before any code exists, and
output with placeholders must vary, so a program that prints a constant can't pass. Several
generated blocks may cover one behavior (divisors on both sides of zero, for example) when they
share their templates. Checked tests are frozen until the spec changes: the lock records them and
the server keeps a copy that restores them.

The examples in the skills are checked by the test suite against lengua's own checks, so the
documentation can't drift from the checker.

## Running programs

Tests and `run` start a program the same way: through the server's own binary in launcher mode,
which waits until the server has attached a CPU performance counter that turns on at `exec`, then
replaces itself with the program (no `unsafe` code needed). The program gets its stdin up front
and then the end of input, an empty environment, its own temporary folder, a time limit (2 s per
test, 10 s for `run`) and 1 MiB of output, and it's killed past either. The counters report three
measures, which the optimize step compares by their medians: instructions executed in the
program's own code (the steadiest measure of work, though a count can be one higher now and then,
because the CPU counts the odd interrupt or page fault as one), CPU cycles in its own code (time,
where slow instructions such as division weigh more), and CPU time including the kernel's work for
its system calls (fewer instructions isn't always faster: a program that writes one byte at a time
can execute fewer instructions yet spend far more time in the kernel).

`check_code` draws a new seed on every call, runs boundary values first, and reports the seed and
inputs with any failure, down to the line of a failed assertion. Only when the rules and every
test pass does it stamp `[code]`, including the executable's SHA-256 and the toolchain versions.
`run` rebuilds `main.s` and runs it only if the build is byte for byte that executable.

## The lengua dialect

AI-written assembly takes the best of C (transparency, no macros, the standard calling convention,
named constants, read-only data), C++ (one exit and cleanup path per function, assembler-computed
constants, local labels, specialized code instead of templates) and Rust (one owner per buffer, no
allocation, single-threaded, pointer + length bounds checks, `Result`-style errors, exhaustive
branches, marked `unsafe` blocks, dialect versions), checked against NASA's "Power of 10" rules
adapted for assembly. The full rule text lives in
`plugin/skills/write-assembly/references/power-of-10.md`. In short:

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
runs before it passes the rules. It reads `main.s` (one instruction per line, constants before
use, no macros), builds it, decodes the executable with iced-x86 and matches every instruction to
its line, operands included; anything that doesn't match is a violation, so what runs is exactly
what `main.s` says. Then it builds each function's control flow and exact stack depth and checks
the rules: unreachable lines are violations too. `assert_fail` is lengua's own code: it's compared
word for word with the template and is exempt from the rules it must break (it may always write
its message to standard error and exit with 134, whatever the spec allows). An assertion is
reached through a conditional jump, the fall-through of one, or a `jmp` right after one (the end
of a comparison chain or of an exhausted loop).

Instruction sets: dialect 1 uses x86-64's integer instructions, plus the integer extensions BMI1,
BMI2, POPCNT, LZCNT, ADX and MOVBE only when the CPU has them (the checker reads which CPU features
each instruction needs). Vector instructions (SSE, AVX2, AVX-512) come in dialect 2, right after
the MVP, with rules for their wider memory accesses.

## Translation validation

lengua treats the AI as an untrusted compiler. Instead of trusting it, lengua checks every output
it produces, the idea behind translation validation (Pnueli, Siegel and Singerman, 1998):

| Step | Translated by | How lengua checks it |
|---|---|---|
| spec → `tests.toml` | the AI | Structure and coverage; each behavior's spec values appear in its example; each formula reproduces its example; generator ranges are numbers that the spec's Purpose states (not an example's own values) |
| spec + tests → `main.s` | the AI | The rules, then every example plus fresh random tests on every check |
| `main.s` → machine code | LLVM | The checker reads the linked executable back and matches it to `main.s` instruction by instruction, operands included; the lock records the executable's hash, and `run` refuses any other build |

This is strong evidence, not proof: tests only sample inputs, and a plain-language spec has no
formal meaning a tool can check. After the MVP, a symbolic executor and an SMT solver will prove
that the output equals the formulas for every input in range, as Alive2 does for LLVM. The Power of
10 rules make that proof complete: without recursion and with a fixed maximum for every loop,
every path through a program is finite.

## Platforms

The MVP builds for Linux x86-64 (ELF, direct system calls), assembled by LLVM's `llvm-mc` and
linked by `ld.lld`. LLVM is the toolchain on every OS. Later, in this order: Linux ARM64 (testable
here with QEMU), macOS ARM64 (on a Mac, through Apple's LLVM), Windows x86-64 and Windows ARM64
(import libraries generated by `llvm-dlltool`, no Windows SDK). The server is organized around
per-(OS, CPU) platform profiles so each platform is an added profile, not a rewrite.

The plugin's own binary will ship for x86_64/aarch64 Linux (musl), aarch64 macOS and x86_64/aarch64
Windows.

After the MVP: dialect 2 (vector instructions), the proof step, and the other platforms in the
order above. The order between the proof and the platforms is decided after the MVP.

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
- An approval means the client reported that the user accepted the dialog. Some clients can be
  set up to answer dialogs automatically (in Claude Code, an `Elicitation` hook), and lengua can't
  tell the difference, so don't set one up for lengua.
- Approval dialogs travel as elicitation requests on protocol 2025-11-25 and as input-required
  results on 2026-07-28 (which has no server-to-client requests). There, the nonce is single-use,
  bound to the folder and to the spec text the dialog showed, and expires after 30 minutes. The
  spec is hashed again before the lock is written, so an edit made while the dialog is open is
  never approved.
- Rule 9 checks the shape of each bounds check (`cmp` and an unsigned `jae` before the access)
  and, for the program's own data symbols, that the limit fits the buffer. Moving a pointer
  instead of indexing isn't checked, nor is pointer depth, and compare-chain detection is
  pattern-based. So passing the rules doesn't make a program memory-safe: the tests guard the
  rest until the proof step.
- Tests sample inputs, so until the proof step a program is checked, not proven. The same AI
  writes the formulas and the code: each formula must reproduce its spec example, which narrows
  that gap, but a wrong formula can still match an example by coincidence.

## Status

The MVP is built: milestones 1 foundation, 2 spec and lock, 3 tests, 4 rule checker, 5 code and
run, and 6 end to end, each committed after the project owner approved it. `tests/e2e.rs` runs the
whole workflow on `examples/add` in both MCP protocol generations. Next comes dialect 2 (vector
instructions); then the proof step and the platforms (Linux ARM64, macOS ARM64, Windows x86-64,
Windows ARM64, in that order), with the order between those two decided after the MVP.
