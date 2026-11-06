---
type: lengua/program-idea
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Lets a shell script add two numbers: it adds two whole numbers between `-1000000000` and
`1000000000`, written in decimal with an optional leading minus sign, and prints their sum. If
writing the output fails, the program exits with code 3.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Shows usage unless given two arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number in range
- Given the arguments `2` and `x`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

### For any `a` and `s`
- Given the arguments `{a}` and `{s}`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

| a | s | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `choice("x", "", "-", "1.5", "1000000001", "-1000000001", "9223372036854775808")` | 50 |

## B4: Exits with code 3 when its output can't be written
- Given the arguments `2` and `3`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 20 |

# Constraints
- Uses: nothing
