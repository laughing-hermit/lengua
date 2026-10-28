---
name: write-tests
description: Writes a lengua program's tests.toml from its approved spec.md, with one example per behavior holding the spec's own values plus generated tests whose formulas compute the expected results for fresh random inputs on every run. Use after a spec is approved or changed, when the lengua status tool says to write tests.toml, or when check_tests reports problems. Ends with check_tests, which freezes the tests.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write lengua tests

`tests.toml` turns the approved spec into tests. Write it from `spec.md` alone, never from code:
the tests come before the code, and they decide whether the code is right.

## Steps

1. Call the lengua `status` tool with `path` set to the program folder's absolute path, and note
   `program.spec_label` (it looks like `sha256:…`).
2. Read `<path>/spec.md`.
3. Write `<path>/tests.toml` by the rules below.
   [The format reference](references/test-format.md) lists every field and function, with more
   examples.
4. Call `check_tests` with `path`. It lists every problem at once; fix them all and call it again
   until it passes.
5. Follow the `next_step` it returns. Next comes `main.s`, with the write-assembly skill.

## Rules

- **`[meta]`** holds `spec = "<spec_label>"`, copied from status.
- **One `[[example]]` per behavior,** with `id` set to the behavior's ID and the behavior's
  values copied exactly into `args`, `stdin`, `stdout`, `stderr` and `exit_code`. Every backtick
  value of the behavior must appear in its example.
- **Output is exact,** including the final newline (`"5\n"`). Leave out a stream the program
  doesn't write; it must then stay empty.
- **`exit_code` is required** and equals the code the spec states. 134 is reserved for failed
  assertions.
- **A `[[generated]]` block for every behavior whose example has args or stdin,** with the same
  `id`. Its `vars` draw new inputs on every run, and its formulas compute the expected results.
- **Ranges come from Purpose.** `int(lo, hi)` and `string(min, max, "chars")` may only use
  numbers that the spec's `# Purpose` writes in backticks; numbers from behaviors don't count.
  If Purpose doesn't state a range you need, ask the user to add it to `spec.md` and approve the
  spec again.
- **Invalid input gets hand-picked values.** For a behavior about input that isn't valid, use
  `choice(…)` with the example's own value plus values that go wrong late, not only at the first
  character. For a number: `"12x"`, `"-"`, `"1.5"`, `"--1"` and `""`.
- **Inputs are bare variables.** In `args` and `stdin`, a placeholder holds just a variable
  (`{a}`); computations belong in `stdout`, `stderr` and `exit_code` (`{a + b}`).
- **Formulas agree with the spec.** Given the example's inputs, they must produce the example's
  results. They must never overflow or divide by zero for inputs in range, and the output must
  change with the inputs.
- **Tests freeze when they pass.** After `check_tests` passes, `tests.toml` can't change until the
  spec does. To change a test, change the spec and get it approved again.

## Example

For this spec:

```markdown
---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`. If writing the output fails, the
program exits with code 2.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
```

the tests are:

```toml
[meta]
spec = "sha256:<from status>"

[[example]]
id = "B1"
args = ["2", "3"]
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1

[[generated]]
id = "B1"
runs = 200
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
args = ["{a}", "{b}"]
stdout = "{a + b}\n"
exit_code = 0
```

B2 has no inputs, so it needs no generated block. Each run of the generated block draws a new
`a` and `b`, trying boundary values first (the range's ends, 0, 1 and -1), and expects their
sum.
