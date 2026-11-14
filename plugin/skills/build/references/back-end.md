# The back end: Rust compiled to WebAssembly

`check` writes `Cargo.toml` (only the crates the ideas declare) and `wit/world.wit` (only the
interfaces the grants allow), and builds for `wasm32-wasip2`. An import the grants don't allow
is `import-ungranted`.

## A program

`main` reads its standard input and arguments with the standard library and writes to
standard output. Files are reached only in the folders of `- Reads:` and `- Writes:`, by
relative paths. The exit code is `std::process::exit(<code>)` or the end of `main`.

## A server

The idea has ``- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the
HTTP world of `WASI` `` and `- Serves: HTTP on TCP port <n>`. Each request is an instance of
its own: keep state in a `- Writes:` folder, not in memory.

```rust
use wasip2::http::types::{IncomingRequest, ResponseOutparam};

struct App;

wasip2::http::proxy::export!(App);

impl wasip2::exports::http::incoming_handler::Guest for App {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let path = request.path_with_query().unwrap_or_default();
        debug_assert!(path.len() < 1 << 16);
        respond(response_out, answer(&path));
    }
}
```

Name `App::handle` in the structure. Answer the routes under `/api/` with JSON, and every other
`GET` from the front end: `lengua_web::asset(path)` gives `Some((media_type, bytes))` for a file
of the static export. The same helpers (`method_name`, `body_of`, `respond`, `header`) are in
every server: write them with `wasip2::http::types` (`Fields`, `OutgoingResponse`,
`OutgoingBody`).

## An AI model

With `- Asks:`, send `POST http://ai.lengua/ask` (the body is the prompt; `/stream` for pieces,
`/embed` for a vector) through `wasip2::http::outgoing_handler`. The header
`x-lengua-reply: json` asks for JSON. Past the limits the host answers 429; with no model, 503.
Check every answer: the library `ai` has `fill`, `then`, `fields`, `retryable` and
`estimate_tokens`.

## Other services, secrets, native parts

- ``- Calls: HTTP at `<host>:<port>` `` (or `HTTPS`, L10): outgoing requests only there.
- ``- Reads: the secret `NAME` ``: `std::env::var("NAME")`; never write it to a file or an
  answer.
- A native part (L10): `POST http://<name>.native/` with the input as the body; the answer is
  200 with its standard output, or 502.

## lengua's shared libraries

Use one with ``- Uses: `<path>/libraries/<name>` `` in the top idea, then `use <name>::...`.

| Library | Gives |
|---|---|
| `json` | `Value`, `parse`, `write`, `get` |
| `http` | `Request`, `Response`, `parse_request`, `parse_response`, `write_request`, `write_response`, `header`, chunked bodies |
| `decimal` | integers and fixed-point numbers as text, `add`, `multiply` |
| `text` | `lines`, `split`, `words`, `trim`, UTF-8 checks |
| `sort` | `stable_sort`, `stable_sort_by_key`, `binary_search` |
| `hashes` | `sha256`, `sha256_hex`, `crc32` |
| `dates` | `Date`, days from civil dates and back, `parse_date`, `format_timestamp` |
| `ai` | prompts and the fields of a JSON answer |
| `logins` | `hash_password`, `verify_password`, `sign_token`, `check_token`, `hmac_sha256_hex` |
| `vectors` | `parse_vector`, `dot`, `cosine`, `nearest` |

A library of your own is a folder whose top idea has `- Exports:`; its examples are calls.

## Running and deploying

- `lengua-run <folder>` runs a checked program; a server listens on its port, or on
  `--listen 0.0.0.0:<port>`.
- A container: the repository's `deploy/Dockerfile` checks the folder in its first stage and
  serves it with `lengua-run` in the second.
- Vercel: the front end's static export natively, the back end as that container, `/api/` sent
  to it.
- A desktop app (L10): `lengua-run --desktop <folder>`.
