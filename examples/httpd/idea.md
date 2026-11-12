---
type: Attested Computation
title: httpd
description: A web server that serves the files of a folder and keeps a store of JSON values, for up to 8 clients at once.
runtime: webassembly
computation: main.wat
executor:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/run.md
  receipt: [stdout, stderr, exit, files]
attester:
  resource: https://github.com/laughing-hermit/lengua/blob/main/docs/check.md
---
# Purpose
This program is a small web server for the folder that it runs in. It speaks HTTP/1.1 on TCP
port 8080 of this computer, for up to 8 clients at once. It answers one request on each
connection and then closes the connection. It never stops.

The server has these routes:

- `GET /health` answers `ok`, so that a person can see that the server runs.
- `GET /files/` and a path answers the file of that path in the folder `public`. The answer
  names the type of the file from the end of its name, and has an `ETag` field.
- `GET /files/` and a path that ends with a slash answers a list of that folder, in byte order.
  Each line of the list is one name, and the name of a folder ends with a slash.
- `GET /kv` answers the store, which is one JSON object.
- `GET /kv/` and a key answers the value of that key in the store.
- `PUT /kv/` and a key puts the JSON value in the body of the request under that key.
- `DELETE /kv/` and a key removes that key from the store.

The server keeps the store in the file `store.json`, with one member on each line and the keys
in byte order. When there is no such file, the store is empty.

These are the limits:

- The head of a request has at most `4096` bytes, and its body at most `4096` bytes. The tests
  give a body length of `4097` to `999999` to a request that is too large.
- The target of a request has at most `1024` bytes.
- The server holds a file in memory to send it, so a file has at most `1048576` bytes.
- A folder has at most `256` names in a list.
- A key has `1` to `64` letters, digits, hyphens, points and underscores.
- A value in the store has at most `1024` bytes, and the store has at most `65536` bytes.
- The tests give files of `0` to `2000` bytes, parts of files of `5` bytes, queries of `0` to
  `20` characters, numbers from `0` to `999999`, and values of `8` characters.
- The tests ask for parts of files that start at byte `2001` to `999999`.

The server answers a request for a part of a file with that part, when the request has one
`Range` field of one range. It answers status 304 when the `If-None-Match` field of the request
is the `ETag` of the file. The `ETag` is the CRC-32 of the file, in decimal.

For each request, the server prints one line to standard output: the method, the target, the
status and the length of the body of the answer. It prints two hyphens in place of the method
and the target when it cannot read the request line, or when the target is too long. The server
does not stop when it cannot
print this line.

# Behaviors
## B1: Says that it runs
- Given a client that sends `GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And it prints `GET /health 200 3` and a newline
- And it does not stop

### For any query and header field
- Given a client that sends `GET /health?{q} HTTP/1.{minor}\r\nHost: localhost\r\nX-{name}: {value}\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And it prints `GET /health?{q} 200 3` and a newline
- And it does not stop

| q | minor | name | value | runs |
|---|---|---|---|---|
| `string(0, 20, "abcxyz019=&")` | `choice(0, 1)` | `string(0, 20, "abcXYZ-")` | `string(0, 20, "abc xyz:,;=")` | 10 |

## B2: Serves a file with its type and its ETag
- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nHost: localhost\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nETag: "3421780262"\r\nConnection: close\r\n\r\n123456789`
- And it prints `GET /files/notes.txt 200 9` and a newline
- And it does not stop

### For any file
- Given a file `public/notes.txt` that contains `{body}`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nHost: localhost\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len(body)}\r\nETag: "{crc32(body)}"\r\nConnection: close\r\n\r\n{body}`
- And it prints `GET /files/notes.txt 200 {len(body)}` and a newline
- And it does not stop

| body | runs |
|---|---|
| `string(0, 2000, "abcdefghijklmnopqrstuvwxyz0123456789 <>/\n")` | 10 |

## B3: Names the type of a file from the end of its name
- Given a file `public/index.html` that contains `<p>hi</p>`
- And a file `public/site.css` that contains `body`
- And a file `public/app.js` that contains `let a`
- And a file `public/art/logo.svg` that contains `<svg/>`
- And a file `public/UP.TXT` that contains `up`
- And a file `public/blob` that contains `0101`
- And a client that sends `GET /files/index.html HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/site.css HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/app.js HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/art/logo.svg HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/UP.TXT HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/blob HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 9\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\n<p>hi</p>`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 4\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\nbody`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: 5\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\nlet a`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: 6\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\n<svg/>`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\nup`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\n0101`
- And it prints `GET /files/index.html 200 9\nGET /files/site.css 200 4\nGET /files/app.js 200 5\nGET /files/art/logo.svg 200 6\nGET /files/UP.TXT 200 2\nGET /files/blob 200 4\n`
- And it does not stop

### For any files
- Given a file `public/index.html` that contains `{a}`
- And a file `public/site.css` that contains `{b}`
- And a file `public/app.js` that contains `{c}`
- And a file `public/art/logo.svg` that contains `{d}`
- And a file `public/UP.TXT` that contains `{e}`
- And a file `public/blob` that contains `{f}`
- And a client that sends `GET /files/index.html HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/site.css HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/app.js HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/art/logo.svg HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/UP.TXT HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/blob HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {len(a)}\r\nETag: "{crc32(a)}"\r\nConnection: close\r\n\r\n{a}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: {len(b)}\r\nETag: "{crc32(b)}"\r\nConnection: close\r\n\r\n{b}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {len(c)}\r\nETag: "{crc32(c)}"\r\nConnection: close\r\n\r\n{c}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {len(d)}\r\nETag: "{crc32(d)}"\r\nConnection: close\r\n\r\n{d}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len(e)}\r\nETag: "{crc32(e)}"\r\nConnection: close\r\n\r\n{e}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {len(f)}\r\nETag: "{crc32(f)}"\r\nConnection: close\r\n\r\n{f}`
- And it prints `GET /files/index.html 200 {len(a)}\nGET /files/site.css 200 {len(b)}\nGET /files/app.js 200 {len(c)}\nGET /files/art/logo.svg 200 {len(d)}\nGET /files/UP.TXT 200 {len(e)}\nGET /files/blob 200 {len(f)}\n`
- And it does not stop

| a | b | c | d | e | f | runs |
|---|---|---|---|---|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | 6 |

## B4: Answers status 304 when the client has the file
- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nIf-None-Match: "3421780262"\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 304 Not Modified\r\nETag: "3421780262"\r\nConnection: close\r\n\r\n`
- And it prints `GET /files/notes.txt 304 0` and a newline
- And it does not stop

### For any query
- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET /files/notes.txt?{q} HTTP/1.1\r\nIf-None-Match: "3421780262"\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 304 Not Modified\r\nETag: "3421780262"\r\nConnection: close\r\n\r\n`
- And it prints `GET /files/notes.txt?{q} 304 0` and a newline
- And it does not stop

| q | runs |
|---|---|
| `string(0, 20, "abcxyz019=&")` | 6 |

## B5: Sends the file when the client has another ETag
- Given a file `public/notes.txt` that contains `abcdefgh`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nIf-None-Match: "7"\r\n\r\n`
- When the program runs
- Then the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 8\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\nabcdefgh`
- And it prints `GET /files/notes.txt 200 8` and a newline
- And it does not stop

### For any other ETag
- Given a file `public/notes.txt` that contains `{body}`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nIf-None-Match: "{n}"\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len(body)}\r\nETag: "{crc32(body)}"\r\nConnection: close\r\n\r\n{body}`
- And it prints `GET /files/notes.txt 200 {len(body)}` and a newline
- And it does not stop

| body | n | runs |
|---|---|---|
| `string(8, 8, "abcdefgh")` | `int(0, 999999)` | 6 |

## B6: Sends a part of a file
- Given a file `public/notes.txt` that contains `helloworldagain`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=5-9\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=10-\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=-5\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=0-4,10-14\r\n\r\n`
- When the program runs
- Then the client receives text that matches `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{any(0, 4294967295)}"\r\nContent-Range: bytes 5-9/15\r\nConnection: close\r\n\r\nworld`
- And the client receives text that matches `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{any(0, 4294967295)}"\r\nContent-Range: bytes 10-14/15\r\nConnection: close\r\n\r\nagain`
- And the client receives text that matches `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{any(0, 4294967295)}"\r\nContent-Range: bytes 10-14/15\r\nConnection: close\r\n\r\nagain`
- And the client receives text that matches `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 15\r\nETag: "{any(0, 4294967295)}"\r\nConnection: close\r\n\r\nhelloworldagain`
- And it prints `GET /files/notes.txt 206 5\nGET /files/notes.txt 206 5\nGET /files/notes.txt 206 5\nGET /files/notes.txt 200 15\n`
- And it does not stop

### For any three parts
- Given a file `public/notes.txt` that contains `{a}{b}{c}`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=5-9\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=10-\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=-5\r\n\r\n`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=0-4,10-14\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{crc32(a + b + c)}"\r\nContent-Range: bytes 5-9/15\r\nConnection: close\r\n\r\n{b}`
- And the client receives `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{crc32(a + b + c)}"\r\nContent-Range: bytes 10-14/15\r\nConnection: close\r\n\r\n{c}`
- And the client receives `HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nETag: "{crc32(a + b + c)}"\r\nContent-Range: bytes 10-14/15\r\nConnection: close\r\n\r\n{c}`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 15\r\nETag: "{crc32(a + b + c)}"\r\nConnection: close\r\n\r\n{a}{b}{c}`
- And it prints `GET /files/notes.txt 206 5\nGET /files/notes.txt 206 5\nGET /files/notes.txt 206 5\nGET /files/notes.txt 200 15\n`
- And it does not stop

| a | b | c | runs |
|---|---|---|---|
| `string(5, 5, "abcdefghijklmnopqrstuvwxyz")` | `string(5, 5, "abcdefghijklmnopqrstuvwxyz")` | `string(5, 5, "abcdefghijklmnopqrstuvwxyz")` | 8 |

## B7: Refuses a part that starts after the end of the file
- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes=2001-\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 416 Range Not Satisfiable\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nContent-Range: bytes */9\r\nConnection: close\r\n\r\n416 Range Not Satisfiable\n`
- And it prints `GET /files/notes.txt 416 26` and a newline
- And it does not stop

### For any file and start
- Given a file `public/notes.txt` that contains `{body}`
- And a client that sends `GET /files/notes.txt HTTP/1.1\r\nRange: bytes={n}-\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 416 Range Not Satisfiable\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nContent-Range: bytes */{len(body)}\r\nConnection: close\r\n\r\n416 Range Not Satisfiable\n`
- And it prints `GET /files/notes.txt 416 26` and a newline
- And it does not stop

| body | n | runs |
|---|---|---|
| `string(0, 2000, "abcdefghijklmnopqrstuvwxyz0123456789 <>/\n")` | `int(2001, 999999)` | 8 |

## B8: Lists a folder in byte order
- Given a file `public/b.txt` that contains `b`
- And a file `public/a.txt` that contains `a`
- And a file `public/Z.txt` that contains `z`
- And a file `public/sub/c.txt` that contains `c`
- And a client that sends `GET /files/ HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/sub/ HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nConnection: close\r\n\r\nZ.txt\na.txt\nb.txt\nsub/\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nc.txt\n`
- And it prints `GET /files/ 200 23\nGET /files/sub/ 200 6\n`
- And it does not stop

### For any query
- Given a file `public/b.txt` that contains `{body}`
- And a file `public/a.txt` that contains `a`
- And a file `public/Z.txt` that contains `z`
- And a file `public/sub/c.txt` that contains `c`
- And a client that sends `GET /files/?{q} HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/sub/ HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nConnection: close\r\n\r\nZ.txt\na.txt\nb.txt\nsub/\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nc.txt\n`
- And it prints `GET /files/?{q} 200 23\nGET /files/sub/ 200 6\n`
- And it does not stop

| body | q | runs |
|---|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcxyz019=&")` | 6 |

## B9: Answers status 404 when there is no such file or route
- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET /files/other.txt HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 14\r\nConnection: close\r\n\r\n404 Not Found\n`
- And it prints `GET /files/other.txt 404 14` and a newline
- And it does not stop

### For any target that names nothing
The names that start with a point, and the folder itself, are not files that the server sends.

- Given a file `public/notes.txt` that contains `123456789`
- And a client that sends `GET {target} HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 14\r\nConnection: close\r\n\r\n404 Not Found\n`
- And it prints `GET {target} 404 14` and a newline
- And it does not stop

| target | runs |
|---|---|
| `choice("/files/other.txt", "/", "/health/", "/healthy", "/files", "/files/notes.txt/", "/files/none/", "/files/../store.json", "/files/%2e%2e/store.json", "/files/.hidden", "/files/sub/.x", "/kvs", "/kv/missing", "/public/notes.txt")` | 15 |

## B10: Names the methods of a route when the method is not one of them
- Given a client that sends `POST /health HTTP/1.1\r\n\r\n`
- And a client that sends `POST /files/notes.txt HTTP/1.1\r\n\r\n`
- And a client that sends `POST /kv HTTP/1.1\r\n\r\n`
- And a client that sends `POST /kv/color HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET, PUT, DELETE\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And it prints `POST /health 405 23\nPOST /files/notes.txt 405 23\nPOST /kv 405 23\nPOST /kv/color 405 23\n`
- And it does not stop

### For any other method
- Given a client that sends `{m} /health HTTP/1.1\r\n\r\n`
- And a client that sends `{n} /files/notes.txt HTTP/1.1\r\n\r\n`
- And a client that sends `{n} /kv HTTP/1.1\r\n\r\n`
- And a client that sends `{m} /kv/color HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And the client receives `HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nContent-Length: 23\r\nAllow: GET, PUT, DELETE\r\nConnection: close\r\n\r\n405 Method Not Allowed\n`
- And it prints `{m} /health 405 23\n{n} /files/notes.txt 405 23\n{n} /kv 405 23\n{m} /kv/color 405 23\n`
- And it does not stop

| m | n | runs |
|---|---|---|
| `choice("POST", "HEAD", "PATCH", "OPTIONS", "TRACE")` | `choice("POST", "PUT", "DELETE", "HEAD", "PATCH")` | 10 |

## B11: Answers status 400 to a request that is not valid
- Given a client that sends `GET hello HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `- - 400 16` and a newline
- And it does not stop

### For any request that is not valid
- Given a client that sends `{request}`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `- - 400 16` and a newline
- And it does not stop

| request | runs |
|---|---|
| `choice("GET hello HTTP/1.1\r\n\r\n", "get / HTTP/1.1\r\n\r\n", "GET  / HTTP/1.1\r\n\r\n", "GET / HTTP/1.1 \r\n\r\n", "GET /a b HTTP/1.1\r\n\r\n", "GET / FTP/1.1\r\n\r\n", "\r\n\r\n", "ABCDEFGHIJKLMNOPQ / HTTP/1.1\r\n\r\n", "GET /\r\n\r\n")` | 9 |

## B12: Answers status 400 to a path or a field that is not valid
- Given a client that sends `GET /files/%zz HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `GET /files/%zz 400 16` and a newline
- And it does not stop

### For any such request
- Given a client that sends `GET {target} HTTP/1.1\r\n{field}\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `GET {target} 400 16` and a newline
- And it does not stop

| target | field | runs |
|---|---|---|
| `choice("/files/%zz", "/files/%4", "/files/a%00b", "/%e9", "/kv/a%20b", "/kv/", "/kv/a/b", "/kv/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")` | `choice("", "Host: localhost\r\n")` | 16 |
| `choice("/health", "/files/notes.txt", "/kv")` | `choice("No colon\r\n", ": empty\r\n", " folded: x\r\n", "A: b\nC: d\r\n", "Content-Length: x\r\n", "Content-Length: -1\r\n", "Content-Length: 1 2\r\n", "Content-Length:\r\n")` | 16 |

## B13: Answers status 431 when the head of a request is too long
- Given a client that sends `GET /health HTTP/1.1\r\nX-Pad: ` then `a` `4096` times then `\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Type: text/plain\r\nContent-Length: 36\r\nConnection: close\r\n\r\n431 Request Header Fields Too Large\n`
- And it prints `- - 431 36` and a newline
- And it does not stop

### For any longer head
- Given a client that sends `GET /health HTTP/1.1\r\nX-Pad: ` then `a` `4096` times then `{more}\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Type: text/plain\r\nContent-Length: 36\r\nConnection: close\r\n\r\n431 Request Header Fields Too Large\n`
- And it prints `- - 431 36` and a newline
- And it does not stop

| more | runs |
|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | 6 |

## B14: Answers status 413 when the body of a request is too large
- Given a client that sends `PUT /kv/color HTTP/1.1\r\nContent-Length: 4097\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 413 Content Too Large\r\nContent-Type: text/plain\r\nContent-Length: 22\r\nConnection: close\r\n\r\n413 Content Too Large\n`
- And it prints `PUT /kv/color 413 22` and a newline
- And it does not stop

### For any length that is too large
- Given a client that sends `PUT /kv/color HTTP/1.1\r\nContent-Length: {n}\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 413 Content Too Large\r\nContent-Type: text/plain\r\nContent-Length: 22\r\nConnection: close\r\n\r\n413 Content Too Large\n`
- And it prints `PUT /kv/color 413 22` and a newline
- And it does not stop

| n | runs |
|---|---|
| `int(4097, 999999)` | 8 |

## B15: Says which requests it cannot serve
- Given a client that sends `GET /health HTTP/2.0\r\n\r\n`
- And a client that sends `PUT /kv/color HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n`
- And a client that sends `PUT /kv/color HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/` then `a` `1024` times then ` HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 505 HTTP Version Not Supported\r\nContent-Type: text/plain\r\nContent-Length: 31\r\nConnection: close\r\n\r\n505 HTTP Version Not Supported\n`
- And the client receives `HTTP/1.1 501 Not Implemented\r\nContent-Type: text/plain\r\nContent-Length: 20\r\nConnection: close\r\n\r\n501 Not Implemented\n`
- And the client receives `HTTP/1.1 411 Length Required\r\nContent-Type: text/plain\r\nContent-Length: 20\r\nConnection: close\r\n\r\n411 Length Required\n`
- And the client receives `HTTP/1.1 414 URI Too Long\r\nContent-Type: text/plain\r\nContent-Length: 17\r\nConnection: close\r\n\r\n414 URI Too Long\n`
- And it prints `- - 505 31\nPUT /kv/color 501 20\nPUT /kv/color 411 20\n- - 414 17\n`
- And it does not stop

### For any key
- Given a client that sends `GET /health HTTP/2.0\r\n\r\n`
- And a client that sends `PUT /kv/{k} HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n`
- And a client that sends `PUT /kv/{j} HTTP/1.1\r\n\r\n`
- And a client that sends `GET /files/` then `a` `1024` times then ` HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 505 HTTP Version Not Supported\r\nContent-Type: text/plain\r\nContent-Length: 31\r\nConnection: close\r\n\r\n505 HTTP Version Not Supported\n`
- And the client receives `HTTP/1.1 501 Not Implemented\r\nContent-Type: text/plain\r\nContent-Length: 20\r\nConnection: close\r\n\r\n501 Not Implemented\n`
- And the client receives `HTTP/1.1 411 Length Required\r\nContent-Type: text/plain\r\nContent-Length: 20\r\nConnection: close\r\n\r\n411 Length Required\n`
- And the client receives `HTTP/1.1 414 URI Too Long\r\nContent-Type: text/plain\r\nContent-Length: 17\r\nConnection: close\r\n\r\n414 URI Too Long\n`
- And it prints `- - 505 31\nPUT /kv/{k} 501 20\nPUT /kv/{j} 411 20\n- - 414 17\n`
- And it does not stop

| k | j | runs |
|---|---|---|
| `string(1, 64, "abcXYZ019-._")` | `string(1, 64, "abcXYZ019-._")` | 6 |

## B16: Puts a value in the store and gives it back
- Given a client that sends `PUT /kv/color HTTP/1.1\r\nContent-Length: 21\r\n\r\n[ "bluegrey" , true ]`
- And a client that sends `GET /kv/color HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\nConnection: close\r\n\r\n["bluegrey",true]\n`
- And it prints `PUT /kv/color 204 0\nGET /kv/color 200 18\n`
- And the file `store.json` contains `{\n"color": ["bluegrey",true]\n}\n`
- And it does not stop

### For any key and value
- Given a client that sends `PUT /kv/{k} HTTP/1.1\r\nContent-Length: 21\r\n\r\n[ "{v}" , true ]`
- And a client that sends `GET /kv/{k} HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\nConnection: close\r\n\r\n["{v}",true]\n`
- And it prints `PUT /kv/{k} 204 0\nGET /kv/{k} 200 18\n`
- And the file `store.json` contains `{{\n"{k}": ["{v}",true]\n}}\n`
- And it does not stop

| k | v | runs |
|---|---|---|
| `string(1, 64, "abcXYZ019-._")` | `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | 10 |

## B17: Keeps the keys of the store in byte order
- Given a file `store.json` that contains `{\n"b": "oldvalue",\n"d": "ddddvalu"\n}\n`
- And a client that sends `PUT /kv/c HTTP/1.1\r\nContent-Length: 10\r\n\r\n"cccvalue"`
- And a client that sends `PUT /kv/a HTTP/1.1\r\nContent-Length: 10\r\n\r\n"aaavalue"`
- And a client that sends `PUT /kv/e HTTP/1.1\r\nContent-Length: 10\r\n\r\n"eeevalue"`
- And a client that sends `PUT /kv/b HTTP/1.1\r\nContent-Length: 10\r\n\r\n"newvalue"`
- And a client that sends `GET /kv HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 88\r\nConnection: close\r\n\r\n{\n"a": "aaavalue",\n"b": "newvalue",\n"c": "cccvalue",\n"d": "ddddvalu",\n"e": "eeevalue"\n}\n`
- And it prints `PUT /kv/c 204 0\nPUT /kv/a 204 0\nPUT /kv/e 204 0\nPUT /kv/b 204 0\nGET /kv 200 88\n`
- And the file `store.json` contains `{\n"a": "aaavalue",\n"b": "newvalue",\n"c": "cccvalue",\n"d": "ddddvalu",\n"e": "eeevalue"\n}\n`
- And it does not stop

### For any values
- Given a file `store.json` that contains `{{\n"b": "oldvalue",\n"d": "{d}"\n}}\n`
- And a client that sends `PUT /kv/c HTTP/1.1\r\nContent-Length: 10\r\n\r\n"{c}"`
- And a client that sends `PUT /kv/a HTTP/1.1\r\nContent-Length: 10\r\n\r\n"{a}"`
- And a client that sends `PUT /kv/e HTTP/1.1\r\nContent-Length: 10\r\n\r\n"{e}"`
- And a client that sends `PUT /kv/b HTTP/1.1\r\nContent-Length: 10\r\n\r\n"{b}"`
- And a client that sends `GET /kv HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 88\r\nConnection: close\r\n\r\n{{\n"a": "{a}",\n"b": "{b}",\n"c": "{c}",\n"d": "{d}",\n"e": "{e}"\n}}\n`
- And it prints `PUT /kv/c 204 0\nPUT /kv/a 204 0\nPUT /kv/e 204 0\nPUT /kv/b 204 0\nGET /kv 200 88\n`
- And the file `store.json` contains `{{\n"a": "{a}",\n"b": "{b}",\n"c": "{c}",\n"d": "{d}",\n"e": "{e}"\n}}\n`
- And it does not stop

| a | b | c | d | e | runs |
|---|---|---|---|---|---|
| `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | `string(8, 8, "abcdefghijklmnopqrstuvwxyz ")` | 8 |

## B18: Removes a key from the store
- Given a file `store.json` that contains `{\n"a": 1,\n"b": "bbbvalue",\n"c": 3\n}\n`
- And a client that sends `DELETE /kv/b HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/b HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/c HTTP/1.1\r\n\r\n`
- And a client that sends `GET /kv/a HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/a HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 14\r\nConnection: close\r\n\r\n404 Not Found\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n1\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And it prints `DELETE /kv/b 204 0\nDELETE /kv/b 404 14\nDELETE /kv/c 204 0\nGET /kv/a 200 2\nDELETE /kv/a 204 0\n`
- And the file `store.json` contains `{\n}\n`
- And it does not stop

### For any first value
- Given a file `store.json` that contains `{{\n"a": {n},\n"b": "bbbvalue",\n"c": 3\n}}\n`
- And a client that sends `DELETE /kv/b HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/b HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/c HTTP/1.1\r\n\r\n`
- And a client that sends `GET /kv/a HTTP/1.1\r\n\r\n`
- And a client that sends `DELETE /kv/a HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 14\r\nConnection: close\r\n\r\n404 Not Found\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {len(n) + 1}\r\nConnection: close\r\n\r\n{n}\n`
- And the client receives `HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n`
- And it prints `DELETE /kv/b 204 0\nDELETE /kv/b 404 14\nDELETE /kv/c 204 0\nGET /kv/a 200 {len(n) + 1}\nDELETE /kv/a 204 0\n`
- And the file `store.json` contains `{{\n}}\n`
- And it does not stop

| n | runs |
|---|---|
| `choice("1", "22", "\"three\"", "[4,4]", "null")` | 5 |

## B19: Answers status 400 to a body that is not JSON
- Given a client that sends `PUT /kv/color HTTP/1.1\r\nContent-Length: 4\r\n\r\n[1,]`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `PUT /kv/color 400 16` and a newline
- And it does not stop

### For any body of four bytes that is not JSON
- Given a client that sends `PUT /kv/{k} HTTP/1.1\r\nContent-Length: 4\r\n\r\n{body}`
- When the program runs
- Then the client receives `HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 16\r\nConnection: close\r\n\r\n400 Bad Request\n`
- And it prints `PUT /kv/{k} 400 16` and a newline
- And it does not stop

| k | body | runs |
|---|---|---|
| `string(1, 64, "abcXYZ019-._")` | `choice("[1,]", "tru ", "\"abc", "[1 2", "    ", "1.e1", "nul]", "{a:1")` | 8 |

## B20: Answers status 500 when the store is not in its form
- Given a file `store.json` that contains `not a store`
- And a client that sends `GET /kv/color HTTP/1.1\r\n\r\n`
- And a client that sends `GET /kv HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And it prints `GET /kv/color 500 26\nGET /kv 500 26\n`
- And it does not stop

### For any text that is not a store
- Given a file `store.json` that contains `{text}`
- And a client that sends `GET /kv/{k} HTTP/1.1\r\n\r\n`
- And a client that sends `GET /kv HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And it prints `GET /kv/{k} 500 26\nGET /kv 500 26\n`
- And it does not stop

| text | k | runs |
|---|---|---|
| `choice("not a store", "", "{}", "{\n}", "[\n]\n", "{\n\"a\": 1\n")` | `string(1, 64, "abcXYZ019-._")` | 6 |

## B21: Answers status 500 when a file is too large
- Given a file `public/big.bin` that contains `a` `1048577` times
- And a client that sends `GET /files/big.bin HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And it prints `GET /files/big.bin 500 26` and a newline
- And it does not stop

### For any query
- Given a file `public/big.bin` that contains `a` `1048577` times then `{more}`
- And a client that sends `GET /files/big.bin?{q} HTTP/1.1\r\n\r\n`
- When the program runs
- Then the client receives `HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 26\r\nConnection: close\r\n\r\n500 Internal Server Error\n`
- And it prints `GET /files/big.bin?{q} 500 26` and a newline
- And it does not stop

| more | q | runs |
|---|---|---|
| `string(0, 20, "abcdefghijklmnopqrstuvwxyz0123456789 <>/")` | `string(0, 20, "abcxyz019=&")` | 3 |

## B22: Answers status 507 when the store has no room for a value
- Given a file `store.json` that contains `{\n` then `"k": "01234567890123456789012345678901234567890123456789",\n` `1110` times then `"z": 1\n}\n`
- And a client that sends `PUT /kv/y HTTP/1.1\r\nContent-Length: 42\r\n\r\n"0123456789012345678901234567890123456789"`
- When the program runs
- Then the client receives `HTTP/1.1 507 Insufficient Storage\r\nContent-Type: text/plain\r\nContent-Length: 25\r\nConnection: close\r\n\r\n507 Insufficient Storage\n`
- And it prints `PUT /kv/y 507 25` and a newline
- And it does not stop

### For any key
- Given a file `store.json` that contains `{{\n` then `"k": "01234567890123456789012345678901234567890123456789",\n` `1110` times then `"z": 1\n}}\n`
- And a client that sends `PUT /kv/{k} HTTP/1.1\r\nContent-Length: 42\r\n\r\n"0123456789012345678901234567890123456789"`
- When the program runs
- Then the client receives `HTTP/1.1 507 Insufficient Storage\r\nContent-Type: text/plain\r\nContent-Length: 25\r\nConnection: close\r\n\r\n507 Insufficient Storage\n`
- And it prints `PUT /kv/{k} 507 25` and a newline
- And it does not stop

| k | runs |
|---|---|
| `string(1, 64, "abcXYZ019-._")` | 3 |

## B23: Answers clients at once
- Given a client that sends `GET /health?first HTTP/1.1\r\n\r\n`
- And a client that sends `GET /health?second HTTP/1.1\r\n\r\n`
- And the clients connect at once
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And it prints `GET /health?second 200 3\nGET /health?first 200 3\n`
- And it does not stop

### For any two queries
- Given a client that sends `GET /health?{a} HTTP/1.1\r\n\r\n`
- And a client that sends `GET /health?{b} HTTP/1.1\r\n\r\n`
- And the clients connect at once
- When the program runs
- Then the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And the client receives `HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n`
- And it prints `GET /health?{b} 200 3\nGET /health?{a} 200 3\n`
- And it does not stop

| a | b | runs |
|---|---|---|
| `string(0, 20, "abcxyz019=&")` | `string(0, 20, "abcxyz019=&")` | 8 |

# Constraints
- Uses: files
- Network: listens on TCP port 8080, up to 8 clients at once
- Builds on: [http](../http/idea.md), [json](../json/idea.md), [decimal](../decimal/idea.md), [crc32](../crc32/idea.md)
