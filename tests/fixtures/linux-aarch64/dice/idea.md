---
type: lengua/program-idea
title: dice
description: Rolls a fair six-sided die and prints the roll.
---
# Purpose
The program rolls a fair six-sided die for a board game. It prints the roll, a whole number from
`1` to `6`.

The program reads random bytes from the computer. It uses the first byte that is below `252`. It
divides that byte by 6, and the roll is the remainder plus 1. The program ignores bytes from `252`
to `255`, so each face has the same chance.

If the computer gives no random bytes, the program exits with code 3. It also exits with code 3 if
it ignores all of the first 64 bytes. A fair source almost never gives 64 such bytes. If the
program cannot write its output, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the random bytes `fc` and `2a`
- When the program runs
- Then it prints `rolled 1` and a newline
- And it exits with code `0`

### For any `first` and `second`
- Given the random bytes `{hex(first, 1)}` and `{hex(second, 1)}`
- When the program runs
- Then it prints `rolled {if(first < 252, first % 6, if(second < 252, second % 6, 0)) + 1}` and a newline
- And it exits with code `0`

| first | second | runs |
|---|---|---|
| `int(0, 255)` | `int(0, 255)` | 200 |

# Constraints
- Uses: random bytes
