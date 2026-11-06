#!/usr/bin/env bash
# For CI: installs what a job needs, the same way on both runners (Linux on x86-64 and on ARM64).
# Usage: bash scripts/ci/setup.sh <target> <step>...
#
# Steps:
#   rust     the pinned Rust with clippy and rustfmt, first on PATH
#   llvm     LLVM 22, for lengua's assembler and linker, from apt.llvm.org
#   solvers  z3 and cvc5, pinned by SHA-256 in tools.tsv
set -euo pipefail

RUST_VERSION="1.99.1"
LLVM_VERSION="22"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
target="${1:?usage: setup.sh <target> <step>...}"
shift
case "$target" in
  linux-x86_64 | linux-aarch64) ;;
  *)
    echo "setup.sh: $target isn't a lengua target (linux-x86_64 or linux-aarch64)" >&2
    exit 1
    ;;
esac

# Puts a folder first on PATH, for this script and for the job's later steps.
add_to_path() {
  export PATH="$1:$PATH"
  if [ -n "${GITHUB_PATH:-}" ]; then
    echo "$1" >> "$GITHUB_PATH"
  fi
}

rust() {
  if ! command -v rustup > /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain none
    add_to_path "$HOME/.cargo/bin"
  fi
  rustup toolchain install "$RUST_VERSION" --profile minimal --component clippy,rustfmt
  rustup default "$RUST_VERSION"
  rustc --version --verbose
}

llvm() {
  wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key \
    | sudo tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc > /dev/null
  echo "deb http://apt.llvm.org/noble/ llvm-toolchain-noble-$LLVM_VERSION main" \
    | sudo tee /etc/apt/sources.list.d/llvm.list
  sudo apt-get update -q
  sudo apt-get install -y -q "llvm-$LLVM_VERSION" "lld-$LLVM_VERSION"
  add_to_path "/usr/lib/llvm-$LLVM_VERSION/bin"
  llvm-mc --version | head -2
  ld.lld --version
}

# Checks a file against its SHA-256.
check_sha() {
  echo "$2  $1" | sha256sum -c -
}

solvers() {
  dest="$HOME/solvers"
  if [ ! -e "$dest/bin/cvc5" ]; then
    rm -rf "$dest"
    mkdir -p "$dest/bin"
    for tool in z3 cvc5; do
      row="$(awk -F'\t' -v t="$tool" -v g="$target" '$1 == t && $2 == g { print $3 " " $4 }' "$here/tools.tsv")"
      [ -n "$row" ] || { echo "tools.tsv has no $tool for $target" >&2; exit 1; }
      url="${row%% *}"
      sha="${row##* }"
      mkdir -p "$dest/download/$tool"
      (
        cd "$dest/download/$tool"
        curl -sSfL -o "$tool.zip" "$url"
        check_sha "$tool.zip" "$sha"
        unzip -q "$tool.zip"
      )
      bin="$(find "$dest/download/$tool" -type f -name "$tool" | head -1)" || true
      [ -n "$bin" ] || { echo "no $tool binary in the $tool archive" >&2; exit 1; }
      if [ "$tool" = z3 ]; then
        cp -R "$(dirname "$bin")/." "$dest/bin/"
      else
        cp "$bin" "$dest/bin/"
      fi
    done
    rm -rf "$dest/download"
    chmod +x "$dest/bin/"* 2> /dev/null || true
  fi
  add_to_path "$dest/bin"
  z3 --version
  cvc5 --version | head -1
}

for step in "$@"; do
  "$step"
done
