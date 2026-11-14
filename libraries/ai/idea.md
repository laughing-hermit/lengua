---
type: lengua Library
description: "The library helps a program that asks an AI model."
module: main
source: src/main.rs
uses: [../json/idea]
used_by: []
label: checked
idea_sha256: a34da3e5332be0bff2b9bbbd446eacb23effb46a81876bb0549824e75ab52098
source_sha256: 6ce9818ffe41bd66ce5edf56fc8ec8a1d1f25bfbdd600a311d1364c743c9f848
cargo_sha256: b6ed441d712ca9a9d8c11110e03107f50fae4071035ef64614deb6ee3adc3bdb
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:16:41Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:16:41Z }
title: ai
status: draft
---

# Purpose

The library helps a program that asks an AI model. It fills the parts of a prompt and chains
two steps. It reads the fields of an answer in JSON, and it tells which answers of the host to
try again. It uses the library `json`, so it is a `lengua` library that uses another `lengua`
library.

A library has no grants, so it does not ask the model. The program asks the model through its
host with `- Asks: an AI model`, and gives the answers to this library. The answer of a model
can be wrong: `fields` refuses an answer that is not a JSON object with each field.

# Behaviours

## Fill a prompt

Given a prompt with names in braces,
When a program calls `fill` with a value for each name,
Then each name and its braces become its value.

### Example: two names

- Call: `fill("Write a title for {text} in {words} words.", &[("text", "a note"), ("words", "3")])`
- Expect value: `"Write a title for a note in 3 words."`

### Example: a name with no value

- Call: `fill("Hello {name}.", &[("other", "x")])`
- Expect value: `"Hello {name}."`

### Example: braces that stay braces

- Call: `fill("{{\"a\": {n}}}", &[("n", "1")])`
- Expect value: `"{\"a\": 1}"`

### Example: a brace at the end

- Call: `fill("a } b {", &[])`
- Expect value: `"a } b {"`

## Chain two steps

Given the answer of the first step,
When a program calls `then` with the prompt of the next step,
Then the name `answer` in the prompt becomes the first answer.

### Example: a summary of a title

- Call: `then("Make this shorter: {answer}", "Buy two bottles of milk.")`
- Expect value: `"Make this shorter: Buy two bottles of milk."`

## Read the fields of an answer

Given an answer in JSON,
When a program calls `fields` with the names of the fields,
Then it gives each field as text, or the error of a bad answer.

### Example: two fields

- Call: `fields(" {\"title\":\"Milk\",\"words\":2,\"done\":false} ", &["title", "words", "done"])`
- Expect value: `Ok([("title", "Milk"), ("words", "2"), ("done", "false")])`

### Example: a field that is not there

- Call: `fields("{\"title\":\"Milk\"}", &["title", "summary"])`
- Expect value: `Err(Missing("summary"))`

### Example: a field that is a list

- Call: `fields("{\"title\":[]}", &["title"])`
- Expect value: `Err(Missing("title"))`

### Example: an answer that is not JSON

- Call: `fields("Sure! Here is the title: Milk", &["title"])`
- Expect value: `Err(NotJson)`

### Example: an answer that is a list

- Call: `fields("[1, 2]", &["title"])`
- Expect value: `Err(NotObject)`

## Try an answer of the host again

Given the status of an answer of the host,
When a program calls `retryable`,
Then it gives true when the model server failed for a short time.

### Example: the statuses to try again

- Call: `[500, 502, 503, 504].map(retryable)`
- Expect value: `[true, true, true, true]`

### Example: the statuses that stay the same

- Call: `[200, 404, 429, 501, 505].map(retryable)`
- Expect value: `[false, false, false, false, false]`

## Find the tokens of a text

Given a text,
When a program calls `estimate_tokens`,
Then it gives one token for each four bytes, as the host counts them in tests.

### Example: texts of 0, 4 and 5 bytes

- Call: `[estimate_tokens(""), estimate_tokens("abcd"), estimate_tokens("abcde")]`
- Expect value: `[0, 1, 2]`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `Error`, `fill`, `then`, `fields`, `retryable`, `estimate_tokens`
- Uses: `../json`

# Structure

- `fill`: gives the prompt with each name in braces as its value.
- `placeholder`: gives the text for the brace at the start of a text, and the bytes it uses.
- `then`: gives the prompt of the next step, with the answer of the step before it.
- `plain`: gives a text, a number or a value of true or false as text.
- `fields`: gives the fields with the given names of an answer in JSON, or the error of a bad answer.
- `retryable`: tells if the program can send a request again after the status of an answer.
- `estimate_tokens`: gives the tokens of a text, one token for each four bytes.

# Links

* Uses: [json](../json/idea.md).
* Index: [ai](index.md).
