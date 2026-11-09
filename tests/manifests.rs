//! The shipped plugin folder is valid for the Agent Plugins standard and stays in sync with its
//! Claude Code compatibility files.

use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn assert_valid(schema_file: &str, instance_file: &str) {
    let schema = read_json(&repo().join("tests/schemas").join(schema_file));
    let instance = read_json(&repo().join("plugin").join(instance_file));
    let validator = jsonschema::validator_for(&schema).expect("compile the official schema");
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{instance_file} is invalid:\n{}",
        errors.join("\n")
    );
}

#[test]
fn plugin_json_matches_the_official_schema() {
    assert_valid("plugin.schema.json", "plugin.json");
}

#[test]
fn mcp_json_matches_the_official_schema() {
    assert_valid("mcp.schema.json", "mcp.json");
}

#[test]
fn claude_code_manifest_mirrors_the_standard_one() {
    let standard = read_json(&repo().join("plugin/plugin.json"));
    let claude = read_json(&repo().join("plugin/.claude-plugin/plugin.json"));
    for field in [
        "name",
        "version",
        "description",
        "license",
        "author",
        "keywords",
    ] {
        assert_eq!(standard[field], claude[field], "field {field} differs");
    }
    assert!(
        claude.get("extensions").is_none(),
        "Claude Code rejects extensions under --strict"
    );
}

/// The manifests offer main.wat alone: lengua compiles to WebAssembly since 0.2.0, and the tag
/// native-final keeps the native lengua, which compiled to assembly.
#[test]
fn manifests_offer_webassembly_alone() {
    let standard = read_json(&repo().join("plugin/plugin.json"));
    let description = standard["description"].as_str().expect("a description");
    assert!(
        description.contains("WebAssembly") && !description.to_lowercase().contains(" assembly"),
        "the description offers WebAssembly alone: {description}"
    );
    let keywords = standard["keywords"].as_array().expect("keywords");
    assert!(
        !keywords.iter().any(|keyword| keyword == "assembly"),
        "no keyword says assembly: {keywords:?}"
    );
}

#[test]
fn manifests_carry_the_crate_version() {
    let standard = read_json(&repo().join("plugin/plugin.json"));
    assert_eq!(standard["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn both_mcp_configs_start_the_same_server_binary() {
    let standard = read_json(&repo().join("plugin/mcp.json"));
    let claude = read_json(&repo().join("plugin/.mcp.json"));
    assert_eq!(
        standard["mcpServers"]["lengua"]["command"],
        "./server/lengua-mcp"
    );
    assert_eq!(
        claude["mcpServers"]["lengua"]["command"],
        "${CLAUDE_PLUGIN_ROOT}/server/lengua-mcp"
    );
    for config in [&standard, &claude] {
        let servers = config["mcpServers"].as_object().expect("mcpServers object");
        assert_eq!(servers.len(), 1, "exactly one server");
        assert!(
            servers["lengua"].get("args").is_none(),
            "no per-harness data folder"
        );
    }
}

#[test]
fn shipped_license_matches_the_repository_license() {
    let root = std::fs::read(repo().join("LICENSE")).expect("read LICENSE");
    let shipped = std::fs::read(repo().join("plugin/LICENSE")).expect("read plugin/LICENSE");
    assert!(root == shipped, "plugin/LICENSE drifted from LICENSE");
}
