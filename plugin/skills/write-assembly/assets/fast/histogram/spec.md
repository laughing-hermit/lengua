---
type: lengua/program-spec
title: histogram
description: Counts each byte value of standard input.
---
# Purpose
Shows how often each byte occurs in data, such as a file piped in: it reads standard input of any
length, a piece at a time, and prints one line for each byte value that occurs, from the smallest
value to the largest: the value in decimal, a space, and how many times it occurs. Tests give it
up to `1048576` bytes. If reading the input or writing the output fails, the program exits with
code 2.

# Behaviors
## B1: Counts each byte value
- Given the input `abba` followed by a newline
- When the program runs
- Then it prints `10 1`, `97 2` and `98 2`, each followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
