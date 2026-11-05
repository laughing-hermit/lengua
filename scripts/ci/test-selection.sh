#!/usr/bin/env bash
# Usage: test-selection.sh [name ...]
#
# The cargo arguments that select test targets, for ci.yml's `test_files` input: `lib` is the
# library's own tests (--lib), any other name is a file under tests/ (--test <name>). Nothing
# given selects nothing, so every test target runs.
set -eu

for name in "$@"; do
  case "$name" in
    lib) printf '%s ' --lib ;;
    *) printf '%s ' --test "$name" ;;
  esac
done
