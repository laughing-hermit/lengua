---
type: Attested Computation
title: checksums
description: Prints the CRC-32 of each line of standard input.
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
A shell script can use this program to find the lines of a text that changed. The program reads
standard input and prints the CRC-32 of each line, as zlib, gzip and PNG compute it. It prints
each CRC-32 as a whole number in decimal, on a line of its own. A line ends at a newline or at
the end of the input, and the newline is not a part of the line.

The input has at most `4096` bytes, and input of `4097` to `5000` bytes is too long. The tests
give each line `0` to `24` bytes, and a last line that has no newline `1` to `24` bytes. If the
program cannot read its input, it exits with code 3. If the program cannot write its output, it
exits with code 4.

The program is made from three parts of the library of lengua. It finds the lines with the part
lines, computes each CRC-32 with the part crc32, and writes the numbers with the part decimal.

# Behaviors
## B1: Prints the CRC-32 of each line
- Given standard input that contains `123456789\nabc\n`
- When the program runs
- Then it prints `3421780262\n891568578\n`
- And it exits with code `0`

### For any two lines
- Given standard input that contains `{a}\n{b}\n`
- When the program runs
- Then it prints `{crc32(a)}\n{crc32(b)}\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | `string(0, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | 200 |

## B2: Takes a last line that has no newline
- Given standard input that contains `123456789\nabc`
- When the program runs
- Then it prints `3421780262\n891568578\n`
- And it exits with code `0`

### For any two lines
- Given standard input that contains `{a}\n{b}`
- When the program runs
- Then it prints `{crc32(a)}\n{crc32(b)}\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | `string(1, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | 100 |

## B3: Prints nothing for empty input
- Given empty standard input
- When the program runs
- Then it prints nothing
- And it exits with code `0`

## B4: Rejects input that is too long
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

## B5: Exits with code 4 when it cannot write its output
- Given standard input that contains `123456789\nabc\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any two lines
- Given standard input that contains `{a}\n{b}\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| a | b | runs |
|---|---|---|
| `string(0, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | `string(0, 24, "abcdefghijklmnopqrstuvwxyz0123456789 ")` | 20 |

# Constraints
- Uses: standard input
- Builds on: [lines](../lines/idea.md), [crc32](../crc32/idea.md), [decimal](../decimal/idea.md)
