---
type: lengua Server
description: "The program is a desktop app."
module: main
source: src/main.rs
uses: [ideas/ui/home, ../../libraries/json/idea]
used_by: []
label: checked
idea_sha256: ce8028a930da2bf0df4453774ed2040cebc231baacdb87ed549d49018bb41ffe
source_sha256: 8fe52be33c3a9f52bd888c7c600e4f85a0a6d1974546c7e85536041b6f86ce83
cargo_sha256: dbb9541af7745d495a909e05bec141a6f7705ead1d094829219d6d1709cbc5ff
world_sha256: e1fa39b4514b921fb087df71d46f9bda8c247415e4e9fee0409382456311ad86
web_sha256: c3a1a43bde7ed2e7f894e1806dab079c896ca9284da297e5ecaa46dfa856a77d
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:26:19Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:26:19Z }
title: desk
status: draft
approved: { sha256: 7f62915ffbcb2ddb38a1f0b527fee78627be601b8380dfecc57e31e8b63e4dc6, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program is a desktop app. Its window shows a screen with a form for a prompt, and the
server gives the prompt to a small language model. The model is the native part `model` in the
folder next to it. The server is one WebAssembly file, and `lengua-run` runs it beside the window.
The host starts the native part for each request, as a program of the computer.

The server reads the prompt from a JSON body with the library `json`, and it sends the prompt to
the native part. Then it gives the text of the model in JSON. Any other path is a file of the
front end.

# Behaviours

## Write the next letters of a prompt

Given a prompt of letters,
When a client sends `POST /api/write` with the prompt,
Then the answer is 200 with the prompt and the letters that the model gives.

### Example: two letters

- Request: `POST /api/write`
- Request body: "{\"prompt\":\"ab\"}"
- Expect status: 200
- Expect header `content-type`: "application/json"
- Expect body: "{\"text\":\"abfbedbfbe\"}"

### Example: a word

- Request: `POST /api/write`
- Request body: "{\"prompt\":\"hello\"}"
- Expect status: 200
- Expect body: "{\"text\":\"helloeedbfbed\"}"

## Refuse a long prompt

Given a prompt of more than 40 characters,
When a client sends it,
Then the answer is 400 with an error, and the server does not start the model.

### Example: a long prompt

- Request: `POST /api/write`
- Request body: "{\"prompt\":\"a long prompt of more than forty letters in all\"}"
- Expect status: 400
- Expect body: "{\"error\":\"Give a prompt of 40 characters or fewer.\"}"

## Refuse a body that is not a prompt

Given a body with no prompt,
When a client sends it,
Then the answer is 400 with the same error.

### Example: no prompt

- Request: `POST /api/write`
- Request body: "{\"prompt\":\"\",\"more\":1}"
- Expect status: 400
- Expect body: "{\"error\":\"Give a prompt of 40 characters or fewer.\"}"

## Serve the front end

Given the files of the front end,
When a client asks for a path that is not a route,
Then the answer is the file, or 404.

### Example: the first page

- Request: `GET /`
- Expect status: 200
- Expect header `content-type`: "text/html; charset=utf-8"

### Example: a path that is not there

- Request: `GET /api/other`
- Expect status: 404
- Expect body: "not found"

# Constraints

- Serves: HTTP on TCP port 7820
- Native: `../model`, for the fastest local inference
- Native: a desktop window, for the Electron shell of a desktop app
- Memory: 64 MiB
- Time: 20 s
- Modules: `ui/home`
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`
- Uses: `../../libraries/json`
- Route: `POST /api/write`
- Uses shadcn: `button`, `card`, `input`, `label`

# Structure

- `Desk::handle`: answers a request with what `answer` gives.
- `answer`: answers `POST /api/write` with the text of the model, and any other path with a file of the front end.
- `written`: reads the prompt of a body, asks the model, and gives the status and the JSON.
- `prompt_of`: gives the prompt of a JSON body, or nothing when the prompt is empty or too long.
- `error`: gives the JSON of an error.
- `ask`: sends the prompt to the native part and gives its text.
- `body_of`: reads the body of a request as text.
- `respond`: writes the answer to the client.

# Links

* Uses: [ui/home](ideas/ui/home.md), [json](../../libraries/json/idea.md).
* Serves: [`POST /api/write`](api.md).
* Called by: [ui/home](ideas/ui/home.md).
* Index: [desk](index.md).
