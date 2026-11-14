---
name: build
description: Turns approved lengua ideas into code - Rust in src/ for the back end, compiled to one WebAssembly component, and TSX in web/screens/ for the Next.js front end - with no comments, every function named in its idea's Structure, to the Power of 10 rules, the standard library first; then calls check until the program is checked. Use when the person approved the ideas and the program must be written or changed, including desktop apps and native parts.
license: Apache-2.0
---

# build

The code does what the ideas say, and nothing more. `check` holds the two to each other.

## Before any code

1. Read `index.md`, then `vision.md`, `api.md` and the ideas of the side you change. Read
   the code last.
2. Call `check`. Go on only when `approved` is true. If it is false, or a finding is
   `idea-unapproved`, the person has not approved the ideas as they are: use the skill
   `write-idea` to ask them. Never call `approve` on your own judgement.
3. Say "Where it runs" to the person, as the vision and the ideas say it.

## Steps

1. Write one file per idea: `src/main.rs` for `idea.md`, `src/<name>.rs` for
   `ideas/<name>.md` (with `mod <name>;` in `src/main.rs`), `web/screens/<name>.tsx` for
   `ideas/ui/<name>.md`. See [the back end](references/back-end.md) and
   [the front end](references/front-end.md).
2. Give every function the name its idea's `# Structure` gives it, and write no other
   function. When the code needs another function, add it to the structure first: a change of
   the structure alone needs no new approval.
3. Keep to [the rules](references/rules.md): no comments, no recursion, bounded loops, short
   functions, two assertions a function on average, no `unwrap`, no `unsafe`, no `any`.
4. Call `check`. While `build.state` is `building`, `testing` or `mutating`, call it again.
   Fix every finding: a failed example says what it expected and what came.
5. When a mutant survives, the tests can't tell it from the code. Write an example that does,
   at the boundary the note names. A new example changes a behaviour, so show it to the person
   and ask them to approve again.
6. Stop when the state is `done` and the label is `checked`. Tell the person the label, the
   "Where it runs" line, any `trusted` or `native` part, and any crate that is not pure Rust.
7. Run it with the MCP tool `run`, or `lengua-run <folder>` in a shell.

## Never

- Change an idea's behaviours or constraints to make the code pass; ask the person.
- Edit `Cargo.toml`, `wit/world.wit` or a generated file of `web/`: `check` writes them, and a
  hand edit is a finding.
- Add a crate or an npm package that the ideas don't declare.
