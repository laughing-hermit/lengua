# Purpose

The module counts the words of a text and finds the maximum length of a word. Spaces, `tab`
characters and line ends are between the words.

# Behaviours

## Find the maximum length of a word

Given the words `a`, `bbb` and `cc` on one line,
When the program runs,
Then it writes 3 words and 3 as the maximum length.

### Example: three words

- Standard input: "a bbb cc\n"
- Expect standard output: "lines 1, words 3, maximum 3\n"
- Expect exit code: 0

## Count two words with a `tab` between them

Given the words `x` and `yy` with a `tab` character between them,
When the program runs,
Then it writes 2 words.

### Example: a `tab` character

- Standard input: "x\tyy\n"
- Expect standard output: "lines 1, words 2, maximum 2\n"
- Expect exit code: 0

# Structure

- `count_words`: gives the number of words in a text.
- `maximum_length`: gives the maximum length of a word in a text, or 0 for a text with no words.
