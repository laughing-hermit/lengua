---
type: note
title: W22, scale
description: Whether an agent can write, check and change a lengua program of several thousand lines, and what it cost.
---
# W22: scale

The question (the owner, 2026-11-12): can an agent build one real program of several thousand
lines of `main.wat` the product's way, and then change it, with `check` green? One agent did
it in one session on 2026-11-12, and recorded every check. [log.md](log.md) is that record,
cycle by cycle. This page is the summary, the measures, and the verdict.

## What was built

A web server, [examples/httpd](../../examples/httpd/idea.md), on four parts: two written for
it, [json](../../examples/json/idea.md) and [http](../../examples/http/idea.md), and the
library's decimal and crc32. It listens on a granted port for up to 8 clients at once, reads a
request's line and fields with limits, and has these routes: a health route; files of the
folder `public` with their types, an ETag, `If-None-Match` and one `Range`; the list of a
folder, sorted; a store of JSON values in a file, with GET, PUT and DELETE; HEAD on every
route that takes GET. It answers 400, 404, 405, 411, 413, 414, 416, 431, 500, 501, 505 and
507, and prints a line of a log for each request.

| Program | Lines of main.wat | Functions | Assertions | Lines of idea.md | Behaviors | Test runs a check |
|---|---|---|---|---|---|---|
| json (part) | 561 | 10 | 20 | 179 | 9 | 369 |
| http (part) | 930 | 20 | 54 | 261 | 13 | 463 |
| httpd | 3,638 | 85 | 199 | 594 | 24 | 290 |
| All three | 5,129 | 115 | 273 | 1,034 | 46 | 1,122 |

Of httpd's 3,638 lines, 930 are the 25 functions it takes from its parts, word for word, and
about 110 are the parts' constants. 60 functions, in 1,870 lines, and the constants, the
regions and the 54 literal texts are its own. So the agent wrote about 4,100 lines of
`main.wat` by hand and copied about 1,040. The largest program before this one had 544.

## What happened

The count of cycles, from [the log](log.md):

| | Cycles | Stopped by the idea | By a rule | By a failed test | By text that doesn't build |
|---|---|---|---|---|---|
| The build, to `done` | 5 | 3 | 1 | 0 | 0 |
| Change (a): HEAD on every route | 2 | 0 | 1 | 0 | 0 |
| Change (b): a path in UTF-8, a bug of the parser | 2, one shared with (a) | 0 | 0 | 0 | 0 |
| Change (c): two functions moved into the part http | 1 | 0 | 0 | 0 | 0 |

- json (561 lines) and http (691 lines as first written) were each `done` at the first check
  that read their code.
- httpd (3,610 lines as first written) broke one rule at its first check, rule 7, in one
  place, and was `done` at its second: every one of its 23 behaviors passed the first time
  the tests ran.
- No test failed in any cycle, and no text failed to parse or validate.
- The three cycles that the build lost were lost to the form of the ideas' tests: two
  sentences over 25 words; examples that their generated tests could not draw; placeholders
  with no text between them; too few runs to tell two outputs apart.

## What it cost

**Time.** The first dispatch was at 05:33 UTC and httpd was `done` at 08:42: 3 hours 9 minutes
for the build, in five cycles. The three changes were done by 10:38: 1 hour 56 minutes. A
cycle took 35 to 40 minutes, of which lengua's `check` took seconds: the runner builds
lengua's tests for about 17 minutes each time, and the check of the four parts takes three
minutes of proofs before the server is read. With the product on the agent's own computer the
same nine rounds of `check` are minutes, not hours.

**Lines touched by each change.**

| Change | idea.md | main.wat | Of which |
|---|---|---|---|
| (a) HEAD | 55 in, 10 out | 160 in, 112 out | 90 in and 90 out are the places of the texts after two that grew; 13 in and 5 out are the second cycle's fix |
| (b) the parser | http: 21 in, 4 out. httpd: 4 in, 3 out | http: 7 in, 9 out. httpd: 7 in, 9 out | the function and its constants copied again into httpd |
| (c) the refactor | http: 79 in | http: 244 in, 3 out. httpd: 136 in, 158 out | httpd's idea did not change |

**Text written.** The three `main.wat` files are 264,000 bytes and the three ideas 67,000.
The agent keeps no count of its tokens. At three to four bytes a token, the program's text is
about 70,000 to 90,000 tokens written once, and the changes about 10,000 more. Reading the
skills and their references first was about 2,300 lines.

## What broke first

**1. The form of an idea's tests, not the code.** Five of the nine things `check` refused
were in an idea. `check` reports an idea's problems in stages (first the sentences, then
whether each example is in its generated test's ranges, then the shape of the tests), so three
kinds of slip cost three cycles. Before the first dispatch of httpd the agent went through all
46 behaviors by hand after `check` named the first slip of a kind, and found eight more of
that kind. An idea of 594 lines with 24 behaviors, each written twice (the example, then the
generated steps), is where the agent lost track: the same request stands in up to four
places, and a change to one must be made in all.

**2. The agent's hold of the code held.** In about 4,100 lines written by hand: no name that
isn't defined, no local or label that isn't declared, no constant or local never read, no
assertion number used twice, no wrong place in the table of texts, no index outside its
region, no loop without its bound, and no wrong answer that a test caught. The two rule
violations:

| Rule | Where | What it was |
|---|---|---|
| 7, checked results | `$store_load`, cycle 4 | A real slip: the file was closed between `$fill` and the test of what it gave |
| 9, checked indexes | `$send`, change (a) | A limit of the rules: a length that is a parameter on one path and a number loaded from memory on the other can't be bounded by one check after the paths meet |

This did not come for nothing. The agent counted and checked with `grep` and `awk` before each
dispatch (names, counts of operators, shared regions, the places of the texts, the words of
each sentence), since a cycle cost half an hour. Those checks found nothing wrong with names,
and found two functions over the limit of 180 operators before any check ran. They are what
`check` would say in seconds. One of them broke the work once: `awk` dropped a text that was
one space while it re-laid the table, and the agent's comparison of the old and new lines
caught it.

**3. The proofs do not reach the server.** This is the largest gap that the test shows.

| Program | Generated tests | Proven for every input in range | Proven to a length | Tested, not proven |
|---|---|---|---|---|
| json | 10 | 6 | 0 | 4, out of 60 s |
| http | 14 | 12 | 2 (to 3 of 100 bytes, and to 15 of 40) | 0 |
| httpd | 25 | **0** | 0 | 25: `a server's wait on poll_oneoff for its connections` |

Every line of the server's own 1,870 lines of functions is tested and not proven, and so are
the parts' functions as the server calls them: a part's proof is of the part's own program.
What the rules prove of the server is memory: `the rules follow every address and division of
main.wat, so they leave nothing to the proofs`. So at this size lengua gives a server memory
safety for every input and its behavior for the inputs of 290 test runs. Change (c) shows the
only way there is today to get more: move a function into a part whose own program reads
standard input, and its tests are proven there. The reading of a Range field went from tested
to proven for every input in range that way.

**4. The rules.**

- **Rule 4, 180 operators a function, bites first and most.** `$json_minify` came to about 260
  as first written and http's `$main` to about 225; both were split before any check. Every
  `end`, `else`, `local.get` and `global.get` is an operator, so a function with five checked
  stores is already half full. Nothing in the skills lets an agent count before `check`
  counts. The largest functions that stand are at 165 (`$type_of`, `$json_structure`).
- **Rule 5, two assertions a function on average,** shaped the code: a small helper with no
  assertion costs two elsewhere, so the agent wrote fewer and larger functions, and put text
  tests inline that it would have named.
- **Rule 6, one owner a region:** 23 of the server's 31 writable regions carry `;; shared:`.
  At this size the mark is the rule: a program that keeps a request between ten functions has
  no other way.
- **Rule 2, bounded loops, and the memory rules** were never broken. They cost in shape: every
  copy between two regions is a loop of its own with two checks a byte, since a function lent
  an address reaches a fixed number of bytes from it. The server has nine such copy loops that
  a language with a length in its types would write once.
- **`check`'s time did not grow** in a way that matters: 0.1 s for the rules over 3,610 lines,
  3.2 to 3.8 s with 254 to 290 test runs. http, a quarter of the size, takes 20 s to its first
  reply and 60 s in all, because its proofs run and the server's don't.

**5. The parts mechanism held, with four costs.**

- The copies held: 25 functions and their constants, copied by line numbers with `sed`, passed
  the copy check every time, through one function changed (b), two added and one dropped (c).
- A part and the programs that copy from it must change in one step. The agent pushed a
  changed part with the server's old copy once, and the next check of the server could only
  have been refused.
- A part brings no data, so the server's 54 texts, the fields' names padded to 32 bytes, and
  the table of types are the server's own; and a part's function can't be lent one small text
  after another, since it reaches its whole limit from every address.
- The four parts are checked again before each check of the server: 181 s, of which 180 are
  three parts' 60 s of proofs. A lock that keeps a part's proofs would take that away.

**6. Text laid out by hand.** The places of the 54 literal texts are numbers written in the
code. A change to one text moves every text after it: change (a) made two texts six bytes
longer and touched 180 lines for it. This is the one place where the agent would not have
worked without a tool, and where WebAssembly text is plainly too low a level: a name for a
text's place and its length, computed by `check`, is all it needs.

## Where the skills were silent

Each of these the agent learned from lengua's source or from a refused check. An agent with
the plugin alone would have guessed. (A sub-agent of the session read the source for these
answers before the idea was written; it wrote no part of the program.)

| What a program of this size needs to know | Where it is today |
|---|---|
| An example must be drawable by its generated test: every value of it inside the generators' ranges and letters | `check`'s message only |
| Two placeholders need fixed text between them in what a test gives the program | `check`'s message only |
| With few runs, a row of a `choice` and a string can draw the string's first boundary value every time, and then "all runs expect the same output" | `check`'s message only |
| `check` reports an idea's problems a kind at a time, not all at once as write-idea says | found by losing three cycles |
| What counts as one of rule 4's 180 operators, and a way to count before `check` does | the source |
| A number that comes from two paths can't be bounded by one check after they meet | `check`'s message, which names the line and not the cause |
| The layouts that the host writes: an entry of `fd_readdir`, the 64 bytes of `fd_filestat_get`, the flags and rights of `path_open` | the source; host.md names the calls only |
| How to build an answer from many literal texts, given a part's fixed reach and a read-only region's exact size | nowhere; no template builds text from pieces |
| How to lay out a program of 60 functions: what state goes in shared regions, how a request passes between functions | nowhere; the largest template has 591 lines |
| That a test of a server has 2 seconds for all its clients, and compares standard output | test-format.md has the second; the source has the first |

## What was fixed in lengua, and what was not

Fixed on the branch:
- `tests/check_folder.rs` prints how long a folder's parts, its first reply and all of its
  check took, which is where this page's times are from.
- `tests/mutation.rs` takes json, http and httpd (the result is below).
- `examples/index.md` lists the three programs.

Not built in the test, on purpose, since each is more than a small fix; each has its evidence
above. The first, the second and the fourth were built after it (the next section):
- **A count of operators and an idea-only pass in `check`'s reply,** so that an agent sees how
  full each function is and every problem of an idea at once.
- **Names for data:** a text's place and length computed by `check` in place of numbers in
  the code.
- **Proofs that reach a server,** or a part's proof carried into the program that takes its
  function.
- **The skills' missing pages:** the ten rows of the table above.


## What was fixed since

After the test, on 2026-11-12, the conditions that live in `check`'s reply and in the skills
were taken away ([todo.md](../../todo.md) has the runs and the decisions). The numbers above
are the test's own and stay as they were.

| What the test lacked | What there is now |
|---|---|
| `check` reported an idea's problems a kind at a time: three cycles | One `check` reports the sentences' problems and the tests' problems together, in the order of the idea's lines. Only a problem of the idea's own form (frontmatter, a missing section, a step that can't be read) still hides the tests' |
| No count of operators before `check` broke rule 4 | The reply has `functions`, whether or not the rules pass: the largest function as rule 4 counts it, the average, each function with 135 operators or more by name, and rule 5's assertions a function |
| Rule 9's message named the line and not the cause | When the length comes from more than one path, the message says so, names the paths' values (the parameter, a value loaded from memory), and says to check on each path |
| The places of 54 texts computed with `awk`; one changed text moved 90 numbers | A string of a `data` segment marked `;; text: NAME` is a named text. `check` computes its address, its offset in its region and its length, holds each number marked `;; at:`, `;; offset:` or `;; len:` to it, says the right number when one differs, and gives the table of all in `data`. One segment holds all the strings, so a text has no address to write |
| Ten things the skills didn't say | Each is in its page: test-format.md (an example its test can draw, text between placeholders, runs and boundary values, a server's 2 seconds), power-of-10-webassembly.md (how rule 4 counts, a number from two paths), host.md (the layouts of `path_open`, a filestat and an entry of `fd_readdir`), and two new pages, texts.md and large-programs.md |

`examples/httpd` now names its 54 texts: they are one `data` segment, one string a line, and
each offset and the region's size carry their text's name. The memory is the same, and the file
has 3,639 lines. Change (a) made again on it, a text that grows by 9 bytes, is one edit of the
text and then the numbers `check` lists: 44 offsets and the region's size, each with its new
value (`tests/wasm_scale.rs`).

Not fixed, and still the conditions that matter most: **the proofs do not reach a server**, and
**`check` links no modules**. Other agents work on tools and on the run path; neither is part
of this.

## The mutation test

The three programs are in `tests/mutation.rs`: json and http with the proven programs, httpd
with the programs that are tested and not proven, where a mutant that comes out `done` has
survived. The test takes 8 mutants of each program, spread evenly over its lines. Run
43197373671 (Linux on x86-64, at 734c729, 314 s):

| Program | Mutants | Failed a test | Broke a rule | Tested and not all proven | Survived |
|---|---|---|---|---|---|
| json | 8 | 7 | 0 | 1 | 0 |
| http | 8 | 5 | 2 | 1 | 0 |
| httpd | 8 | 4 | 4 | 0 | 0 |

No mutant survived, and none had to be named as one that changes nothing. It is feasible at
this size, and it is thin: 8 mutants over httpd's 3,638 lines is one in 455 lines, where the
add fixture has one in 20. The test's count a program is fixed; a count by size would say
more of a program this large and take longer.

## The verdict

**Passed with conditions.**

It passed: one agent built a real program of 3,638 lines of `main.wat`, 5,129 with its two
parts, from an idea of 24 behaviors; `check` said `done` at the fifth cycle; and the agent
then made a feature that cuts across, a fix in the parser, and a refactor into a part, in two
cycles, two and one, with `check` green after each. No test failed at any point, and the
rules found two things in 4,100 lines. The suite, which takes every program of `examples/`
from its request to `done`, is green on all five systems with the three programs in it (run
43209854658, at b649eab, with main's 77835f0 merged in): Linux on x86-64 and on ARM64, macOS
on ARM64, Windows on x86-64 and on ARM64. It is green on the five again at 94db5ed, with
main's b34d2eb merged in (run 43224575949).

The conditions, each from a number above:

1. **What is proven does not grow with the program.** None of the server's 25 generated tests
   is proven. At scale lengua today proves memory safety and tests behavior. The direction's
   claim of proofs holds for parts that read standard input, and for nothing built on
   `poll_oneoff`. This is the condition that matters most, and size did not cause it: the
   echo server of 344 lines has it too. Size makes it weigh 3,638 lines.
2. **The agent needed tools that the product doesn't give:** a count of operators, a check of
   names before the half-hour cycle, and the places of the literal texts. With `check` at
   hand in seconds the first two fall away. The third does not.
3. **The idea is the harder text to keep right at this size.** Three of the five cycles of the
   build went to the form of tests, and a behavior's request is written in up to four places.
4. **One agent, one session, 3,600 lines.** The whole program was in the agent's context when
   it changed it. Nothing here shows a second agent, a later session, or 30,000 lines.

**Which of the owner's three options, and why.** None of them as a rescue, since nothing
failed; and from the evidence, in this order:

- **Ideas in parts** is what worked, and should be carried on: it is the only place where
  the proofs live. Its limits are the next work: a part that brings its data, a contract with
  a size in it (so that one copy function serves every region), and a part's proof used where
  its function is taken.
- **Modules linked by `check`** would remove the 1,040 copied lines and the "move in one
  step" slip, and would let the lock keep a part's proofs (181 s before each check). It is
  worth building second: the copies held, so it saves cost and adds no safety.
- **A higher-level text** is not needed for the code: writing the WebAssembly was not what
  broke. It is needed for exactly one thing, data laid out by hand, and that is a small
  addition to the text `check` already reads (a named text with its place and length), not a
  new language.

So the direction holds at 3,600 lines, and what the next milestone should buy is proof, not
a new text: a server's behavior proven, or a part's proof carried, is what would make the
second program of this size worth more than its tests.
