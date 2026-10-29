---
type: lengua/program-spec
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
- Given the input `123456789`
- When the program runs
- Then it prints `3808858755` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: read, write, exit
