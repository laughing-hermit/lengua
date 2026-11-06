---
type: lengua/program-idea
title: calc
description: Adds, subtracts or multiplies two whole numbers from its arguments, with an operator between them.
---
# Purpose
A shell script can use this program to do one step of arithmetic. The arguments are a whole
number, an operator and a second whole number. The program prints the sum for `+`, the difference
for `-` and the product for `*`. Each number is between `-1000000000` and `1000000000`, in
decimal, with an optional minus sign at the start. The operator is `1` character.

If a number and the operator are both wrong, the program reports the number. If the program cannot
write its output, it exits with code 4.

# Behaviors
## B1: Adds
- Given the arguments `2`, `+` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `+` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B2: Subtracts
- Given the arguments `2`, `-` and `3`
- When the program runs
- Then it prints `-1` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `-` and `{b}`
- When the program runs
- Then it prints `{a - b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B3: Multiplies
- Given the arguments `2`, `*` and `3`
- When the program runs
- Then it prints `6` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}`, `*` and `{b}`
- When the program runs
- Then it prints `{a * b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 100 |

## B4: Shows the usage line when it does not get three arguments
- Given no arguments
- When the program runs
- Then it prints `usage: calc <a> <op> <b>` and a newline to standard error
- And it exits with code `1`

## B5: Rejects text that is not a number in range
- Given the arguments `2`, `+` and `x`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

### For any `a` and `s`
- Given the arguments `{a}`, `+` and `{s}`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

| a | s | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `choice("x", "", "-", "1.5", "1000000001", "-1000000001", "9223372036854775808")` | 50 |

## B6: Rejects an operator other than `+`, `-` and `*`
- Given the arguments `2`, `/` and `3`
- When the program runs
- Then it prints `error: unknown operator` and a newline to standard error
- And it exits with code `3`

### For any `op`
- Given the arguments `2`, `{op}` and `3`
- When the program runs
- Then it prints `error: unknown operator` and a newline to standard error
- And it exits with code `3`

| op | runs |
|---|---|
| `string(1, 1, "/,.x%#=^")` | 30 |
| `choice("", "++", "+-", "**", " +", "- ")` | 20 |

# Constraints
- Uses: nothing
