---
type: Attested Computation
title: negate
description: Prints the negative of the whole number in its argument.
runtime: webassembly
computation: main.wat
parameters:
  - { name: n, type: integer, required: true, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A shell script can use this program to negate a number. It reads one whole number between `-9223372036854775807`
and `9223372036854775807` and prints its negative. The number is in decimal, with an optional minus
sign at the start. If the program cannot write its output, it exits with code 3.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` and a newline
- And it exits with code `0`

### For any `n`
- Given the argument `{n}`
- When the program runs
- Then it prints `{-n}` and a newline
- And it exits with code `0`

| n | runs |
|---|---|
| `int(-9223372036854775807, 9223372036854775807)` | 200 |

## B2: Shows the usage line when it does not get one argument
- Given no arguments
- When the program runs
- Then it prints `usage: negate <n>` and a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number
- Given the argument `12x`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

| s | runs |
|---|---|
| `choice("12x", "abc", "", "-", "--5", "+5", "1-2", "1:", "9223372036854775808", "-9223372036854775808")` | 50 |

## B4: Exits with code 3 when it cannot write its output
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
