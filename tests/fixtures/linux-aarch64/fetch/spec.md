---
type: lengua/program-spec
title: fetch
description: Fetches a page from a web server on this computer and prints its body.
---
# Purpose
Lets a shell script read a page from a web server on this computer: given the server's port and
a path, it sends an HTTP/1.0 GET request for the path to 127.0.0.1 and prints the response's
body, everything after the first blank line. The port is a number from `1` to `65535`, and the
path holds `1` to `100` characters, such as `/hello`. The response holds up to `65535` bytes and
its body up to `65000`. If the server can't be reached, the program exits with code 3; if writing
the output fails, it exits with code 4.

# Behaviors
## B1: Prints the body of a page
- Given the arguments `8080` and `/hello`
- And a server on port `8080` that receives `GET /hello HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nhello world`
- When the program runs
- Then it prints `hello world`
- And it exits with code `0`

### For any `path` and `body`
- Given the arguments `8080` and `{path}`
- And a server on port `8080` that receives `GET {path} HTTP/1.0\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\n{body}`
- When the program runs
- Then it prints `{body}`
- And it exits with code `0`

| path | body |
|---|---|
| `string(1, 100, "/abcdefghijklmnopqrstuvwxyz0123456789-._~")` | `string(0, 65000, "abcdefghijklmnopqrstuvwxyz \n")` |

## B2: Shows usage unless given a port and a path
- Given no arguments
- When the program runs
- Then it prints `usage: fetch <port> <path>` followed by a newline to standard error
- And it exits with code `1`

## B3: Rejects a port that isn't a number from 1 to 65535
- Given the arguments `80x` and `/`
- When the program runs
- Then it prints `error: bad port` followed by a newline to standard error
- And it exits with code `2`

### For any `port` and `path`
- Given the arguments `{port}` and `{path}`
- When the program runs
- Then it prints `error: bad port` followed by a newline to standard error
- And it exits with code `2`

| port | path |
|---|---|
| `choice("80x", "", "-", "0", "65536", "-80", "1.5", "99999999999999999999")` | `choice("/")` |

## B4: Says when the server can't be reached
- Given the arguments `1` and `/`
- And no server on port `1`
- When the program runs
- Then it prints `error: can't reach the server` followed by a newline to standard error
- And it exits with code `3`

### For any `port`
- Given the arguments `{port}` and `/`
- When the program runs
- Then it prints `error: can't reach the server` followed by a newline to standard error
- And it exits with code `3`

| port |
|---|
| `int(1, 65535)` |

# Constraints
- Network: connects to TCP ports 80 and 8080
