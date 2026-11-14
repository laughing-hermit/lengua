---
okf_version: "0.2"
---

# Back end: program

* [ask](idea.md) - The program asks an AI model and another service, through its host, and writes what it gets. Label: `checked`.

# Back end: libraries

* [ai](../../libraries/ai/idea.md) - The library helps a program that asks an AI model. Label: `checked`.
* [vectors](../../libraries/vectors/idea.md) - The library finds the texts that are near to a question by their vectors. Label: `checked`.

# Back end: crates

* Declared in the ideas: 1: `wasip2` 2.0.1.
* Brought in by them: 2.
* With `unsafe` in their Rust code: 3: `bitflags` 2.13.2, `wasip2` 2.0.1+wasi-0.2.12, `wit-bindgen` 0.62.0.
* Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies): none.
