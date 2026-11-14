---
type: lengua Library
description: "The library reads and writes whole numbers and decimal numbers with a fixed number of places, as `i64` values."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 6f8b62af5416cdfc6cdcd57e2e1ea9c60faf0e7ae3f54731b9ad647d8b06abd3
source_sha256: 0aa2edc6181445c8fbdac7134d55ba2356ab1459260b66676e3dbdb1055d9e1b
cargo_sha256: ccf407feb8f49854dfd01f4a44c8e8268dab795251bb09f5213b8b08bbd11b9e
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T07:41:54Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T07:41:54Z }
title: decimal
status: draft
---

# Purpose

The library reads and writes whole numbers and decimal numbers with a fixed number of places,
as `i64` values. A decimal number is a whole number of its smallest units: with two places,
`12.34` is 1234.
The library reports an overflow when a value is past the range of `i64`, and it never gives a
wrong value. It is a `lengua` library on the standard library only.

# Behaviours

## Read a whole number

Given the text of a whole number, with a sign or with no sign,
When a program calls `parse_integer`,
Then it gives the number.

### Example: a positive number

- Call: `parse_integer("42")`
- Expect value: `Ok(42)`

### Example: a negative number

- Call: `parse_integer("-17")`
- Expect value: `Ok(-17)`

### Example: a plus sign

- Call: `parse_integer("+5")`
- Expect value: `Ok(5)`

### Example: the largest number

- Call: `parse_integer("9223372036854775807")`
- Expect value: `Ok(9223372036854775807)`

### Example: the smallest number

- Call: `parse_integer("-9223372036854775808")`
- Expect value: `Ok(-9223372036854775808)`

### Cases: numbers of one to three digits

- Range `n`: integers from -999 to 999
- Call: `parse_integer("{n}")`
- Expect value: `Ok({n})`

## Report a number that is not valid

Given a text that is not a whole number,
When a program calls `parse_integer`,
Then it gives the error and the position of the first character that is not a digit.

### Example: no digits

- Call: `parse_integer("")`
- Expect value: `Err(Empty)`

### Example: a sign only

- Call: `parse_integer("-")`
- Expect value: `Err(Empty)`

### Example: a letter

- Call: `parse_integer("4x2")`
- Expect value: `Err(Invalid(1))`

### Example: a letter after a sign

- Call: `parse_integer("-4x")`
- Expect value: `Err(Invalid(2))`

## Report an overflow

Given a number that is past the range of `i64`,
When a program calls `parse_integer`,
Then it gives the error `Overflow`.

### Example: one past the largest number

- Call: `parse_integer("9223372036854775808")`
- Expect value: `Err(Overflow)`

### Example: one past the smallest number

- Call: `parse_integer("-9223372036854775809")`
- Expect value: `Err(Overflow)`

## Read a decimal number

Given the text of a decimal number and the number of places,
When a program calls `parse_fixed`,
Then it gives the number of the smallest units.

### Example: two places

- Call: `parse_fixed("12.34", 2)`
- Expect value: `Ok(1234)`

### Example: one digit after the point

- Call: `parse_fixed("12.3", 2)`
- Expect value: `Ok(1230)`

### Example: no point

- Call: `parse_fixed("12", 2)`
- Expect value: `Ok(1200)`

### Example: a negative fraction

- Call: `parse_fixed("-0.05", 2)`
- Expect value: `Ok(-5)`

### Example: no whole part

- Call: `parse_fixed(".5", 2)`
- Expect value: `Ok(50)`

### Example: too many places

- Call: `parse_fixed("1.234", 2)`
- Expect value: `Err(TooPrecise)`

### Example: a second point

- Call: `parse_fixed("1.2.3", 3)`
- Expect value: `Err(Invalid(3))`

### Example: an overflow

- Call: `parse_fixed("92233720368547758.08", 3)`
- Expect value: `Err(Overflow)`

### Example: a point only

- Call: `parse_fixed(".", 2)`
- Expect value: `Err(Empty)`

## Write a decimal number

Given a number of the smallest units and the number of places,
When a program calls `format_fixed`,
Then it gives the text of the number, with all its places.

### Example: two places

- Call: `format_fixed(1234, 2)`
- Expect value: `"12.34"`

### Example: a negative fraction

- Call: `format_fixed(-5, 2)`
- Expect value: `"-0.05"`

### Example: zero places

- Call: `format_fixed(7, 0)`
- Expect value: `"7"`

### Example: zeros after the point

- Call: `format_fixed(100, 2)`
- Expect value: `"1.00"`

### Example: the smallest number

- Call: `format_fixed(i64::MIN, 2)`
- Expect value: `"-92233720368547758.08"`

### Example: too many places for the range

- Call: `format_fixed(-3, 30)`
- Expect value: `"-3"`

### Example: a whole number

- Call: `format_integer(-120)`
- Expect value: `"-120"`

## Add and multiply with an overflow report

Given two numbers,
When a program calls `add` or `multiply`,
Then it gives the result, or the error `Overflow` when the result is past the range of `i64`.

### Example: a sum

- Call: `add(2, 3)`
- Expect value: `Ok(5)`

### Example: a sum past the range

- Call: `add(i64::MAX, 1)`
- Expect value: `Err(Overflow)`

### Example: a product

- Call: `multiply(3, -4)`
- Expect value: `Ok(-12)`

### Example: a product past the range

- Call: `multiply(i64::MAX, 2)`
- Expect value: `Err(Overflow)`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `Error`, `parse_integer`, `parse_fixed`, `format_integer`, `format_fixed`, `add`, `multiply`

# Structure

- `parse_integer`: reads a whole number with an optional sign.
- `parse_fixed`: reads a decimal number as a number of its smallest units.
- `signed`: reads the sign and gives the digits after it and their position.
- `accumulate`: adds the digits to a value, and reports an overflow or a character that is not
  a digit.
- `format_integer`: writes a whole number.
- `format_fixed`: writes a number of the smallest units as a decimal number.
- `add`: adds two numbers, or reports an overflow.
- `multiply`: multiplies two numbers, or reports an overflow.

# Links

* Index: [decimal](index.md).
