---
type: Attested Computation
title: add
description: Adds the two whole numbers in its arguments and prints the sum.
runtime: webassembly
computation: main.wat
parameters:
  - { name: a, type: integer, required: true, from: argument 1 }
  - { name: b, type: integer, required: true, from: argument 2 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A shell script can use this program to add two numbers. The program adds two whole numbers
between `-1000000000` and `1000000000` and prints their sum. Each number is in decimal, with an
optional minus sign at the start. If the program cannot write its output, it exits with code 3.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Shows the usage line when it does not get two arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number in range
- Given the arguments `2` and `x`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

### For any `a` and `s`
- Given the arguments `{a}` and `{s}`
- When the program runs
- Then it prints `error: not a number` and a newline to standard error
- And it exits with code `2`

| a | s | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `choice("x", "", "-", "1.5", "1000000001", "-1000000001", "9223372036854775808")` | 50 |

## B4: Exits with code 3 when it cannot write its output
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
