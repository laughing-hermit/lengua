---
type: Attested Computation
title: clock
description: Prints the current UTC time as hh:mm:ss, or the number of days from today to the date in its argument.
runtime: webassembly
computation: main.wat
parameters:
  - { name: date, type: text, required: false, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a small command-line clock. Without arguments, it reads the clock. Then it prints
the current time of day in UTC as hh:mm:ss, with two digits in each part. The hours are from 00 to
23.

With one date in the form YYYY-MM-DD, it prints the number of days from the UTC date today to that
date. It prints a positive number for a date in the future and `0` for today. For a date in the
past, it prints a negative number with a minus sign at the start. The clock reads from `0` to
`18446744073` seconds since 1970, which is a time in July of the year 2554.

A date is four digits, a dash, two digits, a dash and two digits. For any other text, the program
prints the usage message.

A date in that form names a real day if three things are true. Its year is from `1970` to `9999`.
Its month is from 01 to 12. Its day is from 01 to the last day of that month. For any other date
in that form, the program prints the "no such date" message.

February has 29 days in a leap year. A leap year is a year that 4 divides, but a century is a leap
year only if 400 divides it.

Digit by digit, a real month is `0` then `1` to `9`, or `1` then `0` to `2`. A day from 01 to 28 is
`0` then `1` to `9`, or `1` or `2` then `0` to `8`, or 19. Days 29 to 31 start with `2` or `3`. An
argument has at most `131072` characters. If the program cannot write its output, it exits with
code 4.

# Behaviors
## B1: Prints the current time
- Given the clock at `1793264707`
- When the program runs
- Then it prints `09:05:07` and a newline
- And it exits with code `0`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints `{if(t % 86400 / 3600 < 10, "0", "")}{t % 86400 / 3600}:{if(t % 3600 / 60 < 10, "0", "")}{t % 3600 / 60}:{if(t % 60 < 10, "0", "")}{t % 60}` and a newline
- And it exits with code `0`

| t | runs |
|---|---|
| `int(0, 18446744073)` | 300 |

## B2: Prints the days until a date
- Given the argument `2026-12-25`
- And the clock at `1793264707`
- When the program runs
- Then it prints `57` and a newline
- And it exits with code `0`

### For any `y`, `m1`, `m2`, `d1`, `d2` and `t`
- Given the argument `{y}-{m1}{m2}-{d1}{d2}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{(((y - if((m1 * 10 + m2) <= 2, 1, 0)) / 400) * 146097 + ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) * 365 + ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) / 4 - ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) / 100 + ((153 * (((m1 * 10 + m2) + 9) % 12) + 2) / 5 + (d1 * 10 + d2) - 1) - 719468) - t / 86400}` and a newline
- And it exits with code `0`

| y | m1 | m2 | d1 | d2 | t | runs |
|---|---|---|---|---|---|---|
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(0, 0)` | `int(1, 9)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(1, 2)` | `int(0, 8)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(0, 2)` | `int(0, 0)` | `int(1, 9)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(0, 2)` | `int(1, 2)` | `int(0, 8)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(1, 1)` | `int(9, 9)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(2, 2)` | `int(3, 3)` | `int(0, 1)` | `int(0, 18446744073)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 1)` | `int(3, 3)` | `int(0, 1)` | `int(0, 18446744073)` | 150 |

## B3: Rejects text that is not a date
- Given the argument `christmas`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `1`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `1`

| s |
|---|
| `choice("christmas", "", "2026-12-2", "2026-12-255", "2026/12/25", "2026-1x-25", "20261225", "-2026-12-25", "2026-12-25 ", "202a-12-25", "12026-12-25", "2026-12-2x")` |

## B4: Rejects a day that does not exist
- Given the argument `2026-02-29`
- When the program runs
- Then it prints `clock: no such date: 2026-02-29` and a newline to standard error
- And it exits with code `2`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `clock: no such date: {s}` and a newline to standard error
- And it exits with code `2`

| s |
|---|
| `choice("2026-02-29", "2100-02-29", "2026-04-31", "2026-06-31", "2026-00-10", "2026-13-01", "2026-01-00", "2026-01-32", "1969-12-31", "0000-01-01", "2026-99-99", "2026-12-32")` |

## B5: Rejects extra arguments
- Given the arguments `2026-12-25` and `2027-01-01`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `1`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `1`

| a | b |
|---|---|
| `choice("2026-12-25", "christmas", "")` | `choice("2027-01-01", "x", "")` |

# Constraints
- Uses: the clock
