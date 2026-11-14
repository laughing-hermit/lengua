---
type: lengua Library
description: "The library works with civil dates of the Gregorian calendar and with times in UTC."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 6bf4affe4810658529224033fb0aaf9e5f562f90ea157b8def8e839fa39cc4f3
source_sha256: 0a1f94f97b2541cb43385d4d7ae853ef66453b618dcfb5d92a765384e045ce1b
cargo_sha256: 3ce727db94090cb6aadb6e0249027fa4f7a571ee82a5cf4cf8582838a61fd081
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T09:57:24Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T09:57:24Z }
title: dates
status: draft
---

# Purpose

The library works with civil dates of the Gregorian calendar and with times in UTC. It counts the
days between dates, and it tells the number of days of a month. It reads and writes dates and
times in the ISO 8601 forms `2026-11-13` and `2026-11-13T14:02:11Z`. Day 0 is 1970-01-01. It is
a `lengua` library on the standard library only.

# Behaviours

## Tell a leap year

Given a year,
When a program calls `is_leap`,
Then it tells if the year has a 29 February.

### Example: a year of four

- Call: `is_leap(2024)`
- Expect value: `true`

### Example: a year that is not a year of four

- Call: `is_leap(2023)`
- Expect value: `false`

### Example: a year of a hundred

- Call: `is_leap(1900)`
- Expect value: `false`

### Example: a year of four hundred

- Call: `is_leap(2000)`
- Expect value: `true`

## Give the days of a month

Given a year and a month,
When a program calls `days_in_month`,
Then it gives the number of days of the month, or 0 for a month that is not there.

### Example: the month of January

- Call: `days_in_month(2026, 1)`
- Expect value: `31`

### Example: the month of April

- Call: `days_in_month(2026, 4)`
- Expect value: `30`

### Example: the February of a leap year

- Call: `days_in_month(2024, 2)`
- Expect value: `29`

### Example: the February of another year

- Call: `days_in_month(2026, 2)`
- Expect value: `28`

### Example: no month 13

- Call: `days_in_month(2026, 13)`
- Expect value: `0`

## Count the days from 1970-01-01

Given a date,
When a program calls `days_from_civil`,
Then it gives the number of days from 1970-01-01 to the date.

### Example: the first day

- Call: `days_from_civil(Date { year: 1970, month: 1, day: 1 })`
- Expect value: `0`

### Example: the day before

- Call: `days_from_civil(Date { year: 1969, month: 12, day: 31 })`
- Expect value: `-1`

### Example: the 29 February 2000

- Call: `days_from_civil(Date { year: 2000, month: 2, day: 29 })`
- Expect value: `11016`

### Example: the 1 March 1900

- Call: `days_from_civil(Date { year: 1900, month: 3, day: 1 })`
- Expect value: `-25508`

### Example: the 13 November 2026

- Call: `days_from_civil(Date { year: 2026, month: 11, day: 13 })`
- Expect value: `20770`

## Give the date of a day

Given a number of days from 1970-01-01,
When a program calls `civil_from_days`,
Then it gives the date of that day, and the date gives the same number back.

### Example: day 11017

- Call: `civil_from_days(11017)`
- Expect value: `Date { year: 2000, month: 3, day: 1 }`

### Example: a day before 1970

- Call: `civil_from_days(-25508)`
- Expect value: `Date { year: 1900, month: 3, day: 1 }`

### Example: the last day of a year

- Call: `civil_from_days(20818)`
- Expect value: `Date { year: 2026, month: 12, day: 31 }`

### Cases: days from the year 0 to the year 4000

- Range `d`: integers from -719528 to 741000
- Call: `days_from_civil(civil_from_days({d}))`
- Expect value: `{d}`

## Count the days between two dates

Given two dates,
When a program calls `days_between`,
Then it gives the number of days from the first date to the second, negative when the second is
before the first.

### Example: one leap year

- Call: `days_between(Date { year: 2024, month: 1, day: 1 }, Date { year: 2025, month: 1, day: 1 })`
- Expect value: `366`

### Example: back in time

- Call: `days_between(Date { year: 2026, month: 3, day: 1 }, Date { year: 2026, month: 2, day: 28 })`
- Expect value: `-1`

## Read and write a date

Given a date in the form `YYYY-MM-DD`,
When a program calls `parse_date` or `format_date`,
Then it gives the date, or the error and its position.

### Example: a good date

- Call: `parse_date("2026-11-13")`
- Expect value: `Ok(Date { year: 2026, month: 11, day: 13 })`

### Example: a month that is not there

- Call: `parse_date("2026-13-01")`
- Expect value: `Err(Month)`

### Example: month 0

- Call: `parse_date("2026-00-10")`
- Expect value: `Err(Month)`

### Example: a day that is not there

- Call: `parse_date("2026-02-29")`
- Expect value: `Err(Day)`

### Example: day 0

- Call: `parse_date("2026-02-00")`
- Expect value: `Err(Day)`

### Example: the last day of a month

- Call: `parse_date("2024-02-29")`
- Expect value: `Ok(Date { year: 2024, month: 2, day: 29 })`

### Example: a letter

- Call: `parse_date("20a6-01-01")`
- Expect value: `Err(Form(2))`

### Example: a short month

- Call: `parse_date("2026-1-01")`
- Expect value: `Err(Form(6))`

### Example: a long date

- Call: `parse_date("2026-11-133")`
- Expect value: `Err(Form(10))`

### Example: a short date

- Call: `parse_date("2026-11-1")`
- Expect value: `Err(Form(9))`

### Example: a date as text

- Call: `format_date(Date { year: 987, month: 6, day: 5 })`
- Expect value: `"0987-06-05"`

## Read and write a time

Given a time in the form `YYYY-MM-DDTHH:MM:SSZ`,
When a program calls `parse_timestamp` or `format_timestamp`,
Then it gives the seconds from 1970-01-01T00:00:00Z, or the time as text.

### Example: a time

- Call: `parse_timestamp("2026-11-13T14:02:11Z")`
- Expect value: `Ok(1794578531)`

### Example: an hour that is not there

- Call: `parse_timestamp("2026-11-13T24:00:00Z")`
- Expect value: `Err(Time)`

### Example: a minute that is not there

- Call: `parse_timestamp("2026-11-13T23:60:00Z")`
- Expect value: `Err(Time)`

### Example: a second that is not there

- Call: `parse_timestamp("2026-11-13T23:59:60Z")`
- Expect value: `Err(Time)`

### Example: no zone

- Call: `parse_timestamp("2026-11-13T14:02:11")`
- Expect value: `Err(Form(19))`

### Example: a space for the T

- Call: `parse_timestamp("2026-11-13 14:02:11Z")`
- Expect value: `Err(Form(10))`

### Example: the time as text

- Call: `format_timestamp(1794578531)`
- Expect value: `"2026-11-13T14:02:11Z"`

### Example: a time before 1970

- Call: `format_timestamp(-1)`
- Expect value: `"1969-12-31T23:59:59Z"`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `Date`, `Error`, `is_leap`, `days_in_month`, `days_from_civil`, `civil_from_days`, `days_between`, `parse_date`, `format_date`, `parse_timestamp`, `format_timestamp`

# Structure

- `is_leap`: tells if a year of the Gregorian calendar has a 29 February.
- `days_in_month`: gives the number of days of a month, or 0 for a month that is not there.
- `days_from_civil`: gives the number of days from 1970-01-01 to a date.
- `civil_from_days`: gives the date of a number of days from 1970-01-01.
- `days_between`: gives the number of days from one date to another.
- `number`: reads the digits between two positions of a text as a number.
- `form`: checks that a text has digits and the separators at their positions, and its length.
- `parse_date`: reads a date in the form `YYYY-MM-DD`.
- `format_date`: writes a date in the form `YYYY-MM-DD`.
- `parse_timestamp`: reads a time in the form `YYYY-MM-DDTHH:MM:SSZ` as seconds from
  1970-01-01T00:00:00Z.
- `format_timestamp`: writes seconds from 1970-01-01T00:00:00Z in the form `YYYY-MM-DDTHH:MM:SSZ`.

# Links

* Index: [dates](index.md).
