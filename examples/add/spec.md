---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`, written in decimal with an
optional leading minus sign, and prints their sum. If writing the output fails, the program
exits with code 3.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Wrong number of arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

## B3: Not a number in range
- Given the arguments `2` and `x`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

# Constraints
- Allowed system calls: write, exit
