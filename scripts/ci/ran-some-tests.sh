#!/usr/bin/env bash
# Usage: ran-some-tests.sh <log> <selector>
#
# Fails a run whose filter or files matched no test. libtest filters match test *names* only, so a
# filter such as `limits` (a file's name) matches nothing and the run would pass green having
# run nothing. When <selector> is empty the whole suite ran and there is nothing to check;
# otherwise at least one test must have passed in <log>.
set -eu

log=$1
selector=${2:-}
[ -n "$selector" ] || exit 0

passed=$(grep -E '^test result: .* [0-9]+ passed' "$log" | awk '{ total += $4 } END { print total + 0 }')
echo "Tests that passed: $passed (selected by: $selector)"
if [ "$passed" -eq 0 ]; then
  echo "No test ran: what was asked for matched nothing. A filter matches test names, not files;" >&2
  echo "name a file in test_files." >&2
  exit 1
fi
