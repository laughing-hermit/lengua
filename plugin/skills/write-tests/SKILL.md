---
name: write-tests
description: Writes a lengua program's tests.toml from its approved spec.md, with one example per behavior holding the spec's own values plus generated tests whose formulas compute the expected results for fresh random inputs on every run. Use after a spec is approved or changed, when the lengua status tool says to write tests.toml, or when check_tests reports problems. Ends with check_tests, which records the tests; they freeze when code first runs against them.
license: Apache-2.0
compatibility: Needs the lengua MCP server from the lengua plugin.
---

# Write lengua tests

`tests.toml` turns the approved spec into tests. Write it from `spec.md` alone, never from code:
the tests come before the code, and they decide whether the code is right.

## Steps

1. Take the spec label (`sha256:…`) from the last `next_step`, which names it. If you don't have
   it, call the lengua `status` tool with `path` and read `program.spec_label`.
2. Read `<path>/spec.md`.
3. Write `<path>/tests.toml` by the rules below.
   [The format reference](references/test-format.md) lists every field and function, with more
   examples.
4. Call `check_tests` with `path`. It lists every problem at once; fix them all and call it again
   until it passes.
5. Follow the `next_step` it returns. Next comes `main.s`, with the write-assembly skill.

## Rules

- **`[meta]`** holds `spec = "<spec_label>"`, the label from step 1.
- **One `[[example]]` per behavior,** with `id` set to the behavior's ID and the behavior's
  values copied exactly into `args`, `stdin`, `files`, `stdout`, `stderr`, `files_after` and
  `exit_code`. Every backtick value of the behavior must appear in its example.
- **Output is exact,** including the final newline (`"5\n"`). Leave out a stream the program
  doesn't write; it must then stay empty.
- **Random bytes and the clock come from the test** when the spec gives them: the example has
  `random = "fc2a"` (hex) or `clock = ["1735689600"]` (seconds since 1970), and the generated
  block draws them, `random = "{hex(first, 1)}{hex(second, 1)}"` with
  `first = "int(0, 255)"`, or `clock = ["{t}"]`. The output is then exact. A variable that only
  fills random bytes draws every value they can hold (`int(0, 255)` for one byte); one that only
  sets the clock draws seconds from 0 up. Bytes past the end of `random` are zeros, and the last
  reading repeats.
- **Network tests bring their own server or client.** For a program that connects:
  `server = { receives = "GET / HTTP/1.0\r\n\r\n", replies = "HTTP/1.0 200 OK\r\n\r\nhi" }`
  (a list for several connections), and `{port}` in args or stdin where the program needs the
  server's port: the test's server listens on a free port of its own and lengua writes it in.
  For a server: `client = [{ sends = "hello\n", receives = "hello\n" }]`, plus
  `serves = true` and no `exit_code` when the spec says it keeps serving. The server reads
  exactly `receives`; the client reads all the program answers until it closes the connection.
- **Output the program chooses** without the test supplying it is a rule: in the example,
  `stdout = { matches = "rolled {any(1, 6)}\n" }`, and in the generated block,
  `stdout = "rolled {any(1, 6)}\n"`. `now` is when the run starts in Unix seconds, and only
  bounds a rule: `{any(now - 2, now + 2)}`.
- **`exit_code` is required** and equals the code the spec states. 134 is reserved for failed
  assertions.
- **A `[[generated]]` block for every behavior whose example has args, stdin or files,** with the
  same `id`. Its `vars` draw new inputs on every run, and its formulas compute the expected
  results.
- **Files.** A behavior that reads a file gives it in `files`, and one that writes a file expects
  it in `files_after`: `files = { "notes.txt" = "{text}" }` and
  `files_after = { "loud.txt" = "{upper(text)}" }`. After the run the working folder must hold
  exactly those files, so leave out nothing the program writes.
- **Ranges come from Purpose.** `int(lo, hi)` and `string(min, max, "chars")` may only use
  numbers that the spec's `# Purpose` writes in backticks; numbers from behaviors don't count.
  If Purpose doesn't state a range you need, ask the user to add it to `spec.md` and approve the
  spec again.
- **Long literal inputs,** such as a line or a file just over a limit, are written as repeats
  in the example: `files = { "big.txt" = [{ repeat = "a", times = 1048576 }, "x"] }`. Never
  generate a huge string into tests.toml.
- **Invalid input gets hand-picked values.** For a behavior about input that isn't valid, use
  `choice(…)` with the example's own value plus values that go wrong late, not only at the first
  character. For a number: `"12x"`, `"-"`, `"1.5"`, `"--1"` and `""`.
- **Decimals.** Draw decimal inputs with `decimal(lo, hi)`, written with Purpose's places. For
  exact decimals such as money, compute in whole units and print with `places`:
  `{places(scaled(a, 2) + scaled(b, 2), 2)}`. For measured decimals, compute in doubles and print
  with `fixed`: `{fixed(sqrt(x), 3)}`, or in singles when the spec says single precision:
  `{fixed(sqrt(single(x)), 3)}`. Write a multiply followed by an add, as in a dot product or a
  polynomial, as `fma(a, b, c)`: it rounds once, which is more accurate, and lets the program use
  one `vfmadd` instruction. Write `2.0`, not `2`, next to decimals.
- **Inputs are bare variables.** In `args` and `stdin`, a placeholder holds just a variable
  (`{a}`); computations belong in `stdout`, `stderr` and `exit_code` (`{a + b}`).
- **Small ranges are checked in full.** A block whose ranges hold at most 5000 combinations (and
  no `choice`) runs every one of them, so a range the spec states exactly is worth more than a
  wide one.
- **Formulas agree with the spec.** Given the example's inputs, they must produce the example's
  results. They must never overflow or divide by zero for inputs in range, and the output must
  change with the inputs.
- **One formula per output.** Compute text output from the whole input with the functions, such
  as `{upper(text)}` or `{tr(text, "A-Za-z", "N-ZA-Mn-za-m")}`, not one variable per character.
- **Joined text counts whole.** A template that adds text around a variable, such as `"{a}x"`
  or `"{word} {rest}"`, gives the program more than the variable alone: keep the total inside
  the spec's limits for behaviors about valid input.
- **Lengths count characters.** `string(0, 1048576, "aé€")` can reach 3 MiB, since `é` takes 2
  bytes and `€` 3, while specs and programs count bytes. Draw long text from 1-byte (ASCII)
  characters, and test other characters in short strings.
- **Tests freeze when code first runs against them.** Until `check_code` runs them, a mistake
  can be fixed in `tests.toml` and checked again. After that, they can't change to fit the code.
  If a frozen test turns out to be wrong, fix `tests.toml`, tell the user in a line or two what
  was wrong, and ask them to approve `spec.md` again (it didn't change): their approval reopens
  the tests. Don't change the spec just to unfreeze them.

## Example

For this spec:

```markdown
---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Lets a shell script add two whole numbers: it prints the sum of two numbers between `-1000000000`
and `1000000000`, given as arguments. If writing the output fails, the program exits with code 2.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

## B2: Shows usage when arguments are missing
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
