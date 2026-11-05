#!/usr/bin/env bash
# For CI only, on a runner: runs a command, and if it is still running after SECONDS, writes where
# it is stuck to stall.txt (the busiest processes, and a stack of every thread of the server, the
# test program and the solvers) and stops it, so a hang ends with its cause in the logs instead of
# at the job's time limit.
# Usage: bash scripts/stall-report.sh SECONDS command [arguments...]
set -u

limit="$1"
shift
"$@" &
job=$!

(
  sleep "$limit"
  kill -0 "$job" 2>/dev/null || exit 0
  {
    echo "still running after ${limit}s: $(date -u)"
    ps -eo pid,ppid,pcpu,etime,args --sort=-pcpu | head -30
    sudo apt-get install -y -q gdb > /dev/null 2>&1 || echo "gdb: not installed"
    for pid in $(pgrep -f 'lengua-mcp|/target/release/deps/|z3|cvc5'); do
      echo "=== $pid: $(tr '\0' ' ' < "/proc/$pid/cmdline")"
      sudo gdb -p "$pid" -batch -ex 'thread apply all bt 30' 2>&1 \
        | grep -v '^\[New LWP\|^warning:\|^\[Thread debugging' || true
    done
  } > stall.txt 2>&1
  pkill -f 'lengua-mcp' || true
  pkill -f '/target/release/deps/' || true
  kill "$job" 2>/dev/null || true
) &
watcher=$!

# A line of memory use every 10 seconds in the job's own log, so a machine that dies mid-run
# still says how it was going (the stall report above is lost with it).
(
  while sleep 10; do
    echo "memory: $(free -m | awk 'NR == 2 { print $3 " MB used, " $7 " MB available" }');" \
      "largest: $(ps -eo rss,etime,comm --sort=-rss | sed -n 2,4p | tr -s ' ' | tr '\n' ';')"
  done
) &
meter=$!

wait "$job"
status=$?
kill "$watcher" "$meter" 2>/dev/null || true
exit "$status"
