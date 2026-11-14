---
type: lengua Module
description: "The module writes the report line from the three numbers."
module: report
source: src/report.rs
uses: []
used_by: [idea]
label: checked
idea_sha256: 783221f8a1b2ba9810e97a9123dd2a29e4f1b63ead7b313fdbf8f4d61e75914d
source_sha256: b265f5f44a2e80e961564c9843f9dc7852d8e33b50cf2479af469bb52056d972
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T13:30:53Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T13:30:53Z }
title: report
status: draft
---

# Purpose

The module writes the report line from the three numbers.

# Behaviours

## Write the numbers in one line

Given a text of one line with one word,
When the program runs,
Then it writes the three numbers in one line, with a line end.

### Example: one word

- Standard input: "tea\n"
- Expect standard output: "lines 1, words 1, maximum 3\n"
- Expect exit code: 0

# Structure

- `format_report`: gives the report line from the number of lines, the number of words and the
  maximum length.

# Links

* Used by: [tally](../idea.md).
* Index: [tally](../index.md).
