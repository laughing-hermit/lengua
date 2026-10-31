# lengua's agenda

In order. Each item starts when the one before it is done.

## 1. Perfect lengua for this target (x86-64 Linux)

Marked done on 2026-10-31, then reopened the same day. Round 7 is under way: word counts for
every length, hash tables and word tallies, the fastest word count proven for every length, and
checked jump tables.

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
  every length, by induction over their read loop. Servers that poll, in every order their
  clients' steps can come in. Sorting for short texts.
- **Speed.** Ahead of or level with C, C++ and Rust on every benchmark, with 2.5 to 3.1 times less
  memory (BENCHMARKS.md).
- **A release built locally** that installs with no Rust toolchain, and `lengua-mcp run` for the
  user's own shell.

Tested, not proven, and each proof line says why: sorting past a few bytes, word tallies,
floats computed in another order than the formula's, a folder's listing, the `sha256` formula,
and loops whose state crosses from one buffer-full to the next, as a word count's does.

Left for later on this target:
- Sorting at every length, which needs quantified facts over memory.
- SHA-256 and AES programs: left out after two safety stops, and not routed around them by any
  means.

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

macOS on Apple silicon and Intel, Linux on ARM64, and Windows on x86-64 and ARM64. Each is one profile compiled from the same specs.

## 4. A site for sharing specs

A simple Next.js landing page where people share their specs, never code, and anyone compiles them for their own computer. It promises only what lengua enforces:
- the spec travels, and every copy compiled from it passes the same checks; it never promises the same program;
- each computer compiles and checks its own code;
- main.s is plain text anyone can build outside lengua, so the guarantees hold only through lengua's own `run`.
