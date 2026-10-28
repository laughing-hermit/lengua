---
name: write-spec
description: Writes or updates a lengua program's spec.md together with the user. spec.md is the natural-language source of truth that lengua turns into tests and checked x86-64 assembly. Use when the user wants a new lengua program, wants to change what one does, or when the lengua status tool says to write or fix spec.md. Ends by having the user approve the spec through the approve_spec tool.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write a lengua spec

A lengua program is a folder holding `spec.md`, `tests.toml`, `main.s` and `lengua.lock`. The
spec comes first and is the single source of truth: the tests and the code are derived from it
and checked against it. You write it with the user, and the user approves it.

## Steps

1. Call the lengua `status` tool with `path` set to the program folder's absolute path, such as
   `/home/you/projects/add`. If its `next_step` starts with "Blocked", tell the user and stop.
2. Ask the user what the program should do until you can state every behavior with concrete
   values: its inputs (arguments or standard input), its outputs (standard output and standard
   error), its exit codes, and the range of every number.
3. Copy [the template](assets/spec-template.md) to `<path>/spec.md` and fill it in by the rules
   below. To change an approved spec, edit it in place.
4. Show the user the whole spec in your message, and in the same turn call `approve_spec` with
   `path`. Its dialog, "Approve <path>/spec.md?", is the user's only question: don't ask
   separately whether the spec is right. (On clients without dialogs, the tool's permission
   prompt is the approval.) If the tool lists problems, fix them and call it again. If the user
   declines or closes the dialog, ask what to change, revise the spec, and call it again.
5. Follow the `next_step` it returns. Next comes `tests.toml`, with the write-tests skill.

## Rules

- **Frontmatter:** `type: lengua/program-spec`, a short `title` and a one-sentence
  `description`. Never add a `status` field: approval lives in `lengua.lock`, and any edit to the
  spec, even a status change, needs a new approval.
- **Sections:** `# Purpose`, `# Behaviors` and `# Constraints`.
- **One behavior per situation,** under a heading `## B<n>: <short title>`. IDs are unique: B1,
  B2, B3…
- **Steps** are `- Given …`, `- When …`, `- Then …`, in that order, continued with `- And …` or
  `- But …`. Each behavior has exactly one When.
- **One concrete example per behavior,** with every literal value in backticks: arguments
  (`2`), exact output (`5`), messages (`usage: add <a> <b>`) and the exit code. Use backticks
  only for literal values, because the tests must contain every one of them.
- **Exact output:** say what text is printed, whether a newline follows it, and whether it goes
  to standard output or standard error.
- **Exit codes:** each behavior ends with one, such as ``- And it exits with code `0` ``. Codes go
  from 0 to 255, and 134 is reserved for failed assertions.
- **Errors are behaviors too:** missing or extra arguments, numbers out of range, text that
  isn't a number. Give each one its own exit code other than 0.
- **Ranges:** in Purpose, give in backticks the range of every number the program reads and the
  length of any text it reads, including a range that only one behavior uses. Generated tests
  draw their inputs from exactly these ranges, so put nothing else in backticks there.
- **Write errors:** if the program prints, say in Purpose which exit code it uses when writing
  fails, without backticks (tests can't make writing fail), such as: If writing the output fails,
  the program exits with code 2.
- **System calls:** under `# Constraints`, write `- Allowed system calls: …` with only what the
  program needs, chosen from read, write, openat, close, lseek, exit and exit_group. `exit` is
  always needed. lengua programs can't allocate memory, start processes or threads, handle
  signals or use the network.

## Example

```markdown
---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`. If writing the output fails, the
program exits with code 2.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
```

## After approval

Every edit to `spec.md` makes `tests.toml` and `main.s` stale. lengua won't run the program
until the user approves the edited spec and the tests and code pass their checks again.
