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
   - **A real gap on ARM64, fixed**: a function could overwrite `x30`, its return address, and
     `check` caught that only when the tests crashed, where x86-64 refuses `mov [rsp], 0` before
     running (rule M3). Now a rule refuses any instruction that writes the link register between
     the frame record's save and restore (`src/checker/rules.rs`).
   - A pointer read from memory isn't followed to an owner by the rules (they follow what they
     can; the rest is "tested, not proven"); the sandbox kills the stray read and the program is
     never `done`. That is the design, and the test asserts exactly that, on x86-64.
   - Every other attempt is refused up front on both CPUs.
5. **The ARM64 twins of `total` and `amounts`: done.** `tests/fixtures/linux-aarch64/total/` and
   `amounts/`, each spec copied unchanged; `UNPORTED` is gone from `tests/aarch64_session.rs`, so
   `every_program_has_its_twin_on_the_other_cpu` and `every_aarch64_twin_passes_its_checks` now
   cover every fixture. Both reach `done` on ARM64 hardware.

**Still open from this run.** On ARM64 the one-folder check of the SHA-256 twin
(`check_one_aarch64_folder`) ran past the job's 90 minutes twice, while the same twin passes inside
the full suite's `every_aarch64_twin_passes_its_checks`; why the single-folder run stalls isn't
found (the proof time, 10 s per block, should end it). Worth an engineer's look.

**Not blocked, and safer alternatives.** The crypto support is built: a user can
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
