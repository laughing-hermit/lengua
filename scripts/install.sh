#!/bin/sh
# Installs lengua from its release archive, on Linux or macOS: checks that this computer is the
# system and CPU the archive is for (its TARGET), copies the plugin where Claude Code finds
# plugins, a marketplace folder of lengua's own, which it adds to Claude Code when the claude
# command is installed (and says the commands that do, when it isn't), then runs
# lengua-mcp doctor, which says whether lengua has what it needs here. The archive ships every
# part lengua needs, the solvers included, so nothing else is installed. Its exit code is
# doctor's: 0 when nothing is wrong.
#
# usage: install.sh [--check] [--link] [--prefix=<folder>]
#   --check            only check this computer; install nothing (on macOS, it first clears
#                      from this folder the mark a browser puts on a download, under which
#                      macOS won't start the server)
#   --link             also link lengua-mcp into ~/.local/bin, to run programs from a shell
#   --prefix=<folder>  make <folder> lengua's marketplace folder, the plugin in its lengua/
#                      folder, instead of lengua's own; what it replaces there must be lengua's
#                      (an earlier install), or not there yet
set -eu

here=$(cd "$(dirname "$0")" && pwd)
check_only=false
link=false
prefix=""
for arg in "$@"; do
    case "$arg" in
        --check) check_only=true ;;
        --link) link=true ;;
        --prefix=*) prefix="${arg#--prefix=}" ;;
        *)
            echo "usage: install.sh [--check] [--link] [--prefix=<folder>]" >&2
            exit 2
            ;;
    esac
done

# A target's name as people say it.
described() {
    case "$1" in
        linux-x86_64) echo "Linux on x86-64" ;;
        linux-aarch64) echo "Linux on ARM64" ;;
        macos-aarch64) echo "macOS on ARM64 (Apple silicon)" ;;
        macos-x86_64) echo "macOS on x86-64 (an Intel Mac)" ;;
        windows-x86_64) echo "Windows on x86-64" ;;
        windows-aarch64) echo "Windows on ARM64" ;;
        *) echo "$1" ;;
    esac
}

target=$(cat "$here/TARGET" 2>/dev/null || true)
if [ -z "$target" ]; then
    echo "This folder has no TARGET file: install lengua from its release archive, unpacked." >&2
    exit 1
fi
version=$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$here/plugin.json" | head -n 1)

case "$(uname -s)" in
    Linux)
        system=linux
        cpu=$(uname -m)
        ;;
    Darwin)
        system=macos
        # A shell under Rosetta says x86_64: the hardware's own answer is hw.optional.arm64.
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ]; then
            cpu=aarch64
        else
            cpu=x86_64
        fi
        ;;
    # A Unix shell on Windows (Git Bash, MSYS2, Cygwin), which no archive with install.sh is for.
    MINGW* | MSYS* | CYGWIN*)
        echo "This archive is lengua for $(described "$target"), and this computer is Windows:" \
            "use the archive for Windows on its CPU (windows-x86_64 or windows-aarch64), and" \
            "its install.ps1, in PowerShell." >&2
        exit 1
        ;;
    *)
        system=$(uname -s)
        cpu=$(uname -m)
        ;;
esac
if [ "$cpu" = arm64 ]; then
    cpu=aarch64
fi
computer="$system-$cpu"
if [ "$computer" != "$target" ]; then
    case "$computer" in
        linux-x86_64 | linux-aarch64 | macos-aarch64)
            echo "This archive is lengua for $(described "$target"), and this computer is" \
                "$(described "$computer"): use the archive for $computer, with its install.sh." >&2
            ;;
        *)
            echo "This archive is lengua for $(described "$target"), and this computer is" \
                "$(described "$computer"), which lengua isn't built for: it is built for" \
                "linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64 and windows-aarch64." >&2
            ;;
    esac
    exit 1
fi

# Linux's server and solvers are built against glibc 2.39: Ubuntu 24.04, Debian 13, Fedora 40
# and later.
if [ "$system" = linux ]; then
    glibc=$(ldd --version 2>/dev/null | head -n 1 | grep -o '[0-9][0-9]*\.[0-9][0-9]*$' || true)
    if [ -z "$glibc" ]; then
        echo "lengua needs glibc 2.39 or later, and this computer's C library isn't glibc." >&2
        exit 1
    fi
    major=${glibc%%.*}
    minor=${glibc#*.}
    if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 39 ]; }; then
        echo "lengua needs glibc 2.39 or later, and this computer has $glibc." >&2
        exit 1
    fi
fi

# A browser marks what it downloads, and macOS won't start an unsigned program so marked. The
# archive isn't signed, so whether to trust it is the user's choice: this says how, and changes
# nothing itself.
quarantine_note() {
    if [ "$system" = macos ] && command -v xattr >/dev/null 2>&1 \
        && xattr -r "$1" 2>/dev/null | grep -q com.apple.quarantine; then
        echo "macOS marks these files as downloaded, and won't start the unsigned server so marked."
        echo "If you trust this archive, clear the mark with:"
        echo "  xattr -dr com.apple.quarantine '$1'"
        echo
    fi
}

if [ "$check_only" = true ]; then
    quarantine_note "$here"
    echo "This computer is $(described "$computer"), which this archive is for."
    echo
    exec "$here/server/lengua-mcp" doctor
fi

# lengua's marketplace folder: the plugin in lengua/, and .claude-plugin/marketplace.json,
# which names it for Claude Code.
case "$system" in
    macos) data="$HOME/Library/Application Support" ;;
    *) data="${XDG_DATA_HOME:-$HOME/.local/share}" ;;
esac
market="${prefix:-$data/lengua/plugins}"
dest="$market/lengua"
marketplace="$market/.claude-plugin/marketplace.json"
# What the install replaces must be lengua's: an earlier install, which holds its TARGET file,
# and the marketplace.json that names lengua, or nothing yet.
for folder in "$dest" "$dest.new"; do
    if [ -e "$folder" ] && [ ! -f "$folder/TARGET" ]; then
        echo "$folder is there and isn't lengua (it has no TARGET file), so the install won't" \
            "replace it: give --prefix a folder of lengua's own, or move $folder away." >&2
        exit 1
    fi
done
if [ -e "$marketplace" ] \
    && [ "$(sed -n 's/^ *"name": *"\([^"]*\)".*/\1/p' "$marketplace" | head -n 1)" != lengua ]; then
    echo "$marketplace is another marketplace's, so the install won't replace it: give" \
        "--prefix a folder of lengua's own." >&2
    exit 1
fi
mkdir -p "$market/.claude-plugin"
rm -rf "$dest.new"
cp -R "$here" "$dest.new"
rm -f "$dest.new/install.sh" "$dest.new/install.ps1" "$dest.new/INSTALL.md"
rm -rf "$dest"
mv "$dest.new" "$dest"
quarantine_note "$dest"
cat > "$marketplace" <<EOF
{
  "name": "lengua",
  "owner": {
    "name": "lengua"
  },
  "plugins": [
    {
      "name": "lengua",
      "source": "./lengua",
      "version": "$version"
    }
  ]
}
EOF
server="$dest/server/lengua-mcp"
if [ "$link" = true ]; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$server" "$HOME/.local/bin/lengua-mcp"
fi

echo "Installed lengua $version for $(described "$target") in $dest."
echo
# Claude Code installs a plugin from a marketplace it knows: lengua's, added once and updated
# after.
registered=false
if command -v claude >/dev/null 2>&1; then
    if claude plugin marketplace add "$market" >/dev/null 2>&1 \
        || claude plugin marketplace update lengua >/dev/null 2>&1; then
        if claude plugin update lengua@lengua >/dev/null 2>&1 \
            || claude plugin install lengua@lengua >/dev/null 2>&1; then
            registered=true
        fi
    fi
fi
if [ "$registered" = true ]; then
    echo "Claude Code has lengua now: start a new session, or run /reload-plugins in one."
else
    echo "Add it to Claude Code:"
    echo "    claude plugin marketplace add '$market'"
    echo "    claude plugin install lengua@lengua"
    echo "or load it for one session: claude --plugin-dir '$dest'"
fi
echo "Other agent harnesses load the plugin folder $dest."
echo "Then describe a program: \"Use lengua to write a program that ...\"."
echo
echo "Run a program it built from a shell, script or service:"
if [ "$link" = true ]; then
    echo "    lengua-mcp run <program folder> -- <arguments>"
else
    echo "    '$server' run <program folder> -- <arguments>"
fi
echo
"$server" doctor
