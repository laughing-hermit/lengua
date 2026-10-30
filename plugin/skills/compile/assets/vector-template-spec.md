---
type: lengua/program-spec
title: shout
description: Reads a line from standard input and prints it in capital letters.
---
# Purpose
Lets a shell script shout a line of text: it reads one line of `1` to `64` lowercase letters and
spaces from standard input and prints it in capital letters. The line ends at a newline or at the
end of the input. A line of `65` to `4096` characters is too long. If reading the input fails,
the program exits with code 3. If writing the output fails, the program exits with code 4.

# Behaviors
## B1: Shouts a line
- Given standard input holding `hello world` followed by a newline
- When the program runs
- Then it prints `HELLO WORLD` followed by a newline
- And it exits with code `0`

### For any `line`
- Given standard input holding `{line}` followed by a newline
- When the program runs
- Then it prints `{upper(line)}` followed by a newline
- And it exits with code `0`

| line | runs |
|---|---|
| `string(1, 64, "abcdefghijklmnopqrstuvwxyz ")` | 200 |

## B2: Rejects empty input
- Given no arguments
- When the program runs
- Then it prints `error: no input` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects a line that is too long
- Given standard input holding `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` followed by a newline
- When the program runs
- Then it prints `error: line too long` followed by a newline to standard error
- And it exits with code `2`

### For any `line`
- Given standard input holding `{line}` followed by a newline
- When the program runs
- Then it prints `error: line too long` followed by a newline to standard error
- And it exits with code `2`

| line | runs |
|---|---|
| `string(65, 4096, "abcdefghijklmnopqrstuvwxyz ")` | 50 |

# Constraints
- Uses: standard input
