---
type: Attested Computation
title: total
description: Reads the CSV file of expenses that its argument names and prints the total amount with two decimals.
runtime: webassembly
computation: main.wat
parameters:
  - { name: file, type: text, required: true, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose

A person can use this program to find the total of a CSV file of expenses. The one argument is the
name of the file. The file is in the folder where the program runs, and it has at most `1048576`
bytes.

Each line is one expense: a date, a comma, and an amount, as in 2026-10-01,12.50. The date is any
text without a comma, and the program does not examine it. Each amount is a decimal with exactly
two places, from `0.00` to `99999.99`. The program adds the amounts exactly, in whole cents, so the
total needs no rounding.

Each line ends with a newline, but the last line can have no newline. For a file with no lines, the
total is 0.00.

If the file does not exist or the program cannot open it, the program exits with code 2. If the
program cannot read the file, it exits with code 4. If the program cannot write its output, it
exits with code 5.

# Behaviors

## B1: Prints the total of all expenses
- Given the argument `expenses.csv`
- And a file `expenses.csv` that contains `2026-10-01,12.50\n2026-10-02,7.25\n2026-10-03,100.00\n`
- When the program runs
- Then it prints `119.75` and a newline
- And it exits with code `0`

### For any `d1`, `d2`, `d3`, `a`, `b`, `c` and `end`
- Given the argument `expenses.csv`
- And a file `expenses.csv` that contains `{d1},{a}\n{d2},{b}\n{d3},{c}{end}`
- When the program runs
- Then it prints `{places(scaled(a, 2) + scaled(b, 2) + scaled(c, 2), 2)}` and a newline
- And it exits with code `0`

| d1 | d2 | d3 | a | b | c | end | runs |
|---|---|---|---|---|---|---|---|
| `choice("2026-10-01", "2025-12-31", "", "Jan 5")` | `choice("2026-10-02", "x", "2026/10/02")` | `choice("2026-10-03", "", "today")` | `decimal(0.00, 99999.99)` | `decimal(0.00, 99999.99)` | `decimal(0.00, 99999.99)` | `choice("\n", "")` | 300 |

## B2: Prints zero for an empty file
- Given the argument `empty.csv`
- And an empty file `empty.csv`
- When the program runs
- Then it prints `0.00` and a newline
- And it exits with code `0`

### For any `e`
- Given the argument `empty.csv`
- And a file `empty.csv` that contains `{e}`
- When the program runs
- Then it prints `0.00` and a newline
- And it exits with code `0`

| e | runs |
|---|---|
| `choice("")` | 1 |

## B3: Shows the usage line when there is no file name
- Given no arguments
- When the program runs
- Then it prints `usage: total <file.csv>` and a newline to standard error
- And it exits with code `1`

## B4: Reports a file that does not exist
- Given the argument `missing.csv`
- And no file `missing.csv`
- When the program runs
- Then it prints `total: cannot open file` and a newline to standard error
- And it exits with code `2`

### For any `name`
- Given the argument `{name}`
- When the program runs
- Then it prints `total: cannot open file` and a newline to standard error
- And it exits with code `2`

| name | runs |
|---|---|
| `choice("missing.csv", "nope.csv", "expenses")` | 20 |

## B5: Rejects a line that is not a date and an amount
- Given the argument `bad.csv`
- And a file `bad.csv` that contains `2026-10-01,12.50\n2026-10-02,lunch\n`
- When the program runs
- Then it prints `total: bad line 2` and a newline to standard error
- And it exits with code `3`

### For any `d1`, `a` and `bad`
- Given the argument `bad.csv`
- And a file `bad.csv` that contains `{d1},{a}\n2026-10-02,{bad}\n`
- When the program runs
- Then it prints `total: bad line 2` and a newline to standard error
- And it exits with code `3`

| d1 | a | bad | runs |
|---|---|---|---|
| `choice("2026-10-01", "")` | `decimal(0.00, 99999.99)` | `choice("lunch", "12.5", "12.500", "1x.50", "", "-1.00", "100000.00", "12,50", "12.50x", ".50", "12.", "12 .50")` | 100 |

# Constraints

- Uses: files
