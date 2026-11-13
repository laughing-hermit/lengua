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

A screen example has one `Expect` item or more.

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
