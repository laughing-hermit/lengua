---
type: lengua Module
description: "The module counts the lines of a text."
module: lines
source: src/lines.rs
uses: []
used_by: [idea]
label: checked
idea_sha256: b0c04c3aa0476d6bfd9d7010c7fab9d9b6cd2a26f00d73eadb90c3866b72c409
source_sha256: 03ba8a5f304d1e933b2e97d3f275206bd05235eefb61d340902beed321e85673
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:07:17Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:07:17Z }
title: lines
status: draft
---

# Purpose

The module counts the lines of a text. A line ends with a line end, and the last line can have
no line end.

# Behaviours

## Count a last line with no line end

Given the text `a` and `b` on two lines, with no line end after `b`,
When the program runs,
Then it writes 2 as the number of lines.

### Example: no last line end

- Standard input: "a\nb"
- Expect standard output: "lines 2, words 2, maximum 1\n"
- Expect exit code: 0

# Structure

- `count_lines`: gives the number of lines in a text.

# Links

* Used by: [tally](../idea.md).
* Index: [tally](../index.md).
