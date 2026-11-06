#!/usr/bin/env bash
# For routes.yml: runs lengua's suite inside a guest Linux, Ubuntu 24.04 in WSL 2 or in a Lima VM,
# the way ci.yml's test job runs it on a runner. Run it from a clone on the guest's own disk, as a
# user with passwordless sudo (not root: the runners' user isn't either).
# Usage: bash scripts/ci/guest.sh <target>
# TEST_FILES and FILTER select tests as ci.yml's inputs do; the exit status is the suite's.
set -euo pipefail

target="${1:?usage: guest.sh <target>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

# What a runner image has and a fresh guest doesn't.
sudo apt-get update -q
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q \
  build-essential ca-certificates curl git unzip wget

bash scripts/ci/machine.sh "$target"

# setup.sh puts its folders on PATH through $GITHUB_PATH on a runner, and through a file here.
path_file="$(mktemp)"
SETUP_PATH_FILE="$path_file" bash scripts/ci/setup.sh "$target" rust llvm solvers
while read -r dir; do
  PATH="$dir:$PATH"
done < "$path_file"
export PATH

# Built first, as on the runners, so that the tests' time is the tests' and not the compiler's.
cargo test --release --locked --workspace --no-run

export LENGUA_PROOF_SECONDS=60 RUST_TEST_THREADS=2 LENGUA_SKIP_LOG="$root/skips.log"
test_files="${TEST_FILES:-}"
filter="${FILTER:-}"
# shellcheck disable=SC2086 # the selection and the filter are lists of words
selection=$(bash scripts/ci/test-selection.sh $test_files)
status=0
# shellcheck disable=SC2086
cargo test --release --locked --workspace --no-fail-fast $selection -- $filter 2>&1 \
  | tee test.log || status=$?
bash scripts/ci/ran-some-tests.sh test.log "$filter$test_files" || status=1
bash scripts/skip-summary.sh skips.log || true
exit "$status"
