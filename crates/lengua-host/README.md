---
type: lengua Crate
title: lengua-host
description: "The host that runs a lengua program's component on Wasmtime under exactly the grants of its idea."
---

# lengua-host

## Purpose

`lengua-host` runs a lengua program's WebAssembly component on Wasmtime under exactly the grants
of its idea. It links only the interfaces of the program's world, gives the arguments, the
standard input, the clocks, the random numbers and the folders only as the grants allow, and
holds the idea's memory and time. It is shared by `lengua-mcp` (its `run` tool and the tests of
`check`, in a worker process) and by the lean runner `lengua-run`. It depends on Wasmtime with
the `runtime`, `std` and `component-model` features only, so it has no compiler: it runs a
component that `lengua-seal` loaded from sealed precompiled code. The grants and the host are in
[Lengua3.md](../../Lengua3.md), section 4, "The host".

## Behaviours

- Given a component, its build and an input, when `Host::run` runs it with `Streams::Memory`,
  then standard output and standard error are kept in memory (16 MiB each) and returned in the
  `Outcome`, with how the run ended: `Exited` with the exit code, `OutOfTime`, `OutOfMemory`,
  `Trapped` or `Refused`.
- Given `Streams::Inherit`, when the program runs, then its standard output and standard error
  are the process's own, and its standard input is the process's own only when the idea has
  `- Reads: standard input` (otherwise empty).
- Given the idea's `- Time:`, when the program computes past it, then the epoch stops it
  (`OutOfTime`); a program that waits in a host call is stopped by its process's parent, or by
  `lengua-run`'s own watch, not here.
- Given the idea's `- Memory:`, when the program grows its memory past it, then the run ends
  `OutOfMemory`.
- Given `fixed`, when the program reads the clocks or random numbers, then it gets the fixed
  wall clock, the stepped monotonic clock and random bytes from a fixed seed.

- Given a program with trusted components (`Program::trusted`), when it runs, then the host
  instantiates each component first, in the same store and under the same grants, and links
  every function of every interface it exports into the program's imports (since L7).
- Given `- Connects to:` or `- Listens on:`, when the program uses `wasi:sockets`, then TCP is on,
  UDP is off, the host resolves each granted `host:port` when the run starts, and its socket
  address check permits a connection only to those addresses and a bind or a listen only on the
  granted ports of `127.0.0.1` or of any local address (since L7).
- Given a server (`- Serves: HTTP on TCP port N`), when `Host::run` gets a request on its
  standard input, then the host reads it, gives it to the component's `wasi:http/incoming-handler`
  through `wasmtime-wasi-http` (its synchronous bindings, with lengua's own hooks that refuse
  every outgoing request the idea does not grant), and returns the answer as HTTP/1.1 bytes with a
  `content-length`;
  `Host::serve` does the same for each connection on `127.0.0.1:N` or a given address, eight at a
  time, a request that may write alone, with one line of log a request (since L7; L9).
- Given `- Asks: an AI model`, when the program sends `POST /ask`, `/stream` or `/embed` to
  `ai.lengua`, then the host answers from the example's `- Model answers:` in a fixed run, or from
  the model server of `LENGUA_MODEL_URL` in a live one, within the calls and tokens of the grant
  (429 past them) (since L9).
- Given ``- Calls: HTTP at `host:port` ``, when the program sends a request there, then the host
  sends it over TCP and streams the answer back; any other address is denied (since L9).
- Given ``- Reads: the secret `NAME` ``, when the program reads its environment, then it finds
  `NAME` with the host's value, or `test-NAME` in a fixed run (since L9).
## Structure

- `config`: the engine's `wasmtime::Config`: epoch interruption on, relaxed SIMD and
  shared-everything threads off. `lengua-mcp` adds NaN canonicalization, a compiler setting
  that a precompiled component keeps.
- `Host::new` makes the engine and its epoch thread (every `TICK_MS`, 10 ms); `Host::engine`
  gives it; `Host::run` and `Host::execute` run a component: `link` adds one interface of the
  world to the linker, `context` makes the WASI context from the grants, the input and the
  `Streams`, `Limits` (with `limiter`) holds the memory, `Steps` and `Fixed` are the stepped and
  the fixed clock, `seeded` the fixed random bytes, `Guest` and `Table` the store's data, and
  `ending` reads how the run ended; `hooks` makes the hooks of the grants and `secrets` gives
  the granted environment variables.
- `Input`, `Ending`, `Outcome` (with `Outcome::refused`), `Streams` and `OUTPUT` are its input
  and result.
- `Program` is what runs: the folder, the component, the trusted components and the build;
  `prepare` makes the linker and the store, `link_trusted` links the trusted components,
  `link_sockets` adds a `wasi:sockets` or `wasi:http` interface, `streams_of` gives the
  standard streams, `trap_ending` reads a trap, and `exported_interfaces` names the interfaces a
  component exports.
- `network.rs`: `allow` sets TCP, UDP, name lookup and the socket address check from the grants;
  `granted_addresses` resolves the granted hosts; `permitted` decides each use of an address.
- `http.rs`: `Whole` is a request body in
  memory; `Unpark` wakes the thread that reads the answer's body; `parse`, `head_end`,
  `content_length`, `chunked` and `dechunk` read a request; `collect` reads the answer's body;
  `answer_bytes` and `response_bytes` write the answer; `Host::answer` and `Host::handle` serve
  one request; `Host::serve`, `Host::serve_one`, `request_line`, `log_line` and `read_request`
  serve a TCP port, `WORKERS` at a time.
- `outgoing.rs`: `Hooks` are the hooks of the granted calls; `send` sends a request over TCP;
  `Pieces`, `whole`, `done` and `reply` make an answer; `authority_of` gives `host:port`.
- `model.rs`: `ask` answers the address `ai.lengua`; `Budget` holds the calls and tokens left;
  `Model` and `Endpoint` are the given answer or the model server; `given`, `live`,
  `request_body` and `finish` make the answer; `Events` and `event_text` read a streamed one.
- `json.rs`: `parse`, `quote` and `Value` read and write the JSON of a model server.
