---
type: lengua/program-spec
title: my-program
description: One sentence that says what the program does.
---
# Purpose

Say what the program does, in plain words. Give in backticks the range of every number it reads
and the length of any text it reads, for example: whole numbers between `-1000` and `1000`. Put
nothing else in backticks here. If it prints, say which exit code it uses when writing fails, for
example: If writing the output fails, the program exits with code 2.

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
