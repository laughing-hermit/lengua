---
type: lengua/program-spec
title: negate
description: Prints the negative of a whole number given as an argument.
---
# Purpose
Lets a shell script negate a number: it reads one whole number between `-9223372036854775807`
and `9223372036854775807`, written in decimal with an optional leading minus sign, and prints its
negative. If writing the output fails, the program exits with code 3.

# Behaviors
## B1: Negates a number
- Given the argument `7`
- When the program runs
- Then it prints `-7` followed by a newline
- And it exits with code `0`

## B2: Shows usage unless given one argument
- Given no arguments
- When the program runs
- Then it prints `usage: negate <n>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects text that is not a number
- Given the argument `12x`
- When the program runs
- Then it prints `error: not a number` followed by a newline to standard error
- And it exits with code `2`

# Constraints
- Allowed system calls: write, exit
