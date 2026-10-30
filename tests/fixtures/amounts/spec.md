---
type: lengua/program-spec
title: total
description: Reads a CSV from standard input and prints the total of its amount column.
---
# Purpose
Lets someone total the money in a CSV file, such as a bank export, from the shell:
`./total < payments.csv`. The first line of standard input is a header naming the columns,
separated by commas; one of them is named exactly `amount`. Every following line is a row with
its fields in the same order. The program adds up the amount field of every row, exactly in
whole cents, and prints the total with two decimals.

Standard input holds up to `1048576` bytes. A file has from `0` to `1000` rows and from `1` to
`10` columns. Fields hold no commas, quotes or line breaks, and every line ends with a newline.
The header's other column names and the rows' other fields are from `0` to `20` letters,
digits, spaces, hyphens, points and slashes. Each amount has an optional minus sign, then a whole part from `0` to
`999999` written without extra leading zeros, a point, and exactly two digits, from `-999999.99`
to `999999.99`. A negative total is printed with a minus sign; a zero total prints as `0.00`.
If writing the output fails, the program exits with code 4.

# Behaviors
## B1: Prints the total of the amount column
- Given standard input holding `date,amount,note\n2026-01-05,12.50,lunch\n2026-01-06,-2.25,refund\n2026-01-07,100.00,rent\n`
- When the program runs
- Then it prints `110.25` followed by a newline
- And it exits with code `0`

## B2: Prints zero when there are no rows
- Given standard input holding `amount\n`
- When the program runs
- Then it prints `0.00` followed by a newline
- And it exits with code `0`

## B3: Rejects a header without an amount column
- Given standard input holding `date,price\n2026-01-05,12.50\n`
- When the program runs
- Then it prints `total: no amount column` followed by a newline to standard error
- And it exits with code `1`

## B4: Rejects an amount that is not two decimals
- Given standard input holding `amount,note\n12.50,lunch\n7.5,coffee\n`
- When the program runs
- Then it prints `total: bad amount on line 3` followed by a newline to standard error
- And it exits with code `2`

## B5: Rejects a row without an amount field
- Given standard input holding `note,amount\nlunch,12.50\ncoffee\n`
- When the program runs
- Then it prints `total: missing amount on line 3` followed by a newline to standard error
- And it exits with code `3`

# Constraints
- Allowed system calls: read, write, exit
