---
type: lengua Module
description: "The module keeps the accounts of the users in the file `data/users.txt`, one account on a line."
module: accounts
source: src/accounts.rs
uses: [ideas/notes]
used_by: [idea, ideas/notes]
label: checked
idea_sha256: 1eeca9403e02620668b7d47c627b1969b76bf8336cc71602491685c941edd447
source_sha256: e4571e36c096c054eff70ffbc0757996070dade5dbede676e00d242cff17eac0
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T15:31:10Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T15:31:10Z }
title: accounts
status: draft
---

# Purpose

The module keeps the accounts of the users in the file `data/users.txt`, one account on a line.
A line has the name and the hash of the password. The module makes the hash and the token of a
session with the library `logins`. The salt of a hash comes from random numbers. The key of the
tokens is the secret `SESSION_KEY`, and a token stops one hour after the time of the clock.

# Behaviours

## Make an account

Given a name and a password,
When a client sends them with `POST /api/register`,
Then the answer is 201 with the name, and the server keeps the account.

### Example: a new account

- Request: `POST /api/register`
- Request body: "{\"name\":\"ann\",\"password\":\"green tea\"}"
- Expect status: 201
- Expect body: "{\"name\":\"ann\"}"

### Example: a name with a large letter

- Request: `POST /api/register`
- Request body: "{\"name\":\"Ann\",\"password\":\"green tea\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A name has 1 to 20 small letters.\"}"

### Example: a name that is too long

- Request: `POST /api/register`
- Request body: "{\"name\":\"abcdefghijabcdefghijk\",\"password\":\"green tea\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A name has 1 to 20 small letters.\"}"

### Example: a name of 20 letters

- Request: `POST /api/register`
- Request body: "{\"name\":\"abcdefghijabcdefghij\",\"password\":\"12345678\"}"
- Expect status: 201
- Expect body: "{\"name\":\"abcdefghijabcdefghij\"}"

### Example: a short password

- Request: `POST /api/register`
- Request body: "{\"name\":\"ann\",\"password\":\"1234567\"}"
- Expect status: 400
- Expect body: "{\"error\":\"A password has 8 characters or more.\"}"

### Example: a name with an account

- File `data/users.txt`: "ann\tpbkdf2-sha256$1000$a1b2c3d4$3bfd31d78939ea9ee2b68254c3ca3ccf9f4d3942b7b81ac6038b68947e2c937c\n"
- Request: `POST /api/register`
- Request body: "{\"name\":\"ann\",\"password\":\"green tea\"}"
- Expect status: 409
- Expect body: "{\"error\":\"This name has an account.\"}"

## Sign in

Given the account of `ann`,
When a client sends her name and password with `POST /api/login`,
Then the answer is 200 with a token of a session, or 401 for a wrong password.

### Example: the right password

- File `data/users.txt`: "ann\tpbkdf2-sha256$1000$a1b2c3d4$3bfd31d78939ea9ee2b68254c3ca3ccf9f4d3942b7b81ac6038b68947e2c937c\n"
- Request: `POST /api/login`
- Request body: "{\"name\":\"ann\",\"password\":\"green tea\"}"
- Expect status: 200
- Expect body: "{\"token\":\"ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24\"}"

### Example: a wrong password

- File `data/users.txt`: "ann\tpbkdf2-sha256$1000$a1b2c3d4$3bfd31d78939ea9ee2b68254c3ca3ccf9f4d3942b7b81ac6038b68947e2c937c\n"
- Request: `POST /api/login`
- Request body: "{\"name\":\"ann\",\"password\":\"black tea\"}"
- Expect status: 401
- Expect body: "{\"error\":\"The name or the password is wrong.\"}"

## Know the user of a token

Given a token of a session,
When a client sends `GET /api/me` with the token,
Then the answer is 200 with the name, or 401 when the token is not good.

### Example: a good token

- Request: `GET /api/me`
- Request header `authorization`: "Bearer ann.1767229200.7600c8ab4a9f796bbef41d93945dfd379637ea30660a254e5a52e3b3e33ffa24"
- Expect status: 200
- Expect body: "{\"name\":\"ann\"}"

### Example: a token that has stopped

- Request: `GET /api/me`
- Request header `authorization`: "Bearer ann.1767225600.74063eb626eb2955a3c23c6ed38d4d06b00e383e6026f712c06688d3f94333ae"
- Expect status: 401
- Expect body: "{\"error\":\"Sign in first.\"}"

### Example: no token

- Request: `GET /api/me`
- Expect status: 401
- Expect body: "{\"error\":\"Sign in first.\"}"

# Constraints

- Uses: `notes`

# Structure

- `register`: keeps a new account with the hash of its password, or gives an error.
- `login`: gives a token for a name and its password, or an error.
- `me`: gives the name of the token in a header, or an error.
- `good_name`: tells if a name has 1 to 20 small letters.
- `now`: gives the seconds from 1970-01-01 of the clock.
- `key`: gives the secret key of the tokens.
- `load`: gives the accounts of the file.
- `save`: writes the accounts to the file, one account on a line.

# Links

* Uses: [notes](notes.md).
* Used by: [notes](../idea.md), [notes](notes.md).
* Serves: [`POST /api/register`](../api.md), [`POST /api/login`](../api.md), [`GET /api/me`](../api.md).
* Called by: [ui/login](ui/login.md).
* Index: [notes](../index.md).
