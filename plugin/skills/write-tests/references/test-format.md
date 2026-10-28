# tests.toml format

`tests.toml` has one `[meta]` table, one `[[example]]` table per behavior, and `[[generated]]`
tables for every behavior whose example has inputs.

## [meta]

| Field | Meaning |
|---|---|
| `spec` | The approved spec's label from the `status` tool: `"sha256:<64 hex digits>"` |

## [[example]]

A test with fixed values, copied from the spec's behavior.

| Field | Default | Meaning |
|---|---|---|
| `id` | required | The behavior's ID, such as `"B1"` |
| `args` | `[]` | Command-line arguments, without the program name |
| `stdin` | `""` | Everything the program can read; then input ends |
| `stdout` | `""` | Exact standard output |
| `stderr` | `""` | Exact standard error |
| `exit_code` | required | 0 to 255, the code the spec states; never 134 |

Example values are literal: braces in them are plain characters.

## [[generated]]

A test that draws new inputs on every run and computes the expected results.

| Field | Default | Meaning |
|---|---|---|
| `id` | required | The behavior's ID |
| `runs` | `100` | 1 to 1000 runs |
| `vars` | required | Each variable's generator, such as `{ a = "int(-10, 10)" }` |
| `args` | `[]` | Templates that hold bare variables only, such as `"{a}"` or `"--n={a}"` |
| `stdin` | `""` | A template that holds bare variables only |
| `stdout`, `stderr` | `""` | Templates with any expressions |
| `exit_code` | required | A number, or a template that is one placeholder, such as `"{if(a < 0, 1, 0)}"` |

Runs try boundary values first: every combination when there are 64 or fewer, otherwise each
boundary value once. Random values fill the remaining runs, and a failing run reports its inputs
and the seed that drew them.

Several blocks may share an `id` when one range can't cover the inputs, such as divisors on both
sides of zero. Their `args`, `stdin`, `stdout`, `stderr` and `exit_code` must be identical; only
`vars` and `runs` differ.

## Generators

| Generator | Draws | Boundary values tried first |
|---|---|---|
| `int(lo, hi)` | Whole numbers from `lo` to `hi` inclusive, within 64 bits | `lo`, `hi`, 0, 1 and -1, when in range |
| `choice(a, b, …)` | One of the values: all numbers or all strings | Every value (the first and last when there are more than 16) |
| `string(min, max, "chars")` | `min` to `max` characters (at most 4096) drawn from `chars` | The shortest string of the first character, the longest of the last |

Arguments must be literal values. The limits of `int` and the lengths of `string` must be numbers
that the `# Purpose` section of `spec.md` writes in backticks, so every range comes from the
approved spec. Numbers from behaviors don't count: an example's own inputs aren't a range.

For input that isn't valid, list hand-picked values with `choice`: the example's own value plus
values that go wrong late, such as `choice("abc", "12x", "-", "1.5", "--1", "")` for a number.

## Templates

Text with `{expression}` placeholders. `{{` and `}}` stand for literal braces. Numbers print in
decimal and strings as they are; true and false can't be printed, so use `if` to choose text.

## Expressions

- **Numbers** are whole and checked 128-bit: overflow is an error, never a wrapped value.
- **Strings** are written `"text"`, with the escapes `\"`, `\\`, `\n` and `\t`.
- **True and false** come from comparisons, or from `true` and `false`.
- **Operators**, from lowest precedence to highest: `||`, then `&&`, then the comparisons
  `== != < <= > >=` (one per comparison; strings compare only with `==` and `!=`), then `+ -`,
  then `* / %`, then unary `-` and `!`. Parentheses group.
- **Division** truncates toward zero and `%` keeps the sign of the dividend, like the CPU's
  `idiv`: `-7 / 2` is `-3` and `-7 % 2` is `-1`. Dividing by zero is an error.
- **Variable names** use lowercase letters, digits and `_`, start with a letter or `_`, and aren't
  function or generator names.

| Function | Result |
|---|---|
| `len(s)` | Length of `s` in bytes |
| `abs(n)` | Absolute value |
| `min(a, b, …)`, `max(a, b, …)` | Smallest or largest number |
| `if(condition, then, else)` | `then` or `else`; only the chosen one is evaluated, so `if(b == 0, 0, a / b)` is safe |
| `upper(s)`, `lower(s)` | ASCII letters in upper or lower case |
| `reverse(s)` | The characters in reverse order |
| `count(s, part)` | How many times `part` appears in `s`, without overlaps: `count(line, "e")` |

## Writing TOML strings

- Basic strings `"…"` use escapes: `\n` newline, `\t` tab, `\"` quote, `\\` backslash.
- Literal strings `'…'` have no escapes, which suits generators that hold quotes:
  `vars = { s = 'string(1, 10, "abc")' }`.
- Multi-line strings `"""…"""` keep their newlines, except one right after the opening `"""`.

## What check_tests checks

1. The file is valid TOML and uses only the fields above.
2. `[meta].spec` is the approved spec's label.
3. There is exactly one example per behavior, and no unknown IDs.
4. Each example has its behavior's exit code and contains every backtick value of the behavior.
5. Each behavior whose example has inputs has a generated block.
6. Generators are valid and their limits are numbers from the spec's Purpose.
7. `args` and `stdin` hold bare variables; every variable is declared and reaches the program.
8. A dry run computes every run's expected results: no overflow, no division by zero, and exit
   codes from 0 to 255 that are never 134.
9. Output with placeholders differs between runs, so a program that prints a constant can't pass.
10. Blocks of one behavior share their templates.
11. Given each example's inputs, the formulas produce the example's results.

When everything passes, `lengua.lock` records the tests and the server keeps a copy. The tests
then stay frozen until the spec changes, and the copy can restore them.

## More examples

### Reading standard input

```markdown
---
type: lengua/program-spec
title: shout
description: Reads one line from standard input and prints it in capital letters.
---
# Purpose
Reads a line of `1` to `80` lowercase letters and spaces from standard input, then prints it in
capital letters.

# Behaviors
## B1: Shouts a line
- Given the input `hello world` followed by a newline
- When the program runs
- Then it prints `HELLO WORLD` followed by a newline
- And it exits with code `0`

## B2: Rejects empty input
- Given empty input
- When the program runs
- Then it prints `error: no input` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
```

```toml
[meta]
spec = "sha256:<from status>"

[[example]]
id = "B1"
stdin = "hello world\n"
stdout = "HELLO WORLD\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "error: no input\n"
exit_code = 1

[[generated]]
id = "B1"
vars = { line = 'string(1, 80, "abcdefghijklmnopqrstuvwxyz ")' }
stdin = "{line}\n"
stdout = "{upper(line)}\n"
exit_code = 0
```

### Two ranges and an error

```markdown
---
type: lengua/program-spec
title: divide
description: Divides one whole number by another and prints the quotient, rounded toward zero.
---
# Purpose
Divides a whole number between `-1000000` and `1000000` by a whole number between `-1000` and
`-1` or between `1` and `1000`, and prints the quotient rounded toward zero.

# Behaviors
## B1: Divides two numbers
- Given the arguments `7` and `2`
- When the program runs
- Then it prints `3` followed by a newline
- And it exits with code `0`

## B2: Rejects division by zero
- Given the arguments `7` and `0`
- When the program runs
- Then it prints `error: division by zero` followed by a newline to standard error
- And it exits with code `2`

# Constraints
- Allowed system calls: write, exit
```

```toml
[meta]
spec = "sha256:<from status>"

[[example]]
id = "B1"
args = ["7", "2"]
stdout = "3\n"
exit_code = 0

[[example]]
id = "B2"
args = ["7", "0"]
stderr = "error: division by zero\n"
exit_code = 2

[[generated]]  # negative divisors
id = "B1"
vars = { a = "int(-1000000, 1000000)", b = "int(-1000, -1)" }
args = ["{a}", "{b}"]
stdout = "{a / b}\n"
exit_code = 0

[[generated]]  # positive divisors
id = "B1"
vars = { a = "int(-1000000, 1000000)", b = "int(1, 1000)" }
args = ["{a}", "{b}"]
stdout = "{a / b}\n"
exit_code = 0

[[generated]]
id = "B2"
runs = 50
vars = { a = "int(-1000000, 1000000)", b = "choice(0)" }
args = ["{a}", "{b}"]
stderr = "error: division by zero\n"
exit_code = 2
```
