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
- `second`: two lines from standard input, read with one `fill` to the first newline a read
  ends with, so it's right only when both lines arrive together, as they do in the tests: the
  proof, which takes every way standard input can be cut, finds the cut after the first line.
