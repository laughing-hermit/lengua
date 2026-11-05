Programs Claude Code wrote through lengua in earlier interactive sessions, rebuilt from those
sessions' transcripts, which the prover's tests prove:

- `total`: the total of a CSV file's amounts, whose valid-input block has 24,696 input
  combinations;
- `clock`: the time of day from the clock, in hours, minutes and seconds, with six divisions;
- `days` (interactive round 2): that clock plus the days from today to a date, Hinnant's
  days_from_civil in the program and in the tests' formula, written in different orders;
- `amounts` (interactive round 2): the total of a CSV file's amount column, three rows of
  signed amounts with free text in the other fields;
- `dates` (interactive round 3): the clock again, its day count's tests in one block for valid
  and invalid dates, which prints `if(valid, days, "")` and the error otherwise.

And written for the tests themselves:

- `cat`: standard input of up to 5,000 bytes printed back, whose proof covers texts up to the
  4,096 bytes a proof reaches.
- `padded`: a `wc -lw` from interactive round 2, which pads its counts to seven columns with a
  chain of `if(…)`s choosing among texts of different lengths. It counts 16 bytes a step with
  SSE2, the baseline form that runs on every x86-64 computer, as the fast `wc` does.
- `lines`: `wc -l` over standard input of any length, a buffer-full at a time through `fill`,
  its count kept in a register: proven for every length by induction, the count growing by each
  buffer-full's newlines.
- `words`: `wc -lw` over standard input of any length, a buffer-full at a time through `fill`,
  16 bytes a step with SSE2 masks (the baseline form every x86-64 computer runs; the AVX-512
  tier is `plugin/skills/compile/assets/fast/wc-avx512`), whether the last byte read was
  whitespace kept in r14: proven for every length by induction, each buffer-full's words counted
  after the last byte of the one before.
- `calc`: a whole number, an operator and another, its `switch` on the operator's character a
  checked jump table (dialect 4) over `*`, `+`, `,` and `-`, the comma its unknown case.
- `cat-stream`: standard input of any length copied back a piece at a time, so input larger than
  memory works.
- `pairs`: the pairs of `aa` in standard input of any length, a piece at a time, each `a` in at
  most one pair, taken from the left.
- `nap`: half a minute's wait on the clock that only moves forward, then `awake`.
- `sweep`: the empty files of the working folder deleted, and how many said.
- `second`: two lines from standard input, read with one `fill` to the first newline a read
  ends with, so it's right only when both lines arrive together, as they do in the tests: the
  proof, which takes every way standard input can be cut, finds the cut after the first line.

**Every one has its twin on the other CPU.** `linux-aarch64/` holds the AArch64 twin of each
fixture here and of each program in `examples/`, 29 in all, each compiled from the very same
`spec.md` (the twin test, `every_program_has_its_twin_on_the_other_cpu`, asks for the same bytes),
and a program added on either CPU comes with its twin. One test,
`every_program_passes_its_checks_on_every_target` (tests/every_program.rs), takes them all
through the server on each target a computer runs, and `tests/programs.rs` takes the ones a case
asks about further, each case written once with a cell for each CPU (the twin's folder, an edit
of its text, the extension a variant needs): decimals, files, folders, sleep, the environment,
dice, programs built on programs, words and pairs proven for every length, the vector, streaming,
tally and parallel templates, and the benchmark's programs.

**The session tests start from them.** `tests/sessions.rs` and `tests/workflow.rs` (each case once,
a cell for each target, run natively on each CPU's runner) start from the template and from the
twins of `add`, `cat`, `cat-stream` and `echo`: `cat` is proven up to its 4,096-byte bound and has
its proof interrupted by an edit, `cat-stream` is stamped, run on a long input and proven for every
length, and `echo` is served from a shell until it is stopped. A fixture they use is needed on
both CPUs, with the very same `spec.md`.

**Some are also the subject of a case written once for both CPUs.** `tests/network.rs` makes one
change to a program of each CPU and holds the report to it: `second` read with a read of its own
instead of `fill`, or with a spec whose first line the tests' cuts never end a read after (the
proof's cut finds it); the `echo` and `fetch` examples served, fetched from, proven, and edited to
wait for the client to close or to stop after one connection; and `add` with the exit code of a
failed write changed. Each case's edit is a cell for each CPU in the test, checked against that
CPU's program on every runner, so a twin that drifts from its sibling's text fails the build.

**`sha256` and `aes` are baseline forms.** The twins `linux-aarch64/sha256` and `linux-aarch64/aes`,
like `examples/sha256` and `examples/aes`, are plain integer code (rotates, adds and xors, a table
of 256 words for AES) that runs on every computer of its CPU; the forms that use the CPU's SHA and
AES instructions are fast tiers in the compile skill's assets (`fast/sha256-ni` and `fast/aes-ni`;
on ARM64 `linux-aarch64/fast/sha256-sha2` and `fast/aes-aes`), each beside the very same
`spec.md`. `tests/crypto_forms.rs` holds that to account: the same spec bytes in every form, a
baseline that uses nothing beyond its CPU's baseline, and the tiers through the server where the
CPU has what they use.
