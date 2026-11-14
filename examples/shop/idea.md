---
type: lengua Server
description: "The program is the HTTP server of a small shop."
module: main
source: src/main.rs
uses: [ideas/catalog, ideas/basket, ideas/pages, ../../libraries/json/idea, ../../libraries/decimal/idea]
used_by: []
label: checked
idea_sha256: 6de32c9b2b4463dab867da9b9c30f2638a936a30e00c516da32ea4ddb76f9640
source_sha256: 25e2f5222330b87373e03f87e859b22c7ac993a9749041af007aed87e655128a
cargo_sha256: b890b840c9bd3eabcc1b59fdfa73e6f4d6efceead9a1f01ee313da4f5a1f6ac4
world_sha256: 6aa2f03d29a6dae284b24b3801913f715788aad0e8f74739ae1fde5efab7af5d
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T10:12:36Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T10:12:36Z }
title: shop
status: draft
approved: { sha256: 88b2ba64344288d7cbaaf5779371e619eb81845c9d2d5e899e752e50d4d9edac, by: human:owner, at: 2026-11-14T12:00:00Z }
---

# Purpose

The program is the HTTP server of a small shop. It lists the items of its catalog with their
prices, and it gives the total of a basket, with the tax, as JSON. It is one WebAssembly file
of three modules, `catalog`, `basket` and `pages`. It uses two libraries of `lengua`,
`json` and `decimal`, and a component in C that gives the tax.

# Behaviours

## List the items

Given the catalog of three items,
When a client sends `GET /items`,
Then the answer is 200 with the names and the prices of the items as JSON.

### Example: the items

- Request: `GET /items`
- Expect status: 200
- Expect header `content-type`: "application/json"
- Expect body: "[{\"name\":\"tea\",\"price\":\"2.50\"},{\"name\":\"cake\",\"price\":\"3.20\"},{\"name\":\"jam\",\"price\":\"4.75\"}]"

## Give the total of a basket

Given a basket of two `tea` and one `cake`,
When a client sends `GET /total?tea=2&cake=1`,
Then the answer is the net sum, the tax of 8% and the total.

### Example: two items

- Request: `GET /total?tea=2&cake=1`
- Expect status: 200
- Expect body: "{\"net\":\"8.20\",\"tax\":\"0.66\",\"total\":\"8.86\"}"

### Example: an empty basket

- Request: `GET /total`
- Expect status: 200
- Expect body: "{\"net\":\"0.00\",\"tax\":\"0.00\",\"total\":\"0.00\"}"

## Answer 404 for a page that is not there

Given the path `/other`,
When a client sends a request for it,
Then the answer is 404 with an error in JSON.

### Example: another path

- Request: `GET /other`
- Expect status: 404
- Expect body: "{\"error\":\"no page\"}"

# Constraints

- Serves: HTTP on TCP port 7811
- Memory: 64 MiB
- Time: 5 s
- Modules: `catalog`, `basket`, `pages`
- Uses: `../../libraries/json`, `../../libraries/decimal`
- Uses component: `components/tax.wasm`, built from C by `wasi-sdk` 34 and `wit-bindgen` 0.62.0, from `components/rates.c` and `components/tax.wit`
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`
- Uses crate: `wit-bindgen` 0.62.0, for the interface of the program to the component in C

# Structure

- `Shop::handle`: answers a request with the status and the JSON body that the pages give.
- `tax_for`: gives the tax of a number of cents, from the component in C.

# Links

* Uses: [catalog](ideas/catalog.md), [basket](ideas/basket.md), [pages](ideas/pages.md), [json](../../libraries/json/idea.md), [decimal](../../libraries/decimal/idea.md).
* Components: [tax](components/tax.md).
* Index: [shop](index.md).
