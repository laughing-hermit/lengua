---
type: lengua Screen
description: "The screen is the window of the desktop app."
module: ui/home
source: web/screens/home.tsx
uses: []
used_by: [idea]
label: checked
idea_sha256: 37b29caebc456745c67049c3dba7664f6e3f7131e451938d2d3a8d2b8eb237d8
source_sha256: d1fdf091f4b50310ae7f5ba5ae7919082fb639645e87e4f4739360f84a71c334
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T13:34:47Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T13:34:47Z }
title: ui/home
status: draft
---

# Purpose

The screen is the window of the desktop app. It has a form for a prompt, and it shows the
letters that the model gives after the prompt.

# Behaviours

## Write the next letters

Given the prompt `ab`,
When the user clicks `Write`,
Then the screen shows the text of the model.

### Example: two letters

- Type into `Prompt`: "ab"
- Click: `Write`
- Expect text: "abfbedbfbe"
- Expect state `written`: true

## Show the form first

Given no prompt,
When the screen opens,
Then it shows the form and no text.

### Example: the form

- Expect label: `Prompt`
- Expect text: "Write with the local model"
- Expect state `written`: false

# Structure

- `HomeScreen`: shows the form, the text of the model and the error.
- `write`: sends the prompt to the server and keeps the text or the error.

# Links

* Used by: [desk](../../idea.md).
* Calls: [`POST /api/write`](../../api.md).
* Index: [desk](../../index.md).
