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
2. The code: main.wat builds, the functions it copies from its parts match them, and it follows
   every rule.
3. The tests, then the proofs, in a background job for the folder, keyed by the hashes of its
   files. A `check` on changed files cancels the old job.

Its reply's `state` is one of `write_idea`, `idea_problems`, `needs_request`, `write_code`,
`code_problems`, `testing`, `tests_failed`, `proving` and `done`, with one `next_step`. A reply
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
    port it listens on, for one client at a time or for up to N at once.
  - `- Secret:` the inputs whose values must not decide how it runs: standard input, a named file,
    an environment variable or an argument.
  - `- Builds on:` links to the idea.md of each part it is made from.
  - `- Threads:` still parses, so that `check` can refuse it: a WebAssembly program runs as one
    thread.

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
must match the part's function word for word, comments and spacing aside (`src/wasm/parts.rs`).

`BUILDER` names a build: a hash of lengua's version, the build's source, the library and
Cargo.lock, which pins `wast` and the encoder exactly. `check` keeps each module whose tests passed
in the data folder under its builder and its SHA-256. It names a build and secures nothing: what
runs is held to the module hash in the signed lock.

`lengua-mcp wasm <main.wat> [-o <file>]` writes the module, with no rules, tests or lock: a plain
WASI preview 1 module, which any WASI runtime runs when it reaches only standard streams and files.

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

The memory rules work over regions: an immutable global `$x` holding an address, with `$x.size`
(writable) or `$x.rosize` (read-only). Regions start at address 1024 or above, inside the memory,
and don't overlap.
- M1, owners: every address comes from a region or a pointer a caller lent, never a plain number.
- M2, bounds: every access stays inside its owner; a computed one is rule 9's.
- M3, lifetimes; M5, initialization; M7, races: these hold by construction (locals can't be
  addressed and nothing is freed; memory and locals start at zero; there are no threads), and the
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
listing turned round, and a different result fails, with advice to sort. Each run is compared on
standard output and error (exactly, or by a rule's pattern), the exit code, and the working
folder, which must hold the expected files and nothing else. An exit of 134 is read back as
"assertion N failed at main.wat:LINE".

Once every test passes, each test's largest input runs alone 5 times, and the done reply gives
each test's cost: the WebAssembly operations it executes, which the engine counts as fuel, the
same on every system, and its time on the clock.

## The prover

`check` proves each generated test on a model of the built module (`src/prove/`). A block's line
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
with it. From any state the loop can be in at its head, one more buffer-full must keep what it
writes and counts in step with the formula; the base proves every input of up to a buffer-full.
A program that reads with `fd_read` itself, reads in several places, waits on `poll_oneoff` or is
on the network is proven by length only.

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

It refuses, as "tested, not proven": tables, element segments and a start function; recursion, and
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
Cranelift compiles a module the first time it runs on a machine, for that machine's CPU, and
Wasmtime's cache keeps it, in the `cache` folder of the plugin's data folder when the agent's
harness gives one (`PLUGIN_DATA`, or `CLAUDE_PLUGIN_DATA`), else of lengua's own. There is no
separate compile ahead of time, and the features are one set on every machine.

Three setups:

| Setup | Used by | Counts operations | Stops at a deadline |
|---|---|---|---|
| Test | the tests | yes, as fuel | yes, through the engine's epochs, every 10 ms |
| Run | the `run` tool and a background server | no | yes |
| Shell | `lengua-mcp run`, the benchmark and `doctor` | no | no |

### The host

lengua gives WASI preview 1 itself, on the engine's linker, so its semantics are its own, the same
on every system, and the prover models one implementation (`src/wasm/host/`, `src/wasm/wasi.rs`).
All 46 functions of preview 1 are linked, so a toolchain's module links whatever it imports. 30
are given. 16 answer `NOSYS`: `fd_advise`, `fd_allocate`, `fd_datasync`, `fd_fdstat_set_flags`,
`fd_fdstat_set_rights`, `fd_filestat_set_size`, `fd_filestat_set_times`, `fd_pread`, `fd_pwrite`,
`fd_renumber`, `fd_sync`, `path_filestat_set_times`, `path_link`, `path_readlink`, `path_symlink`
and `proc_raise`. Every error is one of a closed set of 21 of WASI's numbers, and an error outside
a function's own list becomes `IO`.

Each call checks the idea first: one the idea doesn't allow answers before any of its buffers is
looked at. An allowed call checks every buffer it borrows against the program's memory, then does
its work. The idea's lines decide what is allowed (`src/wasm/host/policy.rs`):
- every program: its arguments, `fd_write`, `fd_fdstat_get`, `proc_exit` and `sched_yield`;
- `standard input` or `the terminal`: reading standard input, and `poll_oneoff`;
- `files`: the working folder and the files in it;
- `the clock`: the clocks, and `poll_oneoff`'s waits on time;
- `random bytes`: `random_get`;
- `the environment`: the variables it is given;
- any `- Network:` line: its connections; `listens on`: `sock_accept` on its listener.

A call the idea doesn't allow answers `NOTCAPABLE` and does nothing, except five that toolchains'
startup code calls, which answer harmlessly: an empty environment, no folder, and zeros for random
bytes. main.wat itself can't import a function its idea doesn't allow: the checker refuses it.

Descriptors are fixed by role: 0 to 2 the standard streams, 3 the working folder (named `.`, only
for an idea that uses files), 4 the listener, and 5 on the connections lengua made, in the idea's
order. A program holds at most 4,352 descriptors.

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

The network: a program never connects or binds by itself. Before it starts, lengua makes each
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
toolchain's module, which the benchmark runs, is no main.wat and may declare up to 4 GiB. A
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

`lengua-mcp run` runs the program in its own process, on a thread of its own, attached to the
shell. The engine starts on that thread while the checks run on another, and nothing is compiled
until the checks pass.

## The lock, the key and trust

### lengua.lock

Only lengua writes it, atomically, and it is canonical TOML ending in a `[signature]` table
(`src/lock.rs`). lengua writes format 7 and reads 6 and 7; a format before 6 is the native
lengua's, and lengua says so. It holds, in order: the version; the dialect, `webassembly 1`; a
fingerprint over the idea's and the code's hashes, once code is stamped; `[request]`, the user's
words (at most 6 a build, the first and the latest, each at most 2,000 characters) and the idea's
hash when the build began; `[idea]`, idea.md's hash; `[code]`, main.wat's SHA-256, the module's,
the engine (`wasmtime 50.0.1`), the deepest stack the rules count, the unsafe blocks, the target
`webassembly` and the features used beyond WebAssembly 1.0; `[proofs]`, about that module: the
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
- environment variables are given only to an idea that uses the environment;
- what runs is the module `check` tested. The `run` tool builds main.wat again and compares the
  build's SHA-256 with the lock's. A shell's run takes the module `check` kept in the data folder
  when its bytes hash to the lock's, and otherwise builds main.wat, compares it, and keeps it.

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
doesn't check the parts an idea builds on, which `check` does, and it can't prove further.

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
only when they say something: `problems`, `machine` (while the code isn't written: the engine,
the CPU's vectors and caches, and the features main.wat may use), `tests`, `proofs`, `memory`,
`example` (the idea's first example as the tests ran it), `performance`, `changes` (the requests,
how idea.md changed, and lengua's list of test changes that test less), `shell` and `graph`. Words
given with an idea that still has problems are held for that folder and recorded once it checks;
a restart loses them.

`run` takes `path`, `args`, `stdin` or `stdin_file` (a file in `folder`, at most 16 MiB, opened
once beneath the folder and read from the file that opened, so no name or link leads out and
nothing that takes its name afterwards is read), `folder`,
`timeout_seconds`, `env` and `stop`. Its reply gives standard output and error, the exit code, how
the program ended, its time on the clock, where a server serves, and the connections lengua made.
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
wall time, peak resident memory and size. lengua's runs go through `lengua-mcp run`, with every
check `run` makes. On Linux a program starts from `lengua-spawn`, a small process that forks it, so
its peak memory is its own. The runs alternate between the versions, in an order shuffled from
round to round. On each row, lengua against one other version, a one-sided Mann-Whitney U test
decides CPU and wall time: the gate looks after 20, 40, 80, 160, 320, 640, 1,280 and 2,000 runs of
each version, each look at p < 0.00125, the 1% split over the eight looks, and a workload's rows
stop together at ten minutes. Peak memory and size are compared as measured. A row the test can't
decide is undecided, never level.

`KNOWN_BEHIND` names the rows where lengua isn't ahead yet, and it may only shrink: a run fails
where lengua is behind or undecided on a row the list doesn't name, or ahead on one it names, and
where a change in the list's history named a row the list before it didn't. Each run writes a
report, BENCHMARKS-<runner>.md, from its results alone. The latest run's five reports, from the
finish-line run 42947294489 at main 33ed7fe, are in [docs/benchmarks](docs/benchmarks/README.md).
In that run the gate passed on no runner. It found lengua ahead on 20 rows the list named, which
have left it, and behind or undecided on 22 rows it doesn't name. KNOWN_BEHIND now names 1,037
rows. Against C, C++ and Rust compiled to WebAssembly they are 46 on Linux on x86-64, 31 on Linux
on ARM64, 48 on macOS on ARM64, 66 on Windows on x86-64 and 66 on Windows on ARM64, with peak
memory the most common measure among them. Against the native builds they are 152 to 160 of each
runner's 168 rows: tally is the one workload whose time is below the best native build's, on
every runner, at 0.46 to 0.69 times. Windows counts CPU time in 15.6 ms ticks, so a short run's
CPU time there is a whole number of ticks. Against the other modules, 37 CPU rows of the two
Windows runners were undecided in that run and 4 behind, each at 0.99× or 1.00×; the native
builds' shortest runs read no ticks at all.

The ratchet has a limit the runs show. Each of the 22 rows was ahead in an earlier run, which is
why the list doesn't hold it, and 14 of them left the list one run before. They are CPU time and
wall time of short workloads and peak memory, at 0.99× to 1.07×. Such rows move between ahead and
behind from run to run, and the list may not take a row back, so the list alone doesn't bring
the gate to pass. What to do with them is the owner's (todo.md, "The owner's").

## The release and the install

`cargo xtask release` builds the archive of the runner it runs on, one of the five
(`xtask/src/release.rs`): `dist/lengua-<version>-<target>.tar.gz`, a `.zip` on Windows, with its
SHA-256 beside it. The archive is the plugin, ready to load: the skills and the manifests,
`server/lengua-mcp` (`.exe` on Windows), `solvers/` with the z3 and cvc5 that `xtask/tools.tsv`
pins for the target, checked by SHA-256, the licences of everything it ships, `TARGET` and
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
add from its idea to done, run through `run` and from a shell, and attested. Each step of the
workflow is one command, most of them calls into the task crate (`cargo xtask`), and Rust 1.99.1
is pinned in `rust-toolchain.toml`. Nothing builds on the owner's computer: the format job hands
back the formatting patch and the resolved `Cargo.lock`.

## The threat model

The agent writes main.wat, and lengua treats it as untrusted code. What lengua trusts instead is
listed below.

### What lengua defends

- **A program that reaches more than its idea grants.** The checker refuses an import from outside
  WASI preview 1, one lengua doesn't give, one the idea doesn't allow, or one with another type.
  The host checks every call against the idea's lines, and every buffer a call borrows against the
  program's memory. Every path is resolved beneath the working folder by `cap-std`, and no
  symbolic link or junction is followed; a file with more than one name, a hard link, isn't
  opened, since its other name may be outside the folder (what that costs is with the host,
  above). The working folder is never lengua's data folder, its
  folder of compiled modules, or on Linux `/proc`, `/sys` or `/dev`, nor a folder that holds or
  lies inside one of them. A program can't connect or bind by itself: it gets the connections its
  idea names, made by lengua before it starts, and its one listener, and a named computer is
  never this one, by its name or by the address its name resolves to. It gets environment
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
  its fresh working folder, and a test's program reaches only the test's own servers.
- **A lock that isn't lengua's.** A lock lengua didn't write, or one changed outside lengua, fails
  its HMAC with the user's key, and `run` refuses the folder. So does a lock of another layout, a
  format lengua doesn't read, or proofs about another module.
- **A build that isn't what was checked.** `run` runs only the module whose SHA-256 the signed lock
  records: the `run` tool builds main.wat again and compares, and a shell's run compares the kept
  module's bytes. An idea or code changed since the record, a module a proof found wrong, and a
  build that differs after lengua changed are refused. A module found wrong stays refused when an
  earlier lock, which the user's key signed too, is put back: the data folder keeps the verdict.
- **A secret that reaches timing within main.wat.** For an idea with a `- Secret:` line, the secret
  rule refuses a branch, an address, a length, a loop's count, a division or a float operation
  that depends on a secret, in main.wat; and in the machine code the engine made on this CPU for
  each operator that takes a secret, a branch, a table, a call or a division.

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
  and its clock and random bytes.
- **The user's own folders**: the data folder, which holds the key, the recorded ideas, the
  modules `check` kept and what proofs found wrong, and the cache of compiled modules, which
  `PLUGIN_DATA` may place elsewhere: whoever sets that variable names a folder lengua runs machine
  code from. Whoever can read the key, or write the
  data folder, can sign locks lengua trusts: the lock's HMAC is a record of what this user's lengua
  checked, not a wall against the user's own account. Wasmtime 50.0.1 loads a cached module's
  machine code as it finds it, checking only that it suits this engine, so the cache folder is
  trusted as the key is.
- **lengua's own code**: lengua's crate forbids unsafe code; the few calls into the operating
  system that need it sit in `crates/lengua-os`, each in a safe function with a test on its
  system's runner. TLS is rustls with ring; the solvers, ring, zstd and a file of Wasmtime's carry
  C (todo.md's decisions).

### Out of scope

Programs that need these can't be written in lengua yet, and the host has no way to give them:
- threads, until WebAssembly's threads are standard: W20 plans structured fork-join that the rules
  show free of data races;
- connections a program opens itself, at run time, which need WASI's newer sockets; UDP; DNS;
  starting other programs; files outside the working folder; and the terminal's raw mode and
  window size: W18 plans each as a grant the idea declares and the host enforces;
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
