#!/usr/bin/env bash
# For CI: prints, as Markdown, what the tests skipped, from the log lengua::skiplog writes
# (LENGUA_SKIP_LOG): one row per reason, with the number of tests that skipped for it.
# Usage: bash scripts/skip-summary.sh skips.log
set -u

log="${1:-skips.log}"
echo "### What the tests skipped"
echo
if [ -s "$log" ]; then
  echo "| tests | because |"
  echo "|---:|---|"
  # A line is "reason<TAB>test"; a test that notes the same reason twice counts once.
  sort -u "$log" | cut -f1 | sort | uniq -c | sort -rn \
    | awk '{ n = $1; $1 = ""; sub(/^ /, ""); printf "| %s | %s |\n", n, $0 }'
else
  echo "Nothing: no test skipped itself."
fi
