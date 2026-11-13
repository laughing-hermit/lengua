# Purpose

The program adds prices. It reads one price a line from the standard input, with two places
after the point, and it writes the sum with two places. It uses the `lengua` library
`decimal`. So a price that is not valid or a sum past the range of `i64` is an error, and
never a wrong sum.

# Behaviours

## Add two prices

Given the prices 12.34 and 0.66,
When the program runs,
Then it writes 13.00 and a line end.

### Example: two prices

- Standard input: "12.34\n0.66\n"
- Expect standard output: "13.00\n"
- Expect exit code: 0

## Report a price that is not valid

Given the price `1.234`, with three places,
When the program runs,
Then it writes the error `TooPrecise` and stops with the exit code 1.

### Example: three places

- Standard input: "1.234\n"
- Expect standard output: "TooPrecise\n"
- Expect exit code: 1

## Write zero for no prices

Given an empty standard input,
When the program runs,
Then it writes 0.00 and a line end.

### Example: no prices

- Standard input: ""
- Expect standard output: "0.00\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 5 s
- Uses: `../../libraries/decimal`

# Structure

- `main`: reads the prices and writes their sum, or the error.
- `total`: adds the prices of the lines of a text.
