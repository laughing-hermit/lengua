# Purpose

The program counts the lines and the words of a text. It reads the text from the standard
input. It writes one line with the number of lines, the number of words and the maximum length
of a word. The program has three modules: `lines`, `words` and `report`.

# Behaviours

## Count a text of two lines

Given the text `a bb` and `ccc` on two lines,
When the program runs,
Then it writes `lines 2, words 3, maximum 3` and a line end.

### Example: two lines

- Standard input: "a bb\nccc\n"
- Expect standard output: "lines 2, words 3, maximum 3\n"
- Expect exit code: 0

## Count an empty text

Given an empty standard input,
When the program runs,
Then it writes `lines 0, words 0, maximum 0` and a line end.

### Example: empty input

- Standard input: ""
- Expect standard output: "lines 0, words 0, maximum 0\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s
- Modules: `lines`, `words`, `report`

# Structure

- `main`: reads the standard input and writes the report of the text.
