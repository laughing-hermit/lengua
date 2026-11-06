---
type: lengua/program-idea
title: cat-stream
description: Prints standard input of any length back unchanged.
---
# Purpose
The program copies standard input of any length to standard output without a change. It copies
one piece at a time, so it can copy input that is larger than its memory. The tests give it up to
`9000` bytes. If the program cannot read its input, it exits with code 3. If the program cannot
write its output, it exits with code 4.

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
| `string(0, 9000, "abehlo \n")` |

## B2: Exits with code 4 when it cannot write its output
- Given standard input that contains `hello`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `text`
- Given standard input that contains `{text}`
- And standard output is closed
- When the program runs
- Then it exits with code `{if(len(text) == 0, 0, 4)}`

| text |
|---|
| `string(0, 9000, "abehlo \n")` |

# Constraints
- Uses: standard input
