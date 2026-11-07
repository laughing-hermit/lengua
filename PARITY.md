# The parity ledger: the native programs and their WebAssembly twins

lengua's native product compiles an idea to `main.s`, assembly for Linux on x86-64 or ARM64. Its
WebAssembly product compiles an Attested Computation to `main.wat`, one module for every
computer. Each of the 29 programs the repository ships has a native folder for each CPU and,
since milestone W9 ([ROADMAP.md](ROADMAP.md)), a WebAssembly twin compiled from the same idea in
its new form. This ledger lists, for each program, every difference in what the program does or
in what lengua says of it, with the reason.

It was made on 2026-11-07, at commit b761dd3, from the two ideas and the programs of each pair,
the tests, the commit messages of the ports, and the logs of CI's runs. Where a verdict comes from
a run, the run is named; where it comes from the code or a test alone, that is said. The native
programs stay in the repository until the native product is retired (W11); the tag
`native-final` keeps them after that.

## Where the runs stand

- **The native programs.** `tests/every_program.rs` takes all 29, on each CPU, through the
  server: every one follows its rules and passes its tests but `second`, which is wrong by design
  and whose tests fail, as the test expects. It passed on both Linux runners (Linux on x86-64 and
  Linux on ARM64) in run 42361995155, as it did in the native suite at `native-final` (run
  42178888801). Native programs run on Linux only; on the other runners their tests end at once.
- **The WebAssembly twins.** `tests/wasm_every_program.rs` takes each folder of
  `tests/fixtures-wasm` from the user's request to done. In run 42361995155 (branch `wip/ports`
  at b229e01, on all five runners) 27 of the 29 went to done on every runner. `echo` and `greet`
  broke rules 5, 9 and 10 there: they were the prover's fixtures, written before the checker's
  rules. b761dd3 fixes both, and changes no other program; every other `main.wat` and every idea
  at b761dd3 is the same, byte for byte, as at b229e01. Run 42367496557, of b761dd3, was still
  running when this ledger was written.
- **The prover's model.** `tests/wasm_model.rs` runs every test of every twin in the model the
  WebAssembly prover proves with, beside the engine. In run 42361995155 the model did what the
  engine did on every test of 21 twins, and listed the runs it doesn't follow yet in the other 8
  (aes, count, hypotenuse, lines, padded, sha256, sort and upcase), the same on all five runners.
- **No run has recorded `check`'s proofs of the 26 ports yet.** At b229e01, `check` didn't prove
  main.wat; it does since 135dd58, but `tests/wasm_every_program.rs` gives its session no proof
  time, so its replies' proofs end at once. The WebAssembly proofs below come from the prover's own
  tests, from `tests/wasm_check.rs`, and from the code.

## What every pair differs in

| | Native | WebAssembly |
|---|---|---|
| The idea's type | `lengua/program-idea` | `Attested Computation`, with `runtime`, `computation`, `parameters` (where each input comes from), `executor` and `attester` |
| The code | `main.s` for each CPU: two programs, one in `examples/` or `tests/fixtures/` and its twin in `tests/fixtures/linux-aarch64/` | `main.wat`: one module for every computer |
| Where it runs | Linux on x86-64 and Linux on ARM64 | the five runners: Linux on x86-64 and on ARM64, Windows on x86-64 and on ARM64, and macOS on ARM64 |
| The rules | the native dialect 4 ([power-of-10.md](plugin/skills/compile/references/power-of-10.md)) | WebAssembly dialect 1 ([power-of-10-webassembly.md](plugin/skills/compile/references/power-of-10-webassembly.md)) |
| The calls | lengua's edge (`os_read`, `os_open`, …), through the broker and the Linux sandbox | WASI preview 1, as lengua's host implements it |
| Descriptors | the listener 3, the connections from 4 | the working folder 3, the listener 4, the connections from 5 |
| Names | 255 bytes a name and 1023 a path (`src/names.rs`), held by the broker | the same rules, held by the host |
| Arguments | read in place, where the system put them | copied into a region the program declares (below) |
| The read loops | at most 2^40 buffer-fulls (sha256: 2^28) | at most 2^31 − 1, as a loop's counter is a 32-bit local (below) |
| `check`'s `performance` | instructions, CPU cycles, CPU time and time on the clock | WebAssembly operations (the engine's fuel, the same on every computer) and time on the clock |
| `check`'s `memory` line | each access inside the object its pointer was made from, on every run a proof covers | the rules M1 to M8 for every address they follow, WebAssembly's own bounds for the rest, and what the rules leave to the proofs, each on its line |
| The record | lengua.lock, and lengua's line in main.s | lengua.lock in format 6, and lengua's entry in the idea's `verified` list at done |
| The proofs | by induction over a read loop first, then by length; the model follows vector code, floats, names the input chooses and servers' waits | by length only; the model doesn't follow SIMD128, floats, a name the input chooses or a server's wait on `poll_oneoff` yet |

### Arguments

A native program reads each argument where the system put it, of any length the system passes.
A twin copies the text of all its arguments into one region, with the program's own name,
`main.wasm`, and a NUL after each, and answers as below when that text doesn't fit:

| Twin | Its arguments' region | When the text doesn't fit |
|---|---|---|
| absdiff, calc, dates, days, hypotenuse, mean | 131072 bytes | the usage line; `dates` says the date is invalid |
| archive, total, upcase | 32768 bytes | the usage line; `total` says it can't open the file |
| fetch | 4096 bytes | the usage line |
| add, sort | 64 bytes | the usage line |

`count` reads only how many arguments it has, and `greet` reads only its environment (below). The
others don't read their arguments. Below its region, each twin answers as its idea says, as the
native one does. Past it, the answers can differ: for `absdiff`, `calc`, `hypotenuse` and `mean`,
arguments of more than 128 KiB in all get the usage line, where the native program says what is
wrong with the number. The reason is the dialect's: every address is a region's (rule 9 and M1),
so an argument is read in a region the program declares, sized from the idea's limits.

### Read loops

| Twin | A buffer-full | Native bound | WebAssembly bound | Past the WebAssembly bound |
|---|---|---|---|---|
| cat-stream, lines, words | 4096 bytes | 2^40 (4 PiB) | 2^31 − 1 (8 TiB) | assertion 2, exit code 134 |
| pairs | 256 bytes | 2^40 (256 TiB) | 2^31 − 1 (512 GiB) | assertion 2, exit code 134 |
| count, padded | 65536 bytes | 2^40 | 2^31 − 1 (128 TiB) | assertion 3 (count) or 2 (padded), exit code 134 |
| sha256 | 4096 bytes | 2^28 (1 TiB) | 2^31 − 1 (8 TiB) | assertion 2, exit code 134 |

Past its own bound, a native `cat-stream` exits as when a read fails, a native `lines` or `pairs`
prints what it counted, and the others fail an assertion. Every idea's tests stay within 1 MiB, far
below each bound. The reason is the dialect's rule 2: a loop's counter is an `i32` local, set from
a named constant, so a bound is at most 2147483647.

## Program by program

Each section names the native folder for x86-64; its ARM64 twin is the folder of the same name in
`tests/fixtures/linux-aarch64/`. "The same, but the frontmatter" means the idea's text below its
frontmatter is the native one's, byte for byte.

### absdiff

`examples/absdiff`, and `tests/fixtures-wasm/absdiff`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers; its arguments as the table above has them.
- **Proofs:** native: B1 to B4 proven for every input in range, and memory on every run they
  cover, on both CPUs (`tests/programs.rs`, `the_absolute_difference_example_is_proven`, passed on
  both Linux runners in run 42361995155). WebAssembly: the model follows every test (run
  42361995155); no run has recorded `check`'s proofs.

### add

`examples/add`, and `tests/fixtures-wasm/add`.

- **Idea:** smaller. Its Purpose is one sentence; B2's title is "Shows the usage line without
  arguments"; it has no B3 (text that isn't a number in range, `error: not a number`, exit code 2)
  and no B4 (exit code 3 with standard output closed).
- **Program:** text that isn't a number, a number of more than 19 digits, or one past
  9223372036854775807 either way, gets the usage line and exit code 1, where the native program
  prints `error: not a number` and exits with code 2. A number past the idea's range, up to
  9223372036854775807 either way, is parsed and added in 64 bits, where the native program
  refuses it as not a number. A sum from -9223372036854775807 to 9223372036854775807 is printed.
  A sum past what 64 bits hold wraps, and the twin prints a wrong number and exits with code 0
  (`9223372036854775807` and `9223372036854775807` print `-2`). A sum that comes to
  -9223372036854775808, wrapped or not, has no magnitude in 64 bits, so assertion 5 fails with
  exit code 134 (`9223372036854775807` and `1`). A closed standard output gives exit code 3, as
  natively, though no test of the twin tries it.
- **Why:** the twin came first, as the fixture of `check`'s path for main.wat in W6 (d5d37ae,
  a7da239), then as the WebAssembly prover's (d31ee1c). No commit says why its idea has two
  behaviors where the native one has four. Without B3, its main.wat checks only that each number
  fits in 64 bits, and its comment on the sum (line 92) holds only for the idea's numbers.
- **Proofs:** native: every pair of inputs (`src/prove/tests.rs`,
  `proves_add_for_every_pair_of_inputs`, ignored as slow; no run of it is named here). WebAssembly:
  B1 proven for numbers of up to three digits, by the prover (`src/prove/wasm/tests.rs`,
  `proves_greet_and_echo_and_add_to_three_digits`, all five runners in run 42361995155) and
  through `check`, with the lock keeping it (`tests/wasm_check.rs`,
  `add_s_proof_is_named_and_the_lock_keeps_it`, all five runners in run 42362037786 at c6508b4).
  A planted wrong constant and an unguarded division are found by the prover
  (`finds_a_planted_bug_in_each_fixture`, run 42361995155), and the constant through `check`
  (`a_planted_wrong_constant_is_a_counterexample_through_check`, run 42362037786). Every pair is
  the ignored `proves_add_for_every_pair_of_inputs` of `src/prove/wasm/tests.rs`.

### aes

`examples/aes`, and `tests/fixtures-wasm/aes`.

- **Idea:** the description and the Purpose. The native idea says the program uses the CPU's AES
  instructions and shows how lengua checks them, which only its fast tier,
  `plugin/skills/compile/assets/fast/aes-ni`, does from the same idea bytes. The twin's says it
  computes AES in portable code, looks up no table with a byte of the key or the data, and has no
  branch that depends on them. The behaviors and tests are the same.
- **Program:** the native baseline is integer code with a table of 256 words that the state's
  bytes index. The twin holds a block in one 128-bit vector, takes each byte's inverse in GF(2^8)
  as x to the 254th by masked shifts and xors, 16 bytes at once, then the affine map; ShiftRows and
  MixColumns' turns are shuffles with constant lanes, and the key expands through the same
  SubBytes. Hexadecimal digits are read with compares that give 1 or 0. The limits are the same.
- **What lengua says:** nothing of the constant time yet. The `- Secret:` line and its checks are
  W10's and aren't on this branch, so the twin's constant time is its code's design and its idea's
  words, not a verdict.
- **Proofs:** B1 to B5's tests compute `aes128`, which the prover doesn't model on either path
  ("the test function aes128, which the proof doesn't model yet", `src/prove/formula.rs`), so they
  are tested, not proven. WebAssembly: before that, the model stops at main.wat line 267,
  `I32x4Splat`, on the runs of B1 to B5 (run 42361995155); it follows B6's (bad input). No test
  pins B6's verdict on either path.

### amounts

`tests/fixtures/amounts`, and `tests/fixtures-wasm/amounts`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. Both read a byte at a time; the twin reads each byte through
  `$byte_at`, which checks its place.
- **Proofs:** native: every amount proven where the text fields are fixed, and B2 and B3 proven
  (`src/prove/tests.rs`, `proves_the_csv_total_for_every_amount`, both Linux runners in run
  42361995155); the blocks with free text in several fields up to a length
  (`proves_the_csv_totals_free_text_blocks`, ignored). WebAssembly: the model follows every test
  (run 42361995155); no run has recorded `check`'s proofs.

### archive

`examples/archive`, and `tests/fixtures-wasm/archive`.

- **Idea:** the same, but the frontmatter, whose receipt adds `files`.
- **Program:** the same answers. The twin looks for the file with `path_filestat_get` before
  `path_create_directory` makes the folder, so a name with no file makes nothing, as the native
  program's open does, and moves it with `path_rename`.
- **Proofs:** native: B1 proven for every input (`tests/programs.rs`,
  `a_program_makes_folders_and_moves_files_inside_its_own`), and a file name the input chooses
  proven (`src/prove/tests.rs`, `proves_a_file_name_the_input_chooses`), both Linux runners in run
  42361995155. WebAssembly: the model follows every test (run 42361995155), but it doesn't follow a
  name the input chooses yet (`src/prove/wasm/host.rs`), which B3 draws (`string(1, 64, "egno")`):
  by the code, B3's proof stops there.

### calc

`tests/fixtures/calc`, and `tests/fixtures-wasm/calc`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. The operator's case is a `br_table` whose default is an
  assertion, where the native program has a checked jump table; in both, `,` is in the table's
  range, as the unknown operator's case.
- **Proofs:** native: every case the jump table can take (`src/prove/tests.rs`,
  `proves_each_case_a_jump_table_can_take`, both Linux runners in run 42361995155). WebAssembly: the
  model follows every test (run 42361995155); no run has recorded `check`'s proofs.

### cat

`tests/fixtures/cat`, and `tests/fixtures-wasm/cat`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. The twin reads standard input with `$fill`, which reads the same
  however the input arrives; the native program reads it with a loop of `os_read` of its own.
- **Proofs:** native: proven up to 4096 of its 5000 bytes, as "standard input past 4096 bytes is
  more than one pipe write, which the proof reads whole; read it with fill to be proven for any
  length" (`tests/sessions.rs`, `long_text_is_proven_up_to_the_bound`, both Linux runners in run
  42361995155). WebAssembly: the model follows every test (run 42361995155); no run has recorded
  `check`'s proofs.

### cat-stream

`tests/fixtures/cat-stream`, and `tests/fixtures-wasm/cat-stream`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers, 4096 bytes at a time in both; the read loop's bound as above.
- **Proofs:** native: every length, by induction over its read loop, by the prover
  (`src/prove/tests.rs`, `proves_a_streaming_filter_for_every_length`) and through the server
  (`tests/sessions.rs`, `a_streaming_program_is_proven_for_every_length`), both Linux runners in
  run 42361995155. WebAssembly: by length only (`src/prove/mod.rs`, `attempt_wasm`), so at most up
  to a length; the model follows every test (run 42361995155).

### clock

`tests/fixtures/clock`, and `tests/fixtures-wasm/clock`.

- **Idea:** the same, but the frontmatter. Its clock reads to `4102444799`, inside WASI's range.
- **Program:** the same answers; the twin reads the clock in nanoseconds and keeps whole seconds.
- **Proofs:** native: proven (`src/prove/tests.rs`, `proves_the_hh_mm_ss_clock`, both Linux
  runners in run 42361995155). WebAssembly: the model follows every test (run 42361995155); no run
  has recorded `check`'s proofs.

### count

`examples/count`, and `tests/fixtures-wasm/count`.

- **Idea:** two generators draw from fewer bytes. In the native idea, the alphabets of B1's
  second row and of B2's row hold the vertical tab and the form feed, as raw bytes beside `\t`,
  `\n` and `\r`; the twin's file has neither byte, so its tests and proofs never draw them. Its
  Purpose still counts both as whitespace, and its main.wat does (the bytes 9 to 13). No commit
  says why the two bytes are missing.
- **Program:** the same answers, 65536 bytes at a time and 16 a step in both (SSE2, NEON and
  SIMD128), with the lanes past the end masked; the read loop's bound as above.
- **Proofs:** native: lines and words over text with accented letters and symbols, up to 8 bytes
  (`src/prove/tests.rs`, `proves_a_word_count_over_text_beyond_ascii`, on a copy of the idea
  narrowed to 16 characters, both Linux runners in run 42361995155). WebAssembly: the model stops at
  main.wat line 149, `I8x16Splat`, on the runs of B1 and B2 (run 42361995155), so their proofs are
  tested, not proven, with that reason; it follows B4's (arguments, the usage line).

### dates

`tests/fixtures/dates`, and `tests/fixtures-wasm/dates`.

- **Idea:** the clock's range. The native clock reads from `0` to `253402300799` seconds, the
  end of 9999; the twin's to `18446744073`, a time in July 2554, and its tests draw from that
  range. A date's own year is still `1970` to `9999`.
- **Why:** WASI's clock gives a 64-bit count of nanoseconds, which holds 18446744073 whole seconds;
  past it, lengua's host and the prover's model both hold the reading at the most that 64 bits
  hold.
- **Program:** the same answers in the shared range; arguments past 128 KiB get the invalid date's
  message.
- **Proofs:** native: the day count's block, valid and invalid dates at once, proven for every
  input (`src/prove/tests.rs`, `proves_a_number_printed_only_where_the_input_is_valid`, both Linux
  runners in run 42361995155). WebAssembly: the model follows every test (run 42361995155); no run
  has recorded `check`'s proofs.

### days

`tests/fixtures/days`, and `tests/fixtures-wasm/days`.

- **Idea:** the clock's range, as for `dates`: `18446744073` seconds in place of `253402300799`,
  in its Purpose and in each of its tests that draws the clock.
- **Program:** the same answers in the shared range; arguments past 128 KiB get the usage line.
- **Proofs:** native: every block proven for every input (`src/prove/tests.rs`,
  `proves_every_block_of_the_day_count`, both Linux runners in run 42361995155). WebAssembly: the
  model follows every test (run 42361995155); no run has recorded `check`'s proofs.

### dice

`examples/dice`, and `tests/fixtures-wasm/dice`.

- **Idea:** the same, but the frontmatter, which has no `parameters`: random bytes aren't one.
- **Program:** the same answers: 64 random bytes drawn at once, the roll from the first below 252.
- **Proofs:** native: `tests/programs.rs`'s dice case reads its proof lines with a check that
  "tested, not proven" passes too, so no test pins its verdict. WebAssembly: the model follows every
  test (run 42361995155); no run has recorded `check`'s proofs.

### echo

`examples/echo`, and `tests/fixtures-wasm/echo`.

- **Idea:** smaller limits. A line has at most `8` characters, and the server reads at most 16
  bytes, where the native one takes `1023` and 1024.
- **Program:** a failed `sock_accept` or `fd_close` ends the twin with an assertion (exit code
  134), where the native server goes on to the next connection. The twin sends its answer with
  `$write_all`, every byte however many writes it takes; the native one writes once and closes the
  connection after a short write. Its listener is descriptor 4.
- **Why:** it is the WebAssembly prover's fixture (d31ee1c), whose test proves every block of it
  for every input (`src/prove/wasm/tests.rs`). No commit gives another reason for its limits.
- **Proofs:** native: proven however the network cuts the line (`src/prove/tests.rs`,
  `proves_the_echo_server`, both Linux runners in run 42361995155). WebAssembly: every block
  proven for every input, of the twin before b761dd3
  (`proves_greet_and_echo_and_add_to_three_digits`, all five runners in run 42361995155). b761dd3
  changed it, and no finished run has proven the new one.

### fetch

`examples/fetch`, and `tests/fixtures-wasm/fetch`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. Its connection is descriptor 5, where the native one's is 4.
- **Proofs:** native: every path and body and every delivery, and a server it can't reach
  (`src/prove/tests.rs`, `proves_the_fetch_client`, on a copy with paths of up to 6 characters and
  bodies of up to 11, both Linux runners in run 42361995155). WebAssembly: the model agrees with the
  engine on its tests (run 42361995155); no run has recorded `check`'s proofs.

### greet

`examples/greet`, and `tests/fixtures-wasm/greet`.

- **Idea:** smaller limits. A name has `1` to `8` characters, and the too-long names are `9` to
  `16`, where the native ones are `1` to `64` and `65` to `100`; B2's range goes to 9 characters,
  as its own example's `HOME` holds 9 (dac4c7d).
- **Program:** more than 16 environment variables, or more than 1024 bytes of their text, get the
  usage line, where the native program looks through up to 256.
- **Why:** it is the WebAssembly prover's fixture (d31ee1c), whose test proves every block of it
  for every input (`src/prove/wasm/tests.rs`). No commit gives another reason for its limits.
- **Proofs:** native: B1 proven for every input in range (`tests/programs.rs`,
  `a_program_gets_only_the_environment_its_test_or_run_gives`, both Linux runners in run
  42361995155). WebAssembly: every block proven for every input, of the twin before b761dd3 (run
  42361995155, as for echo). b761dd3 changed it, and no finished run has proven the new one.

### hypotenuse

`examples/hypotenuse`, and `tests/fixtures-wasm/hypotenuse`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. Both read each side in thousandths and compute in doubles, the
  formula's steps; the twin rounds with `f64.nearest`, ties to even, then `i64.trunc_sat_f64_s`, and
  makes every float from a whole number, so none is NaN.
- **Proofs:** native: B1 to B5 proven for every input in range, and memory on every run they
  cover, on both CPUs (`tests/programs.rs`, `the_hypotenuse_computes_in_doubles`, both Linux
  runners in run 42361995155). WebAssembly: the model doesn't follow floats yet. It stops at
  main.wat line 221, `F64ConvertI64S`, on runs of B1 and B2 (run 42361995155), so their proofs are
  tested, not proven, with that reason, as `tests/wasm_check.rs` shows of a float in add (run
  42362037786). It follows B3 to B5's runs.

### lines

`tests/fixtures/lines`, and `tests/fixtures-wasm/lines`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers, 4096 bytes at a time and 16 a step in both, with a scalar tail in
  the twin; the read loop's bound as above.
- **Proofs:** native: every length, by induction (`src/prove/tests.rs`,
  `proves_a_count_for_every_length_by_induction`, both Linux runners in run 42361995155).
  WebAssembly: the model stops at main.wat line 89, `I8x16Splat`, on the runs of B1 (run
  42361995155), so its proof is tested, not proven, with that reason.

### mean

`examples/mean`, and `tests/fixtures-wasm/mean`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. Both compute exact cents with integers, and no float takes part:
  the sum a cent further from zero, then halved, rounds a half cent away from zero.
- **Proofs:** native: every block proven for every input in range, on both CPUs
  (`tests/programs.rs`, `the_decimal_example_is_proven`, ignored as slow; its comment names runs
  41914657858 and 41947013349), and the whole means and the fixed outputs
  (`src/prove/tests.rs`, `proves_the_mean_of_whole_numbers` and
  `proves_decimal_inputs_where_the_output_is_fixed`, both Linux runners in run 42361995155).
  WebAssembly: the model follows every test (run 42361995155); no run has recorded `check`'s
  proofs.

### nap

`tests/fixtures/nap`, and `tests/fixtures-wasm/nap`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. The twin waits with `poll_oneoff` on one time, 30 seconds on the
  clock that only moves forward; in a test the wait ends at once, in the host and in the model.
- **Proofs:** no test pins the native verdict. WebAssembly: the model follows every test (run
  42361995155); no run has recorded `check`'s proofs.

### padded

`tests/fixtures/padded`, and `tests/fixtures-wasm/padded`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers, 65536 bytes at a time and 16 a step in both, the counts
  right-aligned in fields of 7; the read loop's bound as above.
- **Proofs:** native: proven for every input (`src/prove/tests.rs`,
  `proves_counts_padded_to_a_column`, on a copy with texts of up to 120 bytes, both Linux runners in
  run 42361995155). WebAssembly: the model stops at main.wat line 125, `I8x16Splat`, on the runs of
  B1 to B3 (run 42361995155), so their proofs are tested, not proven, with that reason.

### pairs

`tests/fixtures/pairs`, and `tests/fixtures-wasm/pairs`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. Both read 256 bytes at a time and look at each byte in turn,
  and carry across whether the last byte is an `a` that starts no pair yet; the read loop's bound
  as above.
- **Proofs:** native: B1 proven for every length, by induction (`tests/programs.rs`,
  `a_count_of_a_part_that_overlaps_itself_is_proven_for_every_length`, and `src/prove/tests.rs`,
  both Linux runners in run 42361995155). WebAssembly: by length only (`src/prove/mod.rs`); the
  model follows every test (run 42361995155).

### second

`tests/fixtures/second`, and `tests/fixtures-wasm/second`.

- **Idea:** the same, but the frontmatter.
- **Program:** the native program is wrong by design: one `fill` to the first newline a read ends
  with reads both lines only when they arrive in one read, and the tests' own pieces find it. The
  twin reads standard input to its end with `$fill`, however it arrives, then copies the second
  line to a start of its own, and passes every test. `$NEWLINE` in place of `$READ_TO_END` in its
  call of `$fill` plants the native bug again.
- **Why:** the brief for W9's ports asked for a twin that passes (bc4e2b7); `tests/every_program.rs`
  still expects the native tests to fail, and `tests/wasm_every_program.rs` expects the twin to
  reach done.
- **What lengua says:** natively `tests_failed`; for the twin, done (run 42361995155).
- **Proofs:** native: a read of its own is proven wrong on a short read
  (`src/prove/tests.rs`, `a_raw_read_of_short_input_is_proven_for_every_count`, both Linux runners
  in run 42361995155). WebAssembly: the model follows every test (run 42361995155); no run has
  recorded `check`'s proofs.

### sha256

`examples/sha256`, and `tests/fixtures-wasm/sha256`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers, 4096 bytes at a time in both. The native baseline is integer
  code; the twin reads a block's words 4 at a time, turns each word's bytes round with one shuffle,
  and adds the rounds' constants 4 words at a time, the rounds themselves integer code on eight
  locals. The read loop's bound as above.
- **Proofs:** its tests compute `sha256`, which the prover doesn't model on either path ("the test
  function sha256, which the proof doesn't model yet", `src/prove/formula.rs`), so they are tested,
  not proven; natively a proof can stop for memory first, as `scripts/ci/golden-expected.txt`
  notes. WebAssembly: before that, the model stops at main.wat line 182, `V128Load`, on the runs of
  every block (run 42361995155).

### sort

`examples/sort`, and `tests/fixtures-wasm/sort`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers and the same limits, with a merge sort in both. The native
  program finds, compares and copies lines a byte at a time; the twin does it 16 bytes at a time
  with SIMD128.
- **Why:** speed: the brief for W9's ports asked for the vector template's patterns where speed
  is a program's point, as in sort, count and sha256.
- **Proofs:** native: proven for every input up to the length where its comparisons branch past
  the proof's paths, at least 5 bytes (`src/prove/tests.rs`, `proves_sorting_for_short_texts`, both
  Linux runners in run 42361995155). WebAssembly: the model stops at main.wat line 217,
  `I8x16Splat`, on runs of B1 to B3 (run 42361995155), so their proofs are tested, not proven, with
  that reason; it follows B4 to B6's. Here the port's speed costs proofs the native program has.

### sweep

`tests/fixtures/sweep`, and `tests/fixtures-wasm/sweep`.

- **Idea:** the same, but the frontmatter, whose receipt adds `files`.
- **Program:** the same answers for the folders the idea gives (it prints `0` to `16`). Past them:
  the native program fails assertion 9 (exit code 134) when it deleted 1000 files or more, after
  deleting them, where the twin prints counts up to 10944. Both take at most 64 batches of 4096
  bytes of the folder's entries and exit with code 3 past them; a native entry is 4 bytes and the
  name with a NUL, rounded up to 8, and a WASI entry 24 bytes and the name, so a batch holds at most
  512 native entries and 171 WebAssembly ones. lengua's host lists a folder as it was when the
  listing began, so a file the twin deletes changes no later batch.
- **Proofs:** native: B1 tested, not proven: "listing a folder" (`tests/programs.rs`,
  `a_program_lists_its_folder_and_deletes_inside_it`, both Linux runners in run 42361995155).
  WebAssembly: the model takes a folder's listing in both of a test's orders (d31ee1c) and follows
  every test (run 42361995155); no run has recorded `check`'s proofs.

### total

`tests/fixtures/total`, and `tests/fixtures-wasm/total`.

- **Idea:** the same, but the frontmatter.
- **Program:** the same answers. The twin copies the file's name to a region of its own, so
  `path_open`'s path starts where its region does; a name over 1023 bytes, or arguments over its
  region, get the same message the native program gives when the broker refuses the name.
- **Proofs:** native: the 24,696 input combinations of its first block proven
  (`src/prove/tests.rs`, `proves_the_csv_totals_24696_combinations`, both Linux runners in run
  42361995155). WebAssembly: the model follows every test (run 42361995155); no run has recorded
  `check`'s proofs.

### upcase

`examples/upcase`, and `tests/fixtures-wasm/upcase`.

- **Idea:** the same, but the frontmatter, whose receipt adds `files`.
- **Program:** the twin capitalizes a byte at a time, where the native x86-64 program takes 16
  bytes at a time with SSE2 (its ARM64 twin is a byte at a time, with no branch on the byte). A name
  over 1023 bytes, or arguments over 32 KiB, get the usage line and exit code 1 from the twin; the
  native program passes the name on, and the broker's refusal gives exit code 4. Both are past the
  idea's names of up to `255` bytes.
- **Why:** the port kept the stream template's scalar form; a vector form is left to the
  performance milestone, W12.
- **Proofs:** native: B1 proven up to a length of its 1048576 bytes, with `prove_further` going
  on, and B2 to B4 proven for every input in range (`tests/programs.rs`,
  `the_file_example_proves_its_file_handling`, both Linux runners in run 42361995155). WebAssembly:
  92 runs of B1, its largest files, are too long for the model to follow (run 42361995155); a
  sample too long says nothing, and the proof by length goes on from the shorter ones
  (`src/prove/mod.rs`, `agrees_with`). The model follows B2 to B4.

### words

`tests/fixtures/words`, and `tests/fixtures-wasm/words`.

- **Idea:** the same, but the frontmatter.
- **Program:** the twin looks at each byte in turn, 4096 bytes at a time, where the native
  programs take 16 at a time with SSE2 and NEON; whether the last byte is whitespace crosses from
  one buffer-full to the next in both. The read loop's bound as above.
- **Why:** so the model and the proof follow it (9852fb3); a vector form is W12's, as for upcase.
- **Proofs:** native: B1 proven for every length, by induction (`tests/programs.rs`,
  `a_word_count_is_proven_for_every_length`, and `src/prove/tests.rs`,
  `proves_words_for_every_length_by_induction`, each on a copy with a buffer of 128 bytes, both
  Linux runners in run 42361995155). WebAssembly: by length only (`src/prove/mod.rs`); the model
  follows every test (run 42361995155).

## The verdicts that changed

- **Wrong by design, then right:** `second`'s native tests fail; its twin's pass.
- **Every length, then up to a length:** the native `cat-stream`, `pairs` and `words` (`words` on
  a copy with a smaller buffer) are proven for every length by induction; the WebAssembly prover
  proves by length only, so their twins can be proven only up to a length until it proves by
  induction (W8's third step, todo.md).
- **Proven, then tested, not proven:** the model doesn't follow SIMD128 or floats yet (W8's fourth
  step), so the proofs of `count`, `hypotenuse`, `lines`, `padded` and `sort` that the native
  programs have (`lines` for every length) end tested, not proven for the blocks whose runs reach
  those instructions; `archive`'s B3 waits for a name the input chooses.
- **Tested, not proven, on both paths:** `aes` and `sha256`, whose test functions the prover
  doesn't model.
- **Not yet known:** `check`'s proofs of the 26 ports, which no run has recorded; and `echo` and
  `greet` as b761dd3 changed them.
