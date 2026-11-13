# Purpose

The module writes the report line from the three numbers.

# Behaviours

## Write the numbers in one line

Given a text of one line with one word,
When the program runs,
Then it writes the three numbers in one line, with a line end.

### Example: one word

- Standard input: "tea\n"
- Expect standard output: "lines 1, words 1, maximum 3\n"
- Expect exit code: 0

# Structure

- `format_report`: gives the report line from the number of lines, the number of words and the
  maximum length.
