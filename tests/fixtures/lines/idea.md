---
type: lengua/program-idea
title: lines-stream
description: Counts the newlines of standard input of any length.
---
# Purpose
Counts the lines of standard input of any length, as `wc -l` does, a piece at a time, so input
larger than memory works: it prints the number of newline bytes and a newline. Tests give it up
to `65536` bytes. If reading the input fails, the program exits with code 3; if writing the
output fails, with code 4.

# Behaviors
## B1: Counts newlines
- Given standard input holding `one\ntwo\n`
- When the program runs
- Then it prints `2` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, "\n")}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "enotw \n")` | 60 |

# Constraints
- Uses: standard input
