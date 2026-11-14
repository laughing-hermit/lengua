---
type: lengua Library
description: "The library reads and writes the messages of HTTP / 1.1, as RFC 9112 gives them."
module: main
source: src/main.rs
uses: []
used_by: []
label: checked
idea_sha256: 831ca8a9e89d7e2dd8922bb4822c50598280d90e8d3530f6c22db1a3949d39c3
source_sha256: 21dd14779e5006b4ca78eb5ec14033f8aaec90d2e4dc9176be7ee0cc4cdc00be
cargo_sha256: a3bf7d6d1451d1966e988f6b934131778b48f1856b6d3f0c26c6026738355ce9
world_sha256: 43e5e032ac9bee5f73ecde94bf9f71093003bcc169f5f7d6e9e2310be5b0b626
generated: { by: lengua-mcp/0.1.0, at: 2026-11-14T04:28:08Z }
verified:
  - { by: process:lengua-check, at: 2026-11-14T04:28:08Z }
title: http
status: draft
---

# Purpose

The library reads and writes the messages of HTTP/1.1, as RFC 9112 gives them. A message is a
request or an answer, with its headers and a body with a length or in chunks. A header name is
in lower case after a read, and a search for a header ignores the case. It is a `lengua`
library on the standard library only; a server uses it on the bytes that its host gives.

# Behaviours

## Read a request

Given the bytes of a request,
When a program calls `parse_request`,
Then it gives the method, the target, the headers and the body.

### Example: a request with no body

- Call: `parse_request(b"GET /items?a=1 HTTP/1.1\r\nHost: shop\r\n\r\n")`
- Expect value: `Ok(Request { method: "GET", target: "/items?a=1", headers: [("host", "shop")], body: [] })`

### Example: a body with a length

- Call: `parse_request(b"POST /cart HTTP/1.1\r\nContent-Length: 3\r\n\r\nteaextra").map(|r| String::from_utf8(r.body).unwrap())`
- Expect value: `Ok("tea")`

### Example: a body in chunks

- Call: `parse_request(b"POST / HTTP/1.0\r\nTransfer-Encoding: chunked\r\n\r\n3\r\ntea\r\n0\r\n\r\n").map(|r| String::from_utf8(r.body).unwrap())`
- Expect value: `Ok("tea")`

## Report a request that is not valid

Given bytes that are not a valid request,
When a program calls `parse_request`,
Then it gives the error, and for a header the number of its line.

### Example: no blank line

- Call: `parse_request(b"GET / HTTP/1.1\r\n")`
- Expect value: `Err(Incomplete)`

### Example: no version

- Call: `parse_request(b"GET /\r\n\r\n")`
- Expect value: `Err(Line)`

### Example: another version

- Call: `parse_request(b"GET / HTTP/2\r\n\r\n")`
- Expect value: `Err(Version)`

### Example: a target that is not a path

- Call: `parse_request(b"GET x HTTP/1.1\r\n\r\n")`
- Expect value: `Err(Line)`

### Example: no method

- Call: `parse_request(b" / HTTP/1.1\r\n\r\n")`
- Expect value: `Err(Line)`

### Example: a header with no colon

- Call: `parse_request(b"GET / HTTP/1.1\r\nA: b\r\nBad\r\n\r\n")`
- Expect value: `Err(Header(2))`

### Example: a space in a name

- Call: `parse_request(b"GET / HTTP/1.1\r\nBad name: x\r\n\r\n")`
- Expect value: `Err(Header(1))`

### Example: no name

- Call: `parse_request(b"GET / HTTP/1.1\r\n: x\r\n\r\n")`
- Expect value: `Err(Header(1))`

### Example: a length that is not a number

- Call: `parse_request(b"GET / HTTP/1.1\r\nContent-Length: x\r\n\r\n")`
- Expect value: `Err(Length)`

### Example: a body that is short

- Call: `parse_request(b"PUT / HTTP/1.1\r\nContent-Length: 5\r\n\r\nab")`
- Expect value: `Err(Incomplete)`

## Read an answer

Given the bytes of an answer,
When a program calls `parse_response`,
Then it gives the status, the reason, the headers and the body.

### Example: an answer with a body to the end

- Call: `parse_response(b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nnone").map(|r| (r.status, r.reason, r.headers, String::from_utf8(r.body).unwrap()))`
- Expect value: `Ok((404, "Not Found", [("content-type", "text/plain")], "none"))`

### Example: the lowest status

- Call: `parse_response(b"HTTP/1.0 100 Continue\r\n\r\n").map(|r| r.status)`
- Expect value: `Ok(100)`

### Example: the highest status

- Call: `parse_response(b"HTTP/1.1 599 \r\n\r\n").map(|r| (r.status, r.reason))`
- Expect value: `Ok((599, ""))`

### Example: a status that is too low

- Call: `parse_response(b"HTTP/1.1 99 X\r\n\r\n")`
- Expect value: `Err(Status)`

### Example: a status that is too high

- Call: `parse_response(b"HTTP/1.1 600 X\r\n\r\n")`
- Expect value: `Err(Status)`

### Example: an answer of another version

- Call: `parse_response(b"HTTP/2 200 OK\r\n\r\n")`
- Expect value: `Err(Version)`

## Write a request and an answer

Given a request or an answer,
When a program calls `write_request` or `write_response`,
Then it gives the bytes, with a length header when the body is not empty and has no length.

### Example: a request with a body

- Call: `String::from_utf8(write_request(&Request { method: "POST".into(), target: "/cart".into(), headers: vec![("host".into(), "shop".into())], body: b"tea".to_vec() })).unwrap()`
- Expect value: `"POST /cart HTTP/1.1\r\nhost: shop\r\ncontent-length: 3\r\n\r\ntea"`

### Example: an answer with no body

- Call: `String::from_utf8(write_response(&Response { status: 204, reason: "No Content".into(), headers: vec![], body: vec![] })).unwrap()`
- Expect value: `"HTTP/1.1 204 No Content\r\n\r\n"`

### Example: an answer with its own length

- Call: `String::from_utf8(write_response(&Response { status: 200, reason: "OK".into(), headers: vec![("content-length".into(), "2".into())], body: b"hi".to_vec() })).unwrap()`
- Expect value: `"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nhi"`

### Example: an answer in chunks

- Call: `String::from_utf8(write_response(&Response { status: 200, reason: "OK".into(), headers: vec![("transfer-encoding".into(), "chunked".into())], body: b"0\r\n\r\n".to_vec() })).unwrap()`
- Expect value: `"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n0\r\n\r\n"`

## Find a header

Given headers and a name in another case,
When a program calls `header`,
Then it gives the value of the header with that name.

### Example: another case

- Call: `header(&[("content-type".into(), "text/plain".into())], "Content-Type")`
- Expect value: `Some("text/plain")`

### Example: no header

- Call: `header(&[], "host")`
- Expect value: `None`

## Write and read chunks

Given a body,
When a program calls `encode_chunked` and then `decode_chunked`,
Then the chunks have their sizes in hexadecimal digits, and the read gives the body back.

### Example: chunks of four bytes

- Call: `String::from_utf8(encode_chunked(b"teapot", 4)).unwrap()`
- Expect value: `"4\r\nteap\r\n2\r\not\r\n0\r\n\r\n"`

### Example: an empty body

- Call: `String::from_utf8(encode_chunked(b"", 4)).unwrap()`
- Expect value: `"0\r\n\r\n"`

### Example: chunks of zero bytes

- Call: `String::from_utf8(encode_chunked(b"ab", 0)).unwrap()`
- Expect value: `"1\r\na\r\n1\r\nb\r\n0\r\n\r\n"`

### Example: a read of chunks with an extension

- Call: `decode_chunked(b"4\r\nteap\r\n2;x=1\r\not\r\n0\r\n\r\nrest").map(|(body, used)| (String::from_utf8(body).unwrap(), used))`
- Expect value: `Ok(("teapot", 25))`

### Example: a size that is not hexadecimal

- Call: `decode_chunked(b"3\r\nabc\r\nz\r\n")`
- Expect value: `Err(Chunk(8))`

### Example: a chunk that is short

- Call: `decode_chunked(b"3\r\nab")`
- Expect value: `Err(Incomplete)`

### Example: a chunk with no line end

- Call: `decode_chunked(b"3\r\nabcX\r\n")`
- Expect value: `Err(Chunk(6))`

### Example: no blank line after the last chunk

- Call: `decode_chunked(b"0\r\nXY")`
- Expect value: `Err(Chunk(3))`

### Example: the end of the bytes after the last chunk

- Call: `decode_chunked(b"0\r\nX")`
- Expect value: `Err(Incomplete)`

### Cases: bodies that go through chunks

- Range `s`: text of 0 to 40 characters from "abc xyz"
- Range `n`: integers from 1 to 9
- Call: `decode_chunked(&encode_chunked(b"{s}", {n})).map(|(body, _)| String::from_utf8(body).unwrap())`
- Expect value: `Ok("{s}")`

# Constraints

- Memory: 16 MiB
- Time: 5 s
- Exports: `Request`, `Response`, `Error`, `parse_request`, `parse_response`, `write_request`, `write_response`, `header`, `decode_chunked`, `encode_chunked`

# Structure

- `line_end`: gives the position of the next carriage return and line end from a position.
- `split_head`: cuts the bytes into the lines of the head and the bytes after the blank line.
- `headers_of`: reads the header lines, with the names in lower case.
- `header`: gives the value of the first header with a name, in any case.
- `body_of`: reads the body by its length, in chunks, or to the end of an answer.
- `parse_request`: reads a request.
- `parse_response`: reads an answer.
- `head_and_body`: writes the first line, the headers, a length header when it is necessary, and
  the body.
- `write_request`: writes a request.
- `write_response`: writes an answer.
- `decode_chunked`: reads a body in chunks, and gives the body and the number of bytes it used.
- `encode_chunked`: writes a body in chunks of a size, with a size of 1 or more.

# Links

* Index: [http](index.md).
