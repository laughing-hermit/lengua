# Purpose

The program counts the different words in a text. It reads the text from the standard input.
It writes the number of different words and a line end to the standard output. It keeps the
words in a `HashMap`, and its idea does not grant random numbers. The host gives the `HashMap`
a fixed value for its keys, because the program does not have random numbers.

# Behaviours

## Count the different words in a text

Given the text `a b a` on the standard input,
When the program runs,
Then it writes 2 and a line end to the standard output.

### Example: a word two times

- Standard input: "a b a\n"
- Expect standard output: "2\n"
- Expect exit code: 0

## Count no words in an empty text

Given an empty standard input,
When the program runs,
Then it writes 0 and a line end to the standard output.

### Example: empty input

- Standard input: ""
- Expect standard output: "0\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: reads the standard input and writes the number of different words.
