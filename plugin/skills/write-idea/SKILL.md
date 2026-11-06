---
name: write-idea
description: Writes or updates a lengua program's idea.md from the user's request. idea.md is the program's source in natural language, written in ASD-STE100 Simplified Technical English (STE), with its behaviors, each shown by an example in Given/When/Then steps, and the generated tests that try fresh inputs on every run; the compile skill then compiles it into checked assembly. Use when the user asks for a program, such as a command, a filter, a server or a client, unless they name another language; when they want to change a lengua program; or when the lengua check tool says to write or fix idea.md. Ends when the check tool records the idea with the user's own words, without waiting for an approval.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write a lengua idea

A lengua program is a folder of two files: `idea.md`, which says what the program does and holds
every test, and `main.s`, the assembly the compile skill compiles from it. lengua keeps its own
record beside them in `lengua.lock`. The idea is the source of truth and the only file people
share: `main.s` is disposable, compiled from it on each computer and checked there, so whatever
must be the same everywhere goes in the idea. You write it from the user's request; their request
is the go-ahead, so don't ask them to approve it. They read it in your final report, which says
what it promises.

## Steps

1. Use the folder the user names. If they name none, use a new folder named after the program in
   the current working directory, such as `<cwd>/add`. Tools take its absolute path.
2. Write `<path>/idea.md` in the shape of [the template](assets/idea-template.md), by the rules
   below: its behaviors, then the tests under each one. Ask the user only what their request
   leaves open and only they can decide, such as what the program computes; choose the rest
   yourself: messages, exit codes 1, 2, 3… and ranges, and the ports a program on the network
   may use. To change an idea, edit it in place.
3. Call the lengua `check` tool with `path` and, as `request`, the user's words that asked for
   this program or this change, quoted exactly from their message. Never make words up or put
   them in the user's mouth. Don't wait for an approval: go straight on. Only if the user asked to
   see the idea before you build, tell them in a line where it is, wait for their go-ahead, and
   call `check` then.
4. `check` lists every problem at once, in the idea and in its tests, with the fix for each; fix
   them all and call it again. If its `next_step` starts with "Blocked", tell the user and stop.
   Then follow its `next_step`: next comes `main.s`, with the compile skill. Its `graph` lines
   say how the program's bundle stands, each starting with how much it matters: fix what
   `blocks` first, then the warnings in the ideas you wrote; tell the user the rest.

## The idea

An idea follows behavior-driven development (BDD): each behavior is one rule of the program, shown
by one concrete example in its steps, and its generated tests try that rule with fresh inputs.

- **The same on every computer:** an idea says what the program does, never what it runs on. Name
  no CPU, instruction set, operating system, kernel, system call or toolchain, and no one
  computer's limits; how the program is built for each computer is the compile skill's. Only a
  program whose subject is such a thing, such as an instruction decoder, names it, in its title
  too; `check` points out any other.
- **Frontmatter:** `type: lengua/program-idea`, a short `title` and a one-sentence
  `description`, which `check` requires and the bundle's `index.md` lists the program with.
  Never add a `status` field: the record lives in `lengua.lock`, and any edit to the idea, even a
  status change, is recorded again.
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
  or writes, Purpose says which exit code it uses when a read or a write fails, without
  backticks, such as: If the program cannot write its output, it exits with code 2. Give a
  program that prints to standard output a behavior for that code, with ``- And standard output
  is closed`` and ``- Then it exits with code `2` ``, and inputs that make it print something:
  tests can make standard output fail, as a closed pipe does, but not reading, nor writing a
  file.
- **Decimals** are written with a point, in Purpose too: `0.00` to `999.99` means two places.
  **Exact decimals** such as money (`12.50`) are computed exactly, in whole cents; say in words
  how a result rounds when it has more places than it prints ("half a cent rounds away from
  zero"). **Measured decimals** (lengths, ratios, square roots) are computed in doubles: in their
  examples, avoid values that end in exactly 5 past the printed places, such as `1.015` printed
  with 2, because binary doubles round them in ways people don't expect. When the user wants
  speed more than digits, Purpose may say the results are computed in single precision (about 7
  significant digits), which lets a computer work on twice as many values at a time; it pays
  off when the program computes many values, not one. Either way the tests compute in the same precision, step
  by step, so they always agree with the program: precision decides which digits print, never
  whether a test passes.
- **One rule per behavior,** under a heading `## B<n>: <title>` with a unique ID (B1, B2, B3…).
  The title names the rule in the user's words, such as `Adds two numbers` or `Rejects a line
  that is too long`, and no two titles are the same. Don't add a behavior that only repeats
  another with other values: adding negative numbers is still adding.
- **Steps in lengua's phrasings,** which [the reference](references/test-format.md#the-steps)
  lists, so the steps are the behavior's example test and nothing is written twice. Given steps
  set the scene (``- Given the arguments `2` and `3` ``, ``- Given standard input that contains
  `…` ``, ``- Given a file `notes.txt` that contains `…` ``), the one When is ``- When the
  program runs``, and Then steps say what the user can see: ``- Then it prints `5` and a
  newline``, ``- Then the file `loud.txt` contains `HELLO` ``, and last ``- And it exits with
  code `0` ``. `- And …` continues the step before. Values go in backticks, exactly as the
  program reads or prints them, with `\n`, `\r` and `\t` for line breaks and tabs; describe
  nothing in a step that isn't one of these. Each step is a short sentence in Simplified
  Technical English, as the next section says.
- **Declarative steps:** say what the user gives and sees, never how the code works. No buffers,
  registers, system calls or function names in a behavior.
- **One concrete example per behavior,** with just the values its rule needs. A long input,
  such as a line one character over a limit, is a repeat: ``standard input that contains `a`
  `4097` times and a newline``.
- **Exact output:** say what text is printed, whether a newline follows it, and whether it goes
  to standard output or standard error.
- **Exit codes:** each behavior ends with one. Codes go from 0 to 255, and 134 is reserved for
  failed assertions.
- **Errors are behaviors too:** missing or extra arguments, numbers out of range, text that
  isn't a number. Give each one its own exit code other than 0. Every limit Purpose states, such
  as the largest input, the longest line or the most rows, gets a behavior for input just past
  it, with its message and exit code: a program never quietly drops what's past a limit.
- **Files:** a program works only on files in the folder it runs in, named relative to it; it
  can't reach other files at all. Within it, it can read, write, list folders, read a file's
  size, make and remove folders, and delete and rename files. Purpose states the largest file it reads in backticks, like any
  other range, and what happens when a file is missing. Names are held to what every system
  takes, so a test's files must be too: no `< > : " \ | ? *`, no name ending in a dot or a
  space, none of Windows's device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to
  `LPT9`, with any extension), no short name's shape (eight characters at most ending in `~` and
  digits, such as `REPORT~1.TXT`), at most 255 bytes, and never two names in one folder that
  differ only in case. A program can't rename or remove a file it holds open, and its result must not
  depend on the order a folder lists in, which differs between file systems: sort what it
  lists.
- **What it uses:** under `# Constraints`, ``- Uses: …`` names what the program needs beyond
  writing its output and exiting, from `standard input`, `files`, `the clock`, `random bytes` and
  `the environment` (its environment variables: only those a test, or the user's run, gives it),
  `the terminal` (keys as they're pressed, without Enter or echo, a wait for the next key, and the
  window's size: for games, menus and other interactive programs, whose tests give the keys as
  standard input and write escape sequences with `\e`),
  or `- Uses: nothing`. lengua programs can't start processes or handle signals, and their memory
  is fixed by the idea's limits. lengua holds the program to this line when it runs, so it's a promise the user can rely
  on, and it means the same on every kind of computer. `# Constraints` holds only `- Uses:`,
  `- Network:`, `- Threads:` and `- Builds on:` lines; anything else goes in Purpose.
- **Network:** plain TCP, to the places the idea names, and nothing else. A program never opens
  a connection itself: lengua makes each one its idea names before the program starts and hands
  it over, so it's the same on every kind of computer. Choose the places yourself, without
  asking: an idea can't allow "any port" or "any computer", so when the request leaves the port
  to the user, as "given a port" does, pick the one such programs usually use, such as 8080 for a
  local web server, and name it in your final report with an offer to change it.
  - A program that talks to a server says `- Network: connects to TCP port 8080` for one on this
    computer, or `- Network: connects to TCP port 443 on example.com` for another computer, named
    as DNS writes it (or by address, never `localhost`: a port alone is this computer's). A line
    may name several ports, as `ports 80 and 8080`, and an idea several lines, up to 16
    connections. Each is one connection, made before the program starts, in the order the lines
    name them; the program reads and writes the first on descriptor 4, the next on 5, and so on.
    With no server there, the program gets that connection already ended: its first
    read gives the end and its first write fails. Give such a program a behavior for it, as
    fetch's "Says when the server can't be reached", with ``- And no server on port `8080` ``.
    The program needs no port or address in its arguments; the person running it can't point
    it elsewhere, unless the idea lets them name the computer.
  - A tool whose user types the computer, as with curl, an ssh or a database client, says so:
    `- Network: connects to TCP port 443 on the computer its first argument names` (`second` to
    `ninth` for another argument, and `over TLS` too). lengua reads that argument when the
    program starts and connects there; an argument that names this computer or no computer, or
    a computer that doesn't answer, gives the program the connection already ended, as for a
    named computer. The program still gets the argument, for its `Host:` line. Its tests' server
    stands in for the connection whatever the argument says, so give the argument values like
    `example.com` in the tests, and a behavior for a computer it can't reach. The person running
    it chooses only the computer; the port stays the idea's.
  - `over TLS` at the end of a line that names a computer, as in `- Network: connects to TCP
    port 443 on api.example.com over TLS`, has lengua do TLS on that connection: it checks the
    computer's certificate against the same roots on every computer, and the program sends and
    reads plain bytes, so an HTTPS client speaks plain HTTP on its descriptor. TLS needs the
    computer's name, so never `over TLS` for this computer.
  - A server says `- Network: listens on TCP port 8080`: lengua listens on that port and hands
    the program its connections, and its behaviors may end with ``- And it does not stop``
    instead of an exit code. Only programs on this computer can
    connect to it, unless the line ends with `for any computer`: write that only when the user
    asks for a server other computers reach, and say so in your report. A server serves one
    client at a time unless the line says how many at once, as `- Network: listens on TCP port
    8080, up to 64 clients at once` does: write that whenever clients may overlap, as a chat,
    web or game server's do, sized from what the user says or a few dozen, and give a behavior
    two or more clients and `- And the clients connect at once` (see the test format). A request ends with something the
    server can see, such as its newline, its blank line or a stated length: clients such as curl
    and browsers keep their side of the connection open until they have the answer, so a server
    that waits for them to close waits forever.
  - In the final report, tell the user in plain words what the program may reach, such as "it
    talks to TCP port 8080 on this computer, through a connection lengua makes for it before it
    starts; it can reach nothing else."
- **Websites and APIs:** a program reaches a website or an API itself, over HTTPS, with `-
  Network: connects to TCP port 443 on api.example.com over TLS`: it sends a plain HTTP request
  on its connection, with the `Host:` line HTTP/1.1 needs, and reads the response, as `fetch`
  does on this computer's port. Ask for `Connection: close` (or use HTTP/1.0) so the server ends
  the response by closing, unless the program reads `Content-Length`. Its tests stand a server of
  their own in for the site, as for any connection, so give them the exchanges the site would
  give. Each site is one line, and the program can reach no other; when the user wants any URL,
  say that a lengua program reaches only the sites its idea names, and offer to name them.
- **Output from random bytes or the clock** is best made exact. Purpose says how the program
  turns them into output ("the roll is the first byte below `252`, modulo 6, plus 1"), and the
  behavior gives them as its scene: ``- Given the random bytes `fc` and `2a` `` or
  ``- Given the clock at `1735689600` ``. The tests then supply exactly those bytes and readings,
  and the proof covers every value they could take. When the user only cares that output falls
  in a range, the behavior says so with a rule: ``- Then it prints text that matches
  `rolled {any(1, 6)}` and a newline``.
- **Large inputs:** a program for data of any size says so in Purpose, such as "it reads standard
  input of any length, a piece at a time", and still gives the largest input the tests use in
  backticks (`1048576`): tests stay within a megabyte, and the program streams beyond it.
- **Limits that make sense:** a limit is what the program can really take, never a number for
  the tests' sake. A program that can stream has no limit on its total input. One that must hold
  all of it at once, such as a sort, states a generous limit, such as `1073741824` bytes: its
  memory costs nothing until it's used, and the tests and proofs still try smaller inputs, given
  in backticks of their own.
- **Programs built from programs:** a program can build on others, each a folder with its own
  idea.md and main.s, linked as a graph is: `- Builds on: [sort lines](../sort/idea.md)` under
  `# Constraints`, each part by a path relative to this idea. Build on a part when the program
  needs what another program already does, such as sorting, parsing numbers or a hash table,
  rather than doing it again. A part is a whole program, with its own behaviors and tests: a
  library, such as a hash table, is specified through a small program that uses it, so the same
  steps test it. Keep the parts in sibling folders, in one bundle. Each part is written,
  compiled and checked on its own, before the programs that build on it. A cycle of
  `- Builds on:` links, such as a part that builds on the program, blocks every program in it.
- **The graph:** program folders are the nodes of a knowledge graph in the Open Knowledge
  Format, and `check` keeps it. A program's bundle is the nearest folder above it whose
  `index.md` has `okf_version` in its frontmatter, or else the folder that holds the program.
  There `check` keeps `index.md`, with one entry per program: its title, its description and
  its code; and `log.md`, newest first, of what it recorded and checked, never the user's words.
  Don't write lengua's entries yourself; every other line of `index.md` is yours, and stays as
  it is. lengua writes neither file in the home folder, the temporary folder itself, or a file
  system's root. Link an idea to another it relates to with a plain markdown link in its text,
  as `[sort lines](../sort/idea.md)`, by a path relative to the idea, or from the bundle's root
  starting with `/`: these links and `- Builds on:` are the graph's edges, and `check` warns of
  one that leads nowhere. Any other page in the bundle needs a frontmatter `type`, as
  `type: note`, as every Open Knowledge Format page does.
- **Threads:** when the user wants a program that does heavy work on large inputs as fast as
  possible, add `- Threads: up to N` under `# Constraints`, with N up to the machine's cores
  (`check` reports them in `machine`; at most 64). The program may then split its work across
  that many cores. Behaviors don't change: the output is the same as on one thread.

## Write it in Simplified Technical English

Write the idea in ASD-STE100 Simplified Technical English (STE), the controlled English that ASD
made for technical text, in the style of behavior-driven development. Then a reader for whom
English is a second language, and an agent, read each sentence one way. Here are STE's rules in
lengua's words; [the STE reference](references/ste.md) has the full guide and the link to ASD's
free download of the standard.

- **Words:** simple, common words, each with one meaning, and the same word for the same thing
  every time. Technical names are words too: standard input, file, folder, argument, environment
  variable, exit code, connection, port, byte, newline.
- **Nouns:** at most three in a row. Write "the length of a line", not "line length limit
  value".
- **Verbs:** the simple present for what the program does ("it prints"), the simple past and
  future where you need them, and the imperative for an instruction. Use a past participle only
  as an adjective ("a closed output"), and no -ing form except in a technical name (string,
  padding, encoding, floating point). Use the active voice, and the passive only when nobody
  knows who acts.
- **Sentences:** one idea in each, at most 25 words in a description and 20 in an instruction.
  Keep the articles, verbs and subjects: never drop them to make a sentence shorter.
- **Paragraphs:** one topic in each, at most six sentences. Use a list for complex text.
- **Behaviors:** each is a `## B1:` title in the simple present ("Adds two numbers"), then its
  Given/When/Then steps, each a short STE sentence in lengua's step words.

`check` holds the idea to what a computer can see. It reads the description, the paragraphs of
`# Purpose`, the behaviors' titles and every step, but not code blocks, and a value in backticks
is one word there. A sentence has at most 25 words, and a paragraph at most six sentences, where
each list item is a paragraph of its own. A word of 5 letters or more that ends in "ing" is
refused, except lengua's technical names: string, strings, nothing, something, anything,
everything, during, spring, ceiling, morning, evening, building, heading, padding, encoding,
encodings, rounding, bring, thing, things, and "floating" before "point". A step may use none of
lengua's old phrasings. Each problem starts with `STE:` and says what to write; the other rules
are yours to keep.

The steps' STE words are ``standard input that contains `…` ``, ``a file `notes.txt` that
contains `…` ``, ``the environment variable `NAME` with the value `…` ``, ``it prints text that
matches `rule` ``, ``the file `out.txt` contains `…` ``, ``the client receives `…` ``, `it does
not stop` and ``no server on port `8080` ``. In a value's text, `and a newline` adds a line
break, `then` joins two parts, and `` `4097` times`` after a value repeats it. [The
reference](references/ste.md#the-step-words) gives the old phrasing that each one replaces.

Before:

```text
## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
```

After:

```text
## B2: Shows the usage when there are no arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
```

Before, in `# Purpose`:

```text
Reads standard input of any length, keeping only a piece of it in memory at a time, and if
writing the output fails, the program exits with code 2.
```

After:

```text
Reads standard input of any length, one piece at a time. If the program cannot write its
output, it exits with code 2.
```

## The tests

Under each behavior whose steps give the program something, write its generated tests: a `###`
heading such as `### For any two numbers`, then the behavior's steps again with `{placeholders}`
where values vary, then a table with a column for each variable, holding its generator, and a row
for each variant, as a Gherkin Scenario Outline has its Examples. Each run draws new values and
the formulas compute what the program must do. [The reference](references/test-format.md) lists
every phrasing, generator and function. Write them from the idea alone, never from code: they come
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
  with `fixed`: `{fixed(sqrt(x), 3)}`, or in singles when the idea says single precision:
  `{fixed(sqrt(single(x)), 3)}`. Write a multiply followed by an add, as in a dot product or a
  polynomial, as `fma(a, b, c)`: it rounds once, which is more accurate, and computers do it in
  one step. Write `2.0`, not `2`, next to decimals.
- **A computing behavior gets a row of its numbers alone.** The proof goes through text lengths
  one by one, so a row with several free texts proves only short ones. When a behavior computes
  with numbers among other text (a total of amounts between other fields, a date, a count), add a
  row where only the numbers vary, with each other variable a `choice(…)` of one or a few values.
  The proof then covers every number.
- **Small ranges are checked in full.** A row whose ranges hold at most 5000 combinations (and no
  `choice`) runs every one of them, so a range the idea states exactly is worth more than a wide
  one.
- **Joined text counts whole.** A template that adds text around a variable, such as `"{a}x"` or
  `"{word} {rest}"`, gives the program more than the variable alone: keep the total inside the
  idea's limits for behaviors about valid input.
- **Lengths count characters.** `string(0, 1048576, "aé€")` can reach 3 MiB, since `é` takes 2
  bytes and `€` 3, while ideas and programs count bytes. Draw long text from 1-byte (ASCII)
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
type: lengua/program-idea
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Lets a shell script add two whole numbers. It prints the sum of two numbers between `-1000000000`
and `1000000000`, which it gets as arguments. If the program cannot write its output, it exits
with code 2.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any two numbers
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Shows the usage when there are no arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
```

B2 gives the program nothing, so it needs no generated tests. Each of B1's runs draws a new `a`
and `b` and expects their sum. The first runs try the boundary values: the ends of the range, 0,
1 and -1.

## After recording

Every edit to `idea.md`, its tests included, is recorded by the next `check` under the words
already recorded, and the code is checked against it again: pass `request` again only with the
user's new words, when they ask for something new. When `check` reports a failing test, decide
from the idea whether the test or `main.s` is wrong; if the test is, fix it and call `check`,
with no one to ask. lengua shows every change after the user's request in its final reply
(`changes`), with its own marks on the changes that test less (`tests_less`), and keeps every
request of a build until its code passes, so nothing drops out: tell the user each change, and
why, in your final report. Never change the idea or weaken a test to make code pass; fix the
code.

A folder from an older lengua may still have a `tests.toml`: make sure each behavior's steps say
what its `[[example]]` said, write its `[[generated]]` tables as the behavior's generated tests,
their fields as steps and their `vars` and `runs` as rows of the table, then delete `tests.toml`.
