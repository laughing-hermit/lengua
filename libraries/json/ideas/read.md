---
type: lengua Module
description: "The module reads JSON text into a value."
module: read
source: src/read.rs
uses: []
used_by: [idea]
label: checked
idea_sha256: 134bbdcd4d5c5c7a9d84674c7a1293318a7226215554f522a9012ac1c9d5e26e
source_sha256: 19dd9fd3c03d83630ebb94368c5213634aa3134c289fa5561036c38987758e63
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:30:39Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:30:39Z }
title: read
status: draft
---

# Purpose

The module reads JSON text into a value. It cuts the text into tokens, and then it puts the
tokens together with a stack of open lists and objects. An error gives the position of the
character where the text stops to be valid JSON.

# Behaviours

## Read a single value

Given the text of one value,
When a program calls `parse`,
Then it gives the value.

### Example: null

- Call: `parse("null")`
- Expect value: `Ok(Null)`

### Example: true

- Call: `parse("true")`
- Expect value: `Ok(Bool(true))`

### Example: false

- Call: `parse(" false ")`
- Expect value: `Ok(Bool(false))`

### Example: a number with a fraction and an exponent

- Call: `parse("-0.5e+10")`
- Expect value: `Ok(Number("-0.5e+10"))`

### Example: a number with a small exponent

- Call: `parse("12E-3")`
- Expect value: `Ok(Number("12E-3"))`

### Example: a text with escapes

- Call: `parse("\"a\\nb\\t\\\"c\\\\ \\/\"")`
- Expect value: `Ok(Text("a\nb\t\"c\\ /"))`

### Example: a character as a code

- Call: `parse("\"\\u00e9\"")`
- Expect value: `Ok(Text("é"))`

### Example: a character as two codes

- Call: `parse("\"\\ud83d\\ude00x\"")`
- Expect value: `Ok(Text("😀x"))`

### Example: two other escapes

- Call: `parse("\"\\b\\f\\r\"")`
- Expect value: `Ok(Text("\u{8}\u{c}\r"))`

## Read lists and objects

Given lists and objects in other lists and objects,
When a program calls `parse`,
Then it gives the value with all its parts in order.

### Example: a list in a list

- Call: `parse("[1, [2, {\"k\": null}], \"x\"]")`
- Expect value: `Ok(List([Number("1"), List([Number("2"), Object([("k", Null)])]), Text("x")]))`

### Example: an object of two fields

- Call: `parse("{\"a\":1,\"b\":[]}")`
- Expect value: `Ok(Object([("a", Number("1")), ("b", List([]))]))`

### Example: an empty object

- Call: `parse("{ }")`
- Expect value: `Ok(Object([]))`

## Report text that is not JSON

Given text that is not valid JSON,
When a program calls `parse`,
Then it gives the error and the position of the character where the text is wrong.

### Example: no text

- Call: `parse("")`
- Expect value: `Err(End)`

### Example: a list with no end

- Call: `parse("[1,")`
- Expect value: `Err(End)`

### Example: a comma before the end

- Call: `parse("[1,]")`
- Expect value: `Err(Unexpected(3))`

### Example: no colon

- Call: `parse("{\"a\" 1}")`
- Expect value: `Err(Unexpected(5))`

### Example: a second value

- Call: `parse("[1] 2")`
- Expect value: `Err(Unexpected(4))`

### Example: the wrong bracket

- Call: `parse("[}")`
- Expect value: `Err(Unexpected(1))`

### Example: a number for a name

- Call: `parse("{1:2}")`
- Expect value: `Err(Unexpected(1))`

### Example: a zero before a digit

- Call: `parse("01")`
- Expect value: `Err(Number(0))`

### Example: a point with no digits

- Call: `parse("[1.]")`
- Expect value: `Err(Number(1))`

### Example: an exponent with no digits

- Call: `parse("1e+")`
- Expect value: `Err(Number(0))`

### Example: a sign only

- Call: `parse("-")`
- Expect value: `Err(Number(0))`

### Example: a short word

- Call: `parse("tru")`
- Expect value: `Err(Unexpected(0))`

### Example: an escape that is not JSON

- Call: `parse("\"a\\x\"")`
- Expect value: `Err(Escape(2))`

### Example: half of a pair of codes

- Call: `parse("\"\\ud83d\"")`
- Expect value: `Err(Escape(1))`

### Example: a code that is not hexadecimal

- Call: `parse("\"\\u+0e9\"")`
- Expect value: `Err(Escape(1))`

### Example: a control character

- Call: `parse("\"a\u{1}\"")`
- Expect value: `Err(Unexpected(2))`

### Example: a text with no end

- Call: `parse("\"abc")`
- Expect value: `Err(End)`

### Example: too deep

- Call: `parse(&"[".repeat(600))`
- Expect value: `Err(Deep)`

### Example: deep and valid

- Call: `parse(&format!("{}{}", "[".repeat(512), "]".repeat(512))).is_ok()`
- Expect value: `true`

### Example: one frame too many

- Call: `parse(&format!("{}{}", "[".repeat(513), "]".repeat(513)))`
- Expect value: `Err(Deep)`

### Example: zero

- Call: `parse("0")`
- Expect value: `Ok(Number("0"))`

# Structure

- `parse`: reads JSON text into a value, or gives the error and its position.
- `tokens`: cuts the text into brackets, commas, colons and single values, with their positions.
- `word_at`: reads `null`, `true` or `false` at a position.
- `digits`: counts the digits from a position.
- `number_at`: reads a number at a position, as its text.
- `string_at`: reads a text in quotes at a position, with its escapes.
- `hex4`: reads four hexadecimal digits as a code.
- `escape_at`: reads one escape, and two escapes for a character above `U+FFFF`.
- `place`: puts a value into the open list or object, or keeps it as the result.
- `close`: closes the open list or object, and places it.
- `assemble`: puts the tokens together into one value.

# Links

* Used by: [json](../idea.md).
* Index: [json](../index.md).
