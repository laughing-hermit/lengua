#!/usr/bin/env bash
# Builds the lengua MCP server and installs it into plugin/server/, the path both
# plugin/mcp.json (Agent Plugins) and plugin/.mcp.json (Claude Code) point to.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --release --manifest-path "$root/Cargo.toml"
install -D -m 0755 "$root/target/release/lengua-mcp" "$root/plugin/server/lengua-mcp"
echo "Installed $root/plugin/server/lengua-mcp"
