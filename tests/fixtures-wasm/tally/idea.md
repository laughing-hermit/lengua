---
type: Attested Computation
title: tally
description: Counts how often each word of standard input of any length occurs and prints the words, the most frequent first.
runtime: webassembly
computation: main.wat
parameters:
  - { name: text, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
The program counts how often each different word occurs in standard input of any length, as a
word frequency count does. It reads one piece of 64 KiB at a time, so it can count input that is
larger than its memory. For each word, it prints a line with the count, a space, the word and a
newline. The most frequent word comes first, and words with the same count come in byte order.

A word is a run of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage
returns. A word has up to `255` bytes, and a word of `256` bytes or more is an error. The program
keeps up to `65536` different words, and their bytes take up to `4194304` bytes in all. For empty
input, or input of whitespace only, the program prints nothing.

The tests give it up to `1048576` bytes. They also try short inputs of up to `64` bytes with a
wider mix of characters. The program rejects a word that is longer than the limit, and it prints
no count. If the program cannot read its input, it exits with code 3. If the program cannot write
its output, it exits with code 2. The test of a write that fails gives it `1` to `64` letters.

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
| `string(0, 1048576, "abcd  \n")` | 10 |
| `string(0, 64, "abcdeghnot \n\t")` | 300 |

## B2: Rejects a word that is too long
- Given standard input that contains `x ` then `a` `256` times
- When the program runs
- Then it prints `error: a word is longer than 255 bytes` and a newline to standard error
- And it exits with code `4`

### For any word past the limit
- Given standard input that contains `x ` then `{head}`
- When the program runs
- Then it prints `error: a word is longer than 255 bytes` and a newline to standard error
- And it exits with code `4`

| head | runs |
|---|---|
| `string(256, 1048576, "ab")` | 20 |

## B3: Exits with code 2 when it cannot write its output
- Given standard input that contains `cat`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

| text | runs |
|---|---|
| `string(1, 64, "abcdeghnot")` | 20 |

# Constraints
- Uses: standard input
