#!/usr/bin/env bash
# Usage: golden-diff.sh raw|ledger <base dir> <head dir>
#
# Compares the goldens recorded at a base commit with those recorded at the head (ci.yml's golden
# mode). Each dir holds the four recordings, somewhere beneath it: checker-x86.txt,
# checker-aarch64.txt, prover-x86.txt and prover-aarch64.txt (src/checker/golden.rs and
# src/prove/golden.rs write them under target/golden/).
#
# - raw: the four files must be identical, byte for byte. This is the view while no program
#   changes, so that a change to the checker or the prover that moves any report or proof shows.
# - ledger: only the prover's proofs, one line per program and block, with what changes when a
#   program is rewritten left out (see `ledger` below). This is the view after programs change.
#
# scripts/ci/golden-expected.txt lists, one extended regular expression a line, the lines to leave
# out of both sides before comparing (a difference that is expected and understood).
# scripts/ci/golden-rename.sed renames the base's programs to the head's, for the ledger.
#
# Exit: 0 when the two agree; 1 when they differ or a recording is missing; 2 when the only
# differences are proofs that ran out of time on one side (the machine's speed, not the code):
# run the legs again.
set -eu

view=$1
base=$2
head=$3
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The expected-difference patterns, without comments and blank lines.
grep -v -E '^[[:space:]]*(#|$)' "$here/golden-expected.txt" > "$work/expected" || true

# The recording called $2 beneath dir $1 (artifacts download into a folder of their own).
recording() {
  find "$1" -name "$2" -type f | head -n 1
}

# $1 without the lines the expected-difference patterns match.
without_expected() {
  if [ -s "$work/expected" ]; then
    grep -v -E -f "$work/expected" "$1" || true
  else
    cat "$1"
  fi
}

# The proof ledger of a prover recording: for each program (a `== path` line) and each generated
# block (a `  B<n> proofs: ...` line), one `path B<n>: verdict` line. A verdict's counts are left
# out (shapes, paths, questions, the induction's reach and lookback): they change with every
# rewrite of a program, and say nothing of whether it is proven. So are the words of a bound's
# `why` (they name functions and print addresses) and hexadecimal numbers. What stays is what a
# proof covers: the kind of verdict, the bound's bytes, the cut, the induction, and which
# functions no run reached.
ledger() {
  awk '
    /^== / { program = substr($0, 4); next }
    /^  B[0-9]+ proofs: / {
      line = $0
      block = $1
      sub(/^  B[0-9]+ proofs: /, "", line)
      print program " " block ": " line
    }
  ' "$1" | sed -E \
    -e 's/[0-9]+ (shapes|paths|questions), //g' \
    -e 's/, (reach|lookback) [0-9]+//g' \
    -e 's/why: "([^"\\]|\\.)*"/why: _/g' \
    -e 's/0x[0-9a-fA-F]+/0x_/g'
}

status=0

case "$view" in
  raw)
    for name in checker-x86.txt checker-aarch64.txt prover-x86.txt prover-aarch64.txt; do
      b=$(recording "$base" "$name")
      h=$(recording "$head" "$name")
      if [ -z "$b" ] || [ -z "$h" ]; then
        echo "$name: a recording is missing (base: ${b:-none}, head: ${h:-none})"
        status=1
        continue
      fi
      without_expected "$b" > "$work/base.txt"
      without_expected "$h" > "$work/head.txt"
      if diff -u --label "base/$name" --label "head/$name" "$work/base.txt" "$work/head.txt" > "$work/diff.txt"; then
        echo "$name: identical, $(wc -l < "$work/head.txt") lines"
      else
        echo "$name: DIFFERS (the first 60 lines of the difference follow)"
        head -n 60 "$work/diff.txt"
        status=1
      fi
    done
    ;;
  ledger)
    for name in prover-x86.txt prover-aarch64.txt; do
      b=$(recording "$base" "$name")
      h=$(recording "$head" "$name")
      if [ -z "$b" ] || [ -z "$h" ]; then
        echo "$name: a recording is missing (base: ${b:-none}, head: ${h:-none})"
        status=1
        continue
      fi
      # Key and verdict, tab apart, so that the two sides join on the program and the block.
      ledger "$b" | sed -f "$here/golden-rename.sed" | without_expected /dev/stdin \
        | sed -E 's/^(.* B[0-9]+): /\1\t/' | sort > "$work/base.ledger"
      ledger "$h" | without_expected /dev/stdin \
        | sed -E 's/^(.* B[0-9]+): /\1\t/' | sort > "$work/head.ledger"
      awk -F'\t' -v name="$name" '
        # A proof that ran out of time says nothing of the code: it differs only in the machine.
        function unknown(text) { return text ~ /~ (out of time|proven as far as the time allowed)/ }
        NR == FNR { base[$1] = $2; next }
        { head[$1] = $2 }
        END {
          same = 0; differ = 0; rerun = 0; missing = 0
          for (key in base) {
            if (!(key in head)) { print name ": only in the base: " key; missing++; continue }
            if (base[key] == head[key]) { same++; continue }
            if (unknown(base[key]) || unknown(head[key])) {
              print name ": TIME " key; print "    base: " base[key]; print "    head: " head[key]
              rerun++
            } else {
              print name ": DIFFERS " key; print "    base: " base[key]; print "    head: " head[key]
              differ++
            }
          }
          for (key in head) if (!(key in base)) { print name ": only in the head: " key; missing++ }
          print name ": " same " blocks agree, " differ " differ, " rerun " ran out of time on one side, " missing " missing"
          code = 0
          if (differ + missing > 0) code = 1
          else if (rerun > 0) code = 2
          exit code
        }
      ' "$work/base.ledger" "$work/head.ledger" || status=$(( $? > status ? $? : status ))
    done
    ;;
  *)
    echo "usage: golden-diff.sh raw|ledger <base dir> <head dir>" >&2
    exit 64
    ;;
esac

exit "$status"
