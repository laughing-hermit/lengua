Programs Claude Code wrote through lengua in earlier interactive sessions, rebuilt from those
sessions' transcripts, which the prover's tests prove:

- `total`: the total of a CSV file's amounts, whose valid-input block has 24,696 input
  combinations;
- `clock`: the time of day from the clock, in hours, minutes and seconds, with six divisions.

And one written for the tests themselves:

- `cat`: standard input of up to 5,000 bytes printed back, whose proof covers texts up to the
  4,096 bytes a proof reaches.
- `padded`: an AVX-512 `wc -lw` from interactive round 2, which pads its counts to seven
  columns with a chain of `if(…)`s choosing among texts of different lengths.
