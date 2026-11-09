---
type: Attested Computation
title: echo
description: A server that sends each line back to the client that sent it.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a small network service. Use it to test clients. On each connection, it reads a
line of at most `1023` characters and a newline. It sends the line back and closes the
connection. Then it waits for the next connection.

It reads until the data ends with a newline, until the client closes its side, or until it has
1024 bytes. Then it sends back exactly the data that it read. It serves on TCP port 7007 of this
computer.

# Behaviors
## B1: Echoes a line
- Given a client that sends `hello` and a newline
- When the program runs
- Then the client receives `hello` and a newline
- And it does not stop

### For any `line`
- Given a client that sends `{line}` and a newline
- When the program runs
- Then the client receives `{line}` and a newline
- And it does not stop

| line |
|---|
| `string(0, 1023, "abcdefghijklmnopqrstuvwxyz ")` |

# Constraints
- Network: listens on TCP port 7007
