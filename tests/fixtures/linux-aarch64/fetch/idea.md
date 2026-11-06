---
type: lengua/program-idea
title: fetch
description: Fetches a page from the web server on this computer's port 8080 and prints its body.
---
# Purpose
Lets a shell script read a page from the web server on this computer's TCP port `8080`: given a
path, it sends an HTTP/1.0 GET request for the path and prints the response's body, everything
after the first blank line. The path holds `1` to `100` characters, such as `/hello`. The
response holds up to `65535` bytes and its body up to `65000`. If the server can't be reached,
the program exits with code 3; if writing the output fails, it exits with code 4.

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

## B2: Shows usage unless given a path
- Given no arguments
- When the program runs
- Then it prints `usage: fetch <path>` followed by a newline to standard error
- And it exits with code `1`

## B3: Says when the server can't be reached
- Given the argument `/`
- And no server on port `8080`
- When the program runs
- Then it prints `error: can't reach the server` followed by a newline to standard error
- And it exits with code `3`

### For any `path`
- Given the argument `{path}`
- And no server on port `8080`
- When the program runs
- Then it prints `error: can't reach the server` followed by a newline to standard error
- And it exits with code `3`

| path |
|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` |

## B4: Exits with code 4 when its output can't be written
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
