# lengua's design

This file says how lengua works, as the code does it: each part, its limits, and what lengua
defends and trusts. Where the code and this file disagree, fix one of them. The finish line and
what is left are in [todo.md](todo.md); each milestone, with the runs that verify it, is in
[ROADMAP.md](ROADMAP.md). The native lengua, which compiled ideas to assembly for Linux on x86-64
and Linux on ARM64, is kept at the tag `native-final`.

## What lengua is

lengua is an AI language. A person's request becomes an idea file, `idea.md`: behaviors written as
Given/When/Then examples, generated tests that try fresh inputs, and what the program may reach.
The agent compiles the idea into `main.wat`, WebAssembly text. lengua doesn't trust that code: it
builds it, holds it to its dialect's rules, runs every test, proves the generated tests with two
solvers where it can, and records what it checked in a signed lock. It runs the program only as
that record says, and the program reaches only what its idea grants.

The product is an Agent Plugin: two skills, `write-idea` and `compile`, and one MCP server in Rust,
`lengua-mcp`, with two tools, `check` and `run`. It has no hooks and no subagents. The same module
runs the same way on the five runners lengua-mcp is built for: Linux on x86-64 and on ARM64,
macOS on ARM64, and Windows on x86-64 and on ARM64 (`xtask/targets.json`).

## A program, from request to run

A program is a folder:

| File | Written by | What it is |
|---|---|---|
| `idea.md` | the agent, with the write-idea skill | The source: an Open Knowledge Format page of type `Attested Computation`, with the behaviors, their tests and what the program may reach |
| `main.wat` | the agent, with the compile skill | WebAssembly text in lengua's dialect, which lengua validates instead of trusting |
| `lengua.lock` | lengua only | This computer's signed record: the user's words, the hashes of the idea, the code and the module, and what the proofs found |

At the root of the folder's bundle, `check` keeps `index.md` and `log.md`, the knowledge graph's
files. lengua's data folder holds the signing key, a copy of each idea.md it recorded, each module
whose tests passed, the engine's compiled modules (unless the agent's harness gives the plugin a
data folder of its own) and the graph's lock files (`src/data_dir.rs`).

The agent's loop is the server's instructions: write idea.md with write-idea; call `check` with
the folder's absolute path, the first time with the user's words as `request`; compile the idea
into main.wat with compile; call `check` after each edit and follow `next_step` until it says
done; never edit lengua.lock; run programs only with `run`.

`check` goes stage by stage and stops at the first that needs work:
1. The idea: idea.md parses, its tests check, it is an Attested Computation with no `- Threads:`
   line, its parts exist and no `- Builds on:` link closes a cycle. Then it is recorded in the
   lock with the user's words.
2. The code: main.wat builds, each function it takes from a part is the part's own, with the
   constants it reads ([Parts](#parts)), it follows every rule, and, for a part, its contract
   is what the rules find.
3. The tests, then the proofs, in a background job for the folder, keyed by the hashes of its
   files. A `check` on changed files cancels the old job.

Its reply's `state` is one of `write_idea`, `idea_problems`, `needs_request`, `write_code`,
`code_problems`, `testing`, `tests_failed`, `proving` and `done`, with one `next_step`; for an
outside module (below), `trusted_not_proven` stands where `done` would, and there is no
`proving`. A reply
comes within about 40 seconds, since agent harnesses cut off long tool calls; the tests and proofs
go on in the background, and the next `check` picks them up. Once the tests pass, `check` waits
at most 20 seconds more for the proofs, then says done and lists the proofs still running. All of
one build's proofs share 300 seconds (`src/server.rs`).

After done, three things reach the program: the `run` tool; `lengua-mcp run <folder>` from a
shell, the command the done reply gives; and `lengua-mcp check <folder>`, which attests the folder
on any computer with no lock and no key.

## idea.md

### The frontmatter

YAML between two `---` lines (`src/idea.rs`). lengua requires:
- `type: Attested Computation` (OKF v0.2 §10). An idea of the native lengua's type,
  `lengua/program-idea`, still parses, so that `check` can say lengua compiles an idea to main.wat
  since 0.2.0 and that the tag `native-final` keeps the native lengua.
- `title` and `description`, not empty. index.md lists each program with its description.
- `runtime: webassembly` and `computation: main.wat`.
- `executor`, whose `resource` is the URL of [docs/run.md](docs/run.md) and whose `receipt` is
  drawn from stdout, stderr, exit and files; and `attester`, whose `resource` is the URL of
  [docs/check.md](docs/check.md).
- `parameters`, if any: each with a `name`, a `type` (integer, decimal, text or bytes),
  `required`, and lengua's key `from` (an argument, standard input, an environment variable or a
  named file).

lengua validates the parameters and the receipt, but nothing reads them yet: `run`'s arguments and
the tests' comparisons don't use them. `status` may be draft, stable or deprecated. Every other
key, OKF's `tags`, `sources`, `generated` and `verified` among them, is kept and never refused.

### The body

- `# Purpose` says what the program does. The values in backticks there are the only numbers a
  generated test may use as the limit of a range.
- `# Behaviors` holds each `## B<n>: <title>`, n from 1 to 9999, with Given, When and Then steps.
  A behavior has exactly one When, which can't come after a Then. It ends with an exit code from
  0 to 255, never 134, which lengua keeps for failed assertions; a server's ends with a step that
  says it keeps serving.
- Under a behavior, one `###` section may hold its generated tests: the same steps with
  `{placeholders}`, then a table with a column for each variable's generator and a row for each
  set of runs.
- `# Constraints` holds what the program may reach:
  - `- Uses:` standard input, files, the clock, random bytes, the environment, the terminal, or
    nothing.
  - `- Network:` the places it connects to (a TCP port of this computer, of a named computer, over
    TLS to a named computer, or of the computer an argument names), at most 16; and the one TCP
    port it listens on, for one client at a time or for up to N at once; and, each on a line of
    its own, what it opens by itself while it runs (the host, below).
  - `- Files:` the files and folders the user names when the program runs, by the argument that
    names each: `reads the file its first argument names`, `writes`, `reads and writes`, `the
    folder its second argument names`, or `the files its arguments name` for every argument. At
    most 16.
  - `- Runs:` a program it may start, by its name in backticks, with the arguments the line
    bounds: each written in backticks, `a value` for one the program chooses, `up to N values`
    last, and `, within N seconds` for its time. Never a shell. At most 16.
  - `- Secret:` the inputs whose values must not decide how it runs: standard input, a named file,
    an environment variable or an argument.
  - `- Builds on:` links to the idea.md of each part it takes functions from.
  - `- Splits:` the work it may split into pieces that run at once, each a run of the program
    itself: how many pieces at most (2 to 64), how many bytes a piece gets at most, and how many
    bytes each gives back (1 to 65,536). One line (the host, below).
  - `- Threads:` still parses, so that `check` can refuse it: a program has no threads of its
    own, and uses more cores through `- Splits:`.

- `# Contract`, for a part: each function the idea offers to the programs that build on it,
  with what it reads and writes through the addresses it is lent ([Parts](#parts)). The idea
  parser leaves the section to `check`'s code stage, which holds it to main.wat.

### Wording

An idea names no computer. Outside backticks and code, its prose may not use a fixed list of
machine words, such as x86, aarch64, simd, syscall, linux and llvm, nor the phrases "system call"
and "file descriptor". Words with everyday meanings, such as windows and kernel, aren't on the
list.

Idea files are written in ASD-STE100 Simplified Technical English (`src/ste.rs`). `check` holds
the description, Purpose, the behaviors' titles and every step to what a computer can see without
STE's dictionary: at most 25 words a sentence and 6 sentences a paragraph, no -ing form but a
list of technical names, and the step vocabulary's STE forms, each of 12 old phrasings refused
with its rewrite. The rest of STE, its dictionary among it, is the write-idea skill's to teach.

### The tests inside

A behavior's steps are its example test, read through a fixed step vocabulary (`src/testfile/`):
arguments, standard input, files and folders, environment variables, clock readings, random bytes,
a server the program connects to, clients that connect to it, and standard output closed; then
what it prints, the files left, what a client receives, and the exit code. A step lengua can't read
is a problem, with a rewrite built from its own values.

A generated test's rows draw from generators: `int`, `decimal`, `choice` and `string`. A `runs`
column gives 1 to 1000 runs, 100 when empty. Boundary values run first on every check, and the
other inputs are drawn fresh from a seed each check draws. A block with no `choice` and at most
5,000 combinations runs every one instead, while the whole suite stays within 20,000 runs.
Expected results come from formulas: checked 128-bit whole numbers, exact decimals, doubles and
singles, and text, with 31 functions, from `len` to `tally`, `sha256`, `aes128`, `crc32` and
`fma` among them.

`check` holds the generated tests to their behavior before anything runs: the formulas must give
the example's results from its inputs, a behavior whose example gives the program anything must
have generated tests, ranges come from Purpose's numbers, and every variable must reach the
program. A dry run evaluates every run of every block, from a fixed seed hashed from the generated
sections. It covers its own draws, not the fresh ones a later check makes, so a formula's error
there shows as a failed test.

One `check` reports every problem that it finds without running code, in the order of the
idea's lines (`idea_findings`, `src/server.rs`). An idea whose problems are of its words alone,
a rule of STE or a word that names one kind of computer, still reads (`idea::read`), so its
tests are checked in the same pass, and with them its type, a `- Threads:` line, the parts it
links to and a cycle of those links. Only a problem of the idea's own form hides the tests'
problems, since the tests can't be read before it is fixed. The cap stays: 20 problems of the
idea and 20 of its tests, then one count of the rest.

## main.wat, lengua's library and the build

`main.wat` is parsed by `wast` 261.0.0, keeping each instruction's line (`src/wasm/text.rs`). A
component, or a module given as bytes, is refused, and so is any name that starts with `lengua.`.
Of lengua's library, the functions main.wat calls and doesn't define are merged in, with what they
need: `$assert_fail`, which prints `lengua: assertion N failed` and exits 134; `$fill`, which reads
until its buffer is full, the input ends or a stop byte comes; and `$write_all`, which writes every
byte (`src/wasm/library.wat`). Memory's bytes 0 to 15 are the library's while one of its calls
lasts. The merged module is validated by `wasmparser` 0.261.0 with exactly the engine's features,
so a module the validator takes is one the engine runs.

A build keeps, for each operator of main.wat's own functions, its offset in the module and its
line in main.wat, so a trap, the validator's complaint or a failed assertion names the line. The
module that runs carries no name section. A function written under a `;; from: <part>` line
must be the part's function word for word, comments and spacing aside ([Parts](#parts)).

`BUILDER` names a build: a hash of lengua's version, the build's source, the library and
Cargo.lock, which pins `wast` and the encoder exactly. `check` keeps each module whose tests passed
in the data folder under its builder and its SHA-256. It names a build and secures nothing: what
runs is held to the module hash in the signed lock.

`lengua-mcp wasm <main.wat> [-o <file>]` writes the module, with no rules, tests or lock: a plain
WASI preview 1 module, which any WASI runtime runs when it reaches only standard streams and files.

## An outside module

The owner decided on 2026-11-12 that the most useful thing lengua has is a safe way to run code an
agent wrote, and that it should hold for code in any language. So an idea's program may be
`main.wasm`, a WebAssembly module that a compiler made from Rust, C or another language, in place
of `main.wat` (`src/wasm/outside.rs`, `src/server/wat/outside.rs`). lengua runs it in the same
host under the same grants, and marks it "trusted, not proven" wherever it names it.

**The shape.** The folder holds `idea.md` and `main.wasm`, and no `main.wat`. The idea says what
it is twice, and `check` holds the two to each other: the frontmatter's `computation: main.wasm`,
and one line of `# Constraints`, `- Module: main.wasm, built from …`, with up to 200 characters
that say what the module was built from. Those words are the idea's own: lengua records and shows
them and can't check them. The line is in the idea's body, which lengua hashes and a run reads, so
which file is the program is part of what the lock records (`Snapshot::outside`, `src/state.rs`).
An idea with the line has no `- Secret:` line, since the secret rule reads main.wat, no
`- Splits:` line, since the rules that bound the pieces together read main.wat, and no
`- Builds on:` line, since a part gives functions of its main.wat.

**What `check` does.** After the idea stage, in place of the build and the rules, it reads
`main.wasm` and refuses, before anything of it is compiled or run:

- a file past 16 MiB;
- a module the validator doesn't take with exactly the engine's features (The engine, below): so
  no threads, tail calls, exceptions, 64-bit or multiple memories, GC, relaxed SIMD, wide
  arithmetic or components;
- an import that isn't a function, isn't one of WASI preview 1 or of lengua's own modules, or has
  another type; and a function of lengua's own modules that the idea doesn't grant. A function of
  WASI preview 1 that the idea doesn't grant links, since a toolchain's startup imports several
  whatever the program does, and the host refuses each call of it;
- a memory that isn't exported as `memory`, one that declares no maximum, and one that may hold
  more than 16,384 pages, main.wat's own most;
- a module with no `_start` that takes and gives nothing;
- `main.wat` beside it.

Then it runs the idea's tests against the module in the workers, as it runs main.wat's, keeps the
module and its compiled, sealed form, and stamps it. It checks no rule, starts no proof, and
writes no entry in idea.md's `verified`. Its last state is `trusted_not_proven`; it never says
`proving` or `done`. Every reply about the module carries `outside`: the trust and the idea's
words first, then a line for each guarantee that does not hold, then what the host holds. The
bundle's log records `checked: trusted, not proven`. The attester refuses the folder: it attests
main.wat alone.

**The lock** keeps its format. An outside module's code stamp has one hash twice, since the code
file is the module, which no main.wat's stamp has; its `engine` line ends in `; an outside module:
trusted, not proven`; and its stack and unsafe blocks are 0. A run refuses a folder whose idea, read
bytes and stamp don't all say the same kind of code.

**A run** makes every check a run makes, then says the trust before the program starts: on
standard error from a shell, and in the reply's `outside` from the `run` tool. The module that
runs is `main.wasm` itself, held to the SHA-256 the lock records; the `run` tool reads it again
as `check` did.

**What holds it is the host alone.** The host was written to answer whatever module comes: every
function checks the idea before it does anything, and every buffer against the program's memory
(The host, below). The review of an outside module
([docs/reviews/outside-2026-11-12.md](docs/reviews/outside-2026-11-12.md)) went through each
guarantee that leaned on a rule of main.wat's, and says what holds it now:

| The guarantee | For main.wat, also by | For an outside module, by |
|---|---|---|
| No call the idea doesn't grant | the checker, which refuses the import | the host, at each call (`answer`); and `check`'s reading, for lengua's own modules |
| No function of another host | the checker | `check`'s reading; the linker has no such function, so the module wouldn't start |
| Memory within a stated size, at most 1 GiB | the rule: one memory, minimum equal to maximum, at most 16,384 pages, no `memory.grow` | `check`'s reading (a maximum, at most 16,384 pages), and the store's limit at each `memory.grow` |
| The memory the host reads and writes is the program's own | the rule: exports are `_start` and `memory` | `check`'s reading (exported as `memory`); a module without it stops at its first call of the host |
| A buffer a call borrows lies inside the memory | the memory rules, and the sizes call before `args_get` | the host, which checks every buffer of every call |
| It ends | rule 2, bounded loops, and rule 1, no recursion | the engine's epochs at the deadline of a test and of the `run` tool; the engine's stack limit; nothing in a run from a shell, where the user stops it |
| Nothing runs before `_start` | the rule: no start function | the limits are set before the module is instantiated, so a start function is stopped as `_start` is |
| No table, no indirect call | rule 9 | the engine, which checks each call through a table, and the store's limit of 2^20 entries |
| The same bits everywhere | the dialect's determinism checks | the engine's canonical NaNs, and relaxed SIMD refused. lengua says nothing of an outside module's own use of the clock or random bytes |
| A secret decides nothing | the secret rule, and the machine code's check | nothing: the idea is refused |
| Each access inside its own object | the memory rules M1 to M8, and the proofs | nothing: the reply says so |

A run from a shell has no deadline for any program, by design: a server runs as long as its user
wants. main.wat ends by its rules; an outside module may not, and the reply and docs/run.md say
so.

## Parts

A part is a program whose idea offers some of its functions to other programs (W21;
`src/wasm/contract.rs`, `src/wasm/parts.rs`, `src/server/parts.rs`). It is a program folder like
any other, with its own behaviors, tests and proofs, and one more section in its idea.md:

```markdown
# Contract
- `$decimal_parse`: reads 20 bytes at `$at`; writes 8 bytes at `$into`. It gives 0, or 28 when
  the text is not a number.
```

An item is a function's name, then its clauses up to the first full stop: `reads N bytes at` a
parameter, `writes N bytes at` a parameter, or `uses no memory`. The words after the full stop
are the idea's own. The clauses are what the memory rules compute for every function anyway: how
far it reaches through each address a parameter lends it, and whether it writes there. `check`
holds the two to each other, and refuses a part whose contract says another parameter, another
number of bytes, or `reads` where main.wat writes; so does the attester. A contract offers only
a function a program can take: one the part defines itself, that names no region and no global
that changes, names functions and globals by `$name`, and calls no function of the part that the
contract doesn't offer too. Such a function's result depends only on its arguments, the bytes it
is lent and the host.

A program builds on a part by copying. Its idea names the part (`- Builds on:`), the part's
folder stands in the same bundle, and main.wat holds each function it takes under a
`;; from: <part>` line. At every check, before anything runs, `check` holds main.wat to its
parts, and refuses it with `code_problems` unless:
- each part is checked on this computer (its lock records its idea.md and its main.wat as they
  are), has a contract, and has no proof that found it wrong;
- each function under a `;; from:` line is the part's function of that name, word for word,
  comments and spacing aside, and the part's contract offers it;
- each constant the function reads is in main.wat exactly as the part defines it, each function
  of the part that it calls is taken from the part too, and each host function it calls is
  imported exactly as the part imports it;
- each `;; from:` line names a part the idea builds on, and main.wat takes something from every
  part the idea builds on.

The check is on main.wat's text, field by field, and runs again on every `check`, since a job on
the same two files may be older than a change to a part. A part that changed is no longer
checked, so the program is refused until the part is checked again and its functions copied
again.

**The mechanism, and why.** main.wat stays the one text lengua builds, checks, tests, proves and
runs: nothing is linked, and the module that runs is built from main.wat alone. So each
guarantee is stated once, of main.wat, as before: every rule, the memory rules and the secret
rule among them, goes through a taken function as through any other, and the tests and the
proofs run it as the program calls it. What a part adds is one statement: a function under a
`;; from:` line is the function the part's own tests and proofs ran, with the same constants
and the same callees. Linking modules would have needed a second artifact, a linker lengua
trusts, and rules that cross a module's edge (todo.md's decisions).

**The memory rules at the boundary.** A taken function is checked where it is called: the
caller's region must hold every byte the function may reach through the address it is lent
(M2), a read-only region can't be lent to a function that writes (M6), and no two arguments
name bytes one of which the function writes (M4). A contract's numbers are those same reaches,
so a program that lends less room than the contract states is refused, with the call's line.
What a function's other arguments must be, such as a length of at most 4096, the function
checks itself, with an assertion.

**The proofs.** A part's proofs are of the part's own program, for the inputs its idea gives.
The prover doesn't use them in a program built on the part: it follows each taken function
again, in the program's own module, for the program's generated tests. Using a part's contract
in place of its body would need the contract to state what the function computes, as a formula
the prover holds the body to once, for every argument and every memory it may be lent; the
contract states only what it reaches, and the part's proofs cover only the calls the part's
program makes. So a program's `proofs` say what holds of it, the taken functions included, and
its `parts` lines say what each part's own lock records, as evidence about the function, not as
a proof of the program.

lengua.lock records the parts with the code: each part's folder, lengua's hash of its idea.md,
the SHA-256 of its main.wat, and the functions taken. The record is of the parts as they were
when the program's tests passed, and a run holds the parts to it: `run`, `lengua-mcp run` and
lengua-run read each recorded part's idea.md and main.wat and refuse the program when one
changed or is gone. `check` then says what to do, and once the part is checked again it
stamps the program again with the part as it is.

**The library.** lengua ships five parts, in the compile skill's `assets/parts`, and holds
the same files in `examples/`, where the suite and the golden ledger take them with every other
program: `decimal` (`$decimal_parse`, `$decimal_format`), `lines` (`$lines_read`, `$lines_end`),
`crc32` (`$crc32_update`), `bytes` (`$bytes_equal`) and `sort64` (`$sort64`). lengua-mcp carries
their text, so `check` says of a part whether it is the library's, unchanged. A library part is
checked on the user's computer like any other part; lengua trusts no part for being its own.
`examples/sum` and `examples/checksums` are built on them. Their limits are fixed: a function
reaches a fixed number of bytes whatever length it is given, so each part states the most it
takes: 20 bytes for a number, 4096 for a text and 256 numbers for a sort. The library's
assertions are numbered from 800, each part its own.

How far each is proven, by its own program on lengua's runners (todo.md has the runs): `decimal`,
`lines` and `bytes` for every input in their ideas' ranges; `sort64` for two numbers, with three
tested; `crc32` for a text of 2 characters in five minutes, and tested beyond. `examples/sum` is
proven for every input in its range, and `examples/checksums` as far as crc32.

## The rules

The rules are NASA's Power of 10, adapted to WebAssembly, with eight memory rules
([power-of-10-webassembly.md](plugin/skills/compile/references/power-of-10-webassembly.md);
`src/checker/wasm/`). The checker reads the module back with `wasmparser` and takes each field's
line from the text. A violation stops `check` at `code_problems`, so nothing that breaks a rule
runs. A report names its rule and line, `rule 4 (small functions) main.wat:12: …`, with the fix,
at most 20 at once.

| Rule | What main.wat must do |
|---|---|
| dialect | No annotations and no custom section but `name`; no reference types in its own code; no `$lengua.` names; the library's names left to the library |
| 1, simple control flow | No recursion; `unreachable` only right after a call that never returns; a `br_table`'s default leads to a block that ends in an assertion or an error exit |
| 2, bounded loops | Each loop has `;; bound: N` and a counter the checker follows; one `;; bound: forever` loop, which calls `sock_accept`, only for an idea that listens |
| 3, fixed resources | Imports only from WASI preview 1, only functions lengua gives, only what the idea allows, with WASI's types; one memory, its minimum equal to its maximum; no `memory.grow` |
| 4, small functions | At most 180 operators a function; one end, no `return`; blocks at most 256 deep |
| 5, assertions | An assertion is `i32.const N` and `call $assert_fail`, N from 1 to 899 and used once, reached only when a check fails; at least two a function on average |
| 6, smallest scope | Exports exactly `_start` and `memory`; a mutable global, and a writable region more than one function reads, says why on a `;; shared:` line |
| 7, checked results | Each error a host call or the library returns is tested at once, never dropped |
| 8, named constants | Exit codes, descriptors, lengths and modes are named constants; `_start` never falls off its end; a read-only region's size is its data's |
| 9, no indirection | No start function, no tables, no element segments; every computed index checked against its region, or left to the proof inside a `;; unsafe:` block |
| 10, checked by tools | The module validates with the engine's features; no function, import, global or local left unused; no code no path reaches |

Three things in the rules serve a large program (W22's scale test asked for them):

- **A count of operators.** `check`'s reply has `functions` once main.wat builds, whether or
  not the rules pass: how many functions it has, the largest as rule 4 counts it and the
  average; each function with 135 operators or more, 75% of the limit, by name, at most 12, the
  largest first; and rule 5's assertions a function, with how many the rule needs
  (`Report::size_lines`, `src/checker/wasm/mod.rs`). Rule 4 counts every operator of a body as
  the module holds it, its last `end` aside.
- **Rule 9 names a length from two paths.** A length that is a parameter on one path and a
  number loaded from memory on another is one of several values after the paths meet, and a
  check there bounds a value of one owner, not one of several. The rule's message says so and
  says the fix, a check on each path. The checker was not made to accept the single check: a
  value of several owners that passed a check would have to become a plain number, and a
  program that then used it as an address would break M1 where it passes today.
- **Named data.** `;; text: NAME` on the line of one string of a `data` segment names its
  bytes; on a segment's first line, with no string there, it names all of the segment.
  `;; at: NAME`, `;; offset: NAME` and `;; len: NAME` on a constant global's line say that its
  number is the text's address, its offset in its region, or its length. The checker computes
  each from the module and from the segment's strings as `wast` read them
  (`src/checker/wasm/texts.rs`), and a number that differs is a violation of rule 8 that says
  the right number. The reply's `data` has the table of every text while a number is wrong or a
  text has no number, and one line when all are right. Two named texts of two segments that
  share bytes are a violation too. The markers are comments: main.wat stays plain WebAssembly
  text and the one text that is checked and run, the module does not change, and a number with
  no marker is checked by nothing here, as before.

The memory rules work over regions: an immutable global `$x` holding an address, with `$x.size`
(writable) or `$x.rosize` (read-only). Regions start at address 1024 or above, inside the memory,
and don't overlap.
- M1, owners: every address comes from a region or a pointer a caller lent, never a plain number.
- M2, bounds: every access stays inside its owner; a computed one is rule 9's.
- M3, lifetimes; M5, initialization; M7, races: these hold by construction (locals can't be
  addressed and nothing is freed; memory and locals start at zero; a program has no threads, and
  the pieces it splits its work into share no memory with it or with each other), and the
  checker never reports them.
- M4, exclusive writes: a buffer a call writes overlaps no other buffer of the same call, as far
  as the rules can tell them apart.
- M6, immutability: nothing writes a read-only region, and data segments lie inside read-only
  regions.
- M8, the stack: the deepest call path from `_start`, the library's frames included, takes at most
  262,144 bytes, half of the engine's 512 KiB.

Determinism: no atomics, no relaxed SIMD, no trapping float-to-int conversion, no reinterpretation
of a float that may be NaN, and no division whose divisor may be zero. The engine makes every NaN
canonical, so every machine computes the same bits.

`args_get` and `environ_get` take no length: the host writes an address for each argument or
variable, and every byte of their text. The rules hold the program to ask first and to check: they
follow the count and the size `args_sizes_get` or `environ_sizes_get` wrote, from where it wrote
them, through an `i32.load` and a local, to the unsigned comparisons that bound them, and judge
the two buffers by those bounds, four bytes for each of the count. A call with a size that no
check bounds on the way, or that the program may have written over, is refused (bounds), with
what to write; the calls and the checks stand in one function, since a call of one of main.wat's
own functions may write anywhere.

What the rules can't settle they leave to the proofs, as obligations, each on its line: an access
through an address loaded from memory, an index checked against a limit the rules don't follow,
an access inside an unsafe block, and a signed division that may divide the most negative number
by -1. A proof settles a division's obligation in full. Of an access, it shows only that it stays
inside the program's memory, since the model follows no region: whether it stays inside its
region is tested, not proven. check's `memory` line says which. The rules themselves take no such
access to stay in its region: a write they leave to the proof, or one in an unsafe block, may
land in any region, so after it they know nothing of what memory holds (no iovec array, no size
the host wrote), and the buffers of a host call that follows are the proof's as well, or, for
`args_get` and `environ_get`, refused. What a write of that kind may do to another region, a
read-only one included, is what the memory line calls tested, not proven.

### Secrets

For an idea with a `- Secret:` line, the secret rule follows the named inputs from `_start` through
the operand stack, locals, globals, memory and calls (`src/checker/wasm/secret.rs`). It refuses,
each on its line with what to write instead:
- a secret condition of `if`, `br_if`, `br_table` or `select`, so a loop whose count depends on a
  secret;
- a secret address of a load, a store, `memory.copy` or `memory.fill`, or a secret length of a
  copy or fill;
- a secret operand of a division, a remainder or any float operator;
- a secret argument of a host call or of the library, but the bytes `fd_write`, `sock_send` and
  `$write_all` send; and `$fill` reading a secret up to a stop byte.

Masks made with arithmetic pass, as the compile skill's constant-time.md shows. What it can't tell
apart it takes as secret: every argument's text when one argument is secret, every variable's
when one variable is, every file the program reads when one file is. What it can't follow it
refuses: past 4,000,000 followed operators, or blocks and calls 256 deep. The rule's soundness
review is docs/reviews/secret-rule-2026-11-10.md.

The rule is over main.wat, and WebAssembly with no branch on a secret can still become machine
code with one, since the engine chooses the CPU's instructions. So for a module the rule passes,
`check` also reads the machine code (W13, `src/checker/wasm/machine.rs`). The secret rule gives
every operator that takes a secret operand, by its offset in the module. `check` compiles the
module with the engine as `run` and a run from a shell configure it, with the compiled modules'
cache they use, and takes the code, each function's place in it, and Wasmtime's address map, which
says for each stretch of code the offset of the operator it was made for. Those offsets are the
checker's own: Cranelift's translator names each operator by where its reader stands in the module
before it reads the operator, which is the offset `wasmparser` gives the checker for the same
bytes, and the module the engine compiles is the module the checker read, without its names, which
come after the code. Each stretch made for an operator that takes a secret is read, x86-64 with
`iced-x86` and ARM64 by the architecture's encoding index, which sorts each 32-bit word by a few
fixed bits. On the operator's line of main.wat, `check` refuses:
- a conditional branch (x86-64 `jcc`, `jrcxz`, `loop`; ARM64 `b.cond`, `cbz`, `cbnz`, `tbz`,
  `tbnz`);
- a jump or a call through a register, and a call of any function, which is one of the engine's;
- a division or a square root;
- the address of data in the code taken into a register (x86-64 `lea` of an address counted from
  the instruction; ARM64 `adr`, `adrp`): how the engine starts a lookup in a table, or a jump
  through one. A constant loaded from the code with no index isn't refused;
- bytes it can't read as an instruction.

What the secret rule already holds of an operator, its code may do: a `select`'s may branch on its
condition, which isn't secret, as the engine's code for a `select` between vectors does on both
CPUs; a load's, a store's, a `global.set`'s and a bulk memory operator's may check addresses and
lengths and call the engine's copy or fill; the code of a call of one of main.wat's functions
calls it. ARM64's code holds constants inside itself, as a load of the bytes after the next word
and a jump over them; those bytes aren't read as instructions.

Reading Cranelift 0.137.1's lowering for every place it emits a branch, a table or a call found
one operator the secret rule passes whose code isn't straight: on x86-64, `i8x16.shl` and
`i8x16.shr_u` by an amount that isn't a constant read a mask from a table at an index made from
the amount (`ishl_i8x16_mask_table`, `ushr_i8x16_mask_table`). `check` refuses it there, and when
only the vector is secret too, since it can't tell which operand the index came from. On ARM64
none was found: its branches are those of `select` between vectors, of traps for divisions and
float conversions, which the secret rule refuses before, and of atomics, which lengua has off. The
tests pin the refusal on x86-64 and the pass on ARM64 with one module, and pin both CPUs' readers
with code written as bytes.

The address map gives some code to no operator: a function's entry, the constants it ends with,
values moved between registers and the stack, and what Cranelift's optimizer made anew from a
function's operators, whose new instructions carry no offset. The first build of this check
trusted that code, and its own test showed why it can't: in secret-equal with a lane shift by
an amount made from its mask, the shift's code came out with no offset, made anew it seems, so
the table's address is in code with no operator, and that build passed it (run 42927099111).
So in a function that has code of an operator that takes a secret, `check` reads the code given
to no operator too, all but the function's first stretch, its entry, and its last, its
constants, and holds it as code that computes with a secret. A finding there is reported beside
the first operator after it that takes a secret, or the last before it, so its line is near
the cause and may not be the cause's own. A table or a branch the optimizer made from operators
that take no secret, in such a function, is refused as well.

`check`'s `tests` say which CPU's code was read. `run`, a run from a shell and the attester read
the code again before they run it, each for its own configuration, so the code that runs was read
on the computer that runs it, whichever computer wrote lengua.lock; the lock records no CPU. An
idea with no `- Secret:` line compiles nothing for this, in `check` or in a run.

What the check of the machine code trusts, on both CPUs: the time the CPU takes for the
instructions it leaves, such as a multiplication, a shift or a vector instruction; a function's
entry; all the code of a function where no operator that takes a secret has code; the code of
operators that take no secret, of lengua's library and of the engine's own functions and the
host's; that the engine compiles a module to the same code each time on one computer, and gives
its address map the offsets its code came from; and, on ARM64, that a word is the instruction its
group's bits say, since nothing else of a word is decoded.

## The tests

The tests run in the server's pool of workers, one at a time per CPU core (`src/testrun.rs`). The
examples run first. Then each generated block's runs go in parallel, each drawing its inputs and
expected results as it starts; once one fails, no later run of that block starts. At most 10
failures are reported, and all the tests must end within 90 seconds. Each test run has 2 seconds,
8 MiB of output, 64 MiB a file and 256 MiB of files in all.

A test gives its input as the program would meet it. Odd-numbered generated runs get standard
input, and what a test's server or client sends, in three pieces: the first byte, up to the
middle, and the rest, with 2 ms between them. A run that listed a folder runs again with every
listing turned round, and a different result fails, with advice to sort; a run that split its
work runs again with its pieces one at a time, the last first. Each run is compared on
standard output and error (exactly, or by a rule's pattern), the exit code, and the working
folder, which must hold the expected files and nothing else. An exit of 134 is read back as
"assertion N failed at main.wat:LINE".

Once every test passes, each test's largest input runs alone 5 times, and the done reply gives
each test's cost: the WebAssembly operations it executes, which the engine counts as fuel, the
same on every system, and its time on the clock.

## The prover

`check` proves each generated test on a model of the built module (`src/prove/`), a test of a
piece of split work on a model of a piece's run. A block's line
says one of three things:
- proven for every input in range, or for every input with text up to B of the idea's O bytes,
  with how many input combinations and paths it took, and whether two solvers confirmed each
  question;
- found wrong: the proof found inputs the tests didn't try where main.wat is wrong. The inputs run
  for real in the pool before the reply says so; a confirmed one goes into lengua.lock at once,
  lengua's `verified` entry comes out of idea.md, and `run` refuses the build from then on;
- tested, not proven, and why: what the model doesn't follow, the time running out, or what a real
  run made of what the proof found.

Before it trusts the model, `check` runs the block's first 24 cases, drawn from the tests' own
seed, through the model and the real program: they must agree.

### Two solvers

lengua asks z3 4.16.0 and cvc5 1.3.4, over pipes in SMT-LIB 2 (`src/prove/smt.rs`). z3 answers
first, by bit-blasting into its SAT solver, since z3 4.16's own simplifier gets some bit-vector
formulas wrong; lengua simplifies the terms itself. cvc5 confirms, with four times z3's time: an
"impossible" counts only when both say it. A "possible" from cvc5 whose inputs check out overrules
z3. Where floats come in and the bit-vector session can't settle a question, cvc5 alone answers
it, and the line says so. Inputs a solver finds count only when lengua's own evaluator shows they
meet every condition. With one solver installed, it answers alone, and the line says that too.
The prover starts whatever solver it finds, in the `solvers` folder beside lengua-mcp's folder,
then on PATH; `doctor` checks their versions.

### By length and by induction

By length, the inputs split into shapes: number ranges by sign and digits, each choice, each text
length shortest first, and text beyond ASCII by the widths of its characters. One proof lays out
at most 65,536 lengths from where it starts; where time runs out, it keeps the lengths it finished,
and `check` with `prove_further` proves on from there in another stretch of time. At a branch the
prover first tries the path's own comparisons and up to 8 witness inputs; where they settle
nothing it follows both sides, and asks a solver at each branch only when that gives too many
paths. Every obligation and trap is asked on each finished run, and the output check is one
question.

By induction, a block is proven for every length when its program reads standard input in one
loop of its own through `$fill`, as one ASCII text, and maps it byte by byte or counts in step
with it. The counts are `len`, `words` and `count` of the text or of the text mapped byte by byte
(`lower`, `upper`, `tr`), and sums of them, as a count of vowels is; output that is the same for
every text, as with standard output closed, needs no count. From any state the loop can be in at
its head, one more buffer-full must keep what it writes and counts in step with the formula; the
base proves every input of up to a buffer-full. At the head, what a time round changes is any
value, a count is anything it can have grown to, and a value that stays in a range by its own
terms, as a byte carried from one buffer-full to the next does, is any value in that range. A
program that reads with `fd_read` itself, reads in several places, waits on `poll_oneoff` or is
on the network is proven by length only. So is a program that splits its work when it reads in
the whole program's loop and in the piece's: one that reads in one loop for both is proven by
induction as a piece.

The base takes each final piece of up to a buffer-full: in one shape for a buffer of up to 256
bytes, else one length at a time. So the buffer's size decides whether the proof for every
length ends in its time. Measured on Linux on x86-64 for a count of vowels 16 bytes a step, in a
proof's 300 seconds (run 42976169988): with a buffer of 1,024 bytes every length is proven in 15
seconds, and with 4,096 bytes in 191. With 64 KiB the step holds in 9 seconds, the base reaches
4,635 bytes, and the line bounds the proof there. The proof by length alone reaches 5,679 bytes
in that time.

Counts meet as terms, with no solver: the conditions a count counts that test the same byte are
written as the byte's sets of values, so five counts of one letter each and one count of the
bytes that are one of the five are the same term (`src/prove/term.rs`). A step of 16 bytes with
SIMD128's compares, `i8x16.bitmask` and `i32.popcnt` is then the formula's own count.

How input arrives is part of a proof. A module that reads only through `$fill` with a stop it knows,
off the network, is proven with its input cut into pieces any way. A networked block is proven for
four deliveries: whole, in three pieces, and with the first two or the last two pieces together.
A program off the network that reads standard input with its own `fd_read` is proven only for its
input as one read gives it, and is refused past 4,096 bytes, one pipe write.

### What the model follows, and what it refuses

The model follows main.wat's integer, float and memory operators as WebAssembly defines them, with
the engine's canonical NaNs, and SIMD128's loads, stores, lane moves, bitwise operators, integer
lane arithmetic, comparisons, shifts and shuffles; and each WASI function as lengua's host gives
it, with the test's files, network, clock and random bytes. Each trap is placed on main.wat's line.

It refuses, as "tested, not proven": a call that splits the program's work into pieces, whose
runs it doesn't follow; tables, element segments and a start function; recursion, and
calls that may take more than 262,144 bytes of stack; SIMD128's float lanes' arithmetic, its
narrowing, widening and pairwise forms and its lane loads; the trapping float-to-int conversions;
a number or a name the input chooses in a host call; a server's wait on `poll_oneoff` for its
connections, so the server template's proofs stop there; clients at once; a block that uses
`now`; and the test functions `sha256` and `aes128`, with `sort`, `uniq` and `tally` past 8 bytes.

### Budgets

A build's proofs share 300 seconds, smallest block first, each with an equal share of what's left.
A shape gets at most 256 paths, a question 20 seconds, and a shape once begun at least 20 seconds
past its deadline. Proof workers are as many as free memory allows at 1 GiB each, and a watchdog
stops the solver holding the most when free memory runs low; the proof then says the computer ran
low on memory, as does a proof whose own terms outgrow what is free (`src/prove/watch.rs`). lengua
reads the memory from `/proc` on Linux and from the system's own counters on macOS and Windows
(`crates/lengua-os`). On Linux the workers also run below the user's own work; on macOS and Windows
their priority isn't lowered.

## The runtime

### The engine

Wasmtime 50.0.1 with Cranelift, pinned exactly and embedded in lengua-mcp, with no Wasmtime WASI,
component model, GC or threads built in (`src/wasm/engine.rs`). Every feature is turned off, then
these on: floats, mutable globals, saturating float-to-int, sign extension, reference types,
multiple values, bulk memory and SIMD128. Reference types are on for the tables of C, C++ and Rust
toolchains' modules, which the benchmark runs; the dialect refuses tables in main.wat. NaNs are
made canonical. A program's WebAssembly calls may take 512 KiB of stack, past which a call traps.
The features are one set on every machine. That is point 3 of the finish line as the owner
decided it on 2026-11-11: the same bits on every machine, so proofs and tests hold on each, with
the machine code compiled for the machine's own CPU. Relaxed SIMD and wide arithmetic stay off.

Three setups:

| Setup | Used by | Counts operations | Stops at a deadline |
|---|---|---|---|
| Test | the tests | yes, as fuel | yes, through the engine's epochs, every 10 ms |
| Run | the `run` tool and a background server | no | yes |
| Shell | lengua-run, `lengua-mcp run`, the benchmark and `doctor` | no | no |

Each setup has two engines, with the same settings wherever compiled code depends on them
(`shared`, `src/wasm/engine.rs`): Wasmtime holds a compiled module to those settings when it loads
one, and a test loads each setup's module into the other engine.

- **The engine that compiles.** Cranelift compiles a module for this machine's CPU. The tests'
  workers and the `run` tool compile a module the first time they run it, and Wasmtime's cache
  keeps it, in the `cache` folder of the plugin's data folder when the agent's harness gives one
  (`PLUGIN_DATA`, or `CLAUDE_PLUGIN_DATA`), else of lengua's own.
- **The engine that only runs.** It has no compiler in it, and loads a module compiled ahead of
  the run. A run from a shell uses it, and reads no cache.

When a program's tests pass, `check` compiles its module for a run from a shell
(`Engine::precompile_module`) and keeps the result beside the tested module, in
`tested/<builder>/<module's SHA-256>.shell.cwasm` of the data folder, with a seal after it: an
HMAC-SHA256, keyed with the user's key, over a domain of its own, the module's SHA-256 and the
compiled bytes (`compile_sealed`, `crates/lengua-os`). A run loads that file only when its seal
holds (`load_sealed`), compared in constant time, and copies its code into memory of the
engine's own. Wasmtime then holds the module to its own version, its settings and this CPU's
features, and refuses one that doesn't suit. The Shell setup maps no copy-on-write image of a
module's first memory, which pays where a module starts many times: a run starts its module
once.

### The host

lengua gives WASI preview 1 itself, on the engine's linker, so its semantics are its own, the same
on every system, and the prover models one implementation (`src/wasm/host/`, `src/wasm/wasi.rs`).
All 46 functions of preview 1 are linked, so a toolchain's module links whatever it imports. 30
are given. 16 answer `NOSYS`: `fd_advise`, `fd_allocate`, `fd_datasync`, `fd_fdstat_set_flags`,
`fd_fdstat_set_rights`, `fd_filestat_set_size`, `fd_filestat_set_times`, `fd_pread`, `fd_pwrite`,
`fd_renumber`, `fd_sync`, `path_filestat_set_times`, `path_link`, `path_readlink`, `path_symlink`
and `proc_raise`. Besides WASI's, lengua gives fifteen functions of its own, each in a module of
its own name, for what a program reaches by itself while it runs (below), and two for work it
splits into pieces: seven in
`lengua_net_1` for the network, two in `lengua_fs_1` for the files its arguments name, one in
`lengua_run_1` to start a program, and three in `lengua_term_1` for the terminal. Every error,
of WASI's functions and of lengua's own, is one of a closed set of 24 of WASI's numbers, and an
error outside a function's own list becomes `IO`. The two for split work are in
`lengua_split_1`.

Each call checks the idea first: one the idea doesn't allow answers before any of its buffers is
looked at. An allowed call checks every buffer it borrows against the program's memory, then does
its work. The idea's lines decide what is allowed (`src/wasm/host/policy.rs`):
- every program: its arguments, `fd_write`, `fd_fdstat_get`, `proc_exit` and `sched_yield`;
- `standard input` or `the terminal`: reading standard input, and `poll_oneoff`;
- `files`: the working folder and the files in it;
- `the clock`: the clocks, and `poll_oneoff`'s waits on time;
- `random bytes`: `random_get`;
- `the environment`: the variables it is given;
- any `- Network:` line but one that only looks up addresses: its connections; `listens on`:
  `sock_accept` on its listener;
- `opens TCP connections to`: `tcp_open`; `looks up the addresses of`: `dns_lookup`; `sends UDP
  datagrams to`: `udp_open`, `udp_send` and `udp_recv`; `receives UDP datagrams on`:
  `udp_recv_from` and `udp_send_to`;
- a `- Files:` line: `file_open`, and what reads, writes, seeks and closes a file; one that
  names a folder: `folder_open`, and what `files` allows on a folder and the paths in it;
- a `- Runs:` line: `program_run`;
- `the terminal`: `term_is`, `term_size` and `term_raw`.

A call the idea doesn't allow answers `NOTCAPABLE` and does nothing, except five that toolchains'
startup code calls, which answer harmlessly: an empty environment, no folder, and zeros for random
bytes. main.wat itself can't import a function its idea doesn't allow: the checker refuses it.

Descriptors are fixed by role: 0 to 2 the standard streams, 3 the working folder (named `.`, only
for an idea that uses files), 4 the listener, 5 on the connections lengua made, in the idea's
order, and after them the UDP port the idea receives datagrams on. What a program opens, accepts
or is granted to open by itself takes the lowest number free after those. A program holds at most
4,352 descriptors.

Files: every path goes through `cap-std`, which resolves it beneath the working folder. A path that
climbs out of it, or starts with `/`, gives `NOTCAPABLE`; a link on the way gives `ACCES`. Names
are held to what every system takes (`src/names.rs`): 255 bytes a name and 1,023 a path, UTF-8 of
characters Unicode 9.0 assigned, none of the characters, device names (`COM0`, `LPT0`, `CONIN$`
and `CONOUT$` among them, and a device's name with spaces before its dot) and short-name shapes
Windows refuses. Names are found byte for byte, and a new name that differs from one in its folder
only in case or normalization gives `EXIST`. A folder lists sorted by name. A symbolic link, and
on Windows a junction, is a link. A hard link is a regular file to every system, with nothing on
the way to it that could be refused, so lengua asks the system how many names the file that
opened has, and opens none with more than one: `path_open` gives `ACCES`, before the file is
emptied when it was to be. The count is the open file's own (`fstat` on Linux and macOS,
`GetFileInformationByHandle` on Windows), so it holds on all five systems, on every file system
that counts hard links; one that has none, or reports every file with one name, opens every
file. What it costs: a file with a second name anywhere on its volume, inside the working folder
too, since the names can't be told apart, isn't opened: files in a tree of hard-linked backups
(`cp -al`, `rsync --link-dest`), in a package manager's folders that link from a store (pnpm,
uv), in a Cargo `target` folder, in an OSTree checkout, or after a tool that joins equal files.
The user copies such a file to give it to a program. The file still lists as a file, and its
size can be asked.

A folder's listing is lengua's memory, not the program's: it is read once, shared by every
descriptor that lists the folder while it stays the same, and the listings a program's descriptors
hold are held to 1,048,576 entries in all (`LISTED`, `src/wasm/host/files.rs`). The `fd_readdir`
that would pass it answers `IO`, as does any call on a path in a folder with more entries.

The working folder is never one of lengua's own. For a program that uses files, and for a
`stdin_file`, `run` refuses a folder that is, holds or lies inside the data folder or the folder of
compiled modules, the plugin's own (`PLUGIN_DATA`) too, and on Linux `/proc`, `/sys`, `/dev` and
anything beneath them, with links followed before paths are compared (`own_folder_problem`,
`src/server.rs`). The program's own folder is a working folder like any other: what a program
changes there only stops its next run.

The network: by default a program never connects or binds by itself. Before it starts, lengua makes each
connection its idea names, one after another in order, within 5 seconds each and 10 seconds in
all, after which the rest are handed over ended, untried, and binds its
listener, IPv4 only, on 127.0.0.1, or on every address when the idea serves any computer
(`src/connect.rs`, `src/wasm/host/net.rs`). A connection lengua couldn't make is handed over
already ended, and the run's report says why. A named computer, in the idea or in an argument, is
another computer: a name whose last label is a number (`2130706433`, `127.1`, `0x7f.1`) is no name,
and when any address a name resolves to is this computer's loopback or the unspecified address,
written as IPv4 inside IPv6 too, the connection isn't made. Over TLS, lengua does the TLS in Rust (rustls 0.23
with ring, Mozilla's roots from webpki-roots, the same on every system), and the program reads and
writes plain bytes. A stream never waits in the system: lengua naps between looks, within the
run's deadline, since a socket's own timeout on Windows can lose or repeat bytes.

What a program opens by itself (W18, `src/reach.rs`, `src/wasm/host/open.rs`): an idea may grant
more than connections made before the start, each grant a `- Network:` line of its own:

| The idea's line | What the program may do, with which import of `lengua_net_1` |
|---|---|
| `opens TCP connections to port 443 on example.com over TLS` | open such a connection: `tcp_open(name, name_len, port, tls, out_fd)` |
| `looks up the addresses of example.com` | read the name's addresses: `dns_lookup(name, name_len, out, out_len, out_count)` |
| `sends UDP datagrams to port 53 on 192.0.2.53` | send datagrams there and receive the answers: `udp_open(name, name_len, port, out_fd)`, `udp_send`, `udp_recv` |
| `receives UDP datagrams on port 5353` | receive on that port of this computer, which lengua binds, and answer the senders: `udp_recv_from`, `udp_send_to` |

The computers of a line are a name or an address, `any subdomain of example.com` (not the domain
itself), `the computer its first argument names`, or `any computer`; the ports are the line's
own. WASI preview 1 has no function that opens a connection, and its successor's sockets aren't
preview 1, so these are lengua's own, in a module of their own name: a module that imports one
says so, another WASI runtime refuses to link it rather than give it something else, and the
name carries a version, so a change of meaning gets a new name. A connection `tcp_open` gives is
a descriptor like the ones lengua made, for the same WASI functions.

Every such call goes the same way, and the order is the defence:
1. The name is read as lengua spells it (`spelled`): lower case, no dot at its end, an address
   in its one usual writing, an IPv4 address inside an IPv6 one as the IPv4 address. A number
   that only an older parsing reads as an address is no name (`INVAL`), and this computer, by
   `localhost` or by its loopback or unspecified address, is refused (`NOTCAPABLE`).
2. A rule of the idea must cover the name, the port, and TLS or not (`Rule::covers`). Nothing is
   resolved or reached for a name no rule covers.
3. lengua resolves the name itself, once, with the system's resolver, and every address is held
   to the rule (`vetted`). One address that is this computer's refuses the name, whatever the
   others are; so does one that many computers answer at (multicast, broadcast); so does one
   in a private network (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, 100.64.0.0/10,
   0.0.0.0/8, fc00::/7, fe80::/10), unless the line ends with `, private addresses too` or
   writes that address itself. An IPv6 address that carries an IPv4 one (written inside it,
   IPv4-compatible, NAT64's 64:ff9b::/96, 6to4's 2002::/16) is held to the IPv4 address's class.
   The program hears `NOENT` for a name lengua refuses by its addresses, as for a name with
   none, so it can't learn from the refusals which names a private network has; the run's
   report says what it was. An address the program wrote itself gets `NOTCAPABLE`.
4. lengua connects to the addresses it checked and to no others, within 5 seconds (`dial`), so
   a name that resolves anew between the check and the connection changes nothing. Over TLS, it
   checks the computer's certificate for the name.

The limits: a program holds at most 64 connections and datagram sockets of its own at once
(`MFILE` past them). lengua ends a connection the program opened once no byte has crossed it
for 300 seconds: a thread of lengua's own looks every quarter second, so a program that
computes without a call of the host is held to it too. A name has at most 253 bytes, a look at
a name gives at most 16 addresses, IPv4 first and each family in order, and a datagram holds at
most 65,507 bytes. On the port its idea receives on, a program answers only an address that sent
it something; where the line says `for any computer`, it sends an address at most three times
the bytes that address sent, since a sender can write another computer's address on its
datagrams. An idea listens on a TCP port or receives on a UDP port, one of the two.

In a test, the test's own servers stand in for every computer: a step names each by its
computer and its port, lengua listens for it on the loopback, and a program's `tcp_open`,
`udp_open` or `dns_lookup` reaches those and nothing else, after the same checks. Nothing a
test's program opens reaches a real computer. The prover refuses a module that imports from
`lengua_net_1`: which computer a program reaches is its own choice while it runs, and the model's
network is the connections made before the start, so such a program is tested, not proven.
`check`'s reply and `run`'s (`may_reach`) say each grant in words, and a run from a shell says
them on standard error before the program starts, with what lengua refused while it ran.

Files the user names (W18, `src/named.rs`, `src/wasm/host/named.rs`): by default a program
reaches files only beneath its working folder. A `- Files:` line grants a file or a folder that
one of its arguments names, to read, to write or both:

| The idea's line | What the program may do, with which import of `lengua_fs_1` |
|---|---|
| `reads the file its first argument names` (`writes`, `reads and writes`) | open that file: `file_open(argument, mode, out_fd)` |
| `reads the files its arguments name` | open the file any argument names, the same way |
| `reads the folder its first argument names` (`reads and writes`) | open that folder, and the paths in it with WASI's own functions: `folder_open(argument, out_fd)` |

The program gives the argument's place, from 1, and never a path: lengua reads the argument,
which the user typed, so no path a program builds from what it read is ever opened. `mode` adds
1 to read, 2 to write, 4 to make the file when it isn't there, 8 to empty it and 16 to write at
its end. Every call goes the same way:
1. A line of the idea must cover the argument's place, as a file or as a folder, and the mode:
   a file granted for reading isn't opened to write (`NOTCAPABLE`).
2. lengua resolves the argument's text. In a test, beneath the test's own folder, by the
   working folder's rules, so a path that starts at the root or climbs out is `NOTCAPABLE` and a
   test reaches no file of the user's. In a run, on the user's file system: a path that doesn't
   start at the root counts from the shell's current folder, or from the `run` tool's `folder`,
   and the links of the folders on the way are followed once, as the system follows them for
   any program the user runs.
3. The place it found is never one of lengua's own (`named::forbidden`): a file may not lie in
   the data folder or the folder of compiled modules, and a folder may not be one, hold one or
   lie in one. Nor the system's: `/proc`, `/sys` and `/dev` on Linux and macOS, and on Windows
   anything that isn't on a drive or a share, which takes the devices and the pipes.
4. The file is opened beneath the folder lengua resolved, with every rule of a file in the
   working folder: its last name is held to what every system takes and found byte for byte,
   it is never a link, what opens is a regular file, and a file with more than one name isn't
   opened, before it is emptied when it was to be. A folder is a root like the working folder,
   with its rules; one granted for reading takes no file to write, makes nothing, removes
   nothing and renames nothing (`NOTCAPABLE`), and nothing moves from one root to another.

A named file is a descriptor like any other: the run's file limits, the descriptors' count and
the deadline hold it. What lengua refused is in the run's report, and a run from a shell says,
before the program starts, the place each argument names.

Programs a program starts (W18, `src/starts.rs`, `src/wasm/host/start.rs`): by default a program
starts nothing. A `- Runs:` line names one program and bounds its arguments, and the program
starts it with `program_run(command, command_len, input, input_len, output, output_len,
out_size, out_code)` of `lengua_run_1`: the command is the program's name and then each
argument, each ended by a NUL. lengua starts the program, gives it the input as its standard
input, waits until it ends, and writes what it printed, how many bytes that is, and its exit
code. Nothing stays behind the call: no process and no descriptor. The order is the defence:
1. A rule of the idea must cover the command (`Rule::covers`): its name, each argument the line
   writes byte for byte, a value where the line leaves one to the program, and no argument
   more. A value is 1 to 255 bytes of text with no control character, and never starts with
   `-` or `/`, so it is no option on any system. Nothing is looked for otherwise.
2. lengua finds the program itself (`starts::found`): in the absolute folders of the user's
   `PATH`, in order; on Windows a `.exe` alone, never a `.bat` or a `.cmd`. A line never names a
   shell or a program that only starts another (`sh`, `bash`, `cmd`, `powershell`, `env`,
   `xargs`, `sudo` and others, `starts::SHELLS`), and the file a name resolves to, with its
   links followed, isn't one by its own name either. The file never lies where the program
   could have written it: its working folder, a place one of its arguments names under a
   `- Files:` line, or the system's temporary folder.
3. lengua starts that file with no shell between: an empty environment (on Windows the one
   variable `SystemRoot`, with the system's own value, which Windows's libraries need to load;
   Windows on ARM64 adds `PROCESSOR_ARCHITECTURE`, the CPU's name, to every process by itself);
   a new empty folder of lengua's as its current folder, removed afterwards; standard input a
   pipe with the program's bytes, standard output a pipe lengua reads, standard error nothing;
   and no other handle of lengua's.
4. It has its line's seconds (10, or `within N seconds`, 1 to 600) and never past the run's
   deadline, and the program's buffer for what it prints. Past either, lengua ends it and every
   process it started, its process group on Linux and macOS and its job on Windows, and the
   call answers `TIMEDOUT`, or `FBIG` with the buffer full.

The started program is the user's own, with the user's rights: lengua doesn't confine it, and
what bounds it is its name and its arguments, which `check` and `run` show. A lengua program
started this way, through `lengua-mcp run`, is held to its own lock and idea as always, and gets
nothing of the starter's grants.

The terminal (W18, `src/wasm/host/term.rs`, `lengua_os::terminal`): an idea that uses `the
terminal` may call three functions of `lengua_term_1`. `term_is(fd, out)` writes whether
standard input, output or error is a terminal. `term_size(out_columns, out_rows)` writes its
size. `term_raw(mode)` with 1 makes standard input give keys as they're pressed, with no echo
and no line editing, Enter as the byte 13 and the keys as a terminal's sequences on every
system; with 0 the terminal is as it was. The interrupt key stays, so Ctrl-C still ends the
program. Once raw, a thread of lengua's reads the keys, so `poll_oneoff` can wait for a key
until a time. lengua puts the terminal back as it was when the program's host is dropped, which
every end of a run comes to (its return, `proc_exit`, a trap, the deadline, lengua's stop), and
from a handler when a signal ends the process (`SIGINT`, `SIGTERM`, `SIGHUP`, `SIGQUIT`) or
Windows's console closes. Only a run from a shell has a terminal: in a test and in the `run`
tool `term_is` writes 0 and the other two answer `NOTTY`.

In a test nothing of these reaches the user's computer: a named path stays in the test's own
folder, and no program is started. A step of the test says what a command gives (``the program
`hostname` prints `box\n` and exits with code `0` ``), every test of the idea sees those, and a
command no step gives isn't there (`NOENT`). The prover refuses a module that imports from any
of lengua's own modules, as it does for the network: the model's files are the working folder's,
it has no other program, and its input is the test's bytes, so such a program is tested, not
proven. `check`'s reply and `run`'s (`may_reach`) say each grant in words, the terminal's too.

Work split into pieces (W20, `src/split.rs`, `src/wasm/host/split.rs`): by default a program is
one run on one core. A `- Splits:` line lets it split its work into pieces that run at once:
`- Splits: its work into up to 8 pieces, each of up to 1048576 bytes, each gives back 8192
bytes`. The program calls two functions of `lengua_split_1`:

| Function | What it does |
|---|---|
| `split(input, input_len, pieces, output, output_len)` | cuts the `input_len` bytes at `input` into `pieces` pieces, runs each, waits for all, and writes each piece's answer at the start of its `output_len / pieces` bytes of `output`, in the pieces' order, with zeros after it |
| `piece()` | answers 0 in a run that is a piece, and `NOENT` in the run that is the whole program |

A piece is a run of the same module from `_start`, in a store of the engine's own: its own
memory, as the module declares it, its own globals and its own stack. Its standard input is its
share of the bytes, what it prints is its answer, and it reaches nothing else: not what the
idea grants the program (files, the clock, random bytes, the network, programs, the terminal),
and not `split`, which answers `NOTCAPABLE` in a piece. So `_start` asks `piece()` first, and a
piece is an ordinary program that reads standard input and prints.

A piece reads its share out of the program's own memory: lengua lends those bytes for the
call (`lengua_os::lent`), read-only, and each read of a piece copies from them into the
piece's memory. Nothing writes them meanwhile, since the program waits in the call and no
piece has its memory, and the call returns only once no thread holds them. So there is still
no memory two runs can both write, and none a run can write that another reads.

Two designs were weighed. WebAssembly's threads proposal shares one memory between threads, with
atomics; it needs a rule that shows each thread writes only what no other reads or writes, the
engine's threads feature on, which changes what compiled code depends on, and a prover that
follows interleavings. Separate runs share nothing: no rule has to show the absence of a data
race, since no byte of a piece's memory is another piece's or the program's; every rule and
every proof holds for a piece as for any program, since a piece is one; the engine's settings
don't change; and the code is the same on all five runners. The cost is that a piece's bytes
are copied, once, from the program's memory into the piece's as it reads them, with one copy of
its answer, and that each piece is a new store. lengua has the second design. Measured on two
examples, a byte count and a CRC, with a piece for each core against the same work on one
core: on 256 MiB the split takes 0.58 to 0.94 times the time on every runner; on 64 MiB it
takes 0.74 to 0.97 times on three runners and 1.10 to 1.13 times on Linux on x86-64 and, for
the count, on macOS (ROADMAP.md, W20, has the table and what still costs).

What a call gives depends on its bytes and its numbers alone:
- the cut is arithmetic: piece `k` of `n` holds the bytes from `floor(k × len / n)` to
  `floor((k + 1) × len / n)`, so the pieces follow each other and differ by a byte at most;
- a piece gets no clock, no random bytes, no argument and no variable, and starts from the
  module's first memory, so it gives the same answer for the same bytes;
- the answers are written in the pieces' order, whichever piece ended first, and only when
  every piece exited with code 0;
- where a piece doesn't exit with code 0 (an exit code, a trap, a failed assertion, its time,
  an answer past its room), the run ends as the first such piece in the pieces' order ended,
  with what that piece wrote to standard error, and nothing is written;
- how many pieces run at once, one for each core the computer gives the process, or what
  `LENGUA_PIECES_AT_ONCE` says (1 to 64), changes the time alone.

The limits, which the idea's line and the rules give:
- a call splits into 1 to the line's pieces, never more than 64, each no longer than the
  line's bytes, with room for exactly the line's bytes back from each; any other call answers
  `INVAL`, and a call without the line `NOTCAPABLE`;
- `check` refuses a main.wat that imports either function without the line (rule 3), one
  whose call gives its number of pieces other than as a named constant the line covers, one
  that splits and never asks `piece()`, and one whose memory, times the line's pieces and one
  more for the program, is past 16,384 pages: a program and its pieces together hold at most
  1 GiB (`src/checker/wasm/split.rs`);
- a piece can't split, so the pieces of a run are never more than the line's at once. They run
  on threads of lengua's, each with a stack of 4 MiB, started at the run's first call that
  splits, as many as the line's pieces or the computer's cores, whichever is fewer, and kept
  to the run's end. A thread is all a piece shares with the one before it: each piece is a
  new store, with the module's first memory and globals;
- the run's deadline is its pieces': in a test and in the `run` tool the engine's epochs stop
  a piece at it, and stop a piece that still runs once a piece before it in the order didn't
  end well. A run from a shell has no deadline and its engine stops nothing: there a piece
  that started runs to its end, and pieces not yet started aren't;
- a piece's answer is held to the line's bytes, and what it writes to standard error to 64
  KiB; past either the piece is stopped, and the run ends so.

An idea with a `- Splits:` line names `standard input` in its `- Uses:` line, since a piece
reads its bytes there, and names no secret: the pieces run at once, beside each other, and
lengua doesn't show that a secret in one changes nothing another can time, so `check` refuses
an idea with both lines. No secret crosses into a piece, since such a program has none.

A test runs a program that split its work a second time with its pieces one at a time, the
last first, as it runs one that listed a folder with the listing turned round, and both runs
must pass. A step, `the program is a piece`, makes a test's run a piece: with a piece's reach,
and its output held to the line's bytes. So an idea tests its pieces as programs, and the
prover proves such a test as any other, on a model in which `piece()` answers 0, as far as it
follows the piece's work: of the example's piece, a histogram in tables, it proves the empty text
alone. A test of the
whole program is tested, not proven: the model stops at `split`, since it doesn't follow the
pieces' runs. What a piece executes counts in a test's cost with the program's own operations.
`check`'s reply and `run`'s (`may_reach`) say the grant in words, and a run from a shell says it
on standard error before the program starts.

The clock: both clocks are rounded down to a microsecond (`src/wasm/host/clock.rs`). In a test the
clock and the random bytes may come from the test; otherwise random bytes come from the system.

### Limits by kind of run

| Kind of run | Time | Output | Files |
|---|---|---|---|
| A test | 2 s | 8 MiB a stream | 64 MiB a file, 256 MiB in all, a fresh working folder |
| The `run` tool | 10 s by default, 1 to 40 | 8 MiB a stream | a test's limits in its own empty temporary folder; no limit in the user's `folder` |
| A background server | 600 s, or until `run` with `stop`, or the session's end | 8 MiB a stream | as the `run` tool |
| `lengua-mcp run` | none | none: the shell's own streams | no limit, in the current folder or `--in`'s |

A host call ends at the run's deadline however much it was asked to do: `random_get` and a write
to a file go a mebibyte at a time with a look at the deadline between, a read of a file looks
before each buffer, and the end of a TLS connection waits two seconds at most and never past the
deadline. What still outlasts a deadline, such as one slow write of a mebibyte, is ended by the
pool, which kills a worker 20 seconds after its job's time.

Memory is held to what the module declares: the dialect makes main.wat's minimum its maximum, of
at most 16,384 pages, 1 GiB (the largest a shipped program declares is 1,538). A C, C++ or Rust
toolchain's module, an outside module, is held to the same 1 GiB, and the benchmark builds its own so. A
module's one table holds at most 1,048,576 entries, whatever it declares or grows to.

### Workers

Tests, the `run` tool and background servers run in child processes, `lengua-mcp --wasm-worker`,
which read jobs and write events as length-prefixed JSON (`src/wasm/pool.rs`, `src/wasm/worker.rs`).
A worker is reused job after job, each in a fresh store, and keeps up to 16 compiled modules. The
`run` tool and a background server each get a fresh worker of their own. A worker that broke, or
didn't answer within its job's time and 20 seconds more, is killed. A worker exits when its
standard input ends, so a killed server leaves no program running. Before a job starts, the pool
holds its arguments and environment to what every system can pass: no NUL, at most 32,000 of
Windows's command-line characters for the arguments, 32,767 for a variable, and no two names that
differ only in case. A shell's run doesn't go through the pool and isn't held to them.

### A run from a shell

Two programs run a checked program from a shell, attached to the shell's own streams, each in its
own process. Both make the checks below (What run checks) first, and nothing of the program is
loaded, compiled or run until they pass.

**lengua-run** is the program `check`'s done reply names. It is the lengua crate built without
the engine's compiler (the `compiler` feature off, `crates/lengua-run`): 5.9 MiB on Linux on
x86-64, where lengua-mcp is 23.1 MiB, and a run of the empty program takes 1.3 ms and 5.5 MiB
there, where `lengua-mcp run` took 2.2 ms and 11.4 MiB before it (ROADMAP.md, W19, has every
runner). It loads the module's compiled form under the
user's key and runs it on its main thread, where the system lets that thread's stack grow to 8 MiB
or more, as Linux and macOS do by default; elsewhere, and on Windows, on a thread of its own with
16 MiB. It gives the engine only the WASI functions the module imports. Two runs it leaves to
lengua-mcp, the program beside it, which it starts with the same words: one whose module has no
compiled form it can load, which it says on standard error, and one whose idea names a secret.
On a Unix lengua-run becomes lengua-mcp. On Windows it starts lengua-mcp and waits for it, in a job
that ends with lengua-run, so whoever stops lengua-run stops the program too.

**`lengua-mcp run`** runs every checked program. It loads the compiled form as lengua-run does.
Where none loads, it compiles the module, seals the result and keeps it, so the next run of
either program loads it. For an idea that names a secret, it builds main.wat, finds the operators
that take a secret, and reads the code of the module it loaded before it runs it. It runs the
program on a thread of its own.

## The lock, the key and trust

### lengua.lock

Only lengua writes it, atomically, and it is canonical TOML ending in a `[signature]` table
(`src/lock.rs`). lengua writes format 7 and reads 6 and 7; a format before 6 is the native
lengua's, and lengua says so. It holds, in order: the version; the dialect, `webassembly 1`; a
fingerprint over the idea's and the code's hashes, once code is stamped; `[request]`, the user's
words (at most 6 a build, the first and the latest, each at most 2,000 characters) and the idea's
hash when the build began; `[idea]`, idea.md's hash; `[code]`, main.wat's SHA-256, the module's,
the engine (`wasmtime 50.0.1`), the deepest stack the rules count, the unsafe blocks, the target
`webassembly`, the features used beyond WebAssembly 1.0 and, for a program built on parts, each
part with the hashes of its idea.md and its main.wat and the functions taken ([Parts](#parts);
the line is written only then, so a lock without parts is as it was); `[proofs]`, about that module: the
proven, the bounded with their lengths, the solvers, the wrong and the unproven with why; and the
signature, an HMAC-SHA256 of every byte before it, keyed with the user's key.

A lock is trusted only when it is at most 64 KiB, UTF-8, signed with this user's key (compared in
constant time), of a format lengua reads, with only fields lengua writes, laid out byte for byte as
lengua writes it, and with proofs only about the module its code stamp names. A lock exactly as
lengua writes it is read line by line with no TOML; any other text is read as TOML, which says
what is wrong. Both readers give the same lock from the same text: the line reader takes a body
only when lengua's writer gives it back byte for byte (todo.md's decisions, and a seeded test of
2,000 random locks).

The code stamp is written when every test passes, before the proofs end, so `run` works while
`check` still says proving. Proofs are stamped only while the code stamp names the same module.

### The key

32 random bytes in `key` in lengua's data folder, made on the first record (`src/lock.rs`,
`src/data_dir.rs`). The data folder is `$XDG_DATA_HOME/lengua` or `~/.local/share/lengua` on
Linux, `~/Library/Application Support/lengua` on macOS, `%LOCALAPPDATA%\lengua` on Windows, or
`--data-dir`. On Linux and macOS the folder is made readable only by the user (0700) when the key
is made or loaded for a record, and, where it is there with another mode, as when the engine's
cache made it first, when the server starts and at each run from a shell, before the key is read
(`DataDir::keep_private`); the key's file is the user's alone (0600) from the start; on Windows lengua
sets no permissions of its own, so the key is as private as the user's
`%LOCALAPPDATA%`. One key serves every agent harness, so a lock recorded in one verifies in
another. A lock signed on another computer doesn't verify here: the HMAC binds a lock to this
user's key. It is a record, not a way to trust another computer's lock; `lengua-mcp check` is.

### The idea's hash and the verified entry

lengua's hash of idea.md is the SHA-256 of the file without its top-level `verified` and
`generated` keys, so lengua's own record of trust changes no lock (`src/trust.rs`). At done, with
both files unchanged and no proof that found the build wrong, `check` writes its entry in idea.md's
`verified`: `{ by: lengua/0.2.0, at, idea: sha256:…, code: sha256:… }`, last in the list, keeping
everyone else's. It edits the frontmatter's lines and never writes its YAML again. Every `check`
first removes lengua's entry once it no longer names the idea and main.wat as they are, and a
proof that finds main.wat wrong removes it at once. What the hash leaves out, lengua doesn't read:
the frontmatter's YAML is parsed without the lines of those two keys, so a key whose value is an
alias of an anchor inside them names nothing and the idea is refused.

A program folder may come from anyone, so `check`, the attester and `run` read its files, and a
bundle's index.md and log.md, only as regular files of at most 32 MiB (`read_file`,
`src/state.rs`): a link isn't followed, a named pipe or a device isn't opened, and a larger file
isn't read. lengua.lock is read the same way, within its own 64 KiB.

### What run checks

`run`, from MCP or from a shell, runs main.wat only when (`src/server/wat.rs`):
- the lock records idea.md and main.wat as they are, and the key and the lock are trusted;
- no proof the lock records, none the running job found, and none lengua keeps in its data
  folder says this module is wrong. Whenever `check` or `run` reads or writes a lock that records
  a module as found wrong, lengua keeps that in `wrong/<idea's hash>-<module's hash>` of the data
  folder, so an earlier lock of the user's own, put back in the folder, doesn't let that module
  run for that idea; `check` reads the record too, and with such a lock in the folder says
  tests_failed, with what the proof found and that the lock doesn't record it, where `run`
  refuses;
- the working folder, for a program that uses files, is none of lengua's own or the system's;
- each part the lock records with the code is still the part it records, by the hashes of its
  idea.md and its main.wat ([Parts](#parts));
- environment variables are given only to an idea that uses the environment;
- what runs is the module `check` tested. The `run` tool builds main.wat again and compares the
  build's SHA-256 with the lock's. A shell's run loads that module's compiled form, which is kept
  in the data folder with the seal of the user's key as the module of that SHA-256 (The engine,
  above). Where no such file loads, `lengua-mcp run` takes the module `check` kept when its
  bytes hash to the lock's, or else builds main.wat and compares it; it compiles that module,
  seals it, keeps it and loads it. lengua-run leaves that run to lengua-mcp.

A build that differs from the tested one, as after lengua itself changed, doesn't run until
`check` tests it.

### The attester

`lengua-mcp check <folder>` attests a folder on any computer, as the idea's attester names it. It
needs no lock and no key, and writes no lock, no index.md, no log.md and no cache: only the
temporary folders its module and its tests run in. It checks lengua's last `verified` entry
against the idea's hash and main.wat's SHA-256; parses the idea and its tests; builds main.wat and
checks every rule; runs every test in workers of its own, from a fresh seed; and, when everything
else passed and the entry names both files, proves each generated test within 300 seconds, the
model held to 24 sample runs. A proof that doesn't finish leaves the folder attested; a confirmed
wrong one doesn't. It prints its lines, then `attested` (exit 0) or `not attested` (exit 1). It
holds a part's contract to its main.wat, as `check` does. It doesn't hold main.wat to the parts
an idea builds on, which only `check` does, on the computer where they are checked, and it
can't prove further.

## The knowledge graph

The program folders are a knowledge graph in the Open Knowledge Format (`src/graph/`). A folder's
bundle root is the nearest folder above whose `index.md` has `okf_version`, else the program
folder's parent. lengua writes no index.md or log.md at a filesystem's root, in the home folder,
in the system's temporary folder, or beside an `index.md` without `okf_version`, which isn't
lengua's.

`check` rebuilds the graph on every call: a breadth-first walk at most 8 folders deep and 20,000
entries, skipping hidden names, links, `.git`, `target` and `node_modules`. Nodes are folders that
hold idea.md; edges are `- Builds on:` links and the ideas' other links. It writes the files only
for an event (an idea recorded, a program done, a proof that found it wrong):
- `index.md`: `okf_version: "0.2"` and `# Programs`, one entry a program with its description and
  its code. In an index that exists, only lengua's entries change; every other line stays byte for
  byte.
- `log.md`: `# Update log`, dates newest first, an entry for each event. A request's words never
  appear in it.

Each file is written whole and renamed into place, one writer at a time: a lock per bundle in the
process, and a lock file in the data folder across processes. Every reply ends with how the graph
stands: a `- Builds on:` cycle through the folder blocks it; other cycles, broken links and pages
without a `type` are warnings; a program not built yet, stale, or an orphan is a note. One known
defect: the reply that first records a program reads the graph before index.md is written, so it
both says index.md lists the program and calls it an orphan (`src/graph/write.rs`). The files are
the graph: the server offers no MCP resource.

## The server and its tools

`lengua-mcp` speaks MCP over standard input and output through the Rust SDK, rmcp 3.8.0, with the
tools capability only: no resources and no prompts (`src/server.rs`). Logs go to standard error.

`check` takes `path`, the absolute path of the program folder; `request`, the user's own words,
at most 2,000 characters; and `prove_further`. Its reply holds the `state` and `next_step`, and
only when they say something: `problems`, `functions` and `data` (once main.wat builds: how
full its functions are by rule 4's count, and its named texts), `machine` (while the code isn't written: the engine,
the CPU's vectors and caches, and the features main.wat may use), `tests`, `proofs`, `parts` (each
part the program builds on, with the functions taken and what the part's lock records of its
proofs), `contract` (for a part, each function it offers), `memory`,
`example` (the idea's first example as the tests ran it), `performance`, `changes` (the requests,
how idea.md changed, and lengua's list of test changes that test less), `shell`, `graph`, and
for an outside module `outside` (that it is trusted, not proven, and what does not hold). Words
given with an idea that still has problems are held for that folder and recorded once it checks;
a restart loses them.

`run` takes `path`, `args`, `stdin` or `stdin_file` (a file in `folder`, at most 16 MiB, opened
once beneath the folder and read from the file that opened, so no name or link leads out and
nothing that takes its name afterwards is read), `folder`,
`timeout_seconds`, `env` and `stop`. Its reply gives standard output and error, the exit code, how
the program ended, its time on the clock, where a server serves, the connections lengua made, and
for an outside module `outside`, that what ran is trusted, not proven.
An idea that listens starts a server in the background, at most one per folder, reached at
127.0.0.1 on its port.

The command line (`src/main.rs`):

| Command | Does |
|---|---|
| `lengua-mcp` | The MCP server |
| `lengua-mcp run <folder> [--in <folder>] [--env NAME=value]… [[--] args]` | Runs a checked program attached to the shell. Exits with the program's code, 134 when it trapped, 127 when it never started, and 125 when lengua won't run it or can't start it |
| `lengua-mcp check <folder>` | The attester: 0 attested, 1 not |
| `lengua-mcp doctor` | Checks this computer: the server, the engine (a module it compiles and runs), the CPU, z3 and cvc5 and their pinned versions, whether the data folder and `PLUGIN_DATA` can be written, and the clocks' resolution. Exits 0 only when nothing is wrong |
| `lengua-mcp install [--check] [--link] [--prefix <folder>]` | Installs lengua from its unpacked release archive |
| `lengua-mcp wasm <main.wat> [-o <file>]` | Writes the module |
| `lengua-mcp measure …` | Runs one program in a process of its own and prints what it cost, for the benchmark |

`--data-dir <absolute path>` comes before the command. An argument that isn't Unicode is refused,
as not every system can pass one on.

## The benchmark and its gate

`cargo bench --bench against_rust_and_c` holds lengua's programs to C, C++ and Rust on the runner
it runs on (`benches/against_rust_and_c.rs`, `src/gate.rs`). There are 14 workloads: empty, add,
hypotenuse, upcase, upcase-stdin, shout, wc, histogram, crc32c, sort, tally, server, files and
readings. Each has lengua's program, which the bench takes from its idea to done as an agent does,
and six other versions from `benches/programs/`: C, C++ and Rust compiled to WebAssembly once, on
Linux on x86-64, by `cargo xtask bench-wasm`, and run in lengua's host with the same engine; and
the same three built natively with each system's own compilers. lengua's programs use SIMD128 by
hand; the others use their standard libraries and plain loops their compilers vectorize.

Each run is a process of its own, measured by `lengua-mcp measure` (`src/measure.rs`): CPU time,
wall time, peak resident memory and size. lengua's runs go through lengua-run, as a user runs a
checked program, with every check a run makes. The other modules run by the same lengua-run, in
the same engine, each from its compiled form kept under the user's key as lengua's is, and with
the same checks (decided 2026-11-11): each runs as an outside module (W23), as lengua runs one
for a user. `measure --idea` reads the module as `check` reads an outside module, keeps a folder
for it with lengua's idea and the `- Module:` line that names it, the module as main.wasm and a
lock the user's key signs, and the run reads that lock, holds the idea and the module to it
and takes its grants from the idea, by the code that runs any outside module's folder. The
finish line sets lengua against the other languages under the same runtime, and a module
lengua runs for a user always pays these checks. Each report's last section gives the same
modules run with no lock, which the gate doesn't judge.
`cargo xtask bench` builds lengua-run by itself first, with no compiler in it, as it ships. On
Linux a program starts from `lengua-spawn`, a small process that forks it, so its peak memory is
its own. The runs alternate between the versions, in an order shuffled from
round to round. On each row, lengua against one other version, a one-sided Mann-Whitney U test
decides CPU and wall time: the gate looks after 20, 40, 80, 160, 320, 640, 1,280 and 2,000 runs of
each version, each look at p < 0.00125, the 1% split over the eight looks, and a workload's rows
stop together at ten minutes. Peak memory and size are compared as measured.

The gate has a noise margin of 5% (`MARGIN` in `src/gate.rs`; the owner's decision of
2026-11-11), since the runners are shared machines and rows this close changed sides from run to
run. A measure has one of three verdicts:

- **Level:** the test doesn't decide it, or lengua's value is from 0.95 to 1.05 times the other
  version's, whatever the test decides. A row decided ahead at 0.98× is level, not ahead.
- **Ahead or behind:** the gap is past the margin, and for CPU and wall time the test decides it
  too. A row decided behind at 1.06× is behind.
- **Size** has no margin, as a file's bytes are the same on every run: a byte more is behind.

A row whose version couldn't be built is "not measured", and the bench fails on that version.

`KNOWN_BEHIND` names the rows where lengua is behind by more than the margin, and it may only
shrink: a run fails where lengua is behind on a row the list doesn't name, level or ahead on one
it names, and where a change in the list's history named a row the list before it didn't. A level
row needs no line and fails no run.

Each run writes a report, BENCHMARKS-<runner>.md, from its results alone. The latest run's five
reports are in [docs/benchmarks](docs/benchmarks/README.md): run 43191148225 at 610aaf5, the first
where the other modules run as outside modules, with lengua's checks. That run measured the
list as it was, 736 rows, and the gate passed on Linux on ARM64, the first pass on any runner.
On the other four it found 3 rows the list named level, which have left it, and 15 rows behind
by more than the margin that it doesn't name, all against native builds. KNOWN_BEHIND now names
733 rows. Against C, C++ and Rust compiled to WebAssembly they are 14: the size of upcase-stdin
against C's and C++'s modules on every runner, and wc's CPU and wall time against C's and
Rust's modules on macOS on ARM64. Of the 840 such rows a measure, lengua is ahead on 489, level
on 337 and behind on those 14, and none is behind and unnamed. Run 43084459708, where the other
modules ran with no lock, had 26 unnamed rows, 21 of them start-up time against C's and C++'s
modules. Against the native builds the list names 127 to
150 of each runner's 168 rows: tally is the one workload whose time is below the best native
build's on every runner, at 0.46 to 0.65 times. Windows counts CPU time in 15.6 ms ticks, so a
short run's CPU time there is a whole number of ticks, and most of its CPU rows against the other
modules are level. Every native C++ version was
built in that run, on all five runners: the bench passes `-std=gnu++17`, as the files are C++17
and Apple's clang and LLVM's clang on Windows take an older standard by default.

The margin has a limit the runs show. It takes the rows at 1.00× out of the gate, which were
most of what changed sides before. It doesn't settle a row whose gap sits near 5%, or one
counted in Windows's ticks: every one of this run's 15 unnamed rows is a row the list named
once, that a run found level and took out, and that is behind again. The list may not take a
row back, so with the list as it is these results fail the gate on four runners and pass it on
Linux on ARM64. todo.md names the rows ("A row the gate can't decide").

## The release and the install

`cargo xtask release` builds the archive of the runner it runs on, one of the five
(`xtask/src/release.rs`): `dist/lengua-<version>-<target>.tar.gz`, a `.zip` on Windows, with its
SHA-256 beside it. The archive is the plugin, ready to load: the skills and the manifests,
`server/lengua-mcp` and `server/lengua-run` (`.exe` on Windows), each built by itself, so that
lengua-run holds no compiler, `solvers/` with the z3 and cvc5 that `xtask/tools.tsv` pins for the
target, checked by SHA-256, the licences of everything it ships, `TARGET` and
`INSTALL.md`. Nothing is published or signed: the archives are unsigned, and publishing them is the
owner's.

`lengua-mcp install` installs lengua from the unpacked archive with the archive's own lengua-mcp,
the same way on every system (`src/install.rs`). It checks that the computer is the system and CPU
the archive is for, the hardware's own CPU even under macOS's Rosetta or Windows's x64 emulation,
and on Linux a glibc of 2.39 or later. It copies the plugin into a marketplace folder of lengua's
own, replacing only an earlier install of lengua's, adds it to Claude Code when the `claude`
command is there, and ends with doctor, whose exit code it keeps. A browser marks what it
downloads, and since the archive isn't signed, trusting it is the user's choice: the install says
how to clear the mark and clears nothing itself.

CI (`.github/workflows/ci.yml`) runs on GitHub's free runners: `ubuntu-24.04`, `ubuntu-24.04-arm`,
`macos-26`, `windows-2025` and `windows-11-arm`. A push that changes the code, to a branch not
named `wip/…`, runs the suite on all five; every Monday the suite, the slow ignored tests and the
bench run on all five; and a dispatch picks the targets and the mode: compile, test, ignored,
golden, bench or release. In release mode a clean runner of each system and CPU unpacks the
archive, installs it with its own `lengua-mcp install`, and checks it through a plain MCP client:
add from its idea to done, run through `run` and from a shell, by `lengua-mcp run` and by
lengua-run, which must say it holds no compiler and must load what `check` compiled with nothing
left to lengua-mcp, and attested. Each step of the workflow is one command, most of them calls
into the task crate (`cargo xtask`), and Rust 1.99.1
is pinned in `rust-toolchain.toml`. Nothing builds on the owner's computer: the format job hands
back the formatting patch and the resolved `Cargo.lock`.

## The threat model

The agent writes main.wat, and lengua treats it as untrusted code. What lengua trusts instead is
listed below.

### What lengua defends

- **A program that reaches more than its idea grants.** The checker refuses an import from outside
  WASI preview 1 and lengua's own modules (`lengua_net_1`, `lengua_fs_1`, `lengua_run_1`,
  `lengua_term_1`, `lengua_split_1`), one lengua doesn't give, one the idea doesn't
  allow, or one with another type.
  The host checks every call against the idea's lines, and every buffer a call borrows against the
  program's memory. Every path is resolved beneath the working folder by `cap-std`, and no
  symbolic link or junction is followed; a file with more than one name, a hard link, isn't
  opened, since its other name may be outside the folder (what that costs is with the host,
  above). The working folder is never lengua's data folder, its
  folder of compiled modules, or on Linux `/proc`, `/sys` or `/dev`, nor a folder that holds or
  lies inside one of them. A program can't connect or bind by itself unless its idea grants it:
  it gets the connections its idea names, made by lengua before it starts, and its one listener,
  and a named computer is never this one, by its name or by the address its name resolves to.
  Where the idea grants connections, names or datagrams of the program's own, lengua holds each
  to the grant (the host, above; the threat cases and their tests are in
  docs/reviews/reach-2026-11-11.md): a computer no line covers, by a redirect or anything else
  the program read; this computer under any spelling, a number an older parsing reads as an
  address and an IPv4 address inside an IPv6 one among them; a name that resolves to this
  computer, to a private network or to an address of many computers; a name that resolves anew
  between the check and the connection; more than 64 connections held, or one held with no byte
  crossing it for 300 seconds; a datagram to a group or a broadcast address, an answer to an
  address that sent nothing, and more than three times what a sender sent on a port any
  computer may reach. Where the idea grants a file or a folder an argument names, or a program
  to start, lengua holds each to the grant too (the threat cases and their tests are in
  docs/reviews/reach-2-2026-11-11.md): a path the program chose, which it can't give, since
  it gives an argument's place; an argument or a mode no line covers; lengua's own folders
  and the system's; a link at the last name, and a file with two names; a test's path that
  leaves the test's folder; a command no line covers, by one argument more or one changed; a
  value that starts as an option does; a shell, by its name or by the file its name resolves
  to; a program that lies where the program could have written it; the user's environment;
  a started program that runs past its time or prints past its room, with every process it
  started; and a terminal left raw, however the program ends. It gets environment
  variables only when its idea uses the environment, and only those its test or the user's `run`
  names. WebAssembly holds every access inside the program's own memory, of at most 1 GiB for
  main.wat, the engine's stack limit holds its calls, the engine's epochs stop it at its
  deadline, where its kind of run has one, and a host call looks at the deadline as it works.
  What lengua holds for a program outside its memory, such as its folders' listings, has a
  stated end.
- **A folder that someone hands the user.** `check`, the attester and `run` read a program
  folder's files only as regular files of a bounded size, follow no link there and open no pipe,
  so a handed folder can't make lengua read another file of this computer, wait for ever, or
  read without end. Its lengua.lock fails the HMAC. `check` writes lengua.lock and idea.md in the
  folder, and index.md and log.md in the folder's bundle, each by renaming a new file over the
  old, never through a link; the attester writes nothing there. A test's files are named inside
  its fresh working folder, and a test's program reaches only the test's own servers, opens no
  file outside that folder, and starts no program: a step says what a command gives.
- **A lock that isn't lengua's.** A lock lengua didn't write, or one changed outside lengua, fails
  its HMAC with the user's key, and `run` refuses the folder. So does a lock of another layout, a
  format lengua doesn't read, or proofs about another module.
- **A function that isn't the part's.** A function main.wat says it takes from a part is held to
  that part at every check: its text, the constants it reads and the functions it calls. A part
  that changed, has no contract, or that a proof found wrong is refused, and so is the program
  built on it. lengua trusts no part for being in its library: each is checked on the computer
  that builds on it.
- **A build that isn't what was checked.** `run` runs only the module whose SHA-256 the signed lock
  records: the `run` tool builds main.wat again and compares, and a shell's run loads the compiled
  form the user's key sealed as that module. An idea or code changed since the record, a module
  a proof found wrong, and a build that differs after lengua changed are refused. A module found wrong stays refused when an
  earlier lock, which the user's key signed too, is put back: the data folder keeps the verdict.
- **Machine code that lengua didn't compile, in a run from a shell.** A run from a shell loads
  compiled code from one file, and only when that file carries the seal of the user's key for
  the module its lock records. `compile_sealed` alone makes a seal, over what the engine's
  compiler returned in that call. So a file put in the data folder by someone without the key
  is never handed to the engine: not other bytes, not another program's compiled module, and
  not the right one with a byte changed. Such a run reads no cache of compiled modules.
- **Work split into pieces that goes past its grant, or gives another result on another
  computer.** A piece is a run of its own with memory of its own, so no piece reads or writes
  what another or the program holds; it reaches standard input and its output alone; it can't
  split, so a run never has more pieces at once than its idea's line grants, nor more memory
  with them than 1 GiB; a call outside the line is refused; the run's deadline stops its
  pieces where the engine can stop a program; and the answers come in the pieces' order, the
  run ending as the first piece in that order that didn't end well, so the cores and the order
  the pieces end in change nothing but the time. A program that splits names no secret. The
  threat cases and their tests are in docs/reviews/threads-2026-11-11.md.
- **A secret that reaches timing within main.wat.** For an idea with a `- Secret:` line, the secret
  rule refuses a branch, an address, a length, a loop's count, a division or a float operation
  that depends on a secret, in main.wat; and in the machine code the engine made on this CPU for
  each operator that takes a secret, a branch, a table, a call or a division. A run from a shell
  reads the code of the very module it loaded and is about to run. A secret never names a
  computer or a port a program opens, and since no branch or loop depends on one, neither does
  how many connections a program opens, nor when.
- **A module lengua didn't build, and that passed no rule.** An outside module, `main.wasm`, is a
  compiler's output or anything written to look like one. `check` reads it before anything of
  it is compiled, and refuses what the host can't hold by itself (An outside module, above).
  What it takes, the host holds to the idea's grants at each call, to its own memory, to the
  size it declares, to the engine's stack and, in a test and in the `run` tool, to the
  deadline. It can't make a reply say more than "trusted, not proven": the state, the lines
  and the lock's stamp come from the idea's `- Module:` line, which the lock records, and never
  from anything the module prints or exports. The threat cases and their tests are in
  docs/reviews/outside-2026-11-12.md.

### What lengua trusts

- **The engine and its compiled code**: Wasmtime 50.0.1 and Cranelift for every bound, the stack,
  fuel and epochs; `wasmparser` 0.261.0 and `wast` 261.0.0 to read main.wat as the engine does;
  and the machine code Cranelift makes. For timing, the code made for operators that take a
  secret is read (Secrets, above), and the rest is trusted: the CPU's own timing of the
  instructions left, the code of functions and operators that take no secret, and the engine's
  own functions. No
  OS sandbox wraps a worker: a worker is a plain child process, and a second wall (seccomp and
  Landlock on Linux, each other system's own) is deferred.
- **The solvers**: z3 4.16.0 and cvc5 1.3.4, an "impossible" counting only when both say it, but
  for float questions, which cvc5 answers alone. lengua's evaluator checks the inputs a solver
  finds, and the model is held to the real program on 24 sample runs of each block and by the
  suite's tests.
- **The operating system**: its process isolation, its file system under `cap-std`, its sockets,
  and its clock and random bytes; and, for what a program opens by itself, its resolver: lengua
  checks the addresses the resolver gives, whatever it asked to find them (DNS, the hosts file,
  a local network's own names).
- **The user's own folders**: the data folder, which holds the key, the recorded ideas, the
  modules `check` kept and what proofs found wrong, and the cache of compiled modules, which
  `PLUGIN_DATA` may place elsewhere: whoever sets that variable names a folder lengua runs machine
  code from. Whoever can read the key, or write the
  data folder, can sign locks lengua trusts: the lock's HMAC is a record of what this user's lengua
  checked, not a wall against the user's own account. Wasmtime 50.0.1 loads a cached module's
  machine code as it finds it, checking only that it suits this engine, so the cache folder is
  trusted as the key is by what reads it: the tests' workers and the `run` tool. A run from a
  shell reads no cache. It trusts the key alone for the code it loads, and whoever holds the key
  can seal code of their own.
- **lengua's own code**: lengua's crate forbids unsafe code; the calls that need it sit in
  `crates/lengua-os`, each in a safe function with a test: the few into the operating system,
  tested on their system's runner, and the load of a sealed compiled module, which hands the
  engine nothing without the seal. TLS is rustls with ring; the solvers, ring, zstd and a file of
  Wasmtime's carry C (todo.md's decisions).

### Out of scope

Programs that need these can't be written in lengua yet, and the host has no way to give them:
- threads that share memory: a program uses more cores through pieces that share none (the
  host, above). What that leaves out: pieces of another program than itself, a piece with
  arguments or files of its own, a piece that splits, work whose parts must talk while they
  run, a proof of the step that joins the answers, which is tested, and a secret in a program
  that splits;
- for a started program: a current folder other than an empty one of lengua's, so a tool
  that works on the user's folder (`git status`) can't be started usefully yet; an
  environment variable; its standard error; more than one at a time, or one that goes on
  while the program does; and a path as a value where it starts with `/`;
- for the terminal: the mouse, and a signal when the window's size changes;
- on the network: a connection a program opens to this computer (it reaches this computer only
  through a connection lengua makes before it starts); a TCP listener and a UDP port in one
  program, or more than one of either; raw sockets, ICMP and multicast; and a proof of a
  program that opens its own connections, files or programs, or uses the terminal's
  functions, which is tested, not proven;
- GUIs, and libraries for other languages to call: no plan yet.

The proofs and the rules have their own limits, said above: what the model refuses is tested, not
proven; an access the rules leave is proven only inside the program's memory; the secret rule
covers main.wat, and of the machine code only what the engine made for operators that take a
secret.

### Covert channels it narrows

Both clocks are rounded down to a microsecond, after OpenAI's 2026 report of agents signalling to
each other through the CPU cache's timing, so a program's clock can't carry finer timing. That
narrows the channel and doesn't close it: a program whose idea uses the clock can still time its
own work to the microsecond.

### What the safety filter stopped

Three items here were stopped by Claude's safety filter (todo.md). The soundness review of the
secret rule was done on 2026-11-10 (docs/reviews/secret-rule-2026-11-10.md): its five findings
have tests and are closed. The review of the host was done the same day
(docs/reviews/host-2026-11-10.md): its eleven findings have tests (tests/host_hard.rs and unit
tests) and are closed, and what it left open is in todo.md. W13, the check of the engine's
machine code for an idea's secrets, was built on 2026-11-10 (Secrets, above). The third item, the
tests that try to get past the sandbox on macOS and Windows, was tried as that review of the host
and its tests, which run on all five runners. None of the three was stopped again. What the two
reviews left open was closed on 2026-11-11, each gap with a test that failed first (todo.md, "The
gaps the two reviews left open"): among them a test of a junction on the Windows runners and of
names that differ in case or normalization against the macOS runner's file system. lengua isn't
to be presented as covering
security because of its examples or its checksums: they show algorithms, not a product that
resists an attacker.

## The standards it follows

- Agent Plugins 1.1.0: `plugin/plugin.json` and `plugin/mcp.json`, with Claude Code's mirrors in
  `plugin/.claude-plugin/plugin.json` and `plugin/.mcp.json`, checked against the vendored schemas
  by `tests/manifests.rs`.
- Agent Skills: the two skills' fields, names and lengths, checked by `tests/skills.rs`.
- MCP, through the Rust SDK rmcp: tools only, structured replies held to each tool's output schema.
- The Open Knowledge Format v0.2: idea.md an `Attested Computation`, the `verified` record, and a
  bundle's `index.md` and `log.md`.
- Karpathy's idea files and LLM Wiki: idea.md is the source people keep and share, and index.md
  and log.md are the wiki's index and log, which `check` keeps.
- ASD-STE100 Simplified Technical English, for idea files, in lengua's own words.
- WASI preview 1, as lengua's host gives it.
- NASA's Power of 10 rules, adapted to WebAssembly, with the memory rules M1 to M8.
