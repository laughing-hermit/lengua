---
type: lengua/program-idea
title: total
description: Reads a CSV file of expenses given as an argument and prints the total amount with two decimals.
---
# Purpose

Lets someone tracking their spending add up a CSV file of expenses. The file's name is the one
argument; the file sits in the folder the program runs in and holds at most `1048576` bytes. Each
line is one expense written as a date, a comma, and an amount, such as 2026-10-01,12.50. The date
is any text without a comma and is not checked. Each amount is a decimal with exactly two places,
from `0.00` to `99999.99`, and the program adds them exactly, in whole cents, so the total needs no
rounding. Every line ends with a newline, except that the last line may lack one. A file with no
lines totals 0.00. If the file is missing or can't be opened, the program exits with code 2. If
reading the file fails, the program exits with code 4. If writing the output fails, the program
exits with code 5.

# Behaviors

## B1: Prints the total of all expenses
- Given the argument `expenses.csv`
- And a file `expenses.csv` holding `2026-10-01,12.50\n2026-10-02,7.25\n2026-10-03,100.00\n`
- When the program runs
- Then it prints `119.75` followed by a newline
- And it exits with code `0`

### For any `d1`, `d2`, `d3`, `a`, `b`, `c` and `end`
- Given the argument `expenses.csv`
- And a file `expenses.csv` holding `{d1},{a}\n{d2},{b}\n{d3},{c}{end}`
- When the program runs
- Then it prints `{places(scaled(a, 2) + scaled(b, 2) + scaled(c, 2), 2)}` followed by a newline
- And it exits with code `0`

| d1 | d2 | d3 | a | b | c | end | runs |
|---|---|---|---|---|---|---|---|
| `choice("2026-10-01", "2025-12-31", "", "Jan 5")` | `choice("2026-10-02", "x", "2026/10/02")` | `choice("2026-10-03", "", "today")` | `decimal(0.00, 99999.99)` | `decimal(0.00, 99999.99)` | `decimal(0.00, 99999.99)` | `choice("\n", "")` | 300 |

## B2: Prints zero for an empty file
- Given the argument `empty.csv`
- And an empty file `empty.csv`
- When the program runs
- Then it prints `0.00` followed by a newline
- And it exits with code `0`

### For any `e`
- Given the argument `empty.csv`
- And a file `empty.csv` holding `{e}`
- When the program runs
- Then it prints `0.00` followed by a newline
- And it exits with code `0`

| e | runs |
|---|---|
| `choice("")` | 1 |

## B3: Shows usage when the file name is missing
- Given no arguments
- When the program runs
- Then it prints `usage: total <file.csv>` followed by a newline to standard error
- And it exits with code `1`

## B4: Reports a missing file
- Given the argument `missing.csv`
- And no file `missing.csv`
- When the program runs
- Then it prints `total: cannot open file` followed by a newline to standard error
- And it exits with code `2`

### For any `name`
- Given the argument `{name}`
- When the program runs
- Then it prints `total: cannot open file` followed by a newline to standard error
- And it exits with code `2`

| name | runs |
|---|---|
| `choice("missing.csv", "nope.csv", "expenses")` | 20 |

## B5: Rejects a line that is not a date and an amount
- Given the argument `bad.csv`
- And a file `bad.csv` holding `2026-10-01,12.50\n2026-10-02,lunch\n`
- When the program runs
- Then it prints `total: bad line 2` followed by a newline to standard error
- And it exits with code `3`

### For any `d1`, `a` and `bad`
- Given the argument `bad.csv`
- And a file `bad.csv` holding `{d1},{a}\n2026-10-02,{bad}\n`
- When the program runs
- Then it prints `total: bad line 2` followed by a newline to standard error
- And it exits with code `3`

| d1 | a | bad | runs |
|---|---|---|---|
| `choice("2026-10-01", "")` | `decimal(0.00, 99999.99)` | `choice("lunch", "12.5", "12.500", "1x.50", "", "-1.00", "100000.00", "12,50", "12.50x", ".50", "12.", "12 .50")` | 100 |

# Constraints

- Uses: files
