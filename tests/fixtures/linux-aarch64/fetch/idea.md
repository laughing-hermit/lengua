---
type: lengua/program-idea
title: fetch
description: Gets a page from the web server on port 8080 of this computer and prints its body.
---
# Purpose
A shell script can use this program to read a page from the web server on TCP port `8080` of this
computer. The program gets a path as its argument. It sends an HTTP/1.0 GET request for the path.
Then it prints the body of the response, which is all of the response after the first blank line.

A path has `1` to `100` characters, as in `/hello`. The response has up to `65535` bytes, and its
body has up to `65000` bytes.

If the program cannot connect to the server, it exits with code 3. If the program cannot write its
output, it exits with code 4.

# Behaviors
## B1: Prints the body of a page
- Given the argument `/hello`
- And a server on port `8080` that receives `GET /hello HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nhello world`
- When the program runs
- Then it prints `hello world`
- And it exits with code `0`

### For any `path` and `body`
- Given the argument `{path}`
- And a server on port `8080` that receives `GET {path} HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- When the program runs
- Then it prints `{body}`
- And it exits with code `0`

| path | body |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

## B2: Shows the usage line when it gets no path
- Given no arguments
- When the program runs
- Then it prints `usage: fetch <path>` and a newline to standard error
- And it exits with code `1`

## B3: Says when it cannot connect to the server
- Given the argument `/`
- And no server on port `8080`
- When the program runs
- Then it prints `error: can't reach the server` and a newline to standard error
- And it exits with code `3`

### For any `path`
- Given the argument `{path}`
- And no server on port `8080`
- When the program runs
- Then it prints `error: can't reach the server` and a newline to standard error
- And it exits with code `3`

| path |
|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` |

## B4: Exits with code 4 when it cannot write its output
- Given the argument `/hello`
- And a server on port `8080` that receives `GET /hello HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nhello world`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

### For any `path` and `body`
- Given the argument `{path}`
- And a server on port `8080` that receives `GET {path} HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- And standard output is closed
- When the program runs
- Then it exits with code `4`

| path | body |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(1, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

# Constraints
- Network: connects to TCP port 8080
