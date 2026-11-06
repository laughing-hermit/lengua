---
type: lengua/program-idea
title: keys
description: Shows each key that the user presses in capitals, until the user presses q.
---
# Purpose
This program is an interactive program for a terminal. It shows each key that the user presses, in
capitals. It does not wait for Enter, and it does not echo the key. It stops when the user presses
q or when the input ends.

In a terminal, the program reads keys one at a time and hides the cursor while it runs. At the end,
it puts the terminal back as it was and shows the cursor again. Between keys, it waits a tenth of a
second at a time, as a game waits between its moves.

Letters are `a` to `z`, and other keys show as they are. The tests give it up to `64` keys.

# Behaviors
## B1: Shows each key in capitals
- Given standard input that contains `ab`
- When the program runs
- Then it prints `\e[?25lAB\e[?25h`
- And it exits with code `0`

### For any `keys`
- Given standard input that contains `{keys}`
- When the program runs
- Then it prints `\e[?25l{upper(keys)}\e[?25h`
- And it exits with code `0`

| keys |
|---|
| `string(0, 64, "abcxyz019 ")` |

## B2: Stops at q
- Given standard input that contains `xqz`
- When the program runs
- Then it prints `\e[?25lX\e[?25h`
- And it exits with code `0`

### For any `before` and `after`
- Given standard input that contains `{before}q{after}`
- When the program runs
- Then it prints `\e[?25l{upper(before)}\e[?25h`
- And it exits with code `0`

| before | after |
|---|---|
| `string(0, 64, "abcxyz019 ")` | `string(0, 64, "abcxyz019 ")` |

# Constraints
- Uses: the terminal
