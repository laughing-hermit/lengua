---
type: lengua/program-idea
title: second
description: Reads two lines from standard input and prints the second line.
---
# Purpose
The program reads two lines from standard input and prints the second line. Each line is `1` to `20`
lowercase letters and a newline. If the program cannot read its input or write its output, it exits
with code 1.

# Behaviors
## B1: Prints the second line
- Given standard input that contains `ab\ncd\n`
- When the program runs
- Then it prints `cd` and a newline
- And it exits with code `0`

### For any `first` and `second`
- Given standard input that contains `{first}\n{second}\n`
- When the program runs
- Then it prints `{second}` and a newline
- And it exits with code `0`

| first | second | runs |
|---|---|---|
| `string(1, 20, "abcdefghijklmnopqrstuvwxyz")` | `string(1, 20, "abcdefghijklmnopqrstuvwxyz")` | 100 |

# Constraints
- Uses: standard input
