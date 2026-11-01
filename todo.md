# lengua's agenda

In order. Each item starts when the one before it is done.

## 1. Perfect lengua for this target (x86-64 Linux)

Marked done on 2026-10-31, then reopened the same day. Round 7: word counts proven for every
length, hash tables with word tallies, and checked jump tables (dialect 4) are done; the fastest
word count proven for every length is deferred (below). Round 8 told the portability story straight
and wrote the porting contract (DESIGN's "Porting a target"). The direction, decided 2026-10-31:
**native per target** — the agent writes each target's assembly and lengua proves the actual
machine code, the novel path and the only one that can beat C, C++ and Rust — with the spec as the
agnostic product that travels to every target.

What lengua is on this target now:
- **Two files.** spec.md, an OKF concept document, holds every behavior and every test, and names
  no CPU or OS, so it can be shared as it is; main.s is the code the agent compiles from it for
  this computer. Programs build on programs: a spec links to the specs of its parts, so a folder
  of programs is a graph of two-file nodes.
- **The agent's experience.** Two skills (write-spec, compile) and two tools (check, run), every
  call under about 40 seconds. In ten scenarios in real Claude Code sessions every program came
  from one request, with no question.
- **Memory safety.** The rules M1 to M8, a borrow checker for assembly: every pointer has an
  owner and every access stays inside it, checked by the rules where they can follow a pointer
  and by the proof everywhere else.
- **The proofs.** For every input in range, each solver step confirmed by z3 and cvc5: whole
  numbers, decimals, choices, texts and text formulas, checksums, floats computed in the
  formula's order, files and folders, the network, threads. Texts as far as the spec's lengths
  and the proof's time reach, and further with `prove_further`. Streaming filters and counts for
  every length, by induction over their read loop, word counts too (two buffer-fulls a step, so
  the proof never needs to know how the program carries a word across them). Servers that poll,
  in every order their clients' steps can come in. Hash tables, through a write log of their
  stores, and word tallies for short texts. Sorting for short texts. `switch`es as checked jump
  tables, through every case their index can reach.
- **Speed.** Ahead of or level with C, C++ and Rust on every benchmark, with 2.5 to 3.1 times less
  memory (BENCHMARKS.md).
- **A release built locally** that installs with no Rust toolchain, and `lengua-mcp run` for the
  user's own shell.

Tested, not proven, and each proof line says why: sorting past a few bytes, word tallies past a
few bytes, floats computed in another order than the formula's, a folder's listing, the `sha256`
formula, counts of a part that can overlap itself (such as `"aa"`), and the benchmark's fastest
word count, which reads a megabyte at a time and is proven by length.

Left for later on this target:
- mean's decimal means (its B1 and B3, two prices up to a million) aren't proven in the proof's
  five minutes: z3 runs out of time on one path. Found by Round 9's G4 at R1 (2026-10-31): the
  ignored `proves_the_mean_of_two_prices` and check_code's `the_decimal_example_is_proven` fail,
  each run alone. They're proven for prices up to 9.99, as the prover golden proves them.
- Sorting at every length, which needs quantified facts over memory.
- The fastest streaming programs proven for every length. The induction's base proves each final
  length one by one, so its cost grows with the square of the buffer; a base that covers every
  final length in one shape would let a 64 KiB buffer, C's size, be proven. The design review
  recommended this as its own round (about 1,000 to 1,500 lines).

Not done because a safety filter stopped the work (Claude's own, in Claude Code, 2026-10-31). What
a stopped reply held is never produced again — not reworded, and not by routing it through other
models or subagents (the user asked to use Sonnet 5.5 / Haiku 5.5 agents for it; declined, because
switching models to get around a safety stop is circumvention, whoever runs the subagent). Anything
a safety filter stops from here on goes on this list, with what it was working toward:
- **SHA-256 and AES programs.** Replies working toward them were stopped, so there is no SHA-256 or
  AES program among lengua's own examples and assets, and no proof (by lengua) that such a program
  meets the `sha256` formula. **The crypto support itself is not blocked:** the SHA and AES CPU
  instructions are modeled in the prover and probed against this CPU, and the test language has
  `sha256`, `crc32` and `crc32c`. So a user can write or drop in a standard SHA-256/AES
  implementation and lengua will check and test it, and a proof against `sha256` can be attempted
  on that supplied program. Only lengua hand-authoring the bundled example is blocked.

## 2. A white paper: lengua is an AI language, not a programming language

So people understand the idea, and so it can stand as a new standard. It explains:

- **What lengua is.** A person describes a program in natural language, a behavior-driven spec where each behavior is shown by an example and checked by tests. The AI agent compiles that spec into machine code (assembly) for the person's own CPU and OS.
- **Why it was created.** Programming languages and their compilers were made so people could tell machines what to do. With AI agents, people can say it in their own words, and the agent can write the machine's code itself. What was missing is a way to trust that code: lengua is that way, and a new standard for programs written in natural language.
- **Why it isn't a programming language.**
  - Nobody writes code in lengua.
  - The spec is the source, and it says what a program does, not how.
  - Programming languages and their compilers were middlemen between people and machines. With AI, the middleman can go.
- **What the compiler is, and what it isn't.**
  - The compiler is the AI agent harness, such as Claude Code: a pseudo compiler that turns the spec into assembly.
  - LLVM's assembler and linker only turn that assembly into bytes.
  - lengua itself is not the compiler. It's the checker that makes an untrusted compiler trustworthy: translation validation.
- **How lengua checks the AI's work.**
  - Every test, with fresh random inputs.
  - Its own rules: NASA's Power of 10 for assembly, plus memory safety and its borrow checker.
  - Proofs that the program is right for every input, confirmed by two independent solvers.
  - The kernel holding every run to what its spec allows.
- **Why assembly.** The fastest code for each machine, with nothing between the spec and the CPU.
- **Why the spec is the mother.**
  - The spec is the source of truth, and people share specs, never code.
  - Code is disposable: each computer compiles the spec for itself, and lengua checks it there.
  - Each copy is compiled afresh, so two computers get two different programs that pass the same checks: the spec's tests, lengua's rules and the proofs. They can differ wherever the spec leaves room, such as inputs beyond its ranges, cases it doesn't mention, speed, and even what's proven. So the promise is "every copy passes the same checks", never "the same program", and whatever must match belongs in the spec's behaviors and ranges.
- **How it differs** from programming languages, from code generators that emit C or Rust, and from ordinary AI coding assistants.
- **The format.** spec.md is an Open Knowledge Format document, Karpathy's LLM Wiki idea, so specs are plain markdown any person or AI can read: headings, lists and tables, with no other language inside. Each behavior's Given/When/Then steps are its example, and its generated tests are the same steps with placeholders, over a table of where their variables come from. As in an LLM Wiki, the person's request is the raw source, the spec is the page the AI keeps, and lengua's checks are the lint that keeps the page and the code true to each other.

## 3. The other five targets

macOS on Apple silicon and Intel, Linux on ARM64, and Windows on x86-64 and ARM64.

The spec already travels to all of them: it names no CPU or OS, and `check` enforces that, so the
source people share is agnostic today. What is per-target is the compiled `main.s` and lengua's
checking of it. LLVM's assembler and linkers cover every target, but LLVM does not check, prove or
sandbox — so each target is a profile **and** its own instruction decoder, prover model, dialect
reference and runner. The porting contract in DESIGN's "Porting a target" lists exactly what each
one needs.

- **Linux ARM64 is nearest, and under way (Round 9):** the same OS and sandbox (seccomp and
  Landlock) and the same kinds of direct system calls, but a new decoder, prover model, feature
  detection and dialect reference, and an OS edge that differs in detail: other call numbers, no
  `poll` (only `ppoll`), another `struct stat` and `O_DIRECTORY`, other supervisor registers. It's
  tested under QEMU on this computer and on GitHub's ARM64 runners for the sandbox.

  Round 9 covers the template's kind of program: arguments, whole numbers, text, standard input
  through `fill`, and files and folders, checked, tested and proven. Until the rounds after it
  land, lengua doesn't "run on ARM64", and nothing should say so:
  - an install package for ARM64 Linux computers (today's release and installer are x86-64 only,
    and the ARM64 server is built only on CI);
  - vector instructions (NEON, then SVE), lengua's main speed tool, with ARM64 benchmarks against
    C, C++ and Rust;
  - floating point; threads; networking and servers; the terminal; the crypto and checksum
    instructions.
- **macOS and Windows need more than a new CPU.** They don't allow a program to call the OS
  directly, as lengua's programs do; a program must go through the system library (libSystem,
  ntdll), which lengua never checked, and there is no seccomp or Landlock. So those four targets
  need a different model of the program's edge with the OS and a different sandbox.

The plugin itself (skills plus one MCP server) is agnostic in its skills, which are markdown and
install the same on every host. Its server is a native binary, built per host, and today builds
and runs only on Linux, since its sandbox and tracer are Linux-only. So installing the plugin on,
say, a Windows x86-64 machine and having it work is the Windows host's server build **and** the
Windows target profile — the same work as item 3 for that host. See DESIGN's "Porting a target".

## 4. A site for sharing specs

A simple Next.js landing page where people share their specs, never code, and anyone compiles them for their own computer. It promises only what lengua enforces:
- the spec travels, and every copy compiled from it passes the same checks; it never promises the same program;
- each computer compiles and checks its own code;
- main.s is plain text anyone can build outside lengua, so the guarantees hold only through lengua's own `run`.
