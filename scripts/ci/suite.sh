#!/usr/bin/env bash
# For CI: runs lengua's suite. It is the one definition of it: ci.yml's test job runs it on the
# runners, and scripts/ci/guest.sh runs it inside WSL 2 and a Lima VM (routes.yml), so the two
# can't drift (tests/parity_inventory.rs holds them to it).
# Usage: bash scripts/ci/suite.sh, after the tests are built.
# Reads TEST_FILES and FILTER (ci.yml's inputs), STALL_SECONDS and the LENGUA_* settings; writes
# test.log; fails when the suite fails or when what was asked for matched no test.
set -eu

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LENGUA_PROOF_SECONDS="${LENGUA_PROOF_SECONDS:-60}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"
export FILTER="${FILTER:-}"
test_files="${TEST_FILES:-}"

# shellcheck disable=SC2086 # a list of file names
selection=$(bash "$here/test-selection.sh" $test_files)
tests='set -o pipefail; cargo test --release --locked --workspace --no-fail-fast '"$selection"' -- $FILTER 2>&1 | tee test.log'
if [ -n "${STALL_SECONDS:-}" ]; then
  bash "$here/../stall-report.sh" "$STALL_SECONDS" bash -c "$tests"
else
  bash -c "$tests"
fi
bash "$here/ran-some-tests.sh" test.log "$FILTER$test_files"
