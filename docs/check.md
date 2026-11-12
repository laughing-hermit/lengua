# Checking a lengua program

Every lengua idea names this page as its `attester.resource`: deterministic code, with no
language model, that says whether a folder's computation is the one its idea sanctions
([OKF v0.2 §10](https://github.com/GoogleCloudPlatform/open-knowledge-format)).

## Check it

```sh
lengua-mcp check <program folder>
```

It takes one program folder, its path absolute or from the current folder, and needs no data
folder, no lock and no key, so anyone with the folder can check it on their own computer. It
reads the folder's `idea.md` and `main.wat`, then, in order:

1. compares lengua's last entry in the idea's `verified` list with the files: `idea`, lengua's
   SHA-256 of `idea.md` with its `verified` and `generated` keys left out, and `code`, the
   SHA-256 of `main.wat`;
2. reads the idea as `check` does: it must parse, be an Attested Computation with no
   `- Threads:` line, and hold tests that check;
3. builds `main.wat` into its WebAssembly module, as [`run`](run.md) does;
4. checks the module against every rule of lengua's WebAssembly dialect and, for an idea with a
   `- Secret:` line, the secret rule: no value computed from an input the line names decides a
   branch, an `if`, a `br_if`, a `br_table` or a `select`, and so a loop's count; nor an address,
   an index or a length of a load, a store, a copy or a fill; nor a division, a remainder or a
   float operation; nor reaches a host call but as the bytes it writes out. The rule can't tell
   inputs of one kind apart: one secret file makes every file the program reads secret, and so
   does one variable for the environment and one argument for the arguments. A secret the
   program writes to a file or sends to a peer makes what it reads from every file and
   connection secret, since the secret may come back. Then it compiles the module for this
   computer's CPU, x86-64 or ARM64, and reads the machine code the engine made for each operator
   that takes a secret: a conditional branch, a jump or a call through a register, a call of one
   of the engine's functions, a division, the address of a table in the code, or bytes it can't
   read is refused on the operator's line of `main.wat`, and a line of the output says which
   CPU's code was read. In a function that computes with a secret, the code the engine gives
   to no operator is read the same way, since its optimizer makes some operators anew. It
   trusts the CPU's own timing of the instructions left, the code of functions and operators
   that take no secret, and the engine's and the host's own functions;
5. runs every test the idea holds, its examples and its generated tests, in workers of its own,
   the generated tests' inputs drawn from a fresh seed, and compares each run's standard output,
   standard error, exit code and working folder with what the idea says;
6. proves each generated test, as lengua's `check` tool does, but only when every step above
   passed and lengua's entry names both files: the smallest first, each with its share of five
   minutes for them all (`LENGUA_PROOF_SECONDS` sets another time). Before a proof, the model of
   the module must agree with the real program on up to 24 of the test's own inputs, and inputs
   a proof finds where `main.wat` is wrong are run for real before it says so. It doesn't prove
   on from where a proof stopped: that is the `check` tool's, which keeps a lock.

For a part, an idea with a `# Contract` section, it holds the section to `main.wat`: each
function the section offers, with what the memory rules find it reads and writes through the
addresses it is lent. A section that says anything else, or offers a function no program
could take, leaves the folder not attested, and each function that holds is a line of the
output, `idea.md: its contract holds: …`.

It doesn't hold `main.wat` to the parts its idea's `- Builds on:` line names, as the `check` tool
does: it reads one folder, and a part is checked on the computer that builds on it. What it
attests of a program built on parts is the program itself, the functions it took included.

It prints one line for each thing it finds, then `attested` or `not attested`, and exits 0 when
the folder is attested and 1 when it isn't:

```text
verified: lengua/0.2.0 checked this idea.md and this main.wat at …
main.wat: builds into a module of … bytes, sha256:…
main.wat: follows every rule of lengua's WebAssembly dialect
tests: passed all … test runs, the generated ones drawn from seed …
proofs: generated B1: …
memory: the rules (M1 to M8) hold every address they can follow to its owner, …
attested
```

A folder is attested when its idea and its `main.wat` pass every step, no proof finds inputs
where `main.wat` is wrong, and lengua's entry names them both. A proof that doesn't finish, or
that the prover can't make, leaves the folder attested, and its line says "tested, not proven"
with the reason. The memory line says what the rules settle for every input, and what the
proofs settle of the rest.

It writes nothing in the folder or in lengua's data folder: no lock, no `index.md`, no `log.md`
and no compiled modules. While it runs, it keeps the module and each test's working folder in
temporary folders, which go when it ends.

## An outside module is never attested

An idea whose frontmatter says `computation: main.wasm`, and whose `# Constraints` has a line
`- Module: main.wasm, built from …`, names an outside module as its program: a WebAssembly
module that a compiler made from Rust, C or another language. lengua runs it under the idea's
grants ([run.md](run.md)) and marks it "trusted, not proven". This page's steps 3, 4 and 6 need
`main.wat`, so no outside module passes them, and `lengua-mcp check` says so and stops:

```text
main.wasm: an outside module, which is trusted, not proven: lengua attests an idea and its main.wat, by its rules and its proofs, and holds main.wasm to neither
not attested
```

lengua's `check` tool takes such a folder as far as its tests. In place of steps 3 to 6 it:

1. reads `main.wasm`, a regular file of at most 16 MiB, and refuses, with the reason and what to
   build differently: a module the engine's validator doesn't take with exactly the engine's
   features (no threads, tail calls, exceptions, 64-bit or multiple memories, GC, relaxed SIMD,
   wide arithmetic or components); an import that isn't a function of WASI preview 1 or of
   lengua's own modules with its own type; a function of lengua's own modules that the idea
   doesn't grant; a memory that isn't exported as `memory`, declares no maximum, or may hold
   more than 16,384 pages (1 GiB); and a module with no `_start` that takes and gives nothing.
   `main.wat` beside it is one code file too many, and so is a `# Contract` section, since an
   outside module offers no function to another program. An idea with a `- Secret:` line or a
   `- Builds on:` line is refused at the idea;
2. runs every test the idea holds against the module, in its workers, as step 5 above;
3. stamps the module in `lengua.lock`: the stamp's two hashes, of the code file and of the
   module, are one, and its `engine` line ends with `; an outside module: trusted, not proven`.

It checks no rule of the dialect, proves nothing, and writes no entry in the idea's `verified`
list. Its reply's `state` is `trusted_not_proven` where `main.wat` gets `done`, and is never
`proving` or `done`. From the first reply about the module on, the reply's `outside` list says:

- first, `trusted, not proven`, and what the idea says the module was built from, which lengua
  can't check;
- a line `not held: …` for main.wat's rules (the memory rules M1 to M8), for bounded loops and
  no recursion, for the secret rule, and for assertions and the other rules of the Power of 10;
- `not proven: nothing…`: a generated test shows only the runs it made;
- `not attested: …`;
- `held, by lengua's host: …`: it reaches only what its idea grants, its memory stays its own
  within the pages it declares, its calls stay within the engine's stack, and what it prints
  and writes has the limits of any program's;
- the functions of WASI preview 1 it imports that its idea doesn't grant, which the host
  refuses at each call.

Each line of `proofs` says `not proven`, and the reply has no `memory` line. The bundle's
`log.md` records `checked: trusted, not proven`. The threat cases an outside module was reviewed
against, with the test of each, are in
[docs/reviews/outside-2026-11-12.md](reviews/outside-2026-11-12.md).

## lengua's entry in `verified`

When lengua's `check` tool takes a program to done, it adds its entry to the idea's `verified`
list, as OKF §5.2 defines it:

```yaml
verified:
  - { by: lengua/0.2.0, at: 2026-11-06T23:00:00Z, idea: "sha256:…", code: "sha256:…" }
```

The entry goes last in the list. lengua's earlier entries go, and everyone else's stay. A
`verified` written as a flow list is rewritten one entry a line, with its comments; a single
mapping becomes the list's first entry; with no `verified` key, the key goes just before the
frontmatter's closing `---`. Every line outside `verified` stays byte for byte, and lengua's
hash of the idea leaves `verified` and `generated` out, so writing the entry changes no lock.

When the idea or `main.wat` changes, the next `check` removes lengua's entry until the program
is done again. When one of its proofs, which can go on after done, finds inputs where `main.wat`
is wrong, lengua removes the entry at once. So a consumer reads "machine-confirmed" (OKF §5.3)
only for the files lengua checked.
