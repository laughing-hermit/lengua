# The test format

Each behaviour of an idea is a test. Its examples run against the built program at L5, and
are compared exactly.

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
| `Expect standard output` | a string, compared exactly |
| `Expect standard error` | a string, compared exactly (not compared when absent) |
| `Expect exit code` | a number from 0 to 255; every example has one |
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
