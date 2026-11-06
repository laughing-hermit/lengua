---
type: lengua/program-idea
title: shout-stream
description: Copies standard input of any length to standard output with every ASCII letter in capitals.
---
# Purpose
A filter for shell pipelines: it copies standard input of any length to standard output with each
ASCII letter `a` to `z` in capitals, a piece at a time, so input larger than memory works. Other
bytes pass through unchanged. Tests give it up to `65536` bytes. If reading the input fails, the
program exits with code 3; if writing the output fails, with code 4.

# Behaviors
## B1: Shouts its input
- Given standard input holding `hello, world`
- When the program runs
- Then it prints `HELLO, WORLD`
- And it exits with code `0`

### For any `text`
- Given standard input holding `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "abcdehlorwxyz019 ,.")` | 60 |

# Constraints
- Uses: standard input
