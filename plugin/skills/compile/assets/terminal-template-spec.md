---
type: lengua/program-spec
title: keys
description: Shows each key as it's pressed, in capitals, until q.
---
# Purpose
An interactive program for a terminal: it shows each key as it's pressed, in capitals, without
waiting for Enter or echoing it, until q is pressed or the input ends. In a terminal it reads keys
one at a time, hides the cursor while it runs, and sets both back at the end; between keys it
waits a tenth of a second at a time, as a game waits between its moves. Letters are `a` to `z`;
other keys show as they are. Tests give it up to `64` keys.

# Behaviors
## B1: Shows each key in capitals
- Given standard input holding `ab`
- When the program runs
- Then it prints `\e[?25lAB\e[?25h`
- And it exits with code `0`

### For any `keys`
- Given standard input holding `{keys}`
- When the program runs
- Then it prints `\e[?25l{upper(keys)}\e[?25h`
- And it exits with code `0`

| keys |
|---|
| `string(0, 64, "abcxyz019 ")` |

## B2: Stops at q
- Given standard input holding `xqz`
- When the program runs
- Then it prints `\e[?25lX\e[?25h`
- And it exits with code `0`

### For any `before` and `after`
- Given standard input holding `{before}q{after}`
- When the program runs
- Then it prints `\e[?25l{upper(before)}\e[?25h`
- And it exits with code `0`

| before | after |
|---|---|
| `string(0, 64, "abcxyz019 ")` | `string(0, 64, "abcxyz019 ")` |

# Constraints
- Uses: the terminal
