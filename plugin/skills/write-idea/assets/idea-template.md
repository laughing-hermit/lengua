---
type: lengua/program-idea
title: my-program
description: One sentence that says what the program does.
---
# Purpose

Say in simple words what the program does, for whom and why. Write in Simplified Technical
English: short sentences, simple words, one word for one thing, and the active voice.

Give in backticks the range of each number that the program reads and the length of each text
that it reads. For example: whole numbers between `-1000` and `1000`. Put nothing else in
backticks here. If the program reads or writes, give the exit code for a failed read or write. For
example: If the program cannot write its output, it exits with code 2.

# Behaviors

## B1: The rule this example shows, such as: Echoes a number
- Given the argument `7`
- When the program runs
- Then it prints `7` and a newline
- And it exits with code `0`

### For any number in range
- Given the argument `{n}`
- When the program runs
- Then it prints `{n}` and a newline
- And it exits with code `0`

| n |
|---|
| `int(-1000, 1000)` |

## B2: An error rule, such as: Shows the usage line when there is no number
- Given no arguments
- When the program runs
- Then it prints `usage: my-program <number>` and a newline to standard error
- And it exits with code `1`

# Constraints

- Uses: nothing
