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
| `files` | `{}` | Files in the working folder before the run, by name: `{ "notes.txt" = "hello\n" }` |
| `stdout` | `""` | Exact standard output, or a rule: `{ matches = "rolled {any(1, 6)}\n" }` |
| `stderr` | `""` | Exact standard error, or a rule |
| `files_after` | `{}` | Files the program must leave, with their exact contents or a rule: `{ "loud.txt" = "HELLO\n" }` |
| `random` | none | The bytes the program's `getrandom` calls receive, in hex: `"fc2a"`. See [Random bytes and the clock](#random-bytes-and-the-clock) |
| `clock` | `[]` | What its `clock_gettime` calls read, in seconds since 1970: `["1735689600", "1735689600.25"]` |
| `server` | none | The test's own server, for a program that connects: `{ receives = "…", replies = "…" }`, or a list, one a connection. See [The network](#the-network) |
| `client` | `[]` | The test's own client, for a program that serves: `[{ sends = "…", receives = "…" }]`, one a connection |
| `serves` | `false` | `true` when the program must still be serving at the end; then there's no `exit_code` |
| `exit_code` | required | 0 to 255, the code the spec states; never 134. Left out when `serves = true` |

Example values are literal: braces in them are plain characters, except in a rule (see
[Rules](#rules-output-the-program-chooses)). A long value, such as a file one
byte over a limit, can be written as a list of strings and repeated runs instead of one string:
`files = { "big.txt" = [{ repeat = "a", times = 1048576 }, "x"] }` is 1048576 `a`s and an `x`. It
works for `args`, `stdin`, `files`, `stdout`, `stderr` and `files_after`, up to 8 MiB a value.

## [[generated]]

A test that draws new inputs on every run and computes the expected results.

| Field | Default | Meaning |
|---|---|---|
| `id` | required | The behavior's ID |
| `runs` | `100` | 1 to 1000 runs |
| `vars` | required | Each variable's generator, such as `{ a = "int(-10, 10)" }` |
| `args` | `[]` | Templates that hold bare variables only, such as `"{a}"` or `"--n={a}"` |
| `stdin` | `""` | A template that holds bare variables only |
| `files` | `{}` | File names and templates that hold bare variables only: `{ "notes.txt" = "{text}" }` |
| `stdout`, `stderr` | `""` | Templates with any expressions |
| `files_after` | `{}` | File names and templates with any expressions: `{ "loud.txt" = "{upper(text)}" }` |
| `random` | none | Hex digits and `{hex(x, n)}`: `"{hex(first, 1)}{hex(second, 1)}"` |
| `clock` | `[]` | Readings that hold bare variables only: `["{t}"]` |
| `server` | none | `receives` with any expressions, `replies` with bare variables only |
| `client` | `[]` | `sends` with bare variables only, `receives` with any expressions and rules |
| `serves` | `false` | As in an example |
| `exit_code` | required | A number, or a template that is one placeholder, such as `"{if(a < 0, 1, 0)}"`; left out when `serves = true` |

Runs try boundary values first: every combination when there are 64 or fewer, otherwise each
boundary value once. Random values fill the remaining runs, and a failing run reports its inputs
and the seed that drew them.

Small ranges run in full instead: when a block's generators are all ranges (`int`, `string`, no
`choice`) with at most 5000 combinations, every combination runs and `runs` is ignored, as long as
the whole file stays within 20000 runs. `check_code` then reports that the block tried every
value in its range: for those inputs the program is checked, not sampled.

Several blocks may share an `id` when one range can't cover the inputs, such as divisors on both
sides of zero. Their `args`, `stdin`, `files`, `stdout`, `stderr`, `files_after` and `exit_code`
must be identical; only `vars` and `runs` differ.

## Random bytes and the clock

A test can give the program its random bytes and clock readings, so output that depends on them
is exact and the proof covers every value they could take. The spec allows `getrandom` for
`random` and `clock_gettime` for `clock`.

- **`random`** holds the bytes the program's `getrandom` calls receive, in order, as hex digits.
  Bytes asked for past its end are zeros, so a program may draw in batches of any size and only
  the bytes it uses decide its output. In a generated block, `{hex(x, n)}` writes the whole
  number `x` as `n` bytes (1 to 8), least significant first: `hex(258, 2)` is `0201`, and eight
  bytes take any 64-bit value, negatives as two's complement.
- **A variable that only fills random bytes draws every value they can hold,** as the kernel's
  could: `int(0, 255)` for one byte, `int(0, 65535)` for two, and
  `int(-9223372036854775808, 9223372036854775807)` for eight. Purpose needn't state that range.
- **`clock`** holds what successive `clock_gettime` calls read, in seconds since 1970, with up to
  9 decimal places; the last reading repeats. A variable that only sets the clock draws seconds
  from 0 up, with `int` or `decimal`, and Purpose needn't state its range either. A block that
  supplies the clock knows the time exactly, so it never uses `now`.
- **An example and its generated blocks supply alike:** random bytes in all or none, and clock
  readings in all or none. The example's bytes and readings count as inputs, so its block reads
  its variables back from them when checking the formulas.

## The network

A spec that says `- Network: connects to TCP port N` or `- Network: listens on TCP port N` gets
tests with their own server or client, on the loopback, on a free port of their own: never the
spec's port, which the kernel allows only for `run`.

- **A program that connects** gets `server`: the test's server takes each connection, reads
  exactly `receives` (failing at the first byte that differs), sends `replies`, closes its side,
  and expects nothing more. The program learns the port from its arguments or input: write
  `{port}` there, in an example as in a generated block, and lengua writes in the test server's
  port. In tests, the kernel lets the program connect to that port alone.
- **A program that serves** gets its listening socket from lengua as file descriptor 3, in every
  test. `client` connects once per entry: it sends `sends`, closes its side, and reads everything
  the program answers until it closes the connection, which must match `receives` (rules
  allowed). With `serves = true`, the program must then be waiting for the next connection; lengua
  stops it, and the test has no `exit_code`.
- **Pieces:** on every other generated run, the test's server and client send in three pieces
  with short pauses, as a network may deliver them, so a program must read until it has what it
  needs.
- The spec's own port numbers count as present in a network test's example, since the test
  talks over its own port.

## Rules: output the program chooses

When the program chooses part of its output, such as a die's roll or the current time, a test
can't know the exact text. A rule stands for that part: `{any(lo, hi)}` is any whole number from
`lo` to `hi`, written plainly (an optional `-`, then digits with no leading zero).

- **In an example,** the expected output is a table: `stdout = { matches = "rolled {any(1, 6)}\n" }`.
  Its placeholders are `any(…)` only, with bounds that are whole numbers or `now` plus or minus a
  whole number; `{{` and `}}` stand for braces.
- **In a generated block,** `any(lo, hi)` is a function, and its bounds may be formulas:
  `stdout = "rolled {any(1, sides)}\n"`.
- **`now`** is when the run starts, in whole Unix seconds. No test knows that second exactly, so
  `now` only bounds a rule: `{any(now - 2, now + 2)}`. A block that declares its own variable
  named `now` draws it like any other.
- **Only expected output holds rules,** in `stdout`, `stderr` and `files_after`: a test's inputs
  are exact. Text functions such as `len` and `upper` can't look inside a rule.
- **Proofs cover rules:** `prove` shows the printed number is in range for every input and for
  every value the clock or `getrandom` could give, unless the block uses `now`.

## Files

Each run happens in a fresh, empty working folder. `files` puts files there before the program
starts, and after it ends the folder must hold exactly:
- every file in `files_after`, with exactly those contents;
- every file in `files` that `files_after` doesn't name, unchanged;
- nothing else.

So a program that writes a stray file, changes its input or leaves out its output fails. Name files
relative to the working folder (`notes.txt`, `in/data.csv`), never with `/` at the start or `..`.
A test names at most 16 files of at most 8 MiB each. The program can't reach files outside its
working folder at all: the kernel stops it.

## Generators

| Generator | Draws | Boundary values tried first |
|---|---|---|
| `int(lo, hi)` | Whole numbers from `lo` to `hi` inclusive, within 64 bits | `lo`, `hi`, 0, 1 and -1, when in range |
| `decimal(lo, hi)` | Decimals from `lo` to `hi`, with the places the bounds are written with: `decimal(0.00, 999.99)` prints 12.30, and reads 12.3, 4 and 12.30 alike | `lo`, `hi`, 0 and the smallest steps either side of it, when in range |
| `choice(a, b, …)` | One of the values: all whole numbers, all decimals or all strings | Every value (the first and last when there are more than 16) |
| `string(min, max, "chars")` | `min` to `max` characters (at most 1048576) drawn from `chars` | The shortest string of the first character, the longest of the last |

Linux limits each argument to 131072 bytes, so `check_tests` refuses a run whose argument is
longer; long text belongs on standard input.

Arguments must be literal values. The limits of `int` and `decimal` and the lengths of `string`
must be numbers that the `# Purpose` section of `spec.md` writes in backticks, so every range
comes from the approved spec, and a `decimal` uses the places Purpose writes (`0.00` means 2). A
`string` may start at length 0 without Purpose saying so.
Numbers from behaviors don't count: an example's own inputs aren't a range.

For input that isn't valid, list hand-picked values with `choice`: the example's own value plus
values that go wrong late, such as `choice("abc", "12x", "-", "1.5", "--1", "")` for a number.

## Templates

Text with `{expression}` placeholders. `{{` and `}}` stand for literal braces. Whole numbers print in
decimal, `decimal(…)` values with exactly their places, and strings as they are. A computed
decimal prints only through `fixed(x, places)`, and true and false can't be printed, so use `if`
to choose text.

## Expressions

- **Whole numbers** are checked 128-bit: overflow is an error, never a wrapped value.
- **Decimals** are written `12.50`, with 1 to 15 places. Arithmetic on decimals (`+ - * /`) is
  done in doubles, exactly as `addsd`, `subsd`, `mulsd` and `divsd` compute, so `0.1 + 0.2` is
  0.30000000000000004, and the order of operations is part of the formula: `a + b + c` is
  `(a + b) + c`. A result must be a finite number.
- **Singles** (32-bit floating point) come from `single(x)`. Arithmetic with a single is done in
  singles, as `addss`, `subss`, `mulss` and `divss` compute, and a decimal written next to one
  becomes the nearest single, as a `.float` constant does: `single(x) * 2.5` multiplies by
  `.float 2.5`. Singles hold about 7 significant digits; use them only when the spec's precision
  allows it.
- **Singles and doubles don't mix:** convert one side with `single(x)` or `double(x)`.
- **`fma(a, b, c)`** is `a × b + c` rounded once, as `vfmadd231sd` (or `vfmadd231ss` for singles)
  computes. `a * b + c` rounds twice, so the formula says which of the two the program does.
- **Whole numbers and decimals don't mix** in arithmetic: write `2.0`, not `2`, or `double(n)`.
  Comparisons may mix them (`x > 0`), and `12.5 == 12.50`.
- **Strings** are written `"text"`, with the escapes `\"`, `\\`, `\n` and `\t`; `+` joins two
  strings: `"rolled " + upper(s)`.
- **True and false** come from comparisons, or from `true` and `false`.
- **Operators**, from lowest precedence to highest: `||`, then `&&`, then the comparisons
  `== != < <= > >=` (one per comparison; strings compare only with `==` and `!=`), then `+ -`,
  then `* / %`, then unary `-` and `!`. Parentheses group.
- **Division** truncates toward zero and `%` keeps the sign of the dividend, like the CPU's
  `idiv`: `-7 / 2` is `-3` and `-7 % 2` is `-1`. Dividing by zero is an error.
- **Variable names** use lowercase letters, digits and `_`, start with a letter or `_`, and aren't
  function or generator names. `now` is built in unless a block declares it.

| Function | Result |
|---|---|
| `len(s)` | Length of `s` in bytes |
| `abs(n)` | Absolute value |
| `min(a, b, …)`, `max(a, b, …)` | Smallest or largest number |
| `if(condition, then, else)` | `then` or `else`; only the chosen one is evaluated, so `if(b == 0, 0, a / b)` is safe |
| `upper(s)`, `lower(s)` | ASCII letters in upper or lower case |
| `reverse(s)` | The characters in reverse order |
| `tr(s, from, to)` | Each character of `s` listed in `from` becomes the one at the same place in `to`, as Unix `tr` does; `a-z` is a range. ROT13 is `tr(s, "A-Za-z", "N-ZA-Mn-za-m")` |
| `count(s, part)` | How many times `part` appears in `s`, without overlaps: `count(line, "e")` |
| `words(s)` | How many words `s` has: runs of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage returns, as `wc -w` counts them |
| `fixed(x, places)` | `x` printed with exactly `places` decimals (0 to 15): x·10^places rounded to the nearest whole number, ties to even, like `mulsd` then `cvtsd2si`. `fixed(2.5, 0)` is "2", `fixed(-0.004, 2)` is "0.00". A single widens exactly first (`cvtss2sd`), then prints as a double does |
| `double(x)` | `x` as a double: a whole number exactly (up to 2^53), a single exactly (like `cvtss2sd`) |
| `single(x)` | `x` as a single: a whole number exactly (up to 2^24), a double to the nearest single (like `cvtsd2ss`), an exact decimal to the nearest single (up to 7 digits and 10 places, like `cvtsi2ss` then `divss`) |
| `fma(a, b, c)` | `a × b + c` rounded once, like `vfmadd231sd`; in singles when an argument is a single |
| `round(x)`, `trunc(x)`, `floor(x)`, `ceil(x)` | A whole number: nearest with ties to even (like `cvtsd2si`), toward zero (like `cvttsd2si`), down, up |
| `sqrt(x)` | The square root, as `sqrtsd` computes it, or `sqrtss` for a single |
| `scaled(x, places)` | An exact decimal as a whole number of its smallest units: `scaled(12.5, 2)` is 1250 (cents) |
| `places(n, places)` | A whole number of units printed as a decimal: `places(1250, 2)` is "12.50", `places(-5, 2)` is "-0.05" |
| `sha256(s)` | The SHA-256 of `s`'s bytes, in lowercase hex |
| `crc32(s)` | The CRC-32 of `s`'s bytes, as zlib, gzip and PNG compute it |
| `crc32c(s)` | The CRC-32C (Castagnoli) of `s`'s bytes, as the `crc32` instruction computes it |
| `any(lo, hi)` | In expected output only: any whole number from `lo` to `hi`, written plainly. See [Rules](#rules-output-the-program-chooses) |
| `hex(x, n)` | In `random` only: `x` as `n` bytes, least significant first, in hex. See [Random bytes and the clock](#random-bytes-and-the-clock) |

Exact decimals such as money stay exact in whole units: `{places((scaled(a, 2) + scaled(b, 2)), 2)}`
adds two prices in cents, and any rounding rule the spec states is whole-number arithmetic with
`if`. Use `fixed` for measurements computed in doubles or singles.

## Writing TOML strings

- Basic strings `"…"` use escapes: `\n` newline, `\t` tab, `\"` quote, `\\` backslash.
- Literal strings `'…'` have no escapes, which suits generators that hold quotes:
  `vars = { s = 'string(1, 10, "abc")' }`.
- Multi-line strings `"""…"""` keep their newlines, except one right after the opening `"""`.

## What check_tests checks

1. The file is valid TOML and uses only the fields above. Its text never holds the characters
   U+FDD0, U+FDD1 and U+FDD2, which mark rules inside lengua.
2. `[meta].spec` is the approved spec's label.
3. There is exactly one example per behavior, and no unknown IDs.
4. Each example has its behavior's exit code and contains every backtick value of the behavior.
5. Each behavior whose example has inputs has a generated block.
6. Generators are valid and their limits are numbers from the spec's Purpose.
7. `args`, `stdin`, `files` and `clock` hold bare variables, and `random` hex digits and
   `hex(x, n)`; every variable is declared and reaches the program. `random` and `clock` need the
   spec to allow `getrandom` and `clock_gettime`.
8. A dry run computes every run's expected results: no overflow, no division by zero, and exit
   codes from 0 to 255 that are never 134.
9. Output with placeholders differs between runs, so a program that prints a constant can't pass.
   Output with a rule may stay the same, since the rule stands for what the program chooses.
10. Blocks of one behavior share their templates.
11. Given each example's inputs, the formulas produce the example's results. Where the example
    has a rule, the block's output must have the same rule, or exact text the rule allows.

When everything passes, `lengua.lock` records the tests and the server keeps a copy. Until
`check_code` first runs them, a fixed `tests.toml` can be checked again; from then on the tests
stay frozen until the spec changes, and the copy can restore them.

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

### Random bytes from the test

```markdown
---
type: lengua/program-spec
title: dice
description: Rolls a fair six-sided die and prints the roll.
---
# Purpose
Rolls a fair six-sided die for a board game and prints the roll, a whole number from `1` to `6`.
It reads random bytes from the kernel and rolls with the first byte below `252`: the roll is that
byte's remainder after dividing by 6, plus 1. Bytes from `252` to `255` are set aside, so every
face is equally likely. If the kernel gives no random bytes, or the first 64 are all set aside
(which a fair kernel practically never does), the program exits with code 3. If writing the
output fails, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the kernel's random bytes `fc` and `2a`
- When the program runs
- Then it prints `rolled 1` followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, getrandom, exit
```

```toml
[meta]
spec = "sha256:<from status>"

[[example]]
id = "B1"
random = "fc2a"
stdout = "rolled 1\n"
exit_code = 0

[[generated]]
id = "B1"
runs = 200
vars = { first = "int(0, 255)", second = "int(0, 255)" }
random = "{hex(first, 1)}{hex(second, 1)}"
stdout = "rolled {if(first < 252, first % 6, if(second < 252, second % 6, 0)) + 1}\n"
exit_code = 0
```

When both bytes are set aside, the third byte is past the end of `random`, so it's 0, and the
roll is 1.

### Output the program chooses

```markdown
---
type: lengua/program-spec
title: roll
description: Rolls a six-sided die for a player and prints the roll.
---
# Purpose
Rolls a fair six-sided die for a player numbered from `1` to `9`, given as an argument. If
writing the output fails, the program exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the argument `3`
- When the program runs
- Then it prints `player 3 rolled ` and a number from `1` to `6`, followed by a newline
- And it exits with code `0`

# Constraints
- Allowed system calls: write, getrandom, exit
```

```toml
[meta]
spec = "sha256:<from status>"

[[example]]
id = "B1"
args = ["3"]
stdout = { matches = "player 3 rolled {any(1, 6)}\n" }
exit_code = 0

[[generated]]
id = "B1"
vars = { player = "int(1, 9)" }
args = ["{player}"]
stdout = "player {player} rolled {any(1, 6)}\n"
exit_code = 0
```
