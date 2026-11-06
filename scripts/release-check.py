#!/usr/bin/env python3
"""Checks a lengua release as a user gets it: its own server takes a program from its idea to
done over MCP, then the program runs from a shell with the command the done reply gives.

usage: release-check.py <lengua-mcp> <example folder with idea.md and main.s>
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile

REQUEST = "Make me a program that adds two numbers"


def main() -> int:
    server, example = sys.argv[1], sys.argv[2]
    with tempfile.TemporaryDirectory() as data, tempfile.TemporaryDirectory() as work:
        folder = os.path.join(work, "add")
        os.mkdir(folder)
        process = subprocess.Popen(
            [server, "--data-dir", data],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        calls = 0

        def send(message: dict) -> None:
            process.stdin.write(json.dumps(message) + "\n")
            process.stdin.flush()

        def answer(number: int) -> dict:
            for line in process.stdout:
                message = json.loads(line)
                if message.get("id") == number:
                    return message
            raise RuntimeError("the server stopped answering")

        def check(arguments: dict) -> dict:
            nonlocal calls
            calls += 1
            send(
                {
                    "jsonrpc": "2.0",
                    "id": calls + 1,
                    "method": "tools/call",
                    "params": {"name": "check", "arguments": arguments},
                }
            )
            result = answer(calls + 1)["result"]
            if "structuredContent" in result:
                return result["structuredContent"]
            return json.loads(result["content"][0]["text"])

        send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "release-check", "version": "1"},
                },
            }
        )
        answer(1)
        send({"jsonrpc": "2.0", "method": "notifications/initialized"})

        shutil.copy(os.path.join(example, "idea.md"), folder)
        report = check({"path": folder, "request": REQUEST})
        if report["state"] != "write_code":
            raise RuntimeError(f"the idea wasn't recorded: {report}")
        shutil.copy(os.path.join(example, "main.s"), folder)
        while True:
            report = check({"path": folder})
            running = any(
                ": proving" in line or line.endswith("waiting for its turn")
                for line in report.get("proofs", [])
            )
            if not running and report["state"] not in ("testing", "proving"):
                break
        if report["state"] != "done":
            raise RuntimeError(f"the program isn't done: {report}")
        process.stdin.close()
        process.wait(timeout=30)

        ran = subprocess.run(
            report["shell"] + " 2 3", shell=True, capture_output=True, text=True, check=False
        )
        if ran.stdout != "5\n" or ran.returncode != 0:
            raise RuntimeError(f"the program didn't run from a shell: {ran}")
    print(f"The release took add from its idea to done in {calls} checks, and ran it from a shell.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
