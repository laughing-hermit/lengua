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
  resource: lengua run
  receipt: [stdout, stderr, exit]
attester:
  resource: lengua check
---
# Purpose
The program adds two whole numbers between `-1000000000` and `1000000000`.

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

## B2: Shows the usage line without arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
