#!/usr/bin/env bash
# For routes.yml: builds the named test files and prints their executables, one per line, so that
# another machine (an emulated VM) can run them without building them.
# Usage: bash scripts/ci/test-binaries.sh <test file>...
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
selection=$(bash "$here/test-selection.sh" "$@")
# shellcheck disable=SC2086 # a list of cargo arguments
cargo test --release --locked $selection --no-run --message-format=json \
  | jq -r 'select(.profile.test == true and .executable != null) | .executable'
