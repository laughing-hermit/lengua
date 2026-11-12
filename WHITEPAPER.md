# lengua: an AI language, not a programming language

*A white paper, November 2026. lengua is a work in progress. Each claim about it names its
source, in brackets: a file of the repository as it is at this writing, a test, or a run on
GitHub's runners, listed under "Sources" at the end.*

## Summary

A person describes a program in their own words. Their AI agent writes it down as an idea: what
the program does, as behaviors, each shown by an example and tried by generated tests, in plain
markdown [1]. The agent then compiles the idea into WebAssembly text, and lengua checks the
result. It holds the code to rules of its own for safe WebAssembly [2], runs every test, the
generated ones with fresh inputs on every check [3], proves the tests for every input where the
code's shape allows [4], and runs the program in a host that gives it only what the idea grants
[5]. Nobody writes code, and nobody has to trust the AI that did. The idea is what people keep
and share; the code is the agent's, and lengua checks it before it runs.

lengua is a plugin for AI agent harnesses: two skills and one tool server, built only on open
standards [6]. The same WebAssembly runs, and gets the same checks, on Linux on x86-64 and on
ARM64, macOS on ARM64, and Windows on x86-64 and on ARM64 [7]. This paper explains the idea, why
it isn't a programming language, and how it makes an untrusted compiler trustworthy.

## What lengua is

A lengua program is a folder of two files [8].

- **idea.md** is the source. It holds every behavior of the program and every test of it, in
  plain markdown that any person or AI can read. It is an Open Knowledge Format page of type
  Attested Computation, written in Simplified Technical English, and it names no CPU and no
  operating system, so it can be shared as it is [1][9].
- **main.wat** is the WebAssembly text the agent compiles from the idea: one module for every
  computer [7][10].

lengua keeps two records of its own. **lengua.lock**, signed with a key in the user's own data
folder, records the user's words, the idea and the code lengua checked, and what was proven; a
program runs only while both files still match it [11]. And when lengua takes a program to done,
it adds an entry to the idea's `verified` list with the hashes of the idea and the code it
checked, which anyone can check again, on any computer, with no lock and no key [12].

The person's part is the request and the idea. The agent's part is everything after: it writes
the idea from the request, compiles the idea to WebAssembly, and asks lengua to check it [13].
lengua's part is to judge the agent's work. Its reply says what passed, what's proven and how
far, and anything that failed, with the line to fix [14]. The one step no machine can check,
from the request to the idea, comes back to the person in the agent's report: what the idea
promises, and every change made to it after the request [15].

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
code, and check that code with tests, rules and proofs, so nobody has to read it to trust what
those checks cover. That gives programs written in natural language a source format anyone can
read, and checks anyone can run.

## Why it isn't a programming language

- **Nobody writes code in lengua.** There's no syntax to learn and no grammar for statements or
  types. The code the agent writes is WebAssembly, a standard's, not lengua's, and the person
  never has to read it.
- **The idea is the source, and it says what a program does, not how.** "Prints the sum of its
  two arguments," with an example and a table of where the tests' numbers come from [16]. No
  variables, loops or memory: those are the agent's concern, and lengua's to check.
- **Programming languages were middlemen between people and machines.** They exist because
  machines couldn't understand people and people couldn't write machine code at scale. With AI,
  the middleman can go: the person talks to the agent, and the agent writes for the machine.

## What the compiler is, and what it isn't

- **The compiler is the AI agent harness**, such as Claude Code: a pseudo compiler that turns
  the idea into WebAssembly. Unlike a classical compiler, it's neither deterministic nor
  trusted, and it doesn't need to be.
- **Standard tools turn that text into a module**: `wast` reads it and `wasmparser` validates it
  with the engine's own features, as an assembler does for a compiler that emits assembly [10].
  Wasmtime's Cranelift then compiles the module into the machine's code [17].
- **lengua itself is not the compiler.** It's the checker that makes an untrusted compiler
  trustworthy. Compiler research calls this *translation validation*: rather than proving the
  compiler right for every program it could ever compile, check each translation it actually
  made. Each step is validated: the idea against its own tests, the WebAssembly against the
  rules, a model of the module against the idea's formulas, and each run against what the idea
  allows.

## How lengua checks the AI's work

The same AI writes the code and the tests, so the judgment of both stays in lengua, not in the
agent.

- **Every test, with fresh inputs on every check.** Each behavior's example is a test, and its
  generated tests draw their inputs from the table the idea gives: boundary values first, then
  random ones from a seed drawn afresh on every check [3]. Where a row has no `choice(…)` and its
  ranges hold at most 5,000 combinations, every one runs, while the whole idea stays within
  20,000 runs [18]. Expected results come from formulas in a small language with no loops, over
  whole numbers checked at 128 bits, so an overflow is an error, not a wrong answer [19]. The
  formulas must reproduce each behavior's example, and a dry run of every formula catches one
  that overflows, divides by zero or prints the same thing for every input, before any code
  exists [20]. lengua keeps a copy of every idea it records, so when an idea's tests change, the
  reply marks every change that tests less [21].
- **Its own rules.** NASA's "Power of 10" rules for safety-critical code, adapted to AI-written
  WebAssembly: simple control flow with no recursion, a fixed bound on every loop, resources
  fixed up front, functions of at most 180 operators, at least two assertions a function on
  average, every result checked, named constants, no indirection, and more [2][22]. On top of
  them come memory rules M1 to M8, a borrow checker for WebAssembly: every address has an owner,
  a region of memory, and every access stays inside it. The rules check that where they can
  follow an address. Everywhere else it is left to the proofs, which keep the access inside the
  program's memory but follow no region, so whether it stays inside its own region is tested,
  not proven, and the reply's memory line says so [23]. The checker refuses any import the idea
  doesn't allow [24], and lengua refuses anything whose results differ between computers:
  relaxed SIMD, atomics, a NaN's bits read as a number [25].
- **Proofs that the program is right for every input.** lengua runs a model of the module
  symbolically, every path through it, and compares what it does with what the idea's formulas
  say, for every input in the idea's ranges [4]. Before a proof, the model must agree with the
  real program on samples of the test's own inputs, and inputs a proof finds are run for real
  before lengua says the code is wrong [26]. A step the terms or a known input settle needs no
  solver. Where one needs a solver, the question goes to z3, and with both installed, its "no
  counterexample" counts only when cvc5 gives it too; questions about floats that the bits don't
  settle go to cvc5 alone, and the proof's line says so [27]. How far a proof reaches depends on
  the code's shape, not only on what it computes, and every proof line says exactly what it
  covers [28]:
  - The suite proves its greet and echo programs for every input, its add for numbers of up to
    three digits, the hypotenuse in doubles, and the behaviors of the compile skill's vector
    template [29].
  - A text is proven one length after another, as far as the idea's lengths and the proof's
    time reach, and further each time a `check` asks to prove on [30].
  - A filter or a count that reads standard input a buffer-full at a time through lengua's
    `$fill` is proven for every length, by induction over its read loop, when it writes each
    buffer-full's bytes mapped one by one or keeps counts that add up [31]. The suite proves a
    streaming copy and a line count in SIMD128 this way. The word count in SIMD128 carries the
    last byte of one buffer-full to the next and asserts that it is a byte; at the loop's head
    the induction's step lets it be any value, so the step fails, and the count is proven by
    length only [32]. A program that reads standard input only through `$fill` is proven however
    that input arrives, cut into any pieces; a program on the network is proven for four ways its
    peer's bytes can arrive [33].
  - Sorting and word tallies are proven for texts of up to 8 bytes and tested beyond; SHA-256
    and AES-128 aren't modelled; and a server that waits on its connections with `poll_oneoff`
    isn't followed yet. Those stay "tested, not proven", and their lines say so [34].

  So "the line count is proven" doesn't mean every streaming program is. lengua says which
  programs, never that all are.
- **The host holds every run to what the idea names.** An idea names what its program uses:
  standard input, files, the environment, the clock, random bytes, the terminal, the network,
  the files the user names, and the programs it starts
  [35]. Every run, in tests and for the person, gets only that: lengua's own host checks the idea
  before every call, and resolves every path beneath the working folder through `cap-std`
  [5][36]. A program that names no network has none. By default a program doesn't connect at
  all: lengua makes each connection its idea names before it starts, does TLS itself for HTTPS,
  and hands the program a plain stream, so the program holds no network and no crypto. An idea
  may grant more, a line for each grant: connections the program opens itself, the addresses
  of names and UDP datagrams, each to the computers and ports the line names, with every
  address checked by lengua and this computer never reached; a file or a folder one of its
  arguments names, opened by the argument's place and never by a path the program built; a
  program it starts, by its name and the arguments the line bounds, never a shell; and the
  terminal's mode, put back however the program ends [37]. The
  clock is rounded down to a microsecond, so it can't carry finer timing [38]. What holds a
  program in is WebAssembly's own bounds, the host and `cap-std`: no operating system sandbox
  wraps the engine, and that second wall is deferred [39].
- **Secrets.** An idea's `- Secret:` line names inputs whose values must not steer the code: the
  rule refuses a branch, an address, a length, a loop's count, a division or a float operation
  of `main.wat` that depends on them [40]. It checks the WebAssembly, and then, on the CPU that
  runs the program, the machine code Wasmtime made for each operator that takes a secret, where
  it refuses a branch, a table, a call or a division. The CPU's own timing of the instructions
  left, and the code of operators that take no secret, are trusted [41].

The checks are fast enough to sit inside the agent's loop: every call of `check` replies within
about 40 seconds, and the proofs that need longer go on in the background, for up to five
minutes [42].

## Why WebAssembly

The native lengua compiled an idea to assembly, for Linux on x86-64 and on ARM64, and checked
the machine code itself; each CPU had its own code and its own prover [43]. WebAssembly gives one
module for every computer: the same `main.wat` and the same checks on all five systems and CPUs,
where the suite takes every program of the repository from its request to done [7][44]. Its
semantics are small and exact, so a prover can model them: the model follows its integer and
float operators, and SIMD128 but for float lanes' arithmetic, narrowing, widening, pairwise forms
and lane loads, each held to the engine's own bits [45]. Its memory is the program's own by
construction: an access outside it traps [46]. And it's a standard: the module lengua builds is a
WASI preview 1 module, which another runtime can run when the program reaches only standard
streams and files [47].

It costs speed. lengua's program runs in an engine, with lengua's checks before every run [48].
On the latest run of the gate, on each of the five runners, against C, C++ and Rust compiled to
WebAssembly and run in the same engine, lengua's program was ahead on module size in 40 of 42
rows. The gate has a noise margin of 5%: a row within it, or one its test doesn't decide, is
level, neither ahead nor behind [50]. By that count lengua was ahead by more than the margin on
wall time in 20 to 36 rows and behind in 0 to 2, and ahead on CPU time in 13 to 34 and behind in
0 to 7; Windows counts CPU time in 15.6 ms ticks, which leaves most of its short rows' CPU time
level. Its peak memory was 0.90 to 1.16 times theirs: behind by more than the margin on 7 to 18
rows of the Linux and macOS runners and on 39 of each Windows runner's [49]. Against the same
languages' native builds, it was behind on most rows: its CPU time was 0.51 to 7.00 times the
best native build's, below 1 only on the word tally and, on Windows on x86-64, the sort [49].
Those gaps are published per runner, and the list of rows where lengua is behind may only shrink
[50]. The gate passed on no runner in that run, which measured the list as it was before the
margin; with the list the run left, 6 rows it doesn't name are behind, on four runners [49]. W19
plans a lean runner and native builds of the proven `main.wat` [51].

## Why the idea is the mother

- **The idea is the source of truth.** People share ideas, and lengua checks code against them.
  An idea holds everything a program must do, in words and tests anyone can read and change.
- **Code is the agent's.** Anyone's agent compiles a shared idea for itself, and lengua checks
  that copy on that computer [52]. A folder of an idea and its code can be shared too: lengua
  checks its `main.wat` from scratch before it runs there, and `lengua-mcp check` checks the
  folder again on any computer, from its two files alone [12][52].
- **Every copy passes the same checks, never "the same program".** Two agents compile two
  different programs from one idea, which pass the same checks: the idea's tests, lengua's rules
  and the proofs. They can differ wherever the idea leaves room: inputs beyond its ranges, cases
  it doesn't mention, speed, even what's proven. So whatever must match belongs in the idea's
  behaviors and ranges.

Programs build on programs: an idea links to the ideas of its parts with `- Builds on:`, so a
folder of programs is a graph of two-file nodes, and lengua holds each program's `main.wat` to
the code of the parts it takes [53].

## How it differs

- **From programming languages.** In a programming language, a person writes how; the compiler
  is trusted, and what the program does is whatever the code says. In lengua, the person states
  what; the compiler isn't trusted, and what the program must do is the idea, checked.
- **From code generators that emit C or Rust.** A generator's output is code to keep, read and
  maintain, and it still depends on a trusted compiler and its own correctness. lengua keeps the
  idea, and checks the code the agent wrote, in the WebAssembly the engine runs; from there it
  trusts the engine's compiler [17][41].
- **From ordinary AI coding assistants.** An assistant writes code in a programming language for
  a person to review, and the review is the safety. lengua replaces review with checks the person
  doesn't have to perform: tests, rules, proofs and a host that holds every run. The person
  reviews only the idea, in their own words.

## The format

idea.md is an idea file, after Andrej Karpathy's LLM Wiki: plain markdown that any person or AI
can read, made of headings, lists and tables, with no other language inside. Its frontmatter
makes it an Open Knowledge Format page of type Attested Computation, whose computation is
`main.wat`, whose executor is lengua's `run` and whose attester is lengua's `check` [9]. Its prose
is in ASD-STE100 Simplified Technical English, which `check` holds to what a computer can see:
25 words a sentence, six sentences a paragraph, no -ing forms but technical names, and the step
vocabulary's phrasings [54]. An idea has a purpose, behaviors and constraints.

- Each behavior has Given/When/Then steps: that's its example.
- Its generated tests are the same steps with placeholders, over a table of where their
  variables come from, such as `int(-1000000000, 1000000000)`, and the expected results are
  formulas over those variables [16].
- The constraints name what the program uses, which is all its host gives it [35].

As in an LLM Wiki, the person's request is the raw source, the idea is the page the AI keeps,
and lengua's checks are the lint that keeps the page and the code true to each other. `check`
also keeps the wiki's index and log, `index.md` and `log.md`, for a folder of programs [55].

## What it promises, and what it doesn't

lengua's guarantees hold through lengua: `main.wat` builds into a standard module that can run
outside it, and then none of the checks above stand behind it [47]. Within lengua:

- every copy compiled from an idea passes the same checks, on every system lengua is built for
  [7][44];
- each proof covers what its line says, for every input in range or up to a length it names
  [28];
- every run is held to what its idea allows, the same way on every system: lengua's host gives
  its own answers, with one closed set of errors and the file names every system takes, so one
  operating system's extra freedom never makes an idea behave differently on it [56].

What lengua doesn't cover yet: threads and GUIs; and, of what a program reaches by itself, a
started program that works in the user's folder, a connection it opens to this computer, and a
proof of any program that opens its own connections, files or programs, which is tested, not
proven [57]. W18 gave each of those grants, each declared in
the idea and shown to the user before the program runs; W20 plans threads, as structured fork-join
that the rules show free of data races; GUIs aren't planned [51]. And lengua isn't a security
product: its examples show algorithms, such as AES-128 and SHA-256, and aren't built to resist
an attacker [58].

## Where it stands

lengua 0.2.0 works on five systems and CPUs as one product: the same skills, tools, rules,
tests and prover on each [7]. The suite passes natively on all five runners, and takes every
program of the repository from its request to done there [44]. Each runner's release archive is
built on that runner, then installed and checked on a clean runner of its own system and CPU,
through a plain MCP client [59]. In an interactive Claude Code session, an agent followed an
archive's two skills from a request to done, driving its server as the MCP client [60]. The
finish line isn't reached: the gate against C, C++ and Rust doesn't pass on any runner yet [49].
A try to run z3 as WebAssembly was measured against its bar, the same verdicts on all five
runners with the replayed questions taking at most twice as long: it took 1.70 to 2.28 times as
long and lost a proof on three runners, so native z3 stays, and the try for cvc5 wasn't made
[61]. After the finish line, four milestones aim at a replacement for C, C++ and Rust: reach,
native speed, parallelism, and a library of proven parts [51].

lengua is an Agent Plugin: two skills and one MCP server, using only open standards, with its
manifests held to the Agent Plugins 1.1.0 schemas [6]. No run has loaded it in a harness yet, and
loading it in Claude Code is the owner's step [60].

## Sources

Files are as they are in the repository at this writing; runs are GitHub Actions runs of
https://github.com/laughing-hermit/lengua.

1. `src/idea.rs`, the idea's format: the type `Attested Computation` and the sections
   `# Purpose`, `# Behaviors` (Given/When/Then steps and generated tests) and `# Constraints`;
   `plugin/skills/write-idea/SKILL.md`.
2. `src/checker/mod.rs` (the dialect, rules 1 to 10, M1 to M8 and secrets) and
   `src/checker/wasm/`; `plugin/skills/compile/references/power-of-10-webassembly.md`.
3. `src/expr.rs` (boundary values first, then draws from a seed); `src/server.rs` (a seed drawn
   for each check, and "Boundary values run first every time; the other generated inputs are new
   on every check").
4. `src/prove/wasm/mod.rs` (the model of a WebAssembly program, one operator a step over terms)
   and `src/prove/mod.rs`.
5. `src/wasm/host/mod.rs` ("Every function checks the program's idea before it does anything")
   and `src/wasm/host/policy.rs`.
6. `plugin/plugin.json` and `plugin/mcp.json` (Agent Plugins 1.1.0), held to the standard by
   `tests/manifests.rs`; `plugin/skills/` (Agent Skills), by `tests/skills.rs`; the server's two
   tools, by `tests/protocol.rs`.
7. `src/wasm/mod.rs` (one module, run identically on Linux, macOS and Windows, on x86-64 and
   ARM64); `xtask/targets.json` (the five runners); `xtask/src/release.rs` (an archive for each).
8. `src/state.rs` (`idea.md` and `main.wat`, a program folder's two files).
9. `src/idea.rs` (`runtime: webassembly`, `computation: main.wat`, and the executor and attester
   [docs/run.md](docs/run.md) and [docs/check.md](docs/check.md)); `src/ste.rs`.
10. `src/wasm/text.rs` (`main.wat` read by `wast`, lengua's library merged in, the module validated
    with the engine's features); `Cargo.toml` (`wast` 261.0.0 and `wasmparser` 0.261.0, pinned).
11. `src/lock.rs` (the lock signed with an HMAC-SHA256, keyed with a per-user secret, so an edit
    outside lengua is detected); `src/data_dir.rs` (the key in the user's data folder);
    `src/state.rs` (a program is ready to run only while its idea and code match the lock).
12. `src/trust.rs` (lengua's entry in `verified`); `src/server/wat.rs` (`attest`, which needs no
    lock and no key); [docs/check.md](docs/check.md).
13. `src/server.rs` (the server's instructions to the agent); `plugin/skills/`.
14. `src/server.rs` (`check`'s reply: its state, next step, problems, tests, proofs and memory).
15. `src/server.rs` (the done reply's next step, and its changes: the requests, the idea's diff,
    the tests that test less); `src/testdiff.rs`.
16. [examples/add/idea.md](examples/add/idea.md).
17. `Cargo.toml` (Wasmtime 50.0.1, with Cranelift, pinned); `src/wasm/engine.rs`.
18. `src/testfile.rs` (`EVERY_VALUE_MAX`, 5,000, and `SUITE_RUNS_MAX`, 20,000; `mark_every_value`,
    which leaves out a row with `choice(…)`).
19. `src/expr.rs` (the formula language: checked 128-bit integers, exact decimals, doubles and
    singles, and functions, with no loops).
20. `src/testfile.rs` (each behavior's formulas must reproduce its example; the dry run).
21. `src/state.rs` (each recorded idea kept by its hash); `src/testdiff.rs`.
22. `src/checker/wasm/rules.rs` (180 operators a function, two assertions a function on average)
    and `src/checker/wasm/flow.rs` (recursion and loops).
23. `src/checker/wasm/memory.rs`; `src/server/wat.rs` (the memory line).
24. `src/checker/wasm/structure.rs` (rule 3's imports).
25. `src/checker/wasm/determinism.rs`.
26. `src/server.rs` (the model held to the real program on up to 24 of a test's inputs before a
    proof); `src/server/wat.rs` (what a proof finds, run for real).
27. `src/prove/smt.rs` (z3 first, and cvc5 to confirm; floats to cvc5 alone; a condition the terms
    settle needs no solver); `src/prove/mod.rs` (inputs known to reach a branch, tried before a
    solver).
28. `src/server.rs` (each proof's line: proven for every input in range, or with text up to a
    length, or tested, not proven, with why).
29. `src/prove/wasm/tests.rs`: `proves_greet_and_echo_and_add_to_three_digits`,
    `proves_the_hypotenuse_in_doubles`, `proves_the_vector_templates_behaviors`.
30. `src/prove/mod.rs` (by length, a window of 65,536 lengths); `src/server.rs` (`prove_further`).
31. `src/prove/induction.rs`; `src/prove/wasm/mod.rs`.
32. `src/prove/wasm/tests.rs`: `proves_a_streaming_copy_for_every_length_by_induction`,
    `proves_a_vector_line_count_for_every_length_by_induction`,
    `proves_a_vector_word_count_by_length_as_its_flag_is_asserted`.
33. `src/prove/wasm/mod.rs` (a program that reads only through `$fill` is proven for every way
    its input can be cut); `src/prove/mod.rs` and `src/testrun.rs` (the four deliveries of a
    test's peer).
34. `src/prove/formula.rs` (`sort`, `uniq` and `tally` of texts up to 8 bytes; any other test
    function "the proof doesn't model yet"); `src/prove/wasm/host.rs` (a server's wait on
    `poll_oneoff` for its connections).
35. `src/idea.rs` (the `- Uses:` and `- Network:` lines); `src/wasm/host/policy.rs`.
36. `src/wasm/host/files.rs` and `Cargo.toml` (`cap-std` 4.0.3).
37. `src/connect.rs` and `src/wasm/host/net.rs` (the connections lengua makes before a program
    starts, and the TLS it does); `src/reach.rs` and `src/wasm/host/open.rs` (what a program
    opens on the network by itself, where its idea grants it); `src/named.rs`, `src/starts.rs`
    and `src/wasm/host/` (the files its arguments name, the programs it starts, the terminal);
    docs/reviews/reach-2026-11-11.md and docs/reviews/reach-2-2026-11-11.md (the threat cases
    and the test that holds each).
38. `src/wasm/host/clock.rs`.
39. [todo.md](todo.md), the decisions ("An OS sandbox as a second wall is deferred") and
    "Deferred, and why"; `src/wasm/pool.rs` (workers are child processes of `lengua-mcp`).
40. `src/checker/wasm/secret.rs`.
41. `src/checker/wasm/machine.rs` (what it reads of the machine code, and what it trusts);
    [DESIGN.md](DESIGN.md), "Secrets".
42. `src/server.rs` (`CHECK_WAIT`, 40 seconds; `PROOF_TIME`, 300 seconds).
43. [todo.md](todo.md), "Where lengua stands" and "Superseded by the pivot"; the tag
    `native-final`.
44. `tests/wasm_every_program.rs`; run 42947270993 (the suite on all five runners, main at
    33ed7fe).
45. `tests/wasm_model.rs` (the model against the engine, each float and SIMD128 operator against
    the engine's bits); `src/prove/wasm/image.rs` (the SIMD128 forms the model doesn't follow).
46. `src/wasm/runner.rs` (an access out of bounds traps); `src/wasm/host/limits.rs` (memory held
    to what the module declares).
47. `src/main.rs` (`lengua-mcp wasm`, a WASI preview 1 module); [todo.md](todo.md), the
    decisions on WASI preview 1.
48. `src/server/wat.rs` (`run_from_shell`: run's checks, then the module).
49. Run 42964232679, at cd54e79, on all five runners, the first with the gate's margin: each
    runner's report in
    [docs/benchmarks/](docs/benchmarks/), its gate table, its native gaps and where the run and
    KNOWN_BEHIND disagree.
50. [KNOWN_BEHIND](KNOWN_BEHIND); `src/gate.rs`; `benches/against_rust_and_c.rs`.
51. [todo.md](todo.md), "After the finish line" (W18 to W21) and "Deferred, and why".
52. `plugin/skills/compile/SKILL.md`, "Shared ideas".
53. `src/wasm/parts.rs` and `src/server/wat.rs` (a function taken from a part must be the part's
    word for word); `src/graph/mod.rs`.
54. `src/ste.rs`.
55. `src/graph/mod.rs` and `src/graph/okf.rs`.
56. `src/wasm/host/mod.rs` (lengua's own semantics: the closed error set, the names every system
    takes, byte-exact names); `src/names.rs`.
57. `src/server/wat.rs` (an idea with a `- Threads:` line is refused); [todo.md](todo.md), the
    finish line's point 5 and "Deferred, and why".
58. [todo.md](todo.md), "Stopped by the safety filter".
59. Run 42947303258, at main 33ed7fe (each runner's archive built, installed and checked);
    `tests/release_check.rs`; `.github/workflows/ci.yml` (the install job).
60. [todo.md](todo.md), "The agent's test in this session".
61. [todo.md](todo.md), the decisions on z3 and cvc5 as WebAssembly.
