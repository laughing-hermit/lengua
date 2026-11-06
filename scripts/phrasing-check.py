#!/usr/bin/env python3
"""Checks that agents choose lengua for programs however a person asks: for each of twelve
phrasings of a request for a program, starts a real interactive Claude Code session with the
plugin in a fresh folder, types the request, and records the first tool the agent calls. It
passes when every first call is lengua's write-idea skill. Each session stops at that call, so a
run costs about $4; run it by hand before each interactive round, never while rebuilding the
server.

  scripts/phrasing-check.py [CLAUDE ARGS...]   # such as --model claude-opus-5-5
"""

import json
import pathlib
import shutil
import subprocess
import sys
import time

SESSION = pathlib.Path(__file__).resolve().parent / "interactive-session.py"
FOLDERS = pathlib.Path("/tmp/lengua-ux/phrasing")
PHRASINGS = [
    "make me a program that adds two numbers",
    "write a script that counts the words in a file",
    "I need a server that sends back each line it gets",
    "build a tool that converts temperatures from Celsius to Fahrenheit",
    "can you create a command that prints today's date",
    "I want a little utility that reverses each line of a text file",
    "code me a CLI that sums a column of numbers from standard input",
    "make a fast program that computes the CRC-32 of its input",
    "write something that rolls two dice",
    "create an app that fetches a page from my web server on port 8080",
    "I need a filter that puts its input in capital letters",
    "put together a program that says whether a number is prime",
]
WANTED = "lengua:write-idea"


def session(*args):
    return subprocess.run([sys.executable, str(SESSION), *args], capture_output=True,
                          text=True, check=True).stdout


def trust(name, seconds=30):
    """Answers the new folder's trust prompt, which starts on "No, exit", with "Yes", before
    anything is typed: text typed into the prompt would answer it."""
    for _ in range(seconds):
        time.sleep(1)
        try:
            screen = session("screen", name, "4000").lower()
        except subprocess.CalledProcessError:
            # The session hasn't drawn its screen yet.
            continue
        if "trust" in screen and "folder" in screen:
            session("keys", name, "Down", "Enter")
            break
    time.sleep(3)


def first_call(name, seconds=150):
    """The first tool the session's agent calls, as `name input`, or why there is none."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        time.sleep(1)
        path = session("transcript", name).strip()
        if not path or path == "None" or not pathlib.Path(path).exists():
            continue
        for line in pathlib.Path(path).read_text().splitlines():
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            content = (entry.get("message") or {}).get("content")
            if entry.get("type") != "assistant" or not isinstance(content, list):
                continue
            for block in content:
                if block.get("type") == "tool_use":
                    called = block.get("input") or {}
                    return f"{block.get('name')} {called.get('skill') or json.dumps(called)[:80]}"
    return "no tool call in time"


def main(claude_args):
    FOLDERS.mkdir(parents=True, exist_ok=True)
    results = []
    for number, phrasing in enumerate(PHRASINGS, 1):
        name = f"phrasing-{number}"
        folder = FOLDERS / str(number)
        shutil.rmtree(folder, ignore_errors=True)
        folder.mkdir(parents=True)
        session("start", name, str(folder), "--permission-mode", "auto", *claude_args)
        trust(name)
        session("type", name, phrasing)
        called = first_call(name)
        session("stop", name)
        chose = called == f"Skill {WANTED}"
        results.append(chose)
        print(f"{'ok  ' if chose else 'MISS'} {phrasing!r}: {called}", flush=True)
        time.sleep(2)
    shutil.rmtree(FOLDERS, ignore_errors=True)
    print(f"{sum(results)} of {len(results)} chose {WANTED} first")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main(sys.argv[1:])
