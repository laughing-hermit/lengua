---
type: lengua Module
description: "The module counts the words of a text and finds the maximum length of a word."
module: words
source: src/words.rs
uses: []
used_by: [idea]
label: checked
idea_sha256: 5e392512324e1c617f0bd2be524f1e1dc8ef212aed1a8094acacf8404061566e
source_sha256: 7138ad75a2fa7a3110244d74233436038ea184bc86cb851338354b01335d0360
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T13:30:53Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T13:30:53Z }
title: words
status: draft
---

# Purpose

The module counts the words of a text and finds the maximum length of a word. Spaces, `tab`
characters and line ends are between the words.

# Behaviours

## Find the maximum length of a word

Given the words `a`, `bbb` and `cc` on one line,
When the program runs,
Then it writes 3 words and 3 as the maximum length.

### Example: three words

- Standard input: "a bbb cc\n"
- Expect standard output: "lines 1, words 3, maximum 3\n"
- Expect exit code: 0

## Count two words with a `tab` between them

Given the words `x` and `yy` with a `tab` character between them,
When the program runs,
Then it writes 2 words.

### Example: a `tab` character

- Standard input: "x\tyy\n"
- Expect standard output: "lines 1, words 2, maximum 2\n"
- Expect exit code: 0

# Structure

- `count_words`: gives the number of words in a text.
- `maximum_length`: gives the maximum length of a word in a text, or 0 for a text with no words.

# Links

* Used by: [tally](../idea.md).
* Index: [tally](../index.md).
