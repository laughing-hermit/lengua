# The network a program chooses while it runs

Most programs need no line of this page. A program that talks to places its idea can name gets
its connections from lengua, made before it starts (`- Network: connects to …`), and a server
gets its listener (`- Network: listens on …`): that is the least a program can have, so choose
it whenever the places are known when the idea is written.

A program that learns where to connect while it runs can't have its connections made before it
starts: a client that follows a redirect, a crawler, a proxy, a tool that asks a name server. Its
idea grants what it may open by itself, a `- Network:` line for each grant. lengua checks the
lines, tells the user what they grant before the program runs, and holds every call the program
makes to them.

## The lines

| Line | What the program may do |
|---|---|
| `- Network: opens TCP connections to port 443 on example.com over TLS` | Open connections to those ports of those computers, as many as it needs, one after another or several at once |
| `- Network: looks up the addresses of example.com` | Ask for the addresses of those names, for a program whose output is the addresses |
| `- Network: sends UDP datagrams to port 53 on 192.0.2.53` | Send datagrams to those ports of those computers, and receive the answers |
| `- Network: receives UDP datagrams on port 5353` | Receive the datagrams that arrive on that port of this computer, which lengua binds, and answer each sender |

- **The ports are the idea's**: `port 443`, or `ports 80 and 443`. No line grants any port.
- **`over TLS`**, at the end of a line that opens connections, has lengua do the TLS and check
  the computer's certificate; the program reads and writes plain bytes. A line without it
  grants plain connections only, and the two never mix: a computer granted over TLS is never
  reached in plain.
- **A program that only connects needs no line for names**: lengua resolves a name itself when
  the program opens a connection to it.
- **A UDP port any computer may send to** says so, `receives UDP datagrams on port 5353 for any
  computer`, as a server's `listens on` line does. Without it, only programs on this computer
  can send to it.
- **One of the two**: an idea listens on a TCP port or receives on a UDP port, not both.

## The computers a line may name

| In the line | Which computers |
|---|---|
| `on example.com`, `on 192.0.2.7` | That one computer, by its name as DNS writes it or by its address |
| `on any subdomain of example.com` | Every name beneath that domain, such as `www.example.com` and `a.b.example.com`, and not `example.com` itself, which is a name of its own: give it its own line when the program needs it |
| `on the computer its first argument names` | The computer the person who runs it types, `first` to `ninth` |
| `on any computer` | Any computer of the internet |

Grant the least that does the work: a named computer before a domain, a domain before `any
computer`. `any computer` is for a tool whose whole purpose is any computer, such as a crawler
or a proxy; say in your final report that the program may then reach any such computer on those
ports.

## What lengua never lets such a program reach

- **This computer**, by `localhost`, by its loopback or by any other spelling. A program reaches
  this computer only through `- Network: connects to TCP port N`, made before it starts.
- **A private network**: an address a home or an office uses inside, or a cloud's own, whether
  the program names the address or a name resolves to it. A line that ends with `, private
  addresses too` reaches them, and so does a line that writes the address itself, as `on
  192.168.1.20`. Write either only when the user asks for a computer of their own network.
- **An address of many computers**: a broadcast or a multicast address.
- **A computer no line covers**, whatever sent the program there: a redirect, a link, a reply.

lengua resolves every name itself and checks each address it finds, so none of this depends on
how the program, or a server it talks to, spells a computer. The program gets an error for what
lengua refuses, and the run's report says what it was. Give the program a behavior for it: what
it prints, and its exit code, when a computer isn't one its idea grants.

## The limits

A program holds at most 64 connections and datagram sockets of its own at once. lengua ends a
connection that no byte crosses for five minutes, and gives up on one that doesn't open within
5 seconds. A datagram holds at most 65,507 bytes. On a UDP port any computer may send to, the
program sends an address at most three times the bytes that address sent it, and on any UDP
port it answers only an address that sent it something. Purpose says the program's own limits,
as for any input: the longest name, path and reply it takes.

## The tests

The tests never reach a real computer: each step gives the program a server of the test's own.
[The test format](test-format.md#the-network) has every step; these are the ones such a program
needs:

- ``a server at `www.example.com` on port `443` that receives … and replies …``: the test's
  server for one connection the program opens to that computer and port. One step for each
  connection, in the order the program opens them there.
- ``no server at `www.example.com` on port `443` ``: that computer takes no connection in this
  test, as a computer that is down.
- ``a computer at `192.0.2.53` on UDP port `53` that receives … and replies …``: one datagram
  each way.
- ``the addresses `192.0.2.7` and `2001:db8::1` for the name `www.example.com` ``: what a look
  at that name gives.
- For a program that receives datagrams, ``a client that sends …`` and ``the client receives
  …``: one datagram each way.

Write the computers in the steps as they are, never as placeholders: a generated test draws what
is sent and received, not where. Give the program one behavior where it reaches a computer its
idea grants, one where a granted computer takes no connection, and one where it is sent to a
computer its idea doesn't grant, as `follow` has.

## An example

`follow` gets a page from a web server of example.org and follows one redirect to another of
its web servers. Its idea grants `- Network: opens TCP connections to port 80 on any subdomain
of example.org`, and these two behaviors show the redirect followed and refused:

```markdown
## B2: Follows a redirect to another web server of example.org
- Given the arguments `www.example.org` and `/old`
- And a server at `www.example.org` on port `80` that receives `GET /old HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://files.example.org/new\r\n\r\n`
- And a server at `files.example.org` on port `80` that receives `GET /new HTTP/1.0\r\nHost: files.example.org\r\n\r\n` and replies `HTTP/1.0 200 OK\r\n\r\nmoved here`
- When the program runs
- Then it prints `moved here`
- And it exits with code `0`

## B3: Refuses a redirect to a computer that its idea does not name
- Given the arguments `www.example.org` and `/old`
- And a server at `www.example.org` on port `80` that receives `GET /old HTTP/1.0\r\nHost: www.example.org\r\n\r\n` and replies `HTTP/1.0 302 Found\r\nLocation: http://elsewhere.test/new\r\n\r\n`
- When the program runs
- Then it prints `error: this program may not reach that computer` and a newline to standard error
- And it exits with code `5`
```
