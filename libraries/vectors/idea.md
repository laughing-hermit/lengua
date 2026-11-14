---
type: lengua Library
description: "The library finds the texts that are near to a question by their vectors."
module: main
source: src/main.rs
uses: [../json/idea]
used_by: []
label: checked
idea_sha256: 9d3bc26c1ee5e0bace3a9067d2198ea80b3eeb6a39294dc96271fabd932ebe18
source_sha256: 73b6d3e80199c380b36b0e0d40eb661fc109e3924fb6dd2627e0dedc04929c45
cargo_sha256: ea71cc58f8a0095c38eeec84dc4245a43d2768c97fc90ddce611fb750a77342f
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:06:06Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:06:06Z }
title: vectors
status: draft
approved: { sha256: 120e17099b03b58b78c86b4605650a8e881917354c02eb20dada182b44f7c91a, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The library finds the texts that are near to a question by their vectors. An AI model gives the
vector of a text, its embedding, as a JSON list of numbers. The library reads the list, compares
two vectors by the cosine of their angle, and finds the nearest vectors of a list. It uses the
library `json`, so it is a `lengua` library that uses another `lengua` library.

# Behaviours

## Read a vector

Given the answer of a model as a JSON list of numbers,
When a program calls `parse_vector`,
Then it gives the numbers, or no vector when the answer is not a list of numbers.

### Example: three numbers

- Call: `parse_vector(" [0.5, -1, 2e1] ")`
- Expect value: `Some([0.5, -1.0, 20.0])`

### Example: a list with a word

- Call: `parse_vector("[1, \"two\"]")`
- Expect value: `None`

### Example: an object

- Call: `parse_vector("{\"a\": 1}")`
- Expect value: `None`

### Example: a number that is too large

- Call: `parse_vector("[1e999]")`
- Expect value: `None`

## Multiply two vectors

Given two vectors,
When a program calls `dot`,
Then it gives the sum of the products of their numbers.

### Example: two vectors of three numbers

- Call: `dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0])`
- Expect value: `32.0`

## Compare two vectors

Given two vectors,
When a program calls `cosine`,
Then it gives the cosine of their angle, from -1 to 1, or 0 for vectors of two sizes and for a vector of zeros.

### Example: the same vector

- Call: `cosine(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0])`
- Expect value: `1.0`

### Example: two vectors near each other

- Call: `cosine(&[3.0, 4.0], &[4.0, 3.0])`
- Expect value: `0.96`

### Example: two vectors at a right angle

- Call: `cosine(&[1.0, 0.0], &[0.0, 1.0])`
- Expect value: `0.0`

### Example: two opposite vectors

- Call: `cosine(&[1.0, 1.0], &[-2.0, -2.0])`
- Expect value: `-1.0`

### Example: vectors of two sizes

- Call: `cosine(&[1.0], &[1.0, 0.0])`
- Expect value: `0.0`

### Example: a vector of zeros

- Call: `cosine(&[0.0, 0.0], &[1.0, 2.0])`
- Expect value: `0.0`

### Example: two empty vectors

- Call: `cosine(&[], &[])`
- Expect value: `0.0`

## Find the nearest vectors

Given a question and a list of vectors,
When a program calls `nearest` with a count,
Then it gives the places of the nearest vectors in the list, the nearest first.

### Example: two of three

- Call: `nearest(&[1.0, 0.0], &[vec![0.0, 1.0], vec![1.0, 0.1], vec![1.0, 1.0]], 2)`
- Expect value: `[1, 2]`

### Example: two vectors at the same angle

- Call: `nearest(&[1.0, 0.0], &[vec![2.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]], 3)`
- Expect value: `[0, 1, 2]`

### Example: a count larger than the list

- Call: `nearest(&[1.0], &[vec![1.0]], 5)`
- Expect value: `[0]`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `parse_vector`, `dot`, `cosine`, `nearest`
- Uses: `../json`

# Structure

- `parse_vector`: gives the numbers of a JSON list, or no vector.
- `dot`: gives the sum of the products of the numbers of two vectors.
- `cosine`: gives the cosine of the angle of two vectors.
- `nearest`: gives the places of the nearest vectors to a question, the nearest first.

# Links

* Uses: [json](../json/idea.md).
* Index: [vectors](index.md).
