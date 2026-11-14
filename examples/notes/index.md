---
okf_version: "0.2"
---

# Back end: program

* [notes](idea.md) - The program is a small web application for notes. Label: `unchecked`.

# Back end: modules

* [notes](ideas/notes.md) - The module keeps the notes and answers the routes under `/api/`. Label: `unchecked`.

# Back end: libraries

* [titles](../titles/idea.md) - The library checks the title of a note. Label: `checked`.
* In the browser, `titles` is `@/lib/wasm/titles`.

# Back end: crates

* Declared in the ideas: 1: `wasip2` 2.0.1.
* Brought in by them: 2.
* With `unsafe` in their Rust code: 3: `bitflags` 2.13.2, `wasip2` 2.0.1+wasi-0.2.12, `wit-bindgen` 0.62.0.
* Not pure Rust (a build script that links a native library, or `cc`, `cmake`, `pkg-config`, `bindgen` or `vcpkg` among the build dependencies): none.

# Front end: screens

* [ui/home](ideas/ui/home.md) - The screen shows the list of the notes, with a link to each note and a link to write a new note. Label: `unchecked`.
* [ui/note](ideas/ui/note.md) - The screen shows one note, and a summary of it that the AI part of the screen draws from the answer of the server. Label: `unchecked`.
* [ui/write](ideas/ui/write.md) - The screen is a form to write a new note, with a title and a text. Label: `unchecked`.

# Front end: npm packages

* 8 in `web/package.json` (`@json-render/core` 0.22.0, `@json-render/react` 0.22.0, `clsx` 2.1.1, `next` 16.4.6, `react` 19.3.1, `react-dom` 19.3.1, `tailwind-merge` 3.7.0, `zod` 4.6.7), with the packages every front end has for its build and tests, all pinned in `web/pnpm-lock.yaml`.
* Large, over 1 MiB in pnpm's copy: `@json-render/core` (1191 KiB), `next` (187794 KiB), `react-dom` (7878 KiB), `tailwind-merge` (1027 KiB), `zod` (6004 KiB).

# Interface

* [api](api.md) - The routes of the back end that the front end calls, with the shapes of their requests and answers. Label: `unchecked`.
