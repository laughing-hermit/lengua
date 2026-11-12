# Running a lengua program

A lengua program is a folder with two files: `idea.md`, an [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
Attested Computation that says what the program does and holds its tests, and `main.wat`, the
computation, in WebAssembly text. Every lengua idea names this page as its `executor.resource`:
this is how a lengua computation runs.

## Run it

```sh
lengua-run [--data-dir <absolute path>] <program folder> [--in <folder>] [--env NAME=value]... [--] [<argument>...]
lengua-mcp [--data-dir <absolute path>] run <program folder> [--in <folder>] [--env NAME=value]... [--] [<argument>...]
```

Both are in `server/` of the lengua plugin's folder, and `lengua-mcp install --link` also puts
them in `~/.local/bin` on Linux and macOS. `lengua-run` only runs checked programs. It is lengua
built without the engine's compiler, so it starts sooner and holds less memory, and `check`'s
done reply names it. `lengua-mcp` is lengua's MCP server, whose `run` runs every checked program
and compiles what `lengua-run` can't load. Both make the same checks and give the program the
same things.

They run a folder whose `lengua.lock`, signed with this user's key, records `idea.md` and
`main.wat` as they are. `check` writes that record on this computer once `main.wat` builds,
follows every rule of lengua's WebAssembly dialect and passes every test of its idea. The proofs
may still be running then. A proof that finds inputs where `main.wat` is wrong is recorded in the
lock, and from then on `run` refuses that build. For a program built on parts, the lock records
each part's `idea.md` and `main.wat` too, and no run starts once one of them changed or is gone,
until `check` holds the program to the part again.

Given such a folder, a run:

1. loads the module `check` tested, `main.wat` with lengua's small library merged in, in the
   form `check` compiled it to for this computer. lengua keeps that form in its data folder with
   a seal: an HMAC-SHA256, made with this user's key, of the compiled bytes and of the module's
   SHA-256, which must be the one `lengua.lock` records. A file without that seal is never
   loaded, and neither is one the engine doesn't take, as after the data folder moved to another
   kind of CPU. Where no compiled form loads, `lengua-mcp run` takes the module `check` kept,
   when its SHA-256 is the lock's, or builds `main.wat` and takes the build only when it is that
   same module: after lengua itself changed, a build that differs doesn't run until `check`
   tests it. It compiles that module, seals it and keeps it, and the next run loads it.
   `lengua-run` has no compiler: it says on standard error that the module has no compiled form
   here, and starts `lengua-mcp run`, beside it, with the same words. For an idea with a
   `- Secret:` line, `lengua-run` always starts `lengua-mcp run`, which builds `main.wat` every
   time, finds the operators that take a secret as `check` does, and reads the machine code of
   the module it loaded, on this computer's CPU ([check.md](check.md), step 4): code that
   doesn't keep the secret rule isn't run;
2. runs it on Wasmtime, in lengua's own WASI host, in this process, with this shell's standard
   input, output and error. The working folder is the current one, or the existing folder `--in`
   names, from the current folder when it isn't absolute, never `/`. For a program whose idea uses
   `files`, it is never lengua's data folder or its folder of compiled modules, a folder that
   holds one of them, such as the home folder, or one inside them, and on Linux never `/proc`,
   `/sys` or `/dev` or a folder beneath them: such a program reads and writes everything in its
   working folder. The words after the program
   folder are the program's arguments, and one that starts with `-` goes after `--`.

Any other folder a run refuses, saying why on standard error, and exits with 125. To check a
folder that came from another computer, where this user's key never signed its lock, use
[`lengua-mcp check`](check.md), or have the agent call `check` on it.

## What the program reaches

Only what its idea's `- Uses:`, `- Network:`, `- Files:` and `- Runs:` lines name:

- always its arguments, standard output and standard error, and its exit;
- standard input, with `standard input` or `the terminal`;
- the files and folders beneath the working folder, with `files`: a path that starts with `/`,
  or that would climb above the working folder, is refused with `NOTCAPABLE`. A symbolic link,
  and on Windows a junction, is never followed, and a file with more than one name, a hard link,
  is never opened (`ACCES`): its other name may be a file outside the folder, and the system
  gives lengua no way to tell. That holds on Linux, macOS and Windows, wherever the file system
  counts a file's names. It costs the files that have a second name for another reason: in a
  tree of hard-linked backups (`cp -al`, `rsync --link-dest`), in a package manager's folders
  that link from a store (pnpm, uv), in a Cargo `target` folder or an OSTree checkout. Copy
  such a file to a new name to give it to a program;
- the environment variables each `--env NAME=value` gives, with `the environment`, and no
  others; `--env` for an idea without it is refused;
- the clock, rounded down to a microsecond, with `the clock`;
- the system's random bytes, with `random bytes`;
- with a `- Network:` line that says `connects to` or `listens on`, the connections lengua makes
  for it before it starts, in the idea's order, and for a server the listener lengua binds on
  the idea's port, on 127.0.0.1, or on every address when the idea serves any computer. Such a
  program opens no connection itself. Over TLS, lengua does the TLS, and the program reads and
  writes plain bytes. A connection lengua couldn't make is handed over already ended, and lengua
  says why on standard error;
- what its idea grants it to open by itself, a line for each grant:
  - `opens TCP connections to port 443 on example.com over TLS`: connections to those ports of
    those computers, which the program opens while it runs;
  - `looks up the addresses of example.com`: the addresses of those names;
  - `sends UDP datagrams to port 53 on 192.0.2.53`: datagrams to those ports of those computers,
    and their answers;
  - `receives UDP datagrams on port 5353`: the datagrams that arrive on that port of 127.0.0.1,
    or of every address with `for any computer`, and an answer to each sender.

  The computers are a name or an address, `any subdomain of example.com`, `the computer its
  first argument names`, or `any computer`. Before the program starts, `lengua-mcp run` says
  each grant on standard error, one line each, as `lengua-mcp: it may open TCP connections to
  port 443 on example.com over TLS`. lengua resolves every name itself and checks each address:
  such a program never reaches this computer by a name or an address, never an address of many
  computers (multicast or broadcast), and an address of a private network (10.0.0.0/8,
  172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, 100.64.0.0/10, fc00::/7, fe80::/10) only when
  the idea's line ends with `, private addresses too` or writes that address. What lengua
  refuses, the program gets as an error (`NOTCAPABLE`), and `lengua-mcp run` says it on
  standard error, the first 64 times. A program holds at most 64 connections of its own at
  once, and lengua ends one that no byte crosses for five minutes.
- the file or folder one of its arguments names, with a `- Files:` line for that argument:
  - `reads the file its first argument names`, `writes the file its second argument names`,
    `reads and writes …`, or `reads the files its arguments name` for every argument;
  - `reads the folder its first argument names`, or `reads and writes …`, for a folder and
    everything in it.

  The path is the user's: it counts from the shell's current folder when it doesn't start at
  the root, and the folders on the way are followed as the system follows them. The program
  never gives a path: it tells lengua which argument to open. Before the program starts,
  `lengua-mcp run` says each grant and the place each such argument names, as `lengua-mcp: its
  argument 1 names /home/you/notes/in.txt`. lengua opens no file in its own data folder or its
  folder of compiled modules, and no folder that is one, holds one or lies in one; nothing in
  `/proc`, `/sys` or `/dev`, and on Windows nothing that isn't on a drive or a share; no link at
  the last name; and no file with a second name. The program gets an error for each
  (`NOTCAPABLE`, or `ACCES` for a link), and lengua says why on standard error;
- the programs its `- Runs:` lines name, each with the arguments its line bounds, as
  ``- Runs: `hostname` with no arguments``. lengua finds each in the absolute folders of `PATH`
  and says the file before the program starts, as ``lengua-mcp: `hostname` is
  /usr/bin/hostname``. A started program gets the input the program gives it, an empty
  environment, an empty folder of its own, and its line's time, 10 seconds unless the line
  says more. It is the user's own program: lengua doesn't confine it. lengua starts no shell,
  and no file that lies in the working folder, in a place an argument names, or in the
  system's temporary folder;
- the terminal's mode and size, with `the terminal`: keys as they're pressed, with no echo.
  lengua puts the terminal back as it was when the program ends, by its exit, a trap, or a
  signal that ends `lengua-mcp` (Ctrl-C, which still ends the program, among them);
- more cores, with a `- Splits:` line: the program splits its work into the pieces the line
  bounds, each a run of the same program with a memory of its own, its share of the bytes as
  its standard input, and nothing else of what the idea grants. The pieces run at once, one
  for each core the computer gives the process, and the result is the same on every computer
  and on every run. `LENGUA_PIECES_AT_ONCE`, 1 to 64, sets how many run at once instead, which
  changes the time and nothing else. A piece that fails ends the run as it failed.

## Limits and exit codes

From a shell, a run has no time limit, no limit on its output and none on its files. Memory goes
up to what the module declares, a program's calls take at most 512 KiB of stack, and it holds at
most 4,352 open descriptors, its standard streams included.

`lengua-run` and `lengua-mcp run` exit with:

- the program's own exit code, the low 8 bits of what it gives `proc_exit`;
- 134 when the program trapped: the engine stopped it at an instruction that can't go on, which
  standard error names with its line in `main.wat` when lengua knows it. A failed assertion also
  ends with 134, after `lengua: assertion N failed` on standard error;
- 127 when the module never started, with why on standard error;
- 125 when lengua refuses the folder or can't run it, with why on standard error.

In an agent, the MCP tool `run` runs the program in a process of its own, for 10 seconds by
default and at most 40, with at most 8 MiB of each output stream, in an empty folder or in the
folder it names. A server, of TCP connections or of UDP datagrams, keeps serving in the
background until `run` stops it, ten minutes pass, or the session ends. Its reply's `may_reach`
says what the idea grants the program to reach by itself, as `check`'s reply does, and its
`connections` what the program asked to open or start and what lengua refused. The `run` tool
has no terminal, and it counts the paths of a `- Files:` line from `folder`, or takes them whole.

## The receipt

An idea's `executor.receipt` lists what a run returns, from `stdout`, `stderr`, `exit` and
`files`, each at most once; `check` refuses any other name:

| Field | What it is |
|---|---|
| `stdout` | everything the program wrote to standard output |
| `stderr` | everything it wrote to standard error |
| `exit` | its exit code |
| `files` | the files and folders it left in the working folder |

lengua's tests compare all four for every run, whatever the receipt lists: standard output,
standard error, the exit code, and the working folder, which must hold what the test expects
and nothing else. The `run` tool gives back the program's standard output and error, its exit
code, how it ended and its time on the clock; the files stay in the folder it ran in. A run from
a shell writes to the shell's own streams and exits with the program's code. To check a run
against its idea, see [check.md](check.md).

## Measure it

```sh
lengua-mcp [--data-dir <absolute path>] measure [--in <folder>] [--stdin <file>] [--stdout <file>] [--stderr <file>] [--env NAME=value]... [--uses <words>] [--network <line>] [--serve <port> [--clients <n>] [--requests <n>]] [--counters] <program folder, module.wasm or executable> [[--] <argument>...]
```

`lengua-mcp measure` runs a program once, in a process of its own, and prints one line of JSON:
its CPU time in user space and in the kernel, its time on the clock, its peak resident memory
and the size of what ran, as the system counts them for that process (`getrusage` on Linux and
macOS, the process's own counters on Windows). Standard input comes from the file `--stdin`
names, and standard output and error go to the files `--stdout` and `--stderr` name. The program
gets only the environment variables `--env` gives, and on Windows also `SystemRoot`, without
which its sockets don't start. On Linux the program starts from `lengua-spawn`, a small process
beside `lengua-mcp`, so that its peak memory counts from that process's few hundred KiB and not
from `lengua-mcp`'s; `--counters` adds the CPU's counts of its L1 data, last-level and data TLB
read misses, on Linux.

A program folder runs as `lengua-run` runs it, with every check a run makes, or as `lengua-mcp
run` does where `lengua-run` isn't beside `lengua-mcp`. Any other WebAssembly module of WASI
preview 1, such as a C, C++ or Rust toolchain makes, runs in lengua's own host, by the same
program in the same configuration of the engine, with what `--uses` and `--network` allow,
written as an idea's `- Uses:` and `- Network:` lines are. It runs from its compiled form too,
kept in the data folder under the user's key as a checked program's is: its first run compiles
it, and `measure` reads and hashes its file, so the run it measures does neither. A native
executable runs as a shell
starts it. With `--serve <port>`, the program is a server: 64 clients (`--clients`) each send
1,000 lines (`--requests`) of 64 bytes, one at a time, and must get each back in capitals, and
its time on the clock runs from its first connection to its last answer. The gate that holds
lengua to C, C++ and Rust, `benches/against_rust_and_c.rs`, measures every program this way.
