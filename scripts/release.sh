#!/usr/bin/env bash
# Builds the release archive for this computer's CPU, dist/lengua-<version>-<cpu>-linux.tar.gz
# (x86_64 or aarch64): the plugin with its server built, install.sh and a short README. Then
# checks it as a user gets it: unpacked in a fresh folder, validated, and a program taken from its
# idea to done through its own server and run from a shell. Nothing is published or pushed.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)"
cpu="$(uname -m)"
# The program the archive is checked with: add, from one idea, compiled for each CPU.
case "$cpu" in
    x86_64)
        label="x86-64"
        example="$root/examples/add"
        ;;
    aarch64)
        label="ARM64"
        example="$root/tests/fixtures/linux-aarch64/add"
        ;;
    *)
        echo "release.sh builds lengua for Linux x86-64 or ARM64; this computer is $cpu." >&2
        exit 1
        ;;
esac
name="lengua-$version-$cpu-linux"
"$root/scripts/build-plugin.sh"

stage="$(mktemp -d)"
check="$(mktemp -d)"
trap 'rm -rf "$stage" "$check"' EXIT
mkdir -p "$stage/$name"
(cd "$root" && git ls-files plugin) | while read -r file; do
    install -D -m "$(stat -c %a "$root/$file")" "$root/$file" "$stage/$name/$file"
done
install -D -m 0755 "$root/plugin/server/lengua-mcp" "$stage/$name/plugin/server/lengua-mcp"
install -m 0755 "$root/scripts/install.sh" "$stage/$name/install.sh"
install -m 0644 "$root/LICENSE" "$stage/$name/LICENSE"
# The CPU the server is built for, which install.sh holds the computer to.
printf '%s\n' "$cpu" > "$stage/$name/CPU"
cat > "$stage/$name/README.md" <<EOF
# lengua $version for Linux $label

lengua is an AI language: an agent writes a program's idea in plain words, compiles it to
assembly for this computer, and lengua checks, tests and proves what it compiled.

Install it with \`./install.sh\`, which checks what lengua needs (glibc 2.39 or later, LLVM's
assembler and linker, and for proofs the z3 and cvc5 solvers), says how to install anything
missing, copies the plugin and says how to load it. \`./install.sh --check\` only checks, and
\`./install.sh --link\` also puts \`lengua-mcp\` on your PATH, to run programs from a shell.
EOF

mkdir -p "$root/dist"
tar -C "$stage" -czf "$root/dist/$name.tar.gz" "$name"
(cd "$root/dist" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")

# As a user gets it.
tar -C "$check" -xzf "$root/dist/$name.tar.gz"
unpacked="$check/$name"
test "$("$unpacked/plugin/server/lengua-mcp" --version)" = "lengua-mcp $version"
"$unpacked/install.sh" --check
"$unpacked/install.sh" --prefix="$check/installed" > /dev/null
test -x "$check/installed/server/lengua-mcp"
if command -v claude > /dev/null; then
    claude plugin validate "$unpacked/plugin" --strict
fi
python3 "$root/scripts/release-check.py" "$unpacked/plugin/server/lengua-mcp" "$example"
echo "Built dist/$name.tar.gz ($(du -h "$root/dist/$name.tar.gz" | cut -f1))"
