---
type: lengua Module
description: "The module holds the items of the shop and their prices in cents."
module: catalog
source: src/catalog.rs
uses: []
used_by: [idea]
label: checked
idea_sha256: 765521e18e9bbe7b241435168cea4cc8d3eaeece547e57ed894577154f4d2de2
source_sha256: ebe159378247986c940391d9824b075148988ce116dfa065ac60333f465ed9e4
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T13:40:53Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T13:40:53Z }
title: catalog
status: draft
---

# Purpose

The module holds the items of the shop and their prices in cents. The item `tea` is 2.50, the
item `cake` is 3.20 and the item `jam` is 4.75.

# Behaviours

## Give the prices with two places

Given the price of `jam`, 475 cents,
When a client sends `GET /items`,
Then the price of `jam` is `4.75`.

### Example: the price of `jam`

- Request: `GET /items`
- Expect status: 200
- Expect body: "[{\"name\":\"tea\",\"price\":\"2.50\"},{\"name\":\"cake\",\"price\":\"3.20\"},{\"name\":\"jam\",\"price\":\"4.75\"}]"

# Structure

- `find`: gives the item with a name, or nothing.
- `listing`: gives the items as a JSON list of names and prices.

# Links

* Used by: [shop](../idea.md).
* Index: [shop](../index.md).
