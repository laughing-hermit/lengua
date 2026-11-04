# lengua's agenda: what's left for people

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

1. **lengua's own SHA-256 example: done.** `examples/sha256/` (spec.md and main.s) prints the
   SHA-256 digest of standard input with the CPU's `sha256rnds2`, `sha256msg1` and `sha256msg2`;
   the spec has FIPS 180-4's vectors (abc, the empty text, the 448-bit message) and generated
   blocks whose output is `{sha256(text)}`, with the block-boundary lengths (0, 50 to 70, 118 to
   134, 4090 to 4102) stated and tested. Its twin `tests/fixtures/linux-aarch64/sha256/` has the
   same spec bytes and uses `sha256h`, `sha256h2`, `sha256su0`, `sha256su1`. Both reach `done`
   (x86-64: 1204 test runs). Proof: "tested, not proven", as expected; `palignr`, which the
   schedule uses on x86-64, isn't modelled by the prover either, so a proof isn't near.
2. **lengua's own AES example: done.** `examples/aes/` encrypts hex data (32 to 131072 digits) with
   a 128-bit key, block by block (ECB), with `aesenc`, `aesenclast` and the key schedule from
   `aesenclast` and rotated words; bad input exits 3 with "aes: bad input". The test language got
   `aes128(key, data)` (`src/aes.rs`, a software AES with FIPS 197 and SP 800-38A unit tests;
   `src/expr.rs`; documented in the write-spec skill's test-format.md), so the spec's generated
   blocks test every length against it. The twin `tests/fixtures/linux-aarch64/aes/` uses `aese`,
   `aesmc`, `tbl`, `ext`. Both `done` with 1406 test runs; proof "tested, not proven". ECB is the
   mode the spec states because the spec is a function of the data alone; it shows the
   instructions, it isn't a product for hiding data.
3. **Replies stopped on 2026-11-01**: nothing to do; no work was lost.
4. **The hardening pass's red-team part: done, defensively.** `tests/adversarial.rs` (x86-64) and
   `tests/adversarial_aarch64.rs` (ARM64, on ARM64 hardware) take the template and make one change
   each that tries to get past lengua; every attempt must be refused or never stamped `done`:
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
   `amounts/`, each spec copied unchanged; `UNPORTED` is gone from `tests/aarch64_session.rs`, so
   `every_program_has_its_twin_on_the_other_cpu` and `every_aarch64_twin_passes_its_checks` now
   cover every fixture. Both reach `done` on ARM64 hardware.

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
  example and fixture through the server, as `every_aarch64_twin_passes_its_checks` does for
  ARM64. Both CPUs now give `total` the same 426 test runs and the same proof lines.
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

**Found by the parity audit, still open.** Nothing below is a known wrong answer; each is a place
where one CPU has less than the other. By the audit's severity, then checked:
- *High.* ARM64 now has planted-bug fixtures for the memory rules M1 to M6 and M8
  (`aarch64_memory_rules_find_each_planted_bug`, 14 cases, and a stack-size case), but not the rest
  of what x86-64 has in `src/checker/tests.rs`: one address lent twice for writing
  (`one_address_is_never_lent_twice_for_writing`, M4), `the_stack_is_tracked_exactly`, the thread
  rules (M7), and the Power of 10 rules one by one (about half as many fixtures); each is a
  one-line edit of the ARM64 template (`with_aarch64`), and the saved-return-address gap above is
  what their absence cost. The prover is tested on 7 of the 29 ARM64 twins against about 20
  x86-64 programs, and its memory-bug tests are x86-only.
- *Medium.* ARM64's sandbox has about 8 native tests against x86-64's 25: nothing that a program
  holds no network, that connections hand over by their rules, or that a refused one is reported;
  the limits the owner made identical on every system (time, output, environment, signals, files,
  open files, names, rename, listing), the clone-flag pin, spec approval of the clock and random
  bytes, and the supervisor's failure paths and threads are pinned on x86-64 only. Session
  behaviors beyond check-to-done run only with x86-64 programs, and the unmodified AArch64
  template has its rules checked but goes through `check` to `done` in no test but the ignored
  mutation test. About half as many per-rule fixtures exist
  for AArch64. The checker's and the prover's golden guards record x86-64 programs only. Executor
  tests of loop detection, connections and sleep are x86-only. The x86-64 job installs no QEMU, so
  the ARM64-under-QEMU tests skip there without saying so. GitHub's x86-64 runners have no
  AVX-512, so the x86-64 programs that use it (`count`, `words`, `padded`, the fast `wc`) have
  their rules checked there but are not taken through the server, where their NEON twins run
  everywhere; a runner with AVX-512 (or a self-hosted one) would close that. The ARM64 benchmark workloads omit
  programs x86-64 measures, and say AArch64 has no threads.
- *Medium, the prover.* x86-64's dialect admits vector instructions its prover can't follow
  (`palignr`, `pblendw`, `punpck*`, `pack*`, saturating and widening arithmetic, string
  compares, `pextr*`/`pinsr*`, `pmovzx*`, `gf2p8*` and more: `src/checker/x86_64.rs` against
  `src/prove/exec/vector.rs`), where AArch64's prover models its whole dialect. The same spec
  therefore ends differently: x86-64's SHA-256 proof stops at `palignr` ("doesn't model yet") and
  ARM64's runs until its time is up ("ran out of time"). Both say "tested, not proven", the only
  result either can have, but for different reasons. It also follows pointer owners through more
  instructions on x86-64 (`cmov`, `and` to a boundary, scaled `lea`) than ARM64 does through
  `csel`, `and`-immediate and shifted `add` (`plan` against `plan_a64` in the executor); ARM64
  then holds those accesses to the weaker "object the value points into" rule.
- *Low.* `cbz`/`tbz` on `xzr` or `wzr` count as conditional branches, so the dead-code and
  "assertion always fails" checks can be bypassed on ARM64; `ppoll`'s timeout and signal-mask
  pointers are missing from the static system call summary; the AArch64 vector model ends some
  matches with a wildcard that would run an unmodelled op as zeros, where x86-64's says
  "doesn't model yet"; the AArch64 dialect has no add-with-carry; AArch64 refusal texts say "no
  floating point on vectors" and keep x86-64 wording in a few layout messages; an emulated session
  always gets AArch64's CPU features; the model-against-CPU harnesses differ in feature gating and
  host (x86-64's runs only on x86-64, ARM64's needs the crypto extensions to be there).
  Programs: twin programs differ in buffer sizes (`count` 256 KiB against 64 KiB, `padded` and the
  fast `crc32c` 1 MiB against 64 KiB, `upcase` retries writes 4096 against 64 times), the ARM64
  main template's assertions skip number 3, and x86-64 ships its fast `wc` for AVX-512 only where
  ARM64 has the baseline NEON form. Documents: DESIGN lists the x86-64 instructions the prover
  models and the probe counts of its CPU harness but has no AArch64 list or counts;
  `test-format.md` explains the float functions by x86-64 instructions alone; `SESSIONS.md` ran
  the ARM64 suite under QEMU, without the kernel's filter, and its thread and terminal rows prove
  less than x86-64's.

write or drop in a standard SHA-256 or AES program and lengua checks and tests it. For a value
computed from the input, checked, tested and proven, lengua's own program can be a plain checksum:
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
- lengua isn't to be presented as covering security because of them, in the white paper
  (WHITEPAPER.md), the site (`site/`) or anywhere else;
- they're not for a program that must resist an attacker, such as checking that a download wasn't
  tampered with, or keeping data secret;
- that still takes SHA-256 or AES, which a user can supply and lengua checks and tests (`examples/sha256`
  and `examples/aes` show the instructions, not a product that resists an attacker).

## Skipped, and why

- **The other four targets: macOS and Windows, each on x86-64 and on ARM64.** Each OS is a port
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
