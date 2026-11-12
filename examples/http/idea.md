---
type: Attested Computation
title: http
description: A part that reads the head of an HTTP request, and prints the method, the path and the host of the request on standard input.
runtime: webassembly
computation: main.wat
parameters:
  - { name: request, type: text, required: true, from: standard input }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a part. Other programs take its functions, which read the head of an HTTP/1.1
request: the request line and the header fields, up to the blank line.

The program itself reads one request from standard input. It prints the method, a space and
the path on one line, and the value of the Host field on the next line. The second line is
empty when the request has no Host field. The program ignores the bytes after the blank line.

- The head has at most `4096` bytes. Input of `4097` to `5000` bytes is too long.
- The request line is a method, a space, a target, a space and the version, 1.0 or 1.1.
- A method has 1 to 16 capital letters.
- A target starts with a slash and has only visible characters. The path is the target up to
  its first question mark.
- The program decodes each percent sign and its two hexadecimal digits. A decoded path has no
  control character, and at most `1024` bytes. A byte from 128 up is a part of the path, so a
  path can have text in UTF-8.
- Each header field is a name, a colon and a value, on a line that ends with a carriage return
  and a newline. The program compares names without regard to capital letters.
- The tests give paths of `0` to `100` characters and hosts of `1` to `40` characters. They give
  values of `0` to `40` characters and names of `0` to `10` characters. They also give short
  texts of up to `4` characters.

If the request is not valid, the program exits with code 1. If the request has another version
of HTTP, it exits with code 5. If the program cannot read its input, it exits with code 3. If
the program cannot write its output, it exits with code 4.

# Behaviors
## B1: Prints the method, the path and the host of a request
- Given standard input that contains `GET /hello.txt HTTP/1.1\r\nUser-Agent: curl/8.0\r\nhOsT:  example.com \r\nX-a: b\r\n\r\n`
- When the program runs
- Then it prints `GET /hello.txt\nexample.com\n`
- And it exits with code `0`

### For any method, path and fields
The second row has few texts, so that a proof can cover all of them.

- Given standard input that contains `{method} /{path} HTTP/1.{minor}\r\nUser-Agent: curl/8.0\r\nhOsT:  {host} \r\nX-{name}: {value}\r\n\r\n`
- When the program runs
- Then it prints `{method} /{path}\n{host}\n`
- And it exits with code `0`

| method | path | minor | host | name | value | runs |
|---|---|---|---|---|---|---|
| `choice("GET", "PUT", "DELETE", "HEAD", "POST", "OPTIONS")` | `string(0, 100, "abcdefghijklmnopqrstuvwxyz0189/._-~")` | `choice(0, 1)` | `string(1, 40, "abcdefghijklmnopqrstuvwxyz0123456789.-:")` | `string(0, 10, "abcXYZ-")` | `string(0, 40, "abc xyz:,;=\t")` | 60 |
| `choice("GET", "PUT")` | `choice("hello.txt", "", "a/b")` | `choice(0, 1)` | `choice("example.com", "a")` | `choice("a")` | `choice("b")` | 24 |

## B2: Decodes the percent escapes of a path and drops the query
- Given standard input that contains `GET /a%20b%2fc?a=b HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `GET /a b/c\n\n`
- And it exits with code `0`

### For any path and query
- Given standard input that contains `GET /{a}%20{b}%2f{c}?{q} HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `GET /{a} {b}/{c}\n\n`
- And it exits with code `0`

| a | b | c | q | runs |
|---|---|---|---|---|
| `string(0, 40, "abcxyz019._-")` | `string(0, 40, "abcxyz019._-")` | `string(0, 40, "abcxyz019._-")` | `string(0, 40, "abc=&%?/")` | 40 |

## B3: Rejects a request that is not valid
- Given standard input that contains `GET hello HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `error: bad request` and a newline to standard error
- And it exits with code `1`

### For any request that is not valid
- Given standard input that contains `{request}`
- When the program runs
- Then it prints `error: bad request` and a newline to standard error
- And it exits with code `1`

| request | runs |
|---|---|
| `choice("GET hello HTTP/1.1\r\n\r\n", "get / HTTP/1.1\r\n\r\n", "GET / HTTP/1.1\r\n", "GET  / HTTP/1.1\r\n\r\n", "GET / HTTP/1.1 \r\n\r\n", "GET /a b HTTP/1.1\r\n\r\n", "GET / FTP/1.1\r\n\r\n", "GET /%zz HTTP/1.1\r\n\r\n", "GET /%4 HTTP/1.1\r\n\r\n", "GET /%00 HTTP/1.1\r\n\r\n", "GET /%7f HTTP/1.1\r\n\r\n", "GET /a%1fb HTTP/1.1\r\n\r\n", "GET / HTTP/1.1\r\nNo colon\r\n\r\n", "GET / HTTP/1.1\r\n: empty\r\n\r\n", "GET / HTTP/1.1\r\n folded: x\r\n\r\n", "GET / HTTP/1.1\r\nA: b\nC: d\r\n\r\n", "", "\r\n\r\n", "ABCDEFGHIJKLMNOPQ / HTTP/1.1\r\n\r\n", "GET /\t HTTP/1.1\r\n\r\n", "GET /\r\n\r\n")` | 60 |

## B4: Rejects a path of more than 1024 bytes
- Given standard input that contains `GET /` then `a` `1024` times then ` HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `error: bad request` and a newline to standard error
- And it exits with code `1`

### For any longer path
- Given standard input that contains `GET /` then `a` `1024` times then `{more} HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `error: bad request` and a newline to standard error
- And it exits with code `1`

| more | runs |
|---|---|
| `string(0, 100, "ab")` | 20 |

## B5: Takes a path of 1024 bytes
- Given standard input that contains `GET /` then `a` `1023` times then ` HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `GET /` then `a` `1023` times then `\n\n`
- And it exits with code `0`

### For any method
- Given standard input that contains `{method} /` then `a` `1023` times then ` HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `{method} /` then `a` `1023` times then `\n\n`
- And it exits with code `0`

| method | runs |
|---|---|
| `choice("GET", "PUT", "DELETE")` | 6 |

## B6: Says when the request has another version of HTTP
- Given standard input that contains `GET / HTTP/2.0\r\n\r\n`
- When the program runs
- Then it prints `error: version` and a newline to standard error
- And it exits with code `5`

### For any other version
- Given standard input that contains `GET /{path} HTTP/{v}\r\n\r\n`
- When the program runs
- Then it prints `error: version` and a newline to standard error
- And it exits with code `5`

| path | v | runs |
|---|---|---|
| `string(0, 100, "abcxyz0189/._-~")` | `choice("2.0", "1.2", "0.9", "3.0")` | 20 |

## B7: Rejects a head that is too long
- Given standard input that contains `GET /` then `a` `4092` times
- When the program runs
- Then it prints `error: request too long` and a newline to standard error
- And it exits with code `2`

### For any input that is too long
- Given standard input that contains `{pad}`
- When the program runs
- Then it prints `error: request too long` and a newline to standard error
- And it exits with code `2`

| pad | runs |
|---|---|
| `string(4097, 5000, "GET/ a\r\n")` | 20 |

## B8: Takes a path that has text in UTF-8
- Given standard input that contains `GET /abc%c3%a9xyz%e2%82%ac HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `GET /abcéxyz€\n\n`
- And it exits with code `0`

### For any path around the two characters
- Given standard input that contains `GET /{a}%c3%a9{b}%e2%82%ac HTTP/1.1\r\n\r\n`
- When the program runs
- Then it prints `GET /{a}é{b}€\n\n`
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `string(0, 40, "abcxyz019._-")` | `string(0, 40, "abcxyz019._-")` | 20 |

## B9: Exits with code 4 when it cannot write its output
- Given standard input that contains `GET / HTTP/1.1\r\n\r\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any path
- Given standard input that contains `GET /{path} HTTP/1.1\r\n\r\n`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| path | runs |
|---|---|
| `string(0, 100, "abcxyz0189/._-~")` | 20 |

# Constraints
- Uses: standard input

# Contract
A program that builds on this part names it on the `- Builds on:` line of its idea. It copies
each function it takes from main.wat, word for word, under a `;; from: ../http` line, with the
`$HTTP_` constants that the function reads and the other functions of the part that it calls.
In each function, `$text` is the head and `$length` its length, at most 4096 bytes.

- `$http_at`: reads 4096 bytes at `$text`. Its arguments are `$text`, `$i` and `$length`. It
  gives the byte at place `$i`, or 256 when `$i` is not below `$length`.
- `$http_span_end`: reads 4096 bytes at `$text`. Its arguments are `$text`, `$from`, `$length`,
  `$first` and `$span`. It gives the place after the bytes from `$first` to `$first` plus
  `$span` that start at place `$from`.
- `$http_head_end`: reads 4096 bytes at `$text`. Its arguments are `$text` and `$length`. It
  gives the place after the first blank line, which is the length of the head, or 0 when the
  bytes hold no blank line.
- `$http_line_end`: reads 4096 bytes at `$text`. Its arguments are `$text`, `$from` and
  `$length`. It gives the place of the first line end at or after place `$from`, or `$length`.
- `$http_request_line`: reads 4096 bytes at `$text`; writes 16 bytes at `$into`. Its arguments
  are `$text`, `$length` and `$into`. It gives 0 and writes four numbers of 4 bytes: the length
  of the method, the place of the target, the length of the target, and the last digit of the
  version. It gives 505 for another version of HTTP and 400 for any other text.
- `$http_headers_valid`: reads 4096 bytes at `$text`. Its arguments are `$text` and `$length`.
  It gives 1 when each header field is a name, a colon and a value, up to the blank line.
- `$http_name_is`: reads 4096 bytes at `$text`; reads 32 bytes at `$name`. Its arguments are
  `$text`, `$from`, `$length`, `$name` and `$name_length`. It gives 1 when the name, in small
  letters, is at place `$from` with a colon after it.
- `$http_blank_end`: reads 4096 bytes at `$text`. Its arguments are `$text`, `$from` and `$end`.
  It gives the place of the first byte from `$from` that is not a space or a tab.
- `$http_blank_start`: reads 4096 bytes at `$text`. It takes the same arguments, and gives the
  place where the spaces and tabs at the end of the bytes start.
- `$http_header`: reads 4096 bytes at `$text`; reads 32 bytes at `$name`; writes 8 bytes at
  `$into`. Its arguments are `$text`, `$length`, `$name`, `$name_length` and `$into`. It gives
  1 and writes the place and the length of the value of the first field with that name, each
  as 4 bytes. It gives 0 when the head has no such field.
- `$http_hex`: uses no memory. It gives the value of a hexadecimal digit, or 16.
- `$http_path`: reads 4096 bytes at `$text`; writes 1024 bytes at `$into`. Its arguments are
  `$text`, `$from`, `$end` and `$into`. It decodes the path of the target from place `$from`
  to place `$end`, writes it at `$into` and gives its length. It gives -1 for a path that is
  not valid or has more than 1024 bytes.
