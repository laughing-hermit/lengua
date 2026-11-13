# Purpose

The program counts the words in a text. It reads the text from the standard input. It
writes the number of words and a line end to the standard output.

# Behaviours

## Count the words in a text

Given a text with two words on the standard input,
When the program runs,
Then it writes 2 and a line end to the standard output.

### Example: two words

- Standard input: "hello world\n"
- Expect standard output: "2\n"
- Expect exit code: 0

### Cases: one word

- Range `word`: text of 1 to 8 characters from "abc"
- Standard input: "{word}\n"
- Expect standard output: "1\n"
- Expect exit code: 0

## Count no words in an empty text

Given an empty standard input,
When the program runs,
Then it writes 0 and a line end to the standard output.

### Example: empty input

- Standard input: ""
- Expect standard output:
  ```
  0
  ```
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s
- Modules: `words`, `ui/summary`

# Structure

- `main`: reads the standard input and writes the number of words.
