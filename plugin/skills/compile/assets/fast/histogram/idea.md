---
type: lengua/program-idea
title: histogram
description: Counts each byte value of standard input.
---
# Purpose
Shows how often each byte occurs in data, such as a file piped in: it reads standard input of any
length, a piece at a time, and prints one line for each byte value that occurs, from the smallest
value to the largest: the value in decimal, a space, and how many times it occurs. Tests give it
up to `1048576` bytes. If reading the input or writing the output fails, the program exits with
code 2.

# Behaviors
## B1: Counts each byte value
- Given standard input holding `abba` followed by a newline
- When the program runs
- Then it prints `10 1\n97 2\n98 2\n`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{if(count(text, "\n") > 0, "10 ", "")}{if(count(text, "\n") > 0, count(text, "\n"), "")}{if(count(text, "\n") > 0, "\n", "")}{if(count(text, "a") > 0, "97 ", "")}{if(count(text, "a") > 0, count(text, "a"), "")}{if(count(text, "a") > 0, "\n", "")}{if(count(text, "b") > 0, "98 ", "")}{if(count(text, "b") > 0, count(text, "b"), "")}{if(count(text, "b") > 0, "\n", "")}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "ab\n")` | 200 |

# Constraints
- Uses: standard input
