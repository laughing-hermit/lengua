#!/usr/bin/env bash
# Builds the lengua MCP server and installs it into plugin/server/, the path both
# plugin/mcp.json (Agent Plugins) and plugin/.mcp.json (Claude Code) point to: lengua-mcp, or
# on Windows (under Git Bash) lengua-mcp.exe, which the release archive's manifests name with
# its .exe. scripts/release.sh builds the archive users install.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exe=""
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) exe=".exe" ;;
esac
cargo build --release --manifest-path "$root/Cargo.toml"
mkdir -p "$root/plugin/server"
cp "$root/target/release/lengua-mcp$exe" "$root/plugin/server/lengua-mcp$exe"
chmod 755 "$root/plugin/server/lengua-mcp$exe"
echo "Installed $root/plugin/server/lengua-mcp$exe"
