---
type: lengua/program-spec
title: cat-stream
description: Prints standard input of any length back unchanged.
---
# Purpose
Copies standard input of any length to standard output unchanged, a piece at a time, so input
larger than memory works. Tests give it up to `9000` bytes. If reading the input fails, the
program exits with code 3; if writing the output fails, with code 4.

# Behaviors
## B1: Prints standard input back
- Given standard input holding `hello`
- When the program runs
- Then it prints `hello`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{text}`
- And it exits with code `0`

| text |
|---|
| `string(0, 9000, "abehlo \n")` |

## B2: Exits with code 4 when its output can't be written
- Given standard input holding `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `text`
- Given standard input holding `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `{if(len(text) == 0, 0, 4)}`

| text |
|---|
| `string(0, 9000, "abehlo \n")` |

# Constraints
- Uses: standard input
