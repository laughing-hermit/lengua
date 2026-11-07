#!/usr/bin/env bash
# Builds lengua's release archive for the system and CPU of the computer it runs on, one of the
# five lengua is built for: linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64 and
# windows-aarch64. It writes dist/lengua-<version>-<target>.tar.gz (.zip on Windows), and its
# SHA-256 beside it in the same name with .sha256 added.
#
# The archive is the plugin, ready to load: the skills and the manifests, server/lengua-mcp
# (.exe on Windows, which the manifests then start), solvers/ with the z3 and cvc5 that
# scripts/ci/tools.tsv pins for the target, LICENSE and licenses/ (the licences of everything
# it ships: lengua's, the solvers', and those of the Rust crates in lengua-mcp), TARGET (the
# system and CPU it is for), INSTALL.md, and install.sh (install.ps1 on Windows), which checks
# that the computer is that system and CPU and puts the plugin where Claude Code finds plugins.
# CI installs and checks each archive on a clean runner of its own system and CPU (release
# mode, .github/workflows/ci.yml). On Windows this runs under Git Bash, as CI's jobs do. It
# needs Rust, git and curl, and unzip, 7-Zip or PowerShell to unpack the solvers. Nothing is
# published, signed or pushed.
#
# usage: scripts/release.sh [<target>]   (a target given must be this computer's)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)"
tools="$root/scripts/ci/tools.tsv"

# The target is the one Rust builds for here.
host="$(rustc -vV | sed -n 's/^host: //p')"
case "$host" in
  x86_64-unknown-linux-gnu) target=linux-x86_64 described="Linux on x86-64" ;;
  aarch64-unknown-linux-gnu) target=linux-aarch64 described="Linux on ARM64" ;;
  aarch64-apple-darwin) target=macos-aarch64 described="macOS on ARM64 (Apple silicon)" ;;
  x86_64-pc-windows-msvc) target=windows-x86_64 described="Windows on x86-64" ;;
  aarch64-pc-windows-msvc) target=windows-aarch64 described="Windows on ARM64" ;;
  *)
    echo "lengua is built for linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64 and" \
      "windows-aarch64, and Rust here builds for $host." >&2
    exit 1
    ;;
esac
if [ -n "${1:-}" ] && [ "$1" != "$target" ]; then
  echo "release.sh builds the archive of the computer it runs on, $target, not $1." >&2
  exit 1
fi
case "$target" in
  windows-*) exe=".exe" ;;
  *) exe="" ;;
esac
name="lengua-$version-$target"

# A file's SHA-256.
sha256_of() {
  if command -v sha256sum > /dev/null; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# Checks the file $1 against the SHA-256 $2.
check_sha() {
  local found
  found="$(sha256_of "$1")"
  if [ "$found" != "$2" ]; then
    echo "$1's SHA-256 is $found, not the $2 that tools.tsv pins" >&2
    exit 1
  fi
}

# Unpacks the zip $1, named in the current folder, into the current folder.
unzip_here() {
  if command -v unzip > /dev/null; then
    unzip -q "$1"
  elif command -v 7z > /dev/null; then
    7z x -y "$1" > /dev/null
  else
    powershell -NoProfile -Command "Expand-Archive -Force -Path '$1' -DestinationPath ."
  fi
}

# lengua-mcp, built as a release: with none of the shell's RUSTFLAGS, so that it is the same
# wherever it is built, and on Windows with the C runtime linked in, so that it needs no Visual
# C++ redistributable (--target keeps that flag off build scripts and procedural macros).
unset CARGO_ENCODED_RUSTFLAGS
case "$target" in
  windows-*)
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --locked \
      --manifest-path "$root/Cargo.toml" --bin lengua-mcp --target "$host"
    built="$root/target/$host/release/lengua-mcp.exe"
    ;;
  *)
    RUSTFLAGS="" cargo build --release --locked --manifest-path "$root/Cargo.toml" \
      --bin lengua-mcp
    built="$root/target/release/lengua-mcp"
    ;;
esac

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
out="$stage/$name"
mkdir -p "$out"

# The plugin as the repository keeps it: the skills, the manifests, its README and LICENSE.
(cd "$root" && git ls-files -z plugin) | while IFS= read -r -d '' file; do
  kept="${file#plugin/}"
  mkdir -p "$out/$(dirname "$kept")"
  cp "$root/$file" "$out/$kept"
done
# On Windows the server is lengua-mcp.exe, and both manifests start it by that name.
if [ -n "$exe" ]; then
  for manifest in mcp.json .mcp.json; do
    sed 's#/server/lengua-mcp"#/server/lengua-mcp.exe"#' "$out/$manifest" > "$out/$manifest.new"
    mv "$out/$manifest.new" "$out/$manifest"
    if ! grep -q '/server/lengua-mcp\.exe"' "$out/$manifest"; then
      echo "$manifest doesn't start server/lengua-mcp" >&2
      exit 1
    fi
  done
fi
mkdir -p "$out/server"
cp "$built" "$out/server/lengua-mcp$exe"
chmod 755 "$out/server/lengua-mcp$exe"

# Copies the licences of the solver $1, whose release archive is unpacked in $2 and was
# downloaded from $3, into licenses/$1/: those its archive holds, by their paths below its one
# top folder, or, for an archive that holds none, those of its release's source, fetched from
# its tag, each with its SHA-256 in the log.
solver_licences() {
  local tool="$1" unpacked="$2" url="$3" dest="$out/licenses/$1" top="" count=0 file kept
  mkdir -p "$dest"
  local entries=("$unpacked"/*)
  if [ "${#entries[@]}" -eq 1 ] && [ -d "${entries[0]}" ]; then
    top="${entries[0]##*/}/"
  fi
  while IFS= read -r file; do
    file="${file#./}"
    kept="${file#"$top"}"
    mkdir -p "$dest/$(dirname "$kept")"
    cp "$unpacked/$file" "$dest/$kept"
    echo "$tool's licence $kept, from its archive"
    count=$((count + 1))
  done < <(cd "$unpacked" && find . -type f \( -iname 'license*' -o -iname 'licence*' \
    -o -iname 'copying*' -o -iname 'copyright*' -o -iname 'notice*' -o -path '*/licenses/*' \) \
    ! -path '*/python/*' ! -name '*.py' | sort)
  if [ "$count" -gt 0 ]; then
    return
  fi
  local repo tag source
  repo="$(sed -n 's#^https://github.com/\([^/]*/[^/]*\)/releases/download/.*#\1#p' <<< "$url")"
  tag="$(sed -n 's#.*/releases/download/\([^/]*\)/.*#\1#p' <<< "$url")"
  case "$tool" in
    z3) set -- LICENSE.txt ;;
    # cvc5's COPYING, and the licences it names of what a build links in.
    cvc5)
      set -- COPYING licenses/minisat-LICENSE licenses/lgpl-3.0.txt licenses/gpl-3.0.txt \
        licenses/GCC-exception-3.1.txt licenses/apache-2.0-with-llvm-exceptions.txt
      ;;
  esac
  for kept in "$@"; do
    source="https://raw.githubusercontent.com/$repo/$tag/$kept"
    mkdir -p "$dest/$(dirname "$kept")"
    curl -sSfL --retry 3 -o "$dest/$kept" "$source"
    echo "$tool's licence $kept, which its archive lacks, from $source: SHA-256 $(sha256_of "$dest/$kept")"
  done
}

# The solvers, each as tools.tsv pins it for the target, checked against its SHA-256: z3 with
# its own libraries beside it, as its archive keeps them, for a z3 that loads one.
mkdir -p "$out/solvers" "$out/licenses"
for tool in z3 cvc5; do
  row="$(awk -F'\t' -v t="$tool" -v g="$target" '$1 == t && $2 == g { print $3 " " $4 }' "$tools")"
  if [ -z "$row" ]; then
    echo "tools.tsv pins no $tool for $target" >&2
    exit 1
  fi
  url="${row%% *}"
  unpacked="$stage/download/$tool"
  mkdir -p "$unpacked"
  (
    cd "$unpacked"
    curl -sSfL --retry 3 -o "$tool.zip" "$url"
    check_sha "$tool.zip" "${row##* }"
    unzip_here "$tool.zip"
    rm "$tool.zip"
  )
  program="$(find "$unpacked" -type f \( -name "$tool" -o -name "$tool.exe" \) | head -n 1)" || true
  if [ -z "$program" ]; then
    echo "the $tool archive from $url holds no $tool program" >&2
    exit 1
  fi
  cp "$program" "$out/solvers/$tool$exe"
  chmod 755 "$out/solvers/$tool$exe"
  if [ "$tool" = z3 ]; then
    find "$(dirname "$program")" -maxdepth 1 -type f \
      \( -name 'libz3.so*' -o -name 'libz3.dylib' -o -name '*.dll' \) -exec cp {} "$out/solvers/" \;
  fi
  solver_licences "$tool" "$unpacked" "$url"
  tag="$(sed -n 's#.*/releases/download/\([^/]*\)/.*#\1#p' <<< "$url")"
  repo="$(sed -n 's#^\(https://github.com/[^/]*/[^/]*\)/releases/download/.*#\1#p' <<< "$url")"
  printf -v "${tool}_source" '%s' "$repo/tree/$tag"
done
echo "solvers/: $(cd "$out/solvers" && ls | tr '\n' ' ')"

# The licences of the Rust crates compiled into lengua-mcp for this target, from Cargo's own
# copies of their sources: each crate's name, version, licence and repository, then the licence
# files it ships. lengua's own two crates are under LICENSE.
registry="${CARGO_HOME:-$HOME/.cargo}/registry/src"
if [ -n "$exe" ]; then
  registry="$(cygpath -u "$registry")"
fi
{
  echo "The Rust crates compiled into lengua-mcp $version for $target: for each, a line"
  echo "\"== <crate> <version>: <licence> (<repository>)\", then the licence files the crate ships,"
  echo "each after a line \"-- <file>\"."
  cargo tree --manifest-path "$root/Cargo.toml" --locked --package lengua-mcp --target "$host" \
    --edges no-build,no-dev,no-proc-macro --prefix none --format '{p}|{l}|{r}' \
    | sed 's/ (\*)$//' | sort -u \
    | while IFS='|' read -r package licence repository; do
      crate="${package%% *}"
      crate_version="${package#* v}"
      crate_version="${crate_version%% *}"
      if [ "$crate" = lengua-mcp ] || [ "$crate" = lengua-os ]; then
        continue
      fi
      echo
      echo "== $crate $crate_version: ${licence:-no licence named} (${repository:-no repository named})"
      folder=""
      for candidate in "$registry"/*/"$crate-$crate_version"; do
        if [ -d "$candidate" ]; then
          folder="$candidate"
          break
        fi
      done
      if [ -z "$folder" ]; then
        echo "Cargo's registry, $registry, has no copy of $crate $crate_version" >&2
        exit 1
      fi
      shipped=0
      while IFS= read -r file; do
        echo "-- ${file##*/}"
        cat "$file"
        echo
        shipped=$((shipped + 1))
      done < <(find "$folder" -maxdepth 1 -type f \( -iname 'license*' -o -iname 'licence*' \
        -o -iname 'copying*' -o -iname 'copyright*' -o -iname 'notice*' -o -iname 'unlicense*' \) \
        | sort)
      if [ "$shipped" -eq 0 ]; then
        echo "-- (the crate ships no licence file; its licence is ${licence:-unnamed})"
      fi
    done
} > "$out/licenses/rust-crates.txt"
echo "licenses/rust-crates.txt: $(grep -c '^== ' "$out/licenses/rust-crates.txt") crates"

cat > "$out/licenses/README.md" <<EOF
# What lengua $version for $described ships, and its licences

- **lengua**: the skills, the manifests and \`server/lengua-mcp$exe\`, under the Apache License 2.0,
  in \`LICENSE\` beside this folder.
- **The Rust crates compiled into lengua-mcp**: each one's licence and the licence files it
  ships, in \`rust-crates.txt\`.
- **z3**, \`solvers/z3$exe\`, as its project releases it: under the licence in \`z3/\`. Its source
  is at $z3_source.
- **cvc5**, \`solvers/cvc5$exe\`, as its project releases it, statically linked: under the
  licences in \`cvc5/\`, its own (the BSD 3-Clause License) and those of what its build links in,
  which its \`COPYING\` names, such as the GNU Multiple Precision Arithmetic Library under the GNU
  LGPL 3 and CaDiCaL under the MIT License. Its source is at $cvc5_source.
EOF

# What the archive is for, which the install script holds the computer to, and how to install
# it.
printf '%s\n' "$target" > "$out/TARGET"
case "$target" in
  windows-*)
    cp "$root/scripts/install.ps1" "$out/install.ps1"
    install='In PowerShell, in this folder, run `powershell -ExecutionPolicy Bypass -File .\install.ps1`.'
    options='`-Check` only checks, and `-Prefix <folder>` installs into another folder.'
    destination='`%LOCALAPPDATA%\lengua\plugins\lengua`'
    ;;
  *)
    cp "$root/scripts/install.sh" "$out/install.sh"
    chmod 755 "$out/install.sh"
    install='In this folder, run `./install.sh`.'
    options='`./install.sh --check` only checks, `--prefix=<folder>` installs into another folder, and `--link` also puts `lengua-mcp` in `~/.local/bin`, to run programs from a shell.'
    case "$target" in
      macos-*) destination='`~/Library/Application Support/lengua/plugins/lengua`' ;;
      *) destination='`~/.local/share/lengua/plugins/lengua` (under `$XDG_DATA_HOME` when it is set)' ;;
    esac
    ;;
esac
cat > "$out/INSTALL.md" <<EOF
# lengua $version for $described

This folder is the lengua plugin: the skills \`write-idea\` and \`compile\`, the MCP server
\`server/lengua-mcp$exe\` with its tools \`check\` and \`run\`, the z3 and cvc5 solvers its
proofs need, in \`solvers/\`, and the licences of all of it, in \`LICENSE\` and \`licenses/\`.

$install It checks that this computer is $described, copies the plugin to $destination, adds
it to Claude Code when the \`claude\` command is installed (or says the two commands that do),
and runs \`lengua-mcp doctor\`, which says whether lengua has what it needs here and what to do
about anything missing. $options

To try it without installing, load this folder for one session: \`claude --plugin-dir <this
folder>\`. Other agent harnesses load it as an Agent Plugins plugin, from \`plugin.json\` and
\`mcp.json\`. The archive is unsigned.
EOF

mkdir -p "$root/dist"
case "$target" in
  windows-*)
    archive="$name.zip"
    rm -f "$root/dist/$archive"
    # Windows's own tar writes zip archives; Git Bash's writes none.
    windows_tar="$(cygpath -u "${SYSTEMROOT:-C:\\Windows}")/System32/tar.exe"
    "$windows_tar" -a -c -f "$(cygpath -m "$root/dist/$archive")" -C "$(cygpath -m "$stage")" \
      "$name"
    ;;
  *)
    archive="$name.tar.gz"
    # Without COPYFILE_DISABLE, macOS's tar adds metadata files of its own (._*).
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$root/dist/$archive" "$name"
    ;;
esac
(cd "$root/dist" && printf '%s  %s\n' "$(sha256_of "$archive")" "$archive" > "$archive.sha256")
echo "Built dist/$archive ($(du -h "$root/dist/$archive" | cut -f 1))"
