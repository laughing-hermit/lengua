# What a program reaches by itself: files the user names, programs, the terminal, more cores

Most programs need no line of this page. A program that works on files in the folder it runs in
says `- Uses: files`, and that is the least it can have, so choose it whenever the user's files
are in one folder. This page is for the three things `- Uses:` doesn't give: a file or folder
anywhere, which the user names when the program runs; another program of the computer; and the
terminal's mode and size; and, at its end, for work split into pieces that run on more cores.
Each is a grant the idea states. lengua checks the lines, tells the
user what they grant before the program runs (`check`'s and `run`'s `may_reach`, and a shell's
run on standard error), and holds every call the program makes to them. Tell the user each grant
in your final report, in plain words.

The network a program chooses has [a page of its own](network.md).

## Files and folders the user names

| Line | What the program may do |
|---|---|
| `- Files: reads the file its first argument names` | Read that one file |
| `- Files: writes the file its second argument names` | Write that one file, and make it when it isn't there |
| `- Files: reads and writes the file its first argument names` | Both |
| `- Files: reads the files its arguments name` | Read the file any of its arguments names (`writes`, `reads and writes` too) |
| `- Files: reads the folder its first argument names` | Read that folder and everything in it |
| `- Files: reads and writes the folder its first argument names` | And change it: make, write, rename and remove files and folders in it |

- **The argument is named by its place**, `first` to `ninth`, or all of them with `the files its
  arguments name`. Each line is one grant, and an idea has up to 16. Two lines about one
  argument add up: `reads` and `writes` on two lines is `reads and writes`.
- **Say `reads` when the program only reads.** A file granted for reading can't be changed, and
  a folder granted for reading takes no change at all. Grant `writes` only where the request
  says the program changes something, and one file, not a folder, when one file is enough.
- **The program never sees a path.** It tells lengua which argument to open, and lengua reads
  that argument, which the user typed. So a program can't open a path it read in a file or got
  from a network: say so in Purpose when it matters to the user.
- **Where the path leads:** anywhere the user can reach, by a path from the folder the user is
  in or a whole path. Never lengua's own folders, and never the system's own (`/proc`, `/sys`,
  `/dev`, a device): a program hears that it may not open them. The last name follows the rules
  of every file a lengua program opens: no link, no file that has a second name (a hard link),
  and a name every system takes.
- **Tests** name files in the test's own folder: ``Given the arguments `in.txt` and `out.txt` ``
  with ``a file `in.txt` that contains …``, and ``the file `out.txt` contains …`` afterwards. A
  test's path can't leave that folder: one that starts with `/` or climbs out with `..` isn't
  opened, in a test. An argument the program doesn't have gives the program an error of its
  own, so write the usage behavior as for any tool.
- **Limits belong in Purpose** as for every file: the largest file the program reads, in
  backticks, with a behavior for a file just past it.

The compile skill's `files-template-wat-idea.md` is a whole idea with these lines.

## Programs the program starts

``- Runs: `hostname` with no arguments`` lets the program start that program, wait for it, and
read what it printed and its exit code. Each line names one program and bounds its arguments:

| Line | What the program may start |
|---|---|
| ``- Runs: `hostname` with no arguments`` | `hostname`, with nothing after it |
| ``- Runs: `git` with the arguments `rev-parse` and `HEAD` `` | exactly that command |
| ``- Runs: `tar` with the arguments `-tf` and a value`` | `tar -tf` and one more argument, which the program chooses |
| ``- Runs: `sort` with the arguments `-n`, a value and up to 4 values, within 60 seconds`` | one to five values after `-n`, for 60 seconds at most |

- **The name is the program's own**, in backticks, as the user types it to run it: no path. The
  user's computer must have it on its `PATH`; a program that isn't there is an error the idea
  gives a behavior.
- **Never a shell**, and never a program whose work is to start another: `check` refuses `sh`,
  `bash`, `cmd`, `powershell`, `env`, `xargs`, `sudo` and their like. Name the program that does
  the work. Don't name a program that runs what an argument says, such as an interpreter with
  `-c`, unless the request is exactly that and the user agrees: the line is as wide as its
  arguments.
- **Arguments in backticks are fixed**, byte for byte. `a value` is one argument the program
  chooses: text of 1 to 255 bytes that doesn't start with `-` or `/`, so it is never an option.
  `up to N values` comes last. Write every option the program needs as a fixed argument; leave
  to the program only what must vary.
- **What the started program gets:** the bytes the program gives it as standard input, and
  nothing else: no environment variable, an empty folder of its own to run in, and no file of
  the program's. So it can't work on the user's folder: a started program suits tools that read
  their input and print a result. Its standard error is dropped.
- **Time and output:** 10 seconds unless the line says `within N seconds`, 1 to 600, and at
  most the bytes Purpose says the program keeps of its output. Past either, lengua ends the
  started program and the program hears an error: give each a behavior when it can happen.
- **lengua doesn't confine a started program.** It is one of the user's own programs and does
  what it does with the user's rights. Say that to the user when you propose the line.
- **Tests start nothing.** A step says what a command gives: ``Given the program `hostname`
  prints `box\n` and exits with code `0` ``, or ``Given the program `tar` with the arguments
  `-tf` and `a.tar` prints nothing and exits with code `2` ``. Write the output with `\n` for
  its line breaks, and the values as they are, not as placeholders. Every test of the idea
  sees every such step, so one command has one result in an idea's tests; a command no step
  gives isn't there, which is how a test shows a missing program. Show a failing program with
  another argument, or with a second program.

The compile skill's `program-template-wat-idea.md` is a whole idea with these lines.

## The terminal

`- Uses: the terminal` gives keys as they're pressed, with no Enter and no echo, a wait for the
next key that can end at a time, the terminal's columns and rows, and whether each standard
stream is a terminal. lengua puts the terminal back as it was when the program ends, however it
ends. Ctrl-C still ends the program.

- **Tests have no terminal.** A test gives the keys as standard input, a key a byte or an
  escape sequence, and the program reads them as from a pipe. A test can't show the terminal's
  size, or that no key is echoed: say those in Purpose, and give the behaviors what a pipe can
  show, the keys and what the program prints for them.
- **Say what the program does without a terminal**, since that is what its tests run: most
  programs go on and read their input as it comes.

The compile skill's `terminal-template-wat-idea.md` is a whole idea that uses the terminal.

## Work split into pieces that run at once

By default a program is one run on one core. When the user's work is large and can be done a
part at a time, such as a count or a sum over a large input, grant a split, on one line:

```markdown
- Uses: standard input
- Splits: its work into up to 8 pieces, each of up to 1048576 bytes, each gives back 32 bytes
```

The line says how many pieces at most (2 to 64), how many bytes a piece gets at most, and how
many bytes each piece gives back (1 to 65536). A piece is one more run of the same program: it
gets its share of the bytes as its standard input, and what it prints is its answer to the
program. It reaches nothing else. The pieces run at the same time when the computer has more
than one core, and the result is the same on every computer.

- **Name `standard input` under `- Uses:`**, also when the whole program reads a file: a piece
  reads its bytes there.
- **Name no secret.** `check` refuses an idea with a `- Splits:` line and a `- Secret:` line.
- **Say in Purpose what a piece does and what the program does with the answers**, in plain
  words: the piece counts its share, and the program adds the counts.
- **Don't promise the user more speed.** Each piece costs a start and a copy of its bytes:
  measured on 3 or 4 cores, a count split in eight was faster than one core on 256 MiB and
  slower on 64 MiB on two of five systems, so
  grant a split only for large inputs or work that is heavy for each byte. Tell the user what it buys only
  after `check`'s performance line, or a run, has shown it.
- **Test a piece as a program.** A behavior with the step `the program is a piece` runs the
  program as a piece: standard input is the piece's share, and it prints its answer.

```markdown
## B5: Counts the bytes of its share as a piece
- Given standard input that contains `a\nb\n`
- And the program is a piece
- When the program runs
- Then it prints `2` and a newline
- And it exits with code `0`
```

`check` proves a behavior of a piece where it can, as it proves any program that reads standard
input. A behavior of the whole program is tested, not proven: each of its tests runs twice,
with the pieces at once and with the pieces one at a time, the last first, and both runs must
pass.
