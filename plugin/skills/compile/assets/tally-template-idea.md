---
type: lengua/program-idea
title: tally
description: Counts how often each word of standard input occurs and prints the words, the most frequent first.
---
# Purpose
The program counts how often each different word occurs in standard input of up to `1048576`
bytes. It works as a word frequency count does. For each word, it prints a line with the count, a
space, the word and a newline. The most frequent word comes first, and words with the same count
come in byte order.

A word is a run of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage
returns. For empty input, or input of whitespace only, the program prints nothing.

The tests also try short inputs of up to `64` bytes with a wider mix of characters. The program
rejects input that is longer than the limit, and it does not count a part of it. If the program
cannot read its input, it exits with code 3. If the program cannot write its output, it exits with
code 2.

# Behaviors
## B1: Counts each word, most frequent first
- Given standard input that contains `the cat and the\tdog the cat\n`
- When the program runs
- Then it prints `3 the\n2 cat\n1 and\n1 dog\n`
- And it exits with code `0`

### For any text
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{tally(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "ab \n")` | 30 |
| `string(0, 64, "abcdeghnot \n\t")` | 300 |

## B2: Rejects input that is too long
- Given standard input that contains `a` `1048576` times then `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `4`

### For any input past the limit
- Given standard input that contains `{head}` then `\n`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `4`

| head | runs |
|---|---|
| `string(1048576, 1048576, "ab \n")` | 20 |

# Constraints
- Uses: standard input
