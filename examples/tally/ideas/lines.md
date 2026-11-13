# Purpose

The module counts the lines of a text. A line ends with a line end, and the last line can have
no line end.

# Behaviours

## Count a last line with no line end

Given the text `a` and `b` on two lines, with no line end after `b`,
When the program runs,
Then it writes 2 as the number of lines.

### Example: no last line end

- Standard input: "a\nb"
- Expect standard output: "lines 2, words 2, maximum 1\n"
- Expect exit code: 0

# Structure

- `count_lines`: gives the number of lines in a text.
