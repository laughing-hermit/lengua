---
type: lengua/program-idea
title: lines-stream
description: Counts the newlines of standard input of any length.
---
# Purpose
The program counts the lines of standard input of any length, as `wc -l` does. It reads one piece
at a time, so it can read input that is larger than its memory. It prints the number of newline
bytes and a newline.

The tests give it up to `65536` bytes. If the program cannot read its input, it exits with code 3.
If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Counts newlines
- Given standard input that contains `one\ntwo\n`
- When the program runs
- Then it prints `2` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "\n")}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "enotw \n")` | 60 |

# Constraints
- Uses: standard input
