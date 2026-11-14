---
type: lengua Program
description: "The program asks an AI model and another service, through its host, and writes what it gets."
module: main
source: src/main.rs
uses: [../../libraries/ai/idea, ../../libraries/vectors/idea]
used_by: []
label: checked
idea_sha256: 09709a40a05d3e740d6bee9bdd5ccdf5a028bd9d5bba5301d1af412bdc6eddbb
source_sha256: 50b99b22e9f4257f0171f167e9b6bae933149e9059917cc23a9e0bf36f8f8772
cargo_sha256: 5c3d5009de6c2099b7bdbc74101b6e4cec227aa48a52a5b04e1cb56bdaa5379b
world_sha256: e28475e151f030a35f872dae67f0acfd874b2a919f2952c1d61dfda0997fb274
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T12:49:15Z }
verified:
  - { by: human:owner, at: 2026-11-14T12:00:00Z }
  - { by: process:lengua-check, at: 2026-11-14T12:49:15Z }
title: ask
status: stable
approved: { sha256: a9e622f74aff4c31fe3837bae568ca6fb9d77a9051f61a7d546969019fad729c, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program asks an AI model and another service, through its host, and writes what it gets.
The first line of the standard input is a mode, and the text after it is a note. It shows each
part of the grant of an AI model. The parts are a whole answer, an answer in pieces,
an answer in JSON, a vector and the limits. It also calls a service by its host name with a key
that the host gives as a secret.

The program uses the libraries `ai` and `vectors`. In the tests, the examples give the answers
of the model, so the tests use no network.

# Behaviours

## Ask for a title

Given a note and the answer of the model,
When the mode is `ask`,
Then the program writes the answer.

### Example: a title for a note

- Standard input: "ask\nBuy milk and tea."
- Model answers: "Shopping"
- Expect standard output: "Shopping\n"
- Expect exit code: 0

### Example: no model

- Standard input: "ask\nBuy milk."
- Expect standard output: "the model failed for a short time (503)\n"
- Expect exit code: 0

## Get an answer in pieces

Given the answer of the model,
When the mode is `stream`,
Then the program reads the answer in pieces and writes it.

### Example: three words

- Standard input: "stream\nBuy milk."
- Model answers: "Hello big world"
- Expect standard output: "streamed in more than one piece: Hello big world\n"
- Expect exit code: 0

### Example: two words

- Standard input: "stream\nBuy milk."
- Model answers: "Hello world"
- Expect standard output: "streamed in more than one piece: Hello world\n"
- Expect exit code: 0

### Example: one word

- Standard input: "stream\nBuy milk."
- Model answers: "Hello"
- Expect standard output: "streamed in one piece: Hello\n"
- Expect exit code: 0

## Get an answer in JSON

Given an answer in JSON,
When the mode is `json`,
Then the program writes the field `title`, or the error of the answer.

### Example: a good answer

- Standard input: "json\nBuy milk."
- Model answers: "{\"title\":\"Milk\"}"
- Expect standard output: "title: Milk\n"
- Expect exit code: 0

### Example: an answer with no title

- Standard input: "json\nBuy milk."
- Model answers: "{\"name\":\"Milk\"}"
- Expect standard output: "the answer has no field `title`\n"
- Expect exit code: 0

### Example: an answer in a list

- Standard input: "json\nBuy milk."
- Model answers: "[\"Milk\"]"
- Expect standard output: "the answer is bad: NotObject\n"
- Expect exit code: 0

### Example: an answer that is not JSON

- Standard input: "json\nBuy milk."
- Model answers: "Milk"
- Expect standard output: "the model failed for a short time (502)\n"
- Expect exit code: 0

## Compare the vector of a note

Given the vector of a note from the model,
When the mode is `embed`,
Then the program writes the cosine of its angle with the vector `[1, 0]`.

### Example: a vector

- Standard input: "embed\nBuy milk."
- Model answers: "[3, 4]"
- Expect standard output: "0.6\n"
- Expect exit code: 0

### Example: a vector of words

- Standard input: "embed\nBuy milk."
- Model answers: "[\"a\"]"
- Expect standard output: "the answer is not a vector\n"
- Expect exit code: 0

## Stop at the limits of the grant

Given a grant of two calls and 60 tokens,
When the program asks the model too many times, or gets a long answer,
Then the host refuses the call with the status 429.

### Example: three calls

- Standard input: "thrice\nBuy milk."
- Model answers: "Milk"
- Expect standard output: "200 200 429\n"
- Expect exit code: 0

### Example: a long answer

- Standard input: "ask\nBuy milk."
- Model answers: "This answer has many more words than the grant of the idea lets the model give to the program, so the host refuses it and gives the status of a refusal in place of the answer itself, with a message. It is long."
- Expect standard output: "the model refused (429): the answer has more tokens than the idea grants\n"
- Expect exit code: 0

## Call a service by its name

Given a service at `localhost:7813` and a key in the secret `SERVICE_KEY`,
When the mode is `call`,
Then the program sends the key to the service and writes the answer.

### Example: the service answers

- Standard input: "call\n"
- Server `localhost:7813`: "HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"
- Expect standard output: "200 hello, and the key has 16 letters\n"
- Expect exit code: 0

## Refuse a mode that is not known

Given a mode that is not known,
When the program reads it,
Then it writes that the mode is not known.

### Example: another mode

- Standard input: "dance\n"
- Expect standard output: "`dance` is not a mode\n"
- Expect exit code: 0

# Constraints

- Reads: standard input
- Reads: the secret `SERVICE_KEY`
- Asks: an AI model, for a title or the vector of a note, up to 2 calls and 60 tokens
- Calls: HTTP at `localhost:7813`
- Memory: 32 MiB
- Time: 5 s
- Uses crate: `wasip2` 2.0.1, for the interface of the program to its host, the HTTP world of `WASI`
- Uses: `../../libraries/ai`, `../../libraries/vectors`

# Structure

- `main`: reads the mode and the note, and writes the answer of the mode.
- `answer`: gives the text to write for a mode and a note.
- `reply`: gives the text for the answer of a request, or for its status.
- `streamed`: tells if the answer came in more than one piece, with the answer.
- `titled`: gives the field `title` of an answer in JSON, or the error of the answer.
- `compared`: gives the cosine of the vector of an answer and the vector `[1, 0]`.
- `statuses`: asks the model three times and gives the three statuses.
- `called`: sends the secret key to the service and gives its status and answer.
- `send`: sends a request through the host and gives the status and the pieces of the answer.
- `write_body`: writes the body of a request.
- `read_pieces`: reads the body of an answer, one piece at a time.

# Links

* Uses: [ai](../../libraries/ai/idea.md), [vectors](../../libraries/vectors/idea.md).
* Index: [ask](index.md).
