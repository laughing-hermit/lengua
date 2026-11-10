//! `cargo xtask plugin`: the lengua MCP server, built as a release, put in plugin/server/, the
//! path both plugin/mcp.json (Agent Plugins) and plugin/.mcp.json (Claude Code) point to:
//! lengua-mcp, or on Windows lengua-mcp.exe, which the release archive's manifests name with its
//! .exe. `cargo xtask release` builds the archive users install.

use crate::ci;

/// `cargo xtask plugin`.
pub fn task(arguments: &[String]) -> Result<u8, String> {
    if !arguments.is_empty() {
        return Err("usage: cargo xtask plugin".to_owned());
    }
    let root = ci::root()?;
    ci::must(
        ci::cargo()
            .current_dir(&root)
            .args(["build", "--release", "--manifest-path"])
            .arg(root.join("Cargo.toml")),
    )?;
    let name = format!("lengua-mcp{}", std::env::consts::EXE_SUFFIX);
    let folder = root.join("plugin").join("server");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let installed = folder.join(&name);
    let built = root.join("target").join("release").join(&name);
    std::fs::copy(&built, &installed)
        .map_err(|error| format!("{} to {}: {error}", built.display(), installed.display()))?;
    ci::executable(&installed)?;
    println!("Installed {}", installed.display());
    Ok(0)
}
