---
type: lengua/program-spec
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

## B2: Accepts sides with decimals
- Given the arguments `1.5` and `2.25`
- When the program runs
- Then it prints `2.704` followed by a newline
- And it exits with code `0`

## B3: Shows usage when the number of arguments is wrong
- Given only the argument `3`
- When the program runs
- Then it prints `usage: hypotenuse <a> <b>` followed by a newline to standard error
- And it exits with code `1`

## B4: Rejects a side that is not a number
- Given the arguments `3` and `four`
- When the program runs
- Then it prints `error: side is not a number` followed by a newline to standard error
- And it exits with code `2`

## B5: Rejects a side out of range
- Given the arguments `0` and `4`
- When the program runs
- Then it prints `error: side out of range` followed by a newline to standard error
- And it exits with code `3`

# Constraints
- Allowed system calls: write, exit
