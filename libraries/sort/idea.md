---
type: lengua Library
description: "The library sorts a list in a stable order and finds a value in a sorted list."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 17a8a55af09597c2cf54343836d105967ac17d979bb8b4a28e269625042524d4
source_sha256: 94a201e3431aa54bdfef66570ac9972e1fa9c95b9a594c6d41657c3146303bea
cargo_sha256: 70c252a02fdeed93a965109d63e004f5094e844d2c03fcd6b656087d6d7d7d15
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:03:22Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:03:22Z }
title: sort
status: draft
approved: { sha256: 44586ccd0c3e27cb432f6c1284f2e11d9f54d8155dd73df36fdb48145b7a3ee5, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The library sorts a list in a stable order and finds a value in a sorted list. A stable sort
keeps the order of equal items. The sort is a merge sort with no recursion, and the search is a
binary search that finds the first equal item. It is a `lengua` library on the standard library
only, and it works on the types of the program.

# Behaviours

## Sort numbers

Given a list of numbers in no order,
When a program calls `stable_sort`,
Then the list is in order from the smallest to the largest.

### Example: five numbers

- Call: `{ let mut list = vec![5, 3, 9, 1, 3]; stable_sort(&mut list); list }`
- Expect value: `[1, 3, 3, 5, 9]`

### Example: numbers in the wrong order

- Call: `{ let mut list = vec![6, 5, 4, 3, 2, 1, 0]; stable_sort(&mut list); list }`
- Expect value: `[0, 1, 2, 3, 4, 5, 6]`

### Example: an empty list

- Call: `{ let mut list: Vec<u8> = vec![]; stable_sort(&mut list); list }`
- Expect value: `[]`

### Example: one item

- Call: `{ let mut list = vec!["one"]; stable_sort(&mut list); list }`
- Expect value: `["one"]`

## Keep the order of equal items

Given items with equal keys,
When a program calls `stable_sort_by_key`,
Then the items with equal keys stay in their first order.

### Example: equal keys

- Call: `{ let mut list = vec![(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd'), (0, 'e')]; stable_sort_by_key(&mut list, |pair| pair.0); list }`
- Expect value: `[(0, 'e'), (1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]`

### Example: three runs

- Call: `{ let mut list = vec![(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd'), (1, 'e'), (0, 'f')]; stable_sort_by_key(&mut list, |pair| pair.0); list }`
- Expect value: `[(0, 'b'), (0, 'd'), (0, 'f'), (1, 'a'), (1, 'c'), (1, 'e')]`

## Find a value in a sorted list

Given a sorted list with a value two times,
When a program calls `binary_search`,
Then it gives the position of the first of the two.

### Example: the first of two

- Call: `binary_search(&[1, 3, 3, 5, 7], &3)`
- Expect value: `Some(1)`

### Example: the first item

- Call: `binary_search(&[1, 3, 3, 5, 7], &1)`
- Expect value: `Some(0)`

### Example: the last item

- Call: `binary_search(&[1, 3, 3, 5, 7], &7)`
- Expect value: `Some(4)`

### Example: a value that is not there

- Call: `binary_search(&[1, 3, 3, 5, 7], &4)`
- Expect value: `None`

### Example: a value after the last

- Call: `binary_search(&[1, 3, 3, 5, 7], &8)`
- Expect value: `None`

### Example: an empty list

- Call: `binary_search(&[] as &[u8], &1)`
- Expect value: `None`

### Cases: each value of a list

- Range `n`: integers from 0 to 15
- Call: `binary_search(&(0..16).map(|k| k * 2).collect::<Vec<i32>>(), &({n} * 2))`
- Expect value: `Some({n})`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `stable_sort`, `stable_sort_by_key`, `binary_search`

# Structure

- `stable_sort`: sorts a list in a stable order.
- `stable_sort_by_key`: sorts a list in a stable order by a key that a closure gives for each
  item.
- `merge`: adds two sorted runs to a list in one sorted run, and takes the left item when the
  keys are equal.
- `binary_search`: gives the position of the first item that is equal to the value, or nothing.

# Links

* Index: [sort](index.md).
