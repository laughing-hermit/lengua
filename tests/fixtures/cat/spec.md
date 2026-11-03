---
type: lengua/program-spec
title: cat
description: Prints standard input back unchanged.
---
# Purpose
Copies standard input of `0` to `5000` bytes to standard output unchanged. If reading or writing
fails, the program exits with code 3.

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
| `string(0, 5000, "abehlo \n")` |

## B2: Exits with code 3 when its output can't be written
- Given standard input holding `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `text`
- Given standard input holding `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `{if(len(text) == 0, 0, 3)}`

| text |
|---|
| `string(0, 5000, "abehlo \n")` |

# Constraints
- Uses: standard input
