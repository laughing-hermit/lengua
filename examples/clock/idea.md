# Purpose

The program writes the time and a random number to the standard output. The time is the
number of seconds after the start of 1970 on the clock of the host. With the fixed clock and
the fixed random numbers of the tests, the program writes the same two numbers on each run.

# Behaviours

## Write the time and a random number

Given the fixed clock and the fixed random numbers of the tests,
When the program runs,
Then it writes 1767225600, a line end, a random number and a line end to the standard output.

### Example: the fixed clock

- Expect standard output: "1767225600\n0\n"
- Expect exit code: 0

# Constraints

- Reads: the clock
- Reads: random numbers
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes the seconds of the clock and a random number to the standard output.
