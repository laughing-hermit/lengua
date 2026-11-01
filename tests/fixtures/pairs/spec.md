---
type: lengua/program-spec
title: pairs-stream
description: Counts the pairs of a letter in standard input of any length.
---
# Purpose
Counts the times `aa` comes in standard input of any length, a piece at a time, so input larger
than memory works: each `a` belongs to at most one pair, taken from the left as they come, so
`aaa` holds one pair and `aaaa` two. It prints the count and a newline. Tests give it up to
`65536` bytes. If reading the input fails, the program exits with code 3; if writing the output
fails, with code 4.

# Behaviors
## B1: Counts the pairs
- Given standard input holding `aaa baaaa`
- When the program runs
- Then it prints `3` followed by a newline
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{count(text, "aa")}` followed by a newline
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "ab \n")` | 60 |

# Constraints
- Uses: standard input
