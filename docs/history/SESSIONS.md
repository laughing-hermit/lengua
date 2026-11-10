> This file describes the native lengua's time. The tag `native-final` keeps that lengua.

# lengua in real sessions

The product as people use it: each request typed into a real interactive Claude Code session with
lengua's plugin from this repository, in a new folder, by a driver (the tag `native-final` keeps
it) that answers as a person who wants the program would (yes to each permission prompt, the first
choice of each question, and "Yes, go ahead." when asked in words) and stops when the program's
lock says `check` passed it. "Proven" lists the behaviors proven for every input in range; the
rest are tested, with each proof line saying why. Turns counts the requests typed, the first
included; minutes are the session's, the agent's work and lengua's checks together.

## x86-64 Linux (2026-11-02)

Natively, on an AMD Ryzen AI 7 PRO 350: 12 of 12 passed from one request each.

| # | request | outcome | proven | turns | minutes |
|---|---|---|---|---|---|
| 1 | make me a program that adds the two whole numbers given as its arguments | passed (the driver at first missed its lock, in the program folder the agent made, which it now finds; the session took 2.5 minutes, and the driver waited 5.3) | B1, B3, B4, B5 | 1 | 2.5 |
| 2 | I need a filter that reverses each line of its standard input | passed | - | 1 | 7.6 |
| 3 | write a fast program that counts the lines, words and bytes of its input, like wc | passed | - | 1 | 3.8 |
| 4 | make a tool that lists the files in the folder given as its argument with their sizes, sorted by name | passed | - | 1 | 6.7 |
| 5 | create a program that prints a random password of 16 letters and digits | passed | B1 | 1 | 4.2 |
| 6 | write a program that prints the date and time now in UTC and then the HOME environment variable | passed | - | 1 | 5.9 |
| 7 | build a server on port 8080 that sends back each line a client sends in capitals, serving several clients at once | passed (first ended with its request unsent, as the driver typed Enter while Claude Code was still starting; the driver now checks the request went, and the run again passed) | - | 1 | 5.4 |
| 8 | make a program that fetches https://example.com/ and prints the page's title | passed | - | 1 | 5.9 |
| 9 | write a program that counts how often each word occurs on standard input and prints the ten most common | passed | - | 1 | 3.2 |
| 10 | make a program that prints the mean and standard deviation of the numbers on its standard input to three decimals | passed | - | 1 | 9.9 |
| 11 | write a program that sums the squares of 1 to N, N its argument, using all of the computer's cores | passed | B3, B4, B5 | 1 | 4.2 |
| 12 | make a program that asks for your name in the terminal and greets you | passed | B1, B2, B3, B4 | 1 | 3.3 |

## ARM64 Linux (2026-11-02)

On the same computer with `LENGUA_TARGET=linux-aarch64`, so the agent compiled for AArch64 and
lengua checked and tested under QEMU's user mode (each test run held by Landlock, though not by
the kernel's filter of system calls, which QEMU's own calls would trip; GitHub's ARM64 runners
test that natively): 12 of 12 passed from one request each.

| # | request | outcome | proven | turns | minutes |
|---|---|---|---|---|---|
| 1 | make me a program that adds the two whole numbers given as its arguments | passed | B3, B4, B5 | 1 | 4.3 |
| 2 | I need a filter that reverses each line of its standard input | passed | - | 1 | 7.0 |
| 3 | write a fast program that counts the lines, words and bytes of its input, like wc | passed | - | 1 | 3.4 |
| 4 | make a tool that lists the files in the folder given as its argument with their sizes, sorted by name | passed | B6, B9 | 1 | 10.7 |
| 5 | create a program that prints a random password of 16 letters and digits | passed | B1 | 1 | 4.1 |
| 6 | write a program that prints the date and time now in UTC and then the HOME environment variable | passed | - | 1 | 6.1 |
| 7 | build a server on port 8080 that sends back each line a client sends in capitals, serving several clients at once | passed | - | 1 | 6.5 |
| 8 | make a program that fetches https://example.com/ and prints the page's title | passed | - | 1 | 7.8 |
| 9 | write a program that counts how often each word occurs on standard input and prints the ten most common | passed | - | 1 | 4.0 |
| 10 | make a program that prints the mean and standard deviation of the numbers on its standard input to three decimals | passed | - | 1 | 10.1 |
| 11 | write a program that sums the squares of 1 to N, N its argument, using all of the computer's cores | passed | - | 1 | 5.4 |
| 12 | make a program that asks for your name in the terminal and greets you | passed | - | 1 | 4.1 |
