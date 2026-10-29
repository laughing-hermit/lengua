---
type: lengua/program-spec
title: dice
description: Rolls a fair six-sided die and prints the roll.
---
# Purpose
Rolls a fair six-sided die for a board game and prints the roll, a whole number from `1` to `6`.
It reads random bytes from the kernel and rolls with the first byte below `252`: the roll is that
byte's remainder after dividing by 6, plus 1. Bytes from `252` to `255` are set aside, so every
face is equally likely. If the kernel gives no random bytes, or the first 64 are all set aside
(which a fair kernel practically never does), the program exits with code 3. If writing the
output fails, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the kernel's random bytes `fc` and `2a`
- When the program runs
- Then it prints `rolled 1` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, getrandom, exit
