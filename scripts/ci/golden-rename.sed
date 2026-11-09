# Maps the base commit's proof ledger to the head's (scripts/ci/golden-diff.sh ledger): one sed
# command a line, applied to the base side only, to each `<program path> B<n>: ...` line.
#
# A step that moves programs adds a rename of their paths here, as W11's layout does below.

# W11 moved the WebAssembly programs out of tests/fixtures-wasm: the examples' twins and
# secret-equal to examples/, the others to tests/fixtures/. A base recorded before the move names
# them by their old folders.
s#^tests/fixtures-wasm/\(absdiff\|aes\|archive\|count\|dice\|fetch\|hypotenuse\|mean\|secret-equal\|sha256\|sort\|upcase\)/#examples/\1/#
s#^tests/fixtures-wasm/#tests/fixtures/#
