---
name: write-spec
description: Writes or updates a lengua program's spec.md from the user's request. spec.md is the program's source in natural language, with its behaviors, each shown by an example in Given/When/Then steps, and the generated tests that try fresh inputs on every run; the compile skill then compiles it into checked assembly. Use when the user asks for a program, such as a command, a filter, a server or a client, unless they name another language; when they want to change a lengua program; or when the lengua check tool says to write or fix spec.md. Ends when the check tool records the spec with the user's own words, without waiting for an approval.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write a lengua spec

A lengua program is a folder of two files: `spec.md`, which says what the program does and holds
every test, and `main.s`, the assembly the compile skill compiles from it. lengua keeps its own
record beside them in `lengua.lock`. The spec is the source of truth and the only file people
share: `main.s` is disposable, compiled from it on each computer and checked there, so whatever
must be the same everywhere goes in the spec. You write it from the user's request; their request
is the go-ahead, so don't ask them to approve it. They read it in your final report, which says
what it promises.

## Steps

1. Use the folder the user names. If they name none, use a new folder named after the program in
   the current working directory, such as `<cwd>/add`. Tools take its absolute path.
2. Write `<path>/spec.md` in the shape of [the template](assets/spec-template.md), by the rules
   below: its behaviors, then the tests under each one. Ask the user only what their request
   leaves open and only they can decide, such as what the program computes; choose the rest
   yourself: messages, exit codes 1, 2, 3… and ranges, and the ports a program on the network
   may use. To change a spec, edit it in place.
3. Call the lengua `check` tool with `path` and, as `request`, the user's words that asked for
   this program or this change, quoted exactly from their message. Never make words up or put
   them in the user's mouth. Don't wait for an approval: go straight on. Only if the user asked to
   see the spec before you build, tell them in a line where it is, wait for their go-ahead, and
   call `check` then.
4. `check` lists every problem at once, in the spec and in its tests, with the fix for each; fix
   them all and call it again. If its `next_step` starts with "Blocked", tell the user and stop.
   Then follow its `next_step`: next comes `main.s`, with the compile skill.

## The spec

A spec follows behavior-driven development (BDD): each behavior is one rule of the program, shown
by one concrete example in its steps, and its generated tests try that rule with fresh inputs.

- **The same on every computer:** a spec says what the program does, never what it runs on. Name
  no CPU, instruction set, operating system, kernel, system call or toolchain, and no one
  computer's limits; how the program is built for each computer is the compile skill's. Only a
  program whose subject is such a thing, such as an instruction decoder, names it, as its
  subject.
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
  `1000000`. Give both ends of every number's range (from `1` to `100`); a text length may start
  at 0 without saying so. Keep text on standard input to at most `1048576` bytes, arguments
  short, at most `4096` bytes each (long text goes on standard input or in a file, so the program
  takes it the same way on every computer), and numbers within 64 bits. If the program reads
  or writes, Purpose says which exit code it uses when reading or writing fails, without
  backticks (tests can't make those fail), such as: If writing the output fails, the program
  exits with code 2.
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
- **Steps in lengua's phrasings,** which [the reference](references/test-format.md#the-steps)
  lists, so the steps are the behavior's example test and nothing is written twice. Given steps
  set the scene (``- Given the arguments `2` and `3` ``, ``- Given standard input holding `…` ``,
  ``- Given a file `notes.txt` holding `…` ``), the one When is ``- When the program runs``, and
  Then steps say what the user can observe: ``- Then it prints `5` followed by a newline``, ``-
  Then the file `loud.txt` holds `HELLO` ``, and last ``- And it exits with code `0` ``. `- And
  …` continues the step before. Values go in backticks, exactly as the program reads or prints
  them, with `\n`, `\r` and `\t` for line breaks and tabs; describe nothing in a step that isn't
  one of these.
- **Declarative steps:** say what the user gives and sees, never how the code works. No buffers,
  registers, system calls or function names in a behavior.
- **One concrete example per behavior,** holding just the values its rule needs. A long input,
  such as a line one character over a limit, is a repeat: ``standard input holding `a` repeated
  `4097` times followed by a newline``.
- **Exact output:** say what text is printed, whether a newline follows it, and whether it goes
  to standard output or standard error.
- **Exit codes:** each behavior ends with one. Codes go from 0 to 255, and 134 is reserved for
  failed assertions.
- **Errors are behaviors too:** missing or extra arguments, numbers out of range, text that
  isn't a number. Give each one its own exit code other than 0.
- **Files:** a program works only on files in the folder it runs in, named relative to it; it
  can't reach other files at all. Within it, it can read, write, list folders, read a file's
  size, make and remove folders, and delete and rename files. Purpose states the largest file it reads in backticks, like any
  other range, and what happens when a file is missing.
- **What it uses:** under `# Constraints`, ``- Uses: …`` names what the program needs beyond
  writing its output and exiting, from `standard input`, `files`, `the clock`, `random bytes` and
  `the environment` (its environment variables: only those a test, or the user's run, gives it),
  or `- Uses: nothing`. lengua programs can't start processes or handle signals, and their memory
  is fixed by the spec's limits. lengua holds the program to this line when it runs, so it's a promise the user can rely
  on, and it means the same on every kind of computer. `# Constraints` holds only `- Uses:`,
  `- Network:` and `- Threads:` lines; anything else goes in Purpose.
- **Network:** plain TCP, on ports the spec names, to this computer only unless the line ends
  with `on any computer`. lengua refuses every other port and every other computer when the
  program runs. Choose the ports yourself, without asking: a spec can't allow "any port", so
  when the request leaves the port to the user, as "given a port" does, list the ports such
  programs usually use, such as 80, 3000, 5000, 8000, 8080 and 8888 for local web servers, and
  name them in your final report with an offer to change them. Write `on any computer` only when
  the user asks to reach other computers, and say so in your report.
  - A program that opens connections says `- Network: connects to TCP port 8080` (or `ports 80
    and 8080`). The address and port it reaches come from its arguments or input, as numbers
    (`127.0.0.1`), since lengua programs don't look up names. Don't have the program refuse other
    ports itself: lengua holds it to the spec's ports when it runs, and its tests talk over a
    free port of their own, so a program that checks for 8080 can't be tested.
  - A server says `- Network: listens on TCP port 8080`: lengua listens on that port of this
    computer and hands the program the socket as file descriptor 3, and its behaviors may end
    with ``- And it keeps serving`` instead of an exit code. A request ends with something the
    server can see, such as its newline, its blank line or a stated length: clients such as curl
    and browsers keep their side of the connection open until they have the answer, so a server
    that waits for them to close waits forever.
  - In the final report, tell the user in plain words what the program may reach, such as "it can
    open TCP connections to port 8080 on this computer; lengua refuses every other port and every
    other computer."
- **Websites and APIs:** lengua programs speak plain TCP, not HTTPS. A program that needs a
  website's data reads it from standard input or a file; you fetch it with your own tools and
  pass it in, and tell the user the line for their shell, such as `curl -s URL | ./program`.
- **Output from random bytes or the clock** is best made exact. Purpose says how the program
  turns them into output ("the roll is the first byte below `252`, modulo 6, plus 1"), and the
  behavior gives them as its scene: ``- Given the random bytes `fc` and `2a` `` or
  ``- Given the clock at `1735689600` ``. The tests then supply exactly those bytes and readings,
  and the proof covers every value they could take. When the user only cares that output falls
  in a range, the behavior says so with a rule: ``- Then it prints text matching
  `rolled {any(1, 6)}` followed by a newline``.
- **Large inputs:** a program for data of any size says so in Purpose, such as "it reads standard
  input of any length, a piece at a time", and still gives the largest input the tests use in
  backticks (`1048576`): tests stay within a megabyte, and the program streams beyond it.
- **Threads:** when the user wants a program that does heavy work on large inputs as fast as
  possible, add `- Threads: up to N` under `# Constraints`, with N up to the machine's cores
  (`check` reports them in `machine`; at most 64). The program may then split its work across
  that many cores. Behaviors don't change: the output is the same as on one thread.

## The tests

Under each behavior whose steps give the program something, write its generated tests: a `###`
heading such as `### For any two numbers`, then the behavior's steps again with `{placeholders}`
where values vary, then a table with a column for each variable, holding its generator, and a row
for each variant, as a Gherkin Scenario Outline has its Examples. Each run draws new values and
the formulas compute what the program must do. [The reference](references/test-format.md) lists
every phrasing, generator and function. Write them from the spec alone, never from code: they come
before the code, and they decide whether it's right.

- **Formulas agree with the example.** Given the example's inputs, the formulas must produce its
  results. They must never overflow or divide by zero for inputs in range, and the output must
  change with the inputs.
- **Ranges come from Purpose.** Generators may only use numbers that `# Purpose` writes in
  backticks; numbers from behaviors don't count. If Purpose lacks a range you need, add it there.
- **Inputs are bare variables.** In what the program is given, a placeholder holds just a
  variable (`{a}`); computations belong in what it prints, the files it leaves and its exit code
  (`{a + b}`).
- **One formula per output.** Compute text output from the whole input with the functions, such
  as `{upper(text)}` or `{tr(text, "A-Za-z", "N-ZA-Mn-za-m")}`, not one variable per character.
- **Invalid input gets hand-picked values.** For a behavior about input that isn't valid, use
  `choice(…)` with the example's own value plus values that go wrong late, not only at the first
  character. For a number: `"12x"`, `"-"`, `"1.5"`, `"--1"` and `""`.
- **Decimals.** Draw decimal inputs with `decimal(lo, hi)`, written with Purpose's places. For
  exact decimals such as money, compute in whole units and print with `places`:
  `{places(scaled(a, 2) + scaled(b, 2), 2)}`. For measured decimals, compute in doubles and print
  with `fixed`: `{fixed(sqrt(x), 3)}`, or in singles when the spec says single precision:
  `{fixed(sqrt(single(x)), 3)}`. Write a multiply followed by an add, as in a dot product or a
  polynomial, as `fma(a, b, c)`: it rounds once, which is more accurate, and lets the program use
  one `vfmadd` instruction. Write `2.0`, not `2`, next to decimals.
- **A computing behavior gets a row of its numbers alone.** The proof goes through text lengths
  one by one, so a row with several free texts proves only short ones. When a behavior computes
  with numbers among other text (a total of amounts between other fields, a date, a count), add a
  row where only the numbers vary, with each other variable a `choice(…)` of one or a few values.
  The proof then covers every number.
- **Small ranges are checked in full.** A row whose ranges hold at most 5000 combinations (and no
  `choice`) runs every one of them, so a range the spec states exactly is worth more than a wide
  one.
- **Joined text counts whole.** A template that adds text around a variable, such as `"{a}x"` or
  `"{word} {rest}"`, gives the program more than the variable alone: keep the total inside the
  spec's limits for behaviors about valid input.
- **Lengths count characters.** `string(0, 1048576, "aé€")` can reach 3 MiB, since `é` takes 2
  bytes and `€` 3, while specs and programs count bytes. Draw long text from 1-byte (ASCII)
  characters, and test other characters in short strings: a proof covers each order of their
  widths, so it reaches about 8 characters where three widths mix, and 13 where two do.
- **Network messages keep their shape.** What a test's client sends is what real clients send:
  for HTTP, curl's request line and headers, such as `"GET /{path} HTTP/1.1\r\nHost:
  localhost\r\nUser-Agent: curl/8.0\r\nAccept: */*\r\n\r\n"`. A header line drawn as text can
  come out empty and end the headers early, so start it with fixed text, such as
  `"X-{name}: {value}\r\n"`.

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

### For any two numbers
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Shows usage when arguments are missing
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
```

B2 gives the program nothing, so it needs no generated tests. Each of B1's runs draws a new `a`
and `b`, trying boundary values first (the range's ends, 0, 1 and -1), and expects their sum.

## After recording

Every edit to `spec.md`, its tests included, is recorded by the next `check` under the words
already recorded, and the code is checked against it again: pass `request` again only with the
user's new words, when they ask for something new. When `check` reports a failing test, decide
from the spec whether the test or `main.s` is wrong; if the test is, fix it and call `check`,
with no one to ask. lengua shows every change after the user's request in its final reply
(`changes`), with its own marks on the changes that test less (`tests_less`), and keeps every
request of a build until its code passes, so nothing drops out: tell the user each change, and
why, in your final report. Never change the spec or weaken a test to make code pass; fix the
code.

A folder from an older lengua may still have a `tests.toml`: make sure each behavior's steps say
what its `[[example]]` said, write its `[[generated]]` tables as the behavior's generated tests,
their fields as steps and their `vars` and `runs` as rows of the table, then delete `tests.toml`.
