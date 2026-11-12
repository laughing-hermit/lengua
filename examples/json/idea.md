---
type: Attested Computation
title: json
description: A part that checks a JSON value and writes it in its short form, and prints the JSON value on standard input in that form.
runtime: webassembly
computation: main.wat
parameters:
  - { name: text, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a part. Other programs take its functions, which check that a text is one JSON
value and write the value in its short form. The short form has no space, tab or line end
between its tokens.

The program itself reads standard input, which has at most `1024` bytes, and prints its JSON
value in the short form. Input of `1025` to `2000` bytes is too long. A value can have arrays
and objects inside others, up to `32` levels. A value with `33` levels is not valid for this
program. The tests give numbers from `-999999` to `999999`, fractions from `0` to `999999`,
exponents from `0` to `20`, and strings of `0` to `20` characters.

The program does not change a string or a number. It does not check that the bytes of a string
are UTF-8. It does not look for a name that an object has two times.

If the text is not JSON, the program exits with code 1. If the program cannot read its input,
it exits with code 3. If the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Removes the spaces between the tokens of an object
- Given standard input that contains `{ "id" : 7 , "tags" : [ "a b" , true ] }` and a newline
- When the program runs
- Then it prints `{"id":7,"tags":["a b",true]}` and a newline
- And it exits with code `0`

### For any name, number and string
The second row changes only the number, so that a proof can cover every number.

- Given standard input that contains `{{ "{k}" : {n} , "tags" : [ "{s}" , true ] }}` and a newline
- When the program runs
- Then it prints `{{"{k}":{n},"tags":["{s}",true]}}` and a newline
- And it exits with code `0`

| k | n | s | runs |
|---|---|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz_ ")` | `int(-999999, 999999)` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz ,:[]")` | 60 |
| `choice("id")` | `int(-999999, 999999)` | `choice("a b")` | 40 |

## B2: Takes a number with a fraction and an exponent
- Given standard input that contains `[ 12.5e3 , -0.25E+7 ]`
- When the program runs
- Then it prints `[12.5e3,-0.25E+7]` and a newline
- And it exits with code `0`

### For any fraction and exponent
- Given standard input that contains `[ {n}.{f}e{e} , -0.25E+{g} ]`
- When the program runs
- Then it prints `[{n}.{f}e{e},-0.25E+{g}]` and a newline
- And it exits with code `0`

| n | f | e | g | runs |
|---|---|---|---|---|
| `int(-999999, 999999)` | `int(0, 999999)` | `int(0, 20)` | `int(0, 20)` | 60 |

## B3: Takes a value that is not in an array or an object
- Given standard input that contains `null` and a newline
- When the program runs
- Then it prints `null` and a newline
- And it exits with code `0`

### For any such value
- Given standard input that contains `{w}` and a newline
- When the program runs
- Then it prints `{w}` and a newline
- And it exits with code `0`

| w | runs |
|---|---|
| `choice("null", "true", "false", "0", "-0", "7e-2", "[]", "[[]]", "\"\"", "\"a\\n\\u00e9\\\\\"")` | 30 |

## B4: Rejects text that is not JSON
- Given standard input that contains `{"a":1,}`
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

### For any text that is not JSON
- Given standard input that contains `{w}`
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

| w | runs |
|---|---|
| `choice("{\"a\":1,}", "[1,]", "[1 2]", "01", "1.", "-", "1e", "tru", "nulll", "falsE", "\"abc", "[1]]", "", " ", "[,]", "[", "\"a\\qb\"", "\"\\u12g4\"", "\"a\tb\"", "+1", ".5", "1 2", "[1}", "]")` | 60 |

## B5: Rejects an object that is not complete
- Given standard input that contains `{"a" 1}`
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

### For any object that is not complete
- Given standard input that contains `{{{w}}}`
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

| w | runs |
|---|---|
| `choice("\"a\" 1", "\"a\":1,", ",", "1:2", "\"a\":", "\"a\"", "a:1", "\"a\":1 \"b\":2", "\"a\"::1", "\"a\":1]")` | 30 |

## B6: Takes arrays that are 32 levels deep
- Given standard input that contains `[` `32` times then `7` then `]` `32` times
- When the program runs
- Then it prints `[` `32` times then `7` then `]` `32` times and a newline
- And it exits with code `0`

### For any number in the middle
- Given standard input that contains `[` `32` times then `{n}` then `]` `32` times
- When the program runs
- Then it prints `[` `32` times then `{n}` then `]` `32` times and a newline
- And it exits with code `0`

| n | runs |
|---|---|
| `int(-999999, 999999)` | 20 |

## B7: Rejects arrays that are 33 levels deep
- Given standard input that contains `[` `33` times then `7` then `]` `33` times
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

### For any number in the middle
- Given standard input that contains `[` `33` times then `{n}` then `]` `33` times
- When the program runs
- Then it prints `error: not JSON` and a newline to standard error
- And it exits with code `1`

| n | runs |
|---|---|
| `int(-999999, 999999)` | 20 |

## B8: Rejects input that is too long
- Given standard input that contains `1` `1025` times
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

### For any input that is too long
- Given standard input that contains `{pad}`
- When the program runs
- Then it prints `error: input too long` and a newline to standard error
- And it exits with code `2`

| pad | runs |
|---|---|
| `string(1025, 2000, "1 ")` | 20 |

## B9: Exits with code 4 when it cannot write its output
- Given standard input that contains `[ 1 ]`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any number
- Given standard input that contains `[ {n} ]`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| n | runs |
|---|---|
| `int(-999999, 999999)` | 20 |

# Constraints
- Uses: standard input

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies
each function it takes from main.wat, word for word, under a `;; from: ../json` line, with the
`$JSON_` constants that the function reads and the other functions of the part that it calls.

- `$json_minify`: reads 1024 bytes at `$text`; writes 1024 bytes at `$into`. Its arguments are
  `$text`, `$length` and `$into`. It checks that the first `$length` bytes at `$text` are one
  JSON value and writes the value at `$into` with no space, tab or line end between its tokens.
  It gives the number of bytes it wrote, which is at least 1 and at most `$length`. It gives 0
  when the text is not one JSON value, or has more than 32 levels. `$length` is at most 1024:
  the function stops the program with assertion 861 when it is not. It calls every other
  function of this contract.
- `$json_structure`: uses no memory. Its arguments are a byte and where the text stands, as 64
  bits. It gives where the text stands after a brace, a bracket, a comma or a colon, or one of
  three negative numbers for a byte that is not one of those there.
- `$json_at`: reads 1024 bytes at `$text`. Its arguments are `$text`, `$i` and `$length`. It
  gives the byte at place `$i`, or 256 when `$i` is not below `$length`.
- `$json_copy`: reads 1024 bytes at `$text`; writes 1024 bytes at `$into`. Its arguments are
  `$text`, `$from`, `$end`, `$into` and `$o`. It copies the bytes from place `$from` to place
  `$end` of `$text` to place `$o` of `$into` and gives the place after the last byte it wrote.
- `$json_scalar_end`: reads 1024 bytes at `$text`. Its arguments are `$text`, `$from` and
  `$length`. It gives the place after the string, the number or the word that starts at place
  `$from`, or 0 when none starts there.
- `$json_string_end`: reads 1024 bytes at `$text`. It takes the same arguments, and gives the
  place after the string whose first quote is at place `$from`, or 0.
- `$json_digits_end`: reads 1024 bytes at `$text`. It takes the same arguments, and gives the
  place after the digits that start at place `$from`, which is `$from` when there is no digit.
- `$json_number_end`: reads 1024 bytes at `$text`. It takes the same arguments, and gives the
  place after the number that starts at place `$from`, or 0.
- `$json_word_end`: reads 1024 bytes at `$text`. It takes the same arguments, and gives the
  place after the word true, false or null that starts at place `$from`, or 0.
