# Purpose

The program tells if a number is small. It reads the number from its first argument. It
writes `small` and a line end when the number is 9 or less. It writes `large` and a line end
when the number is more than 9. When the argument is not a number, it writes nothing and stops
with the exit code 2.

# Behaviours

## Write small for a number of one digit

Given an argument from 0 to 9,
When the program runs,
Then it writes `small` and a line end to the standard output.

### Cases: numbers of one digit

- Range `n`: integers from 0 to 9
- Arguments: "{n}"
- Expect standard output: "small\n"
- Expect exit code: 0

## Write large for a number of two digits

Given the argument 10,
When the program runs,
Then it writes `large` and a line end to the standard output.

### Example: the number 10

- Arguments: "10"
- Expect standard output: "large\n"
- Expect exit code: 0

## Stop when the argument is not a number

Given the argument `ten`,
When the program runs,
Then it writes nothing and stops with the exit code 2.

### Example: a word

- Arguments: "ten"
- Expect exit code: 2

# Constraints

- Reads: arguments
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: reads the first argument as a number and writes its size.
- `size`: gives `small` for a number of 9 or less, and `large` for a number that is more than 9.
