---
type: lengua/program-idea
title: wc
description: Counts the lines and words of standard input, as wc -lw does.
---
# Purpose
The program counts the lines and words of a text on standard input, as `wc -lw` does. It is fast
on very large inputs. It reads standard input of any length, one piece at a time. It prints the
number of newlines, a space, the number of words and a newline.

A word is a run of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage
returns. The tests give it up to `1048576` bytes. If the program cannot read its input or write
its output, it exits with code 2.

# Behaviors
## B1: Counts lines and words
- Given standard input that contains `hello world\nfoo  bar\n`
- When the program runs
- Then it prints `2 4` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{count(text, "\n")} {words(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abdefhlorwxyz \n\t\r")` | 200 |

# Constraints
- Uses: standard input
