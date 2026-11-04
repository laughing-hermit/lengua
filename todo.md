# lengua's agenda: what's left for people

**Targets: two of six exist.** lengua is meant to be one product on six targets: Linux, macOS and
Windows, each on x86-64 and on ARM64. Only the two Linux targets exist. **The other four are
missing:** macOS on x86-64, macOS on ARM64, Windows on x86-64 and Windows on ARM64 (see "Skipped,
and why" below); each is a port of its own and none is started. The owner set this run's finish
line on 2026-11-04: the two Linux targets only, done and the same product, and every item stopped
by the safety filter finished. Nothing in this file about macOS or Windows is part of that line.

As of 2026-11-02, both Linux targets, x86-64 and ARM64, are done, and they're one product: every
capability, rule, limit, example, test, proof and measurement on both, only the hardware differing
(DESIGN's "One product on every target"). Claude (Opus 5.5) did everything else this agenda held:
x86-64's finish line and ARM64's parity with it; the network through the launcher, with TLS, and
the computer an argument names, as a curl-like tool's user types it; Linux held to macOS's and
Windows's limits, Windows's short names among them; fuzzing and mutation testing, with a test step
that closes standard output so a promise about failed writes is tested and proven; ARM64's
carry-less multiply, AES and SHA instructions and SVE; the ARM64 histogram ahead of C and Rust; a
twin of every program on the other CPU; stall counts in both CPUs' benchmarks; the session suites
(SESSIONS.md); the white paper (WHITEPAPER.md); and the site for sharing specs (`site/`,
built and run locally).
The record of that work is in the git history, DESIGN.md, README.md, SESSIONS.md and the
BENCHMARKS files.

What follows is what was stopped by the safety filter and is now done (with what that found), then
what is **skipped**, each with why and how an engineer would finish it.

## Stopped by the safety filter, and done since

Claude's own safety filter, in Claude Code, stopped some replies in earlier runs (2026-10-31 to
2026-11-02), so Opus 5.5 left these items. On 2026-11-04 the user gave Sonnet 5.5 explicit
permission and consent to do them, on GitHub's free runners only (nothing built, tested or proven
on the user's computer), with both Linux targets the same in everything: every program, spec,
limit, test and proof on x86-64 and on ARM64, one byte-identical spec for each pair. The ground
rule held: had the filter stopped a reply, that item would have stopped and been recorded here,
never routed around through other models or subagents. It stopped none.

1. **lengua's own SHA-256 example: done, in plain integer code on both CPUs, with the instructions
   as fast tiers.** `examples/sha256/` (spec.md and main.s) prints the SHA-256 digest of standard
   input with rotates, adds and xors alone (SSE2 only to copy the state), so it runs on every
   x86-64 computer: a schedule loop of 16 big-endian loads and 48 words, and a round function that
   keeps a and e as lists in time order, so a round moves no register. The form with the CPU's
   `sha256rnds2`, `sha256msg1` and `sha256msg2` is the fast tier
   `plugin/skills/compile/assets/fast/sha256-ni/`, beside a spec.md of the same bytes. The spec
   has FIPS 180-4's vectors (abc, the empty text, the 448-bit message) and generated blocks whose
   output is `{sha256(text)}`, with the block-boundary lengths (0, 50 to 70, 118 to 134, 4090 to
   4102) stated and tested. Its twin `tests/fixtures/linux-aarch64/sha256/` has the same spec
   bytes and is `ror`, `eor` and `add`, NEON only to reverse bytes and copy the state, its lists
   growing downward; the tier `linux-aarch64/fast/sha256-sha2/` uses `sha256h`, `sha256h2`,
   `sha256su0`, `sha256su1`. Every form reaches `done` with 1204 test runs. Proof: "tested, not
   proven", as expected: the prover doesn't model the test function `sha256`, and a hash's proof
   fills the memory first (below).
2. **lengua's own AES example: done, in plain integer code on both CPUs, with the instructions as
   fast tiers.** `examples/aes/` encrypts hex data (32 to 131072 digits) with a 128-bit key, block
   by block (ECB); bad input exits 3 with "aes: bad input". The key schedule goes through the
   S-box and each round a column at a time through one table of 256 words (SubBytes, ShiftRows and
   MixColumns together, the word rotated for rows 1 to 3; the S-box is that table's byte 1, which
   the last round and the key schedule use), so it runs on every x86-64 computer. The form with
   `aesenc`, `aesenclast` and the key schedule from `aesenclast` and rotated words is the fast tier
   `plugin/skills/compile/assets/fast/aes-ni/`, beside a spec.md of the same bytes. The test
   language got `aes128(key, data)` (`src/aes.rs`, a software AES with FIPS 197 and SP 800-38A unit
   tests; `src/expr.rs`; documented in the write-spec skill's test-format.md), so the spec's
   generated blocks test every length against it. The twin `tests/fixtures/linux-aarch64/aes/` is
   the same program with `ror` and `ubfx`; its tier `linux-aarch64/fast/aes-aes/` uses `aese`,
   `aesmc`, `tbl`, `ext`. Every form `done` with 1406 test runs; proof "tested, not proven". ECB
   is the mode the spec states because the spec is a function of the data alone; it shows the
   algorithm, it isn't a product for hiding data.
3. **Replies stopped on 2026-11-01**: nothing to do; no work was lost.
4. **The hardening pass's red-team part: done, defensively.** `tests/red_team.rs` (one table of
   attempts with a cell for each target, which replaced `adversarial.rs` and `adversarial_aarch64.rs`,
   two copies that had drifted apart) takes each target's template and makes one change each that
   tries to get past lengua; every attempt must be refused or never stamped `done`:
   system calls the spec doesn't allow, a system call number chosen by a register or label, reads
   past a buffer, an unchecked or late index, an address from a plain number or a pointer read from
   memory, writes to read-only data, a changed return address, loops the rules miscount, recursion,
   calls through a register, a missing `ret`, a tampered `assert_fail`, an ignored result, and a
   wrong program that must never be stamped `done`. What it found:
   - **Two real gaps on ARM64, fixed**: a function could overwrite `x30`, its return address,
     with an instruction (`mov x30, xzr`), or overwrite the copy its frame record keeps through
     memory (`str xzr, [sp, #8]` after `stp x29, x30, [sp, #-16]!`), and `check` caught either only
     when the tests crashed, where x86-64 refuses `mov qword ptr [rsp], 0` before running (rule
     M3). Now a rule refuses any instruction that writes the link register between the frame
     record's save and restore (`src/checker/rules.rs`), and M3 holds the saved return address in
     the frame record's top eight bytes as it holds x86-64's return-address slot
     (`src/checker/memory.rs`); both are red-team attempts on ARM64 now.
   - **A known limit, tested on both CPUs: a pointer read from memory.** The rules follow an
     address to its owner where they can; one loaded from memory has none they can follow, and
     refusing every such load would refuse valid programs (a table of pointers, `argv`). So it
     isn't refused up front, and "any attempt not refused is a bug" holds for the rest, not for
     this. What holds instead, asserted by `an_address_made_from_a_plain_number_is_refused` on x86-64 and
     ARM64: a wrong one is killed by the sandbox and the program is never `done`; a good one may
     be `done`, and the report then says memory is "tested, not proven", never proven. Narrowing
     this (following a loaded pointer back to a table whose entries are all owned addresses) would
     change how M1 and M2 follow owners: a design change for an engineer, with the checker golden
     re-recorded.
   - Every other attempt is refused up front on both CPUs.
5. **The ARM64 twins of `total` and `amounts`: done.** `tests/fixtures/linux-aarch64/total/` and
   `amounts/`, each spec copied unchanged; `UNPORTED` is gone, so
   `every_program_has_its_twin_on_the_other_cpu` and `every_program_passes_its_checks_on_every_target`
   (tests/every_program.rs, which took the place of the per-CPU all-programs tests) now cover every
   fixture. Both reach `done` on ARM64 hardware.

**The ARM64 SHA-256 stall, found and fixed.** The one-folder check of the ARM64 SHA-256 twin ran
past the job's 90 minutes twice, while the same twin passed the full suite (which gives proofs no
time). A stack taken on the runner by `scripts/stall-report.sh` (the workflows' `stall_seconds`
input) showed the proof at 100% CPU inside `Pool::extract`, which pushed each bit extraction down
through shared terms once per path to a term, exponential in a hash's rounds; x86-64's twin was
shielded only because its prover stops at `palignr`. Fixed on both CPUs: extractions are
remembered (`src/prove/term.rs`), a run asks whether the proof's time is up every 4096 steps, though
a hash has no branch on its input to ask at (`Oracle::in_time`, `src/prove/exec.rs`), and the
model's concrete runs of the tests' samples stop at the block's deadline (`agrees_until`,
`src/prove/mod.rs`). Each has a unit test; the check no longer hangs.

**The software SHA-256's proof filled a runner's memory, found and fixed (2026-11-04).** With the
SHA-256 twins in plain integer code, the one-folder check of the ARM64 twin killed its job three
times (exit code 143, no artifacts, the stall report lost with the machine). A line of memory use
every 10 seconds in the job's log (`scripts/stall-report.sh`) showed `lengua-mcp` growing 250 MB a
second to 15 GB of the runner's 16 GB within a minute of the proofs starting, and the stall
report's stacks showed all four proof workers deep in `Pool::extract`, pushing low-bit extractions
down the sums of a hash's rounds. The watchdog (`src/prove/watch.rs`) only ever stopped a solver.
Three steps were needed, each shown by a run. First, `explore` asked the oracle whether the
computer still had room each time its pool had grown by 50,000 terms (`Oracle::has_room`). The
check then ended `done` (runs 41882085670, 41883380507), but the guard's message never showed in
the log and the runner's free memory still fell to 232 MB and 213 MB: one extraction through a
hash's whole history makes terms by the million between two steps of that loop. Second, the pool
looks itself (`Pool::look_at_the_memory`, in `make` and `extract`, each 50,000 terms and kept
extractions it adds): a pool short of memory answers every extraction with zeros at once,
`explore` ends the run at its next step, and `prove_shape`, `check_runs` and the induction step
accept nothing from such a pool, so the zeros never make a proof (one unit test on the pool, one
on `explore`, one on `watch::is_short_for`). That fired in a check (run 41888807170: three
warnings in the server's log, at 15.6, 23.0 and 23.3 million terms, and a proof ending "the
computer ran low on memory"), but the runner still fell from 4.8 GB free to 0.2 GB between two
samples ten seconds apart, probably in the doubling of pools' tables. Third, "short" for a pool
now means below the floor and twice its own size (256 bytes a term, as measured): run 41890097057
logged four warnings (9.75, 12.7, 12.7 and 19.8 million terms), ended `done` with 1204 test runs,
and the runner never had less than 6.6 GB free at a sample. The rule is conservative: the x86-64
AES check (run 41890269784) stopped five proofs at about 22.5 million terms and kept
extractions, when the server held 2.4 GB and 12.5 GB were still free, since each counts for 256
bytes and an extraction's entry takes much less; counting them apart would stop later. (The
four last blocks, the bad inputs, were proven in that run, for 31, 1, 1 and 1 input combinations,
and out of time in runs 41884704060 and 41891739265; why they differ was not looked into.) Why
the pool grows so fast on a hash of several blocks, and whether the extraction can stop short of
the sums' whole history, is not looked into: a change to `Pool::extract` for an engineer, with the
prover's goldens at hand.

**Found by the parity audit (2026-11-04), fixed in this run.** The user asked that the two Linux
targets be the same product, so after the five items four read-only agents compared them for
everything the hardware doesn't force: programs, rules and sandbox, tests, prover and documents.
They returned 56 findings (about 40 once the overlaps are merged), each with file and line on
both sides, which were checked against the code before anything was changed. Fixed:
- **ARM64's saved return address** and **failed assertions' lines** (above and below): a failed
  assertion named its `main.s` line only on x86-64, because the matcher looked for `call` and
  AArch64 calls with `bl` (`src/testrun.rs`, with a test on the ARM64 template and one through the
  server).
- **ARM64 mutation testing** cut each line at the first `#`, which AArch64 uses for numbers, so
  its programs got condition flips and almost nothing else; it now splits on the CPU's own comment
  marker (`tests/mutation.rs`). Its first run on the ARM64 runner (`--ignored
  every_proven_programs_mutants_are_caught`, 8 mutants of each of 7 programs) caught every mutant
  but one, which changes nothing a spec can see: the template's `cmp x0, #0` in `write_i64` moved
  to `#1` sends zero down the negating path, and zero negated is zero. It is in the test's
  `EQUIVALENT` list with that reason; the programs' proofs missed no moved number.
- **ARM64's red team** now has what x86-64's has (a write over the saved return address, a read
  below `sp`, a stray backward jump, a tampered assertion message, a system call's result never
  tested), asserts the pointer-from-memory case as x86-64's does, and its wrong-program test runs
  with time for proofs, where it ran with none and so tested the tests alone.
- **`check_one_aarch64_folder`** never failed; it now fails unless the job ends `done`, as x86-64's
  `check_one_folder` does.
- **ARM64's checker tests** gained planted-bug fixtures for the memory rules M1 to M6 and M8, and
  `b.al`/`b.nv` are now jumps, so the code after one can't run and an assertion it reaches always
  fails (they were read as conditional branches, which bypassed both checks).
- **x86-64's `total` fixture** broke three rules (a character as a constant, a `_start` of 77
  instructions, an assertion reached from two places) and was exempted from the repository-wide
  rules test, while its ARM64 twin passed them all. It is rewritten (`load_file`, `report`,
  `report_bad_line`), the exemption is gone, and `tests/every_program.rs` checks every x86-64
  example and fixture through the server, and every ARM64 twin, in one test for both CPUs. Both
  CPUs now give `total` the same 426 test runs and the same proof lines.
- **The fill model** (the soundness check behind "cut any way" proofs) was compared with the real
  `fill` for every way its input can be cut, with four planted bugs, only on x86-64; the same test
  now runs on AArch64's `fill` (`src/prove/exec.rs`).
- **Documents**: the plugin README, both manifests, the compile skill, `porting.md`, the root
  README and DESIGN no longer speak for x86-64 alone or call ARM64 "under way"; the AArch64
  reference gained what the x86-64 one had (files, the clock and random bytes, the terminal, the
  environment layout, `exit_group`, `shutdown`, the window size, `getrandom`'s 256 bytes, the
  checked jump table, the fast histogram and SVE `wc`, one assertion-number rule); the fixtures
  README lists every fixture and the 29 twins.
- **CI**: formatting is its own job (`targets.yml`), the same for both CPUs, and keeps the patch
  that formats a difference, where it used to stand in front of the ARM64 tests; the x86-64 job
  has the benchmark and release switches the ARM64 job had; both have `stall_seconds`.

**Not blocked, and safer alternatives.** The crypto support is built: a user can
write or drop in a standard SHA-256 or AES program and lengua checks and tests it. For a value
computed from the input, checked, tested and proven, lengua's own program can be a plain checksum:
- CRC-32C, done on both CPUs, a baseline form each and a fast tier each: the checksum iSCSI and
  ext4 use. `plugin/skills/compile/assets/fast/crc32c` is the form that runs on every computer of
  the target, from the very same spec: on x86-64 SSE4.2's `crc32`, one chain, 8 bytes a step (the
  x86-64 baseline holds SSE4.2, so it needs no table), proven against `crc32c` for every text up
  to 1,520 to 1,897 bytes in a runner's minute (runs 41884709896, 41876392984) and further with
  `prove_further`; on ARM64, where the CRC
  instructions are an extension before ARMv8.1, a table of 256 words a byte at a time, which the
  prover reads for texts of one byte in a minute and the tests check beyond (the prover reads the
  table lookup as terms of its own and the formula's chain of CRC steps meets it only at one
  byte). The tiers are `fast/crc32c-pclmul` (three `crc32` chains joined with `pclmulqdq`) and
  `linux-aarch64/fast/crc32c-crc` (`crc32cx` beside `pmull` folds), checked, tested, measured
  against C, C++ and Rust, and proven against `crc32c` up to 4,096 bytes.
- CRC-32, as zip, gzip, PNG and Ethernet use it, could follow: the test language already has
  `crc32`, but a program computes it in software, a table or a loop of shifts, and the prover would
  have to read that loop as the formula's chain of byte steps to prove it for every length.
- Adler-32 (zlib's checksum) and FNV-1a (a hash-table hash) could follow too, each with a function
  added to the test language.

**What the alternatives don't cover: security.** A checksum catches accidental damage, such as a
corrupted download or a flipped bit on disk. It doesn't stand up to anyone changing the data on
purpose, who can always keep the same checksum, and it hides nothing, as AES encryption does. So
the checksums show lengua's kind of proof but don't replace SHA-256 or AES:
- lengua isn't to be presented as covering security because of them, in the white paper
  (WHITEPAPER.md), the site (`site/`) or anywhere else;
- they're not for a program that must resist an attacker, such as checking that a download wasn't
  tampered with, or keeping data secret;
- that still takes SHA-256 or AES, which a user can supply and lengua checks and tests (`examples/sha256`
  and `examples/aes` show the algorithms, and their fast tiers the instructions, not a product that
  resists an attacker).

## What's left for parity and coherence between the two Linux targets

I know of no case where the two CPUs give different answers. What's left is places where one CPU
is checked or modelled less than the other, found by the 2026-11-04 parity audit (the ones fixed
in this run are listed above), heaviest first:

1. **Tests.** The biggest gap, narrowed on 2026-11-04 by writing each case once, as a table with a
   cell for each target (see "Keeping five or six targets the same product" below): the sandbox's
   limits (ARM64 had 8 native tests against x86-64's 25, and now both run the same 31), the red
   team, the memory rules' planted bugs and the all-programs test run natively on both runners.
   What is left:
   - *Checker fixtures.* The planted memory bugs are one table (`PLANTED`, `src/checker/tests.rs`,
     a cell for each CPU), but ARM64 still lacks the rest of what x86-64 has there: one address
     lent twice for writing (M4, `one_address_is_never_lent_twice_for_writing`),
     `the_stack_is_tracked_exactly`, the thread rules (M7), and about half of the Power of 10
     rules' fixtures. Each is a one-line edit of the ARM64 template (`with_aarch64`); the
     saved-return-address gap is what their absence cost.
   - *Prover tests.* ARM64's prover is tested on 7 of the 29 twins, x86-64's on about 20 programs,
     and its memory-bug tests are x86-only.
   - *Other x86-only coverage.* The checker's and the prover's golden recordings; executor tests
     of loop detection, connections and sleep; the 56 session tests of `tests/check_code.rs` and
     `tests/e2e.rs` against ARM64's 17 in `tests/aarch64_session.rs` (the missing ones are named
     in `DEBT`, `tests/parity_inventory.rs`, which lists every test file still made for one CPU);
     and the unmodified AArch64 template has its rules checked but goes through `check` to `done`
     in no test but the ignored mutation test.
2. **Prover reach.**
   - *x86-64's prover* can't follow dozens of vector instructions the ARM64 prover models
     (`palignr`, `pblendw`, `punpck*`, `pack*`, saturating and widening arithmetic, string
     compares, `pextr*`/`pinsr*`, `pmovzx*`, `gf2p8*` and more: `src/checker/x86_64.rs` against
     `src/prove/exec/vector.rs`). The same spec therefore ends "tested, not proven" for different
     reasons: x86-64's SHA-256 stops at `palignr` ("doesn't model yet"), ARM64's runs until its
     time is up ("ran out of time").
   - *Pointer tracking.* ARM64 follows pointers through fewer instructions: x86-64's `plan`
     follows `cmov`, `and` to a boundary and scaled `lea`; ARM64's `plan_a64` doesn't follow
     `csel`, `and`-immediate or shifted `add`, so those accesses are held to the weaker "object the
     value points into" rule.
   - *An unexplained difference.* In the same 60 seconds, ARM64 proved far fewer combinations of
     the `amounts` valid-input block than x86-64 (36 against about 10,000 on the runners). Not
     investigated: the runner's CPU, the model's speed, or the twice-as-many paths.
3. **CI coverage.** GitHub's x86-64 runners are a mixed pool: of 21 runs on 2026-11-04, 10 drew
   a CPU with AVX-512 (Xeon Platinum 8573C and 6975P-C, EPYC 9V45, some EPYC 9V74) and 11 did not
   (EPYC 7763, other EPYC 9V74), so x86-64 coverage was a coin flip per run. On a draw without
   it, eight x86-64 programs that used AVX-512 (`count`, `words`, `padded`, the fast `wc`,
   `crc32c`, three benchmark ones) were neither taken through the server nor rule-checked. Since
   the baseline forms (below) no example or fixture needs AVX-512: every one goes through the
   server and the rules on any draw (`every_x86_64_program_passes_its_checks` on an EPYC 7763
   without AVX-512, nothing skipped: run 41894634835). What still needs the CPU is the AVX-512
   tiers' own tests (`wc-avx512`, `count`'s benchmark variant, the benchmark-only programs), which
   note their skip in the run summary, and the model-against-CPU vector harness's probes, so its
   SSE2, AVX2, AES, SHA and PCLMUL probes meet real silicon only on the draws that run it. The
   x86-64 job also has no QEMU, so about 34 ARM64-under-QEMU tests are silent no-ops there. The
   plan is in "Fitting GitHub's free runners" below.
4. **Programs.**
   - Buffer sizes differ between twins: the fast `crc32c` 1 MiB against 64 KiB, `upcase` retries
     writes 4096 against 64 times (`count` and `padded` are 64 KiB on both since 2026-11-04).
   - The ARM64 main template's assertions skip number 3.
   - **x86-64's fast `wc`, `count`, `words` and `padded` had only an AVX-512 form**, where ARM64
     has the baseline NEON form of each, and an SVE one as an extra. The owner's reason for
     asking (2026-11-04): other targets, whatever their CPU and OS, must behave the same, so a
     spec's behavior can't depend on an optional CPU extension. The rule this implies: every
     program lengua ships has a form that runs on every machine of its target (x86-64's SSE2,
     ARM64's NEON); an optional extension (AVX-512, SVE) is a speedup an agent picks from the
     machine line's features, never the only way to get the behavior. Done for these programs on
     2026-11-04, and what remains is in "Baseline forms for the AVX-512 programs" below.
   - The AArch64 dialect has no add-with-carry (`adc`, `sbc`).
5. **Small items.** `cbz`/`tbz` on `xzr` or `wzr` count as conditional branches, so the dead-code
   and "assertion always fails" checks can be bypassed on ARM64; `ppoll`'s timeout and signal-mask
   pointers are missing from the static system call summary; the AArch64 vector model ends some
   matches with a wildcard that would run an unmodelled op as zeros, where x86-64's says "doesn't
   model yet"; AArch64 refusal texts say "no floating point on vectors" and keep x86-64 wording in
   a few layout messages; an emulated session always gets AArch64's CPU features; the
   model-against-CPU harnesses differ in feature gating and host; the ARM64 benchmark workloads
   omit programs x86-64 measures and say AArch64 has no threads. Documents: DESIGN lists the
   x86-64 instructions the prover models and its CPU harness's probe counts but has no AArch64
   list; `test-format.md` explains the float functions by x86-64 instructions alone; `SESSIONS.md`
   ran the ARM64 suite under QEMU, without the kernel's filter, and its thread and terminal rows
   prove less than x86-64's.

Beyond these two CPUs, the macOS and Windows ports remain skipped (below).

### Baseline forms for the AVX-512 programs: done on x86-64 (2026-11-04), and what is left

Why (the owner, 2026-11-04): each target's code is tailored to its CPU and OS to be fast, yet it is
one product, so every program has a baseline form that runs on every machine of its target and an
optional faster form picked from the machine line's features, both meeting the same spec, tests
and proofs. ARM64 had that (NEON, and SVE as an extra); x86-64 now has it too. Done on GitHub's
runners only (nothing built or run on the user's computer); the run ids are the evidence.
- **The tests no longer depend on which CPU the pool hands out.** The prover tests map x86-64
  programs with every extension, as the ARM64 job does (a proof is math); the all-programs test
  leaves out only a program that needs AVX-512 and notes it (`lengua::skiplog`); the AVX-512
  tier's own tests (`wc-avx512`, the word count's benchmark variant) run where the CPU has it and
  note the skip where it hasn't. `every_x86_64_program_passes_its_checks` passed on an EPYC 7763
  with **no AVX-512**, nothing skipped (run 41894634835, 85 s), and on three draws that have it
  (41893011594, 41894624314, 41894629698).
- **`assets/fast/wc` is the SSE2 form,** 16 bytes a step in the shape of ARM64's NEON `wc`:
  `pcmpeqb`, `psubb`/`psubusb` for the 9 to 13 range, `pmovmskb`, `popcnt`, and a mask for the
  last partial step built with `movd`, `pshufb`, `pminub` and `pcmpeqb`; `pshufb` and `popcnt` are
  its only instructions past SSE2, and it uses no BMI and no `ymm`. `count_piece` is exactly rule
  4's 60 instructions. The AVX-512 form moved to `assets/fast/wc-avx512` with the same spec bytes.
  `proves_the_sse2_fast_word_count_for_every_length` proves it for every length by induction at 64
  KiB with no solver question, and catches four planted bugs (the carry across pieces, the carry
  across steps, a word start past the end, the larger of the bytes left and a step taken for the
  smaller): the main verdict took 231 s on an EPYC 9V74 **without** AVX-512 (run 41889485548) and
  the whole test 152 s on an EPYC 9V45 (run 41895524569).
- **What the prover reads.** The first mask, `pcmpgtb` of the bytes left against the lanes'
  places, a signed compare, was never proven: the length's conditions come out as a signed
  comparison, `Pool::holds_where`/`reading` follow only unsigned ones, so the base of the
  induction went one length at a time with counts, a part for each byte of the buffer, for the
  solver (run 41874848019, gave up at 5829 bytes). The unsigned form is proven at once (a
  256-byte buffer: one shape, 19 paths, 276 ms; run 41889461023). A prover that also read signed
  compares of byte lanes would let more programs mask the natural way. Bugs that show only past a
  buffer-full (a newline counted from the stale bytes beyond the last piece) were not found in a
  minute at 256 bytes or more, and a word start past the end, which a 3-byte text shows, took 4.5
  s at 256, 53 s at 1 KiB and was not found at 4 KiB (run 41892999427): the induction's questions
  of hundreds of parts time out and leave no time for the search by length. So the planted-bug
  test uses a 256-byte buffer and a word start for its past-the-end bug.
- **`words`, `padded`, `count` and `crc32c`.** `tests/fixtures/words`, `tests/fixtures/padded` and
  `examples/count` use the same `count_piece` (count's buffer is now 64 KiB like its ARM64 twin);
  `words` is proven for every length by induction (4096-byte buffer, no solver question), `count`
  for every input up to 3,190 bytes in four minutes on a 4-vCPU runner (run 41895528826), and the
  60 seconds of a `check` on a runner prove none of the ASCII-text blocks of `count` or `padded`
  (run 41893005309), which go on each time the agent asks to prove on. The AVX-512 originals are
  `benches/programs/count/avx512.s` (cited in the README) and `wc-avx512`. `crc32c`'s only
  AVX-512 use, `move_front`, copies 8 bytes at a time and then bytes; its proof and checks pass
  (proven up to 2221 bytes). That three-chain `pclmulqdq` form is now the fast tier
  `fast/crc32c-pclmul` (byte for byte the file this paragraph measured); the shipped `fast/crc32c`
  is one SSE4.2 `crc32` chain, from the crypto work below.
- **Measured on the runners** (`cargo bench --bench against_rust_and_c`, 64 MiB through a pipe,
  the median of 41 runs, CPU time and wall time in ms; the runners expose no cycle counters, so
  user cycles can't be told from the kernel's copy). Baseline SSE2 `wc` against the best of C,
  C++ and Rust; the draw is the CPU the pool handed out, six dispatches:

  | Run | CPU | AVX-512 | lengua `wc` | best other | AVX-512 `wc` |
  |---|---|---|---|---|---|
  | 41893737162 | EPYC 9V74 | yes | 29.9 / 31.2 | 29.9 (Rust) / 31.9 (C): 1.00× / 1.02× | 25.3 / 32.6 |
  | 41893742012 | EPYC 9V74 | yes | 26.6 / 26.9 | 27.9 (C) / 28.3: 1.05× / 1.05× | 23.4 / 28.7 |
  | 41893747152 | EPYC 9V74 | no | 33.3 / 33.8 | 35.8 (C) / 36.3: 1.08× / 1.07× | – |
  | 41895807100 | Xeon 6975P-C | yes | 27.0 / 27.8 | 37.0 (C) / 37.4: 1.37× / 1.35× | 35.7 / 53.6 |
  | 41895812441 | EPYC 7763 | no | 31.3 / 31.8 | 48.6 (C) / 49.0: 1.56× / 1.54× | – |
  | 41895817808 | Xeon 8573C | yes | 35.3 / 37.2 | 47.9 (C) / 48.5: 1.36× / 1.30× | 23.1 / 43.5 |

  The baseline form is level with or ahead of C and Rust on every draw, on both CPUs without
  AVX-512 (1.08× and 1.56× less CPU time), so **the AVX2 tier was not built**: the condition for
  it, an SSE2 `wc` that does not beat C and Rust on a CPU without AVX-512, did not occur. The
  AVX-512 tier saves CPU time on three of the four AVX-512 draws (12 to 35 percent), never wall
  time, and on the Xeon 6975P-C it is slower than SSE2 on both: this workload is a pipe's, and
  the kernel's copy sets the pace. `examples/count`'s SSE2 form took 25.2 to 36.3 ms of CPU time,
  1.04× to 1.47× less than the best other on the same six draws. `crc32c` (the three-chain form,
  now `crc32c-pclmul`, with the scalar `move_front`) took 16.3 to 28.3 ms, 0.96× to 2.09× less CPU time than the best other (one draw,
  the Xeon 6975P-C, level to slightly behind C++), but its wall time was 8 to 29 percent above the
  best other's on all six; an A/B of the old AVX-512 copy (run column `lengua, AVX-512 copy`) has
  the same wall-time gap, so the scalar copy is not the cause and **the cause is not known**
  (the AVX-512 copy saves 2.7 and 4.0 ms of CPU time on the two EPYC 9V74 draws, 0.5 ms on the
  Xeon 8573C and costs 1.9 ms on the Xeon 6975P-C, and its wall time is worse on all four, by
  0.4 to 2.3 ms).
- **Left, for another engineer.** (a) `tests/fixtures/lines` uses `bzhi` (BMI2) and has no
  baseline form yet (the baseline feature set per target below); `aes`, `sha256` and `crc32c` have
  theirs from the crypto work. (b) `crc32c`'s wall-time gap above. (c)
  `move_front` could copy 16 bytes a step with `movdqu` (half the iterations; the scalar copy is a
  few percent of the CPU time). (d) A signed-compare reading in the prover, above. (e) The
  one-folder check's 60 s proof budget cannot finish the 64 KiB proof of `fast/wc` (231 s on a
  runner), so its `check` says "tested, not proven" until asked to prove on.

### Fitting GitHub's free runners

What the runners are (read from the logs of the runs of 2026-11-04): 4 vCPUs, kernel 7.0 (azure),
Landlock and yama in the LSM list, `ptrace_scope` 1, which is all the sandbox needs; the x86-64
CPU varies per run (about half have AVX-512), the ARM64 one has SVE (its SVE proof runs). The
sandbox is not an obstacle, and no step of setup has failed in about 160 jobs. What hurts:
- The full suites take 32 to 35 minutes, about 18 to 19 of them the 141 prover tests, and a push cancels
  the last one (15 of 33 x86-64 push runs, 13 of 33 ARM64 ones).
- The macOS and Windows builds always fail (the Linux-only sandbox crates), cost 1.5 to 6.7
  minutes each from a cold cache, add four red jobs to every push, and twice never got a runner:
  the platform cancelled them at 15 minutes and the run sat "queued" while its tests had finished.
- Skips are silent: a green log doesn't say that AVX-512 tests, the QEMU tests or a counter test
  were skipped. There is no branch protection, so nothing enforces a gate.
- Plan, in order (code = Rust or assembly, workflow = YAML only): (1) *code, tiny*: the
  all-programs test skips by feature, before calling the server, and the rules test checks the
  AVX-512 programs with every extension on any host, as the ARM64 job does; (2) *workflow*: print
  the machine's facts and a summary of what was skipped into `$GITHUB_STEP_SUMMARY`, and a hygiene
  bundle (Node 24 majors of the actions, `cancel-in-progress` off main, the dispatch group key
  including `filter`, the cache saved even when a job fails, no duplicate `build` jobs on a
  dispatch); (3) *workflow, the owner's call*: macOS and Windows out of the push path, to a weekly
  and manual workflow, losing the per-push red marks; (4) *workflow, one dry run first*: a QEMU job
  on x86-64, which un-skips about 34 tests and the ARM half of the mutation test; (5) *workflow and
  script*: shard each suite (3 or 4 shards of prover tests, split by hash; estimated 34 to 39
  minutes down to 12 to 15), not with `cargo-nextest`, whose one process per test breaks the
  process-wide proving lock; (6) *code*: a baseline variant of the model-against-CPU vector test
  that saves `xmm`/`ymm` only, so its SSE2, AVX2, AES, SHA and PCLMUL probes meet real silicon on
  every draw; (7) *code*: the baseline forms above, the only change that ends the dependence on
  which CPU the pool hands out (done for the examples and fixtures on 2026-11-04).
- Not recommended: lowering the proof budgets (calibrated on 16 cores; tests assert verdicts),
  larger or self-hosted runners (outside the $0 rule), dropping the duplicate prover tests (both
  solver builds get exercised), and emulating AVX-512: QEMU's TCG has none (checked in its source),
  and Intel's SDE could only wrap the unsandboxed harness, its licence unread.
- For the owner to decide: whether AVX-512 silicon coverage must stay a hard gate until the
  baseline forms land (an "attempts" job that retries until a draw has the flag would do, about 94%
  likely to hit); whether to read SDE's licence; a single `gate` job as a required check; and how
  many shards fit the Free plan's 20 concurrent jobs.

### Keeping five or six targets the same product

The owner's worry (2026-11-04): with five or six targets whose hardware and OS differ, how is it
known that a gap like the AVX-512 one doesn't happen again on another target? Today it is known
only by audit. The guards cover that a twin exists and that its spec is the same bytes (29 twins),
not that its tests, red team, limits and programs exist and hold on every machine of the target.
Four of the six targets are missing, so these are built and run for the two Linux targets now, and
each later port adds one column. Make each guarantee a failing test:
1. **One suite for all targets.** Express each test case once, with a column of snippets per
   target (its instructions), and run it for every target; a missing cell fails the build. Most of
   the audit's findings were two copies drifting apart (`adversarial.rs` against
   `adversarial_aarch64.rs`; 97 integration tests exist for x86-64 only). Adding a target then
   means adding a column, not copying files.
2. **A baseline feature set per target, enforced.** Define each target's baseline (x86-64: SSE2 and
   what the owner decides to add; ARM64: ARMv8.0 with NEON; Apple's and Qualcomm's ARM64 chips: what
   they all have) and rule-check every shipped example, fixture and asset against it; a program
   that needs more must be a named fast tier beside a baseline sibling with the same spec. The
   AVX-512 case would have failed the day it was added. It also flagged programs that broke it:
   `examples/sha256` and `examples/aes` needed SHA-NI and AES-NI on x86-64 and the ARMv8 crypto
   extensions on ARM64, which older Intel chips and some ARM64 boards lack, and `crc32c` needed
   SSE4.2 and PCLMUL (and the CRC extension on ARM64). *Built for the two Linux targets:*
   `CpuFeatures::baseline(profile)` (`src/platform.rs`) is the table, exhaustive over `Profile`,
   and `tests/baseline.rs` holds every shipped `main.s` of every target in `Profile::ALL` to it,
   on either CPU's runner; what it found is "Baseline debt" below. Done for these three programs on
   both CPUs: each has a baseline form on its target's baseline (plain integer code; SSE4.2's
   `crc32` on x86-64; a table on ARM64), and the forms that use the extensions are the fast tiers
   `fast/sha256-ni`, `fast/aes-ni`, `fast/crc32c-pclmul` and, on ARM64,
   `linux-aarch64/fast/sha256-sha2`, `aes-aes` and `crc32c-crc`, each with a spec.md of the
   baseline's bytes (`tests/crypto_forms.rs` holds the spec, the baseline and the tiers to
   account).
3. **The porting list as an executable conformance suite.** DESIGN's "Porting a target" becomes
   the set of tests every target must pass natively: the rules, the red team, the mutation test,
   the sandbox limits, the session behaviors, every example and fixture through the server.
   GitHub has a free native runner for each of the six targets, so each is checked on its own
   hardware, and a target isn't done until its suite is green there.
4. **Limits tested on each OS.** "One target's limit is every target's" holds only if tests of the
   limits (names, lengths, open files, listing order, ports) run on each OS; today they are pinned
   on x86-64 only.
5. **Visible coverage: done (2026-11-04).** Every skip (a missing CPU feature, no QEMU, no
   counters, no solvers) is a `lengua::skiplog::note` (`src/skiplog.rs`), and both workflows print
   the counts in the run's summary (`scripts/skip-summary.sh`) with the machine's CPU features, so
   a green run says what it didn't cover. The first real run showed it: on the x86-64 runner, 29
   ARM64-under-QEMU tests and 9 AVX-512 tests had been silent no-ops. Also done with it: the
   rules test checks an AVX-512 program for every extension on any CPU, the all-programs test
   skips by CPU feature (noted), `PerTarget<T>` and `Profile::ALL` (`src/targets.rs`) exist so a
   new target breaks every table until its cell is filled, a `wip/**` branch gets no push run, a
   dispatch doesn't build the matrix, and macOS and Windows build in `unported.yml` (weekly and on
   request).

**Where items 1, 3, 4 and 5 stand (2026-11-04).** Built for the two Linux targets, and run
natively on both runners:
- *1, one suite for all targets: begun.* Each of these is a table with a `PerTarget` cell for every
  target (`src/targets.rs`), so a new target breaks the build until its cells exist, and an
  attempt that a CPU can't write says why: the red team (`tests/red_team.rs`, which replaced
  `adversarial.rs` and `adversarial_aarch64.rs`: their 41 attempts each, as 42 rows, with
  x86-64's `openat` added); the planted memory bugs (`PLANTED` in `src/checker/tests.rs`: 19
  bugs, check-only, so every runner checks both CPUs' cells; x86-64 gained the return address
  written at a function's start and through a pointer, ARM64 the frame lent to a function that
  only reads it); every program of each target through the server (`tests/every_program.rs`,
  where a program for an extension the CPU lacks is skipped with a note); and the sandbox's
  limits (`tests/limits.rs`, 31 tests, each program an assembly source per CPU, built for both
  CPUs on every runner and run on its own). Left: the four test files still made for one CPU,
  which `tests/parity_inventory.rs` lists in `DEBT` with what each lacks on the other CPU:
  `check_code.rs` and `e2e.rs` (56 x86-64 session tests) and `aarch64_session.rs` and
  `aarch64.rs` (21 ARM64 ones). Making them tables means writing ARM64 session cases for about 40
  x86-64 ones (and x86-64 ones for `aarch64.rs`'s three standalone tests), program by program.
- *3, the porting list as an executable conformance suite: begun.* The rules, the red team, every
  program through the server and the sandbox's limits are tables both runners run natively.
  `parity_inventory.rs` fails when a target has no job that runs `cargo test` on a runner of its
  own CPU, and when a new test file is made for one target; DESIGN's "Porting a target" item 7
  says which suites a new target must pass. Left: the mutation test (ignored, by dispatch) and
  the session behaviors, which are the `DEBT` files above.
- *4, limits tested on each OS: done for the two Linux targets.* `tests/limits.rs` pins names
  Windows refuses (`EINVAL`), names that aren't UTF-8 (`EILSEQ`), a name or a path too long
  (`ENAMETOOLONG`), a name that differs only in case (`EEXIST`), a file held open (`EBUSY`), the
  4,352 open files, the listing turned round on a second run, the time, output, environment and
  signal limits, the spec's approval of the clock and random bytes, the clone-flag pin, a
  network no program holds, and the supervisor's failure paths and threads. A later port adds a
  column; what only that OS can keep (a name its own file system refuses) is for its runner.
- *5, visible coverage: done* (`lengua::skiplog` and the run's summary). Every skip of the new
  tests notes its reason, and the log no longer mixes the notes of tests that skip at once (a
  note is one write).

### Baseline debt

What `tests/baseline.rs` found on 2026-11-04, when it first held every shipped `main.s` to its
target's baseline (x86-64: SSE2 with SSE3, SSSE3, SSE4.1, SSE4.2 and POPCNT; ARM64: ARMv8.0-A with
NEON and floating point, nothing optional): 8 of x86-64's 39 shipped programs and 3 of ARM64's 40
needed more. The `DEBT` list in the test is what is still owed, with the extensions each program
needs exactly as the checker names them: the test fails for a program that needs more and isn't
listed, for an entry that names other extensions, and for a listed program that now runs on the
baseline, so the list only shrinks. Whoever writes a baseline sibling, from the very same
`spec.md`, removes the entry; where the extension's form stays as a fast tier beside it, the entry
moves to `TIERS`, which the test holds to a sibling that exists, needs nothing beyond the baseline
and shares the spec's bytes.

**Paid off (2026-11-04), on both CPUs:**
- `examples/aes`, `examples/sha256` and the fast `crc32c` on x86-64, and the ARM64 `aes`, `sha256`
  and fast `crc32c` twins: each now runs on its target's baseline (plain integer code; a table of
  256 words for AES and for ARM64's CRC-32C; SSE4.2's `crc32` on x86-64), and the forms that use
  the extensions are the tiers `fast/aes-ni`, `fast/sha256-ni`, `fast/crc32c-pclmul` and, on
  ARM64, `fast/aes-aes`, `fast/sha256-sha2` and `fast/crc32c-crc`, each with the baseline's
  `spec.md` bytes.
- The AVX-512 programs on x86-64: the fast `wc` (SSE2, with the AVX-512 form as the tier
  `fast/wc-avx512`), `examples/count` (its AVX-512 original is the benchmark-only
  `benches/programs/count/avx512.s`) and the fixtures `words` and `padded`.

**Still owed, x86-64, 1 of its shipped programs:**
- `tests/fixtures/lines`: BMI2 (`bzhi`). Sibling: the same loop with its last chunk masked as the
  fast `wc` masks it (`movd`, `pshufb`, `pminub`, `pcmpeqb`), in place of `bzhi`. Several prover
  tests are written against this fixture's `bzhi` (the line counter at 64 KiB proven "with no
  solver question" because the prover reads `bzhi`'s mask as `position < length`), so the change
  comes with their expectations.

ARM64 owes nothing.

Tiers beside a written baseline sibling (`TIERS`): the six above, x86-64's `fast/wc-avx512`
(AVX-512BW, BMI2) and ARM64's `fast/wc-sve` (`sve`) beside their `fast/wc`, and, benchmark-only
and listed apart (`BENCH_ONLY`), x86-64's `benches/programs`
`count/avx512.s`, `shout/avx512.s`, `shout/avx512-one.s` and `upcase/avx512.s` (AVX-512BW) beside
`examples/count`, the parallel template and `examples/upcase`. ARM64's two benchmark variants are
NEON, on the baseline.

Not done by the test: the stamp lengua writes into `main.s` and `lengua.lock` (`needs SSE4.2, …`)
still lists SSE3 to SSE4.2 and POPCNT as extensions, since the checker's own floor is SSE2
(`Facts.features`); leaving the baseline out of the stamp is a change to the checker's facts and
their goldens.

## Skipped, and why

- **The other four targets are missing: macOS and Windows, each on x86-64 and on ARM64.** Each OS is a port
  of its own, not started in this run: neither lets a program call the kernel directly, as
  lengua's programs do, and neither has seccomp or Landlock, so each needs a new model of the
  program's edge with the OS (libSystem, ntdll) and a new sandbox. Their CPUs are done: x86-64's
  and AArch64's decoders and prover models carry over. The six-runner workflow
  (`.github/workflows/targets.yml`) builds lengua natively on all six of GitHub's runners, free
  since the repository went public (2026-11-02), and tests it where it runs: on macOS and
  Windows the build stops at the Linux-only sandbox, which is where each port starts. DESIGN's "Porting a target" lists what
  each target must supply; every limit the Linux targets hold is already theirs (DESIGN's "One
  product on every target"), so a port adds means, never a different lengua. The plugin's
  server, a native binary, is the same work for each host.
- **A connection for each client a program serves**, as a proxy needs, or several to one place.
  It needs connections made on demand: a way for a program to ask lengua for one that every
  OS's sandbox can carry, such as a descriptor handed over on request, so a service of its own,
  which the plan for this run left out. Today a program gets the connections its spec names,
  made before it starts.
- **Launcher services for compression and a GUI.** Each would be one definition every OS must
  be able to provide, outside the program, as lengua does TLS; a GUI is a product of its own.
  Not designed yet: a decision for the owner.
- **Proofs past x86-64's finish line**, which the user set on 2026-11-01 for each target ("a set
  finish line ... a general purpose programming language replacement"). These programs are
  checked, tested and proven up to a length, and each proof line says so: sorting at every
  length and word tallies past a few bytes, which need quantified facts over memory; byte loops
  at 64 KiB, whose counts keep a copy of every part at every byte (counts that share their parts
  would fix it); a routine for the last partial vector at a place the length chooses, and raw
  `read`s of any size. Research, past the line.
- **The public release.** The repository is public since 2026-11-02, a work in progress, at the
  user's word. `scripts/release.sh` builds the archive for either Linux CPU, and it installs and
  runs locally, under the Apache License 2.0 (LICENSE); publishing it as a release, listing the
  plugin where agent plugins are found, and deploying the site wait for the owner.
