---
type: Attested Computation
title: follow
description: Gets a page from a web server of example.org and follows one redirect to another of its web servers.
runtime: webassembly
computation: main.wat
parameters:
  - { name: computer, type: text, required: true, from: argument 1 }
  - { name: path, type: text, required: true, from: argument 2 }
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
A shell script can use this program to read a page from a web server of example.org. The program
gets the name of a computer and a path as its arguments. It opens a connection to TCP port 80 of
that computer and sends an HTTP/1.0 GET request for the path. Then it prints the body of the
response, which is all of the response after the first blank line.

A web server can send the program to a page of another computer. It does this with a header line
`Location: http://` and then the name of the computer and the path. The program then gets that
page and prints its body. It follows one such line only.

The program opens connections only to the subdomains of example.org. It learns the second
computer from the response, so the idea names a domain and not each computer.

The name of a computer has `1` to `253` characters. A path has `1` to `100` characters, as in
`/hello`: a slash and up to `99` more characters. A response has up to `65535` bytes, and its
body has up to `65000` bytes.

The exit codes for errors are these:
- If the program cannot connect to a web server, it exits with code 3.
- If the program cannot write its output, it exits with code 4.
- If its idea does not let it reach a computer, it exits with code 5.

# Behaviors
## B1: Prints the body of a page
- Given the arguments `www.example.org` and `/hello`
- And a server at `www.example.org` on port `80` that receives `GET /hello HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nhello world`
- When the program runs
- Then it prints `hello world`
- And it exits with code `0`

### For any `path` and `body`
- Given the arguments `www.example.org` and `{path}`
- And a server at `www.example.org` on port `80` that receives `GET {path} HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- When the program runs
- Then it prints `{body}`
- And it exits with code `0`

| path | body |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

## B2: Follows a redirect to another web server of example.org
- Given the arguments `www.example.org` and `/old`
- And a server at `www.example.org` on port `80` that receives `GET /old HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://files.example.org/new\r\n\r\n`
- And a server at `files.example.org` on port `80` that receives `GET /new HTTP/1.0\r\nHost: files.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nmoved here`
- When the program runs
- Then it prints `moved here`
- And it exits with code `0`

### For any `path`, `next` and `body`
- Given the arguments `www.example.org` and `{path}`
- And a server at `www.example.org` on port `80` that receives `GET {path} HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://files.example.org/{next}\r\n\r\n`
- And a server at `files.example.org` on port `80` that receives `GET /{next} HTTP/1.0\r\nHost: files.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- When the program runs
- Then it prints `{body}`
- And it exits with code `0`

| path | next | body |
|---|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 99, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

## B3: Refuses a redirect to a computer that its idea does not name
- Given the arguments `www.example.org` and `/old`
- And a server at `www.example.org` on port `80` that receives `GET /old HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://elsewhere.test/new\r\n\r\n`
- When the program runs
- Then it prints `error: this program may not reach that computer` and a newline to standard error
- And it exits with code `5`

### For any `path` and `next`
- Given the arguments `www.example.org` and `{path}`
- And a server at `www.example.org` on port `80` that receives `GET {path} HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://elsewhere.test/{next}\r\n\r\n`
- When the program runs
- Then it prints `error: this program may not reach that computer` and a newline to standard error
- And it exits with code `5`

| path | next |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 99, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` |

## B4: Shows the usage line when it does not get two arguments
- Given no arguments
- When the program runs
- Then it prints `usage: follow <computer> <path>` and a newline to standard error
- And it exits with code `1`

## B5: Says when it cannot connect to the web server
- Given the arguments `www.example.org` and `/`
- And no server at `www.example.org` on port `80`
- When the program runs
- Then it prints `error: can't reach the server` and a newline to standard error
- And it exits with code `3`

### For any `path`
- Given the arguments `www.example.org` and `{path}`
- And no server at `www.example.org` on port `80`
- When the program runs
- Then it prints `error: can't reach the server` and a newline to standard error
- And it exits with code `3`

| path |
|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` |

## B6: Exits with code 4 when it cannot write its output
- Given the arguments `www.example.org` and `/hello`
- And a server at `www.example.org` on port `80` that receives `GET /hello HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nhello world`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `path` and `body`
- Given the arguments `www.example.org` and `{path}`
- And a server at `www.example.org` on port `80` that receives `GET {path} HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| path | body |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(1, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

# Constraints
- Network: opens TCP connections to port 80 on any subdomain of example.org
