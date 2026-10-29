#!/usr/bin/env python3
"""Drives a real interactive Claude Code session the way a person uses lengua: start it in a
program folder with the plugin, type a request, wait until it settles, read what the agent said,
answer. The session runs in a pseudo-terminal owned by a small background process; commands
reach it through a file, and the agent's words come from the session's own transcript. No hooks:
the session is the product as people use it.

  scripts/interactive-session.py start NAME FOLDER [CLAUDE ARGS...]
  scripts/interactive-session.py type NAME TEXT       # types TEXT and presses Enter
  scripts/interactive-session.py keys NAME KEY...     # Up, Down, Enter, Escape, Tab or text
  scripts/interactive-session.py wait NAME [SECONDS]  # until still that long; prints the reply
  scripts/interactive-session.py screen NAME [CHARS]  # the end of the screen, escapes removed
  scripts/interactive-session.py transcript NAME      # the session transcript's path
  scripts/interactive-session.py stop NAME
"""

import json
import os
import pathlib
import re
import subprocess
import sys
import time

PLUGIN = pathlib.Path(__file__).resolve().parent.parent / "plugin"
STATE = pathlib.Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp")) / "lengua-sessions"
KEYS = {"Up": "\x1b[A", "Down": "\x1b[B", "Right": "\x1b[C", "Left": "\x1b[D",
        "Enter": "\r", "Escape": "\x1b", "Tab": "\t", "Space": " "}


def folder_of(name):
    return STATE / name


def clean(text):
    """Terminal output as plain text: cursor steps right become spaces, escapes go."""
    text = re.sub(r"\x1b\[(\d*)C", lambda m: " " * int(m.group(1) or 1), text)
    text = re.sub(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(\x07|\x1b\\)|\x1b[()][A-Z0-9]|"
                  r"\x1b[=>]", "", text)
    return text.replace("\r", "")


def daemon(name, folder, args):
    import pexpect
    state = folder_of(name)
    commands = state / "commands"
    commands.touch()
    # A session of its own, as a person starts it: nothing of the session that drives it.
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("CLAUDE", "CLAUDECODE"))}
    child = pexpect.spawn("claude", ["--plugin-dir", str(PLUGIN), *args], cwd=folder,
                          dimensions=(50, 160), encoding="utf-8", timeout=None,
                          env={**env, "TERM": "xterm-256color"})
    screen, done = "", 0
    while child.isalive():
        try:
            screen = (screen + child.read_nonblocking(65536, timeout=0.2))[-200_000:]
            (state / "screen").write_text(clean(screen))
        except pexpect.TIMEOUT:
            pass
        except pexpect.EOF:
            break
        lines = commands.read_text().splitlines()
        for line in lines[done:]:
            kind, _, rest = line.partition("\t")
            if kind == "type":
                child.send(rest)
                # Long text arrives like a paste; Enter right after it would join the paste.
                time.sleep(0.6 + len(rest) / 150)
                child.send("\r")
            elif kind == "keys":
                for key in rest.split("\t"):
                    child.send(KEYS.get(key, key))
                    time.sleep(0.2)
            elif kind == "stop":
                child.terminate(force=True)
        done = len(lines)
    (state / "ended").touch()


def command(name, *parts):
    with open(folder_of(name) / "commands", "a") as commands:
        commands.write("\t".join(parts) + "\n")


def transcript(folder):
    projects = pathlib.Path.home() / ".claude" / "projects"
    encoded = re.sub(r"[^A-Za-z0-9]", "-", str(pathlib.Path(folder).resolve()))
    found = sorted((projects / encoded).glob("*.jsonl"), key=lambda path: path.stat().st_mtime)
    return found[-1] if found else None


def last_reply(path):
    """The agent's last words in the transcript, and the tools it called since the last user
    message."""
    if path is None:
        return "", []
    words, tools = "", []
    for line in path.read_text().splitlines():
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = entry.get("message") or {}
        content = message.get("content")
        if entry.get("type") == "user" and isinstance(content, str):
            tools = []
        if entry.get("type") != "assistant" or not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") == "text" and block.get("text", "").strip():
                words = block["text"]
            elif block.get("type") == "tool_use":
                tools.append(block.get("name", "?"))
    return words, tools


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    action, name = argv[1], argv[2]
    state = folder_of(name)
    if action == "start":
        folder = pathlib.Path(argv[3]).resolve()
        folder.mkdir(parents=True, exist_ok=True)
        state.mkdir(parents=True, exist_ok=True)
        for old in state.iterdir():
            old.unlink()
        (state / "folder").write_text(str(folder))
        subprocess.Popen([sys.executable, __file__, "daemon", name, str(folder), *argv[4:]],
                         start_new_session=True, stdout=subprocess.DEVNULL,
                         stderr=open(state / "daemon.log", "w"))
    elif action == "daemon":
        daemon(name, argv[3], argv[4:])
    elif action == "type":
        command(name, "type", argv[3])
    elif action == "keys":
        command(name, "keys", *argv[3:])
    elif action == "wait":
        still = float(argv[3]) if len(argv) > 3 else 10.0
        folder = (state / "folder").read_text()
        time.sleep(2)
        last, since = None, time.monotonic()
        while not (state / "ended").exists():
            path = transcript(folder)
            mark = ((state / "screen").stat().st_mtime if (state / "screen").exists() else 0,
                    path.stat().st_size if path else 0)
            if mark != last:
                last, since = mark, time.monotonic()
            elif time.monotonic() - since >= still:
                break
            time.sleep(0.5)
        words, tools = last_reply(transcript(folder))
        print(f"tools: {', '.join(tools) or 'none'}\n---\n{words}")
    elif action == "screen":
        chars = int(argv[3]) if len(argv) > 3 else 2500
        print((state / "screen").read_text()[-chars:])
    elif action == "transcript":
        print(transcript((state / "folder").read_text()))
    elif action == "stop":
        command(name, "stop")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
