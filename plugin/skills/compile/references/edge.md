# The edge: what a program calls

A program never makes a system call of its own. It defines `main` and reaches the system by calling
lengua's operations, `os_read`, `os_write` and the rest, which lengua links after `main.s` in a
file of its own, `lengua.s`: its `_start`, the operations the program calls, and the library
functions it calls without defining them (`assert_fail`, `fill`, `parallel_for`). The operations
are the same on every computer lengua runs on; what differs between computers is the registers
each CPU passes them in, which [the x86-64 reference](x86_64.md) and
[the AArch64 reference](aarch64.md) give.

`syscall`, `svc`, a name that starts `os_`, `lengua_`, `LENGUA_` or `.Llengua_`, and a definition
of `_start`, `assert_fail`, `assert_message`, `fill` or `parallel_for` are refused: they are
lengua's.

## Calling an operation

- **Arguments:** x86-64 `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` (`r10`, not `rcx`); AArch64 `x0` to
  `x5`. A 32-bit argument (a descriptor, a mode) is the register's low half.
- **Result:** `rax` or `x0`. A negative result is an error, one of the closed set below, and
  nothing else: test it right away, `test rax, rax` and `js` on x86-64, `tbnz x0, #63, …` on
  AArch64. Every operation but `os_exit` is followed by that test (rule 7).
- **Registers:** an operation changes only the result register, the flags and the registers its
  CPU's reference lists (x86-64: `rcx`, `r11`, every vector register and the mask registers `k0`
  to `k7`; AArch64: `x8`, `x16`, `x17`, `x30`, the vector registers `v0` to `v7` and `v16` to
  `v31`, and SVE's). Every other register, the argument registers included, comes back as it
  went, so a loop's counter in `rbx` or `x19` survives a call, and so does a pointer in `rsi`. A
  register the operation changes can't be read after the call until something sets it.
- **Never returns:** `os_exit`. A function that calls one of these keeps the CPU's own rules
  for its frame and the link register.

## The operations

Descriptors 0, 1 and 2 are standard input, output and error; 3 is a server's listening socket;
a program's connections are 4 and on. Every operation refuses descriptor 4,352, which is lengua's
own, with `EBADF`.

| Operation | Arguments | Result |
|---|---|---|
| `os_read` | descriptor, buffer, its size | bytes read, 0 at the end of the input |
| `os_write` | descriptor, bytes, how many | bytes written, possibly fewer than asked: loop until done, as the template's `write_all` does |
| `os_open` | name (NUL-terminated, relative to the working folder), mode | a descriptor. Modes: 0 to read, 1 to write (made if it isn't there, emptied if it is), 2 to write at the end (made if it isn't there), 3 to write a file that isn't there yet, 4 a folder to list |
| `os_close` | descriptor | 0 |
| `os_seek` | descriptor, offset, whence (0 the start, 1 here, 2 the end) | the new offset |
| `os_info` | descriptor, 16 bytes | 0. The 16 bytes hold the size, which is 0 unless it is a file, then the kind: 0 a file, 1 a folder, 2 anything else |
| `os_list` | a folder opened with mode 4, buffer, its size | bytes of entries in the buffer, 0 at the end |
| `os_mkdir` | name | 0 |
| `os_remove` | name, 0 for a file or 1 for an empty folder | 0 |
| `os_rename` | the name it has, the name it is to have, which is replaced | 0 |
| `os_clock` | clock (0 the time of day, 1 one that only moves forward), 16 bytes | 0. The 16 bytes hold the seconds, then the nanoseconds |
| `os_sleep` | clock, 0 to wait that long or 1 to wait until that time, 16 bytes of seconds then nanoseconds | 0 |
| `os_random` | buffer, how many bytes, at most 256 | how many it filled, all of them |
| `os_accept` | 3, the listening socket | the next connection's descriptor |
| `os_shutdown` | connection, 0 to stop receiving, 1 to stop sending or 2 both | 0 |
| `os_poll` | a table of 8-byte entries (descriptor 4 bytes, the events waited for 2, the events that came 2), how many, the most milliseconds to wait (-1 for no limit) | how many entries have events, 0 when the time ran out. A negative descriptor is passed over |
| `os_terminal` | 1 to take keys as they are pressed, with no echo or line editing, or 0 to put the mode back as it was found | 0, or `ENOTTY` where standard input isn't a terminal, as in every test |
| `os_window` | 8 bytes | 0, or `ENOTTY`. The 8 bytes hold the rows, then the columns, 4 bytes each |
| `os_thread_start` | the thread's number, 1 to 63, its slot | 0 once it runs. It runs `worker(number, slot)`, which is `main.s`'s |
| `os_thread_join` | a thread's number | what its worker returned |
| `os_exit` | the status | doesn't return |

`os_poll`'s events are `POLLIN` 1, `POLLOUT` 4, `POLLERR` 8 and `POLLHUP` 16. A listing's entries
are, each, a 2-byte length (a multiple of 8, which is where the next starts), a byte for the kind
(as `os_info`'s), a byte for the name's length, then the name and a NUL, padded with zeros; `.`
and `..` are left out. The order is the system's, and a test runs a program that lists a folder
again with the order turned round, so sort the entries before acting on their order.

## Errors

An operation gives only the errors it lists, and any other the system might give is `EIO`. The
numbers are the same on every system: `ENOENT` 2, `EIO` 5, `EBADF` 9, `EAGAIN` 11, `EACCES` 13,
`EBUSY` 16, `EEXIST` 17, `ENOTDIR` 20, `EISDIR` 21, `EINVAL` 22, `EMFILE` 24, `ENOTTY` 25, `EFBIG` 27,
`ENOSPC` 28, `ESPIPE` 29, `EPIPE` 32, `ENAMETOOLONG` 36, `ENOTEMPTY` 39, `EILSEQ` 84,
`ECONNRESET` 104 and `ENOTCONN` 107. A result is `-` one of them.

| Operation | Errors |
|---|---|
| `os_read` | `EIO` `EBADF` `EAGAIN` `EISDIR` `EINVAL` `ECONNRESET` |
| `os_write` | `EIO` `EBADF` `EAGAIN` `EFBIG` `ENOSPC` `EPIPE` `EINVAL` `ECONNRESET` |
| `os_open` | `ENOENT` `EIO` `EACCES` `EBUSY` `EEXIST` `ENOTDIR` `EISDIR` `EINVAL` `EMFILE` `ENOSPC` `ENAMETOOLONG` `EILSEQ` |
| `os_close`, `os_info` | `EIO` `EBADF` |
| `os_seek` | `EBADF` `EINVAL` `ESPIPE` |
| `os_list` | `EIO` `EBADF` `ENOTDIR` `EINVAL` |
| `os_mkdir` | `ENOENT` `EIO` `EACCES` `EEXIST` `ENOTDIR` `EINVAL` `ENOSPC` `ENAMETOOLONG` `EILSEQ` |
| `os_remove` | `ENOENT` `EIO` `EACCES` `EBUSY` `ENOTDIR` `EISDIR` `EINVAL` `ENOTEMPTY` `ENAMETOOLONG` `EILSEQ` |
| `os_rename` | `ENOENT` `EIO` `EACCES` `EBUSY` `EEXIST` `ENOTDIR` `EISDIR` `EINVAL` `ENOSPC` `ENOTEMPTY` `ENAMETOOLONG` `EILSEQ` |
| `os_clock`, `os_sleep`, `os_thread_join` | `EINVAL` |
| `os_poll` | `EBADF` `EINVAL` |
| `os_random` | `EIO` `EINVAL` |
| `os_accept` | `EIO` `EBADF` `EAGAIN` `EINVAL` `EMFILE` `ECONNRESET` |
| `os_shutdown` | `EBADF` `EINVAL` `ENOTCONN` |
| `os_terminal`, `os_window` | `ENOTTY` |
| `os_thread_start` | `EAGAIN` `EINVAL` |

Each error a program meets is a behavior its idea names, with an exit code of its own. A
constant for an error or a mode is a `.equ` line at the top of `main.s` (`.equ ENOENT, -2`), as a
named exit status is.

## What the idea allows

What the idea names under `# Constraints` decides the operations a program may call, and the
checker refuses a call to one the idea doesn't allow: `os_write` and `os_exit` always; `os_read`
for `standard input`, `files`, `the terminal` and every network; `os_open`, `os_seek`, `os_info`,
`os_list`, `os_mkdir`, `os_remove` and `os_rename` for `files`, and `os_close` for `files` and
the network; `os_clock` and `os_sleep` for `the clock`; `os_random` for `random bytes`;
`os_accept` for `- Network: listens on …`; `os_shutdown` for any network; `os_poll` for `the
terminal` and for `up to N clients at once`; `os_terminal` and `os_window` for `the terminal`;
`os_thread_start` and `os_thread_join` for `- Threads:`, which only `parallel_for` calls. The
environment needs no operation: `main` is given it.

## Names

lengua holds every name a program gives `os_open`, `os_mkdir`, `os_remove` and `os_rename` to what
every system takes. A name Windows refuses (`< > : " \ | ? *`, a control character, a dot or space
at the end, `CON`, `NUL.txt`, `com1` and the other device names, and a short name's shape such as
`REPORT~1.TXT`) is `EINVAL`; one that isn't UTF-8 of characters Unicode 9.0 assigned, `EILSEQ`;
one past 255 bytes in a part or 1,023 in all, `ENAMETOOLONG`; a new name that differs only in case
or normalization from one in its folder, `EEXIST`; and renaming or removing a file the program
holds open, `EBUSY`. A name is relative to the working folder: any way out of it, a link or a path
from the root, is `EACCES`, and a missing file is `ENOENT`. A program holds at most 4,352 files
open.

## Files

Open an input with mode 0, create or replace an output with mode 1, and `os_close` each when
done. lengua runs a program in a working folder of its own, an empty temporary folder, or the
user's folder when they ask to run it on their files. A test gives the program the files and
folders its Given steps name (``a file `notes.txt` that contains TEXT``, ``an empty folder
`archive` ``) and checks what it leaves with its Then steps (``the file `out.txt` contains TEXT``,
``there is no file `notes.txt` ``).

## The network

A program never connects or binds. lengua makes each connection its idea names before the program
starts and hands it over, the first `- Network: connects to …` place as descriptor 4, the next 5, and
so on (`.equ CONNECTION, 4`); a server gets its listening socket as descriptor 3 and takes each
connection with `os_accept`. A client sends with `os_write`, reads until `os_read` returns 0, when
the server has closed its side, and may `os_shutdown` its sending side when it's done. A
connection lengua couldn't make comes ended: its first `os_read` gives 0 and its first
`os_write` fails with `EPIPE`, never a signal. A connection `over TLS` is plain bytes to the
program too: lengua does the TLS. A connection to the computer an argument names is made the same
way, before the program starts, so the program only reads the argument, for its `Host:` line. A
server reads until it has the request, answers, and closes the connection. Data can arrive in
pieces, so never assume one `os_read` gets it all.

## The terminal

Keys one at a time with no echo: `os_terminal(1)` takes them as they are pressed, and
`os_terminal(0)` at the end puts the mode back as it was found, as `assets/terminal-template.s`
does. Where standard input isn't a terminal, as in every test, `os_terminal` returns `ENOTTY`, keys
come as they are and nothing changes. `os_poll` on standard input with a time waits for the next
key, a game's tick when none comes. Arrows and other special keys are escape sequences of several
bytes, such as 27, 91, 65 for up, and `\033[…` sequences move the cursor, clear the screen and
color text.

## Random bytes and the clock

Tests usually supply them in place of the system's, so output that depends on them can be checked
exactly. Draw the random bytes the idea's Purpose says how to use, in any batch size: bytes asked
for past what a test supplies are zeros, so only the bytes the program uses decide its output. Each
`os_clock` call reads the test's next reading, and the last one repeats. A test's `os_sleep` ends
at once. Check each result like any call's.

## The library

Functions a program calls without defining them, with the same register convention as its own
functions, which the CPU's reference gives:

- `assert_fail(id)`: the assertion's number, in the first argument register. It prints
  `lengua: assertion NNN failed` to standard error and ends the program, every thread of it, with
  code 134. A program's assertions are numbered from 1 to 899 and each number is used once;
  lengua's own are 901 to 903 (`parallel_for`) and 990 to 992 (`fill`).
- `fill(descriptor, buffer, size, stop)`: reads into the `size` bytes (at least 1) until they are
  full, the input ends, or, when `stop` is a byte, the last byte read is it; `stop` -1 reads to the
  end. However the input arrives, it reads the same, so a proof covers every way it can be cut.
  It returns how many bytes it read, or a negative error. To stream input larger than a buffer,
  call it again for each buffer-full with `stop` -1.
- `parallel_for(count, slots, slot_size, results)`: runs `worker(index, slot)` for every index
  from 0 to `count - 1`, each on its own thread (worker 0 on this one), and returns when all have
  finished. `count` is 1 to 64 and at most the idea's thread count; `worker` is `main.s`'s, takes
  its index and its slot, writes only its own slot and its own stack, and returns a result,
  which lands in `results[index]`, 8 bytes each. A worker calls no operation. Each thread has 1
  MiB of stack, less 64 bytes.

## `main`

`_start` is lengua's: it calls `main` with the argument count, the array of arguments and the array
of environment variables in the first three argument registers. Each argument and variable is a
NUL-terminated string, read-only; the first argument is the program's path. A program gets
environment variables only when its idea uses `the environment`, and then only those its test or
the user's run gives; otherwise the list is empty. `main` is entered by a call, so its stack is
as a function's is; it never returns: it ends with `call os_exit` (`bl os_exit` on AArch64) with the
status in the first argument register, a named constant from 0 to 255.
