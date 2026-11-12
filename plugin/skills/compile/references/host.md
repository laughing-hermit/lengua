# lengua's host: what a WebAssembly program may call

A `main.wat` program reaches the computer only through WASI preview 1's functions
(`wasi_snapshot_preview1`) and, for what it reaches by itself while it runs, lengua's own
(`lengua_net_1` for the network, `lengua_fs_1` for the files its arguments name,
`lengua_run_1` to start a program, `lengua_term_1` for the terminal, `lengua_call_1` for the
bytes of a module it calls, `lengua_split_1` for work it
splits into pieces that run at once), and only those its idea
allows. lengua implements them itself, the same way on
every computer, so a program does the same thing everywhere. `check` refuses any other import
(rule 3 of [the WebAssembly dialect](power-of-10-webassembly.md)).

## What each idea line allows

| The idea says | The program may import |
|---|---|
| (always) | `args_get`, `args_sizes_get`, `fd_write` (to descriptors 1 and 2), `fd_fdstat_get`, `proc_exit`, `sched_yield` |
| `- Uses: standard input` | `fd_read` (descriptor 0), `poll_oneoff` (a wait to read it) |
| `- Uses: the terminal` | what `standard input` allows, `poll_oneoff`'s waits on time, and from `lengua_term_1`: `term_is`, `term_size`, `term_raw` |
| `- Uses: the environment` | `environ_get`, `environ_sizes_get` |
| `- Uses: files` | `fd_prestat_get`, `fd_prestat_dir_name` (descriptor 3), `path_open`, `fd_read`, `fd_write`, `fd_seek`, `fd_tell`, `fd_close`, `fd_filestat_get`, `path_filestat_get`, `fd_readdir`, `path_create_directory`, `path_remove_directory`, `path_unlink_file`, `path_rename` |
| `- Uses: the clock` | `clock_time_get`, `clock_res_get`, `poll_oneoff` (clock waits) |
| `- Uses: random bytes` | `random_get` |
| `- Network: …` (but a line that only looks up addresses) | `sock_recv`, `sock_send`, `sock_shutdown`, and `fd_read`, `fd_write` and `fd_close` on a connection, `poll_oneoff` (waits on descriptors); and with `listens on …`, `sock_accept` (descriptor 4) |
| `- Network: opens TCP connections to …` | from `lengua_net_1`: `tcp_open` |
| `- Network: looks up the addresses of …` | from `lengua_net_1`: `dns_lookup` |
| `- Network: sends UDP datagrams to …` | from `lengua_net_1`: `udp_open`, `udp_send`, `udp_recv` |
| `- Network: receives UDP datagrams on …` | from `lengua_net_1`: `udp_recv_from`, `udp_send_to` |
| `- Files: reads the file its first argument names` (any `- Files:` line) | from `lengua_fs_1`: `file_open`; and `fd_read`, `fd_write`, `fd_seek`, `fd_tell`, `fd_filestat_get`, `fd_close` on what it opened |
| `- Files: reads the folder its first argument names` (a line that names a folder) | from `lengua_fs_1`: `folder_open`; and on the folder it gives, what `- Uses: files` allows on descriptor 3: `path_open`, `path_filestat_get`, `fd_readdir`, `path_create_directory`, `path_remove_directory`, `path_unlink_file`, `path_rename` |
| ``- Runs: `hostname` with no arguments`` (any `- Runs:` line) | from `lengua_run_1`: `program_run` |
| `- Splits: its work into up to 8 pieces, …` | from `lengua_split_1`: `split`, `piece` |
| `- Calls: crc32.wasm, built from …; its functions: …` (any `- Calls:` line) | from `lengua_call_1`: `give`, `take`; and from the called module, by its name, each function the line lists |

## Descriptors

| Descriptor | What it is |
|---|---|
| 0, 1, 2 | standard input, output and error |
| 3 | the working folder, preopened as `.` (only with `- Uses: files`) |
| 4 | the listener, for a program that serves (`- Network: listens on …`) |
| 5 and up | the connections lengua made for the program, in the order its idea names them |
| the next one | the UDP port the idea receives datagrams on (`- Network: receives UDP datagrams on …`): 5 when lengua made no connection |

A descriptor not in use gives `BADF`. A program whose idea says `connects to …` or `listens on …`
never opens a connection or a listener itself: lengua makes what the idea names before the
program starts, so a program reaches only those. What a program opens itself, a file, a
connection it accepts, or a connection or a datagram socket its idea grants it to open, takes the
lowest number free after the ones above: read it from where the call wrote it, never assume it.
Files are names inside the working folder; a name that would leave it, with `..` or from `/`,
gives `NOTCAPABLE`, and a link, which no name may lead through, gives `ACCES`. So does a file
with more than one name, a hard link: its other name may be outside the folder, so `path_open`
opens no such file, to read or to write. A program reports it as it does any file it can't open.

lengua's host passes each read of a file to the system once. On a local disk, a regular file's
read gives every byte it asks for, up to the file's end, as the proofs take it: a read that
gives fewer bytes than it asked for has reached the end, so a file shorter than a buffer takes
one `fd_read`. On a network or FUSE mount a read may give fewer before the end: a program that
must be right there reads until a read gives 0, as `$fill` does. Standard input and a connection
give what has arrived, which may be less before their end: read them through `$fill`.

## What a program opens itself: `lengua_net_1`

WASI preview 1 has no call that opens a connection, so lengua gives its own, in a module of
their own. Each takes a computer's name as text in memory, its address and its length, and gives
an error number as WASI's calls do. lengua reads the name, finds the line of the idea that grants
it, resolves it, and checks every address before it reaches anything: a program can't reach a
computer its idea doesn't grant, by any spelling, and never this computer.

| Import, with its type | What it does |
|---|---|
| `tcp_open (param i32 i32 i32 i32 i32) (result i32)`: name, its length, port, TLS, out | Opens a TCP connection to the port of that computer and writes its descriptor at `out`, 4 bytes. TLS is 0 for a plain connection and 1 for one over TLS, as the idea's line says: lengua does the TLS and checks the certificate, and the program reads and writes plain bytes |
| `dns_lookup (param i32 i32 i32 i32 i32) (result i32)`: name, its length, buffer, its length, out | Writes the name's addresses into the buffer, 17 bytes each: the family, 4 or 6, then 16 bytes, an IPv4 address in the first four. IPv4 comes first, each family in order. It writes as many as fit, at most 16, and how many at `out` |
| `udp_open (param i32 i32 i32 i32) (result i32)`: name, its length, port, out | A socket for datagrams to and from that port of that one computer; writes its descriptor at `out` |
| `udp_send (param i32 i32 i32 i32) (result i32)`: descriptor, data, its length, out | Sends the bytes as one datagram, at most 65,507, and writes how many went at `out` |
| `udp_recv (param i32 i32 i32 i32) (result i32)`: descriptor, buffer, its length, out | Waits for the next datagram from that computer, copies what fits, and writes the datagram's whole size at `out`: a size above the buffer's says it was cut |
| `udp_recv_from (param i32 i32 i32 i32 i32) (result i32)`: descriptor, buffer, its length, out, sender | On the port the idea receives on: the next datagram, as `udp_recv` gives it, and its sender, 19 bytes: an address as `dns_lookup` writes one, then its port, 2 bytes, low byte first |
| `udp_send_to (param i32 i32 i32 i32 i32) (result i32)`: descriptor, data, its length, sender, out | On the port the idea receives on: one datagram back to a sender, named by the 19 bytes `udp_recv_from` wrote |

- **A connection `tcp_open` gave is a connection like any other**: `$fill` and `$write_all`,
  `sock_recv`, `sock_send`, `sock_shutdown`, `poll_oneoff` and `fd_close` take it. Close each one
  when it is done: a program holds at most 64 connections and datagram sockets of its own at
  once, and the next `tcp_open` or `udp_open` then gives `MFILE`.
- **lengua ends a connection that no byte crosses for five minutes**: its next read gives the
  end, and its next write `PIPE`.
- **What the errors mean**: `NOTCAPABLE`, the idea grants no such computer, port or TLS, or the
  text names this computer, or it is an address of a private network the idea's line doesn't
  reach; `INVAL`, the text is no computer's name or address, or the port is none; `NAMETOOLONG`,
  more than 253 bytes; `NOENT`, the name has no address, or none lengua reaches: a name that
  resolves to this computer or into a private network reads as a name with no address, so a
  program can't learn which names a private network has, and the run's report says which it
  was; `CONNREFUSED`, the computer didn't take the connection, or its certificate didn't check;
  `TIMEDOUT`, no answer within 5 seconds; `MFILE`, as above. Give the user a different message
  for `NOTCAPABLE` than for a computer that is down: the first is the idea's, and no retry
  changes it.
- **The name is as the user or a server wrote it**: lengua takes capitals and a dot at the end,
  and an address in any standard writing. It refuses a number that only an older parsing reads
  as an address, such as `2130706433` or `127.1`, with `INVAL`.
- **A sender is answered only once it sent something**: `udp_send_to` to any other address
  gives `NOTCAPABLE`, and on a port any computer may send to, so does more than three times the
  bytes that address sent.
- **In a test** the computers are the test's own: a `tcp_open` to a computer the test gives no
  server gives `CONNREFUSED`, a datagram to one is lost and `udp_recv` then gives `CONNREFUSED`,
  and a name the test gives no addresses gives `NOENT`.

## Files the user names: `lengua_fs_1`

`path_open` opens a path beneath the working folder. A file or a folder the user names when the
program runs, anywhere on the computer, is opened by the place of the argument that names it,
never by a path: lengua reads the argument and opens what it names, so the program needs no
`args_get` for it and can't open a path it built.

| Import, with its type | What it does |
|---|---|
| `file_open (param i32 i32 i32) (result i32)`: argument, mode, out | Opens the file the argument at that place names, 1 for the first, and writes its descriptor at `out`, 4 bytes. The mode adds 1 to read, 2 to write, 4 to make the file when it isn't there, 8 to empty it, 16 to write at its end; 4, 8 and 16 need 2 |
| `folder_open (param i32 i32) (result i32)`: argument, out | Opens the folder the argument at that place names and writes its descriptor at `out`: the `path_` functions and `fd_readdir` take it as they take descriptor 3 |

- **Name the mode**, as any mode: `(global $READ i32 (i32.const 1))`, or `$MAKE_AND_EMPTY` for
  14. The idea's line says which: a file granted with `reads` opens with 1 alone.
- **What the errors mean**: `NOTCAPABLE`, the idea has no line for that argument, or not for
  that mode, or lengua won't open the place: one of its own folders, the system's (`/proc`,
  `/sys`, `/dev`, a device), or, in a test, a path that leaves the test's folder; `INVAL`, the
  program has no such argument, or the mode is none: show the usage line; `NOENT`, no such
  file, or a folder on the way isn't there; `ISDIR`, it is a folder; `ACCES`, it is a link, a
  file with a second name, or nothing a program opens; `EXIST`, a new name that differs from
  one in its folder only in case; `ILSEQ`, `NAMETOOLONG`, a last name not every system takes.
  Give the user one message for a file that can't be opened and another for a missing argument.
- **A file `file_open` gave is a file like any other**: `$fill`, `$write_all`, `fd_seek`,
  `fd_filestat_get` and `fd_close` take it. Close it when it is done.
- **A folder granted for reading takes no change**: `path_open` to write or with `CREAT`,
  `path_create_directory`, `path_remove_directory`, `path_unlink_file` and `path_rename` give
  `NOTCAPABLE`. Nothing is renamed from one folder the program was given into another.

`files-template.wat` is a whole program: it never reads its arguments' text.

## Starting a program: `lengua_run_1`

| Import, with its type | What it does |
|---|---|
| `program_run (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)`: command, its length, input, its length, output, its length, out_size, out_code | Starts the program the command names with its arguments, gives it the input as its standard input, waits until it ends, copies what it printed to `output`, and writes how many bytes that is at `out_size` and its exit code at `out_code`, 4 bytes each |

- **The command** is the program's name and then each argument, each ended by a NUL byte:
  `"hostname\00"`, or `"tar\00-tf\00a.tar\00"`. Keep a fixed command in a read-only region; build
  one with a value in a writable region, each piece with its NUL. The idea's `- Runs:` lines
  say which commands start: every other gives `NOTCAPABLE`, and so does a value that starts
  with `-` or `/`.
- **The call returns when the started program has ended.** It gets nothing but its input: no
  environment, an empty folder of its own, no standard error.
- **What the errors mean**: `NOTCAPABLE`, no line of the idea covers the command, or lengua
  won't start the file it found; `NOENT`, the computer has no such program, which in a test is
  a command no step gives; `TIMEDOUT`, it ran past its line's seconds and lengua ended it;
  `FBIG`, it printed more than the output's length and lengua ended it, with the buffer full;
  `INVAL`, the command isn't a name and arguments each ended by a NUL; `ILSEQ`, it isn't text;
  `IO`, the system didn't start it.
- **Test the error, then the exit code, then use the size**: a program that started and failed
  returns 0 from `program_run` and a code other than 0 at `out_code`. Check the size against
  the output's region before a write of it, as after `$fill`.

`program-template.wat` is a whole program.

## Work split into pieces: `lengua_split_1`

| Import, with its type | What it does |
|---|---|
| `split (param i32 i32 i32 i32 i32) (result i32)`: input, its length, pieces, output, its length | Cuts the input into `pieces` pieces that follow each other and differ in length by a byte at most, runs each, waits for all, and writes what each printed at the start of its share of `output` (its length divided by `pieces`), in the pieces' order, with zeros after it |
| `piece (result i32)` | 0 in a run that is a piece of the work, `NOENT` (44) in the run that is the whole program |

- **A piece is one more run of this same main.wat, from `_start`**, with a memory of its own
  as the module declares it. Its standard input is its share of the bytes, and what it prints
  to standard output is its answer. It reaches nothing else: every other call answers
  `NOTCAPABLE`, `split` too. So `_start` calls `piece` first. Where it gives 0, the run reads
  standard input with `$fill`, does the work and prints the answer with `$write_all`; where
  it gives `NOENT`, the run is the whole program, which reads its input, calls `split`, and
  joins the answers.
- **The pieces run at once**, on as many cores as the computer has, and the result is the
  same on every computer: the answers are in the pieces' order, whichever ended first. Write
  the join so that it needs nothing else, such as a sum of counts. Where a piece's work
  depends on the bytes before it (a word that crosses the cut), the whole program has every
  byte in its own memory and corrects for the cut after the join.
- **Hold the call to the idea's line**, `- Splits: its work into up to 8 pieces, each of up
  to 1048576 bytes, each gives back 8192 bytes`: give `pieces` as a named constant of 1 to
  the line's pieces (`check` refuses any other), the input's length at most `pieces` times
  the line's bytes, and the output's length exactly `pieces` times the bytes each gives back.
  A call the line doesn't cover answers `INVAL`.
- **A piece prints at most the bytes the line gives back**, and exits with code 0. A piece
  that prints more, exits with another code, traps or fails an assertion ends the whole run
  as it ended, and nothing is written to `output`: test the error `split` returns, and expect
  none.
- **Size the memory for every run at once**: the module's pages, times the line's pieces and
  one more for the program, are at most 16,384. Read a large input a buffer-full at a time,
  and split each buffer-full.
- **Give a piece a small buffer of its own**, as `examples/crc32c-split` does: the whole
  program reads megabytes at a time into a large region and splits them, and a piece reads its
  share 64 KiB at a time into a small one, which stays in a core's cache. A program that reads
  in two places is proven by length, not by induction.
- **A split is not faster by itself.** Each piece is a new run that reads its bytes out of the
  program's memory. Measured on the runners, with a piece for each of 3 or 4 cores, a count and
  a CRC split in eight took 0.58 to 0.94 times the one-core program's time on 256 MiB, and on
  64 MiB between 0.74 and 1.13 times: slower than one core on two of five systems. Split work
  of hundreds of megabytes, or work that is heavy for each byte, megabytes at a time, and tell
  the user what the split bought only after a run has shown it.

```wat
(import "lengua_split_1" "split" (func $split (param i32 i32 i32 i32 i32) (result i32)))
(import "lengua_split_1" "piece" (func $piece (result i32)))
;; What `piece` answers in the run that is the whole program: NOENT.
(global $NOT_A_PIECE i32 (i32.const 44))
(global $PIECES i32 (i32.const 8))
…
    ;; First in _start: 0 in a piece, NOT_A_PIECE in the whole program.
    (local.set $whole (call $piece))
    (if (local.get $whole)
      (then
        (if (i32.ne (local.get $whole) (global.get $NOT_A_PIECE))
          (then (call $assert_fail (i32.const 1)) unreachable))))
…
    ;; In the whole program, for the $len bytes read into the buffer:
    (local.set $err
      (call $split (global.get $buffer) (local.get $len) (global.get $PIECES)
        (global.get $answers) (global.get $answers.size)))
    (if (local.get $err)
      (then (call $assert_fail (i32.const 2)) unreachable))
```

## A called module's bytes: `lengua_call_1`

| Import, with its type | What it does |
|---|---|
| `give (param i32 i32 i32 i32) (result i32)`: module, address there, address here, length | Copies the bytes at the address in this program's memory to the address in the called module's memory |
| `take (param i32 i32 i32 i32) (result i32)`: module, address there, address here, length | Copies the bytes at the address in the called module's memory to the address in this program's memory |

- **A called module is another instance, with another memory.** An idea's `- Calls:` line names
  it and its functions, and `main.wat` imports each function from the module named as the
  called module is: `(import "crc32" "crc32" (func $crc32 (param i32) (result i32)))`. A call
  passes numbers and gets a number back. Nothing else crosses but these two copies.
- **`module` counts the idea's `- Calls:` lines from 0.** Name it with a constant.
- **Ask the called module for the address there**, with one of its functions; it is a number
  to this program, not an address of its own memory.
- **Test the error each copy returns**: `INVAL` (28) when the idea calls no such module, the
  module has no memory, or the bytes there are not inside its memory, and then nothing is
  copied. `NOTCAPABLE` for an idea with no `- Calls:` line.
- **The called module reaches nothing of the host**: no file, no network, no clock, no output.
  A call it makes to the host ends the run, as a trap in it does.

[Calls between modules](calls.md) has the whole of it.

## The terminal: `lengua_term_1`

| Import, with its type | What it does |
|---|---|
| `term_is (param i32 i32) (result i32)`: descriptor, out | Writes 1 at `out` when descriptor 0, 1 or 2 is a terminal, and 0 when it isn't |
| `term_size (param i32 i32) (result i32)`: out_columns, out_rows | Writes the terminal's columns and rows, 4 bytes each |
| `term_raw (param i32) (result i32)`: mode | With 1, standard input gives each key as it is pressed, with no echo and no line editing: Enter is the byte 13, and an arrow or a function key is a sequence that starts with the byte 27. With 0, the terminal is as it was |

- **`NOTTY` means there is no terminal**, as in every test and in the `run` tool: go on and
  read standard input as it comes, which is what the idea's tests show. Don't treat it as a
  failure.
- **Put the terminal back before the program prints its last lines**, with `term_raw` and 0.
  lengua puts it back too when the program ends, however it ends.
- **A wait for a key that ends at a time** is `poll_oneoff` with a read of descriptor 0 and a
  clock: once the terminal is raw, the read's event comes only when a key is there.
- **Ctrl-C still ends the program**; it doesn't arrive as a key.

`terminal-template.wat` is a whole program.

## What a call borrows

A call reads or writes only the memory its arguments name, and every byte of it must lie inside
one of the program's regions (M2): an iovec array and each buffer it points to, a path's bytes, a
computer's name, a datagram's bytes, a command, what a started program is given and what it
printed, and the place where a call writes a count, a descriptor, an
address or a time. A buffer a call writes may not overlap another argument it reads (M4).

## Errors

Every call returns 0 on success or one of these numbers, and the program tests it before using
anything the call wrote (rule 7):

| Name | Number | | Name | Number |
|---|---|---|---|---|
| ACCES | 2 | | MSGSIZE | 35 |
| AGAIN | 6 | | NAMETOOLONG | 37 |
| BADF | 8 | | NOENT | 44 |
| BUSY | 10 | | NOSYS | 52 |
| CONNREFUSED | 14 | | NOTDIR | 54 |
| EXIST | 20 | | NOTEMPTY | 55 |
| FBIG | 22 | | NOTSOCK | 57 |
| ILSEQ | 25 | | NOTTY | 59 |
| INVAL | 28 | | PIPE | 64 |
| IO | 29 | | SPIPE | 70 |
| ISDIR | 31 | | TIMEDOUT | 73 |
| MFILE | 33 | | NOTCAPABLE | 76 |

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
