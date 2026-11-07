# Checking a lengua program

Every lengua idea names this page as its `attester.resource`: deterministic code, with no
language model, that says whether a folder's computation is the one its idea sanctions
([OKF v0.2 §10](https://github.com/GoogleCloudPlatform/open-knowledge-format)).

## Check it

```sh
lengua-mcp check <folder>
```

It reads the folder's `idea.md` and `main.wat`, then:

1. builds `main.wat` into its WebAssembly module, as [`run`](run.md) does;
2. checks the module against every rule of lengua's WebAssembly dialect;
3. runs every test the idea holds, its examples and its generated tests, and compares each run's
   receipt with what the idea says;
4. compares the hashes in lengua's entry in the idea's `verified` list with the files: `idea`, the
   idea's SHA-256 with its `verified` and `generated` keys left out, and `code`, the SHA-256 of
   `main.wat`.

It writes nothing, and needs no lock file and no key, so anyone with the folder can check it on
their own computer. It prints one line for each thing it finds, and exits 0 when the folder is
attested and 1 when it isn't.

## lengua's entry in `verified`

When lengua's `check` tool takes a program to done, it adds its entry to the idea's `verified`
list, as OKF §5.2 defines it:

```yaml
verified:
  - { by: lengua/0.1.0, at: 2026-11-06T23:00:00Z, idea: "sha256:…", code: "sha256:…" }
```

It changes no other entry and no other line of the file. When the idea or `main.wat` changes, the
entry is removed until the program is done again, so a consumer reads "machine-confirmed" (OKF
§5.3) only for the files lengua checked.
