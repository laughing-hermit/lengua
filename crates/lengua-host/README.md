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

## Structure

- `config`: the engine's `wasmtime::Config`: epoch interruption on, relaxed SIMD and
  shared-everything threads off. `lengua-mcp` adds NaN canonicalization, a compiler setting
  that a precompiled component keeps.
- `Host::new` makes the engine and its epoch thread (every `TICK_MS`, 10 ms); `Host::engine`
  gives it; `Host::run` and `Host::execute` run a component: `link` adds one interface of the
  world to the linker, `context` makes the WASI context from the grants, the input and the
  `Streams`, `Limits` (with `limiter`) holds the memory, `Steps` and `Fixed` are the stepped and
  the fixed clock, `seeded` the fixed random bytes, `Guest` and `Table` the store's data, and
  `ending` reads how the run ended.
- `Input`, `Ending`, `Outcome` (with `Outcome::refused`), `Streams` and `OUTPUT` are its input
  and result.
