---
type: lengua/program-idea
title: total
description: Reads a CSV from standard input and prints the total of its amount column.
---
# Purpose
A person can use this program to find the total of the money in a CSV file, such as a bank export.
The command at the shell is `./total < payments.csv`.

The first line of standard input is a header with the names of the columns. A comma separates each
two names. The name of one column is exactly `amount`. Each line after the header is a row, with
its fields in the same order as the names.

The program adds the amount fields of all rows, exactly in whole cents. Then it prints the total
with two decimals.

Standard input has up to `1048576` bytes. A file has from `0` to `1000` rows and from `1` to `10`
columns. The fields have no commas, quotes or line breaks, and each line ends with a newline. The
other column names of the header and the other fields of the rows have from `0` to `20`
characters. These characters are letters, digits, spaces, hyphens, points and slashes.

Each amount has an optional minus sign, then a whole part from `0` to `999999`, a point, and
exactly two digits. The whole part has no extra zeros at its start. So each amount is from
`-999999.99` to `999999.99`. The program prints a negative total with a minus sign, and a zero
total as `0.00`. If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Prints the total of the amount column
- Given standard input that contains `date,amount,note\n2026-01-05,12.50,lunch\n2026-01-06,-2.25,refund\n2026-01-07,100.00,rent\n`
- When the program runs
- Then it prints `110.25` and a newline
- And it exits with code `0`

### For any `h1`, `h2`, `p1`, `q1`, `p2`, `q2`, `p3`, `q3`, `a1`, `a2` and `a3`
- Given standard input that contains `{h1},amount,{h2}\n{p1},{a1},{q1}\n{p2},{a2},{q2}\n{p3},{a3},{q3}\n`
- When the program runs
- Then it prints `{places(scaled(a1, 2) + scaled(a2, 2) + scaled(a3, 2), 2)}` and a newline
- And it exits with code `0`

| h1 | h2 | p1 | q1 | p2 | q2 | p3 | q3 | a1 | a2 | a3 | runs |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `choice("date", "", "id", "2026 Q1", "amounts", "Amount", "x-y/z.w")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -./")` | `decimal(-999999.99, 999999.99)` | `decimal(-999999.99, 999999.99)` | `decimal(-999999.99, 999999.99)` | 500 |

## B2: Prints zero when there are no rows
- Given standard input that contains `amount` and a newline
- When the program runs
- Then it prints `0.00` and a newline
- And it exits with code `0`

### For any `h`
- Given standard input that contains `{h}` and a newline
- When the program runs
- Then it prints `0.00` and a newline
- And it exits with code `0`

| h | runs |
|---|---|
| `choice("amount", "amount,note", "date,amount", "id,amount,x y")` | 20 |

## B3: Rejects a header without an amount column
- Given standard input that contains `date,price\n2026-01-05,12.50\n`
- When the program runs
- Then it prints `total: no amount column` and a newline to standard error
- And it exits with code `1`

### For any `h`, `d` and `a`
- Given standard input that contains `{h},price\n{d},{a}\n`
- When the program runs
- Then it prints `total: no amount column` and a newline to standard error
- And it exits with code `1`

| h | d | a | runs |
|---|---|---|---|
| `choice("date", "amounts", "Amount", "amoun", " amount", "amount ", "xamount", "")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | `decimal(-999999.99, 999999.99)` | 100 |

## B4: Rejects an amount that is not two decimals
- Given standard input that contains `amount,note\n12.50,lunch\n7.5,coffee\n`
- When the program runs
- Then it prints `total: bad amount on line 3` and a newline to standard error
- And it exits with code `2`

### For any `h`, `a`, `n1`, `bad` and `n2`
- Given standard input that contains `amount,{h}\n{a},{n1}\n{bad},{n2}\n`
- When the program runs
- Then it prints `total: bad amount on line 3` and a newline to standard error
- And it exits with code `2`

| h | a | n1 | bad | n2 | runs |
|---|---|---|---|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | `decimal(-999999.99, 999999.99)` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | `choice("7.5", "12.505", "12", "12.", ".50", "1.5x", "-", "--1.00", "+1.00", "01.00", "-01.00", "1000000.00", "1.5 ", " 1.50", "a.bc", "1.-5")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | 100 |

## B5: Rejects a row without an amount field
- Given standard input that contains `note,amount\nlunch,12.50\ncoffee\n`
- When the program runs
- Then it prints `total: missing amount on line 3` and a newline to standard error
- And it exits with code `3`

### For any `h`, `p`, `a` and `short`
- Given standard input that contains `{h},amount\n{p},{a}\n{short}\n`
- When the program runs
- Then it prints `total: missing amount on line 3` and a newline to standard error
- And it exits with code `3`

| h | p | a | short | runs |
|---|---|---|---|---|
| `choice("note", "", "date", "x y", "amounts", "amoun", "2026-01-05")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | `decimal(-999999.99, 999999.99)` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 -./")` | 100 |

# Constraints
- Uses: standard input
