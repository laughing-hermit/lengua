---
type: Attested Computation
title: shout
description: Prints standard input of up to 1048576 bytes with every ASCII letter in capitals.
runtime: webassembly
computation: main.wat
parameters:
  - { name: text, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A shell script can use this program to shout a text. The program reads all of its standard input,
up to `1048576` bytes. Then it prints the input with each ASCII letter `a` to `z` in capitals. It
does not change the other bytes, and this includes the bytes of UTF-8 characters.

The tests also try short inputs of up to `64` bytes with the characters next to the letters. The
program rejects input that is longer than the limit, and it prints no part of it. If the program
cannot read its input or cannot write its output, it exits with code 2. The test of a write that
fails gives it `1` to `64` bytes.

# Behaviors
## B1: Shouts its input
- Given standard input that contains `hello, world` and a newline
- When the program runs
- Then it prints `HELLO, WORLD` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abcdehlorwxyzABZ019 ,.\n")` | 30 |
| `string(0, 64, "az{}@[AZ09 \n~é")` | 300 |

## B2: Rejects input that is too long
- Given standard input that contains `a` `1048576` times then `b`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `1`

### For any input past the limit
- Given standard input that contains `{head}` then `b`
- When the program runs
- Then it prints `error: input longer than 1048576 bytes` and a newline to standard error
- And it exits with code `1`

| head | runs |
|---|---|
| `string(1048576, 1048576, "ab \n")` | 10 |

## B3: Exits with code 2 when it cannot write its output
- Given standard input that contains `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `2`

| text | runs |
|---|---|
| `string(1, 64, "abcdehlorwxyz019 ,.")` | 20 |

# Constraints
- Uses: standard input
