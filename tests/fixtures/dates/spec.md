---
type: lengua/program-spec
title: clock
description: Prints the current UTC time as hh:mm:ss, or the number of days until a date given as an argument.
---
# Purpose

A small command-line clock. Run with no arguments, it reads the system clock and prints the
current time of day in UTC as hh:mm:ss, each part two digits. Run with one date written as
YYYY-MM-DD, it prints how many whole days lie between today's date in UTC and that date: `0` for
today and a negative number for a date in the past. The clock reads from `0` to `253402300799`
seconds since 1970 (up to the end of year 9999). A date's year is from `1970` to `9999`, its month
from `1` to `12` and its day from `1` to `31`, always written with four, two and two digits; the
argument is at most `131072` characters. A date that does not exist on the calendar, such as
February 30, is rejected. If writing the output fails, the program exits with code 2.

# Behaviors

## B1: Prints the current time as hh:mm:ss
- Given no arguments
- And the clock at `1735736709`
- When the program runs
- Then it prints `13:05:09` followed by a newline
- And it exits with code `0`

## B2: Prints how many days until a date
- Given the argument `2025-12-25`
- And the clock at `1735689600`
- When the program runs
- Then it prints `358` followed by a newline
- And it exits with code `0`

## B3: Counts a date in the past as negative days
- Given the argument `2024-12-25`
- And the clock at `1735689600`
- When the program runs
- Then it prints `-7` followed by a newline
- And it exits with code `0`

## B4: Rejects text that is not a date
- Given the argument `tomorrow`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` followed by a newline to standard error
- And it exits with code `1`

## B5: Rejects a date that is not on the calendar
- Given the argument `2026-02-30`
- When the program runs
- Then it prints `invalid date: expected YYYY-MM-DD` followed by a newline to standard error
- And it exits with code `1`

## B6: Shows usage when given too many arguments
- Given the arguments `2026-12-25` and `2027-01-01`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `3`

# Constraints

- Allowed system calls: write, clock_gettime, exit
