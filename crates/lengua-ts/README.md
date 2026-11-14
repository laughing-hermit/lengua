---
type: lengua Crate
title: lengua-ts
description: "The checks that hold the TypeScript of a lengua front end to the same rules as its Rust."
---

# lengua-ts

## Purpose

`lengua-ts` reads the TypeScript of a lengua application's screens (`web/screens/*.tsx`) with
`oxc_parser` and its AST, and holds it to the same rules as the Rust: no comments, every
function and component named in its screen idea's `# Structure`, no recursion, bounded loops,
functions of 60 lines or fewer, two `assert` calls a function on average, no module-level state,
every promise handled, no `any`, no casts, no non-null `!`, no ways to run text as code, and
imports only from the packages the idea declares. It also makes the mutants of the screens for
the mutation score. What each rule accepts and refuses is in [Lengua3.md](../../Lengua3.md),
section 5, "The same checks for Rust and TypeScript".

`oxc_parser` merits it as the only practical way to read TypeScript and TSX from Rust: writing a
parser of TypeScript is a large project of its own. The crate is not in `lengua-run`'s tree.

## Behaviours

- Given the screens of an application, each with the names of its idea's `# Structure`, the
  declared packages and the functions of the generated client, when `lint` runs, then it gives
  every finding of every rule, each with the file, the line, the code and a plain message, and
  for each screen the functions of the client that it calls.
- Given a screen that keeps every rule, when `lint` runs, then it gives no findings.
- Given a file that `oxc_parser` can't read, when `lint` runs, then it gives one `ts-read`
  finding for the file.
- Given the screens, when `groups` runs, then it gives the mutants of each top-level function
  (operators, decimal constants, booleans, `if` conditions; nothing inside `assert(...)` and
  nothing in a type), and the text of the screens with the functions named in place of their
  code, for the cache.
- Given a text, when `comment_lines` runs, then it gives the line of every comment in it.

## Structure

- `lib.rs`: `lint` parses each screen, runs the rules of the file (`comments`, `global_rules`),
  the rules of the AST (`check::run`), `check::structure`, and then the rules across the
  screens (`check::assertions`, `check::recursion`); `top_items` finds the top-level functions
  and components; `Lines` gives a line from a byte offset; `comment_lines`.
- `check.rs`: the visitor `Checker` (any, casts, non-null, classes, loops, decorators, `eval`,
  `require`, `import()`, imports, `fetch`, `dangerouslySetInnerHTML`, `innerHTML`, promises,
  the identifiers each function uses, the `assert` calls), `async_names`, `counter` (the
  bounded `for`), `structure`, `assertions`, `recursion`.
- `mutate.rs`: the visitor `Finder` and `groups`.
