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

A program is a folder with four files, created in this order:

| File | Written by | Purpose |
|---|---|---|
| `spec.md` | the AI, from the person's request | The single source of truth: natural language, Given/When/Then behaviors with IDs, in [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf) (OKF) |
| `tests.toml` | the AI, from the spec only | One example test per behavior, plus generated tests with fresh random inputs on every run |
| `main.s` | the AI | x86-64 assembly in the lengua dialect |
| `lengua.lock` | only the lengua server | The person's requests for the build in progress, SHA-256 of all three files and of the checked executable, a combined fingerprint, which tests are proven for every input on that executable, and a signature |

**Workflow:** request → spec (recorded with the person's words; nothing waits for an approval) →
tests (stamped) → code (stamped once every rule and test passes) → proof → run, and optimize
(tests stay green). A program only runs when the lock matches all three files, so documentation,
tests and code can't drift from the spec. The person asks once; the agent's report then shows
the spec and every change after the request.

**Specs follow behavior-driven development (BDD).** Purpose says what the program does, for whom
and why. Each behavior is one rule, titled in the user's words and shown by one concrete example:
Given sets the scene (arguments, input), the one When is the program running, and Then states only
what the user observes (output and exit code). Steps describe behavior, never implementation.
`spec.md` is the living documentation: tests.toml turns each example into an executable test and
adds generated examples of the same rule, and the lock keeps the code in step with it. The parser
enforces the structure (a Purpose, Given before one When before Then, one exit code, unique IDs and
titles); the write-spec skill teaches the style.

| What changed | Result |
|---|---|
| `spec.md` | The next `check` records it under the request's words and checks the tests and code against it again. Changes since the build's first request are shown |
| `tests.toml` only | Checked against the spec again, then the code is tested and proven again. After code has run, the changes since the tests it first ran against are shown, with lengua's own marks on those that test less |
| `main.s` only | The next `check` checks the rules, runs every test and proves it again; once the tests pass, the new code is stamped |
| nothing | `run` works, and `check` answers from what it found |

## What ships

The plugin folder `plugin/` contains:

- **Skills:** `write-spec`, `write-tests`, `write-assembly`. Each teaches one file and ends with
  `check`.
- **MCP server** `lengua-mcp` (Rust, official `rmcp` SDK) with two tools:

| Tool | Does |
|---|---|
| `check` | Every stage, stopping at the first that needs work: validates `spec.md` and records it with the person's words; validates `tests.toml` (coverage, cross-checks, red check) and stamps it; assembles, links and checks the rules; then, in the background, runs every test, counts the instructions executed, stamps and signs, and proves the program. Each reply has one `next_step` |
| `run` | Verifies the lock against every file, rebuilds, confirms the build is byte-identical to the checked one, then runs it |

The agent's loop is one call: edit a file, call `check`, follow `next_step`. The person's words go
with the first call, and with a new request; an edited spec is recorded under the words already
recorded. Nothing has to be copied between files: the lock already records which spec the tests
were checked against and which tests the code passed. `check` replies within about 40 seconds,
because other agent harnesses cut off long tool calls (many stop waiting at 60 s). The tests and
the proof run in a background job per program folder, keyed by the hashes of its three files: a
`check` with the same files waits on the same job, a `check` with new files cancels it and starts
another, and a server restart starts it again, keeping the proofs the lock records about the same
build. Replies carry each fact once, where the next step needs it: the machine's cores and CPU
extensions while `main.s` is unwritten, the changes after the request and the cost of each test
only once done, and the spec's first example as the tests ran it, so the report needs no demo run.

Only open standards are used: MCP (tools and server instructions), Agent Skills and Agent Plugins. No hooks and no subagents, so the plugin works the same in Claude Code, Codex, Cursor and
other harnesses. Every guarantee lives in the server, which detects changes and refuses to stamp
or run.

## Packaging

| File | For |
|---|---|
| `plugin/plugin.json`, `plugin/mcp.json` | The [Agent Plugins](https://agent-plugins.org) standard (Codex, Cursor, Copilot, Kiro, VS Code) |
| `plugin/.claude-plugin/plugin.json`, `plugin/.mcp.json` | Claude Code, which ignores the standard's two files |

Both MCP configs start the same binary, `plugin/server/lengua-mcp` (built by
`scripts/build-plugin.sh`, not committed). The server keeps its private data in
`~/.local/share/lengua` (or `$XDG_DATA_HOME/lengua`): one signing key per user, shared by every
harness, so a lock recorded in one harness verifies in all of them.

## Tests

`tests.toml` has one example per behavior, holding that behavior's backtick values, and generated
blocks that draw new inputs on every run (boundary values first, then seeded random ones) and
compute the expected results with formulas. A block whose ranges hold at most 5000 combinations runs
every one of them instead, within 20000 runs per file: for those inputs the program is checked, not
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
travels in the expected text as a marker between Unicode noncharacters, which tests.toml can't
hold, so expected output stays text everywhere and only matching reads the markers.

`check` ties the tests to the recorded spec: example exit codes equal the spec's, generator
limits are numbers the spec states, and args and stdin hold bare variables so each example can be
read back into variable values and every formula must reproduce it. A dry run evaluates every
run's expected results, so overflow and division by zero are caught before any code exists, and
output with placeholders must vary, so a program that prints a constant can't pass. Several
generated blocks may cover one behavior (divisors on both sides of zero, for example) when they
share their templates. A wrong test can be fixed at any time: every version is checked against
the spec again, so a fix can't drift from it. After code has run, the reply shows the line diff
since the tests the code first ran against, and lengua's own marks on the changes that test less
(fewer runs, a generator that draws fewer values, exact output turned into a rule, a removed
block, variable or expected file). The same AI writes the code and the tests, so that judgment
stays in lengua; the agent only explains each change in its report. The server keeps a copy of
every recorded spec and tests file by hash, so the changes can be shown.

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
`process_vm_writev` and skips the kernel's call, so the program sees the test's values. Signals,
threads, the exit status, the counters and the time limit work as in any run, and runs that
supply nothing have no supervisor at all. Each supplied call adds a few microseconds of wall
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

The proof records its results in a `[proofs]` table: the executable it's about, the generated
tests proven for every input in range or up to a length, and the solvers that checked them.
Checking tests again drops them; a build that is byte for byte the same keeps them, and any other
build is proven again by itself.

## Performance

The bar is that a program the agent writes through lengua runs in fewer cycles, less CPU and wall
time and less memory than the best of C, C++ and Rust built with every optimization for the CPU.
`cargo bench --bench against_rust_and_c` measures it on six workloads, from a program that is
mostly starting up to 64 MiB streams, and writes BENCHMARKS.md.

Speed comes from the agent, guided by the write-assembly skill, which makes it a step of the
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
`plugin/skills/write-assembly/references/power-of-10.md`. In short:

1. Simple control flow: jumps stay in their function, no recursion, compare chains end in an assertion.
2. Every loop counts down from a named maximum.
3. Resources reserved up front: memory is static (`.bss` sized from the spec's limits) plus the stack, nothing is allocated while running, one thread, and only the system calls the spec allows (read, write, exit by default), with a computed maximum stack size.
4. Functions of at most 60 instructions, standard calling convention, one exit.
5. At least two assertions per function on average; a failed assertion exits with code 134.
6. Smallest scope: one owning function per buffer, only `_start` exported, read-only data stays read-only.
7. Every result is checked (`rax` < 0 means error).
8. No macros; named constants for system calls, exit codes, sizes and loop bounds.
9. No function pointers; indexed accesses are bounds-checked over their full width (16 to 64 bytes for vectors), except inside a reported `unsafe` block.
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

Instruction sets: dialect 2 (the current one) uses x86-64's integer instructions, its integer vector
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
- no gathers, scatters or non-temporal moves (floating point arrives with its own rules);
- every byte of an access counts: rule 9's bounds checks and `[rip + name]` accesses must keep
  the full 16 to 64 bytes inside their data symbol.

Dialect 1 was the integer-only subset; locks record the dialect that checked the code, and a
lock from another dialect has to be checked again. `assets/vector-template.s` is a checked SSE2
program that finds a newline and capitalizes a line 16 bytes at a time.

## Translation validation

lengua treats the AI as an untrusted compiler. Instead of trusting it, lengua checks every output
it produces, the idea behind translation validation (Pnueli, Siegel and Singerman, 1998):

| Step | Translated by | How lengua checks it |
|---|---|---|
| request → `spec.md` | the AI | No machine can check intent, so lengua shows it instead: the spec is recorded with the person's words, every change after them is kept as a diff, and the agent's report says what the spec promises and what changed. New words in the middle of a build join it rather than start over, so no change drops out |
| spec → `tests.toml` | the AI | Structure and coverage; each behavior's spec values appear in its example; each formula reproduces its example; generator ranges are numbers that the spec's Purpose states (not an example's own values) |
| spec + tests → `main.s` | the AI | The rules, then every example plus fresh random tests on every check |
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
  unless the address is that slot's.
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
  passing on. `examples/add` takes about 30 seconds for 4×10^18 input pairs, the first CSV total
  Claude Code wrote, 24,696 combinations, about six seconds with no solver question, and the
  template under a second.
- **Small values in few bits.** A division by a constant is sent to the solvers in the bits its
  dividend's range needs, and so is a count, a sum whose parts are each 0 or 1: a count of 16
  bytes divides and adds as 5 bits whatever width the program and the formula wrote it in, so
  its two widths are one small circuit. Other sums keep their width, where bit-blasting does
  best with them. cvc5 bit-blasts each question whole, which takes it milliseconds where its lazy
  default took minutes. When z3 gives up on a question, inputs cvc5 finds still show a branch can
  be taken; only "impossible" needs both.

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
    about 530 probes, legacy SSE, VEX and EVEX forms with register and memory operands and merge
    and zero masks, on about 6,300 runs whose bytes lean to the ones vector code tests (controls,
    blanks, digits, letter bounds, the top bit). Its first run found a masked `vpandq` whose
    memory source the model read by byte instead of by quadword.
  - *The simplifier against plain arithmetic* (`src/prove/term/random.rs`): 4,000 random
    expressions shaped like the prover's are built through the simplifying pool and evaluated at
    random inputs. They must equal SMT-LIB's arithmetic as written, and every term's range must
    hold its value. It found dividing by a constant zero simplified wrongly.
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
- Long text, up to a stated length. Standard input is proven up to what one pipe write holds,
  4,096 bytes, which the model reads whole. Where a block's texts run longer, or its time or the
  model runs out at some length, the proof holds for every input with texts up to the last
  length fully proven: "proven for every input with text up to 4096 bytes; longer texts are
  tested, not proven", with the reason. lengua.lock records such a proof under `bounded`, with
  its length, never as a full one, and the done reply says so.
- Formulas: whole numbers (`+ - * / %` including chains of divisions, `abs`, `min`, `max`, `if`,
  and `scaled(x, places)` for a decimal's digits), printed plainly or with `places(x, k)`; and
  text: `len`, `count` (non-overlapping, leftmost first, as the test language counts), `words`,
  `upper`, `lower`, `tr`, `reverse`, joining texts with `+`, and comparing them with `==` and
  `!=`. All but `reverse` work byte by byte, which is exact on UTF-8; `reverse` goes by
  characters, as the test language does, telling them apart by each byte's values. Any placeholder whose variables all have fixed values in a shape is computed outright,
  so messages that quote an argument prove. A placeholder that's a text, drawn or computed,
  compares byte for byte.
- Instructions: the integer instructions, the bit instructions listed above, and the vector
  instructions programs use most:
  - moves of whole and partial registers, `movd` and `movq`, broadcasts, `vzeroupper`;
  - `pshufd` and `pshufb`; byte shifts and shifts by an immediate;
  - compares into vectors or masks, every `vpcmp` predicate, `vptestm` and `vptestnm`, `ptest`;
  - and, or, xor, and-not, `vpternlog`; adds and subtracts, saturating bytes, `pminub`, `pmaxub`;
  - `pmovmskb`, `vpmovm2*` and `vpmov*2m`, and every mask register instruction (`kmov`, `kor`,
    `kand`, `kandn`, `kxor`, `kxnor`, `knot`, `kshift`, `kortest`, `ktest`).

  The vector template, upcase and the word counts Claude Code wrote are proven for every input
  up to 4096 bytes, with no solver question. An instruction the proof doesn't model yet is named
  with its function: "main.s's count uses vpermb, which the proof doesn't model yet".
- Files: `openat`, `read`, `write` and `close` on the test's files. A missing input file gives
  `ENOENT`, and the checks on the files left after the run are the runner's.
- The network: `socket`, `connect`, `accept4` and `accept` on the listener, `read`, `write`,
  `sendto`, `recvfrom`, `shutdown` and `close` on TCP connections over IPv4 on this computer.
  The echo server is proven for every line and delivery; fetch's argument errors for every
  port, and its fetch for every path with every body up to about 140 bytes in its share of the
  time.

Not yet: shuffles across lanes (`vpermb`, `vpermd`), compress and gather instructions;
`sha256`, `crc32` and `crc32c` formulas and the instructions behind them; floating point;
threads; `tr` with sets beyond ASCII; and halvings the formula writes at 128 bits and the program
at 64, as in mean's block of whole numbers. Those blocks are tested, not proven, and the report
says why. A stream read both with `read` and with `fill` is proven for the four deliveries, not
every cut.

## Resources, threads and capabilities

lengua takes the path that safety-critical C (NASA's Power of 10 and JPL's coding standard) and
Rust share: everything a program may touch must be something the checker can account for.

| Need | C and C++ | Rust | lengua |
|---|---|---|---|
| Memory | `malloc`/`free`, `new`/`delete`: leaks, double frees, use after free | Ownership frees memory automatically; arenas without the standard library | Reserved up front: static buffers sized from the spec's limits, plus the stack. Nothing is allocated or freed while running (rule 3), and `.bss` costs nothing until touched |
| Threads | pthreads: any thread may write anything, and data races are undefined behavior | `Send`/`Sync`, scoped threads and rayon: no data races, checked at compile time | Fork-join only, when the spec allows threads. A canonical `parallel_for` (copied word for word, like `assert_fail`) starts workers with `clone` on static stacks and waits on `futex` for them all. Each worker owns its stack and one output slot, like Rust's `chunks_mut`: the checker proves it writes nothing else (JPL: one owner per shared object). No locks, atomics or shared mutation, so results stay deterministic and testable |
| System access | Any system call | The standard library, within the OS's permissions | Capabilities: the spec lists the system calls the program may make, and the kernel enforces the list (seccomp) and keeps files inside the program's working folder (Landlock) |
| Clock, random | `time()`, `rand()` | `Instant`, `SystemTime`, `rand` | System calls the spec lists, `clock_gettime` and `getrandom`, enforced by the kernel like the rest. Tests supply the bytes and readings, so output that depends on them is exact, and the prover covers every value the test's variables can take; without a supply, a rule such as "a number from 1 to 6" (`{any(1, 6)}`) checks the output, and the prover treats the kernel's values as any value |
| Network | Sockets | `std::net` | Plain TCP on the ports the spec names, to this computer unless the spec says `on any computer`. seccomp allows only TCP sockets (no UDP, raw or local ones), `sendto` without an address, and `accept4` on descriptor 3; Landlock lets `connect` reach only the spec's ports, and lengua's supervisor only this computer. A server never binds: the launcher listens on its port and hands it the socket as descriptor 3, and only its accept loop may run forever (`# bound: forever`, NASA's exception). Tests bring their own server or client on a free loopback port; websites and APIs go through the agent's own tools, which pass the data in |

A worker gets its index and its slot. It and every function it calls write only their own stack
frames and the slot, which it keeps in `rbx`, inside bounds checks; they make no system calls; and
each thread's stack is sized from the worker's deepest call path. Combining `results` in index
order makes reductions deterministic, and laying the slots end to end makes a map's output one
`write`. The kernel allows `clone` only with `parallel_for`'s flags, and `assert_fail` ends every
thread with `exit_group`, so a failed check anywhere stops the whole program.

Roadmap, in order:
1. proofs of decimal arithmetic written in a different shape, and of every way a network may cut
   a message;
2. the other platforms, toward the top six CPU and OS targets.

## Platforms

The MVP builds for Linux x86-64 (ELF, direct system calls), assembled by LLVM's `llvm-mc` and
linked by `ld.lld`. LLVM is the toolchain on every OS. Later, in this order: Linux ARM64 (testable
here with QEMU), macOS ARM64 and macOS x86-64 (on a Mac, through Apple's LLVM; Rosetta 2 runs no
AVX-512), Windows x86-64 and Windows ARM64 (import libraries generated by `llvm-dlltool`, no
Windows SDK). The server is organized around
per-(OS, CPU) platform profiles so each platform is an added profile, not a rewrite.

The plugin's own binary will ship for x86_64/aarch64 Linux (musl), aarch64 macOS and x86_64/aarch64
Windows.

Next: the proof step and the other platforms in the order above, in an order the project owner
decides.

## Code conventions

- The official Rust Style Guide and Rust API Guidelines, plus the parts of Google's Rust style guide
  that apply outside Google's own codebases.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: zero warnings, like rule 10.
- No `unsafe`. `Result` for recoverable errors, `expect("why")` instead of `unwrap()`, newtypes for
  distinct values, doc comments on public items.

## Threat model and known limits

- The lock's signature detects edits not made by the server. It is not a security boundary: any
  process running as the user, including the AI's shell, can read the key.
- Locks are machine-bound: on another machine the spec has to be recorded again.
- Without hooks, the server detects and refuses rather than prevents. Someone could still assemble
  `main.s` by hand outside lengua, but that isn't a lengua run, and nothing lengua stamps or runs is
  affected.
- There is no approval step: the person's request is the go-ahead, and the agent records the
  spec with their words (`words = ["…"]` in the lock). lengua can't tell real words from words
  an agent made up, so it shows instead of gating: every request of a build stays in the lock
  until its code passes, every change to the spec after the build's first request and to the
  tests after code first ran is shown as a diff, and lengua itself marks test changes that test
  less. lengua relies on nothing from the harness, such as its permission prompts, so a
  harness in an auto mode is as safe: what a program may touch is the spec's list, which the
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
run, and 6 end to end, each committed after the project owner approved it. `tests/e2e.rs` runs the
whole workflow on `examples/add` in both MCP protocol generations.

Dialect 2 followed:
- vectors from SSE2 to AVX-512, with masks and broadcasts;
- hashing and checksum instructions;
- floating point in double and single precision, with fused multiply-add, plus exact decimals in
  the test language;
- inputs up to 1 MiB;
- every value tried when a range is small;
- files in tests, and the kernel confining every run to what the spec allows;
- fork-join threads, with each worker proven to write only its own stack and slot;
- proofs of programs for every input in range over whole numbers, decimals,
  choices, texts (beyond ASCII too) and text formulas, files, the network, and random bytes and
  clock readings the tests supply, in integer, bit and vector code from SSE2 to AVX-512, with
  every step confirmed by a second solver, and over long texts up to a stated length; through
  `fill`, for input cut into pieces any way;
- the clock and random bytes, which tests supply through a ptrace supervisor, and rules such as
  `{any(1, 6)}` where they don't;
- plain TCP clients and servers on the ports a spec names, tested against their own loopback
  server or client, whose client keeps the connection open as curl does;
- one `check` tool for the whole loop, each call back within about 40 seconds while the tests
  and the proof run in the background.

All of it is checked on this machine (x86-64 Linux). The long-term goal is the top six CPU and OS
targets, through LLVM and one profile per target; they come next, in the order Platforms gives.
