# Host conformance cases

Each case runs one small WebAssembly module through lengua's WASI preview 1 host
(`src/wasm/host/`) and compares what it did with what the case expects. `tests/host.rs` runs every
case, on every runner. The cases were written from the host's function table alone, without
reading the host, so they check it independently.

## Layout

A case is two files with the same stem, in a folder named after the WASI function it checks:

```text
tests/host-cases/<wasi_function>/<case>.wat
tests/host-cases/<wasi_function>/<case>.toml
```

A case may call other functions too, to set up a file or to show a result.

## The module: `<case>.wat`

- A complete module in WebAssembly text, as the `wast` crate reads it.
- It imports only from `wasi_snapshot_preview1`, with preview 1's exact signatures, and only the
  functions its `uses` and `network` allow: the host refuses any other import.
- Its imports come before every other definition.
- It declares `(memory (export "memory") 1 1)` and exports `_start`.
- It ends by calling `proc_exit`.
- Everything the host reads or writes is at address 1024 or above.
- It names every function it calls and never uses an index, so lengua's library can be merged
  in. No name in it is one of the library's: `$assert_fail`, `$fill`, `$write_all`, or a name
  that starts with `$lengua`.

## The case: `<case>.toml`

```toml
about = "fd_read returns 0 at the end of standard input"
uses = ["standard input"]        # the idea's Uses words (idea.rs Use::from_name), may be empty
network = ""                     # an idea `- Network:` line's text after the colon, or empty
args = ["first"]                 # after the program's name
env = { NAME = "value" }         # only given when uses has "the environment"
stdin = "abc"                    # or stdin_hex = "616263"
files = { "in.txt" = "hello" }   # the working folder before the run (folders: "dir/" = "")
clock_ns = 1_700_000_000_000_000_000   # optional, supplied clock
random_hex = "00ff"              # optional, supplied random bytes
[expect]
exit = 0                         # the exit code
stdout = "abc"                   # or stdout_hex; exact
stderr = ""                      # exact
files = { "out.txt" = "hello" }  # optional: the folder after the run must contain exactly these
                                 # (omit to not check); "dir/" = "" for a folder
```

- `uses` takes the idea's words: `standard input`, `files`, `the clock`, `random bytes` and
  `the environment`.
- `network` is an idea's `- Network:` line after the colon. A case that needs a listener says
  `listens on TCP port 8080`; in a test lengua listens on a free port instead.
- `env` is given only when `uses` has `the environment`. The cases give every other key, empty
  when there is nothing to give.
- In `files`, a name that ends in `/` is a folder. Every folder is listed, also one that holds
  files, and a file in a folder is named by its path, such as `dir/a.txt`.
- `[expect] files`, when given, lists everything in the working folder after the run, folders
  included. When it is left out, the folder isn't checked.
- Every case runs as a test: the clock and random bytes come from `clock_ns` and `random_hex`
  when they are given, sleeps end at once, and no case's output depends on the order of a
  folder's listing, so a second run with listings reversed gives the same output.

## Exit codes

The exit code says what happened, so every expectation can be worked out by hand.

| Code | Meaning |
|---|---|
| 0 | A success case did everything. |
| 1 to 76 | An error case: the errno the call under test returned, in WASI's numbering. |
| 99 | A loop that should have ended didn't. |
| 102 | A write of the case's own output took no bytes. |
| 103 | A call said it moved more bytes than it was given room for. |
| 150 + errno | A step that must succeed failed with that errno. |

A success case prints short, fixed text, with numbers in decimal.

WASI's numbers for the errors the host gives: ACCES 2, AGAIN 6, BADF 8, BUSY 10, EXIST 20,
FBIG 22, ILSEQ 25, INVAL 28, IO 29, ISDIR 31, MFILE 33, NAMETOOLONG 37, NOENT 44, NOSYS 52,
NOTDIR 54, NOTEMPTY 55, NOTSOCK 57, NOTTY 59, PIPE 64, SPIPE 70, NOTCAPABLE 76.

## Memory

Every module uses the same places in its one page:

| Address | Holds |
|---|---|
| 1024 | the iovec `$put` builds, and at 1032 the count it gets back |
| 1040 | the iovec `$read` builds |
| 1056 | the descriptor `$open` gets |
| 1064, 1068 | counts a call returns: bytes, entries, sizes, events |
| 1072, 1080 | 64-bit results: offsets and times |
| 1088 | the digits `$say_num` writes |
| 1112, 1116, 1120 | the counts `$list` keeps |
| 1152 | a struct a call fills: fdstat, filestat or prestat |
| 1216 | the events `poll_oneoff` writes, 32 bytes each |
| 1280 | the subscriptions given to `poll_oneoff`, 48 bytes each |
| 2040 | a space and a newline |
| 2048 | the case's strings |
| 3072 | iovecs given as data |
| 4096 | buffers: pointers to arguments and variables, reads, listings, random bytes |
| 8192 | the strings `args_get` and `environ_get` write |

Before a call that should write a result, a case fills its place with something the call
shouldn't give (99, or bytes of 255, or of 170 where 255 is among the bytes expected), so a host
that writes nothing is caught.

## Helpers

A module holds the helpers it needs, each written the same way everywhere:

- `$need (errno)`: exits with 150 + errno when a step that must succeed fails.
- `$put (fd, at, len)`: writes all `len` bytes, in as many calls to `fd_write` as it takes, since
  a write may take fewer bytes than it was given.
- `$say (at, len)`: `$put` to standard output.
- `$say_num (n)`: writes the 64-bit number `n` in decimal.
- `$sp`, `$nl`: write a space or a newline.
- `$strlen (at)`, `$say_str (at)`: the length of a NUL-terminated string, and writing it.
- `$open (at, len, oflags, rights, fdflags)`: `path_open` beneath descriptor 3, the errno back,
  the new descriptor at 1056.
- `$read (fd, at, cap)`: one `fd_read` into one iovec, the errno back, the count at 1064.
- `$list (fd, cap)`: reads a folder's whole listing with a `cap`-byte buffer, following the
  cookies as wasi-libc does, and counts entries, files and folders.
- `$say_stat (at)`: writes a filestat's type, its size, and 1 if any other field is set or 0 if
  none is.

## What a case can't set up

Some errors in the host's closed set need a state no case file can describe, so other tests
check them: a read from standard input without `uses = ["standard input"]` (the host refuses the
import, which the checker reports), PIPE on a closed standard output, FBIG past the file limit,
the total cap on bytes written, links out of the folder, and anything on a connection, which
needs a peer.

## Flags

The flags the cases pass, from preview 1's definitions:

- Rights to read: `fd_read | fd_seek | fd_tell | fd_readdir | fd_filestat_get` = 2113574.
- Rights to write: `fd_write | fd_seek | fd_tell | fd_filestat_get` = 2097252.
- `oflags`: `creat` 1, `directory` 2, `excl` 4, `trunc` 8. `fdflags`: `append` 1.
- `whence`: `set` 0, `cur` 1, `end` 2. Clocks: `realtime` 0, `monotonic` 1.
- File types: `directory` 3, `regular_file` 4, `socket_stream` 6.
