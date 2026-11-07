# lengua's host: what a WebAssembly program may call

A `main.wat` program reaches the computer only through WASI preview 1's functions
(`wasi_snapshot_preview1`), and only those its idea allows. lengua implements them itself, the
same way on every computer, so a program does the same thing everywhere. `check` refuses any other
import (rule 3 of [the WebAssembly dialect](power-of-10-webassembly.md)).

## What each idea line allows

| The idea says | The program may import |
|---|---|
| (always) | `args_get`, `args_sizes_get`, `fd_write` (to descriptors 1 and 2), `fd_fdstat_get`, `proc_exit`, `sched_yield` |
| `- Uses: standard input` | `fd_read` (descriptor 0) |
| `- Uses: the environment` | `environ_get`, `environ_sizes_get` |
| `- Uses: files` | `fd_prestat_get`, `fd_prestat_dir_name` (descriptor 3), `path_open`, `fd_read`, `fd_write`, `fd_seek`, `fd_tell`, `fd_close`, `fd_filestat_get`, `path_filestat_get`, `fd_readdir`, `path_create_directory`, `path_remove_directory`, `path_unlink_file`, `path_rename` |
| `- Uses: the clock` | `clock_time_get`, `clock_res_get`, `poll_oneoff` (clock waits) |
| `- Uses: random bytes` | `random_get` |
| `- Network: …` | `sock_accept` (descriptor 4), `sock_recv`, `sock_send`, `sock_shutdown`, `poll_oneoff` (waits on descriptors) |

## Descriptors

| Descriptor | What it is |
|---|---|
| 0, 1, 2 | standard input, output and error |
| 3 | the working folder, preopened as `.` (only with `- Uses: files`) |
| 4 | the listener, for a program that serves (`- Network: listens on …`) |
| 5 and up | the connections lengua made for the program, in the order its idea names them |

A descriptor not in use gives `BADF`. A program never opens a connection or a listener itself:
lengua makes what the idea names before the program starts, so a program reaches only those.
Files are names inside the working folder; a name that would leave it, with `..` or from `/`,
gives `NOTCAPABLE`, and a link, which no name may lead through, gives `ACCES`.

## What a call borrows

A call reads or writes only the memory its arguments name, and every byte of it must lie inside
one of the program's regions (M2): an iovec array and each buffer it points to, a path's bytes, and
the place where a call writes a count, a descriptor or a time. A buffer a call writes may not
overlap another argument it reads (M4).

## Errors

Every call returns 0 on success or one of these numbers, and the program tests it before using
anything the call wrote (rule 7):

| Name | Number | | Name | Number |
|---|---|---|---|---|
| ACCES | 2 | | NAMETOOLONG | 37 |
| AGAIN | 6 | | NOENT | 44 |
| BADF | 8 | | NOSYS | 52 |
| BUSY | 10 | | NOTDIR | 54 |
| EXIST | 20 | | NOTEMPTY | 55 |
| FBIG | 22 | | NOTSOCK | 57 |
| ILSEQ | 25 | | NOTTY | 59 |
| INVAL | 28 | | PIPE | 64 |
| IO | 29 | | SPIPE | 70 |
| ISDIR | 31 | | NOTCAPABLE | 76 |
| MFILE | 33 | | | |

A closed standard output gives `PIPE`; a file past its limit gives `FBIG`, and so does a write, a
new file or a new folder past the total a test's program may write, in which each file and folder
counts as 4,096 bytes and a write past a file's end counts the gap it leaves; a name another
program holds gives `BUSY`.

## The clock and random bytes

The clock reads to the microsecond. In tests, the idea's tests supply the clock and the random
bytes, and a wait ends at once, so the output is exact.

## lengua's library

`check` merges these into every module; call them, never define them:
- `$assert_fail (param i32)`: writes `lengua: assertion NNN failed` to standard error and exits
  with code 134 (rule 5).
- `$fill (param $fd i32) (param $at i32) (param $room i32) (param $stop i32) (result i32)`: reads
  from `$fd` into `$at` until `$room` bytes, the end of the input, or the byte `$stop` (-1 for
  none); returns the count, or a negative error.
- `$write_all (param $fd i32) (param $at i32) (param $len i32) (result i32)`: writes every byte,
  retrying a short write; returns 0, or an error number.
