---
type: lengua Crate
title: lengua-seal
description: "The one crate that loads sealed precompiled code, with lengua's one unsafe call."
---

# lengua-seal

## Purpose

`lengua-seal` loads sealed precompiled code into Wasmtime. It is the one place in lengua's own
code where `unsafe` is allowed, and it has exactly one `unsafe` call:
`wasmtime::component::Component::deserialize`.

Why it is `unsafe`: Wasmtime runs precompiled code as machine code, as it is, with no check of
what it does. Bytes that Wasmtime did not write could do anything. So Wasmtime marks every way
to load precompiled code `unsafe` (`Component::deserialize`, `deserialize_raw`,
`deserialize_file`), and its only safe ways to make a component (`Component::new`,
`from_binary`, `from_file`) compile it, which needs Cranelift. The lean runner `lengua-run`
has no compiler, so there is no safe way for it, and `lengua-mcp` loads the same sealed code
so that the tests and `run` run the bytes the lock signed. Checked on docs.wasmtime.dev for
Wasmtime 50.0.1 (2026-11-13).

Why it is sound here: `load` takes a `lengua_idea::lock::Sealed`, and only
`lengua_idea::lock::verify` makes one, after it has checked the lock's HMAC-SHA256 under the
user's key and the SHA-256 of the precompiled file against the lock. The bytes it loads are the
bytes it hashed, read once into memory (not mapped from the file, so the file can't change
between the check and the load). The precompiled file was written by `lengua-mcp`'s own
`Component::serialize` on this computer, and Wasmtime itself refuses code made by another
Wasmtime version or for another CPU.

Every other crate of lengua forbids `unsafe` (`unsafe_code = "forbid"` in the workspace's
lints); this crate denies it, and allows it on `load` alone. Its test
`lengua_seal_has_the_one_unsafe_call_of_lengua` holds that: there is one `unsafe` block in all
of lengua's crates, here, and one manifest without the workspace's lints, this one.

## Behaviours

- Given a `Sealed` from a verified lock, when `load` runs, then it gives the component, or, when
  Wasmtime refuses the code (another Wasmtime version, another CPU, other settings), a message
  that says to run `check`, which seals it again on this computer.

## Structure

- `load`: the one `unsafe` call, `Component::deserialize` of the verified bytes.
- `tests/one.rs`: counts the `unsafe` blocks of every crate's `src/` and the manifests that do
  not take the workspace's lints.
