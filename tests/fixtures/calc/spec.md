---
type: lengua/program-spec
title: calc
description: Adds, subtracts or multiplies two whole numbers given as arguments around an operator.
---
# Purpose
Lets a shell script do one step of arithmetic: it reads a whole number, an operator and another
whole number, and prints their sum for `+`, their difference for `-` and their product for `*`.
Both numbers are between `-1000000000` and `1000000000`, written in decimal with an optional
leading minus sign, and the operator is `1` character. When a number and the operator are both
wrong, it reports the number. If writing the output fails, the program exits with code 4.

# Behaviors
## B1: Adds
- Given the arguments `2`, `+` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `+` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B2: Subtracts
- Given the arguments `2`, `-` and `3`
- When the program runs
- Then it prints `-1` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `-` and `{b}`
- When the program runs
- Then it prints `{a - b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B3: Multiplies
- Given the arguments `2`, `*` and `3`
- When the program runs
- Then it prints `6` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `*` and `{b}`
- When the program runs
- Then it prints `{a * b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B4: Shows usage unless given three arguments
- Given no arguments
- When the program runs
- Then it prints `usage: calc <a> <op> <b>` followed by a newline to standard error
- And it exits with code `1`

## B5: Rejects text that is not a number in range
- Given the arguments `2`, `+` and `x`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

### For any `a` and `s`
- Given the arguments `{a}`, `+` and `{s}`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

| a | s | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `choice("x", "", "-", "1.5", "1000000001", "-1000000001", "9223372036854775808")` | 50 |

## B6: Rejects an operator other than `+`, `-` and `*`
- Given the arguments `2`, `/` and `3`
- When the program runs
- Then it prints `error: unknown operator` followed by a newline to standard error
- And it exits with code `3`

### For any `op`
- Given the arguments `2`, `{op}` and `3`
- When the program runs
- Then it prints `error: unknown operator` followed by a newline to standard error
- And it exits with code `3`

| op | runs |
|---|---|
| `string(1, 1, "/,.x%#=^")` | 30 |
| `choice("", "++", "+-", "**", " +", "- ")` | 20 |

# Constraints
- Uses: nothing
