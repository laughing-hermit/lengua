#!/usr/bin/env bash
# For CI: installs what a job needs, the same way on all six runners (it runs under Git Bash on
# Windows). Usage: bash scripts/ci/setup.sh <target> <step>...
#
# Steps:
#   rust     the pinned Rust with clippy and rustfmt, first on PATH
#   llvm     LLVM 22, for lengua's assembler and linker: apt.llvm.org on Linux, conda-forge's build
#            (installed with micromamba, pinned in tools.tsv) on macOS and Windows
#   solvers  z3 and cvc5, pinned by SHA-256 in tools.tsv
set -euo pipefail

RUST_VERSION="1.99.1"
LLVM_VERSION="22"
LLVM_FULL="22.1.8"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
target="${1:?usage: setup.sh <target> <step>...}"
shift

# Puts a folder first on PATH, for this script and for the job's later steps.
add_to_path() {
  export PATH="$1:$PATH"
  if [ -n "${GITHUB_PATH:-}" ]; then
    case "$target" in
      windows-*) cygpath -w "$1" >> "$GITHUB_PATH" ;;
      *) echo "$1" >> "$GITHUB_PATH" ;;
    esac
  fi
}

rust() {
  if ! command -v rustup > /dev/null; then
    case "$target" in
      windows-*)
        arch=x86_64
        [ "$target" = windows-aarch64 ] && arch=aarch64
        curl -sSfL -o rustup-init.exe \
          "https://static.rust-lang.org/rustup/dist/$arch-pc-windows-msvc/rustup-init.exe"
        ./rustup-init.exe -y --profile minimal --default-toolchain none
        ;;
      *)
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
          | sh -s -- -y --profile minimal --default-toolchain none
        ;;
    esac
    add_to_path "$HOME/.cargo/bin"
  fi
  rustup toolchain install "$RUST_VERSION" --profile minimal --component clippy,rustfmt
  rustup default "$RUST_VERSION"
  rustc --version --verbose
  # The ring crate builds its assembly with clang on Windows ARM64: the image's LLVM has it.
  if [ "$target" = windows-aarch64 ] && [ -d "/c/Program Files/LLVM/bin" ]; then
    add_to_path "/c/Program Files/LLVM/bin"
    clang --version | head -1
  fi
}

llvm() {
  case "$target" in
    linux-*)
      wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key \
        | sudo tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc > /dev/null
      echo "deb http://apt.llvm.org/noble/ llvm-toolchain-noble-$LLVM_VERSION main" \
        | sudo tee /etc/apt/sources.list.d/llvm.list
      sudo apt-get update -q
      sudo apt-get install -y -q "llvm-$LLVM_VERSION" "lld-$LLVM_VERSION"
      add_to_path "/usr/lib/llvm-$LLVM_VERSION/bin"
      llvm-mc --version | head -2
      ld.lld --version
      ;;
    *)
      prefix="$HOME/llvm"
      if [ ! -d "$prefix" ]; then
        row="$(awk -F'\t' -v g="$target" '$1 == "micromamba" && $2 == g { print $3 " " $4 }' "$here/tools.tsv")"
        [ -n "$row" ] || { echo "tools.tsv has no micromamba for $target" >&2; exit 1; }
        mkdir -p "$HOME/micromamba"
        exe="$HOME/micromamba/micromamba"
        case "$target" in windows-*) exe="$exe.exe" ;; esac
        curl -sSfL -o "$exe" "${row%% *}"
        check_sha "$exe" "${row##* }"
        chmod +x "$exe"
        where="$prefix"
        case "$target" in windows-*) where="$(cygpath -w "$prefix")" ;; esac
        "$exe" create -y -p "$where" --override-channels -c conda-forge \
          "lld=$LLVM_FULL" "llvm-tools=$LLVM_FULL"
      fi
      # conda puts a system's programs in bin, and Windows's in Library/bin.
      for dir in "$prefix/bin" "$prefix/Library/bin"; do
        [ -d "$dir" ] && add_to_path "$dir"
      done
      llvm-mc --version | head -2
      ld.lld --version
      ;;
  esac
}

# Checks a file against its SHA-256.
check_sha() {
  if command -v sha256sum > /dev/null; then
    echo "$2  $1" | sha256sum -c -
  else
    echo "$2  $1" | shasum -a 256 -c -
  fi
}

# Unpacks a zip into the current folder.
unzip_here() {
  if command -v unzip > /dev/null; then
    unzip -q "$1"
  elif command -v 7z > /dev/null; then
    7z x -y "$1" > /dev/null
  else
    powershell -NoProfile -Command "Expand-Archive -Force -Path '$1' -DestinationPath ."
  fi
}

solvers() {
  dest="$HOME/solvers"
  if [ ! -e "$dest/bin/cvc5" ] && [ ! -e "$dest/bin/cvc5.exe" ]; then
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
        unzip_here "$tool.zip"
      )
      bin="$(find "$dest/download/$tool" -type f \( -name "$tool" -o -name "$tool.exe" \) | head -1)" || true
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
