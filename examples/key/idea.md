---
type: Attested Computation
title: key
description: Waits until a person presses a key, and reads the key from the terminal without the Enter key.
runtime: webassembly
computation: main.wat
parameters:
  - { name: keys, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose

A shell script can use this program to stop until a person is ready. The program asks for a key
and then reads one key from its standard input. When its standard input is a terminal, the
program gets the key at once. The person does not press Enter, and the terminal does not show the
key. Then the program puts the terminal back as it was and says that it has the key.

When its standard input is not a terminal, the program reads the first byte of its input as the
key. The input has `1` to `16` bytes, and the program reads only the first byte.

The exit codes for errors are these:
- If the input ends before a key comes, the program exits with code 1.
- If the program cannot read its input or write its output, it exits with code 4.

# Behaviors

## B1: Reads one key
- Given standard input that contains `x`
- When the program runs
- Then it prints `Press a key.\nYou pressed a key.\n`
- And it exits with code `0`

### For any `keys`
- Given standard input that contains `{keys}`
- When the program runs
- Then it prints `Press a key.\nYou pressed a key.\n`
- And it exits with code `0`

| keys |
|---|
| `string(1, 16, "abcdefghijklmnopqrstuvwxyz0123456789 \n")` |

## B2: Says when no key comes
- Given empty standard input
- When the program runs
- Then it prints `Press a key.` and a newline
- And it prints `key: no key came` and a newline to standard error
- And it exits with code `1`

# Constraints

- Uses: the terminal
