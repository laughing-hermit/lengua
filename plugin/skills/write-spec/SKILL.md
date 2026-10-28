---
name: write-spec
description: Writes or updates a lengua program's spec.md for the user. spec.md is the natural-language source of truth that lengua turns into tests and checked x86-64 assembly. Use when the user wants a new lengua program, wants to change what one does, or when the lengua status tool says to write or fix spec.md. Ends when the user says they approve the spec and you record it with the approve_spec tool.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write a lengua spec

A lengua program is a folder holding `spec.md`, `tests.toml`, `main.s` and `lengua.lock`. The
spec comes first and is the single source of truth: the tests and the code are derived from it
and checked against it. You write it; the user reads it in their own editor and approves it.

## Steps

1. Use the folder the user names. If they name none, use a new folder named after the program in
   the current working directory, such as `<cwd>/add`. Tools take its absolute path.
2. Call the lengua `status` tool with `path`. If its `next_step` starts with "Blocked", tell the
   user and stop.
3. Write `<path>/spec.md` in the shape of [the template](assets/spec-template.md), by the rules
   below. Ask the user only what you can't sensibly decide yourself, such as what the program
   computes; choose the rest yourself: messages, exit codes 1, 2, 3… and ranges. To change an
   approved spec, edit it in place.
4. Call `status` with `path` again. If its `next_step` lists problems in the spec, fix them.
5. Tell the user in a few lines that `<path>/spec.md` is ready to read in their editor, and name in
   one sentence the choices you made that they didn't state. Don't paste the spec into the chat.
   Ask them to tell you when they approve it, for example "I approve <path>/spec.md".
6. When the user says they approve it, call `approve_spec` with `path` and their words, quoted
   exactly, as `approval`. Only the user approves a spec: never call it on your own or put words
   in their mouth. If they ask for changes instead, edit `spec.md`, tell them it's ready again,
   and wait.
7. Follow the `next_step` it returns. Next comes `tests.toml`, with the write-tests skill.

## Rules

A spec follows behavior-driven development (BDD): each behavior is one rule of the program, shown
by one concrete example, in the user's words. The tests then run that example and more examples
of the same rule.

- **Frontmatter:** `type: lengua/program-spec`, a short `title` and a one-sentence
  `description`. Never add a `status` field: approval lives in `lengua.lock`, and any edit to the
  spec, even a status change, needs a new approval.
- **Sections:** `# Purpose`, `# Behaviors` and `# Constraints`.
- **Purpose** says in plain words what the program does, for whom and why. It then gives in
  backticks the range of every number the program reads and the length of any text it reads,
  including a range only one behavior uses: generated tests draw their inputs from exactly these
  ranges, so nothing else goes in backticks there. If the program reads or writes, Purpose says
  which exit code it uses when reading or writing fails, without backticks (tests can't make
  those fail), such as: If writing the output fails, the program exits with code 2.
- **One rule per behavior,** under a heading `## B<n>: <title>` with a unique ID (B1, B2, B3…).
  The title names the rule in the user's words, such as `Adds two numbers` or `Rejects a line
  that is too long`, and no two titles are the same. Don't add a behavior that only repeats
  another with other values: adding negative numbers is still adding.
- **Given, When, Then:** Given steps set the scene (the arguments, the input), the one When is the
  event (`- When the program runs`), and Then steps say only what the user can observe: the
  output and the exit code. `- And …` or `- But …` continue the step before. Keep each behavior
  to these few steps.
- **Declarative steps:** say what the user gives and sees, never how the code works. No buffers,
  registers, system calls or function names in a behavior.
- **One concrete example per behavior,** holding just the values its rule needs. Backticks hold
  only text that appears character for character in the input or output, because the tests must
  contain every one of them: arguments (`2`), exact output (`5`), messages
  (`usage: add <a> <b>`) and the exit code. Describe anything else in words, without backticks:
  "a line of 4097 characters", "no arguments".
- **Exact output:** say what text is printed, whether a newline follows it, and whether it goes
  to standard output or standard error.
- **Exit codes:** each behavior ends with one, such as ``- And it exits with code `0` ``. Codes go
  from 0 to 255, and 134 is reserved for failed assertions.
- **Errors are behaviors too:** missing or extra arguments, numbers out of range, text that
  isn't a number. Give each one its own exit code other than 0.
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
Lets a shell script add two whole numbers: it prints the sum of two numbers between `-1000000000`
and `1000000000`, given as arguments. If writing the output fails, the program exits with code 2.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Shows usage when arguments are missing
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
