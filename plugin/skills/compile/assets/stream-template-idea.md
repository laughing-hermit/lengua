---
type: lengua/program-idea
title: shout-stream
description: Copies standard input of any length to standard output with every ASCII letter in capitals.
---
# Purpose
This program is a filter for shell pipelines. It copies standard input of any length to standard
output, with each ASCII letter `a` to `z` in capitals. It copies one piece at a time, so it can
copy input that is larger than its memory. It does not change the other bytes.

The tests give it up to `65536` bytes. If the program cannot read its input, it exits with code 3.
If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Shouts its input
- Given standard input that contains `hello, world`
- When the program runs
- Then it prints `HELLO, WORLD`
- And it exits with code `0`

### For any `text`
- Given standard input that contains `{text}`
- When the program runs
- Then it prints `{upper(text)}`
- And it exits with code `0`

| text | runs |
|---|---|
| `string(0, 65536, "abcdehlorwxyz019 ,.")` | 60 |

# Constraints
- Uses: standard input
