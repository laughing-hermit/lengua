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

# Constraints
- Uses: standard input
