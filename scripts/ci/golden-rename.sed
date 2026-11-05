# Renames the base commit's programs to the head's, for the proof ledger
# (scripts/ci/golden-diff.sh ledger): one sed command a line, applied to the base side only, to
# the start of each `<program path> B<n>: ...` line.
#
# Empty until a step moves programs (round 2's step 8b splits the assets by CPU), as in
#   s|^plugin/skills/compile/assets/fast/|plugin/skills/compile/assets/x86_64/fast/|
