#!/usr/bin/env python3
"""Runs a suite of real interactive Claude Code sessions with the plugin, the product as people
use it: for each request, a session in a fresh folder, its trust prompt answered, the request
typed, and then what a person who wants the program would do: yes to each permission prompt
(and not to ask again), the first choice of each question (lengua's skills put the recommended
one first), and "Yes, go ahead." when the agent asks in words. It stops when the program's lock
says check passed it and the agent is quiet, or at the time limit, and appends each session's
outcome, turns and minutes to RESULTS. Each session costs Claude usage, about $2: run it by hand,
never while the server is being rebuilt.

  scripts/session-suite.py RESULTS [--target linux-aarch64] [--only N[,N...]] [CLAUDE ARGS...]
"""

import os
import pathlib
import re
import subprocess
import sys
import time

SESSION = pathlib.Path(__file__).resolve().parent / "interactive-session.py"
FOLDERS = pathlib.Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp")) / "lengua-suite"
LIMIT = 30 * 60
MOST_TURNS = 8

# The spread of what people ask for: arguments, a text filter, counts, files and folders, random
# bytes, the clock and the environment, a server, a client, HTTPS, a tally, floats, threads.
REQUESTS = [
    "make me a program that adds the two whole numbers given as its arguments",
    "I need a filter that reverses each line of its standard input",
    "write a fast program that counts the lines, words and bytes of its input, like wc",
    "make a tool that lists the files in the folder given as its argument with their sizes, "
    "sorted by name",
    "create a program that prints a random password of 16 letters and digits",
    "write a program that prints the date and time now in UTC and then the HOME environment "
    "variable",
    "build a server on port 8080 that sends back each line a client sends in capitals, serving "
    "several clients at once",
    "make a program that fetches https://example.com/ and prints the page's title",
    "write a program that counts how often each word occurs on standard input and prints the "
    "ten most common",
    "make a program that prints the mean and standard deviation of the numbers on its standard "
    "input to three decimals",
    "write a program that sums the squares of 1 to N, N its argument, using all of the "
    "computer's cores",
    "make a program that asks for your name in the terminal and greets you",
]


def session(*args):
    return subprocess.run([sys.executable, str(SESSION), *args], capture_output=True,
                          text=True).stdout


def screen(name):
    return session("screen", name, "4000")


def outcome(folder):
    """What the program's lock says, in the folder or a program folder the agent made in it:
    none, passed (check tested it), and its proofs."""
    locks = sorted(folder.glob("**/lengua.lock"), key=lambda path: path.stat().st_mtime)
    if not locks:
        return "no lock", ""
    lock = locks[-1]
    text = lock.read_text()
    if "[code]" not in text:
        return "idea only", ""
    proven = re.search(r"proven = \[(.*?)\]", text)
    bounded = re.search(r"bounded = \[(.*?)\]\n", text)
    return "passed", "; ".join(part for part in (
        f"proven {proven.group(1)}" if proven and proven.group(1) else "",
        f"bounded {bounded.group(1)}" if bounded and bounded.group(1) else "") if part)


def flat(text):
    """The screen's words without their spaces, lowercase, as the terminal's drawing leaves
    some words joined."""
    return re.sub(r"\s+", "", text).lower()


def trust(name, seconds=40):
    """Answers the new folder's trust prompt, which starts on "No, exit", with "Yes", before
    anything is typed: text typed into the prompt would answer it."""
    for _ in range(seconds):
        time.sleep(1)
        shown = flat(screen(name))
        if "trust" in shown and "folder" in shown:
            session("keys", name, "Down", "Enter")
            break
    time.sleep(3)


def sent(name, tries=4):
    """Presses Enter again while the request has no transcript yet: an Enter typed while
    Claude Code is still starting is lost, leaving the request in the input box."""
    for _ in range(tries):
        time.sleep(8)
        if session("transcript", name).strip() != "None":
            return
        session("keys", name, "Enter")


def answer(name, shown):
    """Answers what the screen asks, as a person who wants the program would; returns whether
    it answered something."""
    tail = flat(shown[-2500:])
    if "doyouwantto" in tail and "1.yes" in tail:
        # "Yes, and don't ask again" where it's offered, else "Yes".
        if "2.yes" in tail:
            session("keys", name, "Down", "Enter")
        else:
            session("keys", name, "Enter")
        return True
    if "❯1." in tail and ("entertoselect" in tail or "tonavigate" in tail):
        session("keys", name, "Enter")
        return True
    return False


def run(number, request, target, claude_args, results):
    name = f"suite-{target}-{number:02d}"
    folder = FOLDERS / name
    if folder.exists():
        subprocess.run(["rm", "-rf", str(folder)], check=True)
    folder.mkdir(parents=True)
    env_note = ""
    if target != "native":
        os.environ["LENGUA_TARGET"] = target
        env_note = f" (LENGUA_TARGET={target})"
    started = time.monotonic()
    session("start", name, str(folder), *claude_args)
    trust(name)
    session("type", name, request)
    sent(name)
    turns, quiet = 1, 0
    while time.monotonic() - started < LIMIT:
        reply = session("wait", name, "25")
        shown = screen(name)
        if answer(name, shown):
            quiet = 0
            continue
        state, _ = outcome(folder)
        words = reply.split("---", 1)[-1].strip()
        if state == "passed" and "proving" not in words.lower():
            quiet += 1
            if quiet >= 2:
                break
            continue
        if words.endswith("?") and turns < MOST_TURNS:
            session("type", name, "Yes, go ahead.")
            turns += 1
            continue
        quiet += 1
        if quiet >= 6:
            break
    minutes = (time.monotonic() - started) / 60
    session("stop", name)
    state, proofs = outcome(folder)
    line = (f"| {number} | {target}{env_note} | {request} | {state} | {proofs or '-'} | "
            f"{turns} | {minutes:.1f} | {folder} |\n")
    with open(results, "a") as file:
        file.write(line)
    print(line, end="", flush=True)


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    results, rest = pathlib.Path(argv[1]), argv[2:]
    target, only = "native", None
    while rest and rest[0] in ("--target", "--only"):
        if rest[0] == "--target":
            target, rest = rest[1], rest[2:]
        else:
            only, rest = {int(n) for n in rest[1].split(",")}, rest[2:]
    if not results.exists():
        results.write_text("| # | target | request | outcome | proofs | turns | minutes | folder |\n"
                           "|---|---|---|---|---|---|---|---|\n")
    for number, request in enumerate(REQUESTS, 1):
        if only is None or number in only:
            run(number, request, target, rest, results)


if __name__ == "__main__":
    main(sys.argv)
