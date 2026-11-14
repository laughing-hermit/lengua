---
type: lengua Interface
description: "The routes of the back end that the front end calls, with the shapes of their requests and answers."
label: checked
client_sha256: 65ff0e28c58ad685ff9e195f241573ac4cdfebbde74a2d65db4a4590e7c6bce0
---

# Routes

## `POST /api/write`

* Function: `postWrite` in `web/lib/lengua.ts`.
* Request body: `{ "prompt": string; "more"?: number }`.
* Answer: `{ "text": string }`.
* Failure: `{ "error": string }`.
* Statuses: 200, 400.
* Served by: [desk](idea.md).
* Called by: [ui/home](ideas/ui/home.md).

# Links

* Index: [index](index.md).
