---
type: lengua/program-idea
title: tally
description: Counts how often each word of standard input appears and prints them, most frequent first.
---
# Purpose
Counts how often each distinct word appears in standard input of up to `1048576` bytes, as a word
frequency count does, and prints a line for each: its count, a space, the word and a newline,
the most frequent first, and words with the same count in byte order. A word is a run of bytes
other than spaces, tabs, newlines, vertical tabs, form feeds and carriage returns. Empty input,
or input of whitespace only, prints nothing. Short inputs of up to `64` bytes are tried with a
wider mix of characters. Input longer than the limit is rejected rather than partly counted. If
reading the input fails, the program exits with code 3; if writing the output fails, it exits
with code 2.

# Behaviors
## B1: Counts each word, most frequent first
- Given standard input holding `the cat and the\tdog the cat\n`
- When the program runs
- Then it prints `3 the\n2 cat\n1 and\n1 dog\n`
- And it exits with code `0`

### For any text
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{tally(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "ab \n")` | 30 |
| `string(0, 64, "abcdeghnot \n\t")` | 300 |

## B2: Rejects input that is too long
- Given standard input holding `a` repeated `1048576` times followed by `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` followed by a newline to standard error
- And it exits with code `4`

### For any input past the limit
- Given standard input holding `{head}` followed by `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` followed by a newline to standard error
- And it exits with code `4`

| head | runs |
|---|---|
| `string(1048576, 1048576, "ab \n")` | 20 |

# Constraints
- Uses: standard input
