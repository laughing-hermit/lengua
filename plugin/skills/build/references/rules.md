# The rules: the Power of 10, for Rust and TypeScript

`check` holds every rule and names it in each finding. The same bar holds both languages.

| # | Rule | Rust | TypeScript |
|---|---|---|---|
| 1 | No recursion | no function reaches itself through calls (`rule-1-recursion`); no `dyn`, no `fn(` types | no cycle through functions, components or callbacks (`ts-recursion`); no `class` (`ts-class`) |
| 2 | Bounded loops | `for` over a range, a slice or a collection; `while`/`loop` only with a counter checked against a constant and stepped by 1 at the loop's top level (`rule-2-loop`) | `for...of`, `for...in`, array methods; a counted `for` to a number, a constant or a `.length`; no `while` (`ts-loop`) |
| 3 | Memory | the idea's `- Memory:`, held by the host | the browser's own limit |
| 4 | Short functions | 60 lines or fewer after `rustfmt` (`rule-4-length`) | 60 lines or fewer as written (`ts-length`) |
| 5 | Assertions | two `assert!` or `debug_assert!` a function on average, `main` not counted (`rule-5-assertions`) | two `assert(condition, message)` a function on average, from `@/lib/lengua`; the screen's main component not counted (`ts-assertions`) |
| 6 | Smallest scope | no `static mut`, no global with interior mutability (`rule-6-static`) | no module-level `let`, `var` or `new` (`ts-global`) |
| 7 | Every result handled | no `unwrap`, `expect` or indexing that can panic; every `Result` used (clippy) | every promise awaited, returned or ended in `.catch`; no `async` event handler; no `!` (`ts-promise`, `ts-non-null`) |
| 8 | Little preprocessor | no `macro_rules!` that writes items, no `build.rs`, macros only from declared crates (`rule-8-*`) | no decorators, `eval`, `require` or `import()`; imports only from `react`, `next/link`, `@/lib/...`, `@/components/ui/...`, `@/screens/...` and declared packages (`ts-import`) |
| 9 | No `unsafe` | `unsafe` is forbidden | no `dangerouslySetInnerHTML`, `innerHTML` or `document.write` (`ts-unsafe`) |
| 10 | All warnings on | clippy `pedantic`, every warning an error; no `#[allow]` (`rule-10-allow`) | `tsc` at its strictest; no `@ts-ignore` or `eslint-disable` (`ts-ignore`) |
| 11 | No comments | none, doc comments too (`rule-11-comment`) | none, JSX comments too (`ts-comment`) |
| 12 | Every function in its idea | named in `# Structure`, and nothing else (`rule-12-*`); no nested `fn` or `mod` with a body | the same for functions and components (`ts-unnamed`, `ts-nested`, `ts-source-unlisted`) |
| 13 | Few dependencies | only crates the ideas declare | only packages the ideas declare, pinned |
| — | No escape from the types | the compiler | no `any`, no `as` cast but `as const` (`ts-any`, `ts-cast`) |

## Tests and the label

Each example of each idea runs against the built component; each screen example is a test of
the screen. Then `check` makes up to 40 mutants of the code. The label is `checked` when every
test passes and the tests catch 80% of the mutants or more, `tested` under 80%.

## Code that runs everywhere

- No threads, no `std::process`: a WebAssembly component has one thread, and a thread or a
  process is a native need (`native-undeclared`).
- A `HashMap` works with no grant and gives the same order on every run.
- The wall clock and random numbers need their grants; the tests fix both.
