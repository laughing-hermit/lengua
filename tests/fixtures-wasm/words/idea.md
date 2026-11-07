---
type: Attested Computation
title: words-stream
description: Counts the lines and words of standard input of any length.
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
The program counts the lines and words of standard input of any length, as `wc -lw` does. It reads
one piece at a time, so it can read input that is larger than its memory. It prints the number of
newline bytes, a space, the number of words and a newline.

A word is a run of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage
returns. The tests give it up to `65536` bytes. If the program cannot read its input, it exits with
code 3. If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Counts lines and words
- Given standard input that contains `ab ba\nb\n`
- When the program runs
- Then it prints `2 3` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "ab \n\t\r")` | 60 |

# Constraints
- Uses: standard input
