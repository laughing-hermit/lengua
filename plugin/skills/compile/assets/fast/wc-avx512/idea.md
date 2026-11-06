---
type: lengua/program-idea
title: wc
description: Counts the lines and words of standard input, as wc -lw does.
---
# Purpose
Counts the lines and words of a text on standard input, as `wc -lw` does, fast on very large
inputs: it reads standard input of any length, a piece at a time, and prints the number of
newlines, a space, the number of words and a newline. Tests give it up to `1048576` bytes. A word
is a run of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage
returns. If reading the input or writing the output fails, the program exits with code 2.

# Behaviors
## B1: Counts lines and words
- Given standard input holding `hello world\nfoo  bar\n`
- When the program runs
- Then it prints `2 4` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abdefhlorwxyz \n\t\r")` | 200 |

# Constraints
- Uses: standard input
