---
okf_version: "0.2"
---

# Where it runs

* An Electron app: the same front end in a window, and the same WebAssembly file run by `lengua-run` beside it. Runs on Linux, macOS and Windows. Native, beside it: `model` (for the fastest local inference), built for this computer and outside the sandbox.

# Back end: program

* [desk](idea.md) - The program is a desktop app. Label: `checked`.

# Back end: libraries

* [json](../../libraries/json/idea.md) - The library reads and writes JSON text, as RFC 8259 gives it, with a value type of its own. Label: `checked`.

# Back end: native parts

* [model](../model/idea.md) - The program is a small language model, and it is a native part. Label: `native`.

# Back end: crates

* Declared in the ideas: 1: `wasip2` 2.0.1.
* Brought in by them: 2.
* With `unsafe` in their Rust code: 3: `bitflags` 2.13.2, `wasip2` 2.0.1+wasi-0.2.12, `wit-bindgen` 0.62.0.
* Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies): none.

# Front end: screens

* [ui/home](ideas/ui/home.md) - The screen is the window of the desktop app. Label: `checked`.

# Front end: npm packages

* 5 in `web/package.json` (`clsx` 2.1.1, `next` 16.4.6, `react` 19.3.1, `react-dom` 19.3.1, `tailwind-merge` 3.7.0), with the packages every front end has for its build and tests, all pinned in `web/pnpm-lock.yaml`.
* Large, over 1 MiB in pnpm's copy: `next` (187794 KiB), `react-dom` (7878 KiB), `tailwind-merge` (1027 KiB).

# Interface

* [api](api.md) - The routes of the back end that the front end calls, with the shapes of their requests and answers. Label: `checked`.
