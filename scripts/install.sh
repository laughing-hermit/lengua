#!/bin/sh
# Installs lengua from its release archive: checks what lengua needs on this computer, says how
# to install anything missing, copies the plugin, and says how to use it.
#
# usage: install.sh [--check] [--link] [--prefix=<folder>]
#   --check            only check what lengua needs; install nothing
#   --link             also link lengua-mcp into ~/.local/bin, to run programs from a shell
#   --prefix=<folder>  install the plugin there instead of ~/.local/share/lengua/plugin
set -eu

here=$(cd "$(dirname "$0")" && pwd)
dest="${XDG_DATA_HOME:-$HOME/.local/share}/lengua/plugin"
check_only=false
link=false
for arg in "$@"; do
    case "$arg" in
        --check) check_only=true ;;
        --link) link=true ;;
        --prefix=*) dest="${arg#--prefix=}" ;;
        *)
            echo "usage: install.sh [--check] [--link] [--prefix=<folder>]" >&2
            exit 2
            ;;
    esac
done

cpu=$(uname -m)
if [ "$(uname -s)" != Linux ] || { [ "$cpu" != x86_64 ] && [ "$cpu" != aarch64 ]; }; then
    echo "lengua runs on Linux x86-64 and ARM64 for now; this is $(uname -s) $cpu." >&2
    exit 1
fi
# An archive's server is built for one CPU, which its CPU file names.
built=$(cat "$here/CPU" 2>/dev/null || echo "$cpu")
if [ "$built" != "$cpu" ]; then
    echo "This is lengua for Linux $built, and this computer is $cpu: use the $cpu archive." >&2
    exit 1
fi

# The server is built against glibc 2.39: Ubuntu 24.04, Debian 13, Fedora 40 and later.
glibc=$(ldd --version 2>/dev/null | head -n 1 | grep -o '[0-9][0-9]*\.[0-9][0-9]*$' || true)
if [ -z "$glibc" ]; then
    echo "lengua needs glibc 2.39 or later, and this computer's C library isn't glibc." >&2
    exit 1
fi
major=${glibc%%.*}
minor=${glibc#*.}
if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 39 ]; }; then
    echo "lengua needs glibc 2.39 or later; this computer has $glibc." >&2
    exit 1
fi

if command -v dnf >/dev/null 2>&1; then
    installer="sudo dnf install"
elif command -v apt-get >/dev/null 2>&1; then
    installer="sudo apt install"
else
    installer="your package manager's install for"
fi
needed=""
for tool in llvm-mc ld.lld; do
    command -v "$tool" >/dev/null 2>&1 || needed="$needed $tool"
done
solvers=""
for solver in z3 cvc5; do
    command -v "$solver" >/dev/null 2>&1 || solvers="$solvers $solver"
done
if [ -n "$needed" ]; then
    echo "lengua builds programs with LLVM's assembler and linker, and${needed} isn't installed:"
    echo "    $installer llvm lld"
fi
if [ -n "$solvers" ]; then
    echo "Proofs use the z3 and cvc5 solvers, and${solvers} isn't installed; programs are still"
    echo "checked and tested without them:"
    echo "    $installer z3 cvc5"
fi
if [ -n "$needed" ]; then
    exit 1
fi
if [ "$check_only" = true ]; then
    echo "This computer has what lengua needs."
    exit 0
fi

mkdir -p "$(dirname "$dest")"
rm -rf "$dest.new"
cp -R "$here/plugin" "$dest.new"
rm -rf "$dest"
mv "$dest.new" "$dest"
server="$dest/server/lengua-mcp"
if [ "$link" = true ]; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$server" "$HOME/.local/bin/lengua-mcp"
    server=lengua-mcp
fi

echo "Installed $("$dest/server/lengua-mcp" --version) in $dest."
echo
echo "Load it in Claude Code:"
echo "    claude --plugin-dir $dest"
echo "and describe a program: \"Use lengua to write a program that ...\"."
echo
echo "Run a program it built from a shell, script or service:"
echo "    $server run <program folder> -- <arguments>"
