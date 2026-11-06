---
type: lengua/program-idea
title: utc
description: Prints the current UTC time of day as HH:MM:SS.
---
# Purpose

A person at a shell can use this program to see the current time of day in UTC. The program reads
the system clock as whole seconds since 1970, from `0` to `4102444799`. It keeps only the seconds
after the most recent UTC midnight. It prints them as hours, minutes and seconds, with a colon
between each two parts.

Each part has two digits, with a zero at the start where necessary. The program ignores fractions
of a second. It takes no arguments, and it ignores the arguments that it gets. If the program
cannot read the clock, it exits with code 3. If the program cannot write its output, it exits with
code 2.

# Behaviors

## B1: Prints the UTC time of day
- Given the clock at `1735735845`
- When the program runs
- Then it prints `12:50:45` and a newline
- And it exits with code `0`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints `{if(t % 86400 / 3600 < 10, "0", "")}{t % 86400 / 3600}:{if(t % 3600 / 60 < 10, "0", "")}{t % 3600 / 60}:{if(t % 60 < 10, "0", "")}{t % 60}` and a newline
- And it exits with code `0`

| t | runs |
|---|---|
| `int(0, 4102444799)` | 1000 |

## B2: Pads each part to two digits
- Given the clock at `1735693509`
- When the program runs
- Then it prints `01:05:09` and a newline
- And it exits with code `0`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints `{if(t % 86400 / 3600 < 10, "0", "")}{t % 86400 / 3600}:{if(t % 3600 / 60 < 10, "0", "")}{t % 3600 / 60}:{if(t % 60 < 10, "0", "")}{t % 60}` and a newline
- And it exits with code `0`

| t | runs |
|---|---|
| `int(0, 4102444799)` | 1000 |

# Constraints

- Uses: the clock
