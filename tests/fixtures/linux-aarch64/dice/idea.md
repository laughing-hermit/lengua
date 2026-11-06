---
type: lengua/program-idea
title: dice
description: Rolls a fair six-sided die and prints the roll.
---
# Purpose
Rolls a fair six-sided die for a board game and prints the roll, a whole number from `1` to `6`.
It reads random bytes from the computer and rolls with the first byte below `252`: the roll is
that byte's remainder after dividing by 6, plus 1. Bytes from `252` to `255` are set aside, so
every face is equally likely. If the computer gives no random bytes, or the first 64 are all set
aside (which a fair source practically never does), the program exits with code 3. If writing
the output fails, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the random bytes `fc` and `2a`
- When the program runs
- Then it prints `rolled 1` followed by a newline
- And it exits with code `0`

### For any `first` and `second`
- Given the random bytes `{hex(first, 1)}` and `{hex(second, 1)}`
- When the program runs
- Then it prints `rolled {if(first < 252, first % 6, if(second < 252, second % 6, 0)) + 1}` followed by a newline
- And it exits with code `0`

| first | second | runs |
|---|---|---|
| `int(0, 255)` | `int(0, 255)` | 200 |

# Constraints
- Uses: random bytes
