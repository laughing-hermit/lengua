---
type: lengua/program-idea
title: cat
description: Prints standard input back unchanged.
---
# Purpose
The program copies standard input of `0` to `5000` bytes to standard output without a change. If
the program cannot read its input or write its output, it exits with code 3.

# Behaviors
## B1: Prints standard input back
- Given standard input that contains `hello`
- When the program runs
- Then it prints `hello`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{text}`
- And it exits with code `0`

| text |
|---|
| `string(0, 5000, "abehlo \n")` |

## B2: Exits with code 3 when it cannot write its output
- Given standard input that contains `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `3`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `{if(len(text) == 0, 0, 3)}`

| text |
|---|
| `string(0, 5000, "abehlo \n")` |

# Constraints
- Uses: standard input
