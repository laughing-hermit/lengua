---
type: lengua/program-idea
title: crc32c
description: Prints the CRC-32C checksum of standard input.
---
# Purpose
Checks that data arrived intact: it reads standard input of any length, a piece at a time, and
prints its CRC-32C, the Castagnoli checksum that iSCSI, ext4 and many file formats use, as a
decimal number followed by a newline, fast on very large inputs. Tests give it up to `1048576`
bytes. If reading the input or writing the output fails, the program exits with code 2.

# Behaviors
## B1: Prints the checksum
- Given standard input holding `123456789`
- When the program runs
- Then it prints `3808858755` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{crc32c(text)}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 1048576, "0123456789abcdefghijklmnopqrstuvwxyz \n")` | 200 |

# Constraints
- Uses: standard input
