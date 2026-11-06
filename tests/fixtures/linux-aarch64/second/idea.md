---
type: lengua/program-idea
title: second
description: Prints the second of two lines read from standard input.
---
# Purpose
Reads two lines from standard input and prints the second one. Each line is `1` to `20`
lowercase letters followed by a newline. If reading or writing fails, it exits with code 1.

# Behaviors
## B1: Prints the second line
- Given standard input holding `ab\ncd\n`
- When the program runs
- Then it prints `cd` followed by a newline
- And it exits with code `0`

### For any `first` and `second`
- Given standard input holding `{first}\n{second}\n`
- When the program runs
- Then it prints `{second}` followed by a newline
- And it exits with code `0`

| first | second | runs |
|---|---|---|
| `string(1, 20, "abcdefghijklmnopqrstuvwxyz")` | `string(1, 20, "abcdefghijklmnopqrstuvwxyz")` | 100 |

# Constraints
- Uses: standard input
