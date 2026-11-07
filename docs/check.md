# Checking a lengua program

Every lengua idea names this page as its `attester.resource`: deterministic code, with no
language model, that says whether a folder's computation is the one its idea sanctions
([OKF v0.2 §10](https://github.com/GoogleCloudPlatform/open-knowledge-format)).

## Check it

```sh
lengua-mcp check <folder>
```

It reads the folder's `idea.md` and `main.wat`, then:

1. compares the hashes in lengua's entry in the idea's `verified` list with the files: `idea`,
   the idea's SHA-256 with its `verified` and `generated` keys left out, and `code`, the SHA-256
   of `main.wat`;
2. builds `main.wat` into its WebAssembly module, as [`run`](run.md) does;
3. checks the module against every rule of lengua's WebAssembly dialect;
4. runs every test the idea holds, its examples and its generated tests, and compares each run's
   receipt with what the idea says;
5. proves each generated test, as lengua's `check` tool does, within five minutes for all of
   them (`LENGUA_PROOF_SECONDS` sets another time): for every input, up to a length, or not, and
   why. Inputs a proof finds where `main.wat` is wrong are run for real before it says so. The
   memory line says what the rules settle for every input and what the proofs settle of the
   rest.

It writes nothing, and needs no lock file and no key, so anyone with the folder can check it on
their own computer. It prints one line for each thing it finds, then `attested` or `not attested`,
and exits 0 when the folder is attested and 1 when it isn't. A folder is attested when its idea
and its `main.wat` pass every step, no proof finds inputs where `main.wat` is wrong, and lengua's
entry names them both. A proof that doesn't finish, or that the prover can't make, leaves the
folder attested and says "tested, not proven" with the reason. The proofs run only for a folder
that passes every other step.

## lengua's entry in `verified`

When lengua's `check` tool takes a program to done, it adds its entry to the idea's `verified`
list, as OKF §5.2 defines it:

```yaml
verified:
  - { by: lengua/0.2.0, at: 2026-11-06T23:00:00Z, idea: "sha256:…", code: "sha256:…" }
```

It changes no other entry and no other line of the file. When the idea or `main.wat` changes, the
next `check` removes the entry until the program is done again, and when one of its proofs, which
can go on after done, finds inputs where `main.wat` is wrong, it removes the entry at once. So a
consumer reads "machine-confirmed" (OKF §5.3) only for the files lengua checked.
