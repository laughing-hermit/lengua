# lengua's host: what a WebAssembly program may call

A `main.wat` program reaches the computer only through WASI preview 1's functions
(`wasi_snapshot_preview1`) and, for what it opens on the network by itself, lengua's own
(`lengua_net_1`), and only those its idea allows. lengua implements them itself, the same way on
every computer, so a program does the same thing everywhere. `check` refuses any other import
(rule 3 of [the WebAssembly dialect](power-of-10-webassembly.md)).

## What each idea line allows

| The idea says | The program may import |
|---|---|
| (always) | `args_get`, `args_sizes_get`, `fd_write` (to descriptors 1 and 2), `fd_fdstat_get`, `proc_exit`, `sched_yield` |
| `- Uses: standard input` | `fd_read` (descriptor 0), `poll_oneoff` (a wait to read it) |
| `- Uses: the environment` | `environ_get`, `environ_sizes_get` |
| `- Uses: files` | `fd_prestat_get`, `fd_prestat_dir_name` (descriptor 3), `path_open`, `fd_read`, `fd_write`, `fd_seek`, `fd_tell`, `fd_close`, `fd_filestat_get`, `path_filestat_get`, `fd_readdir`, `path_create_directory`, `path_remove_directory`, `path_unlink_file`, `path_rename` |
| `- Uses: the clock` | `clock_time_get`, `clock_res_get`, `poll_oneoff` (clock waits) |
| `- Uses: random bytes` | `random_get` |
| `- Network: …` (but a line that only looks up addresses) | `sock_recv`, `sock_send`, `sock_shutdown`, and `fd_read`, `fd_write` and `fd_close` on a connection, `poll_oneoff` (waits on descriptors); and with `listens on …`, `sock_accept` (descriptor 4) |
| `- Network: opens TCP connections to …` | from `lengua_net_1`: `tcp_open` |
| `- Network: looks up the addresses of …` | from `lengua_net_1`: `dns_lookup` |
| `- Network: sends UDP datagrams to …` | from `lengua_net_1`: `udp_open`, `udp_send`, `udp_recv` |
| `- Network: receives UDP datagrams on …` | from `lengua_net_1`: `udp_recv_from`, `udp_send_to` |

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
  computer is this one, or one of its addresses is in a private network the idea's line doesn't
  reach; `INVAL`, the text is no computer's name or address, or the port is none; `NAMETOOLONG`,
  more than 253 bytes; `NOENT`, the name has no address; `CONNREFUSED`, the computer didn't take
  the connection, or its certificate didn't check; `TIMEDOUT`, no answer within 5 seconds;
  `MFILE`, as above. Give the user a different message for `NOTCAPABLE` than for a computer that
  is down: the first is the idea's, and no retry changes it.
- **The name is as the user or a server wrote it**: lengua takes capitals and a dot at the end,
  and an address in any standard writing. It refuses a number that only an older parsing reads
  as an address, such as `2130706433` or `127.1`, with `INVAL`.
- **A sender is answered only once it sent something**: `udp_send_to` to any other address
  gives `NOTCAPABLE`, and on a port any computer may send to, so does more than three times the
  bytes that address sent.
- **In a test** the computers are the test's own: a `tcp_open` to a computer the test gives no
  server gives `CONNREFUSED`, a datagram to one is lost and `udp_recv` then gives `CONNREFUSED`,
  and a name the test gives no addresses gives `NOENT`.

## What a call borrows

A call reads or writes only the memory its arguments name, and every byte of it must lie inside
one of the program's regions (M2): an iovec array and each buffer it points to, a path's bytes, a
computer's name, a datagram's bytes, and the place where a call writes a count, a descriptor, an
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
