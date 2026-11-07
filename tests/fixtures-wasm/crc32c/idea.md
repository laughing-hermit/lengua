---
type: Attested Computation
title: crc32c
description: Prints the CRC-32C of standard input of any length, as iSCSI and ext4 compute it.
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
The program computes the CRC-32C (Castagnoli) of standard input of any length, as iSCSI and ext4
compute it, to find changes in stored data. It reads one piece of 64 KiB at a time, so it can read
input that is larger than its memory. It prints the CRC as a whole number in decimal and a
newline. The CRC of empty input is `0`.

The tests give it up to `1048576` bytes. They also try short inputs of up to `64` bytes. If the
program cannot read its input, it exits with code 2. If the program cannot write its output, it
exits with code 3. The test of a write that fails gives it `1` to `64` bytes.

# Behaviors
## B1: Prints the CRC of its input
- Given standard input that contains `123456789`
- When the program runs
- Then it prints `3808858755` and a newline
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{crc32c(text)}` and a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "abcdefghijklmnopqrstuvwxyz0123456789 \n")` | 30 |
| `string(0, 64, "az09 \n~é")` | 300 |

## B2: Prints 0 for empty input
- Given empty standard input
- When the program runs
- Then it prints `0` and a newline
- And it exits with code `0`

## B3: Exits with code 3 when it cannot write its output
- Given standard input that contains `123456789`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

| text | runs |
|---|---|
| `string(1, 64, "abc0123456789")` | 20 |

# Constraints
- Uses: standard input
