# Purpose

The program is the HTTP server of a small shop. It lists the items of its catalog with their
prices, and it gives the total of a basket, with the tax, as JSON. It is one WebAssembly file
of three modules, `catalog`, `basket` and `pages`. It uses two shared libraries of lengua,
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
- `tax_of`: gives the tax of a number of cents, from the component in C.
