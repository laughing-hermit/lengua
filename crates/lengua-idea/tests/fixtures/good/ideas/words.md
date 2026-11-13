---
title: Words
tags: [text]
label: checked
---

# Purpose

The module finds the words in a text. A word is a group of characters with no space in it.

# Behaviours

## Find the words

Given the text "a b",
When the module reads it,
Then it finds two words.

### Example: two words

- Standard input: "a b\n"
- Expect standard output: "2\n"
- Expect exit code: 0

# Structure

- `count_words`: counts the words in a text.
- `Counter::add`: adds one to the count.
