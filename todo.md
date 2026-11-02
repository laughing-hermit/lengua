# lengua's agenda

In order. Each item starts when the one before it is done.

## 1. Perfect lengua for this target (x86-64 Linux)

Done on 2026-11-01: every part of the finish line set below is met (first marked done on
2026-10-31, then reopened the same day). Round 7: word counts proven for every
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
few bytes, floats computed in another order than the formula's, a folder's listing and the
`sha256` formula. The fastest streaming programs, the benchmark's word count among them, are
proven for every length with a 64 KiB buffer (the round below); what's left of that, and sorting
at every length, is past the finish line set at the end of this item.

**Next round: proofs whose cost grows with the input, not its square** (planned 2026-11-01,
revised after a design review the same day). The fastest streaming programs aren't proven for
every length because the induction's base proves each final piece's length one by one, building
terms for every byte each time: quadratic in the buffer. The step is already linear and
canonical at 64 KiB for counts (`popcnt` is a count, and counts merge). The review showed that
summarizing inner loops with a known number of times round only cuts the constant, and that a
summary built from the formula's own terms leaves nothing downstream to catch it if it's wrong.
What's linear is one run whose final piece has a length the input chooses, each byte's part
guarded by whether it's inside: one shape for every final length, which is also what reading
with raw `read` (any size each time round, as the fastest `wc`'s megabyte reads) needs. Done
when the fastest `wc` is proven for every length with a 64 KiB buffer or more. Plan:
`~/.claude/plans/round-10-loop-summaries.md`. Progress (2026-11-01): the step's fold detection
no longer asks the solvers about every temporary register (the line counter's step at 64 KiB:
five minutes to six seconds); the base takes every final piece at once, its length the input's
choice, and merges the exits of a loop that leaves on that length, so `pairs` at 4 KiB is proven
in under four seconds (nine minutes before); and a condition on the length alone is worked out as
the set of lengths where it holds, so a loop that masks its last chunk with `bzhi` meets the
formula's guards: the line counter with a 64 KiB buffer, C's size, is proven for every length in
under 30 seconds with no solver question; and counts that look back take every final piece at
once after a whole buffer-full, so the skill's fast `wc`, rewritten as `fill` into 64 KiB and one
loop that masks each 64-byte step to the bytes left, is proven for every length in about two
minutes with no solver question. Done, so this round's work for x86-64 is complete; what's left
of it is past the finish line below: a tail routine that loads its last vector at a place the
length chooses, raw `read`s of any size, and byte loops at 64 KiB, whose counts keep a copy of
all their parts at every byte (counts that share their parts would fix it).

**The finish line, set** (2026-11-01). First "everything that can be proven, however long"; then
the user set it: "perfect what you can for each target ... a set finish line ... this just needs
to be a general purpose programming language replacement and improvement". So each target gets a
concrete line judged by what a general-purpose language replacement needs, not open-ended proof
research, and x86-64 Linux is done when:
- Done 2026-11-01: the proof tests that failed only when every slow test ran at once (long proofs
  take turns).
- Done 2026-11-01: mean's decimal means, two prices up to a million, proven in about 80 seconds
  with 16 cores, and all six of its blocks through the server in under three minutes. The digits
  the program printed and the formula's had been built two ways, so the solvers compared them:
  a value halved by a shift now compares as the value itself, and a quotient narrowed to 63 bits
  and widened again is the quotient.
- Done 2026-11-01: counts of a part that can overlap itself (`count(s, "aa")`), proven for every
  length. The formula counts such a part as a matcher reads the text, and flags a program keeps
  as numbers are their conditions, so `tests/fixtures/pairs`' flag chain and the formula's are
  one term (by length: no solver question where the solvers had stopped at 46 bytes); the
  induction learns which register keeps the matcher's phase across a cut from the first time
  round, and proves every later one keeps it (about 2 seconds, no solver question).
- Done 2026-11-01: the fastest streaming programs for every length. The line counter with a
  64 KiB buffer in under 30 seconds, and the skill's fast `wc`, rewritten as `fill` into 64 KiB
  and one AVX-512 loop that masks each step to the bytes left, in about two minutes, both with no
  solver question; the proof keeps its time within one long shape too. Masking every step costs
  the `wc` a third more CPU time than its old, unprovable shape at the same wall time, still
  ahead of C, C++ and Rust (BENCHMARKS.md).
- Done 2026-11-01 (`0334548`, CI green): the gate, the goldens and the benchmark current, and
  the skill and DESIGN saying which shapes are proven for every length.

Past the line, so not blocking the next target: programs of these shapes are checked, tested and
proven up to a length, and each proof line says so.
- Sorting at every length and word tallies past a few bytes, which need quantified facts over
  memory.
- Byte-at-a-time loops at 64 KiB, whose counts keep a copy of every part at every byte (counts
  that share their parts would fix it).
- A separate routine for the last partial vector at a place the length chooses (the fast `wc`
  before 2026-11-01, `tests/fixtures/words`), and raw `read`s of any size.

Not by their nature, so "tested, not proven" is the right final answer: floats computed in
another order than the formula's (the results really differ), a folder's listing (the kernel's
order), the `sha256` formula (beyond the solvers), and the safety-stopped items below.

### Safety-stopped work, for a software engineer to review

Claude's own safety filter, in Claude Code, stopped some replies. Claude never produces what a
stopped reply held again: not reworded, and not by routing it through other models or subagents.
The user asked for Sonnet 5.5 / Haiku 5.5 agents for it on 2026-10-31 and again on 2026-11-01;
both times that was declined, because switching models to get around a safety stop is
circumvention, whoever runs the subagent. This list is so an engineer can review it and, if they
choose, do the work by hand. Each entry says what was stopped, what it was working toward, what
lengua already has for it, and how to finish and check it. Anything a safety filter stops from
here on goes here too.

1. **lengua's own SHA-256 example program** (stopped 2026-10-31, several replies).
   - Working toward: a bundled example, such as `examples/sha256/spec.md` and `main.s`, that prints
     the SHA-256 digest of standard input, written in lengua's x86-64 dialect.
   - Already built, not blocked: the CPU's SHA instructions are in the dialect and modeled in the
     prover (`src/prove/exec/vector/crypto.rs`), checked against this CPU on random values
     (`src/prove/exec/against_cpu/vectors.rs`); the test language's `sha256(s)` gives each test's
     expected digest (`src/expr.rs`).
   - To do it by hand: write spec.md with FIPS 180-4's examples as its examples and a generated
     block of texts whose output is `{sha256(text)}`; write main.s from the compile skill's
     templates (input through `fill`, the rules in `references/power-of-10.md`); call `check`.
   - Expected result: every test runs; the proof says "tested, not proven", since the `sha256`
     formula is beyond the solvers (above), a limit of its own.
   - Repository effects: a program in `examples/` is also checked by
     `every_program_in_the_repository_follows_every_rule` and joins the checker golden's programs
     (`src/checker/golden.rs`): record it again with `LENGUA_GOLDEN=write`.
2. **lengua's own AES example program** (stopped 2026-10-31).
   - Working toward: a bundled example, such as `examples/aes/`, that encrypts with AES-128 using
     the CPU's AES instructions.
   - Already built, not blocked: the AES instructions are modeled and checked against the CPU
     (the same files as above).
   - To do it by hand: spec.md with FIPS 197's known-answer vectors as its examples. The test
     language has no AES function, so generated tests need one added (`src/expr.rs`, and
     `src/prove/formula.rs` for proofs), or the spec keeps to examples.
   - Expected result: tested; a proof isn't expected.
3. **AArch64's AES, SHA and carry-less multiply instructions** (not attempted, 2026-11-02). For
   parity with x86-64, whose prover models them, Claude left these rather than draw the filter
   again: modelling them means writing the algorithms' tables and rounds into the prover, the
   topic of items 1 and 2.
   - Working toward: AArch64's `aese`, `aesd`, `aesmc` and `aesimc`; `sha1c`, `sha1p`, `sha1m`,
     `sha1h`, `sha1su0`, `sha1su1`, `sha256h`, `sha256h2`, `sha256su0` and `sha256su1`; and
     `pmull`/`pmull2` (carry-less multiply, not itself crypto, but part of the AES extension), in
     the dialect, held to a CPU that has them, as `crc32cx` is (2026-11-02).
   - Already built: x86-64's models of the same operations (`src/prove/exec/vector/crypto.rs`) and
     their checks against the CPU (`src/prove/exec/against_cpu/vectors.rs`); AArch64's CRC-32
     instructions as the pattern for an extension: `CpuFeatures.crc`, `Target::features` (QEMU's
     CPU has every extension), `.arch_extension crc` in `checker/source.rs`, the gate in
     `checker/aarch64.rs`, the decoder class in `isa/aarch64/decode.rs` held to LLVM by
     `isa/aarch64/tests.rs` (with `--mattr`), and probes in `prove/exec/against_a64.rs`.
   - To do it: decode and parse each class (LLVM's encodings), give each its facts (vector
     registers read and written), model each from the Arm Architecture Reference Manual's
     pseudocode (AESE is AddRoundKey, SubBytes and ShiftRows; AESMC is MixColumns), detect
     `aes`, `sha2` and `pmull` with `is_aarch64_feature_detected!`, and accept
     `.arch_extension aes` and `sha2`.
   - Expected result: every class agrees with LLVM, and the model with QEMU and ARM64 silicon on
     CI. No lengua SHA or AES program is needed for it.
4. **Replies stopped on 2026-11-01**, with no work lost: a reply to the request to do items 1 and 2
   through Sonnet 5.5 / Haiku 5.5 agents, and a reply that went on with item 1's ordinary work (a
   gate's result), which a new reply then did. The filter can stop replies near this topic even
   when the work in them is ordinary, so Claude leaves the topic and goes on with unrelated work.

**Not blocked, and safer alternatives Claude does itself.** The crypto support is built: a user can
write or drop in a standard SHA-256 or AES program and lengua checks and tests it. For a value
computed from the input, checked, tested and proven, lengua's own program is a plain checksum, far
from the stopped topic, which shows the same kind of proof:
- CRC-32C, done: `plugin/skills/compile/assets/fast/crc32c`, the checksum iSCSI and ext4 use, is
  checked, tested, measured against C, C++ and Rust, and proven against `crc32c`.
- CRC-32, as zip, gzip, PNG and Ethernet use it, could follow: the test language already has
  `crc32`, but a program computes it in software, a table or a loop of shifts, and the prover would
  have to read that loop as the formula's chain of byte steps to prove it for every length.
- Adler-32 (zlib's checksum) and FNV-1a (a hash-table hash) could follow too, each with a function
  added to the test language.

**What the alternatives don't cover: security.** A checksum catches accidental damage, such as a
corrupted download or a flipped bit on disk. It doesn't stand up to anyone changing the data on
purpose, who can always keep the same checksum, and it hides nothing, as AES encryption does. So
the checksums show lengua's kind of proof but don't replace SHA-256 or AES:
- lengua isn't to be presented as covering security because of them, in the white paper (item 2),
  the site (item 4) or anywhere else;
- they're not for a program that must resist an attacker, such as checking that a download wasn't
  tampered with, or keeping data secret;
- that still takes SHA-256 or AES, which a user can supply and lengua checks and tests (items 1 and
  2 above), or the engineer's hand-written examples once they exist.

## 1b. A good product, on both Linux targets

Set by the user on 2026-11-01: the goal is a good product, not only the finish lines set so far.
Claude first brings Linux ARM64 to parity with x86-64 (item 3's rounds R12 to R16), then stops;
the user takes on what follows, for both Linux targets. x86-64 meets its line (item 1), and as a
product Claude puts it at about two-thirds. What's left, most important first:

1. **Evidence that it works for people today.** Nobody has measured the end-to-end success rate
   since many changes landed: one request in real interactive Claude Code, a passing program out.
   A suite of 10 to 15 sessions over representative specs on each Linux target (ARM64's on an
   ARM64 computer), run through `scripts/interactive-session.py`, at about $2 of Claude usage
   each. Record each session's result, rounds and time, and fix what fails.
2. **Reach.** lengua builds self-contained tools, filters and servers, but no program can use an
   existing library, so there's no HTTPS, database, compression or GUI, and memory is fixed up
   front from the spec's limits. HTTPS is the gap most programs hit. A way that keeps lengua's
   guarantees: the launcher, which already hands a server its socket, does TLS on it in Rust
   (rustls), outside the program, so the program holds no crypto. Done 2026-11-02: lengua makes
   every client connection itself and hands it over (below), to this computer or to a computer
   the spec names, so a database or another server's own protocol is in reach, and `over TLS`
   has lengua do TLS on a named computer's connection (rustls with ring, Mozilla's roots built
   in, the same on every system), so HTTPS is in reach and the program speaks plain HTTP. Not
   done: a launcher service for compression or a GUI, and memory sized at run time, which
   NASA's rule against allocating after start leaves out by design. Each such service is one
   definition that every OS must be able to provide (item 3's limits).
3. **Linux held to macOS's and Windows's limits:** item 3's audit, every item a limit to
   establish by the strict rule (2026-11-02), done on both Linux targets on 2026-11-02 (names,
   lengths, arguments and environment, open files, closed pipes, open files renamed or removed,
   listing order). The network is done (2026-11-02): no program
   connects by itself, on either Linux target; lengua makes each connection its spec names before
   the program starts and hands it over as descriptor 4, 5 and on, as it binds a server's port.
   What that leaves for later, each a capability the cut took, to bring back in a shape every OS
   keeps the same way:
   - **A computer the person running the program names.** Today a program reaches only the
     computers its spec names, so a curl-, ssh- or database-client-like tool, whose user types the
     host, can't be built. A way that keeps the promise on every OS: a line such as `- Network:
     connects to TCP port 443 on the computer its first argument names` (`over TLS` too), where
     lengua reads that argument before the program starts, holds it to a computer's name or
     address (never this computer, as for a named host), and makes the connection there; the
     program sees the same argument, for its `Host:` line. Its tests' server stands in for any
     value the argument takes. The person running it chooses the host, as they choose its files.
   - **A connection for each client a program serves**, as a proxy needs, or several to one place:
     connections made on demand, which needs a way to ask lengua for one that every OS's sandbox
     can carry (a descriptor handed over on request), so a service of its own.
4. **Hardening.** Fuzzing and mutation testing are done on both Linux targets (2026-11-02; DESIGN,
   "Every reader is fuzzed"). Every reader, the built executable included, is fuzzed, a short pass
   with every test and a long one by hand, both clean after one fix; every proven example's and
   template's mutants are caught, or change nothing a spec can see; and the one gap it found,
   tests never cutting standard input into pieces, is closed. Left:
   - the red-team pass, programs written to escape the sandbox, fool the rules or the memory
     checks, or pass a proof they shouldn't, which the safety filter stopped while it was being
     planned (2026-11-02; see "What Opus 5.5 didn't allow");
   - the mutants tested and not proven, most at the 60-second proof budget, which a sharper test
     each would catch outright.
5. **Shipping to others.** A public release with the installer for both Linux targets (R16 brings
   ARM64's), the plugin listed where agent plugins are found, the white paper (item 2) and the
   site (item 4).

Also open: item 1's list past its line, and the safety-stopped work at item 1's end, which Claude
leaves to the user.

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
  - Proofs that the program is right for every input, confirmed by two independent solvers. How
    far a proof reaches depends on the code's shape, not only on what it computes (DESIGN lists
    which shapes are proven for every length): "the fast `wc` is proven" isn't "every streaming
    program is", so the paper says which programs, never that all are.
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

Every target is the same lengua, never a different product (DESIGN's "One product on every
target"): the same spec, skills, tools, reports and messages; the same rules, memory safety, tests
and proof engine; and the same limits on every OS. The capabilities a spec can name, and what the
sandbox allows of each, are one definition that each OS enforces with its own means. In the
user's words (2026-11-01), "any limit per OS and cpu is a limit 4 all targets", for equality in
experience; made strict on 2026-11-02: every limit on one target is established on all targets,
whatever their assembly or their OS's permissions, so it's the same product regardless. A limit is
anything a spec or its user can observe: what a program may do, how it behaves, what it's refused
and when, and what lengua promises of it. No target is excused a limit because something else of
its own, lengua's checks included, could keep the same promise there: where one OS can't keep a
promise the way another does, every target takes on its limit, and the capability is reshaped so
every OS keeps it the same way, or that part is cut on every target, Linux too. Only the hardware
may differ (its
instructions, vector widths, speed). Each target's finish line is parity with x86-64's: it's done
when it covers what x86-64 covers, each item with its own hardware's means.

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
  through `fill`, and files and folders, checked, tested and proven. Proven since R9b
  (2026-11-01), with no solver question, through the server too, natively and under QEMU: the
  template, a streaming filter through `fill` for every length, and a file program
  (`tests/fixtures/linux-aarch64/upcase`, the upcase example's spec unchanged). R10's skills and
  references are done too: the compile skill compiles for the machine line's target, with
  `references/linux-aarch64.md`. R11 (2026-11-01) adds floating point in singles and doubles,
  checked, tested and proven: the hypotenuse example's spec unchanged
  (`tests/fixtures/linux-aarch64/hypotenuse`), all five blocks with no solver question, through
  the server too; the model is held to QEMU and ARM64 silicon for every float instruction, NaNs,
  zeros' signs, subnormals and the ends of each conversion included. R12 (2026-11-01) adds
  NEON's integer vectors, checked, tested and proven: the vector template from x86-64's spec
  unchanged, and the fast `wc` for every length with a 64 KiB buffer, as x86-64's is, with no
  solver question; the model held to QEMU and ARM64 silicon for every vector class in every
  arrangement. The memory rules follow a pointer an access's writeback moves, as they follow
  x86-64's `add`. Floating point on vectors too, as x86-64's packed doubles and singles have it:
  singles two or four at a time and doubles two at a time, each lane as its scalar form computes
  it, held to QEMU and ARM64 silicon; the hypotenuse with both sides in one vector of doubles is
  proven as the scalar one is; the benchmarks ran on ARM64 hardware (BENCHMARKS-aarch64.md).
  Then x86-64's other templates, from their specs unchanged: the streaming filter (proven for
  every length), the word tally (a hash table and a merge sort, proven for short texts), and
  checked jump tables proven case by case. R13 (2026-11-02): servers, clients and the terminal,
  with `ppoll` (AArch64 has no `poll`) in the prover, whose time the kernel writes back: the
  event-loop server proven in every order its clients' steps come in, the key reader, and the
  echo server and fetch client, each as x86-64's is. R14 (2026-11-02): threads, the parallel
  template with clone, futex and `ldar` (the join's load-acquire, as glibc's pthread_join reads
  the id on ARM64), proven on several threads and held to x86-64's thread rules, which now read
  each CPU's registers. R15 (2026-11-02): the CRC-32 instructions, `crc32cx` and the rest, held to
  a CPU that has them as x86-64's extensions are, and the fast crc32c from its spec unchanged,
  proven with no solver question as x86-64's is. Then the benchmark's last two programs on AArch64,
  histogram and crc32c, with C, C++ and Rust's crc32c using AArch64's instruction too, and R16
  (2026-11-02): the release archive per CPU, the installer taking x86-64 or ARM64 and refusing an
  archive built for the other, and the ARM64 archive built and checked on GitHub's ARM64 runners. Left of R10, skipped for
  now at the user's word (2026-11-01) so
  it costs nothing: 4 to 6 real interactive Claude Code sessions compiling specs for AArch64 from
  one request each, and one for x86-64 as a regression check; they run on the user's Claude
  account. With R16, lengua runs on ARM64 Linux at parity with x86-64 (2026-11-02), installed
  from its own archive, but for:
  - the CPU's AES, SHA and carry-less multiply instructions, which x86-64 models: left to the
    user, beside the safety-stopped work (item 1's end), since modelling them is the topic the
    filter stopped;
  - the real interactive sessions above, which cost Claude usage (item 1b's first point);
  - SVE, past the line;
  - the histogram's speed. Measured on GitHub's ARM64 runner (BENCHMARKS-aarch64.md, 2026-11-02),
    lengua is ahead or level in cycles and CPU time on every program but this one (add 114×
    fewer cycles, hypotenuse 100×, wc 2.5×, upcase 1.5×, shout 1.3×), which takes 115.6M cycles to C's
    94.7M and Rust's 95.2M (0.8×; 0.9× in CPU and wall time), where x86-64's is level. Tried, each
    measured: 8 bytes a step into 8 tables, which cut its instructions 26% to level with Rust's
    but its cycles only 4% (it stays at about 1.72 cycles a byte to C's and Rust's 1.41, so the
    cost is per byte); the tables spaced off the 4 KiB pattern, no change; each byte loaded with
    ldrb as C and Rust do, slower (120.1M), reverted. Next would be reading the CPU's own stall
    counters on the runner, which the benchmark doesn't record yet. The crc32c, after taking 64
    bytes a step and each read as it comes, is level with Rust in cycles and CPU time (9.05M to
    9.02M); it can't lead as x86-64's does without the carry-less multiply (PMULL, above), since
    one crc32cx chain already keeps the CRC unit busy. Its wall time trails C's (0.7 to 0.8×)
    because the benchmark feeds 64 MiB through a pipe on a 2-core machine, where the faster
    programs, lengua and Rust alike, wait on the writer.
- **macOS and Windows need more than a new CPU.** They don't allow a program to call the OS
  directly, as lengua's programs do; a program must go through the system library (libSystem,
  ntdll), which lengua never checked, and there is no seccomp or Landlock. So those four targets
  need a different model of the program's edge with the OS and a different sandbox.
- **Linux held to macOS's and Windows's limits now, not when they arrive.** Linux allows more than
  macOS and Windows do, which is why the user set the same limits on every OS (2026-11-01): a spec
  that works on Linux must work on the stricter OSes too. So each place a spec can observe Linux's
  extra freedom gets checked against both OSes' documentation and, where one can't do it, cut on
  Linux too, in its tests and sandbox. By the strict rule (2026-11-02), every item below is a
  limit to establish on Linux, both CPUs, before macOS or Windows arrive; none is waived because
  a Linux means, lengua's checks included, could keep it. Done on both Linux targets
  (2026-11-02), each checked against Microsoft's and Apple's own documentation; each limit has a
  test on both CPUs, and the means are lengua's supervisor (`src/names.rs`, `src/supervise.rs`),
  the launcher and the spec's checks:
  - file and folder names, done: what Windows refuses (`< > : " | ? * \`, control characters, a
    trailing dot or space, and `CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9` and
    their superscript forms, with any extension) gets `EINVAL`; a name that isn't UTF-8 of
    characters Unicode 9.0 assigned (macOS's APFS refuses the rest) gets `EILSEQ`; a new name that
    differs only in case or Unicode normalization from one in its folder gets `EEXIST`. The
    supervisor checks each `openat`, `mkdirat`, `unlinkat` and `renameat` in tests and real runs,
    the prover gives the same answers (a name the input chooses is split on Windows's ASCII rules,
    and one that may hold other characters is tested, not proven), and the spec's own test files
    are held to the same rules. Under QEMU, which the supervisor can't follow, the names a run
    leaves are checked afterwards. Not modeled: Windows's 8.3 short names, which a long name can
    collide with where they're on;
  - path lengths, done: a name past 255 bytes or a path past 1,023 (macOS's `PATH_MAX`) gets
    `ENAMETOOLONG`; lengua's Windows programs would use NT paths, which have no 260 limit;
  - arguments and the environment, done: Unicode only (the CLI refuses an argument that isn't);
    the arguments take at most 32,000 characters of Windows's 32,767-character command line,
    counted as Windows quotes them, and each variable at most 32,767; names that differ only in
    case are one variable, so a spec or a run can't give both;
  - open files, done: every program holds at most 4,352 at once on every target (macOS's default
    of 256 rises to that within its `OPEN_MAX` of 10,240; enough for a server's 4,096 clients);
  - a write to a closed pipe, done: an error (`EPIPE`) on every target, as on Windows, never a
    signal;
  - an open file renamed or removed, done: `EBUSY` on every target, as Windows refuses it (it
    allows it only with every handle sharing delete, and never on FAT);
  - the order a folder lists in, done: no system promises one, so a test whose program listed a
    folder runs again with each listing turned round by the supervisor, and fails if its result
    changes; listings stay tested, not proven;
  - the network, done 2026-11-02: Windows's sandbox for apps (AppContainer) can't hold a program
    to one port, blocks the local network by default, and reaches this computer's own address
    only with an administrator's exemption, so a program's own `connect`, which Linux held to the
    spec's ports by Landlock and to this computer by lengua's tracer, was Linux's extra freedom.
    Now no program connects by itself on either Linux target: lengua makes each connection the
    spec names (a TCP port of this computer, or of a computer it names) before the program
    starts, outside the sandbox, and hands the program its end of a local stream that lengua
    relays, so the program holds no network at all, which every OS's sandbox can enforce (on
    Windows the program's end would be a pipe or socket handle, to confirm when Windows lands).
    seccomp allows no `socket` or `connect`, and Landlock no connection or binding of the
    program's own. `on any computer`, where the program chose its address, is cut everywhere: a
    spec names the computer. A connection lengua couldn't make comes ended, the same on every
    OS, and every write to a closed pipe or connection is an error, not a signal.

  Per-target code, such as x86-64 Linux's red zone or Windows's stack probes, isn't such a limit:
  only what a spec can observe is.

The plugin itself (skills plus one MCP server) is agnostic in its skills, which are markdown and
install the same on every host. Its server is a native binary, built per host, and today builds
and runs only on Linux, since its sandbox and tracer are Linux-only. So installing the plugin on,
say, a Windows x86-64 machine and having it work is the Windows host's server build **and** the
Windows target profile — the same work as item 3 for that host. See DESIGN's "Porting a target".

## 4. A site for sharing specs

A simple Next.js landing page where people share their specs, never code, and anyone compiles them for their own computer. It promises only what lengua enforces:
- the spec travels, and every copy compiled from it passes the same checks; it never promises the same program;
- each computer compiles and checks its own code;
- main.s is plain text anyone can build outside lengua, so the guarantees hold only through lengua's own `run`;
- a proof covers what its line says: for every length only for programs in the shapes DESIGN lists, and up to a length otherwise; never that every program is proven.
