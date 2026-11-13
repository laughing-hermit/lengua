# Purpose

The program is an HTTP server. It answers `GET /hello?name=<name>` with the text
`Hello, <name>!`, and any other path with the status 404. The host serves it on the TCP port
7810, and the program reaches nothing else.

# Behaviours

## Answer a request with a name

Given a request for `/hello?name=Ada`,
When the server answers,
Then the status is 200 and the body is `Hello, Ada!`.

### Example: Ada

- Request: `GET /hello?name=Ada`
- Expect status: 200
- Expect header `content-type`: "text/plain"
- Expect body: "Hello, Ada!"

## Answer 404 for an unknown path

Given a request for `/other`,
When the server answers,
Then the status is 404 and the body is `not found`.

### Example: another path

- Request: `GET /other`
- Expect status: 404
- Expect body: "not found"

## Answer 404 for an empty name

Given a request for `/hello?name=` with no name,
When the server answers,
Then the status is 404.

### Example: no name

- Request: `GET /hello?name=`
- Expect status: 404
- Expect body: "not found"

# Constraints

- Serves: HTTP on TCP port 7810
- Memory: 32 MiB
- Time: 5 s
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`

# Structure

- `Greet::handle`: answers a request with the status and the body that `answer` gives.
- `answer`: gives the status and the body for the path of a request.
