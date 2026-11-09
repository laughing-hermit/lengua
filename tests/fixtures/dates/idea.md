---
type: Attested Computation
title: clock
description: Prints the current UTC time as hh:mm:ss, or the number of days until the date in its argument.
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

This program is a small command-line clock. Without arguments, it reads the system clock. Then it
prints the current time of day in UTC as hh:mm:ss, with two digits in each part.

With one date in the form YYYY-MM-DD, it prints the number of whole days from today in UTC to that
date. It prints `0` for today and a negative number for a date in the past.

The clock reads from `0` to `18446744073` seconds since 1970, which is a time in July of the year
2554. The year of a date is from `1970` to `9999`. The month is from `1` to `12`, and the day is
from `1` to `31`. The year always has four digits, and the month and the day have two digits each.
The argument has at most `131072` characters.

The program rejects a date that does not exist on the calendar, such as February 30. If the
program cannot write its output, it exits with code 2.

# Behaviors

## B1: Prints the current time as hh:mm:ss
- Given the clock at `1735736709`
- When the program runs
- Then it prints `13:05:09` and a newline
- And it exits with code `0`

### For any `t`
- Given the clock at `{t}`
- When the program runs
- Then it prints `{if(t%86400/3600 < 10, "0", "")}{t%86400/3600}:{if(t%3600/60 < 10, "0", "")}{t%3600/60}:{if(t%60 < 10, "0", "")}{t%60}{"\n"}`
- And it exits with code `0`

| t | runs |
|---|---|
| `int(0, 18446744073)` | 300 |

## B2: Prints the number of days until a date
- Given the argument `2025-12-25`
- And the clock at `1735689600`
- When the program runs
- Then it prints `358` and a newline
- And it exits with code `0`

### For any `y`, `mt`, `mo`, `dt`, `do` and `t`
- Given the argument `{y}-{mt}{mo}-{dt}{do}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), ((y - if((mt*10 + mo) <= 2, 1, 0))/400*146097 + (y - if((mt*10 + mo) <= 2, 1, 0))%400*365 + (y - if((mt*10 + mo) <= 2, 1, 0))%400/4 - (y - if((mt*10 + mo) <= 2, 1, 0))%400/100 + (153*(((mt*10 + mo) + 9)%12) + 2)/5 + (dt*10 + do) - 1 - 719468 - t/86400), "")}{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), "\n", "")}`
- And it prints `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), "", "invalid date: expected YYYY-MM-DD\n")}` to standard error
- And it exits with code `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), 0, 1)}`

| y | mt | mo | dt | do | t | runs |
|---|---|---|---|---|---|---|
| `int(1970, 9999)` | `choice(0, 1)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | `choice(0, 1, 2, 3)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | `int(0, 18446744073)` | 400 |

## B3: Counts a date in the past as negative days
- Given the argument `2024-12-25`
- And the clock at `1735689600`
- When the program runs
- Then it prints `-7` and a newline
- And it exits with code `0`

### For any `y`, `mt`, `mo`, `dt`, `do` and `t`
- Given the argument `{y}-{mt}{mo}-{dt}{do}`
- And the clock at `{t}`
- When the program runs
- Then it prints `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), ((y - if((mt*10 + mo) <= 2, 1, 0))/400*146097 + (y - if((mt*10 + mo) <= 2, 1, 0))%400*365 + (y - if((mt*10 + mo) <= 2, 1, 0))%400/4 - (y - if((mt*10 + mo) <= 2, 1, 0))%400/100 + (153*(((mt*10 + mo) + 9)%12) + 2)/5 + (dt*10 + do) - 1 - 719468 - t/86400), "")}{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), "\n", "")}`
- And it prints `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), "", "invalid date: expected YYYY-MM-DD\n")}` to standard error
- And it exits with code `{if(((mt*10 + mo) >= 1 && (mt*10 + mo) <= 12 && (dt*10 + do) >= 1 && (dt*10 + do) <= if((mt*10 + mo) == 2, if(((y%4 == 0 && y%100 != 0) || y%400 == 0), 29, 28), if((mt*10 + mo) == 4 || (mt*10 + mo) == 6 || (mt*10 + mo) == 9 || (mt*10 + mo) == 11, 30, 31))), 0, 1)}`

| y | mt | mo | dt | do | t | runs |
|---|---|---|---|---|---|---|
| `int(1970, 9999)` | `choice(0, 1)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | `choice(0, 1, 2, 3)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | `int(0, 18446744073)` | 200 |

## B4: Rejects text that is not a date
- Given the argument `tomorrow`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` and a newline to standard error
- And it exits with code `1`

### For any `s`
- Given the argument `{s}`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` and a newline to standard error
- And it exits with code `1`

| s |
|---|
| `choice("tomorrow", "", "2026-12-2", "2026-12-255", "2026/12/25", "2026-1x-25", "2026-12-2x", "12026-12-25", "-2026-12-25", "2026-12-25 ", "202a-12-25", "1969-12-31", "0000-01-01", "2026--12-5", "+026-12-25")` |

## B5: Rejects a date that is not on the calendar
- Given the argument `2026-02-30`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` and a newline to standard error
- And it exits with code `1`

### For any `y`, `mt`, `mo`, `dt` and `do`
- Given the argument `{y}-{mt}{mo}-{dt}{do}`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` and a newline to standard error
- And it exits with code `1`

| y | mt | mo | dt | do | runs |
|---|---|---|---|---|---|
| `int(1970, 9999)` | `choice(0)` | `choice(2)` | `choice(3)` | `choice(0, 1)` | 50 |
| `int(1970, 9999)` | `choice(0, 1)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | `choice(3)` | `choice(2, 3, 4, 5, 6, 7, 8, 9)` | 100 |
| `int(1970, 9999)` | `choice(0)` | `choice(4, 6, 9)` | `choice(3)` | `choice(1)` | 50 |
| `choice(1970, 1971, 2025, 2026, 2100, 2200, 2300, 2500, 9999)` | `choice(0)` | `choice(2)` | `choice(2)` | `choice(9)` | 20 |
| `int(1970, 9999)` | `choice(1)` | `choice(3, 4, 5, 6, 7, 8, 9)` | `choice(0, 1, 2)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | 100 |
| `int(1970, 9999)` | `choice(0)` | `choice(0)` | `choice(0, 1, 2)` | `choice(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)` | 50 |
| `int(1970, 9999)` | `choice(0, 1)` | `choice(1, 2)` | `choice(0)` | `choice(0)` | 50 |
| `int(1970, 9999)` | `choice(1)` | `choice(1)` | `choice(3)` | `choice(1)` | 20 |

## B6: Shows the usage line for too many arguments
- Given the arguments `2026-12-25` and `2027-01-01`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `3`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` and a newline to standard error
- And it exits with code `3`

| a | b |
|---|---|
| `choice("2026-12-25", "", "x")` | `choice("2027-01-01", "", "tomorrow")` |

# Constraints

- Uses: the clock
