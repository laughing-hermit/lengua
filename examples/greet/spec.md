---
type: lengua/program-spec
title: greet
description: Greets whoever the NAME environment variable names.
---
# Purpose
Greets the person the `NAME` environment variable names, for a shell's welcome line. A name holds
`1` to `64` characters. Without one, or with a name of `65` to `100` characters or more, it prints
how to use it to standard error and exits with code 1. If writing fails, it exits with code 3.

# Behaviors
## B1: Greets whoever NAME names
- Given the environment variable `NAME` set to `Ada`
- When the program runs
- Then it prints `Hello, Ada!` followed by a newline
- And it exits with code `0`

### For any `name`
- Given the environment variable `NAME` set to `{name}`
- When the program runs
- Then it prints `Hello, {name}!` followed by a newline
- And it exits with code `0`

| name |
|---|
| `string(1, 64, "Adabcz -")` |

## B2: Shows usage without a name
- Given the environment variable `HOME` set to `/home/ada`
- When the program runs
- Then it prints `usage: NAME=<who> greet` followed by a newline to standard error
- And it exits with code `1`

### For any `home`
- Given the environment variable `HOME` set to `{home}`
- When the program runs
- Then it prints `usage: NAME=<who> greet` followed by a newline to standard error
- And it exits with code `1`

| home |
|---|
| `string(1, 64, "/adehmo")` |

## B3: Shows usage for a name too long
- Given the environment variable `NAME` set to `a` repeated `65` times
- When the program runs
- Then it prints `usage: NAME=<who> greet` followed by a newline to standard error
- And it exits with code `1`

### For any `name`
- Given the environment variable `NAME` set to `{name}`
- When the program runs
- Then it prints `usage: NAME=<who> greet` followed by a newline to standard error
- And it exits with code `1`

| name |
|---|
| `string(65, 100, "ab")` |

# Constraints
- Uses: the environment
