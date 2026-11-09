---
type: Attested Computation
title: pairs-stream
description: Counts the pairs of a letter in standard input of any length.
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
The program counts the times that `aa` occurs in standard input of any length. It reads one piece
at a time, so it can read input that is larger than its memory. Each `a` is part of at most one
pair. The program takes the pairs from the left, in the order that they come. Thus `aaa` has one
pair and `aaaa` has two pairs.

The program prints the count and a newline. The tests give it up to `65536` bytes. If the program
cannot read its input, it exits with code 3. If the program cannot write its output, it exits with
code 4.

# Behaviors
## B1: Counts the pairs
- Given standard input that contains `aaa baaaa`
- When the program runs
- Then it prints `3` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "aa")}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "ab \n")` | 60 |

# Constraints
- Uses: standard input
