# Purpose

The program writes its arguments to the standard output. It writes each argument on a line of
its own, in the order of the arguments.

# Behaviours

## Write each argument on a line

Given the arguments `one` and `two`,
When the program runs,
Then it writes `one`, a line end, `two` and a line end to the standard output.

### Example: two arguments

- Arguments: "one" "two"
- Expect standard output: "one\ntwo\n"
- Expect exit code: 0

## Write nothing with no arguments

Given no arguments,
When the program runs,
Then it writes nothing to the standard output.

### Example: no arguments

- Expect standard output: ""
- Expect exit code: 0

# Constraints

- Reads: arguments
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: writes each argument and a line end to the standard output.
