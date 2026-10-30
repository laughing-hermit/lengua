---
type: lengua/program-spec
title: my-program
description: One sentence that says what the program does.
---
# Purpose

Say in plain words what the program does, for whom and why. Give in backticks the range of every
number it reads and the length of any text it reads, for example: whole numbers between `-1000`
and `1000`. Put nothing else in backticks here. If it reads or writes, say which exit code it uses
when reading or writing fails, for example: If writing the output fails, the program exits with
code 2.

# Behaviors

## B1: The rule this example shows, such as: Echoes a number
- Given the argument `7`
- When the program runs
- Then it prints `7` followed by a newline
- And it exits with code `0`

### For any number in range
- Given the argument `{n}`
- When the program runs
- Then it prints `{n}` followed by a newline
- And it exits with code `0`

| n |
|---|
| `int(-1000, 1000)` |

## B2: An error rule, such as: Shows usage when the number is missing
- Given no arguments
- When the program runs
- Then it prints `usage: my-program <number>` followed by a newline to standard error
- And it exits with code `1`

# Constraints

- Uses: nothing
