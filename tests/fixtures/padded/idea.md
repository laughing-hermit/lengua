---
type: lengua/program-idea
title: wc
description: Counts the lines and words on standard input like `wc -lw`, as fast as possible.
---
# Purpose
This program is a drop-in replacement for `wc -lw` on very large files. It is for people who count
the lines and words in huge logs and datasets. They want the answer as fast as the machine can give
it.

The program reads standard input of any length, one piece at a time. To count a file, use
`./wc < big.txt`. The tests use standard input of up to `1048576` bytes. They also use short texts
of up to `64` characters with letters such as `é` that take two bytes.

The program counts one line for each newline byte. A word is a run of one or more bytes that are
not whitespace. Whitespace is the space, tab, newline, vertical tab, form feed and carriage return
bytes. Every other byte is part of a word, also the control bytes and the bytes above 127.

The program prints the line count, a space, the word count and a newline. It aligns each count to
the right in a field of 7 characters, with spaces on the left. It prints a count that is longer
than 7 digits in full. This is exactly what GNU `wc -lw` prints for standard input.

If the program cannot read its input, it exits with code 1. If the program cannot write its output,
it exits with code 2.

# Behaviors
## B1: Counts lines and words
- Given standard input that contains `hello world\nfoo bar baz\n`
- When the program runs
- Then it prints `      2       5` and a newline
- And it exits with code `0`

### For any `s`
- Given standard input that contains `{s}`
- When the program runs
- Then it prints `{if(count(s, "\n") < 10, "      ", if(count(s, "\n") < 100, "     ", if(count(s, "\n") < 1000, "    ", if(count(s, "\n") < 10000, "   ", if(count(s, "\n") < 100000, "  ", if(count(s, "\n") < 1000000, " ", ""))))))}{count(s, "\n")} {if(words(s) < 10, "      ", if(words(s) < 100, "     ", if(words(s) < 1000, "    ", if(words(s) < 10000, "   ", if(words(s) < 100000, "  ", if(words(s) < 1000000, " ", ""))))))}{words(s)}` and a newline
- And it exits with code `0`

| s | runs |
|---|---|
| `string(0, 1048576, "abcdefghijklmnopqrstuvwxyz   \n\n\t\r~")` | 300 |
| `string(0, 1048576, "abdefhlorwz \n\n\n")` | 300 |

## B2: Treats any whitespace as a word break
- Given standard input that contains `a\tb\r\ncde  f`
- When the program runs
- Then it prints `      1       6` and a newline
- And it exits with code `0`

### For any `s`
- Given standard input that contains `{s}`
- When the program runs
- Then it prints `{if(count(s, "\n") < 10, "      ", if(count(s, "\n") < 100, "     ", if(count(s, "\n") < 1000, "    ", if(count(s, "\n") < 10000, "   ", if(count(s, "\n") < 100000, "  ", if(count(s, "\n") < 1000000, " ", ""))))))}{count(s, "\n")} {if(words(s) < 10, "      ", if(words(s) < 100, "     ", if(words(s) < 1000, "    ", if(words(s) < 10000, "   ", if(words(s) < 100000, "  ", if(words(s) < 1000000, " ", ""))))))}{words(s)}` and a newline
- And it exits with code `0`

| s | runs |
|---|---|
| `string(0, 1048576, "abcdef  \t\n\r")` | 200 |

## B3: Counts every other byte as part of a word
- Given standard input that contains `café déjà`
- When the program runs
- Then it prints `      0       2` and a newline
- And it exits with code `0`

### For any `s`
- Given standard input that contains `{s}`
- When the program runs
- Then it prints `{if(count(s, "\n") < 10, "      ", if(count(s, "\n") < 100, "     ", if(count(s, "\n") < 1000, "    ", if(count(s, "\n") < 10000, "   ", if(count(s, "\n") < 100000, "  ", if(count(s, "\n") < 1000000, " ", ""))))))}{count(s, "\n")} {if(words(s) < 10, "      ", if(words(s) < 100, "     ", if(words(s) < 1000, "    ", if(words(s) < 10000, "   ", if(words(s) < 100000, "  ", if(words(s) < 1000000, " ", ""))))))}{words(s)}` and a newline
- And it exits with code `0`

| s | runs |
|---|---|
| `string(0, 64, "acfdejéà€ \n")` | 300 |

## B4: Prints zeros for empty input
- Given no arguments
- When the program runs
- Then it prints `      0       0` and a newline
- And it exits with code `0`

# Constraints
- Uses: standard input
