---
type: lengua/program-spec
title: mean
description: Prints the mean of two decimal numbers given as arguments, with two decimal places.
---
# Purpose
Lets a shell script average two decimal numbers: it prints the mean of two numbers between
`-1000000.00` and `1000000.00`, given as arguments, with exactly two decimal places. Each number
is written with an optional leading minus sign, one or more digits, and optionally a point
followed by one or two digits. When the exact mean has a third decimal place (it can only be a
5), the result is rounded away from zero. If writing the output fails, the program exits with
code 4.

# Behaviors
## B1: Prints the mean with two decimals
- Given the arguments `2.5` and `3.76`
- When the program runs
- Then it prints `3.13` followed by a newline to standard output
- And it exits with code `0`

## B2: Prints whole means with two decimals
- Given the arguments `4` and `-10`
- When the program runs
- Then it prints `-3.00` followed by a newline to standard output
- And it exits with code `0`

## B3: Rounds a half cent away from zero
- Given the arguments `-1.01` and `-1.02`
- When the program runs
- Then it prints `-1.02` followed by a newline to standard output
- And it exits with code `0`

## B4: Shows usage when the argument count is wrong
- Given only the argument `1.5`
- When the program runs
- Then it prints `usage: mean <a> <b>` followed by a newline to standard error
- And it exits with code `1`

## B5: Rejects an argument that is not a number
- Given the arguments `1.5` and `abc`
- When the program runs
- Then it prints `mean: not a number: abc` followed by a newline to standard error
- And it exits with code `2`

## B6: Rejects a number out of range
- Given the arguments `1000000.01` and `0`
- When the program runs
- Then it prints `mean: out of range: 1000000.01` followed by a newline to standard error
- And it exits with code `3`

# Constraints
- Allowed system calls: write, exit
