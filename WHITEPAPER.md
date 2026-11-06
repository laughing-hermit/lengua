# lengua: an AI language, not a programming language

*A white paper, November 2026. lengua is a work in progress.*

## Summary

A person describes a program in their own words. Their AI agent writes it down as a spec: what the
program does, as behaviors, each shown by an example and tried by tests. The agent then compiles
that spec into assembly for the person's own CPU and operating system, and lengua checks the
result. It runs every test with fresh inputs, holds the code to its own rules for safe assembly,
proves it right for every input where the code's shape allows, and has the kernel hold every run
to what the spec allows. Nobody writes code, and nobody has to trust the AI that did. The spec is
what people keep and share; the code is disposable, compiled afresh and checked again on each
computer.

lengua is a plugin for AI agent harnesses (two skills and one tool server, built only on open
standards), working today on Linux on x86-64 and ARM64. This paper explains the idea, why it isn't
a programming language, and how it makes an untrusted compiler trustworthy.

## What lengua is

A lengua program is two files.

- **spec.md** is the source. It holds every behavior of the program and every test of it, in plain
  markdown that any person or AI can read. It names no CPU and no operating system, so it can be
  shared as it is.
- **main.s** is the assembly the agent compiles from the spec for one computer. It's plain text,
  and it's disposable.

A third file, **lengua.lock**, is signed by lengua and ties the two together: it records which
spec the code was checked against and what was proven, and a program runs only while both files
still match it.

The person's part is the request and the spec. The agent's part is everything after: it writes
the spec from the request, compiles the spec to assembly, and asks lengua to check it. lengua's
part is to judge the agent's work. Its reply says what passed, what's proven and how far, and
anything that failed, with the line to fix. The one step no machine can check, from the request to
the spec, comes back to the person in the agent's report, in the spec's own words.

## Why it was created

Programming languages and their compilers were made so people could tell machines what to do. A
person learned a language a machine could translate, and a compiler, trusted after decades of
use, did the translating.

AI agents change the first half. People can now say what they want in their own words, and an
agent can write the machine's code itself, in any language or in none. What's missing is the
second half: trust. An agent's code can be wrong in ways that look right, and reading it closely
enough to be sure costs more than writing it did. Today's answer is to have the agent write in a
programming language a person can review, and to review it. That keeps the middleman and adds a
reader who can't keep up.

lengua is the other answer: keep the person's words as the source, let the agent write the
machine code, and check that code so thoroughly that nobody has to read it. That's a new standard
for programs written in natural language: a source format anyone can read, and checks anyone can
run.

## Why it isn't a programming language

- **Nobody writes code in lengua.** There's no syntax to learn and no grammar for statements or
  types. A person never sees a line of lengua code, because there isn't one.
- **The spec is the source, and it says what a program does, not how.** "Prints the sum of its two
  arguments," with an example and a table of where the tests' numbers come from. No variables,
  loops or memory: those are the agent's concern, and lengua's to check.
- **Programming languages were middlemen between people and machines.** They exist because
  machines couldn't understand people and people couldn't write machine code at scale. With AI,
  the middleman can go: the person talks to the agent, and the agent writes for the machine.

## What the compiler is, and what it isn't

- **The compiler is the AI agent harness**, such as Claude Code: a pseudo compiler that turns the
  spec into assembly. Unlike a classical compiler, it's neither deterministic nor trusted, and it
  doesn't need to be.
- **LLVM's assembler and linker** only turn that assembly into bytes, as they do for every
  compiler that emits assembly.
- **lengua itself is not the compiler.** It's the checker that makes an untrusted compiler
  trustworthy. Compiler research calls this *translation validation*: rather than proving the
  compiler right for every program it could ever compile, check each translation it actually made.
  Each step is validated: the spec against its own tests, the assembly against the rules, the
  machine code against the spec's formulas, and each run against what the spec allows.

## How lengua checks the AI's work

The same AI writes the code and the tests, so the judgment of both stays in lengua, not in the
agent.

- **Every test, with fresh inputs on every check.** Each behavior's example is a test, and its
  generated tests draw new inputs every time from the table the spec gives: boundary values first,
  then seeded random ones. Where a row's ranges hold at most 5,000 combinations, every one runs, so
  for those inputs the program is checked, not sampled. Expected results come from formulas in a
  small language with no loops, so every formula finishes, and a dry run of every formula catches
  overflow and division by zero before any code exists. Output with placeholders must vary, so a
  program that prints a constant can't pass. lengua keeps a copy of every spec it's seen, so when a
  spec's tests change, the reply marks every change that tests less.
- **Its own rules.** NASA's "Power of 10" rules for safety-critical code, adapted to AI-written
  assembly: every loop has a fixed bound, there's no recursion and no allocation while running,
  functions check what they're given with assertions, at least two a function on average, every
  result is checked, and more. On top of
  them come memory rules M1 to M8, a borrow checker for assembly. Every pointer has an owner, and
  every access stays inside it, checked by the rules where they can follow a pointer and by the
  proof everywhere else.
- **Proofs that the program is right for every input.** lengua executes the actual machine code
  symbolically, every path through it, and compares what it does with what the spec's formulas
  say, for every input in the spec's ranges. Most steps settle from the terms themselves. Where
  one needs a solver, the question goes to two independent ones, z3 and cvc5, and both must agree.
  How far a proof reaches depends on the code's shape, not only on what it computes, and every
  proof line says exactly what it covers:
  - Whole numbers, decimals, choices, checksums, files, the network, servers that poll in every
    order their clients' steps can come in, threads, and floats computed in the formula's order are
    proven for every input in range. `add` is proven for every pair of its numbers.
  - Texts are proven as far as the spec's lengths and the proof's time reach, and further on
    request.
  - A filter or count that streams its input a buffer-full at a time is proven for every length,
    by induction over its read loop. The fast word count, a 64 KiB buffer and one vector loop, is
    proven for every length on both CPUs. Programs that read with lengua's `fill` are proven
    however their input arrives, cut into any pieces.
  - Sorting and word tallies are proven for short texts and tested beyond. A folder's listing
    order is the kernel's, floats computed in another order than the formula's really differ, and
    the `sha256` formula is beyond the solvers: those stay "tested, not proven", and their proof
    lines say so.

  So "the fast `wc` is proven" doesn't mean every streaming program is. lengua says which
  programs, never that all are.
- **The kernel holds every run to the system calls, files and network its spec names.** A spec
  names what its program uses: standard input, files and folders, the network, threads, the
  terminal, the clock, random bytes. Every run, in tests and for the person, is sandboxed to
  that, by seccomp and Landlock and a broker of lengua's own that opens the program's files for
  it, since the program may touch no path itself. A program that names no network has none. A
  program that connects doesn't connect at all: lengua makes each connection its spec names, does
  TLS itself for HTTPS, and hands the program a plain stream, so the program holds no network and
  no crypto. What the sandbox doesn't hold is how many threads a program starts and how much
  memory and disk it uses in all, and, when QEMU runs the other CPU's program, its sockets;
  DESIGN.md's threat model lists these limits.

The checks are fast enough to sit inside the agent's loop: every call to lengua's tools comes
back in under about 40 seconds, with proofs that need longer continued in the background.

## Why assembly

Assembly is the fastest code for each machine, with nothing between the spec and the CPU. There's
no runtime, no garbage collector and no C library: a lengua program is a few kilobytes, static,
and starts in microseconds. The checks above work on the machine code itself, so nothing between
them and the CPU has to be trusted either.

The agent writes for the person's own CPU and uses what it has: SSE2 on every x86-64 computer
and AVX-512 where the CPU has it, NEON on every ARM64 one and SVE where the CPU has it. Every
program has a form that runs on all of its target's computers, and an extension is only a faster
form beside it that meets the same spec, tests and proofs. On lengua's benchmarks against C, C++
and Rust compiled for speed on the same computer (BENCHMARKS.md for x86-64,
BENCHMARKS-aarch64.md for ARM64), lengua's best is ahead of or level with them on every x86-64
workload in cycles, CPU time and wall time, with 2.5 to 3.3 times less memory.

## Why the spec is the mother

- **The spec is the source of truth.** People share specs, never code. A spec holds everything a
  program must do, in words and tests anyone can read and change.
- **Code is disposable.** Each computer compiles the spec for itself, and lengua checks it there.
  The same spec compiles to x86-64 on one computer and ARM64 on another, and each copy is checked
  on its own hardware.
- **Every copy passes the same checks, never "the same program".** Each copy is compiled afresh,
  so two computers get two different programs that pass the same checks: the spec's tests,
  lengua's rules and the proofs. They can differ wherever the spec leaves room: inputs beyond its
  ranges, cases it doesn't mention, speed, even what's proven. So whatever must match belongs in
  the spec's behaviors and ranges.

Programs build on programs: a spec links to the specs of its parts, so a folder of programs is a
graph of two-file nodes, and lengua holds each program to its parts' current code.

## How it differs

- **From programming languages.** In a programming language, a person writes how; the compiler
  is trusted, and what the program does is whatever the code says. In lengua, the person states
  what; the compiler isn't trusted, and what the program must do is the spec, checked.
- **From code generators that emit C or Rust.** A generator's output is code to keep, read and
  maintain, and it still depends on a trusted compiler and its own correctness. lengua keeps the
  spec and throws the code away, and it checks the final machine code itself, below every compiler.
- **From ordinary AI coding assistants.** An assistant writes code in a programming language for
  a person to review, and the review is the safety. lengua replaces review with checks the person
  doesn't have to perform: tests, rules, proofs and a sandbox. The person reviews only the spec,
  in their own words.

## The format

spec.md is an Open Knowledge Format document, after Andrej Karpathy's LLM Wiki idea: plain
markdown that any person or AI can read, made of headings, lists and tables, with no other
language inside. A spec has a purpose, behaviors and constraints.

- Each behavior has Given/When/Then steps: that's its example.
- Its generated tests are the same steps with placeholders, over a table of where their variables
  come from, such as `int(0, 253402300799)` or `string(0, 5000, "abehlo \n")`, and the expected
  results are formulas over those variables.
- The constraints name what the program uses, the capabilities its sandbox gives it.

As in an LLM Wiki, the person's request is the raw source, the spec is the page the AI keeps, and
lengua's checks are the lint that keeps the page and the code true to each other.

## What it promises, and what it doesn't

lengua's guarantees hold through lengua: main.s is plain text anyone can assemble and run outside
it, and then none of the checks above stand behind it. Within lengua:

- every copy compiled from a spec passes the same checks, on its own computer;
- each proof covers what its line says, for every input in range or up to a length it names;
- every run is held to what its spec allows, and the limits are the same on every target: one
  operating system's extra freedom never makes a spec behave differently on it.

## Where it stands

lengua works today on Linux on x86-64 and on ARM64, as one product: the same skills, tools,
rules, tests and proof engine on both, with each CPU's own instruction model. Every example program
has a twin on the other CPU, compiled from the very same spec, and both pass the same checks. In
real interactive sessions of Claude Code with the plugin, programs come from one plain request:
in the suites of 2026-11-02, all twelve requests on each Linux target, from a wc to an HTTPS
fetch to a multi-client server, gave a program that passed every check (SESSIONS.md). Windows reaches
lengua through WSL 2 or Docker and a Mac through a Linux VM or Docker, which run these same two
targets; native macOS and Windows ports wait, and would be held to every limit the Linux targets have.

lengua is an Agent Plugin: two skills and one MCP server, using only open standards, so it works
in Claude Code and other agent harnesses alike.
