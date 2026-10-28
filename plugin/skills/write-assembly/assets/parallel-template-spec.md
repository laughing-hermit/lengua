---
type: lengua/program-spec
title: shout-parallel
description: Prints its input with every ASCII letter in capitals, using several cores.
---
# Purpose

Lets someone shout a large text fast: it reads up to `1048576` bytes from standard input and
prints them with every ASCII letter from a to z turned into its capital, and every other byte left
as it is. If reading the input or writing the output fails, the program exits with code 2.

# Behaviors

## B1: Capitalizes the input
- Given the input `Hello, World!` followed by a newline
- When the program runs
- Then it prints `HELLO, WORLD!` followed by a newline
- And it exits with code `0`

## B2: Rejects input that is too long
- Given an input of 1048577 bytes
- When the program runs
- Then it prints `shout: the input is longer than 1048576 bytes` followed by a newline to standard error
- And it exits with code `1`

# Constraints

- Allowed system calls: read, write, exit
- Threads: up to 16
