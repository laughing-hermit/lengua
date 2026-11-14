---
type: lengua Screen
description: "The screen is a form to sign in with a name and a password."
module: ui/login
source: web/screens/login.tsx
uses: [ideas/notes]
used_by: [idea]
label: checked
idea_sha256: 8c025d3ae4581d45dadcae910f16d7dbe0551b35a6d58fd66be47f11ce0047af
source_sha256: ddb30d34a0dbfd0c54ec4b40b75fe84aa171c0a081bffd0688f4c54f53a16e7d
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T07:56:57Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T07:56:57Z }
title: ui/login
status: draft
---

# Purpose

The screen is a form to sign in with a name and a password. It sends them to the server, and it
keeps the token of the session that the server gives. Then it sends the token with `GET /api/me`,
and it shows the name that the server gives for the token.

# Behaviours

## Sign in with the right password

Given the name `ann` and her password,
When the user clicks `Sign in`,
Then the screen shows that she is signed in.

### Example: the right password

- Type into `Name`: "ann"
- Type into `Password`: "green tea"
- Click: `Sign in`
- Expect text: "Signed in as ann."
- Expect state `signed`: true

## Show the error of a wrong password

Given the name `ann` and a wrong password,
When the user clicks `Sign in`,
Then the screen shows the error of the server.

### Example: a wrong password

- Type into `Name`: "ann"
- Type into `Password`: "black tea"
- Click: `Sign in`
- Expect text: "The name or the password is wrong."
- Expect state `signed`: false

# Constraints

- Uses: `notes`

# Structure

- `LoginScreen`: shows the form, the error and the result.
- `signIn`: sends the name and the password, then sends the token to get the name, and keeps the
  token or the error.

# Links

* Uses: [notes](../notes.md).
* Used by: [notes](../../idea.md).
* Calls: [`POST /api/login`](../../api.md).
* Index: [notes](../../index.md).
