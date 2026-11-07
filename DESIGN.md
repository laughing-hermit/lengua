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
| `idea.md` | the AI, from the person's request | The source of truth, the mother of everything else, and the only file people share: natural language, Given/When/Then behaviors with IDs whose steps are their example tests, the generated tests under each behavior, and what the program uses. An [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format) (OKF) concept document, the same on every target |
| `main.s` | the AI, compiling the spec | x86-64 or AArch64 assembly in the lengua dialect for one target. It's disposable: each computer compiles its own from the spec. Once it passes, lengua writes its second line, naming the version of the spec it was compiled from, the target and the CPU features it needs |
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
every computer. `idea.md` is the living documentation, and the lock keeps the code in step with
it. The parser enforces the structure (a Purpose, Given before one When before Then, one exit
code, unique IDs and titles, generated tests under their behavior with a well-formed table, and
only `- Uses:`, `- Network:` and `- Threads:` under Constraints); the write-idea skill teaches the
style.

**A spec names what the program uses, not system calls:** `- Uses:` lists `standard input`,
`files`, `the clock`, `random bytes`, `the environment` and `the terminal`, `- Network:` the places it connects
to (a TCP port of this computer, or of a computer it names) and the port it listens on, and
`- Threads:` how many cores it may use. Each target maps them to its own
system calls, so the same spec means the same on every computer. A program gets environment
variables only through `the environment`, and only those its test or the user's `run` names. They
reach the program alone: lengua's launcher, which runs before the program's confinement, gets
them as an argument and sets them only as it becomes the program, so a variable such as
`LD_PRELOAD` can't reach lengua's own code.

| What changed | Result |
|---|---|
| `idea.md` | The next `check` records it under the request's words and checks its tests and the code against it again. Changes since the build's first request are shown, with lengua's own marks on test changes that test less |
| `main.s` only | The next `check` checks the rules, runs every test and proves it again; once the tests pass, the new code is stamped |
| main.s's second line only | Nothing: lengua's hash of main.s leaves its own line out, and writes it back after the next pass |
| nothing | `run` works, and `check` answers from what it found |

## What ships

The plugin folder `plugin/` contains:

- **Skills:** `write-idea` writes the spec, its tests included, and `compile` compiles it into
  `main.s` for this computer. Each teaches one file and ends with `check`.
- **MCP server** `lengua-mcp` (Rust, official `rmcp` SDK) with two tools:

| Tool | Does |
|---|---|
| `check` | Every stage, stopping at the first that needs work: validates `idea.md`, its tests included (coverage, cross-checks, red check), and records it with the person's words; assembles, links and checks the rules; then, in the background, runs every test, counts the instructions executed, stamps and signs, proves the program, and writes main.s's second line. Each reply has one `next_step` |
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
`scripts/build-plugin.sh`, not committed). `scripts/release.sh` builds the release archive of the
computer it runs on, `dist/lengua-<version>-<target>.tar.gz` (a `.zip` on Windows) for one of the
five targets, with its SHA-256 beside it, so installing needs no Rust toolchain and no other tool.
The archive is the plugin, ready to load: the skills and the manifests (on Windows naming
`lengua-mcp.exe`); `server/lengua-mcp`, built with none of the shell's `RUSTFLAGS` and on Windows
with the C runtime linked in; `solvers/` with the z3 and cvc5 that `scripts/ci/tools.tsv` pins for
the target, checked by SHA-256; `LICENSE` and `licenses/`, with each solver's licences (cvc5's
with those of the parts its release build links in, each fetched from the commit cvc5's build
takes the part from and checked against its git object id, and on Linux the Ubuntu copyright
files of the system libraries it holds), every Rust crate compiled into the server with its
licence and the licence files it ships, and a `README.md` that lists them all; `TARGET`;
and `install.sh` or `install.ps1`. The prover, and `lengua-mcp doctor`, look for each solver in
the `solvers` folder beside the server's own folder first, then on PATH. The install script
refuses another system or CPU (on Linux, a glibc older than 2.39 too), naming the archive and
install script to use, clears macOS's download mark (with `--check` too), copies the plugin into
a marketplace folder of lengua's own, `plugins` in lengua's data folder (or `--prefix`, where it
replaces only an earlier install, which its `TARGET` file shows, and lengua's own
`marketplace.json`), adds it
to Claude Code with `claude plugin marketplace add` and `claude plugin install` when `claude` is
installed, and runs `lengua-mcp doctor`: the server, the engine (compiling and running a module
with every feature on), the CPU, both solvers and their pinned versions, whether the data folder
and `PLUGIN_DATA` can be written, and the clock's resolution, one line each and what to do about a
problem. CI's `release` mode builds each archive on its own runner, then installs it on a clean
runner of the same system and CPU, runs doctor, and runs the release check
(`tests/release_check.rs`): add from its idea to done through the archive's own server with
rmcp's client, run through `run` and from a shell, and attested. Nothing is published or signed.
A test holds the crate's version and both manifests' to one. The server keeps its private data in
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
so expected output stays text everywhere and only matching reads the markers. A test can close
the program's standard output (`- And standard output is closed`), a pipe nobody reads as after
`prog | head -1`, so every write to it fails with `EPIPE`, never a signal, and the exit code a
spec promises when writing fails is tested and proven like any other; the prover's write then
returns the same error.

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
- Landlock lets a program that has a channel to lengua's broker (below) touch no file at all: it
  may execute and read its own binary and nothing else, make no network connection and bind no
  port, and signal no other process. Every file and folder it uses is opened for it by the
  broker, which hands the descriptor over. A program with no channel keeps its working folder,
  under the same network and signal limits, and so does a program QEMU's user mode runs (an ARM64
  program on an x86-64 computer), whose own system calls QEMU needs the folder for, so there the
  folder stays granted and the broker serves the same names. The launcher refuses to start any
  program on a kernel where Landlock isn't in force;
- the program gets no descriptor but its standard input, output and error, a server's listener
  and its connections, and its channel to the broker at 4,352: the launcher closes every other
  one it holds before it confines the program, so one a parent of lengua's left open, a harness
  or a CI runner, never reaches it (Landlock holds paths, not descriptors already open;
  GitHub's x86-64 runner showed two);
- the program holds no network of its own. lengua makes each connection its spec names before
  the program starts, outside its confinement (a TCP port of this computer, tried on 127.0.0.1
  and then ::1, of a computer the spec names, or of the computer one of the program's arguments
  names, which lengua reads as the program starts, as a curl-like tool's user types the host; an
  argument naming this computer or no computer gives a connection lengua couldn't make, and the
  tests' server stands in for any value), and hands it over as file descriptor 4, 5 and
  on, in the spec's order, as it binds a server's port and hands it the listening socket as
  descriptor 3. The program's end is a local stream that lengua relays to the place both ways:
  what the place sends arrives on it, the place's end of sending is the program's end of input,
  and the program's `shutdown` of its sending side is the place's. A connection lengua couldn't
  make, or one the place broke off, is shut both ways, so a read gives 0 once what had arrived
  is read and a write fails with `EPIPE`; lengua keeps its end open until the program ends, so
  the end is never a reset. The program can't tell a refusal from a break; `run` reports which
  happened. A connection `over TLS` (to a computer the spec names) is the same to the program:
  lengua does TLS on it in Rust (rustls, with ring's cryptography), outside the program,
  checking the computer's certificate for its name against Mozilla's roots, built in, so every
  system trusts the same certificates; a certificate it can't check, or a stream cut off without
  TLS's close, is a connection it couldn't make or one broken off. The program sends and reads
  plain bytes and never holds crypto, and its tests use the stand-in server in plain bytes too.
  The same holds on every system, since any sandbox can hand a program a local stream
  (a socketpair on Linux and macOS, a pipe or socket handle on Windows), and none of them must
  police the program's own connections: the strict rule of one product on every target
  (below);
- a program that uses files is held to the names every system takes, by lengua's broker, which
  answers each `os_open`, `os_mkdir`, `os_remove` and `os_rename` and reads its name
  (`src/names.rs`): a name Windows refuses (its characters `< > : " \ | ? *`, controls, a dot or
  space at the end, a device name it reserves, such as `CON` or `com1.txt`, or the shape of a
  short (8.3) name, such as `REPORT~1.TXT`, by which Windows can open another file) gets `EINVAL`; one
  that isn't UTF-8 of characters Unicode 9.0 assigned, which macOS's APFS refuses, `EILSEQ`; a
  name past 255 bytes or a path past 1,023 (macOS's `PATH_MAX`), `ENAMETOOLONG`; a new name that
  differs only in case or Unicode normalization from another in its folder, one file on macOS and
  Windows, `EEXIST`; and a file renamed or removed while the program holds it open, which Windows
  refuses, `EBUSY`. The edge also tells the broker whether the program listed a folder: a test
  whose program did runs again with each listing turned round, since no system promises an order,
  and fails if its result changes. The prover gives the same answers for names it knows, and
  splits a name the input chooses on Windows's ASCII rules. The rules stay with only Linux
  targets: a project folder reached from WSL 2 (`/mnt/c`), shared into a Mac's Linux VM or
  bind-mounted into a container on either is a Windows or macOS file system underneath, and a
  spec's tests must mean the same on it;
- the program holds at most 4,352 files open (one number on every target, within macOS's
  `OPEN_MAX`); its arguments take at most 32,000 characters of Windows's command line and its
  environment's names are unique ignoring case, or it doesn't start; and a write to a closed
  pipe or connection returns `EPIPE` rather than raising SIGPIPE, as on Windows;
- seccomp kills it at any system call its spec doesn't allow: the calls the operations its spec
  names make, plus the `write` and `exit_group` that `assert_fail` may always make. Its channel is
  held to the edge's own use: `sendmsg` and `recvmsg` work only on descriptor 4,352, and every
  call that takes a descriptor first (`read`, `write`, `close`, `lseek`, `fstat`, `getdents64`,
  `shutdown`, `sendto`) is refused on it, so nothing but lengua's edge talks to the broker. A
  program with a channel has no `openat`, `mkdirat`, `unlinkat` or `renameat` at all: the broker
  makes and finds every path for it, so it can't probe what Landlock keeps it from opening;
- resource limits cap the files it writes (64 MiB in tests) and leave no core files. A write that
  would take a file past the limit is cut short, and the next fails with `EFBIG`, on every
  target: the launcher ignores SIGXFSZ, so the system's own answer reaches the program
  (Windows has no such limit, so its edge will count the bytes and answer the same way).

The checker already proves the code makes no system call of its own: every operation is a call
into lengua's edge, which makes only the calls its operations need. The kernel makes the promise
hold even if the checker missed something.

**The broker** is lengua's one service for what a program may not do for itself. It runs in the
process that started the program (the server, or `lengua-mcp run`), as a thread for each program
that has a channel to it, and talks to the program's edge over a local stream the program inherits
at descriptor 4,352 (`src/broker.rs`). A program has a channel when it calls `os_open`,
`os_mkdir`, `os_remove`, `os_rename`, `os_list`, `os_clock`, `os_sleep`, `os_random`, `os_accept`
or `os_poll`, the operations that link the channel's code; one that calls none of them has none,
in a test or in `run`. The broker speaks first, before the program starts, with a message naming
the protocol (a magic number and a version), whether this is a test or a `run`, and the file
limit; an inherited descriptor 4,352 that doesn't begin so is not a broker, and the edge calls the
system itself, as a program run outside lengua does (lengua's guarantees hold for what `run` runs
after `check`). Every message the program sends is hostile input: the parser accepts exactly the
messages of the protocol and the service ends at the first that isn't, and it has its own fuzz
target. The broker does its work in the server, outside the program's confinement, so the kernel
can't hold the program to its spec for it: the broker does. It does only what the program's spec
allows (names for a spec that names files, the clock, random bytes), and a request for anything
else ends the service. It is also held to a stop: a program that doesn't read its replies is cut
off after 5 seconds, the server shuts the broker's end of the channel when the program ends, and
waits for the service at most 5 seconds more, so a program can't keep `check` or `run` from
returning.
- *Names.* A request for a name carries the name, at most 1,023 bytes. The broker holds it to
  what every system takes (`src/names.rs`), matches it against the folder's entries byte for
  byte, answers `EBUSY` for a file the program holds open and then renames or removes (the edge
  tells it of each close; the table of open files holds at most 4,352, which a program can't
  exceed, and `EMFILE` answers the request past it), and resolves every path beneath the
  program's folder without following links (`openat2` with `RESOLVE_BENEATH`,
  `RESOLVE_NO_SYMLINKS` and `RESOLVE_NO_MAGICLINKS` on Linux; a link or a way out is `EACCES`).
  Only a file or a folder is opened: a named pipe, a device or a socket in the folder is
  refused with `EACCES`, and nothing waits for its other end. A file or folder comes back as a
  descriptor sent with the reply (`SCM_RIGHTS`). One round trip is what a name costs.
- *Tests.* The broker also answers the clock and random bytes from the test's supply, in the
  order the program asks, and the edge ends every sleep at once, since what the program does
  next can't depend on how long it slept and a test shouldn't wait for it; a real `run` sleeps
  as long as the program asks. A program that waits for a connection or for input tells the
  broker, so the test knows it is waiting. A program that lists a folder tells it too, and the
  test runs again with each listing turned round.
- *`run`.* The broker serves names alone: reads, writes, seeks, polls, listings of a handed
  folder, the clock, random bytes and sleeps go straight to the system.

Signals, threads, the exit status, the counters and the time limit work as in any run. A supplied
call adds wall time, a message each way, and a few of the edge's instructions to the program's
count, the same few for every program, and never any of the program's own. The counters report
three measures, which the optimize step compares by their medians: instructions executed in the
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
tests proven for every input in range or up to a length, the solvers that checked them, the tests
it found wrong, with what it found, the moment it finds them, and why each other test is tested,
not proven. `run` refuses a build that table finds wrong, however often lengua restarts, and a
later `check` shows what was found, and why a test isn't proven. A test proven there is proven
memory-safe on the same runs, so the table says that too, with no field of its own. Checking
tests again drops them; a build that is byte for byte the same keeps them, and any other build is
proven again by itself.

## The edge

A program defines `main` and reaches the system only by calling one of 21 operations of lengua's
**edge**, which lengua links after it: `src/edge/mod.rs` defines them once, and each operating
system's edge file (`src/edge/linux-x86_64.s`, `linux-aarch64.s`) does them in that OS's way and
turns its answers into the same ones. So a program's text is written once per CPU and means the
same on every OS, the OS's layer is written once per target, and each edge is as thin as its OS
allows (raw system calls on Linux; libSystem's thinnest calls on macOS and kernel32 on Windows,
which come with their ports).

| operation | arguments → result | allowed by |
|---|---|---|
| `os_read`, `os_write` | fd, buffer, length → count | input, files, network, terminal; write always |
| `os_open`, `os_close` | name, mode → fd; fd → 0 | files (close: network too) |
| `os_seek`, `os_info` | fd, offset, whence → offset; fd, out[16] → 0 | files |
| `os_list` | folder fd, buffer, length → bytes (0 at the end) | files |
| `os_mkdir`, `os_remove`, `os_rename` | name; name, kind; from, to → 0 | files |
| `os_clock`, `os_sleep` | clock, out[16] → 0; clock, absolute?, time[16] → 0 | the clock |
| `os_random` | buffer, length (at most 256) → count | random bytes |
| `os_accept`, `os_shutdown` | listener → fd; fd, how → 0 | a server; network |
| `os_poll` | table, count, milliseconds (−1: forever) → ready | several clients, the terminal |
| `os_terminal`, `os_window` | mode → 0 or −ENOTTY; out[8] → 0 or −ENOTTY | the terminal |
| `os_thread_start`, `os_thread_join` | index, slot → 0; index → the worker's result | threads (lengua's library only) |
| `os_exit` | status → never returns | always |

- **Arguments and results.** On x86-64 the arguments go in rdi, rsi, rdx, r10, r8, r9 and the
  result comes back in rax; on AArch64 in x0 to x5 and x0. An operation changes only the
  registers its CPU's convention lets it: rax, rcx, r11, the flags, the vector registers and the
  mask registers k0 to k7 on x86-64; x0, x8, x16, x17, x30, the flags and the vector state on
  AArch64 (v8 to v15's low halves are kept). After a call that reaches the OS, directly or
  through `fill`, `parallel_for` or one of the program's functions that makes one, a register in
  that set can't be read before it is written, which the checker holds the program to; the prover
  models an operation whole, as the system call it makes (the result in rax or x0, rcx and r11 as
  `syscall` leaves them on x86-64, the SVE state cleared on AArch64, every other register as it
  was). A program reloads its vector constants after a read, a few loads per 64 KiB, which costs
  less than saving AVX-512 or SVE state on every call.
- **Errors** are one closed set, Linux's numbers negated, on every OS: 2, 5, 9, 11, 13, 16, 17,
  20, 21, 22, 24, 25, 27, 28, 29, 32, 36, 39, 84, 104 and 107. Each operation lists its subset,
  the prover returns exactly that subset, and an error outside it is `EIO`, on the error path
  only. On macOS and Windows, if their native ports return (todo.md, "Deferred"), `errno` and
  `GetLastError` will map through tables in their edge files, and a test will require each table
  to cover the set.
- **Data** is the same bytes on every OS. `os_info` gives {size, kind}: kind 0 a file, 1 a
  folder, 2 anything else, the size 0 unless a file. A listing entry is {u16 length (a multiple
  of 8), u8 kind, u8 name length, name, NUL, padding}, with `.` and `..` left out. The open modes
  are READ 0, WRITE 1, APPEND 2, NEW 3 and FOLDER 4, and names are relative to the working
  folder and use `/` on every OS. `os_poll` takes Linux's 8-byte entries with POLLIN 1, POLLERR
  8, POLLHUP 16 and POLLNVAL 32. The terminal's RAW mode is no echo and no line editing, with
  signals and output processing kept.
- **Start and end.** lengua's `_start` calls `main(argc, argv, envp)` with the three in the first
  argument registers (on Linux from the start stack), and `main` never returns: it ends with
  `os_exit` (`exit_group` on Linux). A program has 4,352 descriptors, 0 to 4,351; the channel to
  its broker is 4,352, which every operation refuses when a program passes it.
- **Threads.** lengua's `parallel_for(count, slots, slot_size, results)` runs the program's
  `worker(index, slot)` on up to 64 threads, each with a static 1 MiB stack of lengua's own (on
  Linux a `clone` into `.bss` and a `futex` to join them); the checker holds the worker's deepest
  call path under what a stack gives it, and every frame under 4,096 bytes, since Windows grows a
  stack a page at a time and these stacks have no guard page.
- **Standard input** of up to 4,096 bytes is in its pipe before a test's program starts, so one
  read returns it whole on every OS, which is the condition the proofs state.

**Sources.** Two OS-neutral CPU libraries (`src/edge/x86_64.s`, `aarch64.s`: `assert_fail`,
`fill`, `parallel_for`) and one edge file for each OS and CPU are embedded in the server. Each
build writes lengua.s, the `_start`, the operations main.s calls, the library functions it calls
and doesn't define, and what they need, always in the same order, and assembles it with main.s as
one object after main.s's last line, so main.s's line numbers don't move and a name clash is an
assembler error. `lengua-mcp link main.s` prints the same text, so a program can still be built
by hand. Only what a program calls is linked.

**What the checker holds.** The executable's functions form two units, main.s's under every rule
and lengua's, matched line by line to the text lengua assembled and exempt from the program's
rules. It refuses `syscall` and `svc` in main.s; a name lengua keeps (`_start`, `os_*`,
`lengua_*`, `LENGUA_*`, `.Llengua_*`, `assert_fail`, `assert_message`, `fill`, `parallel_for`);
`main` returning; a call of an operation the spec doesn't allow (a call of a library function
counts as the operations it makes); and main.s entering lengua anywhere but an operation's or a
library function's entry, or touching its data. The `.equ` lines of main.s for the errors and
modes it compares against are its own constants, which the checker doesn't hold to the edge's
values (`references/edge.md` lists them, and a test holds the edge files' own `.equ` lines to the
same tables). lengua's own functions, the edge files' and the libraries', are read for their
control flow and stack, and two tests keep their text sound: the dialect's parser finds fault in
it only with how its data is laid out and with what only lengua's own text does (system calls,
its reserved names, an exported `_start` and no `main`), and its functions have a flow and a
stack without a fault of the kind rules 1, 3 and 4 find, apart from the guard after an exit that
can never run and the calls to the one helper that needs no aligned stack.

## Performance

The bar is that a program the agent writes through lengua takes less CPU time, less wall time,
less peak memory and a smaller file than the best of C, C++ and Rust built with every optimization
for the CPU, on every workload and every target. It is one program, because a person runs one: the
fastest tier the CPU supports must win all four measures at once.
- A lead in time counts when the runs show it: they alternate between the languages, and lengua's
  median must be lower with 99% confidence. A difference within the noise doesn't count.
- Memory and file size are compared as measured, and lengua's must be smaller.
- The others are shown at their best on each measure, which can be a different program for each
  (threaded for time, not for memory), and lengua's one program must be ahead of both.
- Instructions and cycles are reported rather than compared, since vector code does more per
  instruction.

`cargo bench --bench against_rust_and_c` measures it on seven workloads, from a program that is
mostly starting up to 64 MiB streams, and writes BENCHMARKS.md. README says where lengua isn't
ahead yet. A gate on every target's runner, with nothing excused, is round 5 of the roadmap in
todo.md.

Speed comes from the agent, guided by the compile skill, which makes it a step of the
normal flow: once the tests are green, one pass for the widest vectors the CPU has (with masked
loads for tails), independent chains where each step would wait on the last, large reads and
writes, and streaming for input of any length. `assets/fast` holds checked programs for each
pattern, which the benchmark measures: a word count 16 bytes a step with SSE2, the baseline form
every x86-64 computer runs (and NEON on ARM64), with a faster tier beside it where the CPU has
one, 64 bytes a step with AVX-512 masks (SVE on ARM64); a CRC-32C that runs three `crc32` chains
and joins them with `pclmulqdq` (its fast tier `crc32c-pclmul`; on ARM64, whose CRC unit one chain
already keeps busy, one `crc32cx` chain beside `pmull` folds on the vector units, the tier
`crc32c-crc`), and a four-table histogram. SHA-256, AES-128 and CRC-32C each also have a baseline
form that runs on every computer of its target and is the shipped one, the instructions' forms
being named fast tiers beside it from the very same spec: `sha256-ni` and `aes-ni` on x86-64,
`sha256-sha2` and `aes-aes` on ARM64. Through a pipe the wider tier does not always pay: on
GitHub's runners the AVX-512 word count saved CPU time on three of four draws and wall time on
none, and on one it was slower than the SSE2 form on both (README.md names the six runs); the pipe sets
the pace. The
fork-join template works in place: input lands straight in the workers' slots, which halves the
memory a threaded program touches. Each program processes what each `read` returns as it comes,
so its work overlaps the writer filling the pipe.

What lengua doesn't do for speed is give up a check: bounds checks stay unless a reported
`# unsafe:` block shows they can't fail, every loop is bounded, and vector code is tested
exhaustively at its edges by the generated tests, and proven wherever the prover models its
instructions (see the vector instructions it follows, below).

## What lengua does, and what it leaves to the agent

lengua is an agent plugin: skills plus an MCP server, nothing else. The server does only what the
agent can't be trusted or equipped to do: recording the request and the signed lock, the checks
that keep tests and code true to the recorded spec, marking test changes that test less,
building, running under the kernel's confinement, serving a program's names and a test's clock and
random bytes (the broker), measuring and proving. Everything generative or advisory is the agent's, guided by the
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
3. Resources reserved up front: memory is static (`.bss` sized from the spec's limits) plus the stack, nothing is allocated while running, one thread unless the spec allows more, and only the operations of what the spec names (write and exit by default), with a computed maximum stack size.
4. Functions of at most 60 instructions, standard calling convention, one exit.
5. At least two assertions per function on average; a failed assertion exits with code 134.
6. Smallest scope: one owning function per buffer, nothing exported (`main` is the entry, which lengua's `_start` calls), read-only data stays read-only.
7. Every result is checked (`rax` < 0 means error).
8. No macros; named constants for exit codes, sizes and loop bounds.
9. No function pointers, and no jump through a register but a checked jump table's; indexed accesses are bounds-checked over their full width (16 to 64 bytes for vectors), except inside a reported `unsafe` block.
10. Warnings are errors, every change is checked, and the dialect is versioned.

The checker analyzes the **linked executable** together with the source, and nothing AI-written
runs before it passes the rules. It reads `main.s` (one instruction per line, constants before
use, no macros), builds it, decodes the executable (with iced-x86 for x86-64, with lengua's own
decoder for AArch64) and matches every instruction to its line, operands included; anything that
doesn't match is a violation, so what runs is exactly what `main.s` says. Then it builds each
function's control flow and exact stack depth and checks the rules: unreachable lines are
violations too. `assert_fail`, `fill` and `parallel_for` are lengua's own code, linked after
main.s from lengua's library and never copied into it: the executable's functions form two units,
main.s's, matched line by line to main.s under every rule, and lengua's, matched to the text
lengua assembled and exempt from the rules a program's code must keep (`assert_fail` may always
write its message to standard error and exit with 134, whatever the spec allows). An assertion is
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
calls, operations included, and what it returns) carries owners across calls, so no header has to
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
| request → `idea.md` | the AI | No machine can check intent, so lengua shows it instead: the spec is recorded with the person's words, every change after them is kept as a diff, and the agent's report says what the spec promises and what changed. New words in the middle of a build join it rather than start over, so no change drops out |
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
- **The network is the tests' own peers.** Each connection the spec names is made before the
  program starts, at its own descriptor: answered by the test's server for it, which speaks on
  the program's own stream, or handed over ended when the test has no server there. The server
  waits for exactly what it must receive, so a read before the program has sent all of it waits
  forever: the run fails, as a real one times out. A server program takes the test client's connections on its listener in order; taking
  the next before closing the last waits forever too, and with none left the program is serving.
  The test's client behaves as curl and browsers do: it sends, keeps its side open until the
  program starts answering, then closes it and reads the rest. So a server that reads to the end
  of the request before answering waits forever, in the proof as in the tests. What a peer sends
  arrives whole, in the tests' three pieces (the first byte, up to the middle, the rest), or with
  two of those pieces together, each a shape of its own; standard input too arrives in those
  three pieces on every other generated run, so a program that reads it as if it came whole fails
  a test even where its proof doesn't finish, which mutation testing found; through `fill`, every way at once (see
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
- **fill is read as a whole.** `fill` (lengua's own library function, linked after main.s, which
  the proof finds by its shape) reads until its buffer is full, the input ends, or
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
  the session the agent runs in. A run of the model that makes terms by the million, a hash in
  plain integer code (the ARM64 SHA-256 twin filled a 16 GB runner in a minute), asks the same
  question each time its pool has grown by 50,000 terms and ends the same way. The run's loop
  can't be the only one to ask, since a single extraction through a hash's whole history makes
  terms by the million between two steps: the pool itself looks each time it has made or kept
  50,000 more, and once it found the memory short it answers every extraction with zeros at
  once, the run ends at its next step, and no shape or step of such a pool counts as proven.
  Short, for a pool, is below the floor and twice the pool's own size more, since its tables
  double when they fill, and pools that waited for the floor itself took a 16 GB runner from
  4.8 GB free to 0.2 GB between two samples ten seconds apart, probably in such doublings.
  Each proof worker, and every solver it starts, runs ten steps
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
- **Every reader is fuzzed, and the proofs are mutation-tested.** A seeded fuzzer
  (`src/fuzz.rs`) gives every reader lengua has random bytes and the repository's own specs and
  programs changed, cut, doubled and spliced: idea.md and its tests, main.s for each CPU and its
  header, the test language's formulas, the lock, the executable lengua built, and the AArch64
  decoder's machine words. None may panic or take 5 seconds. A short pass runs with every test
  and a long one, 40,000 inputs a reader, by hand; the first found a panic, a letter past ASCII in
  the search for text outside quotes, now fixed. Mutation testing (`tests/mutation.rs`, by hand)
  changes each proven example and template on both CPUs one way at a time, a number moved by
  one, a branch's condition flipped or a named constant moved by one, and checks each mutant,
  the same programs on both CPUs: of 110 (2026-11-02), 55 broke a rule, 47 failed a test, 5 were
  tested and not proven, and none that changes what a spec can see came out proven. The three
  that came out proven change nothing a spec can see (a limit past the spec's longest line, a
  jump to the next line, and a byte the input never holds), each listed with why, and neither do
  the five tested and not proven (an output file's close skipped after a write that worked, a
  file write's error test flipped where no write falls short, an error code returned only after
  5,001 reads, which can't happen, a buffer a byte past the spec's 5,000, and a close's error
  test flipped where both ways go on to the same line). The runs found two gaps, both closed:
  tests never cut standard input into pieces, and no test could make writing fail, so a
  program's exit code for a failed write, and write_all's error test flipped, went uncaught; now
  every other generated run cuts standard input, and a test can close standard output.

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
  `fill` 16 bytes a step with SSE2, is proven so with no solver question. A count of a
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
  one final length at a time; the line counter, with its last chunk masked by `bzhi` (the form the
  prover's test keeps; the fixture itself masks with SSE2, on the baseline), with a 64 KiB buffer,
  C's size, in under 30 seconds with no solver question. Counts that look back, as
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
  skill's fast `wc`, 16 bytes a step with SSE2, the baseline form every x86-64 computer runs,
  each step masked to the bytes left so the last partial step is the same code, is proven so for
  every length with its 64 KiB buffer, with no solver question, in about four minutes on a
  GitHub runner (4 vCPUs), on a CPU without AVX-512 as on one with it. Its mask is built so the
  proof can read it: the bytes left, at most 16, spread over the lanes by `pshufb`, `pminub` with
  each lane's place plus one, and `pcmpeqb` with the same, so a lane is a byte of the input where
  its place plus one is at most the bytes left, an unsigned comparison of the length. The same
  mask with `pcmpgtb` against the places is a signed comparison, which the reading of the
  length's conditions doesn't follow: the base then went one length at a time, its counts, a
  part for each byte of the buffer, handed to the solver, and the 64 KiB proof didn't finish.
  The AVX-512 tier beside it
  (`wc-avx512`, 64 bytes a step with `k` masks) is proven the same way, in about two minutes on
  the 16-core computer the tests were tuned on. A word count that forgets the byte before each
  buffer-full or each step, reads the carry from the wrong byte, or takes the larger of the bytes
  left and a step for the smaller is caught in either form; the baseline form's planted bugs
  also include a word start counted past the end.
  Masking every step costs the AVX-512 tier about half again the instructions and a third more
  CPU time than the old shape's separate tail, at the same wall time, since a pipe sets the pace
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
  does isn't proven". The CRC-32C fast tier's three chains join only past 12,288 bytes, so its
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
- The network: `accept4` and `accept` on the listener, and `read`, `write`, `sendto`,
  `recvfrom`, `shutdown` and `close` on the connections lengua made or the program took. The
  echo server is proven for every line and delivery; fetch for every path with every body up to
  about 140 bytes in its share of the time, and for a server it can't reach.

- Threads: `parallel_for`'s `clone` starts a thread that runs to its end at once, on the stack
  it's given and with a first frame of its own, while the thread that started it waits; its
  `exit` clears its id, as `CLONE_CHILD_CLEARTID` does, and the waiting thread goes on, so
  `join_worker` finds it done. No thread writes what another reads (M7), and `parallel_for`,
  copied word for word, joins every worker before it returns, so this order gives what any
  order gives. Every run is held to the memory rules across threads. A program that splits its
  input by 64 KiB a thread starts no thread on the 4096 bytes a proof covers, and its proof says
  it never ran `start_worker` or `join_worker`.

Not yet: the `sha256` formula, which the SHA-256 programs, in plain integer code and with the SHA
instructions, run under but no formula yet meets; a CRC joined by `pclmulqdq`, which runs but builds other terms than the chain of steps;
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
| Threads | pthreads: any thread may write anything, and data races are undefined behavior | `Send`/`Sync`, scoped threads and rayon: no data races, checked at compile time | Fork-join only, when the spec allows threads. lengua's `parallel_for` (linked from its library, like `assert_fail`) starts workers on static stacks (`clone` on Linux) and waits for them all (a `futex` there). Each worker owns its stack and one output slot, like Rust's `chunks_mut`: the checker proves it writes nothing else (JPL: one owner per shared object). No locks, atomics or shared mutation, so results stay deterministic and testable |
| System access | Any system call | The standard library, within the OS's permissions | Capabilities: the spec names what the program uses (standard input, files and folders, the clock, random bytes, environment variables, the network), the program reaches the system only through the operations of lengua's edge that those allow, and the kernel enforces them (seccomp) and grants a program that has a channel no path at all (Landlock): its broker opens the files, inside the working folder |
| Files and folders | `fopen`, `stat`, `readdir`, `mkdir`, `unlink`, `rename` | `std::fs`: `File`, `metadata`, `read_dir`, `create_dir`, `remove_file`, `rename` | Under `files`, inside the working folder only: open, read, write, seek, close, a file's size and kind (`os_info`), a folder's entries (`os_list`), and making, removing and renaming. Tests give empty folders and check what's gone and what folders remain. The proof follows folders, deletes and renames; a folder's listing stays tested, not proven, since the kernel orders it its own way |
| Environment | `getenv` | `std::env::var` | Under `the environment`: only the variables a test, or the user's `run`, names, set only as the launcher becomes the program |
| Waiting | `nanosleep` | `thread::sleep` | Under `the clock`: `os_sleep`, which a test ends at once and a real run doesn't |
| The terminal | `termios`, `poll`, `TIOCGWINSZ` | crossterm and the like | Under `the terminal`: `os_terminal` for the mode of standard input and `os_window` for its window's size, and nothing else, and `os_poll` to wait for a key. Tests give the keys as standard input, a pipe, where the mode can't be set, so a program goes on as from a pipe, and `\e` writes escape sequences; the proof models the same. `lengua-mcp run` puts the terminal back as it found it when the program ends, however it ends (`assets/terminal-template.s`) |
| Left out, by design | `fork`/`exec`, `malloc`, recursion, function pointers | `std::process`, `Box`/`Vec`, recursion, `dyn` | Running other programs (their code isn't checked); allocating while running (rule 3: buffers and arenas in `.bss`, sized from the spec, cost nothing until touched, and input of any length streams through `fill`); recursion and function pointers (rules 1 and 9: explicit stacks, and dispatch by comparison) |
| Clock, random | `time()`, `rand()` | `Instant`, `SystemTime`, `rand` | Capabilities the spec names, `the clock` and `random bytes` (`os_clock` and `os_random`), enforced by the kernel like the rest. Tests supply the bytes and readings, so output that depends on them is exact, and the prover covers every value the test's variables can take; without a supply, a rule such as "a number from 1 to 6" (`{any(1, 6)}`) checks the output, and the prover treats the kernel's values as any value |
| Network | Sockets | `std::net` | Plain TCP to the places the spec names, each a TCP port of this computer, of a computer it names, or of the computer an argument names, and TLS on a named computer's when the line says `over TLS`, done by lengua outside the program. The program never opens a socket or connects: lengua makes each connection before it starts and hands it over as descriptors 4, 5 and on, relaying it both ways, and one it couldn't make comes ended. seccomp allows no `socket` or `connect`, `sendto` only without an address, and `accept4` only on descriptor 3; Landlock allows no TCP connection or binding of the program's own. A server never binds: the launcher listens on its port, on the loopback unless the spec says `for any computer`, and hands it the socket as descriptor 3, and only its accept loop, or its loop around `poll`, may run forever (`# bound: forever`, NASA's exception). A server serves one client at a time unless its spec says `up to N clients at once`: then seccomp allows `poll`, the event loop C servers use, over a fixed table of connections (`assets/server-template.s`), and a test whose clients connect at once makes each wait for another's answer, so a server that serves one at a time fails; the proof follows a server that polls through every order its clients' steps can come in, and through the test's own. A test's servers speak on the program's own connections, and its clients connect to the program's listener on a free loopback port. Each `connects to` line names one connection to each place, at most 16 |

A worker gets its index and its slot. It and every function it calls write only their own stack
frames and the slot, which it keeps in `rbx` (x19 on AArch64), inside bounds checks; they call no
operation; and each thread's stack, 1 MiB of lengua's own, holds the worker's deepest call path,
which the checker holds under that. Combining `results` in index order makes reductions
deterministic, and laying the slots end to end makes a map's output one `write`. The kernel
allows `clone` only with the flags lengua's `parallel_for` gives it, and `assert_fail` ends every
thread with `exit_group`, so a failed check anywhere stops the whole program.

Roadmap, in order (round by round in todo.md, the design in ROADMAP.md):
1. reach (round 3): a Linux next to the agent everywhere (Ubuntu 24.04 natively, in WSL 2 on
   Windows, in a Lima VM on a Mac), one package for it tested on clean runners, the kernel floor,
   and optionally a server that installs on more Linux; a container image later, if wanted;
2. the performance finish line, a gate on both targets' runners (round 5);
3. prover parity across the CPUs, the small items, the documents and the release archives
   (round 6).

Native macOS and Windows ports are deferred (todo.md, "Deferred").

Proofs past the finish line are research (todo.md, "Deferred"); "Status" below lists what is
tested, not proven, today.

## Platforms

lengua builds, checks, proves and runs programs for Linux on x86-64 and on ARM64 (ELF, reaching
the system through lengua's edge; tested natively on GitHub's runners of both, and the ARM64 ones
also under QEMU on the x86-64 one), assembled by LLVM's `llvm-mc` and linked by `ld.lld`. Those
are its two targets, and every system runs them the same way: on a real Linux kernel next to the
agent, on Ubuntu 24.04, the GitHub runners' own system. A Linux laptop runs lengua directly (Ubuntu
24.04 promised; Fedora 40+ and Debian 13+ when `doctor` passes), Windows inside WSL 2, whose
Microsoft kernel has Landlock on, and a Mac inside a Lima VM; the agent runs inside too, and lengua
ships as one package for that Ubuntu, tested on clean runners (planned: todo.md, "Round 3";
README.md, "Windows and Mac"). The Windows and Mac routes are documented, not yet tested: their
kernels are the users', and a route is promised only once a GitHub runner has run lengua's checks
inside it. A container image of the same package may come later. Native macOS and Windows ports are
deferred: each would bring its own edge file and confinement (the plans are in ROADMAP.md), and
macOS on x86-64 is not planned at all, since Apple makes no Intel Macs any more (todo.md,
"Deferred").

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
  program, under the host's own sandbox — seccomp and Landlock and the broker on Linux,
  something else on macOS and Windows — so both need that CPU, natively or under an emulator.
  Under QEMU's user mode, which runs Linux ARM64 programs on an x86-64 Linux host, the kernel sees
  QEMU's own system calls, not the program's: Landlock and resource limits still confine a run,
  but the spec-exact system-call filter and the cycle counts need the target natively (GitHub's
  ARM64 runners). The broker serves such a program as any other. So the per-host part of the
  server is the runner (item 6 below). It's also why programs run only on Linux x86-64 and Linux
  ARM64: seccomp and Landlock are Linux's, so macOS and Windows would each need a confinement of
  their own, and lengua reaches them through a Linux that has Landlock instead.

**The plugin decides per target.** The server detects its host at startup (`Profile::for_target`
over `std::env::consts`) and uses that target, so the user installs one plugin and it picks what it
needs rather than choosing a build from a list. On a host that isn't one of the two Linux targets
it says so and points to "Windows and Mac" in README.md, rather than failing deep inside.

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
io_uring; but the spec's exact filter and the counters would see QEMU, not the program. The
broker serves the emulated program as any other, so its tests' names, supplied random bytes and
clock and instant sleeps work, and a client's too, since lengua makes its connections. Nothing is
measured, and the reply says what held the runs. lengua.lock records what ran the tests
(`emulator = "qemu-aarch64 10.2.2"`), and `run` won't run code tested one way in a session of
the other. lengua's line in main.s is the target CPU's comment, `// lengua: …` on AArch64.
Distribution is then: one package for Ubuntu 24.04, an archive for each Linux target with an
install script that runs CI's own setup steps (planned: todo.md, "Round 3"; a build for more
distributions and an image are optional and later); the skills are the same everywhere.

**Building and testing without owning the hardware.** A target doesn't need its machine bought,
and nothing is built or run on the owner's computer. GitHub's free runners are real Linux
machines on both CPUs, and `ci.yml` builds and tests each target natively on its own; QEMU's user
mode adds the other CPU's Linux programs on the x86-64 Linux runner. The runners run Ubuntu 24.04,
the system every route ends in, so they test the userland every user gets. No free runner is
known to run WSL 2 or an Apple-silicon Mac's VM. But a Mac's guest is Ubuntu's own cloud image,
which the Linux runners can boot under Lima and QEMU, so lengua's checks can run inside it there;
a probe reads the kernel under WSL 2 on a Windows runner, under Lima on an Intel macOS one and
under Lima on the Linux runners, and each route is promised only once a runner has run lengua's
checks inside it (todo.md, "Round 3").
What's written per target is the backend and that host's runner.

### Porting a target

`src/platform.rs`'s `Profile` holds the triple, the short name and the linker's flags, and
through `src/syscalls.rs` each CPU's Linux: its system call numbers and the details of the ABI
that differ (`struct stat`'s layout, `O_DIRECTORY`, `poll` or only `ppoll`), pinned against the
kernel's headers and the C library. Beside them `CpuFeatures::baseline(isa)` holds the CPU's
baseline feature set, the extensions every machine of it has (see "One product on every target"
below). Linux on x86-64 and on AArch64 have both; the rest below is still to build per target. A
new profile must supply:

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
   through `fill` for every length, a file program, and the numeric examples' blocks that
   x86-64's settle by terms alone (the absolute difference, the mean of whole numbers, the day
   count, the CSV total's amounts). Its idioms needed four readings of the pool's own: a digit
   taken off with `udiv` and `msub`, `n - (n / 10) × 10` one division at a time, is the
   remainder, where x86-64's `div` gives it, however the pool wrote the quotient (one by 10 of 5
   times a sum is that sum halved); a product's overflow check, its high half against its low
   half's sign, `asr #63`, is the value's sign, which its range settles, as is a result's check,
   `tbnz x0, #63`, which reads `x0 < 0` as x86-64's `js` does; and `adds`, `subs` and `negs`
   overflow nowhere their operands' ranges keep the result in range, as x86-64's flags are set,
   so the `b.vs` after them is settled by the terms.
   Its floating point (`exec/aarch64/float.rs`) is the pool's float terms, which compute as SSE
   does, with AArch64's own rules around them where the two CPUs differ: a signaling NaN operand
   before a quiet one and a positive default NaN, `fmax` and `fmin` passing NaNs on and putting -0
   below +0, and conversions that saturate, a NaN giving 0. Where the operands' bounds rule those
   cases out the term is the pool's own, so the hypotenuse example's spec compiled for AArch64 is
   proven as x86-64's is, step by step the formula's doubles, with no solver question; the
   `against_a64` harness holds every float instruction to the CPU on NaNs of both kinds, zeros of
   both signs, subnormals and the ends of each conversion, and whole V registers, so a scalar
   write clears the rest. Its vectors (`exec/aarch64/vector.rs`, NEON's instructions) are
   lanes over the V registers' bytes: a comparison's lane is a choice of all ones or zero on its
   condition, so a program's tests of bytes are the formula's conditions, and `addv` of lanes
   that are each 0 or 1 is the pool's count, as x86-64's `popcnt` of a mask is. A float lane is
   its scalar instruction's term, so the hypotenuse with both sides in one vector of doubles is
   proven as the scalar one is; `fmaxv` reduces in the architecture's pairs. Its carry-less
multiply, `pmull` and `pmull2` of bytes or of doublewords, is x86-64's `pclmulqdq` product, one
model for both (`carry_less` in `exec.rs`). Its AES and SHA instructions (`exec/aarch64/crypto.rs`)
are the architecture's pseudocode built from x86-64's pieces of the same algorithms: `aese` is
x86-64's last round of the two XORed with no key, `aesmc` its MixColumns, and SHA's rounds and
message steps its words' functions, each held to QEMU's CPU and silicon by `against_a64`. Its SVE
(`exec/aarch64/sve.rs`, the integer subset
of bytes a streaming loop needs) keeps each Z register, the CPU's length of bytes up to 64, in the
vector registers' 64 bytes and each predicate as a condition for each byte; `whilelo`'s lanes are
the bytes left, a load or store reaches only its active lanes, held to the longest vector's
width as the rules hold it, and the flags are the architecture's predicate test. A NEON write
clears the bytes above its 16. A proof runs at the CPU's vector length (the kernel's, or under
QEMU the length a four-instruction program measures), holds for that length alone, and the
lock's needs say it (`sve16` to `sve64`), which `run` holds the CPU to. Its `against_a64` harness
holds every SVE instruction to the CPU with Z and P registers of its length, and
[the SVE wc](plugin/skills/compile/assets/linux-aarch64/fast/wc-sve/main.s) is proven for every
length with a 64 KiB buffer at 16-byte vectors and at 64, about 100 seconds each, with no solver
question, its loop leaving where its index reaches the bytes read as the NEON one's does. An access at a place
   the input chooses is held to the object its own CPU's base register holds (`Through`), so a
   hash table on AArch64 goes through its log of stores as x86-64's does. `ppoll`, AArch64's only
   wait, is `poll` with a time limit the kernel reads and then writes the time left into, which the
   proof takes as unknown. A thread starts as each CPU's convention says where `clone` keeps the
   stack and where the kernel clears the id (x86-64's fourth argument, AArch64's fifth), and what
   the call itself changes (x86-64's rcx and r11); `ldar` is a load to the proof, which takes
   memory as sequentially consistent, as M7's rule that no thread writes what another reads
   makes it. Its loops end
   with `subs` and `b.ne`, or `sub` and `cbnz`, which the proof finds as it finds x86-64's `dec`
   and `jnz`, so a loop over a buffer-full that leaves where its index reaches the bytes read has
   its exits merged into one run. Two rules of the pool's met NEON's mask of the bytes left, the
   low byte of `min(left, 16)` that `dup` spreads over a vector: a branch of a choice keeps the
   range its condition gives it, so `ite(x < k, x, k)` is at most k, and a comparison with the
   low bits of a value that fits in them is the comparison of the value itself. So
   [the fast wc's NEON form](plugin/skills/compile/assets/linux-aarch64/fast/wc/main.s), each
   16-byte step masked with `cmhi` of the bytes left against each lane's place, is proven for
   every length with a 64 KiB buffer, with no solver question, in about 105 seconds and 1.8 GB,
   as x86-64's SSE2 one is; and the vector template from its spec unchanged, every line with
   no solver question. CPU feature detection is per CPU too (x86's `is_x86_feature_detected!`).
4. **Dialect:** the Power of 10 in that CPU's mnemonics and registers, with its reference (today
   `references/x86_64.md` and the x86 parts of `references/power-of-10.md`). The rules
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
5. **The OS edge:** one file for each OS on each CPU (`src/edge/linux-x86_64.s`,
   `linux-aarch64.s`; macOS's and Windows's are to come), in that CPU's assembly, holding the 21
   operations every program calls (`os_read`, `os_write`, `os_open`, ... `os_exit`), which
   `src/edge/mod.rs` defines once: their arguments, results, errors, modes and layouts, the same
   on every OS (see "The edge" below). Each routine does what its OS does and turns the OS's
   answers into those. On Linux an operation is `mov eax, SYS_X`, `syscall` and `ret` with its
   normalization, and the ABI's details, which differ between CPUs even on one OS, are in these
   files and in `src/syscalls.rs`, which the sandbox's call lists and the prover's model read: on
   AArch64 there is no `poll`, only `ppoll`; `clone`'s child-tid argument moves; `struct stat` is
   128 bytes with `st_mode` at 16 (x86-64: 144 and 24); and `O_DIRECTORY` is `0o40000`, x86-64's
   value for `O_DIRECT`. macOS and Windows don't allow a program to call the OS directly: a
   program must go through the system library (libSystem, kernel32), code lengua never checked,
   so their edges call it, and lengua checks the edge's own text and nothing of what the library
   does. The CPU's own library (`src/edge/x86_64.s`, `aarch64.s`: `assert_fail`, `fill`,
   `parallel_for`) is the same on every OS.
6. **The runner and sandbox:** on Linux, seccomp and Landlock (`runner.rs`, `os/linux.rs`), and
   the broker that serves the names, the clock and the random bytes (`broker.rs`: one wire on
   every OS; what differs is how a descriptor or handle reaches the program and how a path is
   held beneath its folder). macOS and Windows have none of the first two; each needs its own
   confinement.
7. **The tests' column, and a runner to run it.** Every table of cases is a `PerCpu`
   (`src/targets.rs`), a struct with a field for each CPU, so a new CPU stops every one of them
   compiling until it has its cell, a system added to a CPU needs none, and a case can't be left
   out of one CPU unnoticed; what depends on the system too (a runner's label, a name a system
   reports) is a `PerTarget`, a field for each target. The cell is the CPU's own hardware's
   version of the case, or the reason, and only a reason of the hardware, that the case can't be
   written there:
   - `tests/red_team.rs`: for each attempt to get past the checks, the edit of the target's
     template and what a refusal must name; and `PLANTED` in `src/checker/tests.rs`: the same
     for the bugs planted against the memory rules (check-only, so every runner checks every
     target's cells);
   - `tests/every_program.rs`: where the target's twins of the examples and fixtures are, and
     which optional extensions of its CPU a program may need (a program for one that a computer
     lacks is skipped, with a note in the run's summary);
   - `tests/limits.rs`: an assembly source of the target's CPU for each program that pins a
     limit: time, output, the environment, open files, the spec's approval of the clock and the
     random bytes, the kernel's hold on the channel, threads, the network, and the rest of
     "Running programs". Every runner builds every target's source (LLVM builds for all of them),
     so a typo fails everywhere, and runs its own;
   - `tests/edge.rs`: the cases of the 21 operations as lists of steps, from which a program is
     written for each CPU, so both are held to the very same case, natively on each CPU's runner,
     through QEMU where it is set up and in the prover's model; it holds the names the broker
     refuses, the listing turned round, and the broker's clock and random bytes;
   - `tests/programs.rs`: for each case that takes the examples, fixtures, templates and fast
     programs through the server (files, folders, sleep, the environment, dice, programs built
     on programs, decimals, words and pairs counted for every length, the vector, streaming,
     tally and parallel templates, the benchmark's programs), the target's twin of the program
     (a folder, a template, a fast program), what the case changes in it (an edit of its text),
     and the optional extension of its CPU a variant needs (a variant for one that a computer
     lacks is skipped, with a note). The proofs, runs and counts are the case's; a cell pins
     what its CPU proves or does differently, where one does, with the reason and todo.md's
     record of it.

   A target must pass them natively, on a runner of its own CPU: `tests/parity_inventory.rs`
   fails unless `scripts/ci/targets.json` lists a runner for each target (`ubuntu-24.04` for
   x86-64 Linux, `ubuntu-24.04-arm` for ARM64 Linux) and `ci.yml`, the one workflow that runs the
   suite, runs `cargo test` on every runner the list names, and unless every test file made
   for one target (`#![cfg(target_arch = ...)]`, or a target in its name) is on its `DEBT` list
   with what the other targets lack, so a new such file fails and the list only shrinks. QEMU's
   user mode is for a developer's computer: it checks and runs, but the kernel sees QEMU's
   system calls, so it can't hold a program to its spec's.
   What the suite still tests on one target only is that `DEBT` list.

**One product on every target.** A port adds a CPU's or an OS's means, never a different lengua.
The same spec means the same on each: the same skills, tools, reports, lock and messages (only
the target's name and hardware facts differ); the same rules, memory safety, tests and proof
engine (only each CPU's instruction model is its own); and the same limits on every target. The
capabilities a spec can name (`- Uses:` standard input, files and folders, the clock (sleeping
too), random bytes, the environment and the terminal; the network and threads have
lines of their own, `- Network:` and `- Threads:`) and what the sandbox allows of each are one
definition. Every limit one target has, from its OS or its CPU,
is every target's limit, whatever their assembly or their OS's permissions: the user's rule,
made strict on 2026-11-02, so lengua is the same product regardless. A limit is anything a spec
or its user can observe: what a program may do, how it behaves, what it's refused and when, and
what lengua promises of it. No target is excused a limit because something else of its own,
lengua's checks included, could keep the same promise there: where one OS can't keep a promise
the way another does (its sandbox can't police something, say), every target takes on that OS's
limit, and either the capability is reshaped so every OS keeps it the same way, or that part is
cut on every target, Linux too. So one OS's extra power, or its sandbox's, never makes a spec
behave differently. What may differ is the hardware:
the instructions a CPU has, its vectors' width, its speed. Every program has a baseline form that
runs on every machine of its target, x86-64's SSE2 and ARM64's NEON, and an optional extension
(AVX-512, SVE) is only ever a named tier beside it (`wc-avx512`, `wc-sve`), picked from the
machine line and meeting the very same spec, tests and proofs, so what a spec means never
depends on an optional extension. A target is done when it covers what x86-64 covers, each item
with its own hardware's means (NEON where x86-64 has SSE2, SVE where it has AVX-512). The two
Linux targets hold each other to it program by program: every x86-64 example and fixture has an
AArch64 twin compiled from the very same spec, under `tests/fixtures/linux-aarch64`, and every
AArch64 one its x86-64 twin; a test checks the pairs and their specs, and another puts every
program of each target through the server, where each follows every rule and passes its tests
(`second` fails its tests on both CPUs, by design). The mutation test covers the same programs on
both CPUs, x86-64's under its own decoder and ARM64's under QEMU or on ARM64.

**The floor is a default container.** The same rule holds for where lengua runs. Every route is a
real Linux today (natively, WSL 2, a Lima VM), and a container image of the same package may come
later, so lengua takes nothing a default Docker or Podman container wouldn't give it on any host:
a limit of the container is a limit of lengua on every system, Linux included, and the image
would change nothing. lengua uses no io_uring, which the default seccomp profiles of both runtimes
block; the hardware counters are optional, since containers and VMs often lack them; the proofs'
memory guard is meant to read a container's own limit, not the host's `/proc/meminfo`; and a
Linux-only speed-up (the vDSO clock, `epoll`, `splice`) is allowed because it needs no privilege
a default container lacks. Landlock is the open case: it depends on the kernel (todo.md, "Round
3").

**Each target has a baseline, and every program runs on it.** The hardware may differ, but a
spec's behavior never depends on an optional CPU extension, so every program lengua ships has a
form that runs on every machine of its target. `CpuFeatures::baseline(isa)` (`src/platform.rs`)
says what that is, the owner's decision, which the owner may change: on x86-64 SSE2 with SSE3,
SSSE3, SSE4.1, SSE4.2 and POPCNT (Windows 11 24H2 requires SSE4.2 and POPCNT, and every GitHub
runner has them), and on ARM64 ARMv8.0-A with NEON and floating point and
nothing optional (no CRC-32, PMULL, AES, SHA, LSE atomics or SVE). An extension beyond it (BMI,
AVX2, AVX-512, SVE, the CPUs' crypto and CRC instructions) is a **fast tier**: a program of its
own, named for the extension (`wc-sve` beside `wc`), beside its baseline sibling and from the very
same `idea.md`, the same bytes on both CPUs since a spec names no CPU, so the same tests and
proofs hold for both. The agent starts from the baseline form and takes a tier only when the
machine line lists its extension, so a computer without AVX-512 has every program to learn from
and copy, and a port to macOS or Windows inherits no gap. `tests/baseline.rs` makes it a failing
test for every target in `Profile::ALL`: it rule-checks every shipped `main.s` (the examples, the
fixtures, the skill's templates and fast programs, and the benchmark's variants, listed apart)
against the target's baseline, and a program that needs more must be listed in `TIERS`, with its
baseline sibling written, passing and from the same spec, or in `DEBT`, with the sibling planned
(both CPUs' lists are empty today). The test fails for a program that needs more and isn't
listed, for a listing that names the wrong extensions, and for a listed program that now passes,
so the debt only shrinks. A new CPU brings its baseline (the `match` on `Isa` stops compiling
without it), a new system on a CPU that has one shares it, and the test then holds its programs
to it.

The network shows the rule at work. A program's own `connect` could be held to its spec's ports
on Linux (Landlock), but Windows's sandbox for apps can't hold a program to one port and reaches
this computer's own address only with an administrator's exemption. So since 2026-11-02 no
program connects by itself, on any target: lengua makes each connection its spec names and hands
it over, as it binds a server's port, and the program holds no network at all, which every
sandbox can enforce. `on any computer`, where the program chose its address, is gone on every
target; a spec names each computer instead. Every write to a closed pipe or connection is an
error (`EPIPE`) rather than a signal, as on Windows: the launcher keeps SIGPIPE ignored into the
program.

**Linux ARM64 was nearest:** the same OS, so the same kinds of system calls, the same sandbox and
ELF, but 2, 3 and 4 and a new triple were new, and 5 and 6 changed in detail (its own edge file,
with the call numbers and the ABI details above). macOS and Windows need 1, 5, 6 and 7 and, from
2, a reader for their executable format (Mach-O, PE), the OS edge and sandbox (5 and 6) most of
all; the decoders, prover models and dialects of their CPUs (2 to 4) are written once per CPU,
and x86-64's and ARM64's exist. QEMU's user mode, on the x86-64 Linux runner, tests Linux ARM64
for all but the sandbox's exact filter, which GitHub's ARM64 runner tests natively.

The plugin's own server binary ships for x86_64 and aarch64 Linux, built on glibc 2.39 today; a
static musl build, or one for older glibc, is planned (todo.md, "Round 3"). That is the server
running, separate from which targets it can compile and check programs for.

Next, in the order of todo.md and ROADMAP.md: reach (a Linux next to the agent: Ubuntu 24.04
natively, in WSL 2 and in a Lima VM, one package for it), the performance finish line, then
prover parity, the documents and the release.

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

main.s's second line, `# lengua: compiled from idea.md sha256:… for linux-x86_64; needs AVX2`,
ties the code to the version of the spec it was compiled from. It names the code; it doesn't
protect it: main.s is plain text anyone can build outside lengua (`lengua-mcp link main.s` prints
it with lengua's own file after it, which is what lengua assembles), past its checks and the
kernel's limits, so lengua's guarantees hold only for a program that `run` runs after `check`.

## Programs built from programs

Programs stack. A spec links to the programs it builds on, `- Builds on: [sort lines](../sort/idea.md)`,
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
  an agent made up, so it shows instead of gating: the build's first request and its latest
  ones, at most six in all, stay in the lock until its code passes, every change to the spec
  after the build's first request is shown as a diff, and lengua itself marks test changes that
  test less. lengua relies on nothing from the harness, such as its permission prompts, so a
  harness in an auto mode is as safe: what a program may touch is what the spec names, which the
  kernel enforces on every run. The person can ask to see the spec before the build.
- Rule 9 checks the shape of each bounds check (`cmp` and an unsigned `jae` before the access)
  and, for the program's own data symbols, that the limit fits the buffer. The memory rules (M1
  to M8) give every pointer an owner and hold constant offsets inside it, and compare-chain
  detection is pattern-based. What no rule settles, such as a pointer loaded from memory or an
  index checked against a length in a register that the code just before doesn't set to a
  constant, the proof settles on every run it covers, and the done reply's `memory` line says
  which proofs held their runs to the rest. So passing the rules doesn't make a program
  memory-safe by itself: where a proof doesn't finish, the tests guard the rest.
- Tests sample inputs, so where a proof doesn't finish or doesn't model the program's
  instructions, a program is checked, not proven. The same AI writes the formulas and the code:
  each formula must reproduce its spec example, which narrows that gap, but a wrong formula can
  still match an example by coincidence. "Proven" means proven against the spec as it ends, so
  the report states the spec's limits.
- The kernel holds a run to its spec's system calls, to the files its broker opens inside its
  working folder, and to the network its spec names, whatever the checker missed. What it does
  not hold, for a program that got past the checker and has no spec to say otherwise (so a limit
  of the sandbox, not a promise about the program):
  - how many threads it starts: `clone` is held to the flags of lengua's `os_thread_start`, but
    not counted, so a hostile program can start threads until its time limit or the system's
    limit;
  - how much memory and disk it uses: its memory is the `.bss` and stack the checker bounds from
    the spec, which a hand-made executable need not obey, and files are capped one by one (64 MiB
    in tests, not in `run`), not in total;
  - whether it outlives a server that is killed: the time limit is the server's, with no
    parent-death signal behind it;
  - what QEMU's user mode runs (a developer's path for the other CPU's programs): the kernel
    sees QEMU's own calls, so only Landlock's file and TCP rules and the limits confine it, and
    sockets, UDP and local sockets stay open;
  - how many descriptors it keeps in flight over its channel, which can fail another program's
    reply on a computer whose soft limit of descriptors is low.

  The broker differs from what Windows would do in two small ways, kept for the ports: a rename
  that replaces a file the program holds open succeeds (`EBUSY` is for the source), and a name
  with `..` in it is collapsed in the name's text before the system sees it.

## Status

The MVP is built: milestones 1 foundation, 2 spec and lock, 3 tests, 4 rule checker, 5 code and
run, and 6 end to end, each committed after the project owner approved it. A program is two
files: the tests are in the spec, whose steps are its examples. `tests/workflow.rs` runs the whole
workflow on `examples/add` and its ARM64 twin in both MCP protocol generations, and
`tests/sessions.rs` the server's flow around a build, each case once with a cell for every
target, on each target's own runner.

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
list prices; twelve phrasings of a request all chose write-idea first. A word-frequency count
followed the same way: from one request with no question, a 1,034-line hash-table program with
every test passing, in about eight minutes for about $2. BENCHMARKS.md, measured
again on 2026-11-02, has lengua's best ahead of or level with C, C++ and Rust on every workload in
cycles, CPU time and wall time, with 2.5 to 3.3 times less memory.

Interactive round 5 (2026-11-02) ran a suite of twelve requests on each Linux target, from a
two-number add to an HTTPS fetch, a multi-client server, threads and the terminal, each typed into
a real Claude Code session with the plugin (SESSIONS.md): on x86-64 all twelve passed from one
request each, and on ARM64, compiled for AArch64 and checked under QEMU, all twelve passed too.

Tested, not proven, for now: sorting and `uniq` past a few bytes and word tallies past a few
bytes, which need quantified facts over memory; byte loops at 64 KiB, whose counts keep a copy
of every part at every byte; a routine for the last partial vector at a place the length chooses,
and raw `read`s of any size; floats computed in another order than the formula's, a folder's
listing, and SHA-256 and AES, in plain integer code and with the instructions. Each proof line says which.
Left out of the dialect on purpose: gathers and scatters, whose addresses can't be bounds-checked,
allocation while running, recursion, function pointers, and running other programs.

lengua has two targets, as one product: Linux on x86-64 and Linux on ARM64, each tested natively
on its own GitHub runner (`.github/workflows/ci.yml`; nothing runs on the owner's computer),
every x86-64 program with its AArch64 twin. Windows reaches it inside WSL 2 and a Mac inside a
Lima VM, both on the Ubuntu 24.04 the runners test (README.md, "Windows and Mac"); their kernels
are not yet tested, and each is promised only once a runner has run lengua's checks inside it
(todo.md, "Round 3").
A native macOS or Windows port, deferred, would need more than a new CPU, since those systems
forbid the direct system calls lengua's edge makes on Linux and have neither seccomp nor
Landlock: each would be a profile with its own toolchain, executable reader, edge file, runner and
sandbox, on the decoder, prover model and dialect reference its CPU already has, as "Porting a
target" sets out, and each would have to hold every limit the Linux targets do. The white paper
(WHITEPAPER.md) says what lengua
is and isn't, and a site where people share specs (`site/`) runs locally: the spec travels, each
computer compiles it for itself, and code is checked again wherever it runs. todo.md holds only
what's left for people.
