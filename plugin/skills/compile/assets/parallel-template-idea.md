---
type: lengua/program-idea
title: shout-parallel
description: Prints its input with every ASCII letter in capitals, on several threads at once.
---
# Purpose

A person can use this program to shout a large text fast. It reads up to `1048576` bytes from
standard input. It prints them with each ASCII letter from a to z changed to its capital. It keeps
all other bytes as they are. If the program cannot read its input or write its output, it exits
with code 2.

# Behaviors

## B1: Capitalizes the input
- Given standard input that contains `Hello, World!` and a newline
- When the program runs
- Then it prints `HELLO, WORLD!` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text |
|---|
| `` string(0, 1048576, "Helo, Wrd!az AZ`{@[\n") `` |

## B2: Rejects input that is too long
- Given standard input that contains `a` `1048576` times then `x`
- When the program runs
- Then it prints `shout: the input is longer than 1048576 bytes` and a newline to standard error
- And it exits with code `1`

### For any `text`
- Given standard input that contains `{text}x`
- When the program runs
- Then it prints `shout: the input is longer than 1048576 bytes` and a newline to standard error
- And it exits with code `1`

| text | runs |
|---|---|
| `string(1048576, 1048576, "ab")` | 5 |

# Constraints

- Uses: standard input
- Threads: up to 16
