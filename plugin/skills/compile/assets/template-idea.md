---
type: lengua/program-idea
title: negate
description: Prints the negative of a whole number given as an argument.
---
# Purpose
Lets a shell script negate a number: it reads one whole number between `-9223372036854775807`
and `9223372036854775807`, written in decimal with an optional leading minus sign, and prints its
negative. If writing the output fails, the program exits with code 3.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` followed by a newline
- And it exits with code `0`

### For any `n`
- Given the argument `{n}`
- When the program runs
- Then it prints `{-n}` followed by a newline
- And it exits with code `0`

| n | runs |
|---|---|
| `int(-9223372036854775807, 9223372036854775807)` | 200 |

## B2: Shows usage unless given one argument
- Given no arguments
- When the program runs
- Then it prints `usage: negate <n>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number
- Given the argument `12x`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

| s | runs |
|---|---|
| `choice("12x", "abc", "", "-", "--5", "+5", "1-2", "1:", "9223372036854775808", "-9223372036854775808")` | 50 |

## B4: Exits with code 3 when its output can't be written
- Given the argument `7`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `n`
- Given the argument `{n}`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

| n | runs |
|---|---|
| `int(-9223372036854775807, 9223372036854775807)` | 20 |

# Constraints
- Uses: nothing
