---
type: lengua/program-spec
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
- Given no arguments
- And the clock at `1793264707`
- When the program runs
- Then it prints `09:05:07` followed by a newline
- And it exits with code `0`

## B2: Prints the days until a date
- Given the argument `2026-12-25`
- And the clock at `1793264707`
- When the program runs
- Then it prints `57` followed by a newline
- And it exits with code `0`

## B3: Rejects text that is not a date
- Given the argument `christmas`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

## B4: Rejects a day that does not exist
- Given the argument `2026-02-29`
- When the program runs
- Then it prints `clock: no such date: 2026-02-29` followed by a newline to standard error
- And it exits with code `2`

## B5: Rejects extra arguments
- Given the arguments `2026-12-25` and `2027-01-01`
- When the program runs
- Then it prints `usage: clock [YYYY-MM-DD]` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: write, clock_gettime, exit
