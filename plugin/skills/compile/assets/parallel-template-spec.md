---
type: lengua/program-spec
title: shout-parallel
description: Prints its input with every ASCII letter in capitals, on several threads at once.
---
# Purpose

Lets someone shout a large text fast: it reads up to `1048576` bytes from standard input and
prints them with every ASCII letter from a to z turned into its capital, and every other byte left
as it is. If reading the input or writing the output fails, the program exits with code 2.

# Behaviors

## B1: Capitalizes the input
- Given standard input holding `Hello, World!` followed by a newline
- When the program runs
- Then it prints `HELLO, WORLD!` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `` string(0, 1048576, "Helo, Wrd!az AZ`{@[\n") `` |

## B2: Rejects input that is too long
- Given standard input holding `a` repeated `1048576` times followed by `x`
- When the program runs
- Then it prints `shout: the input is longer than 1048576 bytes` followed by a newline to standard error
- And it exits with code `1`

### For any `text`
- Given standard input holding `{text}x`
- When the program runs
- Then it prints `shout: the input is longer than 1048576 bytes` followed by a newline to standard error
- And it exits with code `1`

| text | runs |
|---|---|
| `string(1048576, 1048576, "ab")` | 5 |

# Constraints

- Uses: standard input
- Threads: up to 16
