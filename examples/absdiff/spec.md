---
type: lengua/program-spec
title: absdiff
description: Prints the absolute difference of two whole numbers given as arguments.
---
# Purpose
Lets a shell script find how far apart two whole numbers are: it prints the absolute difference
of two numbers between `-1000000` and `1000000`, given as arguments. A number is written in
decimal digits with an optional leading minus sign. If writing the output fails, the program
exits with code 4.

# Behaviors
## B1: Prints the absolute difference of two numbers
- Given the arguments `-3` and `7`
- When the program runs
- Then it prints `10` followed by a newline to standard output
- And it exits with code `0`

## B2: Shows usage when there are not exactly two arguments
- Given only the argument `5`
- When the program runs
- Then it prints `usage: absdiff <a> <b>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects an argument that is not a whole number
- Given the arguments `12x` and `4`
- When the program runs
- Then it prints `absdiff: not a whole number` followed by a newline to standard error
- And it exits with code `2`

## B4: Rejects a number out of range
- Given the arguments `1000001` and `0`
- When the program runs
- Then it prints `absdiff: number out of range` followed by a newline to standard error
- And it exits with code `3`

# Constraints
- Allowed system calls: write, exit
