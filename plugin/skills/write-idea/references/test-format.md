# The test format

Each behaviour of an idea is a test. Its examples run against the built program when `check`
builds it, and are compared exactly.

## A behaviour

```
## <name>

Given <the state before>
And <more of it>
When <the action>
Then <the result>
```

`Given`, `When` and `Then` are each there one time, in that order. `And` adds to the line
above it. A line can continue on the next lines. Every behaviour has one `### Example:` or
`### Cases:` block or more.

## An example of a program or a module

Each item is `- Key: value`. A run gets nothing that the example does not give.

| Key | Value |
|---|---|
| `Standard input` | a string |
| `Arguments` | strings, with spaces between them: `"-w" "file.txt"` |
| ``File `in/a.txt` `` | a string: a file that is there before the run |
| `Expect standard output` | a string, compared exactly; without it, the program must write nothing to standard output |
| `Expect standard error` | a string, compared exactly (not compared when absent) |
| `Expect end` | `exited`, `out of time`, `out of memory` or `trapped`: how the run ends; `exited` when absent |
| `Expect exit code` | a number from 0 to 255; every example that ends `exited` has one, and no other |
| ``Expect file `out/b.txt` `` | a string: the file's content after the run |

A path is relative to the program's folder, with `/` between names, and has no `..`.

## An example of a screen (`ideas/ui/`)

| Key | Value |
|---|---|
| ``State `field` `` | a string, an integer, `true` or `false`: the state before |
| `Click` | `` `element` `` |
| ``Type into `element` `` | a string |
| ``Expect state `field` `` | a string, an integer, `true` or `false`: the state after |
| `Expect text` | a string that the screen shows |
| `Expect label` | `` `label` ``: an element that the accessibility tree names so |

A screen example has one `Expect` item or more and one `Click` at most. The steps run as
State, Type into, Click, then the expectations. Each request the screen sends gets the answer
of the server's example with the same method, path, body and `authorization`, so the screens
and the server keep to the same examples.

## Strings

A string is in double quotes. The escapes are `\n`, `\t`, `\r`, `\0`, `\\`, `\"` and
`\u{41}`. For a long string, leave the value empty and put a fenced block under the item; each
line of the block, with a line end, is the string:

````
- Expect standard output:
  ```
  line one
  line two
  ```
````


## The network: a test server and a test client

A program that connects to a server (`- Connects to:`) or listens on a port (`- Listens on:`)
is tested with a local server or client that lengua starts for the run:

| Key | Value |
|---|---|
| ``Server `127.0.0.1:7801` `` | a string: a server at that address, which sends the string to each client and reads to the end |
| ``Client `7803` `` | a string: a client that connects to that port on `127.0.0.1`, sends the string, and reads the answer |
| `Expect answer` | a string: what the client got, compared exactly (with one `Client` item) |

The host refuses every address and port the idea does not grant, even when a test server is
there; an example can show it.

## An example of a library (an idea with `- Exports:`)

A library has no entry of its own, so each example is a call of its functions in Rust:

| Key | Value |
|---|---|
| `Call` | a Rust expression in backticks: `` `parse("[1]")` ``; a block `` `{ let mut v = vec![2, 1]; sort(&mut v); v }` `` too |
| `Expect value` | the value as Rust's `{:?}` writes it, in backticks or as a string |

lengua writes a harness program that calls each example with the library's exports in scope
(`use <library>::*`), builds it with the library, and compares what `{:?}` gives. An example
has no exit code. In a Cases block, write `{{` and `}}` for the braces of Rust.

## An example of a server (an idea with `- Serves:`)

| Key | Value |
|---|---|
| `Request` | the method and the path in backticks: `` `GET /items?a=1` `` |
| ``Request header `name` `` | a string |
| `Request body` | a string |
| `Expect status` | a number from 100 to 599; every request example has it |
| ``Expect header `name` `` | a string, compared exactly; the name in any case |
| `Expect body` | a string, compared exactly (not compared when absent) |
| `Model answers` | a string: the answer of the AI model to every call of the run; only with `- Asks:` |

A server example has no exit code. A module of a server is tested through requests too. A
2xx example under `/api/` needs its `- Route:` line. Tests never ask a real model: give the
answer with `Model answers`; without it the model answers 503.

## The examples of a module

A module of a program is tested through the program's own entry: its examples give the
program's input and expect the program's output, and they run against the one component.
## Cases: many runs from ranges

```
### Cases: any word

- Range `word`: text of 1 to 8 characters from "abc"
- Range `n`: integers from 0 to 100
- Standard input: "{word}\n"
- Arguments: "{n}"
- Expect standard output: "1\n"
- Expect exit code: 0
```

A Cases block has one `Range` or more. A string in it can name a range as `{word}`; write
`{{` and `}}` for braces. Every name must be a range of the block, and every range must be
used. lengua draws boundary values first, then more values from a fixed seed. Ranges and
`{...}` are only in Cases blocks.

lengua runs the boundary values first: each range's low end, the value above it, the value
below the high end and the high end (for text, those are its lengths). Then it runs 20 more
cases drawn from a fixed seed, so a block has at most 24 cases, the same ones on every system.
A failed case names its drawn values.

## How a run ends

A program that stops at its time limit or its memory limit has no exit code. Give the end:

```
### Example: no input

- Expect end: out of time
```

## What check measures

When every test passes, `check` changes the code in small ways (an operator, a comparison, a
constant, a condition, a return value), at most 40 mutants, and runs the tests on each. A
mutant that a test fails on is caught. The program is `checked` when the tests catch 80% of the
mutants or more, and `tested` when they catch fewer. Each mutant that survived is a note with
its file and line: write an example whose result tells it apart from the code, for one at each
boundary of a comparison.
