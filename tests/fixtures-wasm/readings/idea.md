---
type: Attested Computation
title: readings
description: Reads the clock that only moves forward a given number of times and prints how many readings were not earlier than the previous one.
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
A benchmark uses this program to measure how fast a program can read the clock. The argument is a
count from `1` to `1000000`. The program reads the clock that only moves forward that number of
times. Then it prints how many readings were not earlier than the previous one. On a clock that
only moves forward, this number is the count.

The tests give it counts from `1` to `1000`. If the argument is not such a count, the program
prints a usage line and exits with code 1. If the program cannot read the clock, it exits with
code 3. If the program cannot write its output, it exits with code 2.

# Behaviors
## B1: Counts the readings of the clock
- Given the argument `5`
- And the clock at `1735689600`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `n`
- Given the argument `{n}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{n}` and a newline
- And it exits with code `0`

| n | t | runs |
|---|---|---|
| `int(1, 1000)` | `int(0, 4102444799)` | 40 |

## B2: Rejects an argument that is not a count
- Given the argument `0`
- When the program runs
- Then it prints `usage: readings <count>` and a newline to standard error
- And it exits with code `1`

### For any argument that is not a count
- Given the argument `{text}`
- When the program runs
- Then it prints `usage: readings <count>` and a newline to standard error
- And it exits with code `1`

| text |
|---|
| `choice("0", "1000001", "12x", "", "-5", "99999999")` |

# Constraints
- Uses: the clock
