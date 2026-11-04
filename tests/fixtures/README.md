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
- `padded`: an AVX-512 `wc -lw` from interactive round 2, which pads its counts to seven
  columns with a chain of `if(…)`s choosing among texts of different lengths.
- `lines`: `wc -l` over standard input of any length, a buffer-full at a time through `fill`,
  its count kept in a register: proven for every length by induction, the count growing by each
  buffer-full's newlines.
- `words`: `wc -lw` over standard input of any length, a buffer-full at a time through `fill`,
  64 bytes a step with AVX-512 masks, whether the last byte read was whitespace kept in r14:
  proven for every length by induction, each buffer-full's words counted after the last byte of
  the one before.
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
through the server on each target a computer runs.
