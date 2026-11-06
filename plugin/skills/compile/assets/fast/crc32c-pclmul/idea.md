---
type: lengua/program-idea
title: crc32c
description: Prints the CRC-32C checksum of standard input.
---
# Purpose
The program checks that data arrived intact. It reads standard input of any length, one piece at a
time. It prints the CRC-32C of the input as a decimal number and a newline. CRC-32C is the
Castagnoli checksum that iSCSI, ext4 and many file formats use.

The program is fast on very large inputs. The tests give it up to `1048576` bytes. If the program
cannot read its input or write its output, it exits with code 2.

# Behaviors
## B1: Prints the checksum
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
| `string(0, 1048576, "0123456789abcdefghijklmnopqrstuvwxyz \n")` | 200 |

# Constraints
- Uses: standard input
