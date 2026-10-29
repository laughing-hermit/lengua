---
name: write-spec
description: Writes or updates a lengua program's spec.md from the user's request. spec.md is the natural-language source of truth that lengua turns into tests and checked x86-64 assembly. Use when the user wants a new lengua program, wants to change what one does, or when the lengua check tool says to write or fix spec.md. Ends when the check tool records the spec with the user's own words, without waiting for an approval.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write a lengua spec

A lengua program is a folder holding `spec.md`, `tests.toml`, `main.s` and `lengua.lock`. The
spec comes first and is the single source of truth: the tests and the code are derived from it
and checked against it. You write it from the user's request; their request is the go-ahead, so
don't ask them to approve it. They read it in your final report, which says what it promises.

## Steps

1. Use the folder the user names. If they name none, use a new folder named after the program in
   the current working directory, such as `<cwd>/add`. Tools take its absolute path.
2. Write `<path>/spec.md` in the shape of [the template](assets/spec-template.md), by the rules
   below. Ask the user only what their request leaves open and only they can decide, such as what
   the program computes; choose the rest yourself: messages, exit codes 1, 2, 3… and ranges, and
   the ports a program on the network may use. To change a spec, edit it in place.
3. Call the lengua `check` tool with `path` and, as `request`, the user's words that asked for
   this program or this change, quoted exactly from their message. Never make words up or put
   them in the user's mouth. Don't wait for an approval: go straight on. Only if the user asked to
   see the spec before you build, tell them in a line where it is, wait for their go-ahead, and
   call `check` then.
4. If `check` lists problems in the spec, fix them and call it again; if its `next_step` starts
   with "Blocked", tell the user and stop. Then follow its `next_step`: next comes `tests.toml`,
   with the write-tests skill.

## Rules

A spec follows behavior-driven development (BDD): each behavior is one rule of the program, shown
by one concrete example, in the user's words. The tests then run that example and more examples
of the same rule.

- **Frontmatter:** `type: lengua/program-spec`, a short `title` and a one-sentence
  `description`. Never add a `status` field: the record lives in `lengua.lock`, and any edit to
  the spec, even a status change, is recorded again.
- **Sections:** `# Purpose`, `# Behaviors` and `# Constraints`.
- **Purpose** says in plain words what the program does, for whom and why. It then gives in
  backticks the range of every number the program reads and the length of any text it reads,
  including a range only one behavior uses: generated tests draw their inputs from exactly these
  ranges, so nothing else goes in backticks there. Write each range as the tests will draw it:
  when an input is built from parts, give each part's own range, such as a path that is `/`
  followed by `0` to `2047` characters, or a reply of headers up to `1000` bytes and a body up to
  `1000000`, so the tests never need the spec reworded. Give both ends of every number's range (from
  `1` to `100`); a text length may start at 0 without saying so. Keep text on standard input to at
  most
  `1048576` bytes and each argument to at most `131072` (Linux's limit), and numbers within 64
  bits. If the program reads or writes, Purpose says
  which exit code it uses when reading or writing fails, without backticks (tests can't make
  those fail), such as: If writing the output fails, the program exits with code 2.
- **Decimals** are written with a point, in Purpose too: `0.00` to `999.99` means two places.
  **Exact decimals** such as money (`12.50`) are computed exactly, in whole cents; say in words
  how a result rounds when it has more places than it prints ("half a cent rounds away from
  zero"). **Measured decimals** (lengths, ratios, square roots) are computed in doubles: in their
  examples, avoid values that end in exactly 5 past the printed places, such as `1.015` printed
  with 2, because binary doubles round them in ways people don't expect. When the user wants
  speed more than digits, Purpose may say the results are computed in single precision (about 7
  significant digits), which doubles what each vector instruction does; it pays off when the
  program computes many values, not one. Either way the tests compute in the same precision, step
  by step, so they always agree with the program: precision decides which digits print, never
  whether a test passes.
- **One rule per behavior,** under a heading `## B<n>: <title>` with a unique ID (B1, B2, B3…).
  The title names the rule in the user's words, such as `Adds two numbers` or `Rejects a line
  that is too long`, and no two titles are the same. Don't add a behavior that only repeats
  another with other values: adding negative numbers is still adding.
- **Given, When, Then, in that order:** Given steps first set the scene (the arguments, the input,
  the files), then the one When is the event (`- When the program runs`), then Then steps say only
  what the user can observe: the output, the files written and the exit code. The first step is a
  Given; `- And …` or `- But …` continue the step before. Keep each behavior to these few steps.
- **Files:** name them and quote their contents: ``- Given a file named `notes.txt` holding
  `hello` `` and ``- Then `loud.txt` holds `HELLO` ``. A program works only on files in the folder
  it runs in, named relative to it; it can't reach other files at all. Purpose states the largest
  file it reads in backticks, like any other range, and what happens when a file is missing.
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
  program needs, chosen from read, write, openat, close, lseek, exit, exit_group, clock_gettime
  (reading the time), getrandom (random bytes from the kernel), and for the network socket,
  connect, accept4, shutdown and sendto. `exit` is always needed, and files need openat and
  close. lengua programs can't allocate memory, start processes or handle signals. The kernel
  enforces this list when the program runs, so it's a promise the user can rely on.
- **Network:** plain TCP, on ports the spec names, to this computer only unless the line ends
  with `on any computer`. lengua refuses every other port and every other computer when the
  program runs. Choose the ports yourself, without asking: a spec can't allow "any port", so
  when the request leaves the port to the user, as "given a port" does, list the ports such
  programs usually use, such as 80, 3000, 5000, 8000, 8080 and 8888 for local web servers, and
  name them in your final report with an offer to change them. Write `on any computer` only when
  the user asks to reach other computers, and say so in your report. A program that opens
  connections says `- Network: connects to TCP port 8080` (or `ports 80 and 8080`) and allows
  socket and connect;
  the address and port it reaches come from its arguments or input, as numbers (`127.0.0.1`),
  since lengua programs don't look up names. Don't have the program refuse other ports itself:
  the kernel holds it to the spec's ports when it runs, and its tests talk over a free port of
  their own, so a program that checks for 8080 can't be tested. A server says `- Network: listens on TCP port 8080`
  and allows accept4 and sendto: lengua listens on that port of this computer and hands the
  program the socket as file descriptor 3; it answers with sendto, so a client that hangs up early
  can't stop it (with write, the kernel would end it with SIGPIPE); and its behaviors may end
  with `- And it keeps serving` instead of an exit code. Write a protocol's line breaks as they
  are sent, such as `GET / HTTP/1.0\r\n`. In the final report, tell the user in plain words what the program may reach, such
  as "it can open TCP connections to port 8080 on this computer; lengua refuses every other port
  and every other computer."
- **Websites and APIs:** lengua programs speak plain TCP, not HTTPS. A program that needs a
  website's data reads it from standard input or a file; you fetch it with your own tools and
  pass it in, and tell the user the line for their shell, such as `curl -s URL | ./program`.
- **Output from random bytes or the clock** is best made exact. Purpose says how the program
  turns them into output ("the roll is the first byte below `252`, modulo 6, plus 1"), and the
  behavior gives them as its scene: ``- Given the kernel's random bytes `fc` and `2a` `` or
  ``- Given the clock at `1735689600` `` (seconds since 1970). The tests then supply exactly
  those bytes and readings, and the proof covers every value they could take. When the user
  only cares that output falls in a range, the behavior can say that instead, both ends in
  backticks: ``- Then it prints `rolled ` and a number from `1` to `6` ``.
- **Large inputs:** a program for data of any size says so in Purpose, such as "it reads standard
  input of any length, a piece at a time", and still gives the largest input the tests use in
  backticks (`1048576`): tests stay within a megabyte, and the program streams beyond it.
- **Threads:** when the user wants a program that does heavy work on large inputs as fast as
  possible, add `- Threads: up to N` under `# Constraints`, with N up to the machine's cores
  (`check` reports them in `machine`; at most 64). The program may then split its work across that many
  cores. Behaviors don't change: the output is the same as on one thread.

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

## After recording

Every edit to `spec.md` is recorded by the next `check` under the words already recorded, and
the tests and code are checked against it again: pass `request` again only with the user's new
words, when they ask for something new. lengua shows every change after the user's request in its
final reply (`changes`), and keeps every request of a build until its code passes, so nothing
drops out: tell the user each change, and why, in your final report. Never change the spec to
make code pass; fix the code.
