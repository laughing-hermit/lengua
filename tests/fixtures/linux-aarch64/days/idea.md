---
type: lengua/program-idea
title: clock
description: Prints the current UTC time as hh:mm:ss, or the number of days from today until a given date.
---
# Purpose
A small command-line clock. Run with no arguments, it reads the clock and prints the current time
of day in UTC as hh:mm:ss, each part two digits and the hours from 00 to 23. Run with one date
written as YYYY-MM-DD, it prints how many days lie between today's UTC date and that date: a
positive number for a future date, `0` for today and a negative number, with a leading minus
sign, for a past date. The clock reads from `0` to `253402300799` seconds since 1970 (the end of
the year 9999).

A date is four digits, a dash, two digits, a dash and two digits; anything else gets the usage
message. A date in that shape names a real day when its year runs from `1970` to `9999`, its
month from 01 to 12 and its day from 01 to the last day of that month, counting February 29 in
leap years (years divisible by 4, except centuries not divisible by 400); any other gets the
"no such date" message. Written digit by digit, a real month is `0` then `1` to `9`, or `1`
then `0` to `2`; a day from 01 to 28 is `0` then `1` to `9`, or `1` or `2` then `0` to `8`, or
19; days 29 to 31 begin with `2` or `3`. An argument is at most `131072` characters. If writing
the output fails, the program exits with code 4.

# Behaviors
## B1: Prints the current time
- Given the clock at `1793264707`
- When the program runs
- Then it prints `09:05:07` followed by a newline
- And it exits with code `0`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints `{if(t % 86400 / 3600 < 10, "0", "")}{t % 86400 / 3600}:{if(t % 3600 / 60 < 10, "0", "")}{t % 3600 / 60}:{if(t % 60 < 10, "0", "")}{t % 60}` followed by a newline
- And it exits with code `0`

| t | runs |
|---|---|
| `int(0, 253402300799)` | 300 |

## B2: Prints the days until a date
- Given the argument `2026-12-25`
- And the clock at `1793264707`
- When the program runs
- Then it prints `57` followed by a newline
- And it exits with code `0`

### For any `y`, `m1`, `m2`, `d1`, `d2` and `t`
- Given the argument `{y}-{m1}{m2}-{d1}{d2}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{(((y - if((m1 * 10 + m2) <= 2, 1, 0)) / 400) * 146097 + ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) * 365 + ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) / 4 - ((y - if((m1 * 10 + m2) <= 2, 1, 0)) % 400) / 100 + ((153 * (((m1 * 10 + m2) + 9) % 12) + 2) / 5 + (d1 * 10 + d2) - 1) - 719468) - t / 86400}` followed by a newline
- And it exits with code `0`

| y | m1 | m2 | d1 | d2 | t | runs |
|---|---|---|---|---|---|---|
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(0, 0)` | `int(1, 9)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(1, 2)` | `int(0, 8)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(0, 2)` | `int(0, 0)` | `int(1, 9)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(0, 2)` | `int(1, 2)` | `int(0, 8)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 9)` | `int(1, 1)` | `int(9, 9)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(1, 1)` | `int(2, 2)` | `int(3, 3)` | `int(0, 1)` | `int(0, 253402300799)` | 150 |
| `int(1970, 9999)` | `int(0, 0)` | `int(1, 1)` | `int(3, 3)` | `int(0, 1)` | `int(0, 253402300799)` | 150 |

## B3: Rejects text that is not a date
- Given the argument `christmas`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

| s |
|---|
| `choice("christmas", "", "2026-12-2", "2026-12-255", "2026/12/25", "2026-1x-25", "20261225", "-2026-12-25", "2026-12-25 ", "202a-12-25", "12026-12-25", "2026-12-2x")` |

## B4: Rejects a day that does not exist
- Given the argument `2026-02-29`
- When the program runs
- Then it prints `clock: no such date: 2026-02-29` followed by a newline to standard error
- And it exits with code `2`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `clock: no such date: {s}` followed by a newline to standard error
- And it exits with code `2`

| s |
|---|
| `choice("2026-02-29", "2100-02-29", "2026-04-31", "2026-06-31", "2026-00-10", "2026-13-01", "2026-01-00", "2026-01-32", "1969-12-31", "0000-01-01", "2026-99-99", "2026-12-32")` |

## B5: Rejects extra arguments
- Given the arguments `2026-12-25` and `2027-01-01`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

| a | b |
|---|---|
| `choice("2026-12-25", "christmas", "")` | `choice("2027-01-01", "x", "")` |

# Constraints
- Uses: the clock
