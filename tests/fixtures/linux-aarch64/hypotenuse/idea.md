---
type: lengua/program-idea
title: hypotenuse
description: Prints the length of a right triangle's hypotenuse from its two side lengths, with three decimals.
---
# Purpose
Lets someone at a shell find the long side of a right triangle: given the lengths of the two
shorter sides as arguments, it prints the length of the hypotenuse, the square root of the sum of
their squares, rounded to three decimals. Each side is a number from `0.001` to `1000000.000`,
written as digits with an optional point followed by one to three more digits. If writing the
output fails, the program exits with code 4.

# Behaviors
## B1: Prints the hypotenuse of two whole sides
- Given the arguments `3` and `4`
- When the program runs
- Then it prints `5.000` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{fixed(sqrt(double(a) * double(a) + double(b) * double(b)), 3)}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `choice(1, 3, 5, 8, 12, 20, 99, 1000, 65535, 999999, 1000000)` | `choice(1, 4, 7, 13, 21, 100, 4096, 123456, 1000000)` | 200 |

## B2: Accepts sides with decimals
- Given the arguments `1.5` and `2.25`
- When the program runs
- Then it prints `2.704` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{fixed(sqrt(a * a + b * b), 3)}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `decimal(0.001, 1000000.000)` | `decimal(0.001, 1000000.000)` | 300 |

## B3: Shows usage when the number of arguments is wrong
- Given the argument `3`
- When the program runs
- Then it prints `usage: hypotenuse <a> <b>` followed by a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: hypotenuse <a> <b>` followed by a newline to standard error
- And it exits with code `1`

| a | runs |
|---|---|
| `decimal(0.001, 1000000.000)` | 50 |

## B4: Rejects a side that is not a number
- Given the arguments `3` and `four`
- When the program runs
- Then it prints `error: side is not a number` followed by a newline to standard error
- And it exits with code `2`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `error: side is not a number` followed by a newline to standard error
- And it exits with code `2`

| a | b | runs |
|---|---|---|
| `decimal(0.001, 1000000.000)` | `choice("four", "12x", "-", "--1", "", "3.", ".5", "1.2345", "1e3", "-4", "4 ", "1.2.3")` | 100 |

## B5: Rejects a side out of range
- Given the arguments `0` and `4`
- When the program runs
- Then it prints `error: side out of range` followed by a newline to standard error
- And it exits with code `3`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `error: side out of range` followed by a newline to standard error
- And it exits with code `3`

| a | b | runs |
|---|---|---|
| `choice("0", "0.000", "00", "1000000.001", "1000001", "99999999999999999999")` | `decimal(0.001, 1000000.000)` | 100 |

# Constraints
- Uses: nothing
