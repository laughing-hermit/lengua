---
type: lengua Program
description: "The program adds prices."
module: main
source: src/main.rs
uses: [../../libraries/decimal/idea]
used_by: []
label: checked
idea_sha256: 533383b537de9fd9aa7e74be93491ce9429865cff250d980e7561549ad3d885c
source_sha256: 6e982c49a2d135f377bf9cecf664977415042b03ad8092e7112b6be0743733a1
cargo_sha256: ddbdc4e9c6ab3af45cdc58c850868cfa3bed82d08e3afdfdebc3750ded0a8c54
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:12:00Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:12:00Z }
title: prices
status: draft
---

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

# Links

* Uses: [decimal](../../libraries/decimal/idea.md).
* Index: [prices](index.md).
