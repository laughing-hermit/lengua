---
type: lengua Module
description: "The module sends each path to its page, and writes the answers and the errors as JSON."
module: pages
source: src/pages.rs
uses: []
used_by: [idea]
label: unchecked
idea_sha256: bab1901d5e1eb171ee3ea46595bbf5dacf94a4a260a93a132a8e9bd0d4d2560f
source_sha256: 937471c53a73b8b82088aab4a4ea03c181eb11294ca830c5d47cc451d8692e3a
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T06:11:37Z }
title: pages
status: draft
---

# Purpose

The module sends each path to its page, and writes the answers and the errors as JSON. The
total has the net sum, the tax from the component in C and their sum, each with two places.

# Behaviours

## Give the total of one item

Given a basket of one `tea`,
When a client sends `GET /total?tea=1`,
Then the tax is 0.20 and the total is 2.70.

### Example: one `tea`

- Request: `GET /total?tea=1`
- Expect status: 200
- Expect body: "{\"net\":\"2.50\",\"tax\":\"0.20\",\"total\":\"2.70\"}"

# Structure

- `route`: gives the status and the body of the page of a path.
- `total`: gives the net sum, the tax and the total of a basket, or an error.
- `error`: gives a status and a JSON object with the error.

# Links

* Used by: [shop](../idea.md).
* Index: [shop](../index.md).
