---
type: Attested Computation
title: absdiff
description: Prints the absolute difference of the two whole numbers in its arguments.
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
A shell script can use this program to find how far apart two whole numbers are. The program
prints the absolute difference of two numbers between `-1000000` and `1000000`. It gets the two
numbers as arguments. Each number is in decimal digits, with an optional minus sign at the start.
If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Prints the absolute difference of two numbers
- Given the arguments `-3` and `7`
- When the program runs
- Then it prints `10` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{abs(a - b)}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000, 1000000)` | `int(-1000000, 1000000)` | 300 |

## B2: Shows the usage line when there are not exactly two arguments
- Given the argument `5`
- When the program runs
- Then it prints `usage: absdiff <a> <b>` and a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: absdiff <a> <b>` and a newline to standard error
- And it exits with code `1`

| a | runs |
|---|---|
| `int(-1000000, 1000000)` | 50 |

## B3: Rejects an argument that is not a whole number
- Given the arguments `12x` and `4`
- When the program runs
- Then it prints `absdiff: not a whole number` and a newline to standard error
- And it exits with code `2`

### For any `bad` and `b`
- Given the arguments `{bad}` and `{b}`
- When the program runs
- Then it prints `absdiff: not a whole number` and a newline to standard error
- And it exits with code `2`

| bad | b | runs |
|---|---|---|
| `choice("12x", "-", "1.5", "--1", "", "abc", "+5", " 7", "7 ", "1-2")` | `int(-1000000, 1000000)` | 100 |

## B4: Rejects a number out of range
- Given the arguments `1000001` and `0`
- When the program runs
- Then it prints `absdiff: number out of range` and a newline to standard error
- And it exits with code `3`

### For any `big` and `b`
- Given the arguments `{big}` and `{b}`
- When the program runs
- Then it prints `absdiff: number out of range` and a newline to standard error
- And it exits with code `3`

| big | b | runs |
|---|---|---|
| `choice("1000001", "-1000001", "9999999", "-10000000", "99999999999999999999999", "18446744073709551617")` | `int(-1000000, 1000000)` | 100 |

# Constraints
- Uses: nothing
