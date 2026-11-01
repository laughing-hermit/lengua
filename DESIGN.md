# lengua design

This file records every design decision, so any session or agent harness can pick up the project.
Where the code and this file disagree, fix one of them.

## Thesis

Programming languages and compilers were made as human middlemen to machine code. With AI, the
natural-language **spec is the source code**, the AI translates it, and tools check the result.
lengua aims to make assembly a general-purpose "language that isn't a programming language". It's
built for AI agents: the person describes what they want, and the agent produces the fastest
machine code it can.
In short: turn the spec with its tests into machine code, using Claude Code (or any agent) as the
compiler. lengua's AI writes **assembly**: readable for review, and turned into machine code by a
deterministic assembler that only encodes each line as one instruction and makes no choices of its
own. The guarantees a compiler used to give come from rules and tests instead.

## A lengua program

A program is two files, plus lengua's own record of them:

| File | Written by | Purpose |
|---|---|---|
| `spec.md` | the AI, from the person's request | The source of truth, the mother of everything else, and the only file people share: natural language, Given/When/Then behaviors with IDs whose steps are their example tests, the generated tests under each behavior, and what the program uses. An [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format) (OKF) concept document, the same on every target |
| `main.s` | the AI, compiling the spec | x86-64 assembly in the lengua dialect for one target. It's disposable: each computer compiles its own from the spec. Once it passes, lengua writes its second line, naming the version of the spec it was compiled from, the target and the CPU features it needs |
| `lengua.lock` | only the lengua server | This computer's record: the person's requests for the build in progress, SHA-256 of both files and of the checked executable, the target and features, a combined fingerprint, which tests are proven for every input on that executable, and a signature. It isn't shared |

**Workflow:** request → spec, tests included (recorded with the person's words; nothing waits for
an approval) → code (stamped once every rule and test passes) → proof → run, and optimize (tests
stay green). A program only runs when the lock matches both files, so documentation, tests and
code can't drift from the spec. The person asks once; the agent's report then shows the spec and
every change after the request.

**Specs follow behavior-driven development (BDD).** Purpose says what the program does, for whom
and why. Each behavior is one rule, titled in the user's words and shown by one concrete example:
Given sets the scene (arguments, input, files, the clock), the one When is the program running,
and Then states only what the user observes (output, files and the exit code). The steps use a
fixed set of phrasings, as Gherkin's step definitions do, so lengua reads each behavior's steps as
its example test and nothing is written twice; a step it can't read is a problem in the spec,
reported with the phrasing to write. A behavior's generated tests, which try the same rule with
fresh inputs on every run, are a Gherkin Scenario Outline: under a `###` heading after its steps,
the same steps with `{placeholders}`, then a markdown table with a column for each variable's
generator and a row for each variant. So the whole spec is plain markdown, headings, lists and
tables, which OKF favors, with no other language inside. Steps describe behavior, never
implementation, and a spec names no CPU, operating system or toolchain: it reads the same on
every computer. `spec.md` is the living documentation, and the lock keeps the code in step with
it. The parser enforces the structure (a Purpose, Given before one When before Then, one exit
code, unique IDs and titles, generated tests under their behavior with a well-formed table, and
only `- Uses:`, `- Network:` and `- Threads:` under Constraints); the write-spec skill teaches the
style.

**A spec names what the program uses, not system calls:** `- Uses:` lists `standard input`,
`files`, `the clock`, `random bytes` and `the environment`, `- Network:` the ports it connects to
or listens on, and `- Threads:` how many cores it may use. Each target maps them to its own
system calls, so the same spec means the same on every computer. A program gets environment
variables only through `the environment`, and only those its test or the user's `run` names. They
reach the program alone: lengua's launcher, which runs before the program's confinement, gets
them as an argument and sets them only as it becomes the program, so a variable such as
`LD_PRELOAD` can't reach lengua's own code.

| What changed | Result |
|---|---|
| `spec.md` | The next `check` records it under the request's words and checks its tests and the code against it again. Changes since the build's first request are shown, with lengua's own marks on test changes that test less |
| `main.s` only | The next `check` checks the rules, runs every test and proves it again; once the tests pass, the new code is stamped |
| main.s's second line only | Nothing: lengua's hash of main.s leaves its own line out, and writes it back after the next pass |
| nothing | `run` works, and `check` answers from what it found |

## What ships

The plugin folder `plugin/` contains:

- **Skills:** `write-spec` writes the spec, its tests included, and `compile` compiles it into
  `main.s` for this computer. Each teaches one file and ends with `check`.
- **MCP server** `lengua-mcp` (Rust, official `rmcp` SDK) with two tools:

| Tool | Does |
|---|---|
| `check` | Every stage, stopping at the first that needs work: validates `spec.md`, its tests included (coverage, cross-checks, red check), and records it with the person's words; assembles, links and checks the rules; then, in the background, runs every test, counts the instructions executed, stamps and signs, proves the program, and writes main.s's second line. Each reply has one `next_step` |
| `run` | Verifies the lock against every file, rebuilds, confirms the build is byte-identical to the checked one, then runs it |

The agent's loop is one call: edit a file, call `check`, follow `next_step`. The person's words go
with the first call, and with a new request; an edited spec is recorded under the words already
recorded. Words given with a spec that has problems are kept for that folder and recorded once it
checks, so they're given once. Nothing has to be copied between files: the lock already records
which spec the code passed. `check` replies within about 40 seconds,
because other agent harnesses cut off long tool calls (many stop waiting at 60 s). The tests and
the proof run in a background job per program folder, keyed by the hashes of its two files: a
`check` with the same files waits on the same job, a `check` with new files cancels it and starts
another, and a server restart starts it again, keeping the proofs the lock records about the same
build. `check` says done once the tests pass and 20 seconds more have gone by: proofs still
running are listed with how far each has got, go on for up to five minutes, and are recorded as
they end, so a long proof never holds up the report, and `run` refuses a build that a proof has
found wrong. Problems come with their fixes worked out: the phrasing to write a step in, the line
a network needs with its port, generated tests to start from, and for a function over the limit,
the loop or run of code to make a function of its own. Replies carry each fact once, where the next step needs it: the machine's cores and CPU
extensions while `main.s` is unwritten, the changes after the request and the cost of each test
only once done, and the spec's first example as the tests ran it, so the report needs no demo run.

Only open standards are used: MCP (tools and server instructions), Agent Skills and Agent
Plugins. No hooks and no subagents, so the plugin works the same in Claude Code, Codex, Cursor
and other harnesses. Every guarantee lives in the server, which detects changes and refuses to stamp
or run.

## Packaging

| File | For |
|---|---|
| `plugin/plugin.json`, `plugin/mcp.json` | The [Agent Plugins](https://agent-plugins.org) standard (Codex, Cursor, Copilot, Kiro, VS Code) |
| `plugin/.claude-plugin/plugin.json`, `plugin/.mcp.json` | Claude Code, which ignores the standard's two files |

Both MCP configs start the same binary, `plugin/server/lengua-mcp` (built by
`scripts/build-plugin.sh`, not committed). `scripts/release.sh` builds the release archive,
`dist/lengua-<version>-x86_64-linux.tar.gz`, with a SHA-256 beside it: the plugin with its
server built, `install.sh` and a short README, so installing needs no Rust toolchain. The server
needs glibc 2.39 or later. `install.sh` checks for that, LLVM's assembler and linker and the two
solvers, prints the `dnf` or `apt` line for anything missing, and copies the plugin to
`~/.local/share/lengua/plugin`; `--link` puts `lengua-mcp` in `~/.local/bin`. The script then
checks the archive as a user gets it: unpacked in a fresh folder, installed, validated, and
`examples/add` taken from its spec to done through the archive's own server over MCP
(`scripts/release-check.py`), then run from a shell with the done reply's command. Nothing is
published. A test holds the crate's version and both manifests' to one. The server keeps its private data in
`~/.local/share/lengua` (or `$XDG_DATA_HOME/lengua`): one signing key per user, shared by every
harness, so a lock recorded in one harness verifies in all of them.

## Tests

Each behavior's steps are its example, and its generated tests, one a row of its table, draw new
inputs on every run (boundary values first, then seeded random ones) and compute the expected
results with formulas. A row whose ranges hold at most 5000 combinations runs every one of them
instead, within 20000 runs per spec: for those inputs the program is checked, not
sampled, which is the first step toward the proof below. The formula language has checked 128-bit
whole numbers, strings, comparisons, `&& || !` and the functions `len`, `abs`, `min`, `max`, `if`,
`upper`, `lower`, `reverse`, `count`, `sha256`, `crc32` and `crc32c`. Tests can give a program
files (`files`) and check the exact files it leaves (`files_after`): after a run its working
folder must hold exactly those, so a stray file or a changed input fails. Decimals come in two kinds.
Exact decimals (`decimal(0.00, 999.99)` inputs, money) are computed in whole units with `scaled` and
printed with `places`, so any rounding rule the spec states is exact. Measured decimals are computed
in doubles exactly as `addsd`, `mulsd`, `divsd` and `sqrtsd` do, or in singles where the formula
says `single(x)`, with `fma(a, b, c)` for a fused multiply-add, and printed with `fixed` (x·10^d
rounded ties to even, like `cvtsd2si`); whole numbers and decimals, and singles and doubles, never
mix silently in arithmetic.
It has no loops, so every formula finishes. Tests can supply the program's random bytes
(`random`, in hex, with `hex(x, n)` filling bytes from a variable) and clock readings (`clock`, in
seconds), so output that depends on them is exact: a variable that only fills random bytes draws
every value they can hold, as the kernel's could, and bytes asked for past the supply are zeros,
so a program may draw in batches of any size. Where a test doesn't supply them, a rule stands for
what the program chooses: `{any(lo, hi)}` is any whole number in that
range, written plainly, and `now` (the second a run starts) may bound one. Inside lengua a rule
travels in the expected text as a marker between Unicode noncharacters, which a spec can't hold,
so expected output stays text everywhere and only matching reads the markers.

`check` ties the generated tests to the behaviors above them: generator limits are numbers the
spec's Purpose states, and args and stdin hold bare variables so each behavior's example can be
read back into variable values and every formula must reproduce it. A dry run evaluates every
run's expected results, so overflow and division by zero are caught before any code exists, and
output with placeholders must vary, so a program that prints a constant can't pass. Several rows
may cover one behavior (divisors on both sides of zero, for example), and they share its steps by
construction. A wrong test can be fixed at any time: it's part of the spec, so every
version is checked again, and a fix can't drift from it. The reply shows the line diff since the
spec the build started from, and lengua's own marks on the changes that test less
(fewer runs, a generator that draws fewer values, exact output turned into a rule, a removed
row, variable or expected file). The same AI writes the code and the tests, so that judgment
stays in lengua; the agent only explains each change in its report. The server keeps a copy of
every recorded spec by hash, so the changes can be shown.

The examples in the skills are checked by the test suite against lengua's own checks, so the
documentation can't drift from the checker.

## Running programs

Tests and `run` start a program the same way: through the server's own binary in launcher mode,
which waits until the server has attached a CPU performance counter that turns on at `exec`, then
replaces itself with the program (no `unsafe` code needed). The counters follow every thread the
program starts, and the runner also measures time on the clock, which is what threads shorten.
The program gets its stdin up front
and then the end of input, an empty environment, a working folder of its own (a fresh temporary
one, or for `run` a folder of the user's files), a time limit (2 s per test, 10 s for `run`) and
8 MiB of output, and it's killed past either.

Before it becomes the program, the launcher confines itself, so the kernel enforces what the
spec allows:
- Landlock lets the program touch files only inside its working folder, execute only itself, use
  no network but the ports its spec names, and signal no other process;
- Landlock's network rules know ports, not computers, so for a program that may reach only this
  computer, which is every program whose spec doesn't say `on any computer`, lengua's supervisor
  stops each `connect`, reads where it goes, and refuses any computer but this one with
  `EACCES`, as Landlock refuses a port. The port list is then never a way off this computer.
  Workers make no system calls and write only their own slots, so no other thread can change the
  address between the check and the call;
- seccomp kills it at any system call its spec doesn't list (plus `write` and `exit`, which
  `assert_fail` may always make);
- resource limits cap the files it writes (64 MiB in tests) and leave no core files.

The checker already proves the code makes no other system calls; the kernel makes the promise hold
even if the checker missed something.

When a test supplies random bytes or clock readings, a second process of the server's binary, the
supervisor, traces the launcher with ptrace before it becomes the program. The launcher installs
one more seccomp filter first, which stops only `getrandom` and `clock_gettime` for the
supervisor; the supervisor writes the test's bytes or reading into the program's memory with
`process_vm_writev` and skips the kernel's call, so the program sees the test's values. On
AArch64 a call can't be skipped without a register set nix gives no safe way to write
(`NT_ARM_SYSTEM_CALL`), so the supervisor makes the kernel's call harmless instead (`getrandom`
for no bytes, a clock that doesn't exist, descriptor -1 for a refused `connect`), lets it return
at once, and at its exit puts the answer in x0 and the count it changed back in x1; every other
register stays as the program left it. A test of a program that may sleep has the supervisor
too: it ends each `clock_nanosleep` at once,
since what the program does next can't depend on how long it slept, and a test shouldn't wait
for it; a real `run` sleeps as long as the program asks. Signals, threads, the exit status, the
counters and the time limit work as in any run, and runs that need none of this have no
supervisor at all. Each supplied call adds a few microseconds of wall
time, never instructions or cycles. The counters report three
measures, which the optimize step compares by their medians: instructions executed in the
program's own code (the steadiest measure of work, though a count can be one higher now and then,
because the CPU counts the odd interrupt or page fault as one), CPU cycles in its own code (time,
where slow instructions such as division weigh more), and CPU time including the kernel's work for
its system calls (fewer instructions isn't always faster: a program that writes one byte at a time
can execute fewer instructions yet spend far more time in the kernel).

`check` draws a new seed for every test run, runs boundary values first, and reports the seed and
inputs with any failure, down to the line of a failed assertion. Runs go as many at once as the
machine has cores (fewer for a program that starts threads), and each draws its own inputs from
its own stream of the seed and computes what it should produce as it starts, so that work is
spread over the cores too; the failures reported are the ones running the tests one at a time
would give, each block's lowest-numbered failing run. On the 924 runs of `examples/count`, that
takes 0.74 s instead of 4.2 s. Instruction counts come from every run, since other runs don't
change them; once every test passes, each test's largest input runs again alone five times for
its cycles, CPU time and wall time, which other runs would disturb. Only when the rules and every
test pass does it stamp `[code]`, including the executable's SHA-256 and the toolchain versions.
`run` rebuilds `main.s` and runs it only if the build is byte for byte that executable.

A checked program also runs outside the agent, from the user's own terminal, scripts and
services: `lengua-mcp run <program folder> [--in <folder>] [--env NAME=value]… [-- <argument>…]`,
the command `check`'s done reply gives as `shell`. It makes `run`'s checks (the lock and the
user's key, no proof found it wrong, the build byte for byte the one tested) and confines the
program the same way, with the shell's current folder, or `--in`'s, as its working folder. Its
standard input, output and error are the shell's own, streamed, with no time or output limit; a
server serves on its spec's port until it's stopped; an interrupt, hangup or request to terminate
goes on to the program; and the exit code is the program's, or 128 plus the signal that ended it.
When lengua won't run it, it says why and exits with 125, as `timeout` and `env` do.

The proof records its results in a `[proofs]` table: the executable it's about, the generated
tests proven for every input in range or up to a length, the solvers that checked them, and the
tests it found wrong, with what it found, the moment it finds them. `run` refuses a build that
table finds wrong, however often lengua restarts, and a later `check` shows what was found. A
test proven there is proven memory-safe on the same runs, so the table says that too, with no
field of its own. Checking tests again drops them; a build that is byte for byte the same keeps
them, and any other build is proven again by itself.

## Performance

The bar is that a program the agent writes through lengua runs in fewer cycles, less CPU and wall
time and less memory than the best of C, C++ and Rust built with every optimization for the CPU.
`cargo bench --bench against_rust_and_c` measures it on six workloads, from a program that is
mostly starting up to 64 MiB streams, and writes BENCHMARKS.md.

Speed comes from the agent, guided by the compile skill, which makes it a step of the
normal flow: once the tests are green, one pass for the widest vectors the CPU has (with masked
loads for tails), independent chains where each step would wait on the last, large reads and
writes, and streaming for input of any length. `assets/fast` holds checked programs for each
pattern, which the benchmark measures: a word count 64 bytes a step with AVX-512 masks, a CRC-32C
that runs three `crc32` chains and joins them with `pclmulqdq`, and a four-table histogram. The
fork-join template works in place: input lands straight in the workers' slots, which halves the
memory a threaded program touches. Each program processes what each `read` returns as it comes,
so its work overlaps the writer filling the pipe.

What lengua doesn't do for speed is give up a check: bounds checks stay unless a reported
`# unsafe:` block shows they can't fail, every loop is bounded, and vector code is tested
exhaustively at its edges by the generated tests (the prover doesn't model vectors yet).

## What lengua does, and what it leaves to the agent

lengua is an agent plugin: skills plus an MCP server, nothing else. The server does only what the
agent can't be trusted or equipped to do: recording the request and the signed lock, the checks
that keep tests and code true to the recorded spec, marking test changes that test less,
building, running under the kernel's confinement, measuring, supervising test runs and proving. Everything generative or advisory is the agent's, guided by the
skills: specs, tests, code, optimization, explanations, fetching from the web, and comparisons with
other languages. Style rules, such as the order of Given, When and Then, live in the skills; the
server checks only what it parses and relies on.

## The lengua dialect

AI-written assembly takes the best of C (transparency, no macros, the standard calling convention,
named constants, read-only data), C++ (one exit and cleanup path per function, assembler-computed
constants, local labels, specialized code instead of templates) and Rust (one owner per buffer, no
allocation, single-threaded, pointer + length bounds checks, `Result`-style errors, exhaustive
branches, marked `unsafe` blocks, dialect versions), checked against NASA's "Power of 10" rules
adapted for assembly. The full rule text lives in
`plugin/skills/compile/references/power-of-10.md`. In short:

1. Simple control flow: jumps stay in their function, no recursion, compare chains end in an assertion, and a checked jump table picks among dense cases.
2. Every loop counts down from a named maximum.
3. Resources reserved up front: memory is static (`.bss` sized from the spec's limits) plus the stack, nothing is allocated while running, one thread, and only the system calls of what the spec names (write and exit by default), with a computed maximum stack size.
4. Functions of at most 60 instructions, standard calling convention, one exit.
5. At least two assertions per function on average; a failed assertion exits with code 134.
6. Smallest scope: one owning function per buffer, only `_start` exported, read-only data stays read-only.
7. Every result is checked (`rax` < 0 means error).
8. No macros; named constants for system calls, exit codes, sizes and loop bounds.
9. No function pointers, and no jump through a register but a checked jump table's; indexed accesses are bounds-checked over their full width (16 to 64 bytes for vectors), except inside a reported `unsafe` block.
10. Warnings are errors, every change is checked, and the dialect is versioned.

The checker analyzes the **linked executable** together with the source, and nothing AI-written
runs before it passes the rules. It reads `main.s` (one instruction per line, constants before
use, no macros), builds it, decodes the executable with iced-x86 and matches every instruction to
its line, operands included; anything that doesn't match is a violation, so what runs is exactly
what `main.s` says. Then it builds each function's control flow and exact stack depth and checks
the rules: unreachable lines are violations too. `assert_fail` is lengua's own code: it's compared
word for word with the template and is exempt from the rules it must break (it may always write
its message to standard error and exit with 134, whatever the spec allows). An assertion is
reached through a conditional jump, the fall-through of one, or a `jmp` right after one (the end
of a comparison chain or of an exhausted loop).

Instruction sets: dialect 4 (the current one) uses x86-64's integer instructions, its integer vector
instructions and floating point in double and single precision, with fused multiply-add (scalar and
packed; no half precision, approximations, rounding-mode changes or x87, so the tests can compute
exactly what the code does: `single(x)` and `fma(a, b, c)` in a formula match `ss` instructions and
`vfmadd*`).
Integer extensions (BMI1, BMI2, POPCNT, LZCNT, ADX, MOVBE), crypto and checksum extensions (AES-NI,
VAES, SHA, PCLMULQDQ, VPCLMULQDQ, GFNI) and vector sets beyond SSE2 (SSE3 to SSE4.2, AVX, AVX2 and
AVX-512) are allowed only when the CPU has them: the checker reads which CPU features each
instruction needs. Vectors follow four extra rules:

- unaligned loads and stores only (`movdqu`; an SSE instruction other than `movdqu` or `lddqu` may
  not read 16 bytes of memory, since it would need alignment);
- AVX-512 write masks, zeroing and broadcasts are allowed, but masked accesses count their full
  width; rounding overrides aren't allowed;
- no gathers, scatters or non-temporal moves: each element of a gather or scatter has its own
  address, which no bounds check can hold to its buffer (floating point arrives with its own
  rules);
- every byte of an access counts: rule 9's bounds checks and `[rip + name]` accesses must keep
  the full 16 to 64 bytes inside their data symbol.

Dialect 1 was the integer-only subset, dialect 2 had no memory rules, and dialect 3 no jump
tables; locks record the dialect that checked the code, and a lock from another dialect has to be
checked again.

Checked jump tables (dialect 4) are the one jump through a register: rule 9's bounds check of the
index, then `lea base, [rip + table]`, `movsxd target, dword ptr [base + index*4]`, `add target,
base` and `jmp target` in a row, with the table in `.rodata` holding only `.long .Lcase - table`,
each a label of the same function after the jump (`checker/tables.rs`). The checker gives the jump
the table's cases as successors, so the control flow, the stack depths and the memory rules follow
each one, and since a table only jumps forward, every backward jump stays a loop's end. The proof
forks at the jump into each case the target can be, as the solvers decide, and stops where it
could be anything else, so it rests on neither the checker's match nor the assembler's entries.
An index only its compare bounds can range far past the table: the proof then reads only the
places inside the object the access is held to and asks the solvers whether any input reaches
further, as it does for any address the input chooses.
`assets/vector-template.s` is a checked SSE2 program that finds a newline and capitalizes a line
16 bytes at a time.

Memory safety: since dialect 3, lengua checks memory the way Rust's borrow checker checks
references, on the assembly itself. A dataflow over each function gives every register an owner: a
data symbol's address, the function's own frame, a pointer the caller lent in a register, the
program's arguments, a plain number, or a value it doesn't follow. Functions are checked callees
first, and each one's summary (the registers it reads and writes through, itself and in what it
calls, system calls included, and what it returns) carries owners across calls, so no header has to
declare them. Then the M rules hold:
- no address is made from a plain number (M1);
- constant offsets stay inside their symbol, the one main.s names even where the next symbol
  starts just past its end, bounds checks bound the whole index, and bit tests take constant
  offsets (M2);
- a frame address is never stored outside the frame or returned, and constant offsets from rsp
  stay inside the function's own frame (M3);
- an address a function writes through is lent to it once, never in a second register as well,
  and the program's arguments are only read (M4);
- no frame byte is read before every path has written it (M5);
- read-only data is never written through any register, and no segment is both writable and
  executable (M6);
- a worker thread reaches the per-thread data only through its own slot, each index bounded by a
  constant (M7);
- the deepest path uses at most 1 MiB of stack (M8).

The rules stop where a general-purpose program would. Separate parts of one buffer, such as
objects carved from an arena (the dialect's dynamic memory), may be lent together, and pointers
loaded from memory, as lists and trees need, are allowed. Every program in the repository
passes, and a planted bug for each rule is found on its line.

What no rule settles, such as those pointers or an index checked against a length in a
register that the code just before doesn't set to a constant, the proof settles on every run it
covers. The prover's model holds each run to the
memory rules, as a borrow checker would at run time:
- Each access stays inside the object its pointer was made from: a data symbol, a live frame,
  or one of the program's arguments. The run follows each pointer from where it's made, through
  moves, adding numbers, and storing and loading it whole, so a pointer one step past an
  object's end is still that object's, even where the next object starts right there, as
  symbols, strings and frames often do; for `[rip + name]`, the symbol main.s names decides,
  not the address. A pointer whose origin a run loses, as through a vector register, is held to
  the object it points into, and the argument strings sit a byte apart for it. Each buffer lent
  to the kernel is held the same way, for as many bytes as the call may touch.
- No return address is written, and each `ret` goes back where its call came from.
- No stack byte is read before something wrote it. Stack a function released is gone.
- An address the input chooses that could leave its object becomes an obligation, which the
  solvers check against the run's conditions.

A broken rule is a counterexample with the inputs and what broke, such as "in write_i64, `mov
byte ptr [rcx],0` writes 1 byte at byte 8 of write_i64's own frame, which has 8 bytes, so it
reaches the return address of write_i64". A real run can pass without showing it, so the model
is the witness, and no replay is needed. The done reply's `memory` line says what the rules
settle, and which proofs held their runs to the rest.

## Translation validation

lengua treats the AI as an untrusted compiler. Instead of trusting it, lengua checks every output
it produces, the idea behind translation validation (Pnueli, Siegel and Singerman, 1998):

| Step | Translated by | How lengua checks it |
|---|---|---|
| request → `spec.md` | the AI | No machine can check intent, so lengua shows it instead: the spec is recorded with the person's words, every change after them is kept as a diff, and the agent's report says what the spec promises and what changed. New words in the middle of a build join it rather than start over, so no change drops out |
| behaviors → tests | the AI | Each behavior's steps, in lengua's phrasings, are its example, so the example can't disagree with the behavior; structure and coverage; each formula reproduces its example; generator ranges are numbers that the spec's Purpose states (not an example's own values) |
| spec → `main.s` | the AI | The rules, then every example plus fresh random tests on every check |
| `main.s` → machine code | LLVM | The checker reads the linked executable back and matches it to `main.s` instruction by instruction, operands included; the lock records the executable's hash, and `run` refuses any other build |
| machine code, every input | — | The proof: for generated tests over numbers, choices and texts, the output equals the formulas for every input in range, not only the sampled ones; for long texts, every input up to a stated length |

Tests sample inputs, so on their own they are strong evidence, not proof. The proof turns
them into a proof where it can, as Alive2 does for LLVM. The Power of 10 rules make that possible:
without recursion and with a fixed maximum for every loop, every path through a program is finite.

How the proof works (`src/prove/`):

- **Input shapes.** It splits each `int(lo, hi)` variable into shapes: a sign and a range for each
  digit, like a regular expression for a number range, so that every number a shape allows is in
  range. The full 64-bit range takes about 70 shapes.
- **Symbolic execution.** It runs the executable's own instructions on terms: each register, flag
  and byte holds an expression over the input's digits. A branch the inputs decide splits the run,
  and each side keeps its condition.
- **Ranges settle most branches.** Terms are simplified as they're built and know their ranges:
  digits are 0 to 9, a 10-digit number is below 10^10, and so on. With those ranges, the parse
  loop's checks, the overflow checks and the print loop's digit count are mostly settled with no
  solver at all.
- **Identical terms, not equivalence proofs.** Normalization makes the program's 64-bit
  two's-complement arithmetic and the formula's exact 128-bit arithmetic come out the same term
  where neither can overflow. Printed digits are matched through their common root, the value
  divided by powers of ten. So "prints the formula's value" usually reduces to identical terms,
  and the solver is asked only what remains. The same idea reaches the program's own choices:
  - a comparison of values whose widening simplifies all the way is made on the wide values, so
    a flag test on the program's 64-bit `a - b` and the formula's `a - b < 0` meet as one term;
  - a sign-bit test is `v < 0`, and `-x < 0` is `0 < x`;
  - a choice widens branch by branch, and its condition settles each branch's sign and range;
  - `ite(0 < x, x, -x)` and `ite(x < 0, -x, x)` are one term, the size of `x`;
  - a value stored to memory and loaded back is the value, not its bytes rebuilt.

  With these, the absolute difference Claude Code wrote with `cmovs` and its own parser, which
  timed out before, proves in about ten seconds, and the template in under a second.
- **Sums have one form.** A sum is its parts, each with a constant multiple, sorted, plus a
  constant: `add`, `sub`, `neg` and multiplying or shifting by a constant all build that form,
  so a program's sum and a formula's are one term however each grouped, ordered or scaled it.
  Zero- and sign-extension go inside a sum only where its exact range, from its parts' ranges,
  never wraps; `x - (x / c) × c` folds to `x % c` inside it, and a count stays a count. The
  clock Claude Code wrote computes Hinnant's days-from-civil in one order and its tests in
  another: the seven day-count blocks prove with no solver.
- **The output check settles under the run's path.** Before any question, each comparison in
  the check is held against the path's own comparisons: of the same sum, its negation, or
  either plus a constant, at its width or another up to 128 bits, as sets of intervals that
  conditions joined by and, or and not, and a byte's table, add up to. Where a condition the
  path speaks of but leaves open still chooses, as the sign of a total that may be 0 chooses its
  size, each side is settled on its own with the condition added to the path. So the CSV total
  Claude Code wrote proves every amount with no solver, and its other blocks prove their text
  lengths with no solver either.
- **Divisions meet as one term.** A program divides with 64-bit `div` on 128-bit dividends and a
  formula divides in signed 128 bits, so the same division reaches the pool in different forms.
  The pool gives each one form:
  - a signed quotient or remainder of a value that's never negative is the unsigned one;
  - a division of a value that fits in 64 bits is done at 64 bits;
  - `(x % a) % b` is `x % b` when `b` divides `a`, and a remainder of a value below the divisor
    is the value;
  - `x / c < k` is `x < c·k`, and `x / c = k` is `c·k ≤ x < c·(k + 1)`;
  - comparisons of zero-extended values are made at the narrow width;
  - a compiler's reciprocal, the top bits of `x` times a magic number, is `x / c` when the
    rounding provably never reaches the next whole number;
  - `x - (x / c) × c` is `x % c`.

  So the hh:mm:ss clock Claude Code wrote, six divisions that timed out in the solvers, and a day
  of the week with a second division of the first's quotient, prove in well under a second.
- **Bits meet as one term too.** A shift by a constant is the shifted value's bits with zeros
  beside them, as an extract and a concatenation, and an extract passes through and, or, xor,
  not and choices. So the bits of a mask a program builds with `sete` and `shlx` are each byte's
  own test, and `popcnt` of the mask, a sum of its bits, is the same sum of tests that
  `count(text, "a")` makes. A printed number's first digit takes no remainder, since the same
  check bounds the value below ten to the number of digits: a count printed as one digit is
  the count plus `'0'`, the program's own term.
- **A test of one byte is a table.** Any true/false term computed from one byte alone becomes
  the set of that byte's values where it holds, worked out once per shape of term. So a
  program's `b == ' ' || b - 9 <= 4`, a vector compare's lanes and a formula's six-byte set of
  blanks are one term; an output byte that equals the formula's at every value of its input byte
  folds to true with no solver; and a text's characters are one condition per byte.
- **A count is a set of tests.** A sum of ones and zeros becomes the list of conditions it counts,
  in any order and grouping: `popcnt` of a mask built from byte tests, chunk by chunk, and a
  formula's `count` or `words` over the whole text are one term. Two different groupings of one
  count would otherwise ask SAT to prove additions equal, which it does badly.
- **Branches no witness reaches are followed without asking.** Whether such a branch can happen
  is settled where its run ends: by the output check, which a branch that can't happen passes,
  or by one question about the run's whole path before a run that ends badly counts against the
  proof. A print loop's branches on how many digits a count has are both followed, and the
  output's digit count is the same conditions, so no question is asked. If that makes too many
  runs, the shape is explored again asking at each branch. Witnesses draw a text's bytes from its
  characters.
- **Vector registers are bytes.** xmm, ymm and zmm 0 to 31 are each 64 byte terms, so an
  instruction works lane by lane on the input bytes themselves; AVX-512's k0 to k7 are 64-bit
  terms whose bits are a compare's tests. Legacy SSE writes keep a register's upper bytes, VEX
  and EVEX writes clear them, masked elements are zeroed or kept, and a masked load doesn't read
  an element known to be masked out, as the CPU doesn't fault on it. Data in .bss reads as zeros
  until written, as the kernel leaves it.
- **Addresses the input chooses.** A load or a store at an address with at most 256 places,
  all in memory the access may use, is a choice among the places: a table indexed by an input
  byte, or by a month, is a function of it, and a table of counters keeps each slot's value
  unless the address is that slot's. An index scaled by 8 lands only on every eighth byte, so a
  table of 256 qwords is 256 places, not 2041. Past 256 places, an access held to a data symbol,
  as a hash table's slot is, puts the symbol in a log: its bytes as they stood, then each store
  since, oldest first, and a load anywhere in it is the newest store that may be there, then
  what was there before. Every later access to it goes through the log, at any address; its
  staying inside the symbol is a question for the solvers, and addresses two aligned ones a few
  bytes apart are never taken for the same. So the word tally's index of a million slots, chosen
  by each word's FNV-1a hash, is followed as it is: assets/tally-template.s is proven for every
  text up to 5 bytes in two minutes, both solvers settling where two words' slots meet, and a
  tally that forgets to note a word in its index, or puts tied words in the wrong order, is
  found.
- **The network is the tests' own peers.** A program that connects reaches only the test
  server's port, an input from 1 to 65535 since the kernel chooses it; any other port is
  refused, as the test's rules refuse it, and so is any other computer. The server waits for exactly what it must receive, so
  a read before the program has sent all of it waits forever: the run fails, as a real one times
  out. A server program takes the test client's connections on its listener in order; taking
  the next before closing the last waits forever too, and with none left the program is serving.
  The test's client behaves as curl and browsers do: it sends, keeps its side open until the
  program starts answering, then closes it and reads the rest. So a server that reads to the end
  of the request before answering waits forever, in the proof as in the tests. What a peer sends
  arrives whole, in the tests' three pieces (the first byte, up to the middle, the rest), or with
  two of those pieces together, each a shape of its own; through `fill`, every way at once (see
  below). A run passes when it made exactly the test's connections, sent each server exactly
  what it waits for, answered each client as the test says, and is serving at the end when the
  test says so. A counterexample is run for real with the same delivery. Two texts, a port and
  four deliveries multiply, so combinations go by their longest text without being listed first.
- **A server that polls, in every order.** Once a program waits on its listener and its
  connections with `poll`, the test's clients are agents of their own: each connects, sends
  its pieces, and closes its side once its answer has begun, and between one poll and the next
  each takes any of the steps open to it, each way its own run, so the runs cover every order
  the clients' steps can come in relative to the server's. Clients at once also come with the
  test's own order of steps, each but the newest holding its last byte until every newer client
  has its whole answer: a server that waits on one client while another waits on it is caught
  there, waiting forever. A read of a connection with nothing arrived waits, while every other
  client waits too. Runs that come to the same state after the clients' steps go on as one; a
  run back in a state it was in before, with no client able to step, goes round its loop
  forever while a client waits, and fails. The event-loop template is proven so: one client for
  every line in range in under a second, and two at once as far as the time goes, every pair
  of lines up to 122 bytes each in a check's five minutes, with no solver question; serving its
  first slot alone, or never closing a connection, is found.
- **fill is read as a whole.** `fill` (in the skill's `assets/fill.s`, copied word for word with
  its constants, which the checker holds to) reads until its buffer is full, the input ends, or
  the last byte read is its end byte. At its loop's head the proof takes at once each place a
  read could stop: after any end byte that has arrived, or where the buffer fills or the input
  ends, each a run of its own, with the counters the reads leave unknown. A block whose program
  reads standard input and its connections only through `fill` is proven with each message cut
  into pieces any way, and standard input isn't held to one pipe write; where the program also
  reads with `read` itself, the four deliveries are proven as before. A counterexample at a cut
  replays with that cut: standard input or the peer's message in two pieces, the second a moment
  after the first. lengua's own tests run fill's code for every cut of every input up to 5
  bytes, every capacity up to 7, with and without an end byte: it gives exactly what the whole
  gives, and four bugs planted in it each show.
- **Contradictions need no solver.** A question that holds a condition and its negation, compared
  by their structure with sums as sets of parts whatever their grouping, is impossible on its face:
  a run's own branch and the check that its output breaks often come to exactly that.
- **The solvers.** It asks z3 in SMT-LIB 2, and cvc5 confirms. Each worker thread keeps one pair
  of processes from one input combination to the next, reset in between, and starts them only at
  its first question the terms don't settle. Inputs already known to reach a branch are tried
  first: one that meets the branch's condition shows it can be taken, with no question, so only a
  branch that may be impossible costs one. Combinations go in a scattered order over every core,
  so a counterexample anywhere turns up early and the pace of the first ones shows whether the rest
  fit: a block that can't finish says how long it would take. A block may have up to 2,000,000
  combinations, and each gets its share of the proof's five minutes, the time it doesn't use
  passing on. The time is kept inside a shape too, at every branch and before each run's check,
  so one long shape, as one that takes every final piece at once, can't run past it; a shape
  still gets 20 seconds from its start, so a quick one begun late, as after a block's mebibyte
  samples ran, ends. `examples/add` takes about 30 seconds for 4×10^18 input pairs, the first
  CSV total Claude Code wrote, 24,696 combinations, about six seconds with no solver question,
  and the template under a second.
- **Small values in few bits.** A division by a constant is sent to the solvers in the bits its
  dividend's range needs, and so is a count, a sum whose parts are each 0 or 1: a count of 16
  bytes divides and adds as 5 bits whatever width the program and the formula wrote it in, so
  its two widths are one small circuit. Other sums keep their width, where bit-blasting does
  best with them. cvc5 bit-blasts each question whole, which takes it milliseconds where its lazy
  default took minutes. When z3 gives up on a question, inputs cvc5 finds still show a branch can
  be taken; only "impossible" needs both.
- **Floats as bits first.** A question with floats in it goes first to the same two solvers as
  bits, each float result an unknown in the range its bounds give, which the same operation on
  equal operands gives again. That allows more than the floats do, so "impossible" there is
  impossible, and inputs it finds count only where lengua's evaluator shows they really meet the
  conditions. A branch it can't rule out is followed; whether an output can break goes on to cvc5
  alone, in a session that reads IEEE floats, since z3 4.16 gives wrong conversions and a wrong
  "possible" on singles.
- **Memory stays free.** A proof starts one worker per core, as many as the free memory leaves
  1 GiB each for. A watchdog stops the solver holding the most whenever free memory falls below a
  sixteenth of all there is, so a hard question ends its proof as "not proven: the computer ran
  low on memory" instead of the kernel's out-of-memory killer ending whatever it picks, such as
  the session the agent runs in. Each proof worker, and every solver it starts, runs ten steps
  below normal priority, so a proof takes only the cores nothing else wants: the user's own work,
  and the server answering the agent, always come first.

Trust:

- **Counterexamples are confirmed.** A counterexample is re-run for real before it's reported as
  a failure. If the real program passes, the result is "not proven".
- **The model is checked against the CPU.** Before a proof, the model runs concretely on sample
  inputs the real program has already passed, and must agree, with the test's server or client
  and a sample port in place of the one the kernel chooses.
- **Models are checked.** Every satisfying model a solver returns is checked against the
  assertions by lengua's own evaluator.
- **Only z3's bit-blaster and SAT core.** z3 4.16 (Fedora's) got bit-vector formulas wrong in both
  directions in its own simplifier and its default strategy. So lengua never uses them. It does
  its own simplification, asks z3 only to bit-blast and run SAT, writes negation as `~x + 1`, and
  defines division itself (`x = q × c + r`, `r < c`).
- **A second, independent solver.** Every `unsat` a proof relies on, whether it prunes a branch
  or rules out a wrong output, must also be `unsat` from cvc5, whose bit-blaster and SAT solver
  (CaDiCaL) share no code with z3's. If cvc5 instead finds inputs that really meet the
  conditions, z3 was wrong and those inputs are used. Any other disagreement, or a confirmation
  that runs out of time, leaves the step unsettled: "not proven". cvc5 takes several times longer
  than z3's bit-blasting on these questions, so it gets four times z3's limit. With only one
  solver installed, the report says the proof rests on it alone.
- **lengua's own code is tested against random values.** Both solvers answer questions that
  lengua's own code builds, so a mistake there could make them agree on a wrong answer. Four
  tests draw fresh random values on every run (`LENGUA_RANDOM_SEED` repeats one):
  - *Each instruction against this CPU* (`src/prove/exec/against_cpu.rs`): a harness runs about
    1,440 probes, every instruction form the model runs in every width and operand kind, plus
    each condition after each flag-setting instruction, on about 34,500 register, flag and
    memory values. The bit instructions are among them: `popcnt`, `lzcnt`, `tzcnt`, `bsf` and
    `bsr` (with a zero source too), BMI1 and BMI2 (`andn`, `blsr`, `blsi`, `blsmsk`, `bzhi`,
    `bextr`, `shlx`, `shrx`, `sarx`, `rorx`, `mulx`, `pdep` and `pext`), `adcx` and `adox`,
    `bt`, `bts`, `btr` and `btc` (in memory too), `shld` and `shrd`, `movbe`, and `stc`, `clc`
    and `cmc`. The model must end with the same registers, flags and memory; where the CPU
    leaves a flag or a register undefined, the CPU's value must be one the model allows, and
    `bsf` of zero, which AMD keeps and Intel leaves undefined, may leave anything. Its first run
    found a real bug: a 32-bit shift by zero still clears the register's upper half.
  - *Each vector instruction against this CPU* (`src/prove/exec/against_cpu/vectors.rs`): a
    second harness records zmm0 to zmm7, k0 to k7 and a 128-byte memory cell as well, and runs
    about 1,150 probes, legacy SSE, VEX and EVEX forms with register and memory operands and merge
    and zero masks, on about 13,800 runs whose bytes lean to the ones vector code tests (controls,
    blanks, digits, letter bounds, the top bit). Its first run found a masked `vpandq` whose
    memory source the model read by byte instead of by quadword. About 620 of the probes run
    every float instruction the model runs, on lanes of doubles and singles that lean to float
    edges: zeros of both signs, ties, the ends of the whole numbers conversions take,
    the largest and smallest, infinities, and NaNs quiet, signaling and the default. They found
    a masked `vunpcklps` whose memory source the model read only where the mask was set.
  - *The simplifier against plain arithmetic* (`src/prove/term/random.rs`): 4,000 random
    expressions shaped like the prover's are built through the simplifying pool and evaluated at
    random inputs. They must equal SMT-LIB's arithmetic as written, and every term's range must
    hold its value. It found dividing by a constant zero simplified wrongly.
  - *The crypto instructions against this CPU*, in the same harness: AES rounds, key
    expansion and `aesimc`, the SHA-1 and SHA-256 rounds and message steps, and `pclmulqdq`, on
    random lanes; a constant changed in SHA-1's rounds or AES's MixColumns shows at once.
  - *Float formulas against the test language* (`src/prove/formula.rs`): 23 formulas of
    decimals, singles and doubles, `sqrt`, `fma`, the roundings and `fixed`, over 400 random
    draws, must give the test language's values bit for bit.
  - *The solvers against the pool*: with every input fixed, both solvers must find any other
    value of a random term impossible, and its own value possible, so the SMT-LIB text means
    what the pool evaluates and never rules out the right answer, which would make every proof
    false. Counts and their divisions by 10 are among the terms. It found that z3's bit-blaster
    can't divide by a non-constant; every division is now a quotient and remainder that
    multiply back.
  - *Output conditions* (`src/prove/outputs.rs`): printed numbers, with and without text
    around them and from print loops shaped like the template's, must be accepted exactly when
    the bytes are the formula's text, never for leading zeros, a lost sign or a wrong digit.

What it covers:
- Inputs: `int(lo, hi)`, and `decimal(lo, hi)` as digits around a point. Each value of a
  `choice(…)` is proven in turn. `string(min, max, "chars")` has a variable for each byte, held
  to the characters (past 4,096 bytes, to the range they span), and its lengths go shortest
  first: up to 4,096 bytes, or for a text that starts longer, such as a mebibyte file a program
  only measures, up to 4,096 of its lengths. Text with characters beyond ASCII has a shape for
  each order of its characters' widths in bytes: an ASCII character is a byte variable, and a
  wider one a variable choosing among the characters of its width, each of its bytes that
  choice's byte, so a program's test of a byte is a test of the choice. The shapes multiply with
  the widths, so the proof reaches about 8 characters where three widths mix and 13 where two
  do: "proven for every input with text up to 8 characters".
- Long text, up to the length the spec gives it. A proof goes through lengths shortest first,
  as far as its time reaches, with no length of its own: a text the spec allows a mebibyte is
  proven as far as five minutes go. Where its time or the model runs out at some length, the
  proof holds for every input with texts up to the last length fully proven: "proven for every
  input with text up to 4889 of 1048576 bytes; longer texts are tested, not proven: the proof's
  time ran out at 4890 bytes". lengua.lock records such a proof under `bounded`, with its length,
  the spec's, and whether proving on can reach further, never as a full one. Where it can,
  `check` with `prove_further` proves on from there, for another stretch, keeping what the lock
  records of the rest, so the agent, or the user through it, decides how far a proof goes. One
  limit is the model's, not the time's: a pipe write holds 4,096 bytes, which a read returns
  whole, so standard input read with `read` is proven up to that, and read with `fill`, which
  takes any pieces, as far as time goes.
- Every length, by induction. A program that streams standard input through `fill` a
  buffer-full at a time and either writes each buffer-full mapped byte by byte (`upper`,
  `lower`, `tr`, or the text itself) or keeps counts that add up over the buffer-fulls (`len`,
  and `count` of one character, as `wc -l` and `wc -c` do) and prints them at the end, is proven
  for every length its read loop's bound allows, in two parts, as a proof by induction over the
  number of times round the loop. The step: from any state the loop can be in at fill's head
  with more than a buffer-full left, one time round reads exactly a buffer-full, writes the
  formula's bytes for it and nothing else (for counts, nothing, each count's register grown by
  the buffer-full's count), and comes back to the head in a state of the same kind, each bound
  counter one lower. The base: from any such state, every input of up to a buffer-full gives the
  formula's output and exit code, each count going on from any count so far, proven length by
  length. A count's register is one that went from a constant to the first buffer-full's count
  and goes on adding from any count: a register that holds only the last buffer-full's count
  looks the same the first time round and is caught the next. The states are found as a fixpoint: what one time round changes (the buffer, the
  registers its calls leave, the vector registers, the flags) is made arbitrary at the head,
  and the round is run again until nothing new changes; everything else must be as it was the
  first time, the calls under way, owners and stored pointers included, and the stack below the
  head holds nothing a run can read before writing it. The loop's counter, which rule 2 gives
  every loop, sets the reach: a counter starting at 2^40 covers 2^40 − 1 buffer-fulls, far past
  any test. `assets/stream-template.s` is proven so, every length up to 65,536 bytes and beyond,
  in about 13 seconds with no solver question: "every length by induction over its read loop";
  a `wc -l` counting 16 bytes at a time with `pcmpeqb`, `pmovmskb` and `popcnt` in about a
  minute, also with none. Its first version masked the last chunk with `bzhi` by the bytes left,
  which `bzhi` reads modulo 256, so a chunk 256 bytes from the end counted nothing: no register
  kept the formula's count, and the proof by length found a 256-byte text whose first newlines
  went uncounted. A count whose buffer-full's part depends on the bytes before it, as `words`
  does on whether the last buffer-full ended inside a word, and `count` of a longer part that
  can't overlap itself (`"\r\n"`) on its last bytes, is checked a buffer-full later: the step
  reads two buffer-fulls from any state at the head, and the second's count must be what the
  formula adds for it after the first's last bytes. Whatever the program keeps of what came
  before, a flag, a byte or a mask's bit, its count must come out so, which needs no telling
  where it keeps it, and `F(x ++ y) = F(x) + delta(x's last bytes, y)` carries the count across
  every buffer-full. The base reads a buffer-full from any state at the head before each final
  piece, each count going on from the run's own count after it, and the inputs of up to a
  buffer-full are proven from the start. The first buffer-full's count must hold on every path,
  since nothing else checks it outright. A step that goes wrong gives the solver's two
  buffer-fulls, which run from the program's start show the counterexample where the program is
  wrong: a wc that forgets at each read that the last one ended inside a word gets a text just
  over a buffer-full whose word straddles the two. tests/fixtures/words, a `wc -lw` through
  `fill` 64 bytes a step with AVX-512 masks, is proven so with no solver question. A count of a
  part that can overlap itself, as `count` of `"aa"`, depends on more than the last bytes: on
  `aaa…a` whether the next `a` ends a pair is the parity of everything read. The formula counts
  such a part as a matcher reads the text, one byte at a time, keeping how much of the part the
  bytes since its last match end with, and starting over after each match, which is `count`'s
  leftmost-first rule; a whole's count is then its first piece's and the rest's read from the
  phase the first piece leaves. The step learns where the program keeps that phase: in the first
  time round from the start, it evaluates each register the round changed, and the formula's
  phase, on random buffer-fulls, and takes the register whose values say the phase, each value
  one state, its value at the start standing for nothing matched. From then on the head holds
  that register at any of its values, and each time round must add the formula's matches read
  from the phase its value says and leave it at a value saying the phase after; the base reads
  each final piece from it. tests/fixtures/pairs, which keeps whether its last byte is an `a`
  that starts no pair yet, is proven so for every length in about 2 seconds, with no solver
  question; a program that drops that flag at a buffer-full's end, ignores it coming into the
  count, or starts with an `a` pending gets an input that shows it. Most registers a time round
  changes keep no count; a random input of the text's characters shows each such one so before
  any solver is asked, which took the line counter's step at 4 KiB from 52 seconds to a tenth of
  one, and at 64 KiB from five minutes to six seconds. The base takes every final piece at
  once where it can: a buffer-full of fresh bytes whose length the input chooses (each held to
  the text's characters only where it's inside; those past it stand for whatever the buffer
  held, which is anything), fill returning that length; and where a loop over the piece leaves
  on the length itself, the run doesn't split at each time round: it goes on as if still in the
  loop while each side that left waits at the exit, and when the loop is done the runs merge
  into one, for each input the one that left first. They merge only where the loop stored
  nothing, so their memory is one; past 64 exits, any other register, flag or vector that differs
  between them is a fresh value, which stands for any of theirs, rather than a choice among them.
  A count that only grew between exits is then one count of each part where the loop was still
  running when it was added, and the formula's count of a text of that length guards each part by
  whether it's inside, so the two are one term. What a condition says of the length alone,
  however the program works it out, is the set of lengths where it holds, worked out exactly from
  comparisons of the length moved by constants and their low bits (each a run of intervals), and
  written back, in its place in the condition, as the formula writes it: a mask's bit `b` of the
  bytes left in chunk `j`, as `bzhi` clears the bits from its index (the low byte of
  `min(length - 16j, 16)`), with the loop still running there, is `16j + b < length`. `pairs`
  with a 4 KiB buffer is proven for every length so in under four seconds, against nine minutes
  one final length at a time; the line counter, which masks its last chunk with `bzhi`, with a 64
  KiB buffer, C's size, in under 30 seconds with no solver question. Counts that look back, as
  `words` does, take every final piece at once too: every text up to a buffer-full from the
  program's start, and every final piece after a whole buffer-full read from any state at the
  head, the formula reading that buffer-full's last bytes as a window before the piece. For the
  program's count and the formula's to be one term: a count's parts have one form, their
  conjuncts sorted, so the same conditions joined in another order are one part;
  `(a ∧ c) ∨ (b ∧ c)` is `(a ∨ b) ∧ c`, what two compares under one mask joined by `korq` say of
  a byte; a mask register's low byte, `low8(1 << n) - 1`, says `b < n` of its bits as the whole
  mask does; where a part says the length lies in a set, the length's comparisons inside its
  other conditions are settled by that set, as the word carry into a 64-byte step, bit
  `min(left, 64) - 1` of the step before's whitespace, is bit 63 where the step runs; and the
  output check is first settled by the run's own conditions, which a printed number's digit count
  is, then the run's conditions are rewritten by what settles them, each from its parts. The
  skill's fast `wc`, 64 bytes a step with AVX-512 masks, each step masked to the bytes left so
  the last partial step is the same code, is proven so for every length with its 64 KiB buffer in
  about two minutes with no solver question; one that forgets the byte before each buffer-full or
  each step, reads the carry from the wrong byte, or counts the bytes past the end is caught.
  Masking every step costs it about half again the instructions and a third more CPU time than
  the old shape's separate tail, at the same wall time, since a pipe sets the pace
  (BENCHMARKS.md, 64 MiB): still ahead of C, C++ and Rust. Where every final piece at once isn't
  proven, the base goes one length at a time as before. A loop the induction doesn't fit, or
  whose step doesn't hold, is proven by length as above.
- What's proven for every length depends on the code's shape, not only on what it computes.
  Proven: a read loop through `fill` whose loops over each buffer-full leave only where their
  index reaches the bytes read, and mask each step to the bytes left. Not yet, so proven one
  final length at a time, as far as the proof's time reaches: a separate routine for the last
  partial vector at a place the length chooses (tests/fixtures/words, and the fast `wc` before
  2026-11-01), raw `read`s of any size (a megabyte-at-a-time loop), and byte-at-a-time loops past
  a few kilobytes, whose counts keep a copy of every part at every byte. So "the fast `wc` is
  proven" doesn't mean every streaming program is; the compile skill steers toward the shape that
  is.
- What the proof ran. A function no run of a proof called, such as a fast path that starts past
  the proof's bound, is named in the proof's line: "the proof never ran crc_block, so what it
  does isn't proven". The CRC-32C asset's three chains join only past 12,288 bytes, so its
  proof covers the byte-at-a-time path alone, and says so.
- Formulas: whole numbers (`+ - * / %` including chains of divisions, `abs`, `min`, `max`, `if`,
  and `scaled(x, places)` for a decimal's digits), printed plainly or with `places(x, k)`; and
  text: `len`, `count` (non-overlapping, leftmost first, as the test language counts), `words`,
  `upper`, `lower`, `tr`, `reverse`, joining texts with `+`, and comparing them with `==` and
  `!=`; and checksums: `crc32` and `crc32c`, a CRC step per byte from all ones, inverted at the
  end; and floats as the test language computes them: decimals in doubles, or singles where
  `single(x)` says, with `+ - * /`, `sqrt`, `fma`, `round`, `trunc`, `floor`, `ceil`, `double`,
  `single`, comparisons, and `fixed(x, places)` printed. All but `reverse` work byte by byte, which is exact on UTF-8; `reverse` goes by
  characters, as the test language does, telling them apart by each byte's values. Any placeholder whose variables all have fixed values in a shape is computed outright,
  so messages that quote an argument prove. A placeholder that's a text, drawn or computed,
  compares byte for byte. `sort` and `uniq` of a text of up to 8 bytes are a choice of texts,
  one for each way its newlines can fall and its lines compare, each where that holds: lines
  that are the same term for term sort once, equal lines keep their order, so for any input
  exactly one choice holds, and a random test holds it to the test language's own sort and uniq.
  The sort example is proven so for every input up to 6 bytes, where its comparisons' branches
  pass the 256 paths a shape may take.
- Instructions: the integer instructions, the bit instructions listed above, and the vector
  instructions programs use most:
  - moves of whole and partial registers, `movd` and `movq`, broadcasts, `vzeroupper`;
  - `pshufd` and `pshufb`; byte shifts and shifts by an immediate;
  - across lanes: `vpermb`, `vpermw`, `vpermd`, `vpermq` (by an immediate or by indices),
    `vperm2i128`, and the two-table `vpermt2*` and `vpermi2*`, as choices where the input picks
    the index; `vpcompress*` and `vpexpand*`, to a register or, for compress, to memory, as
    choices on the mask;
  - compares into vectors or masks, every `vpcmp` predicate, `vptestm` and `vptestnm`, `ptest`;
  - and, or, xor, and-not, `vpternlog`; adds and subtracts, saturating bytes, `pminub`, `pmaxub`;
  - `pmovmskb`, `vpmovm2*` and `vpmov*2m`, and every mask register instruction (`kmov`, `kor`,
    `kand`, `kandn`, `kxor`, `kxnor`, `knot`, `kshift`, `kortest`, `ktest`);
  - floats, scalar and packed, singles and doubles, in legacy SSE, VEX and EVEX forms: the four
    operations, `min` and `max`, square roots, `vfmadd` in its three orders, `roundsd`, every
    `cmpsd` predicate into a vector or a mask, `ucomisd` and `comisd`, the conversions between
    whole numbers, singles and doubles, `movsd` and `movss`, broadcasts, `movmskpd`, unpacking,
    `shufpd` and `shufps`, horizontal adds, `movhlps` and `movlhps`, and the bitwise `pd`
    forms. A rounding an EVEX instruction embeds is left to tests;
  - ciphers, hashes and carry-less multiplies: AES's rounds, `aesimc` and `aeskeygenassist`,
    on one lane or, with VAES, two or four; SHA-1's and SHA-256's rounds and message steps; and
    `pclmulqdq` on one lane or, with VPCLMULQDQ, several. Each is the bit arithmetic Intel
    defines, the S-box a table its byte picks from, so known values fold and a proof runs the
    rest.

  The vector template, upcase and the word counts Claude Code wrote are proven for every input
  up to 4096 bytes, with no solver question. An instruction the proof doesn't model yet is named
  with its function: "main.s's count uses vpermb, which the proof doesn't model yet".
- Floats: IEEE-754 singles and doubles as the CPU computes them under the MXCSR every program
  starts with, rounding to nearest with ties to even: the first NaN operand quieted, the default
  NaN for an operation with no meaning, the integer indefinite for a conversion that doesn't fit,
  each checked against this CPU. Every float result has the bounds its operands' give, so a
  branch on a float or on a whole number converted from one, such as a print loop's digit count,
  is settled without a solver. A program that computes in the formula's order meets it term for
  term: hypotenuse divides each side's thousandths by 1000, squares, adds, takes the square root
  and scales by 1000 before `cvtsd2si`, which is what `fixed(sqrt(a * a + b * b), 3)` computes,
  so it's proven for every pair of decimal sides in milliseconds, with no solver question. A
  program that computes in another order goes to the solvers, and may be "not proven".
- Checksums: `crc32` takes the bytes of its source one CRC step each, from the lowest, so 8
  bytes at a time and one at a time are one chain of steps, and so is the formula's `crc32c`:
  the CRC-32C asset is proven for every text up to 4096 bytes in about 20 seconds, with no
  solver question. A table lookup or a bitwise loop computing zlib's `crc32` builds other terms,
  so such a program is tested, not proven.
- Arithmetic across widths: a program that halves a sum of cents in 64 bits and a formula that
  multiplies a sum by 50 in 128 meet as one term. A sum divided by a constant that divides its
  every part divides part by part, as both sides of a choice between such sums do; a factor a
  sum shares with its divisor cancels; a value shifted right, then divided, is divided once; a
  quotient is 0 just where its dividend is below its divisor; a value shifted right compares
  with a constant as the value itself with the constant shifted left; a quotient's low bits,
  widened past the quotient's own width, are the quotient where it fits them; and a choice inside
  a choice whose condition settles it goes. So mean's block of whole numbers is proven in
  milliseconds, with no solver question, and its decimal means, two prices up to a million, in
  about 80 seconds with 16 cores: the program's digits and the formula's meet as one term, and
  the solvers are left how many digits there are. Each rule has random tests against plain
  arithmetic.
- Flags kept as numbers: a value that's only ever 0 or 1, as `setcc` leaves one, is 1 where its
  condition holds, and `and`, `or` and `xor` of two such values are 1 where the and, or or
  exclusive or of their conditions holds; such a value equal to 1 is the same as not 0; and
  `a and not (a and c)` is `a and not c`; and a count of one condition is that condition's 1 or
  0. So a program that keeps a flag in a register and a formula's condition are one term, and a
  sum of flags is a count: `pairs`' flag chain, `pending = is_a and not (is_a and pending)`, is
  the formula's matcher, and its count of `aa` is proven by length with no solver question where
  the two had been compared byte by byte (46 bytes, then cvc5 ran out of time). Random tests
  check each rule against plain arithmetic. A count that extends another, as a running total
  each step adds to does, keeps only the conditions it adds, and a hash of all its conditions
  finds an earlier count of the same ones however that one holds them. So a loop's counts take
  memory in proportion to the text it reads, not its square. Before, each step's total was a copy
  of everything counted so far: NEON's fast wc takes four times as many steps as x86-64's, and its
  proof needed 9 GB, more than CI's ARM64 runners have.
- Files: `openat`, `read`, `write`, `close` and `fstat` on the test's files, and `mkdirat`,
  `unlinkat` and `renameat` on its folders and files. A missing input file gives `ENOENT`, and
  the checks on the files and folders left after the run are the runner's. A file name the input
  chooses splits the run: one for each test file or folder it can be, with its bytes those on
  the run's path, and one where it's none of them and the file isn't there. A folder's listing
  (`getdents64`) is tested, not proven.
- The network: `socket`, `connect`, `accept4` and `accept` on the listener, `read`, `write`,
  `sendto`, `recvfrom`, `shutdown` and `close` on TCP connections over IPv4 on this computer.
  The echo server is proven for every line and delivery; fetch's argument errors for every
  port, and its fetch for every path with every body up to about 140 bytes in its share of the
  time.

- Threads: `parallel_for`'s `clone` starts a thread that runs to its end at once, on the stack
  it's given and with a first frame of its own, while the thread that started it waits; its
  `exit` clears its id, as `CLONE_CHILD_CLEARTID` does, and the waiting thread goes on, so
  `join_worker` finds it done. No thread writes what another reads (M7), and `parallel_for`,
  copied word for word, joins every worker before it returns, so this order gives what any
  order gives. Every run is held to the memory rules across threads. A program that splits its
  input by 64 KiB a thread starts no thread on the 4096 bytes a proof covers, and its proof says
  it never ran `start_worker` or `join_worker`.

Not yet: the `sha256` formula, which the SHA instructions now run under but no formula yet
meets; a CRC joined by `pclmulqdq`, which runs but builds other terms than the chain of steps;
`sort` and `uniq` of texts over 8 bytes, and `tally` of texts over 8 (shorter ones are a choice
of texts, as sorted ones are); a float printed as the test language writes it on its own, its shortest exact digits, rather than
with `fixed`; and `tr` that changes a character's width in bytes (`tr` beyond ASCII is proven
character by character, each keeping its width). Those blocks are tested, not proven, and the
report says why. A stream read both with `read` and with `fill` is proven for the four
deliveries, and, for standard input with at most 8 bytes left at a read, for every count the read
can return: a counterexample then replays with standard input cut where the proof found it.

## Resources, threads and capabilities

lengua takes the path that safety-critical C (NASA's Power of 10 and JPL's coding standard) and
Rust share: everything a program may touch must be something the checker can account for.

| Need | C and C++ | Rust | lengua |
|---|---|---|---|
| Memory | `malloc`/`free`, `new`/`delete`: leaks, double frees, use after free | Ownership frees memory automatically; arenas without the standard library | Reserved up front: static buffers sized from the spec's limits, plus the stack. Nothing is allocated or freed while running (rule 3), and `.bss` costs nothing until touched |
| Threads | pthreads: any thread may write anything, and data races are undefined behavior | `Send`/`Sync`, scoped threads and rayon: no data races, checked at compile time | Fork-join only, when the spec allows threads. A canonical `parallel_for` (copied word for word, like `assert_fail`) starts workers with `clone` on static stacks and waits on `futex` for them all. Each worker owns its stack and one output slot, like Rust's `chunks_mut`: the checker proves it writes nothing else (JPL: one owner per shared object). No locks, atomics or shared mutation, so results stay deterministic and testable |
| System access | Any system call | The standard library, within the OS's permissions | Capabilities: the spec names what the program uses (standard input, files and folders, the clock, random bytes, environment variables, the network), each target allows only those calls, and the kernel enforces them (seccomp) and keeps files inside the program's working folder (Landlock) |
| Files and folders | `fopen`, `stat`, `readdir`, `mkdir`, `unlink`, `rename` | `std::fs`: `File`, `metadata`, `read_dir`, `create_dir`, `remove_file`, `rename` | Under `files`, inside the working folder only: open, read, write, seek, close, a file's size (`fstat`), a folder's entries (`getdents64`), and making, removing and renaming. Tests give empty folders and check what's gone and what folders remain. The proof follows folders, deletes and renames; a folder's listing stays tested, not proven, since the kernel orders it its own way |
| Environment | `getenv` | `std::env::var` | Under `the environment`: only the variables a test, or the user's `run`, names, set only as the launcher becomes the program |
| Waiting | `nanosleep` | `thread::sleep` | Under `the clock`: `clock_nanosleep`, which a test ends at once and a real run doesn't |
| The terminal | `termios`, `poll`, `TIOCGWINSZ` | crossterm and the like | Under `the terminal`: `ioctl` on standard input, output or error for its mode (`TCGETS`, `TCSETS`) and its window's size, and nothing else, and `poll` to wait for a key. Tests give the keys as standard input, a pipe, where the mode can't be set, so a program goes on as from a pipe, and `\e` writes escape sequences; the proof models the same. `lengua-mcp run` puts the terminal back as it found it when the program ends, however it ends (`assets/terminal-template.s`) |
| Left out, by design | `fork`/`exec`, `malloc`, recursion, function pointers | `std::process`, `Box`/`Vec`, recursion, `dyn` | Running other programs (their code isn't checked); allocating while running (rule 3: buffers and arenas in `.bss`, sized from the spec, cost nothing until touched, and input of any length streams through `fill`); recursion and function pointers (rules 1 and 9: explicit stacks, and dispatch by comparison) |
| Clock, random | `time()`, `rand()` | `Instant`, `SystemTime`, `rand` | Capabilities the spec names, `the clock` and `random bytes` (on this target `clock_gettime` and `getrandom`), enforced by the kernel like the rest. Tests supply the bytes and readings, so output that depends on them is exact, and the prover covers every value the test's variables can take; without a supply, a rule such as "a number from 1 to 6" (`{any(1, 6)}`) checks the output, and the prover treats the kernel's values as any value |
| Network | Sockets | `std::net` | Plain TCP on the ports the spec names, to this computer unless the spec says `on any computer`. seccomp allows only TCP sockets (no UDP, raw or local ones), `sendto` without an address, and `accept4` on descriptor 3; Landlock lets `connect` reach only the spec's ports, and lengua's supervisor only this computer. A server never binds: the launcher listens on its port, on the loopback unless the spec says `for any computer`, and hands it the socket as descriptor 3, and only its accept loop, or its loop around `poll`, may run forever (`# bound: forever`, NASA's exception). A server serves one client at a time unless its spec says `up to N clients at once`: then seccomp allows `poll`, the event loop C servers use, over a fixed table of connections (`assets/server-template.s`), and a test whose clients connect at once makes each wait for another's answer, so a server that serves one at a time fails; the proof follows a server that polls through every order its clients' steps can come in, and through the test's own. Tests bring their own server or client on a free loopback port; websites and APIs go through the agent's own tools, which pass the data in |

A worker gets its index and its slot. It and every function it calls write only their own stack
frames and the slot, which it keeps in `rbx`, inside bounds checks; they make no system calls; and
each thread's stack is sized from the worker's deepest call path. Combining `results` in index
order makes reductions deterministic, and laying the slots end to end makes a map's output one
`write`. The kernel allows `clone` only with `parallel_for`'s flags, and `assert_fail` ends every
thread with `exit_group`, so a failed check anywhere stops the whole program.

Roadmap, in order:
1. the fastest streaming programs proven for every length, whose buffer-fulls are too large for
   a base that goes length by length, and hash tables, whose slots the input chooses among
   thousands (Round 7);
2. proofs of sorting past a few bytes, which need facts quantified over memory, and of floats
   computed in another order than the formula's, which are tested today;
3. the other platforms, toward the top six CPU and OS targets.

## Platforms

The MVP builds for Linux x86-64 (ELF, direct system calls), assembled by LLVM's `llvm-mc` and
linked by `ld.lld`. Later, in this order: Linux ARM64 (testable here with QEMU), macOS ARM64 and
macOS x86-64 (on a Mac, through Apple's LLVM; Rosetta 2 runs no AVX-512), Windows x86-64 and
Windows ARM64 (import libraries generated by `llvm-dlltool`, no Windows SDK).

The spec is the portable part and already agnostic: it names no CPU or OS, and `check` rejects a
CPU or OS term in a spec's prose, so the same spec means the same on every target. The per-target
part is `main.s` and lengua's checking of it, and that part is per-target by nature, not a gap to
close: you can't check native code without modelling the native machine. LLVM assembles and links
for every target (`llvm-mc`, and `ld.lld`, `ld64.lld`, `lld-link`), but it does not check, prove
or sandbox, which is where lengua's value is. So a new target is more than a flag.

### The plugin on each host, and the target it compiles for

The plugin is skills plus one MCP server. The **skills** are markdown and install and read the
same on every host. The **server** (`lengua-mcp`) is a native binary, built per host.

What needs the host, and what doesn't:
- **Checking the rules and proving are host-independent.** They're computation over the program
  and the spec: `llvm-mc` cross-assembles for any target, and a proof is math. A Linux x86-64
  server can check and prove a program *for* macOS ARM64, given that target's backend. So one
  server build can carry every target's decoder, prover model and dialect, whatever host it runs
  on.
- **Testing and running need the target's CPU.** `check` runs the spec's tests and `run` runs the
  program, under the host's own sandbox — seccomp, Landlock and the test supervisor on Linux,
  something else on macOS and Windows — so both need that CPU, natively or under an emulator.
  Under QEMU's user mode, which runs Linux ARM64 programs on an x86-64 Linux host, the kernel sees
  QEMU's own system calls, not the program's: Landlock and resource limits still confine a run,
  but the spec-exact system-call filter, the tests' supplied random bytes and clock, and the cycle
  counts need the target natively (Round 9 tests those on GitHub's ARM64 runners). So the per-host
  part of the server is the runner (item 6 below); it's also why the server builds and runs only
  on Linux today, its confinement being Linux-only.

**The plugin decides per target.** The server detects its host at startup (`Profile::for_target`
over `std::env::consts`) and uses that target, so the user installs one plugin and it picks what it
needs rather than choosing a build from a list. On a host lengua doesn't support yet it says so and
points here, rather than failing deep inside.

main.s's first line says which CPU's code it is (`.intel_syntax noprefix` or `.arch armv8-a`) but
never chooses the target: code for another CPU stops `check` ("main.s is AArch64 code, and this
computer is linux-x86_64"), so a main.s someone shares is compiled again for each computer
rather than moving its tests onto a weaker path. For developing lengua, a server started with
`LENGUA_TARGET=linux-aarch64` (as `LENGUA_PROOF_SECONDS` sets the proofs' time) checks and tests
AArch64 code on an x86-64 computer through QEMU's user mode, when its binfmt_misc entry has flag F
(the kernel opens QEMU when it's registered, so a program confined to its folder still runs). Its
machine line starts `linux-aarch64 (emulated by qemu-aarch64 10.2.2 on linux-x86_64)`. Under QEMU
each run keeps Landlock, the file and core limits and the time and output limits, and the kernel
stops the calls QEMU never needs to make, such as tracing, mounts, namespaces, modules and
io_uring; but the spec's exact filter, the supervisor and the counters would see QEMU, not the
program. So tests that need the supervisor (supplied random bytes or clock, instant sleeps, a
server of the test's whose connections lengua checks) are reported as not run there, nothing is
measured, and the reply says what held the runs. lengua.lock records what ran the tests
(`emulator = "qemu-aarch64 10.2.2"`), and `run` won't run code tested one way in a session of
the other. lengua's line in main.s is the target CPU's comment, `// lengua: …` on AArch64. Distribution is then: one server build per host OS,
each with that host's runner and the shared backends; a thin selector picks the host's build; the
skills are the same everywhere.

**Building and testing without owning the hardware.** A target doesn't need its machine bought.
Rust cross-compiles the server (for everything but the Linux-only confinement); CI runners are real
Linux, macOS (Apple silicon included) and Windows machines that build and test each target for
free; and QEMU runs Linux ARM64 here. What's written per target is the backend and that host's
runner — the machines to build and run them on come from CI and emulation.

### Porting a target

`src/platform.rs`'s `Profile` holds the triple, the short name and the linker's flags, and
through `src/syscalls.rs` each CPU's Linux: its system call numbers and the details of the ABI
that differ (`struct stat`'s layout, `O_DIRECTORY`, `poll` or only `ppoll`), pinned against the
kernel's headers and the C library. Linux on x86-64 and on AArch64 have both; the rest below is
still to build per target. A new profile must supply:

1. **Toolchain:** the LLVM triple, the linker binary, its flags and the object format. Today
   `toolchain.rs` `link()` hardcodes `ld.lld` and ELF; macOS needs `ld64.lld` and Mach-O, Windows
   `lld-link` and PE/COFF. Every instruction must link as written: for AArch64, `ld.lld` rewrites
   `adrp` + `add :lo12:` pairs into `nop` + `adr` unless given `--no-relax`.
2. **Decoder:** a decoder for the CPU's machine code that gives every instruction its facts, the
   one thing the checker's rules read (`src/isa`'s `Insn`: the registers it reads and writes, how
   control leaves it, its memory operand, what it does to the stack, the idiom it is), and the
   CPU's calling convention (`isa::Abi`). x86-64's facts come from iced-x86 (`isa/x86_64.rs`),
   and its matching of main.s to the machine code, line by line, and its lists of instructions and
   CPU features, from `checker/x86_64.rs`; `checker/decode.rs` reads the ELF through the `object`
   crate for any CPU. For a new CPU lengua decodes exactly its dialect's subset itself: anything
   outside it is refused by construction, the matching of main.s to the machine code stays
   lengua's own reading rather than LLVM agreeing with itself, and LLVM's disassembler serves as
   the decoder's test. AArch64's (`isa/aarch64`) reads machine words and main.s's text alike into
   one canonical form, the architecture's own instruction rather than an alias (`cmp x1, #3` is
   `subs xzr, x1, #3`), and agrees with LLVM on random words of every encoding class the dialect
   has, refusing every word LLVM refuses. `checker/aarch64.rs` matches main.s to AArch64's
   machine code as `checker/x86_64.rs` does x86-64's: both read into that form, each line with
   its names' addresses in the executable, and numbers compared whole, never masked.
3. **Prover model:** the CPU's instructions in `src/prove/exec`, x86-64's and AArch64's
   (`exec/aarch64.rs`, held to QEMU and to ARM64 silicon by its `against_a64` harness), around one
   shared core: calls by the CPU's convention, `fill`, induction and the term pool. AArch64's
   proofs settle as x86-64's do, with no solver question: its template, a streaming filter
   through `fill` for every length, and a file program. Its idioms needed two readings of the
   pool's own: a digit taken off with `udiv` and `msub`, `n - (n / 10) × 10` one division at a
   time, is the remainder, where x86-64's `div` gives it; and a product's overflow check, its
   high half against its low half's sign, `asr #63`, is the value's sign, which its range settles.
   Its floating point (`exec/aarch64/float.rs`) is the pool's float terms, which compute as SSE
   does, with AArch64's own rules around them where the two CPUs differ: a signaling NaN operand
   before a quiet one and a positive default NaN, `fmax` and `fmin` passing NaNs on and putting -0
   below +0, and conversions that saturate, a NaN giving 0. Where the operands' bounds rule those
   cases out the term is the pool's own, so the hypotenuse example's spec compiled for AArch64 is
   proven as x86-64's is, step by step the formula's doubles, with no solver question; the
   `against_a64` harness holds every float instruction to the CPU on NaNs of both kinds, zeros of
   both signs, subnormals and the ends of each conversion, and whole V registers, so a scalar
   write clears the rest. Its vectors (`exec/aarch64/vector.rs`, NEON's integer instructions) are
   lanes over the V registers' bytes: a comparison's lane is a choice of all ones or zero on its
   condition, so a program's tests of bytes are the formula's conditions, and `addv` of lanes
   that are each 0 or 1 is the pool's count, as x86-64's `popcnt` of a mask is. Its loops end
   with `subs` and `b.ne`, or `sub` and `cbnz`, which the proof finds as it finds x86-64's `dec`
   and `jnz`, so a loop over a buffer-full that leaves where its index reaches the bytes read has
   its exits merged into one run. Two rules of the pool's met NEON's mask of the bytes left, the
   low byte of `min(left, 16)` that `dup` spreads over a vector: a branch of a choice keeps the
   range its condition gives it, so `ite(x < k, x, k)` is at most k, and a comparison with the
   low bits of a value that fits in them is the comparison of the value itself. So
   [the fast wc's NEON form](plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s), each
   16-byte step masked with `cmhi` of the bytes left against each lane's place, is proven for
   every length with a 64 KiB buffer, with no solver question, in about 105 seconds and 1.8 GB,
   as x86-64's AVX-512 one is; and the vector template from its spec unchanged, every line with
   no solver question. CPU feature detection is per CPU too (x86's `is_x86_feature_detected!`).
4. **Dialect:** the Power of 10 in that CPU's mnemonics and registers, with its reference (today
   `references/linux-x86_64.md` and the x86 parts of `references/power-of-10.md`). The rules
   themselves read only the facts, so they're shared; what's per CPU is the syntax and what the
   dialect allows of it. AArch64's (`checker/aarch64.rs`, and the AArch64 half of
   `checker/source.rs`): line 1 is `.arch armv8-a` and comments start `//`; no x18, the platform
   register on Apple's and Windows' ARM64; an address only as `adrp` and, on the next line,
   `add :lo12:`; an index only as a whole `x` register; `br` only as a checked jump table's jump,
   and no `blr`, `adr` or literal pool; sp moved only by multiples of 16; x30 saved by every
   function that calls; loops closed by `subs` and `b.ne` or `sub` and `cbnz`; and a number too
   wide for one `mov` built from a `mov` and `movk`s that each name the same constant, which
   together count as `mov xR, #NAME`. Its messages say AArch64's registers and instructions. Its
   reference comes with the skills.
5. **The OS edge:** on Linux, the system calls' numbers, the instruction and registers that make
   them, and the ABI's details, which differ between CPUs even on one OS. On AArch64 there is no
   `poll`, only `ppoll`; `clone`'s child-tid argument moves; `struct stat` is 128 bytes with
   `st_mode` at 16 (x86-64: 144 and 24); and `O_DIRECTORY` is `0o40000`, x86-64's value for
   `O_DIRECT`. macOS and Windows don't allow a program to call the OS directly: a program must go
   through the system library (libSystem, ntdll), code lengua never checked, so those need the
   system-library calling convention modelled instead.
6. **The runner and sandbox:** on Linux, seccomp and Landlock (`runner.rs`), and the supervisor
   that supplies tests' random bytes and clock (`supervise.rs`, which reads the CPU's registers:
   x86-64's and AArch64's, each in its own small `cpu` module). macOS and Windows have neither;
   each needs its own confinement.

**One product on every target.** A port adds a CPU's or an OS's means, never a different lengua.
The same spec means the same on each: the same skills, tools, reports, lock and messages (only
the target's name and hardware facts differ); the same rules, memory safety, tests and proof
engine (only each CPU's instruction model is its own); and the same limits on every OS. The
capabilities a spec can name (`- Uses:` standard input and output, files and folders, the
network, threads, the terminal, the clock, random bytes, sleep, the environment) and what the
sandbox allows of each are one definition, which each OS enforces with its own means. Where an
OS can't provide or enforce part of it, that part is cut to what every OS can do, on Linux too,
so one OS's extra power never makes a spec behave differently. What may differ is the hardware:
the instructions a CPU has, its vectors' width, its speed. A target is done when it covers what
x86-64 covers, each item with its own hardware's means (NEON where x86-64 has AVX-512).

**Linux ARM64 is nearest:** the same OS, so the same kinds of system calls, the same sandbox and
ELF, but 2, 3 and 4 and a new triple are new, and 5 and 6 change in detail (the call numbers, the
ABI details above, the supervisor's registers). macOS and Windows need all six, the OS edge and
sandbox most of all. QEMU's user mode, installed here on 2026-10-31, tests Linux ARM64 on this
machine for all but the sandbox's exact filter and supplies, which GitHub's ARM64 runners test.

The plugin's own server binary will ship for x86_64/aarch64 Linux (musl), aarch64 macOS and
x86_64/aarch64 Windows; that is the server running, separate from which targets it can compile and
check programs for.

Next: the proof step and the other platforms in the order above, in an order the project owner
decides.

## Code conventions

- The official Rust Style Guide and Rust API Guidelines, plus the parts of Google's Rust style guide
  that apply outside Google's own codebases.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: zero warnings, like rule 10.
- No `unsafe`. `Result` for recoverable errors, `expect("why")` instead of `unwrap()`, newtypes for
  distinct values, doc comments on public items.

## Sharing

People share specs, never code. The spec is the source of truth: the same on every computer, and
all anyone needs to have the program. It's an idea file, as Andrej Karpathy calls his
[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f): it says what's
wanted, and each person's own agent builds out the specifics, here a program for their computer.
In the [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
v0.2 it's a concept document, `type` its one required key, so a folder of specs is an OKF bundle
that a site for specs can index by each one's description. To stay the same on every computer, a
spec names no CPU, instruction set, operating system, system call or toolchain: `check` points
out any such word in its prose, unless it's the program's subject, named in its title. main.s is disposable: each computer compiles its own from
the spec, lengua checks it there, and it can always be compiled again. lengua.lock is that
computer's own record and doesn't travel either.

Because an AI compiles each copy afresh, two computers with one spec get two different programs.
Every copy passes the same checks, the spec's tests, lengua's rules and the proofs, but outside
what those cover copies may differ: inputs beyond the stated ranges, cases the spec doesn't
mention, and speed. What's proven may differ too: "proven for every input" on one computer,
"tested, not proven" on another. So whatever must be the same everywhere belongs in the spec's
behaviors and ranges, and anything that shares lengua programs, such as a site for specs, promises
"every copy passes the same checks", never "the same program". Each compile is a full agent
session: minutes, and some cost.

A main.s that arrives anyway, in a copied folder, is just code to check: lengua checks it from
scratch before `run` runs it, and one whose second line says it was compiled for another target or
for CPU features this computer lacks is refused before building, with the next step to compile the
spec for this computer. A line naming another version of the spec is no problem: the tests decide,
and the line is written again after the next pass.

main.s's second line, `# lengua: compiled from spec.md sha256:… for linux-x86_64; needs AVX2`,
ties the code to the version of the spec it was compiled from. It names the code; it doesn't
protect it: main.s is plain text anyone can build outside lengua, past its checks and the kernel's
limits, so lengua's guarantees hold only for a program that `run` runs after `check`.

## Programs built from programs

Programs stack. A spec links to the programs it builds on, `- Builds on: [sort lines](../sort/spec.md)`,
so the specs are the nodes of a graph and the links its edges, as an OKF bundle's concepts link
to each other and an LLM Wiki's pages do: a folder of program folders, with an `index.md` that
lists each with its description, is a bundle to browse. Each node is a folder of two files, its
spec and its code, checked on its own: its own tests, its own proofs, its own lock. A library,
such as a hash table, is a node too, specified through a small program that uses it.

A program's main.s takes what it uses from its parts' code word for word, each function marked
`# from: ../sort`. `check` holds the graph to the code: a part must be done on this computer
before a program builds on it; a function taken from a part must be that part's current code,
so a part that changes shows in every program that uses it, with the functions to copy again;
a function can come only from a part the spec names; and a part the spec names must give
something. The program's main.s stays one file, its tests and proofs run through all of it, and
`run` needs nothing but its own folder. Sharing a program shares its node and the nodes it links
to: each computer compiles every one of them for itself.

## Threat model and known limits

- The lock's signature detects edits not made by the server. It is not a security boundary: any
  process running as the user, including the AI's shell, can read the key.
- Locks are machine-bound and code is disposable: on another machine the spec is recorded again
  and compiled there. See [Sharing](#sharing).
- Without hooks, the server detects and refuses rather than prevents. Someone could still assemble
  `main.s` by hand outside lengua, but that isn't a lengua run, and nothing lengua stamps or runs is
  affected.
- There is no approval step: the person's request is the go-ahead, and the agent records the
  spec with their words (`words = ["…"]` in the lock). lengua can't tell real words from words
  an agent made up, so it shows instead of gating: every request of a build stays in the lock
  until its code passes, every change to the spec after the build's first request is shown as a
  diff, and lengua itself marks test changes that test less. lengua relies on nothing from the harness, such as its permission prompts, so a
  harness in an auto mode is as safe: what a program may touch is what the spec names, which the
  kernel enforces on every run. The person can ask to see the spec before the build.
- Rule 9 checks the shape of each bounds check (`cmp` and an unsigned `jae` before the access)
  and, for the program's own data symbols, that the limit fits the buffer. Moving a pointer
  instead of indexing isn't checked, nor is pointer depth, and compare-chain detection is
  pattern-based. So passing the rules doesn't make a program memory-safe: the tests guard the
  rest until the proof step.
- Tests sample inputs, so until the proof step a program is checked, not proven. The same AI
  writes the formulas and the code: each formula must reproduce its spec example, which narrows
  that gap, but a wrong formula can still match an example by coincidence. "Proven" means proven
  against the spec as it ends, so the report states the spec's limits.

## Status

The MVP is built: milestones 1 foundation, 2 spec and lock, 3 tests, 4 rule checker, 5 code and
run, and 6 end to end, each committed after the project owner approved it. A program is two
files: the tests are in the spec, whose steps are its examples. `tests/e2e.rs` runs the whole
workflow on `examples/add` in both MCP protocol generations.

Dialect 4 is the current one:
- checked jump tables: a `switch` over dense cases in one bounds check and one jump, each case
  followed by the rules and the proofs;
- memory rules M1 to M8, a borrow checker for assembly: every pointer has an owner and every
  access stays inside it, statically where the rules settle it and by proof elsewhere, down to a
  pointer one step past its buffer;
- vectors from SSE2 to AVX-512, hashing and checksum instructions, and floating point in single
  and double precision with fused multiply-add, all modeled for proofs and checked against this
  CPU;
- files and folders, environment variables, sleeping, the clock and random bytes, fork-join
  threads, the terminal, and TCP clients and servers, reachable from other computers and serving
  many clients at once when the spec says so, each confined by the kernel to what the spec
  allows;
- proofs for every input in range over whole numbers, decimals, choices, texts and text
  formulas, checksums, floats computed in the formula's order, files, the network, servers
  that poll in every order their clients' steps can come in, and threads, each solver step
  confirmed by a second solver, with texts proven as far as the spec's lengths
  and the proof's time reach, and further with `prove_further`, and streaming filters and
  counts proven for every length by induction over their read loop;
- specs that name no CPU or OS, which `check` holds them to, so a spec shared alone compiles on
  another computer and its copy passes the same checks;
- `lengua-mcp run` for the user's own shell, and a release archive that installs with no Rust
  toolchain.

Interactive round 4 (2026-10-31) ran ten scenarios in real Claude Code sessions, each from one
request: a CSV total, a line-reversing server, a tiny web server, a fast `wc`, a SIMD ROT13, an
HTTP fetcher, a clock, a sort, a mean and standard deviation, and a spec shared alone. Every one
finished with no question, every test passing and every call under 45 seconds, for about $20 at
list prices; twelve phrasings of a request all chose write-spec first. A word-frequency count
followed the same way: from one request with no question, a 1,034-line hash-table program with
every test passing, in about eight minutes for about $2. BENCHMARKS.md, measured
the same day, has lengua's best ahead of or level with C, C++ and Rust on every workload and
measure, with 2.5 to 3.1 times less memory.

Tested, not proven, for now: sorting and `uniq` past a few bytes, floats computed in another
order than the formula's, a folder's listing, and the SHA and AES instructions.
Left out of the dialect on purpose: gathers and scatters, whose addresses can't be bounds-checked,
allocation while running, recursion, function pointers, and running other programs.

All of it is checked on this machine (x86-64 Linux). The long-term goal is the top six CPU and OS
targets; they come next, in the order Platforms gives. The spec is agnostic and travels today, and
LLVM assembles and links for every target, but each target is a profile with its own decoder,
prover model, dialect reference and runner, as "Porting a target" sets out — not a flag, and for
macOS and Windows not even the same shape, since they forbid the direct system calls lengua's
programs make. After the targets, a simple site where people share specs: the spec travels, each
computer compiles it for itself, and code is checked again wherever it runs.
