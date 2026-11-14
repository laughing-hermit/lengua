---
type: lengua Module
description: "The module asks an AI model for the summary of a note, through the host."
module: summary
source: src/summary.rs
uses: [ideas/notes]
used_by: [idea, ideas/notes]
label: checked
idea_sha256: d88c9e082f9fe5de466ed16e5809ced8dd971b93578c00333418ea180021b4f5
source_sha256: 8d4941f268d0e0a9a2022546ea8ce5ca875f6ce2680b9a84c4c307ea7294ed6d
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:11:53Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:11:53Z }
title: summary
status: draft
---

# Purpose

The module asks an AI model for the summary of a note, through the host. It describes a card
with the summary for the screen `note`. The front end draws the card with `json-render`. The
host holds the key of the model, so the browser never gets the key.

# Behaviours

## Refuse a summary of no note

Given a request with no number,
When a client sends `POST /api/ui`,
Then the answer is 404 with an error, and the server does not ask the model.

### Example: no number

- Request: `POST /api/ui`
- Request body: "{}"
- Expect status: 404
- Expect body: "{\"error\":\"No note has this number.\"}"

## Give an empty summary

Given an empty answer of the model,
When a client sends `POST /api/ui` with a number,
Then the answer is a card with no text.

### Example: an empty answer

- Request: `POST /api/ui`
- Request body: "{\"id\": 1}"
- Model answers: ""
- Expect status: 200
- Expect body: "{\"root\":\"card\",\"elements\":{\"card\":{\"type\":\"Card\",\"props\":{\"title\":\"Summary\"},\"children\":[\"text\"]},\"text\":{\"type\":\"Text\",\"props\":{\"content\":\"\"},\"children\":[]}}}"

# Constraints

- Uses: `notes`

# Structure

- `summary`: asks the model for the summary of a note, and gives the card or an error.
- `spec`: describes a card with the summary, in the JSON of `json-render`.
- `ask`: sends a prompt to the model through the host, and gives its answer.

# Links

* Uses: [notes](notes.md).
* Used by: [notes](../idea.md), [notes](notes.md).
* Serves: [`POST /api/ui`](../api.md).
* Called by: [ui/note](ui/note.md).
* Index: [notes](../index.md).
