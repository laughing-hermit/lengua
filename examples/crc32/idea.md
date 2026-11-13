---
type: Attested Computation
title: crc32
description: A part that computes the CRC-32 of bytes, and prints the CRC-32 of standard input.
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
This program is a part of the library of lengua. Other programs take its function, which computes
the CRC-32 of bytes as zlib, gzip and PNG compute it, to find changes in data.

The program itself prints the CRC-32 of standard input as a whole number in decimal and a
newline. The CRC-32 of empty input is `0`. The input has at most `4096` bytes, and input of `4097`
to `5000` bytes is too long. The tests also try short inputs of up to `64` bytes.

If the program cannot read its input, it exits with code 3. If the program cannot write its
output, it exits with code 4. The test of a write that fails gives it `1` to `64` bytes.

# Behaviors
## B1: Prints the CRC-32 of its input
- Given standard input that contains `123456789`
- When the program runs
- Then it prints `3421780262` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{crc32(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 4096, "abcdefghijklmnopqrstuvwxyz0123456789 \n")` | 60 |
| `string(0, 64, "az09 \n~é")` | 300 |

## B2: Prints 0 for empty input
- Given empty standard input
- When the program runs
- Then it prints `0` and a newline
- And it exits with code `0`

## B3: Rejects input that is too long
- Given standard input that contains `a` `4097` times
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

### For any long `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

| text | runs |
|---|---|
| `string(4097, 5000, "ab\n")` | 20 |

## B4: Exits with code 4 when it cannot write its output
- Given standard input that contains `123456789`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| text | runs |
|---|---|
| `string(1, 64, "abc0123456789")` | 20 |

# Constraints
- Uses: standard input
- Builds on: [decimal](../decimal/idea.md)

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies the
function from main.wat, word for word, under a `;; from: ../crc32` line, with the `$CRC32_`
constants that the function reads.

- `$crc32_update`: reads 4096 bytes at `$at`; takes `$crc` from 0 to 0; takes `$length` from 0
  to 64; `$at` holds `$length` bytes; gives `crc32(at)`. Its arguments are `$crc`, `$at` and
  `$length`. It gives the CRC-32 of some bytes and then the `$length` bytes at `$at`, where
  `$crc` is the CRC-32 of the bytes before them, and 0 when there are none. So one call with 0
  gives the CRC-32 of the bytes at `$at`, and a call for each piece gives the CRC-32 of a longer
  text. `$length` is at most 4096: the function stops the program with assertion 831 when it is
  not. The caller lends `$at` with room for 4096 bytes, whatever `$length` is. The formula
  `crc32(at)` is for one call with 0 and a text of at most 64 bytes. `check` proves the formula
  of the function alone. The proofs of a program that takes the function use the formula at
  such a call.
