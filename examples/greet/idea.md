---
type: lengua/program-idea
title: greet
description: Greets the person that the NAME environment variable names.
---
# Purpose
The program greets the person that the `NAME` environment variable names, as the welcome line of a
shell. A name has `1` to `64` characters. Without a name, or with a name of `65` to `100`
characters or more, the program prints how to use it. It prints this to standard error and exits
with code 1. If the program cannot write its output, it exits with code 3.

# Behaviors
## B1: Greets the person that NAME names
- Given the environment variable `NAME` with the value `Ada`
- When the program runs
- Then it prints `Hello, Ada!` and a newline
- And it exits with code `0`

### For any `name`
- Given the environment variable `NAME` with the value `{name}`
- When the program runs
- Then it prints `Hello, {name}!` and a newline
- And it exits with code `0`

| name |
|---|
| `string(1, 64, "Adabcz -")` |

## B2: Shows the usage line without a name
- Given the environment variable `HOME` with the value `/home/ada`
- When the program runs
- Then it prints `usage: NAME=<who> greet` and a newline to standard error
- And it exits with code `1`

### For any `home`
- Given the environment variable `HOME` with the value `{home}`
- When the program runs
- Then it prints `usage: NAME=<who> greet` and a newline to standard error
- And it exits with code `1`

| home |
|---|
| `string(1, 64, "/adehmo")` |

## B3: Shows the usage line for a name that is too long
- Given the environment variable `NAME` with the value `a` `65` times
- When the program runs
- Then it prints `usage: NAME=<who> greet` and a newline to standard error
- And it exits with code `1`

### For any `name`
- Given the environment variable `NAME` with the value `{name}`
- When the program runs
- Then it prints `usage: NAME=<who> greet` and a newline to standard error
- And it exits with code `1`

| name |
|---|
| `string(65, 100, "ab")` |

# Constraints
- Uses: the environment
