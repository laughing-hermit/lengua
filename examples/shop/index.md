---
okf_version: "0.2"
---

# Back end: program

* [shop](idea.md) - The program is the HTTP server of a small shop. Label: `unchecked`.

# Back end: modules

* [basket](ideas/basket.md) - The module reads a basket from the query of a request, as pairs of an item and a count, and gives its net sum. Label: `unchecked`.
* [catalog](ideas/catalog.md) - The module holds the items of the shop and their prices in cents. Label: `unchecked`.
* [pages](ideas/pages.md) - The module sends each path to its page, and writes the answers and the errors as JSON. Label: `unchecked`.

# Back end: libraries

* [json](../../libraries/json/idea.md) - The library reads and writes JSON text, as RFC 8259 gives it, with a value type of its own. Label: `checked`.
* [decimal](../../libraries/decimal/idea.md) - The library reads and writes whole numbers and decimal numbers with a fixed number of places, as `i64` values. Label: `checked`.

# Back end: components

* [tax](components/tax.md) - A component built from C by `wasi-sdk` 34 and `wit-bindgen` 0.62.0, from `components/rates.c` and `components/tax.wit`. Label: `trusted`.

# Back end: crates

* Declared in the ideas: 2: `wasip2` 2.0.1, `wit-bindgen` 0.62.0.
* Brought in by them: 32.
* With `unsafe` in their Rust code: 20: `anyhow` 1.0.104, `bitflags` 2.13.2, `foldhash` 0.2.0, `hashbrown` 0.17.1, `indexmap` 2.14.2, `itoa` 1.0.19, `log` 0.4.35, `memchr` 2.8.3, `prettyplease` 0.3.0, `proc-macro2` 1.0.107, `semver` 1.0.28, `serde` 1.0.229, `serde_core` 1.0.229, `serde_json` 1.0.154, `syn` 3.0.10, `unicode-ident` 1.0.27, `wasip2` 2.0.1+wasi-0.2.12, `wit-bindgen` 0.62.0, `wit-bindgen-rust` 0.62.0, `zmij` 1.0.23.
* Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies): none.
