---
type: lengua Library
description: "The library cuts text into lines, parts and words."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 00d9fcae028d21a72bd74ca691544398d7bf99043a6d6b698a5193e3b70d15cd
source_sha256: 1e857ae6a3f7ae38f70c6b508cd6013782cab1c1ca1a080db019f38ce07f7b52
cargo_sha256: 7161273f8c408f6b40b4ddc1d95200629c610afb786562bf6b5c543f18ae4789
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:15:24Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:15:24Z }
title: text
status: draft
approved: { sha256: edbd93798b65ba9c92a3ac387c8183010b1bf1365b0c0ecafdd35cf85bbd9ccd, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The library cuts text into lines, parts and words. It removes the blank characters at the ends of
a text, and it checks that bytes are valid UTF-8. It is a `lengua` library on the standard
library only. The blank characters are the space, the `tab`, the line end and the carriage
return.

# Behaviours

## Cut a text into lines

Given a text with two lines and a line end after the last line,
When a program calls `lines`,
Then it gives the two lines, with no line ends.

### Example: two lines

- Call: `lines("a\nb\n")`
- Expect value: `["a", "b"]`

### Example: a last line with no line end

- Call: `lines("a\nb")`
- Expect value: `["a", "b"]`

### Example: lines that end with a carriage return

- Call: `lines("a\r\nb\r\n")`
- Expect value: `["a", "b"]`

### Example: an empty text

- Call: `lines("")`
- Expect value: `[]`

### Example: an empty line

- Call: `lines("\n\nc")`
- Expect value: `["", "", "c"]`

## Split a text at a character

Given the text `a,b,,c` and a comma,
When a program calls `split`,
Then it gives the four parts, and the part after the second comma is empty.

### Example: commas

- Call: `split("a,b,,c", ',')`
- Expect value: `["a", "b", "", "c"]`

### Example: no comma

- Call: `split("abc", ',')`
- Expect value: `["abc"]`

## Find the words of a text

Given words with spaces, `tab` characters and line ends between them,
When a program calls `words`,
Then it gives the words.

### Example: blank characters between words

- Call: `words(" one\ttwo \n three ")`
- Expect value: `["one", "two", "three"]`

## Remove the blank characters at the ends

Given a text with blank characters at its start and at its end,
When a program calls `trim`,
Then it gives the text with no blank characters at its ends.

### Example: blank characters at the two ends

- Call: `trim(" \t a b \r\n")`
- Expect value: `"a b"`

### Example: only blank characters

- Call: `trim(" \n ")`
- Expect value: `""`

### Example: a character that is not blank

- Call: `trim("_x_")`
- Expect value: `"_x_"`

## Check valid UTF-8

Given bytes that are valid UTF-8 with one, two, three and four bytes for a character,
When a program calls `check_utf8`,
Then it gives no error.

### Example: valid bytes

- Call: `check_utf8("aé€😀".as_bytes())`
- Expect value: `Ok(())`

### Example: no bytes

- Call: `is_utf8(b"")`
- Expect value: `true`

## Find the first byte that is not valid

Given bytes with a byte that is not valid UTF-8,
When a program calls `check_utf8`,
Then it gives the position of the first byte of the character that is not valid.

### Example: a byte that starts nothing

- Call: `check_utf8(b"ab\xffc")`
- Expect value: `Err(2)`

### Example: a short character at the end

- Call: `check_utf8(b"a\xe2\x82")`
- Expect value: `Err(1)`

### Example: a long form of a short character

- Call: `check_utf8(b"\xe0\x80\x80")`
- Expect value: `Err(0)`

### Example: a second byte that does not continue

- Call: `check_utf8(b"x\xc3(")`
- Expect value: `Err(1)`

### Example: a surrogate

- Call: `is_utf8(b"\xed\xa0\x80")`
- Expect value: `false`

### Example: a value past the last character

- Call: `is_utf8(b"\xf4\x90\x80\x80")`
- Expect value: `false`

### Example: the last character

- Call: `is_utf8(b"\xf4\x8f\xbf\xbf")`
- Expect value: `true`

### Example: a last byte that does not continue

- Call: `is_utf8(b"\xe2\x82(")`
- Expect value: `false`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `lines`, `split`, `words`, `trim`, `check_utf8`, `is_utf8`

# Structure

- `lines`: gives the lines of a text, with no line ends and no carriage return at their ends.
- `split`: gives the parts of a text between the separator characters.
- `words`: gives the words of a text, with the blank characters between them.
- `trim`: gives the text with no blank characters at its start or at its end.
- `is_blank`: tells if a character is a space, a `tab`, a line end or a carriage return.
- `check_utf8`: gives no error for valid UTF-8, or the position of the first character that is
  not valid.
- `is_utf8`: tells if bytes are valid UTF-8.
- `sequence_length`: gives the number of bytes of a character from its first byte, or nothing
  for a byte that starts no character.
- `continues`: tells if the bytes after the first byte continue the character.

# Links

* Index: [text](index.md).
