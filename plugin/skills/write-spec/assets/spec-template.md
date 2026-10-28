---
type: lengua/program-spec
title: my-program
description: One sentence that says what the program does.
---
# Purpose

Say what the program does and for whom, in plain words. Give the range of every number it reads
or prints as backtick values, for example: whole numbers between `-1000` and `1000`.

# Behaviors

## B1: Short title of the main behavior
- Given the argument `7`
- When the program runs
- Then it prints `7` followed by a newline
- And it exits with code `0`

## B2: Short title of an error behavior
- Given no arguments
- When the program runs
- Then it prints `usage: my-program <number>` followed by a newline to standard error
- And it exits with code `1`

# Constraints

- Allowed system calls: read, write, exit
