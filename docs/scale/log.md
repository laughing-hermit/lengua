---
type: note
title: W22, the log
description: Every check cycle of the scale test, as it happened.
---
# W22: the log of the scale test

One agent (Claude, model Fable 5.2, in one interactive session on 2026-11-12) wrote one program
of several thousand lines of `main.wat` from an idea, and then changed it three times. This
file is what happened, in order. [README.md](README.md) has the summary and the verdict.

The agent's computer ran no build and no lengua: it edited files and ran git and gh. Each
`check` was a dispatch of `ci.yml` in its `ignored` mode on Linux on x86-64, with the test
`check_one_folder` and the folder in `check_folder`, which checks each part the folder builds on
first, and gives each folder 60 seconds for all its proofs. So a check cycle cost a build of
lengua on a runner before `check` said anything, and the agent put as much as it could into
each cycle. In the product, `check` answers in seconds: the count of cycles here is what
carries over, and the minutes are the runner's.

## What the agent read first

The two skills as shipped, and nothing of lengua's source for how to write code:
`plugin/skills/write-idea` (SKILL.md, references/test-format.md, parts.md) and
`plugin/skills/compile` (SKILL.md, references/wasm.md, power-of-10-webassembly.md, host.md,
parts.md, the server template and its idea, the parts decimal, bytes and crc32). Where the
skills did not answer a question that a program of this size raises, the agent read lengua's
source for the answer, and each such question is listed under "Where the skills were silent"
in the README: each is a place where an agent with the plugin alone would have guessed.

## The program, and why this one

A web server, `examples/httpd`, built on four parts. lengua has a grant to serve (`- Network:
listens on TCP port N, up to M clients at once`), so the server is a real one: it takes its
connections with `poll_oneoff` and `sock_accept`, as the compile skill's event-loop template
does.

| Folder | What it is | Written for W22 |
|---|---|---|
| `examples/json` | A part: checks one JSON value and writes it with no space between its tokens | yes |
| `examples/http` | A part: the head of an HTTP/1.1 request: its end, its request line, its fields, its path decoded | yes |
| `examples/decimal` | The library's part: numbers read and written | no |
| `examples/crc32` | The library's part: the CRC-32, which is the server's ETag | no |
| `examples/httpd` | The server: health, files with types, ETags, parts of files and lists of folders, a store of JSON values in a file, the errors, the log | yes |

Decisions taken before the first line of code, each because of something the skills or
lengua's tests fix:

- **One request a connection, then close.** A test's client reads until the server closes.
- **The log goes to standard output, and every behavior states its lines.** A test compares
  standard output even when the behavior ends with `it does not stop`.
- **The ETag is the CRC-32 in decimal.** The tests' formulas have `crc32(s)`, and they have no
  date and no substring; `fd_filestat_get` gives no time. So `Last-Modified` was left out.
- **A part of a file is tested with a file of three pieces of five bytes.** A formula can't cut
  a string, so the test builds the file from the pieces it expects back.
- **`Content-Length` in a test's request is a fixed number.** A client's message holds bare
  variables only, so a PUT's body has a fixed length in every generated run.
- **No waiting close.** The server closes after its answer. A request that is refused before
  its body has come (413) is tested with a stated length and no body, so that the test's client
  has nothing left to send when the server closes.
- **All literal text in one read-only region, each text ended by a NUL.** A function lent an
  address reaches a fixed number of bytes from it, whatever length it is given, and a read-only
  region holds exactly its data. So a copy function can't be lent one small text after
  another: each would need the copy's whole reach after it. The server keeps its 54 texts in
  one region and passes their places.
- **Each part's function takes the text's start and a place in it**, never the address of the
  place, for the same reason.

## Before the first check

The agent wrote, in this order: json's code and idea; http's code and idea; httpd's idea (631
lines, 23 behaviors); httpd's code in four pieces (the constants and regions; the loop and the
request; the files; the store), 2,570 lines of its own; then the 24 functions it takes from
the four parts, copied with `sed` by their line numbers, each under its `;; from:` line.

`check` was 20 to 30 minutes away each time, so before each dispatch the agent ran checks of
its own with `grep` and `awk`, which stand in here for what `check` says in seconds:

| Check | What it found before cycle 1 |
|---|---|
| Parentheses that balance | nothing |
| A global read that isn't defined, a function called that isn't defined, a local or a label used that isn't declared | nothing, in 4,862 lines |
| A constant never read, a local never read | nothing |
| Operators a function, counted from the text | `$json_minify` as first written came to about 260, counted by hand before any tool: split in two (`$json_structure`). http's `$main` came to about 225: split in two (`$report`) |
| An assertion number used twice | nothing |
| The places of the 54 texts in `$texts` | computed with `awk` from the texts' lengths, not by hand |

The 180 operators a function are the rule the agent met first, twice, before any check ran.

## The check cycles

Each row is one dispatch. "Minutes" is the run from its dispatch to its end: about 17 of them
build lengua's tests on the runner, whatever is checked.

### Cycle 1 (run 43162164513, at 3a1574f): json and http as first written

| Folder | Lines of main.wat | What `check` said |
|---|---|---|
| json | 561 | `idea_problems`: one sentence of `# Purpose` has 26 words |
| http | 675 | `idea_problems`: one sentence of `# Purpose` has 28 words |

Neither main.wat was read: `check` stops at the idea. The agent had counted words while it
wrote and still missed two sentences. Fix: both split, and from here the agent counted each
sentence's words with `awk` before a dispatch, which found two more in httpd's idea. A run
for httpd dispatched beside this one (43164618516) was cancelled, since it would have stopped
at the same part. Cost: one cycle for two sentences.

### Cycle 2 (runs 43166298467 and 43166302283, at 5a2b0b5)

| Folder | Lines | What `check` said |
|---|---|---|
| json | 561 | **`done` at its first check of the code**: every rule holds, 369 test runs pass. No rule broken and no test failed in 561 lines written by hand |
| http | 691 | `idea_problems`: B1's example isn't in its generated test's ranges (`hello.txt` has letters the generator of `path` doesn't draw) |
| httpd | 3,610 | not reached: its part http isn't done |

json's proofs, with 60 seconds for all of them: 5 of its 10 generated tests proven for every
input in range (B3, B4, B5, B7, B8, B9: the values alone, the texts that are refused, 33
levels, input too long, output closed), each "settled by the terms themselves"; 4 tested, not
proven, out of time (B1 at 9 of 12 input combinations with one path cvc5 couldn't confirm in
time, B2 at 4 of 648, B6 at 9 of 12). The contract's nine clauses were what the agent had
written: the reach of each function was as it had reasoned.

`check` reported one problem of http's idea, and the second kind of problem (an example that
its generated test can't draw) only once the first kind (STE) was gone. The agent then held
every example of the three ideas to its generated test by hand and found eight more of the
same kind in httpd's idea: five examples that lacked the `?query` their generated steps have,
one whose two queries used letters outside the generator's, and a key, `color`, outside its
generator's letters. Without that pass, each could have cost a cycle.

### Cycle 3 (runs 43170808554 and 43170812099, at 5370ddf)

| Folder | Lines | What `check` said |
|---|---|---|
| http | 691 | **`done` at its first check of the code**: every rule holds, 278 test runs pass. First reply after 20.2 s, all of it with its proofs 60.2 s |
| httpd | 3,610 | `idea_problems`, three at once: B20's six runs all expect the same output; B6's file `{a}{b}{c}` can't be matched to its example (three placeholders with no text between them); B23's example has a `2` its generator doesn't draw (a slip of the agent's own fix in cycle 2) |

http's proofs in 60 seconds: 7 of its 9 generated tests proven for every input in range (the
few-texts row of B1, B3 to B8), "settled by the terms themselves"; B1's wide row proven for
texts up to 3 of 100 bytes (z3 and cvc5 both), and B2 up to 15 of 40 bytes. The contract's 12
clauses were what the agent had written. The four parts of httpd took 180.8 s to settle
before httpd's own idea was read.

Two of the three problems are rules of the test format that write-idea's reference states
only in part: that neighbouring placeholders need text between them is not in it, and neither
is how few runs can leave an output the same (six runs over a choice of six and a string drew
the string's first boundary value each time).

### Cycle 4 (run 43175796873, at 01d0b0b): httpd's code is read for the first time

| Folder | Lines | What `check` said |
|---|---|---|
| httpd | 3,610 (85 functions, 195 assertions) | `code_problems`, **one**: `rule 7 (checked results) main.wat:2245: $fill's error number isn't tested: it's kept in a local, and something else comes before the test` |

The idea was recorded. The rules ran over 3,610 lines in 0.1 s. In `$store_load` the agent
closed the file between `$fill` and the test of what `$fill` gave, so that a read that failed
would not leave the file open. Fix: the test comes first and sets a local, the file is closed,
and the local decides. Seven lines.

So the first full pass of the rules over the server found one slip in 2,570 lines of the
agent's own and none in the 24 functions taken from parts: no index out of its region, no loop
without its bound, no region shared without its mark, no bare number, no function over 180
operators. Nothing here is a slip that a compiler's type check would have caught: the text
parsed and validated at its first build.

### Cycle 5 (run 43180086861, at 5b4e6bd): httpd is done

| Folder | Lines | What `check` said |
|---|---|---|
| httpd | 3,617 (85 functions, 195 assertions) | **`done`**: `main.wat follows every rule and passed all 254 test runs`. First reply, with the tests, after 3.2 s; its four parts took 180.8 s before it, 60 s of proofs for each of three and less for decimal |

Every one of the 23 behaviors passed at the first run of the tests: no test of the server
failed once, in any cycle. The memory line: "the rules follow every address and division of
main.wat, so they leave nothing to the proofs".

The proofs: **none of the server's 24 generated tests is proven**. Each says `tested, not
proven: a server's wait on poll_oneoff for its connections`. What is proven is in the parts'
own programs, as the `parts` lines repeat: http's B1 (few texts) and B3 to B8 for every input
in range, B1 (wide) to 3 of 100 bytes and B2 to 14 of 40; json's B3, B4, B5, B7, B8 and B9
for every input in range and B1, B2 and B6 not; decimal's three; crc32 to one byte. And, as
parts.md says, a part's proof is of the part's own program, not of the server that took its
functions.

**The build, in numbers.** Five cycles from the first dispatch to `done`: three stopped at an
idea (cycles 1, 2 and 3: sentence lengths, examples outside their generators, a test's
shape), one at one rule (cycle 4), and the fifth passed. No cycle was spent on a failed test
or on text that didn't build. From the first dispatch (05:33 UTC) to `done` (08:40 UTC): 3
hours 7 minutes, nearly all of it the runners' builds; `check` itself took 0.1 s for the rules
and 3.2 s with the tests.

## The three changes

Each was made as a user would ask for it: the idea first, then the code, then `check`.

### Change (a): a feature that cuts across. "Answer HEAD on every route that takes GET"

What changed in the idea: one paragraph of `# Purpose`; the `Allow` field of four answers in
B10, and HEAD out of the methods that B10 refuses; one new behavior, B23, of six clients (the
health route, a file, a list, the store, a key, and a path that names nothing).

What changed in the code (33fd8ce, then eccb139):

| What | Lines of main.wat |
|---|---|
| The two `Allow` texts are six bytes longer each, so every text after them in `$texts` moves: 45 constants and 45 `data` lines get new numbers | 180 (90 out, 90 in) |
| The method HEAD, a flag for "no body" and the place where the head ends, kept in `$logged`; a new function `$head_end` at the nine places that end a head; `$send` and the four places that send a body from a region of their own; the log's length | 62 in, 12 out |
| Cycle 2's fix to `$send` | 13 in, 5 out |

- **Cycle a1 (run 43186041980, at 33fd8ce):** `code_problems`, one: `rule 9 (no indirection)
  main.wat:1202: the buffer $write_all borrows in argument 2 is as long as a number nothing
  checks, so it may reach past $out`. In `$send` the length was the parameter on one path and
  a number loaded from `$logged` on the other, and one check after the two paths met bounded
  neither for the rules. Fix: each path checks its own length and makes its own call.
- **Cycle a2 (run 43191117225, at c15e321, together with change (b)):** `done`, 290 test
  runs.

What else broke: nothing in the tests. Two things in the work itself:
- The re-laying of `$texts` was done with `awk` from the list of texts, and `awk` dropped the
  one text that is a single space. The agent's own comparison of the old and the new `data`
  lines showed it before the dispatch. By hand, 90 numbers would have been 90 chances.
- An answer to a request that `examine` refuses before the method is known (a head that is not
  valid, 431, 413) still has its body when the method was HEAD. The idea doesn't speak of it.

### Change (b): a bug in the middle of the parser. "A file whose name has a letter outside ASCII is refused"

No test found a bug in the parser, so none was fixed that a test had found. The agent read
`$http_path` again as a user would use it, and found one: a decoded byte had to be a character
that prints, from the space to the tilde, so `/files/caf%c3%a9.txt` got status 400 and a file
with such a name could never be served. The idea of the part said so too ("only characters
that print"), so the bug was in the idea first.

What changed: http's idea (one item of `# Purpose`, the texts B3 refuses, a new behavior B8
with a path in UTF-8) and three lines of `$http_path` and one constant fewer (7b37ee8: 28
lines in, 13 out). Then the server: the function and the part's constants copied again, the
idea's list of refused paths, and two targets more in B9, which now get 404 (c15e321: 11 in,
12 out).

- **Cycle b1 (run 43186160469, http at 7b37ee8):** `done`, 299 test runs; the new B8 proven
  for every input in range.
- **Cycle b2 (run 43191117225, httpd at c15e321):** `done`, 290 test runs.

What else broke: the order of the work. The part's change was committed and pushed while the
server still held the old copy of `$http_path`. The next dispatch of the server (43191063742,
for change (a)'s fix alone) would have been refused by the copy check, so it was cancelled
after a minute and the server's side of (b) went into the same run. A part and the programs
that copy from it have to move in one step, and nothing but the copy check says so.

### Change (c): a refactor. "Move the reading of numbers and of the Range field into the part http"

The server had two functions of its own that read the request's text: `$digits`, which read a
number with decimal's `$decimal_parse`, and `$range_read`, which read a Range field. Neither
could be tested or proven but through the server, whose tests are not proven. The refactor
moves both into the part http as `$http_number` and `$http_range`, where the part's own
program prints what they read and its proofs go through them.

What changed (12a8e4b: 459 lines in, 161 out, in three files):

| File | What |
|---|---|
| http/idea.md | A paragraph of `# Purpose`; four behaviors, B10 to B13 (a range, a range with no last byte, one with no first byte, a field that is not one range); two items of `# Contract`; `- Builds on:` decimal (79 lines in) |
| http/main.wat | The two functions and their 10 constants; in the part's own program, `$report_range` and `$number_out`, and decimal's `$decimal_format` taken to print the numbers (247 in, 8 out) |
| httpd/main.wat | `$digits` and `$range_read` out; `$http_number` and `$http_range` taken in; `$content_length`, `$same_tag` and `$range_of` call them; `$decimal_parse`, six of decimal's constants, six constants of the server's own and the region `$number` out (133 in, 153 out) |
| httpd/idea.md | nothing: no behavior of the server changed |

- **Cycle c1 (runs 43197275115 and 43197280226, at 12a8e4b):** http `done`, 463 test runs,
  its four new generated tests proven for every input in range; httpd `done`, 290 test runs.
  One cycle, nothing refused.

What else broke: nothing that a test sees. One thing no test sees: `$decimal_parse` took 19
digits and `$http_number` takes 18, so a `Content-Length` of 19 digits now gets status 400
where it got 413. A refactor changed an answer, and `check` stayed green, since the server's
idea gives lengths up to `999999` only.

The gain is what the refactor was for: the reading of a Range field went from "tested through
a server that is not proven" to "proven for every input in range" in the part's own program.

## The cycles, counted

| | Cycles | Stopped by the idea | By a rule | By a test | By text that doesn't build |
|---|---|---|---|---|---|
| The build, to `done` | 5 | 3 | 1 | 0 | 0 |
| Change (a), HEAD | 2 | 0 | 1 | 0 | 0 |
| Change (b), the path in UTF-8 | 1 (the part) and 1 shared with (a) | 0 | 0 | 0 | 0 |
| Change (c), the refactor | 1 | 0 | 0 | 0 | 0 |
| All | 9 rounds, 15 dispatches, two of them cancelled | 3 | 2 | 0 | 0 |

No cycle was lost to a slip that a compiler catches (a name that isn't defined, a type that
doesn't fit, text that doesn't parse), and none to a wrong answer of the program. Three were
lost to the form of an idea's tests, and two to rules: one real slip (rule 7), and one place
where the rules can't follow a number that comes from two paths (rule 9).
