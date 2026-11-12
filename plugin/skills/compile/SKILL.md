---
name: compile
description: Compiles a lengua program's idea.md, an Open Knowledge Format Attested Computation, into main.wat, WebAssembly text in lengua's dialect that the lengua check tool builds, holds to the lengua Power of 10 (NASA's rules adapted for WebAssembly, with memory rules like Rust's), tests, proves and runs the same way on every computer. Use after check records the idea, when check says to write main.wat or reports problems in it, when someone shares an idea to build here, or when the user asks for a faster program. Ends when check says done, with a short report to the user. Also says when a module that a compiler made from Rust, C or another language may be the program instead, run under the idea's grants and marked trusted, not proven.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Compile a lengua idea

`main.wat` is the program: WebAssembly text that you compile from `idea.md`, one module that runs
the same way on every computer. You are the compiler, and lengua checks your translation instead
of trusting it: `check` parses `main.wat`, merges lengua's library into it (`$assert_fail`,
`$fill`, `$write_all`), validates the module with the engine's own features, holds it to
[the WebAssembly dialect](references/power-of-10-webassembly.md), runs every test in the idea, and
then proves each generated test in the background. Nothing runs before every rule passes. The
program reaches the computer only through [lengua's host](references/host.md), WASI preview 1 as
lengua implements it, and only through the calls its idea allows.
[Writing main.wat](references/wasm.md) shows how to write code that meets the rules on the first
check.
For an idea with a `- Secret:` line, write `main.wat` by [the secret rule](references/constant-time.md),
with masks where a branch would decide on a secret. The `machine` line names the CPU's caches, and
[caches.md](references/caches.md) says how to lay work out for them.

The idea is an Attested Computation (`type: Attested Computation`), and its code is `main.wat`
alone, by the steps below: `check` refuses any other code file and says what to do. The one
exception is [an outside module](#a-program-in-another-language), which the user must ask for.

## Steps

1. **Read the idea and the machine line.** In `<path>/idea.md`, the frontmatter's `parameters`
   say where each input comes from: an argument, standard input, an environment variable or a
   file. `# Purpose` gives every range and limit in backticks, which size the program's regions.
   Each behavior under `# Behaviors` has its example and its generated tests. `# Constraints`
   holds `- Uses:` and `- Network:`, which decide what main.wat may import
   ([the host](references/host.md) lists which line allows which call), and `- Builds on:` for
   parts. The `write_code` reply's `machine` line names this computer, the engine (Wasmtime,
   compiling main.wat with Cranelift for this CPU), how wide this CPU's vectors go, and what
   main.wat may use: WebAssembly 1.0 with sign-extension, saturating-float-to-int, multi-value,
   bulk-memory and simd, and no tables or references. The same main.wat runs on every computer:
   the machine line says how fast vector code will be here, never what the program does. A
   WebAssembly program is one thread, so `check` refuses an Attested Computation with a
   `- Threads:` line.
2. **Pick the template that fits**, and keep its shape. Each is a complete program that passes
   every rule and every test of its idea:

   | Template | Its idea | Start from it for |
   |---|---|---|
   | [negate](assets/main-template.wat) | [idea](assets/main-template-idea.md) | Arguments: a number parsed and printed, an error for each bad input |
   | [the streaming filter](assets/stream-template.wat) | [idea](assets/stream-template-wat-idea.md) | Standard input of any length, a 64 KiB buffer-full at a time, in constant memory |
   | [the vector template](assets/vector-template.wat) | [idea](assets/vector-template-wat-idea.md) | A line found and changed 16 bytes at a time with SIMD128, then a scalar tail |
   | [the event-loop server](assets/server-template.wat) | [idea](assets/server-template-wat-idea.md) | A server for several clients at once: `poll_oneoff`, `sock_accept`, `sock_recv`, `sock_send` |
   | [the word tally](assets/tally-template.wat) | [idea](assets/tally-template-wat-idea.md) | Counts or lookups by key: a hash table, a merge sort and buffered output, in their plain form, which step 7 makes fast |
   | [the client that follows](assets/client-template.wat) | [idea](assets/client-template-wat-idea.md) | A client that opens its own connections, with `tcp_open`, to the computers its idea grants: a request built from a name and a path, and a redirect followed |
   | [the copy](assets/files-template.wat) | [idea](assets/files-template-wat-idea.md) | Files the user names: each opened with `file_open` by the place of the argument that names it, one to read and one to write |
   | [the two programs](assets/program-template.wat) | [idea](assets/program-template-wat-idea.md) | Programs started with `program_run`: a command, its output, its exit code, and a program the computer does not have |
   | [the key](assets/terminal-template.wat) | [idea](assets/terminal-template-wat-idea.md) | The terminal: keys as they are pressed with `term_raw`, put back, and the same program with no terminal |

   A program that needs threads has no WebAssembly form: tell the user so rather than promise
   one.
3. **Plan the functions** before you write them: at most 180 operators each (rule 4), and at
   least two assertions per function on average. `$main`, exported as `_start`, checks the inputs
   and calls the functions that do the work; it never reaches its end: each path ends in
   `$proc_exit` or `$assert_fail` followed by `unreachable`, and a server's one loop that takes
   connections goes on forever. When the idea builds on parts (`- Builds on:`), call
   `check` on each part's folder until it is done first. Then take the functions you use from the
   part's `main.wat` word for word, each with a `;; from: ../sort` line above its `(func`: `check`
   holds each to the part's current code, and says which to copy again when the part changes. A
   part must be in WebAssembly too. Never rewrite what a part already does.
4. **Write `<path>/main.wat` to the dialect**, in the template's layout: the imports, the memory,
   the named constants, the regions, then the functions. [Writing main.wat](references/wasm.md)
   gives each idiom with its snippet, [the dialect](references/power-of-10-webassembly.md) the
   rules, and [the host](references/host.md) the calls, the descriptors and the error numbers.
   What first checks most often find:
   - an import the idea doesn't allow or the code never calls; each import has WASI's own type;
   - a memory whose minimum isn't its maximum, or isn't exported as `memory`; no `memory.grow`;
   - a bare number where a call, a comparison or a loop needs one: each is a named immutable
     global, and an exit code goes from 0 to 255, never 134;
   - a region below address 1024, or without `$x.size` (`$x.rosize` for read-only data); size
     each from the idea's limits, with one byte past a limit to see input that is too long;
   - a host call whose error number isn't tested at once, before anything it wrote is read, or a
     count it gave that isn't bounded before its use;
   - a loop without its counter: set it from a named constant right before the loop, write
     `;; bound: N` on the loop's line, and end the loop by taking one from the counter and
     branching back while it isn't zero;
   - an index not checked, unsigned, against its region's size less the access's width, before
     the access and inside the loop;
   - an assertion number outside 1 to 899, or used twice;
   - recursion, `return`, a table, a trapping float conversion (use `trunc_sat`), or a division
     whose divisor no check keeps from zero.

   Write it correct first; speed comes in step 7.
5. **Call `check` with `path` and read its reply.** `state` says where the program stands, and
   `next_step` says the one thing to do. Make one call at a time: each replies within about 40
   seconds, and the tests and the proofs go on in the background.

   | `state` | What it means, and what to do |
   |---|---|
   | `write_code` | The idea is recorded and there's no main.wat yet: write it. |
   | `code_problems` | Every problem, each on its line: one the build finds as `main.wat line N: …`, text that doesn't parse or validate, and each broken rule as `rule N (name) main.wat:LINE: problem. Fix: …` (`rule M2 (bounds)` for a memory rule), or as `dialect main.wat:LINE: …` for what the dialect doesn't allow, such as a trapping float conversion, the bits of a float that may be NaN, relaxed SIMD or a division by a value that may be zero; or a part's function that differs from the part's, or a second code file. At most 20 broken rules are listed, then `…and N more`. Fix them all, then call `check` again. |
   | `testing` | The tests are running: call `check` again; nothing needs to change. |
   | `tests_failed` | Each failure gives the test, its inputs and the seed, what was expected and what happened, as `example B1 (idea.md line 20): stdout should be "5\n", but was "-1\n"`, with `assertion 9 failed at main.wat:LINE` when an assertion fires. Decide from `idea.md` whether main.wat or the test is wrong: fix main.wat, or fix the test by the write-idea skill's rules, and call `check` again. |
   | `proving` | Every test passed, and the proofs are running: call `check` again. |
   | `done` | Every rule holds and every test passed: the reply has `tests`, `proofs`, `memory`, `example`, `performance`, `shell`, and `changes` when the idea changed after the user's request. |

   Boundary values run first on every check; the other generated inputs are new each time. Keep
   going until every test passes: never stop to ask the user, and never weaken a test to make
   code pass. A proof can also find inputs the tests didn't try: the state is then
   `tests_failed`, with a line such as `generated B1: the proof found inputs the tests didn't try
   where main.wat is wrong: with a = …`, and `run` refuses that build from then on, even after
   lengua restarts. Fix main.wat for those inputs. Any other state is the idea's: follow
   `next_step` with the write-idea skill. The reply's `graph` lines say how the program's bundle
   stands: fix what `blocks` first.
6. **Read the proofs and the memory line.** Once the tests pass, lengua proves each generated
   test for every input it can, with each solver question confirmed by z3 and cvc5: for about 20
   seconds before `check` says done, then on in the background for up to five minutes in all. A
   proof that finds main.wat wrong goes into lengua.lock at once; what the others proved goes in
   together once the last one ends. Each line of `proofs` is one generated test:
   - `proven for every input in range (… input combinations, … paths through the WebAssembly)`:
     every input its ranges allow. The rest of the line says whether two solvers confirmed each
     step, the terms settled every one, or one solver answered alone, and then which to install.
   - `proven: the tests tried every value …`: a range small enough that the tests ran it all.
   - `proven for every input with text up to N of M bytes`: a text proven as far as the proof's
     time reached, and tested beyond, as the line says. When it ends with
     `check with prove_further to prove on from there`, a `check` with `prove_further: true`
     proves on, for another stretch in the background. The WebAssembly prover proves a text one
     length after another, so a stream is proven up to a length, unless it is proven for every
     length by induction over its read loop through `$fill`, as the next line says.
   - `…; every length by induction over its read loop: …`: a stream whose read loop calls
     `$fill`, proven for every length: from any state the loop can be in at its head, one more
     buffer-full keeps what it writes and counts in step with the formula. It takes output that
     is the text mapped byte by byte (`upper`, `lower`, `tr`), or counts written at the end:
     `len`, `words` and `count` of the text or of the text mapped so, and sums of them, counted
     a byte or 16 bytes a step. Its base is every final piece of up to a buffer-full, one length
     at a time, so the buffer's size decides whether it ends in the proof's time: a buffer of
     4,096 bytes is proven for every length in minutes, and one of 64 KiB only up to a few
     thousand bytes. Where the user wants the proof for every length more than speed, take the
     smaller buffer, and tell them which they got.
   - `tested, not proven: <why>`: the tests passed, and the proof couldn't settle it. lengua's
     WebAssembly prover follows scalar floats and SIMD128's integer lanes, but not yet SIMD128's
     float lanes (`f64x2.add`, …), its narrowing, widening, pairwise and dot-product forms,
     `i8x16.popcnt`, or a load or store of less than a whole vector, and names the first such
     instruction it met, as ``main.wat line 89: `F64x2Add`, which the WebAssembly model doesn't
     follow yet``. It doesn't follow a name the input chooses, a server's wait on `poll_oneoff`
     for its connections, or a run longer than its step limit either. Tell the user why in one
     line.
   - `proving, …` or `waiting for its turn`: still running. Don't wait for it, but call `check`
     again before the user relies on the program: a proof that finds the program wrong shows
     there, and `run` refuses that build from then on.
   - `…, as lengua.lock records`: a proof of this very module, taken from the lock.

   `memory` says what's known of memory safety. The rules (M1 to M8) hold every address they can
   follow to its owner, for every input, and WebAssembly holds every access inside the program's
   own memory. What the rules can't follow, such as an address loaded from memory or a signed
   division that may divide the most negative number by -1 (a divisor that may be zero is a code
   problem, as step 4 says), they leave to the proofs, each on its line of main.wat, and the line
   says what the proofs settled of it on the runs they cover. The proofs follow no region, so
   whether such an access stays inside its own region is tested, not proven. A proof that found
   main.wat wrong comes first.

   At done, `check` writes lengua's entry in idea.md's `verified` list: lengua and its version,
   the time, and the hashes of the idea and the main.wat it checked. Never write or edit it. The
   next `check` after an edit to either file takes it away until the program is done again, and a
   proof that finds main.wat wrong takes it away at once.
7. **Make it fast.** When the program loops over its input, make one pass for speed as soon as
   every test passes, then call `check` again: the tests must stay green. Editing main.wat starts
   its proofs over, so make this pass before waiting for them. `performance` gives each test in
   a line: its runs and the WebAssembly operations each executed, the steadiest measure of work,
   the same on every computer; then the median time on the clock of its largest input, run alone
   five times. Starting a program costs time whatever it does, so compare the tests with the
   largest inputs. What makes main.wat fast:
   - **SIMD128 over bytes:** 16 bytes a step with `v128`, as
     [the vector template](assets/vector-template.wat) does. Spread each constant over the lanes
     once with `i8x16.splat`, test a range with one subtract and one unsigned compare, change the
     lanes through masks, and find a byte with `i8x16.eq`, `i8x16.bitmask` and `i32.ctz`. Mask
     the lanes past the input's end so the last step is the same code, or finish with a scalar
     loop of at most 15 bytes. The engine compiles each vector instruction for this CPU's
     vectors. To count bytes, add each compare's lanes, shifted down by 7, into counters of a
     byte, and widen them before 255 steps, as [writing main.wat](references/wasm.md) shows: no
     bitmask, which ARM64 has no one instruction for.
   - **Where words start and end:** 64 bytes a step, four vectors' `i8x16.bitmask`s joined into
     one `i64`, the lanes past the input's end set. The bits that differ from the bit before,
     `spaces ^ ((spaces << 1) | before)` with `before` the last bit of the step before, are the
     edges: take each with `i64.ctz` and drop it with `changes & (changes - 1)`, in a counted
     loop of 64, into a list of places that a second loop counts the words from.
   - **Large pieces:** read through `$fill` into a 64 KiB buffer, and write large pieces with
     `$write_all`, never a line or a byte per call: each host call costs. On a local disk a read
     of a file gives everything up to its end, so a file shorter than the buffer is one
     `fd_read`, where `$fill` takes a second read to find the end. On a network or FUSE mount a
     read may give less, so a program that must be right there reads through `$fill`, as
     [the host](references/host.md) says.
   - **Lookups by byte:** take each byte with `i32.load8_u` at its offset from one address,
     rather than shifting and masking a word, and lend a table's region to the function whose
     loop uses it, as an address in a parameter, so the address stays in a register.
   - **A short key as one number:** a key of up to 8 bytes is one `i64.load` with the bytes past
     its end cleared, `ALL_BITS >> ((8 - length) << 3)`, so its region keeps 8 bytes of room past
     any key's start. Its slot is the top bits of the key times `0x9e3779b97f4a7c15`, and an
     entry that keeps the key and its length matches it with two compares, with no loop and no
     call; a longer key, a slot another key holds and a new key take a slower path.
   - **Sorting by numbers:** give each item an `i64`, its sort key above its index, so one
     unsigned compare orders two (for lines, 4 bytes from a depth, the first the highest, those
     past the line's end cleared, and how many are the line's). Split with no branch on an item:
     swap each with the first place not yet known to be below the pivot, then add the compare's
     0 or 1 to that place. A multikey quicksort keeps its parts on a stack in a region, the
     largest pushed first so the stack stays near 2 log2(n) parts, sorts on the part the same as
     the pivot by the key of its next bytes, and hands a part split about 2 log2(n) times below
     or above a pivot to a heap sort, so no input takes it to n squared steps.
     [Writing main.wat](references/wasm.md) gives the code of these, of the short key and of the
     word edges above.
   - **Input of any length streams:** fill a buffer, process it, keep only what crosses into the
     next buffer-full (such as whether its last byte ended a word), and end at a count below the
     buffer's size, as [the streaming filter](assets/stream-template.wat) does.
   - **Break dependency chains:** where each step waits for the one before, as in a checksum or
     a running sum, run two to four independent chains side by side and join them at the end. A
     checksum by tables takes 16 bytes a step through 16 tables, the bytes' shares xored
     together before the share that waits on the checksum joins them.
   - **Many cases on one small number:** one `br_table`, whose default leads to an assertion or
     an error, not a chain of compares.
   - **Memory the program touches** costs time too: work in place where the rules allow.

   Use only the deterministic instructions: no relaxed SIMD, and no fused multiply-add, which
   core WebAssembly doesn't have. Code on float lanes, and lanes narrowed, widened or added
   pairwise, stays tested, not proven, until the prover follows it: where the user wants a proof
   more than speed, keep scalar or integer-lane code, and tell them which they got.
8. **Tell the user**, from the `done` reply, in a few short lines and plain words: where the
   program is; what its idea promises, in two or three lines (its behaviors, its limits such as
   input sizes and ranges, and what it may touch: files, ports); that it follows every rule and
   passed all its test runs; which behaviors are proven for every input, which up to a length,
   such as "proven for every input up to 4889 of 1048576 bytes, tested beyond; I can prove on",
   which are tested, not proven, and why in a line, and which proofs are still running; what
   `memory` says, in one line; what the idea's first example did (`example`); that they can ask
   you to run it on their own inputs or files with `run`, or to change what it does; and the
   command that runs it from their own terminal, scripts and services, `shell`, as it is,
   followed by the program's arguments. A server isn't running after `check`: offer to start it
   with `run`, which returns its address. If the reply has `changes`, say in a line each what
   changed after their request and why, and name every change that tests less; every request it
   lists must be the user's own words. If the code handles a case the idea doesn't mention, such
   as input longer than a buffer, say so in one line and offer to add it to the idea. When
   `tests` says a generated test tried every value in its range, say that behavior was checked
   for every input in its range. Leave out hashes and seeds.

## Running it

`run` runs the checked program with `path`, plus `args` and `stdin` when it reads them, within 10
seconds, or within the `timeout_seconds` you pass, up to 40. A program that works on files runs
in a folder of the user's: pass it as `folder` and name the files in `args` relative to it; the
program can't touch anything outside it. `stdin_file` names a file in `folder` to give as
standard input, up to 16 MiB, and `env` gives environment variables to a program
whose idea uses the environment, and to no other. A server starts in the background on its
idea's port, and `run` returns its address: try it with your own tools, such as `curl` or `nc`,
then call `run` with `stop: true` to stop it and see what it printed. A program that connects
gets its connections from lengua, made before it starts to the places its idea names, and reaches
nothing else; `run`'s report says which it made, and why one couldn't be. A program whose idea
grants it connections of its own reaches the real computers in a run: `run`'s `may_reach`, as
`check`'s, says what the idea grants, in words. Tell the user those before the first run. The
report's `connections` then lists what the program asked to open, and why lengua refused one or
couldn't make it.

From a shell, `lengua-mcp run <folder> [arguments…]` runs the checked program with the shell's
own standard streams and no time limit, in the current folder or the one `--in <folder>` names,
with only the environment variables each `--env NAME=value` gives; an argument that starts with
`-` goes after `--`. It exits with the program's exit code, with 134 when the program traps, and
with 125, saying why, when it won't run the folder. The `done` reply's `shell` is this command
for this computer. `lengua-mcp check <folder>` is the idea's attester: it builds, checks, tests
and proves a folder with no lock and no key, writes nothing, and ends with `attested` or
`not attested`.

## A program in another language

`main.wat` is the default, because only it is checked against the rules and proven. When the
user asks for existing code to run, or for a program in a language they name (Rust, C, another
that compiles to WebAssembly), the idea's program may be an outside module instead: `main.wasm`,
which that language's compiler makes for `wasm32-wasip1`. Never choose it by yourself, and never
to get past a rule or a proof that `main.wat` fails.

- The idea says so twice: `computation: main.wasm` in its frontmatter, and a line
  `- Module: main.wasm, built from …` in `# Constraints`, with the language, the source and the
  compiler. Its tests and grants are written as for any idea. It has no `- Secret:` line and no
  `- Builds on:` line.
- You build `main.wasm` with the compiler, with a stated maximum for its memory, and put it in
  the folder, with no `main.wat` there. `check` reads it, refuses what lengua's host can't hold
  by itself, and runs the idea's tests against it.
- `check` then says `trusted_not_proven`, never `done`: lengua's host holds the module to the
  idea's grants and to its own memory, as it holds `main.wat`, but no rule of the dialect is
  checked, nothing is proven, and lengua writes no entry in `verified`. The reply's `outside`
  lines say each thing that does not hold. `run` and a shell's run say `trusted, not proven`
  each time.
- Tell the user so, before you build it and in your report: what its idea grants, that lengua
  holds it to that, and that it is tested, not checked and not proven.

[An outside module](references/outside.md) has the build commands for Rust and C, what `check`
refuses, a whole idea, and the report to give.

## Shared ideas

People share ideas, not code: the idea is the source of truth, and main.wat is disposable. Every
computer builds, checks and runs the same main.wat the same way, so an idea someone shares
compiles here like any other, and whatever must be the same everywhere belongs in the idea. A
main.wat that comes along in a copied folder is checked from scratch before `run` runs it: call
`check` on the folder. lengua's entry in `verified` records which idea and which main.wat lengua
checked, and `lengua-mcp check` lets anyone check that on their own computer; lengua's guarantees
hold for the programs that `run` runs after `check`.
