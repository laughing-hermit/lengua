---
type: lengua/program-idea
title: histogram
description: Counts each byte value of standard input.
---
# Purpose
The program shows how often each byte occurs in data, such as a file on standard input. It reads
standard input of any length, one piece at a time. For each byte value that occurs, it prints one
line, from the smallest value to the largest. Each line has the value in decimal, a space, and the
number of times that the value occurs.

The tests give it up to `1048576` bytes. If the program cannot read its input or write its output,
it exits with code 2.

# Behaviors
## B1: Counts each byte value
- Given standard input that contains `abba` and a newline
- When the program runs
- Then it prints `10 1\n97 2\n98 2\n`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{if(count(text, "\n") > 0, "10 ", "")}{if(count(text, "\n") > 0, count(text, "\n"), "")}{if(count(text, "\n") > 0, "\n", "")}{if(count(text, "a") > 0, "97 ", "")}{if(count(text, "a") > 0, count(text, "a"), "")}{if(count(text, "a") > 0, "\n", "")}{if(count(text, "b") > 0, "98 ", "")}{if(count(text, "b") > 0, count(text, "b"), "")}{if(count(text, "b") > 0, "\n", "")}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "ab\n")` | 200 |

# Constraints
- Uses: standard input
