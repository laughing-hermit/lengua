---
type: lengua/program-spec
title: utc
description: Prints the current UTC time of day as HH:MM:SS.
---
# Purpose

Lets someone at a shell see the current time of day in UTC at a glance. The program reads the
system clock as whole seconds since 1970, from `0` to `4102444799`, keeps only the seconds past
the most recent UTC midnight, and prints them as hours, minutes and seconds, each as two digits
with a leading zero where needed, separated by colons. Fractions of a second are dropped. It takes
no arguments and ignores any it is given. If reading the clock fails, the program exits with code
3. If writing the output fails, the program exits with code 2.

# Behaviors

## B1: Prints the UTC time of day
- Given the clock at `1735735845`
- When the program runs
- Then it prints `12:50:45` followed by a newline to standard output
- And it exits with code `0`

## B2: Pads each part to two digits
- Given the clock at `1735693509`
- When the program runs
- Then it prints `01:05:09` followed by a newline to standard output
- And it exits with code `0`

# Constraints

- Allowed system calls: clock_gettime, write, exit
