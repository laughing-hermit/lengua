# Maps the base commit's proof ledger to the head's (scripts/ci/golden-diff.sh ledger): one sed
# command a line, applied to the base side only, to each `<program path> B<n>: ...` line.
#
# A step that moves programs adds a rename of their paths here, as round 2's step 8b would have
# (it moved to round 6), in the form
#   s|^plugin/skills/compile/assets/fast/|plugin/skills/compile/assets/x86_64/fast/|

# Round 2 moved `fill` and the thread code (`start_worker`, `join_worker`) out of main.s into
# lengua's own library. A ledger names the functions no run reached among main.s's own, so the
# base's names for these three have nothing to match at the head: they leave the base's lists.
s/, "\(fill\|start_worker\|join_worker\)"//g
s/\["\(fill\|start_worker\|join_worker\)", /[/g
s/\["\(fill\|start_worker\|join_worker\)"\]/[]/g
