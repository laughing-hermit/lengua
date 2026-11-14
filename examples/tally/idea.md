---
type: lengua Program
description: "The program counts the lines and the words of a text."
module: main
source: src/main.rs
uses: [ideas/lines, ideas/words, ideas/report]
used_by: []
label: checked
idea_sha256: 238fd78c54f537125f9a3cffc11795162b67534197b6398447af91405c3f4e01
source_sha256: 98ebf70faf3b5dc6935b1a3006b791330fec18a9337317b888155da2f079be9c
cargo_sha256: a15fe5435f50ba86fa7d52a899cb13eb574bdd7e40ff1e075d2db341732bd6fe
world_sha256: 0a7313a3dd0aa5c41e90b244f039886104d47ce35544350dbfa738da23ba0d2c
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T06:12:20Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T06:12:20Z }
title: tally
status: draft
---

# Purpose

The program counts the lines and the words of a text. It reads the text from the standard
input. It writes one line with the number of lines, the number of words and the maximum length
of a word. The program has three modules: `lines`, `words` and `report`.

# Behaviours

## Count a text of two lines

Given the text `a bb` and `ccc` on two lines,
When the program runs,
Then it writes `lines 2, words 3, maximum 3` and a line end.

### Example: two lines

- Standard input: "a bb\nccc\n"
- Expect standard output: "lines 2, words 3, maximum 3\n"
- Expect exit code: 0

## Count an empty text

Given an empty standard input,
When the program runs,
Then it writes `lines 0, words 0, maximum 0` and a line end.

### Example: empty input

- Standard input: ""
- Expect standard output: "lines 0, words 0, maximum 0\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Memory: 16 MiB
- Time: 2 s
- Modules: `lines`, `words`, `report`

# Structure

- `main`: reads the standard input and writes the report of the text.

# Links

* Uses: [lines](ideas/lines.md), [words](ideas/words.md), [report](ideas/report.md).
* Index: [tally](index.md).
