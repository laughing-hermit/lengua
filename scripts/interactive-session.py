#!/usr/bin/env python3
"""Drives a real interactive Claude Code session in tmux, the way a person would use lengua:
start it in a program folder with the plugin, type a request, wait until the screen settles,
read the screen, answer a prompt. No hooks: the session is the product as people use it.

  scripts/interactive-session.py start NAME FOLDER [CLAUDE ARGS...]
  scripts/interactive-session.py type NAME TEXT       # types TEXT and presses Enter
  scripts/interactive-session.py keys NAME KEY...     # presses keys, such as Down Enter
  scripts/interactive-session.py wait NAME [SECONDS]  # until the screen is still that long
  scripts/interactive-session.py screen NAME [LINES]  # prints the screen and recent scrollback
  scripts/interactive-session.py stop NAME
"""

import hashlib
import pathlib
import subprocess
import sys
import time

PLUGIN = pathlib.Path(__file__).resolve().parent.parent / "plugin"


def tmux(*args, check=True):
    return subprocess.run(["tmux", *args], check=check, capture_output=True, text=True).stdout


def screen(name, lines=0):
    start = ["-S", f"-{lines}"] if lines else []
    return tmux("capture-pane", "-p", "-t", name, *start)


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    command, name = argv[1], argv[2]
    if command == "start":
        folder = pathlib.Path(argv[3]).resolve()
        folder.mkdir(parents=True, exist_ok=True)
        claude = ["claude", "--plugin-dir", str(PLUGIN), *argv[4:]]
        tmux("new-session", "-d", "-s", name, "-x", "200", "-y", "50", "-c", str(folder),
             " ".join(f"'{part}'" for part in claude))
    elif command == "type":
        tmux("send-keys", "-t", name, "-l", argv[3])
        time.sleep(0.3)
        tmux("send-keys", "-t", name, "Enter")
    elif command == "keys":
        tmux("send-keys", "-t", name, *argv[3:])
    elif command == "wait":
        still = float(argv[3]) if len(argv) > 3 else 8.0
        last, since = None, time.monotonic()
        while True:
            digest = hashlib.sha256(screen(name).encode()).hexdigest()
            if digest != last:
                last, since = digest, time.monotonic()
            elif time.monotonic() - since >= still:
                break
            time.sleep(0.5)
        print(screen(name))
    elif command == "screen":
        print(screen(name, int(argv[3]) if len(argv) > 3 else 0))
    elif command == "stop":
        tmux("kill-session", "-t", name, check=False)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
