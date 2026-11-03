---
type: lengua/program-spec
title: words-stream
description: Counts the lines and words of standard input of any length.
---
# Purpose
Counts the lines and words of standard input of any length, as `wc -lw` does, a piece at a time,
so input larger than memory works: it prints the number of newline bytes, a space, the number of
words and a newline. A word is a run of bytes other than spaces, tabs, newlines, vertical tabs,
form feeds and carriage returns. Tests give it up to `65536` bytes. If reading the input fails,
the program exits with code 3; if writing the output fails, with code 4.

# Behaviors
## B1: Counts lines and words
- Given standard input holding `ab ba\nb\n`
- When the program runs
- Then it prints `2 3` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "ab \n\t\r")` | 60 |

# Constraints
- Uses: standard input
