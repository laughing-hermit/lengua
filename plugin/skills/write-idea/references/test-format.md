# The tests in idea.md

Every test lives in `idea.md`, in plain markdown. Each behavior's Given/When/Then steps are its
example: lengua reads them as a test with fixed values. Its generated tests sit under a `###`
heading after them: the same steps, with `{placeholders}` where values vary, then a table of
where each variable comes from, one row per variant, as a Gherkin Scenario Outline has its
Examples. They draw fresh inputs on every run and compute the expected results. lengua.lock
records the idea the tests are part of, so nothing carries a label.

## The steps

lengua reads each step as one of these phrasings. A step it can't read is a problem in the idea,
reported with the phrasing to write instead. The phrasings are in ASD-STE100 Simplified Technical
English, as the rest of the idea is ([STE in idea files](ste.md)): a step in one of lengua's old
phrasings, such as ``standard input holding `…` ``, is refused with the STE phrasing that
replaces it. Values go in backticks, with `\n`, `\r`, `\t` and
`\\` for the characters they name, so a protocol's line breaks are written as they're sent:
`` `GET / HTTP/1.0\r\n\r\n` ``. A value that holds a backtick goes in double backticks:
``` ``a`b`` ```.

**Given:** what the program gets.

| Step | Gives the program |
|---|---|
| ``the arguments `2` and `3` ``, ``the argument `x` `` | Its command-line arguments, in order |
| `no arguments` | None |
| ``standard input that contains TEXT`` | Everything it can read; then input ends |
| `no standard input`, `empty standard input` | Input that ends at once |
| ``a file `notes.txt` that contains TEXT``, ``an empty file `notes.txt` `` | A file in its working folder |
| ``no file `notes.txt` `` | Nothing: the folder starts empty, and the step says so |
| ``an empty folder `archive` `` | A folder in its working folder, for an idea that uses `files` |
| ``the environment variable `NAME` with the value TEXT`` | That variable, for an idea that uses `the environment`; it gets no others |
| ``the clock at `1735689600` `` | What the program reads from the clock, in seconds since 1970; ``then `1735689601` `` adds a later reading |
| ``the random bytes `fc` and `2a` `` | The random bytes the program asks the computer for, one byte of hex a value |
| ``a server on port `8080` that receives TEXT and replies TEXT`` | The test's own server on the program's connection to that port, one a connection; ``a server at `example.com` on port `443` …`` when two places share a port, and ``a server that receives …`` when the idea names one |
| ``no server on port `8080` `` | That connection handed over already ended: its first read gives the end, and its first write fails |
| ``a server at `www.example.org` on port `80` that receives TEXT and replies TEXT`` | For a program that opens its connections itself: the test's own server for one connection the program opens to that computer and port; a second such step is its second connection there |
| ``no server at `www.example.org` on port `80` `` | Nothing: a computer no step gives a server takes no connection, and the step says so |
| ``a computer at `192.0.2.53` on UDP port `53` that receives TEXT and replies TEXT`` | The test's own computer for one datagram the program sends there, and the datagram it answers with |
| ``the addresses `192.0.2.7` and `2001:db8::1` for the name `example.org` `` | What a look at that name's addresses gives, in every test of the idea |
| ``a client that sends TEXT`` | The test's own client, one step a connection, for a program that serves |
| `standard output is closed` | Standard output as a pipe nobody reads, as after `prog \| head -1` has read its line: every write to it fails with `EPIPE`, never a signal. The behavior then says only its exit code and what it prints to standard error |

**When:** `the program runs`, once.

**Then:** what the program must do.

| Step | Expects |
|---|---|
| ``it prints TEXT`` | Exactly that on standard output |
| ``it prints TEXT to standard error`` | Exactly that on standard error |
| `it prints nothing` | Nothing on either; also what a behavior means when it says neither |
| ``it prints text that matches `rolled {any(1, 6)}` `` | Output the program chooses: a [rule](#rules-output-the-program-chooses), here `rolled ` and a number from 1 to 6 |
| ``the file `loud.txt` contains TEXT`` | A file the program leaves, with exactly that in it, or ``text that matches `…` `` |
| ``there is no file `notes.txt` ``, ``there is no folder `tmp` `` | A file or folder the program deleted, moved away, or never made |
| ``there is a folder `archive` `` | A folder the program made or kept |
| ``the client receives TEXT`` | What the program answers that client, in the order of the clients' steps; or ``text that matches `…` `` |
| ``it exits with code `0` `` | Its exit code: 0 to 255, never 134 |
| `it does not stop` | A server that still waits for its next connection, with no exit code |

**TEXT** is a value in backticks, where `\n`, `\r`, `\t`, `\e` (the escape that starts a
terminal's sequences) and `\\` stand for those characters. `and a newline` adds a line break,
`` `N` times`` after a value repeats it, and `then` joins parts:
``- Given standard input that contains `a` `4096` times then `x` `` is 4096 `a`s and an `x`, up
to 8 MiB a value. Several printing steps print one after another.

A program that connects needs no port in its arguments or input: lengua makes each connection
its idea names before the program starts, and the test's server for it answers there.

## Generated tests

Under its steps, a behavior that gives the program something has its generated tests: a `###`
heading, such as `### For any two numbers`, then the same steps with `{placeholders}`, then a
table. Prose under the heading is the idea's own.

```markdown
### For any two numbers
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |
```

- **The steps** use the example's phrasings. In what the program is given (its arguments,
  standard input, files, clock readings, random bytes, the test server's replies and the test
  client's messages), a placeholder holds a bare variable, as in `{a}` or `--n={a}`; what it
  prints, the files it leaves, what the server receives and what the client receives, and its
  exit code, may hold any formula: `{a + b}`, ``it exits with code `{if(a < 0, 1, 0)}` ``. `{{`
  and `}}` stand for braces. Escapes such as `\n` are read in the text around placeholders, while
  a formula's own strings keep theirs: ``it prints `{count(text, "\n")}` and a newline``. A value
  with `` `N` times`` after it is fixed text here too, for input past a limit that no generator
  reaches: ``standard input that contains `\n` `4194304` times then `{line}` ``. A placeholder is
  never repeated: its value comes from the table.
- **The table** has a column for each variable, holding its generator, and an optional `runs`
  column: 1 to 1000, or empty for 100. Each row is one generated test, so several rows cover
  inputs one range can't, such as divisors on both sides of zero; all rows share the steps.
  Write each cell in backticks, with `\|` for a `|` inside it.

Runs try boundary values first: every combination when there are 64 or fewer, otherwise each
boundary value once. Random values fill the remaining runs, and a failing run reports its inputs
and the seed that drew them.

Small ranges run in full instead: when a row's generators are all ranges (`int`, `string`, no
`choice`) with at most 5000 combinations, every combination runs and `runs` is ignored, as long as
the whole idea stays within 20000 runs. `check` then reports that the row tried every value in
its range: for those inputs the program is checked, not sampled.

## Random bytes and the clock

A test can give the program its random bytes and clock readings, so output that depends on them
is exact and the proof covers every value they could take. The idea's `- Uses:` line names
`random bytes` or `the clock`. `the clock` also lets the program wait: in a test, each wait ends
at once, so a test never waits for it, and a real run waits as long as it asks.

In a generated test, ``the random bytes `{hex(first, 1)}` and `{hex(second, 1)}` `` writes each
variable as bytes, and ``the clock at `{t}` `` reads a variable.

- **Random bytes** are given in order, as hex digits. Bytes the program asks for past the last
  are zeros, so a program may draw in batches of any size and only the bytes it uses decide its
  output. `{hex(x, n)}` writes the whole number `x` as `n` bytes (1 to 8), least significant
  first: `hex(258, 2)` is `0201`, and eight bytes take any 64-bit value, negatives as two's
  complement.
- **A variable that only fills random bytes draws every value they can hold,** as the computer's
  could: `int(0, 255)` for one byte, `int(0, 65535)` for two, and
  `int(-9223372036854775808, 9223372036854775807)` for eight. Purpose needn't state that range.
- **Clock readings** are what the program reads, one reading after another, in seconds since
  1970 with up to 9 decimal places; the last reading repeats. A variable that only sets the
  clock draws seconds from 0 up, with `int` or `decimal`, and Purpose needn't state its range
  either. A generated test that supplies the clock knows the time exactly, so it never uses `now`.
- **The example and its generated tests supply alike:** random bytes in both or neither, and
  clock readings in both or neither. The example's bytes and readings count as inputs, so its
  variables are read back from them when its formulas are checked.

## The terminal

An idea that uses `the terminal` gets its keys as standard input in tests, a key a byte or an
escape sequence of several, such as `\e[A` for up, and its tests write what it prints with `\e`
for the escape that starts each sequence: ``it prints `\e[2J\e[H` then …``. A test's
standard input is a pipe, so the program can't set the terminal's mode there and goes on as from
a pipe; every wait for a key finds one, or the end of the input.

## The network

An idea with a `- Network:` line gets tests with their own servers or client: never the real
places, which only `run` reaches. That holds for the connections lengua makes before the
program starts and for everything the program opens itself.

- **A program that connects** gets each connection its idea names from lengua before it starts,
  in the idea's order, in tests as in `run`. A test's server step stands
  in for one of them: it reads exactly what the program sends there (failing at the first byte
  that differs), sends its reply, closes its side, and expects nothing more. A connection a test
  gives no server comes ended, as one lengua couldn't make does in a run: its first read gives
  the end and its first write fails. One server a connection. A connection `over TLS` is tested
  the same way, in plain bytes: lengua does the TLS only in a run, where the program can't tell
  the difference. So is one to the computer an argument names: the test's server stands in for
  it whatever the argument says, and only `run` reads the argument and connects there.
- **A program that opens its connections itself** (`- Network: opens TCP connections to …`)
  reaches only the test's own servers, each named by its computer and its port: ``a server at
  `www.example.org` on port `80` that receives … and replies …``. The server takes one
  connection, reads exactly what the program sends there, sends its reply and closes its side;
  a second step with the same computer and port is the second connection the program opens
  there, and after the last, nothing listens. A computer the idea grants and no step gives a
  server takes no connection, as a computer that is down does; a computer the idea doesn't
  grant is refused by lengua before anything is reached, whatever the test has. Name the
  computers in the steps as they are, not as placeholders: a generated test draws what is sent
  and received, not where. `over TLS` is tested in plain bytes here too.
- **Datagrams:** for `- Network: sends UDP datagrams to …`, ``a computer at `192.0.2.53` on UDP
  port `53` that receives … and replies …`` reads one datagram, which must be exactly that, and
  answers with one; with no such step, what the program sends is lost and nothing answers. For
  `- Network: receives UDP datagrams on port N`, the test's client steps are datagrams: each
  ``a client that sends …`` is one datagram to the program's port, and its ``the client
  receives …`` the one datagram the program answers with.
- **Names:** for `- Network: looks up the addresses of …`, ``the addresses `192.0.2.7` and
  `2001:db8::1` for the name `www.example.org` `` gives the name those addresses, in every test
  of the idea; any other name has none. The program gets IPv4 addresses first, each family in
  order. lengua holds a test's addresses to its rules as it holds real ones, so a name given
  `127.0.0.1` is refused, which a test may show.
- **A program that serves** gets its listening socket from lengua, in every test. The test's
  client connects once a connection and behaves as curl and browsers do: it
  sends its text and keeps its side of the connection open until the program starts answering
  (or closes it at once when it expects no answer), then reads everything the program answers
  until it closes the connection. So a server must answer once the request is complete, by its
  newline, blank line or length, never by waiting for the client to close. A behavior that ends
  with ``it does not stop`` leaves the program to wait for the next connection; lengua stops it.
- **Clients at once:** a server whose `- Network:` line ends with `, up to N clients at once`
  shows it in a behavior with two or more ``a client that sends …`` steps and ``- And the
  clients connect at once``, at most N of them. Its test opens every connection; all but the
  newest send all but their last byte and hold it, and the newest sends everything and must be
  answered while the others hold; then each, from the newest, sends its last byte and must be
  answered. A server that serves one client at a time is caught waiting on the first. Each
  ``the client receives …`` belongs to its client, in order.
- **Pieces:** on every other generated run, standard input and the test's server and client
  arrive in three pieces with short pauses, as a pipe or a network may deliver them, so a program
  must read until it has what it needs. A program that reads with lengua's `fill` is proven for every way a message
  can be cut, not only these.

## Rules: output the program chooses

When the program chooses part of its output, such as a die's roll or the current time, a test
can't know the exact text. A rule stands for that part: `{any(lo, hi)}` is any whole number from
`lo` to `hi`, written plainly (an optional `-`, then digits with no leading zero).

- **In the steps,** ``it prints text that matches `rolled {any(1, 6)}` and a newline``. Its
  placeholders are `any(…)` only, with bounds that are whole numbers or `now` plus or minus a
  whole number; `{{` and `}}` stand for braces.
- **In a generated test,** `any(lo, hi)` is a function, and its bounds may be formulas:
  ``it prints `rolled {any(1, sides)}` and a newline``.
- **`now`** is when the run starts, in whole Unix seconds. No test knows that second exactly, so
  `now` only bounds a rule: `{any(now - 2, now + 2)}`. A table with a column named `now` draws
  it like any other variable.
- **Only expected output holds rules,** in what the program prints, the files it leaves and what
  a client receives: a test's inputs are exact. Text functions such as `len` and `upper` can't
  look inside a rule.
- **Proofs cover rules:** the proof shows the printed number is in range for every input and for
  every value the clock or the random bytes could give, unless the test uses `now`.

## Files

Each run happens in a fresh, empty working folder. The test puts its files and empty folders
there before the program starts, and after it ends the folder must hold exactly:
- every file the test expects the program to leave, with exactly those contents;
- every file the test gave that it doesn't expect changed or gone, unchanged;
- every folder the test gave, unless it expects it gone, and every folder it expects;
- nothing else: no file, and no folder, the test doesn't name.

So a program that writes a stray file, changes or deletes its input, makes a stray folder or
leaves out its output fails. Name files
relative to the working folder (`notes.txt`, `in/data.csv`), never with `/` at the start or `..`.
A test names at most 16 files of at most 8 MiB each. The program can't reach files outside its
working folder at all: lengua stops it.

## Generators

| Generator | Draws | Boundary values tried first |
|---|---|---|
| `int(lo, hi)` | Whole numbers from `lo` to `hi` inclusive, within 64 bits | `lo`, `hi`, 0, 1 and -1, when in range |
| `decimal(lo, hi)` | Decimals from `lo` to `hi`, with the places the bounds are written with: `decimal(0.00, 999.99)` prints 12.30, and reads 12.3, 4 and 12.30 alike | `lo`, `hi`, 0 and the smallest steps either side of it, when in range |
| `choice(a, b, …)` | One of the values: all whole numbers, all decimals or all strings | Every value (the first and last when there are more than 16) |
| `string(min, max, "chars")` | `min` to `max` characters (at most 1048576) drawn from `chars` | The shortest string of the first character, the longest of the last |

The arguments of a run take at most 32000 characters of a command line together (UTF-16 units,
each argument after a space and quoted as Windows quotes it), the same limit on every computer, so
`check` refuses a run whose arguments take more; long text belongs on standard input.

Arguments must be literal values. The limits of `int` and `decimal` and the lengths of `string`
must be numbers that the `# Purpose` section writes in backticks, so every range comes from the
idea's own words, and a `decimal` uses the places Purpose writes (`0.00` means 2). A `string` may
start at length 0 without Purpose saying so. Numbers from behaviors don't count: an example's own
inputs aren't a range.

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
  done in doubles, exactly as WebAssembly's `f64.add`, `f64.sub`, `f64.mul` and `f64.div`
  compute, so `0.1 + 0.2` is 0.30000000000000004, and the order of operations is part of the
  formula: `a + b + c` is `(a + b) + c`. A result must be a finite number.
- **Singles** (32-bit floating point) come from `single(x)`. Arithmetic with a single is done in
  singles, as `f32.add`, `f32.sub`, `f32.mul` and `f32.div` compute, and a decimal written next
  to one becomes the nearest single, as an `f32.const` does: `single(x) * 2.5` multiplies by
  `(f32.const 2.5)`. Singles hold about 7 significant digits; use them only when the idea's
  precision allows it.
- **Singles and doubles don't mix:** convert one side with `single(x)` or `double(x)`.
- **`fma(a, b, c)`** is `a × b + c` rounded once. WebAssembly has no fused multiply-add, so a
  program can't compute it in one instruction: write `a * b + c`, which rounds after each step,
  as `f64.mul` then `f64.add` do.
- **Whole numbers and decimals don't mix** in arithmetic: write `2.0`, not `2`, or `double(n)`.
  Comparisons may mix them (`x > 0`), and `12.5 == 12.50`.
- **Strings** are written `"text"`, with the escapes `\"`, `\\`, `\n` and `\t`; `+` joins two
  strings: `"rolled " + upper(s)`.
- **True and false** come from comparisons, or from `true` and `false`.
- **Operators**, from lowest precedence to highest: `||`, then `&&`, then the comparisons
  `== != < <= > >=` (one per comparison; strings compare only with `==` and `!=`), then `+ -`,
  then `* / %`, then unary `-` and `!`. Parentheses group.
- **Division** truncates toward zero and `%` keeps the sign of the dividend, as in C and Rust:
  `-7 / 2` is `-3` and `-7 % 2` is `-1`. Dividing by zero is an error.
- **Variable names** use lowercase letters, digits and `_`, start with a letter or `_`, and aren't
  function or generator names. `now` is built in unless a table has a column of that name.

| Function | Result |
|---|---|
| `len(s)` | Length of `s` in bytes |
| `abs(n)` | Absolute value |
| `min(a, b, …)`, `max(a, b, …)` | Smallest or largest number |
| `if(condition, then, else)` | `then` or `else`; only the chosen one is evaluated, so `if(b == 0, 0, a / b)` is safe |
| `upper(s)`, `lower(s)` | ASCII letters in upper or lower case |
| `reverse(s)` | The characters in reverse order |
| `sort(s)` | `s`'s lines in byte order, each ending with a newline, as `LC_ALL=C sort` prints them; a last line without a newline is a line too |
| `uniq(s)` | `s`'s lines with each run of identical ones kept once, each ending with a newline, as `uniq` prints them: `uniq(sort(s))` is each distinct line once |
| `tally(s)` | Each distinct word of `s`, as `words` splits them, with how many times it appears, one per line as `count word`: most frequent first, ties in byte order |
| `tr(s, from, to)` | Each character of `s` listed in `from` becomes the one at the same place in `to`, as Unix `tr` does; `a-z` is a range. ROT13 is `tr(s, "A-Za-z", "N-ZA-Mn-za-m")` |
| `count(s, part)` | How many times `part` appears in `s`, without overlaps: `count(line, "e")` |
| `words(s)` | How many words `s` has: runs of bytes other than spaces, tabs, newlines, vertical tabs, form feeds and carriage returns, as `wc -w` counts them |
| `fixed(x, places)` | `x` printed with exactly `places` decimals (0 to 15): x·10^places rounded to the nearest whole number, ties to even, like `f64.mul`, then `f64.nearest` then `i64.trunc_sat_f64_s`. `fixed(2.5, 0)` is "2", `fixed(-0.004, 2)` is "0.00". A single widens exactly first (`f64.promote_f32`), then prints as a double does |
| `double(x)` | `x` as a double: a whole number exactly (up to 2^53), a single exactly (like `f64.promote_f32`) |
| `single(x)` | `x` as a single: a whole number exactly (up to 2^24), a double to the nearest single (like `f32.demote_f64`), an exact decimal to the nearest single (up to 7 digits and 10 places, like `f32.convert_i64_s` then `f32.div`) |
| `fma(a, b, c)` | `a × b + c` rounded once, in singles when an argument is a single. WebAssembly has no fused multiply-add, so write `a * b + c` for what a program computes |
| `round(x)`, `trunc(x)`, `floor(x)`, `ceil(x)` | A whole number: nearest with ties to even (like `f64.nearest` then `i64.trunc_sat_f64_s`), toward zero (like `i64.trunc_sat_f64_s`), down (like `f64.floor`), up (like `f64.ceil`) |
| `sqrt(x)` | The square root, as `f64.sqrt` computes it, or `f32.sqrt` for a single |
| `scaled(x, places)` | An exact decimal as a whole number of its smallest units: `scaled(12.5, 2)` is 1250 (cents) |
| `places(n, places)` | A whole number of units printed as a decimal: `places(1250, 2)` is "12.50", `places(-5, 2)` is "-0.05" |
| `sha256(s)` | The SHA-256 of `s`'s bytes, in lowercase hex |
| `aes128(key, data)` | The blocks `data` holds, as hex digits in a positive multiple of 32, encrypted with AES-128 under `key`'s 32 hex digits, each block on its own (ECB), in lowercase hex |
| `crc32(s)` | The CRC-32 of `s`'s bytes, as zlib, gzip and PNG compute it |
| `crc32c(s)` | The CRC-32C (Castagnoli) of `s`'s bytes, as iSCSI and ext4 compute it |
| `any(lo, hi)` | In expected output only: any whole number from `lo` to `hi`, written plainly. See [Rules](#rules-output-the-program-chooses) |
| `hex(x, n)` | In the random bytes only: `x` as `n` bytes, least significant first, in hex. See [Random bytes and the clock](#random-bytes-and-the-clock) |

Exact decimals such as money stay exact in whole units: `{places((scaled(a, 2) + scaled(b, 2)), 2)}`
adds two prices in cents, and any rounding rule the idea states is whole-number arithmetic with
`if`. Use `fixed` for measurements computed in doubles or singles.

## What check checks

1. Every step is one of the phrasings above, in its STE form, and no value or cell holds the
   characters U+FDD0, U+FDD1 and U+FDD2, which mark rules inside lengua.
2. Each behavior's generated tests sit under one `###` heading after its steps, with Given, When
   and Then steps, and a table below them: a header, a separator such as `|---|`, and rows with a
   cell for each column. Tests written in TOML are refused, with this form to write instead.
3. Each behavior whose steps give the program something has generated tests.
4. Generators are valid and their limits are numbers from the idea's Purpose.
5. What a test gives the program holds bare variables, and its random bytes hex digits and
   `hex(x, n)`; every variable has a column and reaches the program. Random bytes and clock
   readings need the idea's `- Uses:` line to name them.
6. A dry run computes every run's expected results: no overflow, no division by zero, and exit
   codes from 0 to 255 that are never 134.
7. Output with placeholders differs between runs, so a program that prints a constant can't pass.
   Output with a rule may stay the same, since the rule stands for what the program chooses.
8. Given the inputs of each behavior's example, its formulas produce the example's results. Where
   the example has a rule, the generated output must have the same rule, or exact text the rule
   allows.

A problem in a step is reported on the step's line, and one in a generator on its row's. When
everything passes, `lengua.lock` records the idea, its tests included, and the server keeps a
copy. A wrong test can be fixed at any time and checked again. After code has run, the reply
shows what changed since the idea the code first ran against (`idea_changed`) and lengua's own
marks on the changes that test less (`tests_less`): fewer runs, a generator that draws fewer
values, exact output turned into a rule, or a removed row, variable or expected file.

## More examples

### Reading standard input

```markdown
---
type: Attested Computation
title: shout
description: Reads one line from standard input and prints it in capital letters.
runtime: webassembly
computation: main.wat
parameters:
  - { name: line, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
Reads a line of `1` to `80` lowercase letters and spaces from standard input, then prints it in
capital letters.

# Behaviors
## B1: Shouts a line
- Given standard input that contains `hello world` and a newline
- When the program runs
- Then it prints `HELLO WORLD` and a newline
- And it exits with code `0`

### For any line
- Given standard input that contains `{line}` and a newline
- When the program runs
- Then it prints `{upper(line)}` and a newline
- And it exits with code `0`

| line |
|---|
| `string(1, 80, "abcdefghijklmnopqrstuvwxyz ")` |

## B2: Rejects empty input
- Given no standard input
- When the program runs
- Then it prints `error: no input` and a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: standard input
```

### Two ranges and an error

```markdown
---
type: Attested Computation
title: divide
description: Divides one whole number by another and prints the quotient, rounded toward zero.
runtime: webassembly
computation: main.wat
parameters:
  - { name: a, type: integer, required: true, from: argument 1 }
  - { name: b, type: integer, required: true, from: argument 2 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
Divides a whole number between `-1000000` and `1000000` by a whole number between `-1000` and
`-1` or between `1` and `1000`. It prints the quotient, rounded toward zero.

# Behaviors
## B1: Divides two numbers
- Given the arguments `7` and `2`
- When the program runs
- Then it prints `3` and a newline
- And it exits with code `0`

### For any two numbers
Negative divisors, then positive ones: one range cannot hold both without 0.

- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a / b}` and a newline
- And it exits with code `0`

| a | b |
|---|---|
| `int(-1000000, 1000000)` | `int(-1000, -1)` |
| `int(-1000000, 1000000)` | `int(1, 1000)` |

## B2: Rejects division by zero
- Given the arguments `7` and `0`
- When the program runs
- Then it prints `error: division by zero` and a newline to standard error
- And it exits with code `2`

### For any number divided by zero
- Given the arguments `{a}` and `0`
- When the program runs
- Then it prints `error: division by zero` and a newline to standard error
- And it exits with code `2`

| a | runs |
|---|---|
| `int(-1000000, 1000000)` | 50 |

# Constraints
- Uses: nothing
```

### Random bytes from the test

```markdown
---
type: Attested Computation
title: dice
description: Rolls a fair six-sided die and prints the roll.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
Rolls a fair six-sided die for a board game and prints the roll, a whole number from `1` to `6`.
It reads random bytes from the computer and uses the first byte below `252`. The roll is the
remainder of that byte divided by 6, plus 1. Bytes from `252` to `255` are set aside, so each
face has the same chance.

If the computer gives no random bytes, the program exits with code 3. It also exits with code 3
when the first 64 bytes are all set aside, which a fair source almost never does. If the program
cannot write its output, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the random bytes `fc` and `2a`
- When the program runs
- Then it prints `rolled 1` and a newline
- And it exits with code `0`

### For any random bytes
- Given the random bytes `{hex(first, 1)}` and `{hex(second, 1)}`
- When the program runs
- Then it prints `rolled {if(first < 252, first % 6, if(second < 252, second % 6, 0)) + 1}` and a newline
- And it exits with code `0`

| first | second | runs |
|---|---|---|
| `int(0, 255)` | `int(0, 255)` | 200 |

# Constraints
- Uses: random bytes
```

When both bytes are set aside, the third byte is past the last one given, so it's 0, and the roll
is 1.

### Output the program chooses

```markdown
---
type: Attested Computation
title: roll
description: Rolls a six-sided die for a player and prints the roll.
runtime: webassembly
computation: main.wat
parameters:
  - { name: player, type: integer, required: true, from: argument 1 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
Rolls a fair six-sided die for a player with a number from `1` to `9`, given as an argument. If
the program cannot write its output, it exits with code 2.

# Behaviors
## B1: Rolls a die
- Given the argument `3`
- When the program runs
- Then it prints text that matches `player 3 rolled {any(1, 6)}` and a newline
- And it exits with code `0`

### For any player
- Given the argument `{player}`
- When the program runs
- Then it prints `player {player} rolled {any(1, 6)}` and a newline
- And it exits with code `0`

| player |
|---|
| `int(1, 9)` |

# Constraints
- Uses: random bytes
```
