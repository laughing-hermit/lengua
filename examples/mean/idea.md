---
type: lengua/program-idea
title: mean
description: Prints the mean of the two decimal numbers in its arguments, with two decimal places.
---
# Purpose
A shell script can use this program to find the mean of two decimal numbers. The arguments are two
numbers between `-1000000.00` and `1000000.00`. The program prints their mean with exactly two
decimal places.

Each number has an optional minus sign, one or more digits, and an optional point with one or two
digits after it. If the exact mean has a third decimal place, that digit can only be a 5. Then the
program rounds the result away from zero. If the program cannot write its output, it exits with
code 4.

# Behaviors
## B1: Prints the mean with two decimals
- Given the arguments `2.5` and `3.76`
- When the program runs
- Then it prints `3.13` and a newline
- And it exits with code `0`

### For any `a` and `b`
The formula halves the sum in cents and rounds a half cent away from zero.

- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{places(if(scaled(a, 2) + scaled(b, 2) < 0, scaled(a, 2) + scaled(b, 2) - 1, scaled(a, 2) + scaled(b, 2) + 1) / 2, 2)}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `decimal(-1000000.00, 1000000.00)` | `decimal(-1000000.00, 1000000.00)` | 300 |

## B2: Prints whole means with two decimals
- Given the arguments `4` and `-10`
- When the program runs
- Then it prints `-3.00` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{places((a + b) * 50, 2)}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000, 1000000)` | `int(-1000000, 1000000)` | 200 |

## B3: Rounds a half cent away from zero
- Given the arguments `-1.01` and `-1.02`
- When the program runs
- Then it prints `-1.02` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{places(if(scaled(a, 2) + scaled(b, 2) < 0, scaled(a, 2) + scaled(b, 2) - 1, scaled(a, 2) + scaled(b, 2) + 1) / 2, 2)}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `decimal(-1000000.00, 1000000.00)` | `decimal(-1000000.00, 1000000.00)` | 300 |

## B4: Shows the usage line for a wrong number of arguments
- Given the argument `1.5`
- When the program runs
- Then it prints `usage: mean <a> <b>` and a newline to standard error
- And it exits with code `1`

### For any `a`
- Given the argument `{a}`
- When the program runs
- Then it prints `usage: mean <a> <b>` and a newline to standard error
- And it exits with code `1`

| a | runs |
|---|---|
| `decimal(-1000000.00, 1000000.00)` | 50 |

## B5: Rejects an argument that is not a number
- Given the arguments `1.5` and `abc`
- When the program runs
- Then it prints `mean: not a number: abc` and a newline to standard error
- And it exits with code `2`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `mean: not a number: {b}` and a newline to standard error
- And it exits with code `2`

| a | b | runs |
|---|---|---|
| `decimal(-1000000.00, 1000000.00)` | `choice("abc", "12x", "-", "--1", "", "1.", ".5", "1.234", "1..2", "1.5x", "+3", "1 ")` | 100 |

## B6: Rejects a number out of range
- Given the arguments `1000000.01` and `0`
- When the program runs
- Then it prints `mean: out of range: 1000000.01` and a newline to standard error
- And it exits with code `3`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `mean: out of range: {a}` and a newline to standard error
- And it exits with code `3`

| a | b | runs |
|---|---|---|
| `choice("1000000.01", "-1000000.01", "1000001", "-1000000.1", "9999999999999999999999")` | `decimal(-1000000.00, 1000000.00)` | 100 |

# Constraints
- Uses: nothing
