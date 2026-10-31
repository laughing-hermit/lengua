//! Shared test harness: drives the real `lengua-mcp` binary over stdio, the way an agent harness
//! does, in either MCP protocol generation.

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    ProtocolVersion,
};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Map, Value};
use tempfile::TempDir;

/// A valid spec, tests included, for a program that adds two numbers.
pub const ADD_SPEC: &str = r#"---
type: lengua/program-spec
title: add
description: Adds two whole numbers given as arguments and prints the sum.
---
# Purpose
Adds two whole numbers between `-1000000000` and `1000000000`.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` followed by a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` followed by a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Uses: nothing
"#;

/// What a user says to ask for a program.
pub const REQUEST: &str = "Make me a program that adds two numbers";

/// Which MCP protocol generation the client speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// `initialize` handshake (protocol 2025-11-25 and earlier).
    Handshake,
    /// `server/discover` with per-request metadata (protocol 2026-07-28).
    Discover,
}

impl Protocol {
    /// Both generations, for tests that must pass in each.
    pub const ALL: [Self; 2] = [Self::Handshake, Self::Discover];
}

/// A plain MCP client: lengua needs nothing from it but tool calls.
#[derive(Debug, Clone, Copy, Default)]
pub struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("lengua-test-client", "0.0.0"),
        )
    }
}

/// A running `lengua-mcp` with a private, temporary data folder.
pub struct Session {
    /// The connected client.
    pub service: RunningService<RoleClient, TestClient>,
    /// The server's data folder; deleted when the session is dropped.
    pub data_dir: TempDir,
    protocol: Protocol,
    proof_seconds: Option<u64>,
    wait_ms: Option<u64>,
    grace_ms: Option<u64>,
}

impl Session {
    /// Starts `lengua-mcp` and connects a client to it using `protocol`. Its proofs get no time,
    /// so tests about other things don't share the cores with them.
    pub async fn start(protocol: Protocol) -> Self {
        Self::start_with(protocol, Some(0), None, None).await
    }

    /// Starts `lengua-mcp` with its full time for proofs.
    pub async fn proving(protocol: Protocol) -> Self {
        Self::start_with(protocol, None, None, None).await
    }

    /// Starts `lengua-mcp` giving each version's proofs `seconds` in all.
    pub async fn proving_for(protocol: Protocol, seconds: u64) -> Self {
        Self::start_with(protocol, Some(seconds), None, None).await
    }

    /// Starts `lengua-mcp` with its full time for proofs and `check` waiting at most
    /// `milliseconds` for them.
    pub async fn proving_waiting(protocol: Protocol, milliseconds: u64) -> Self {
        Self::start_with(protocol, None, Some(milliseconds), None).await
    }

    /// Starts `lengua-mcp` with its full time for proofs, saying done `milliseconds` after the
    /// tests pass while the proofs go on.
    pub async fn proving_with_grace(protocol: Protocol, milliseconds: u64) -> Self {
        Self::start_with(protocol, None, None, Some(milliseconds)).await
    }

    /// Starts `lengua-mcp` with `check` waiting at most `milliseconds` for the tests and proof.
    pub async fn start_waiting(protocol: Protocol, milliseconds: u64) -> Self {
        Self::start_with(protocol, Some(0), Some(milliseconds), None).await
    }

    async fn start_with(
        protocol: Protocol,
        proof_seconds: Option<u64>,
        wait_ms: Option<u64>,
        grace_ms: Option<u64>,
    ) -> Self {
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        Self::connect(protocol, data_dir, proof_seconds, wait_ms, grace_ms).await
    }

    /// Ends this server and starts a new one on the same data folder, as when a harness restarts
    /// it: whatever the old one kept in memory is gone.
    pub async fn restart(self) -> Self {
        let Self {
            service,
            data_dir,
            protocol,
            proof_seconds,
            wait_ms,
            grace_ms,
        } = self;
        service.cancel().await.expect("close the session");
        Self::connect(protocol, data_dir, proof_seconds, wait_ms, grace_ms).await
    }

    async fn connect(
        protocol: Protocol,
        data_dir: TempDir,
        proof_seconds: Option<u64>,
        wait_ms: Option<u64>,
        grace_ms: Option<u64>,
    ) -> Self {
        let command =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_lengua-mcp")).configure(|command| {
                command.arg("--data-dir").arg(data_dir.path());
                if let Some(seconds) = proof_seconds {
                    command.env("LENGUA_PROOF_SECONDS", seconds.to_string());
                }
                if let Some(milliseconds) = wait_ms {
                    command.env("LENGUA_CHECK_WAIT_MS", milliseconds.to_string());
                }
                if let Some(milliseconds) = grace_ms {
                    command.env("LENGUA_GREEN_GRACE_MS", milliseconds.to_string());
                }
            });
        let transport = TokioChildProcess::new(command).expect("spawn lengua-mcp");
        let service = match protocol {
            Protocol::Handshake => TestClient.serve(transport).await,
            Protocol::Discover => {
                TestClient
                    .serve_with_lifecycle(
                        transport,
                        ClientLifecycleMode::Discover {
                            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                        },
                    )
                    .await
            }
        }
        .expect("connect to lengua-mcp");
        Self {
            service,
            data_dir,
            protocol,
            proof_seconds,
            wait_ms,
            grace_ms,
        }
    }

    /// Calls a tool with JSON object arguments and returns its result.
    pub async fn call(&self, tool: &'static str, arguments: Value) -> CallToolResult {
        self.service
            .call_tool(params(tool, arguments))
            .await
            .unwrap_or_else(|error| panic!("calling {tool} failed: {error}"))
    }

    /// Calls a tool that must succeed and returns its structured result.
    pub async fn call_ok(&self, tool: &'static str, arguments: Value) -> Value {
        let result = self.call(tool, arguments).await;
        assert_ne!(
            result.is_error,
            Some(true),
            "{tool} returned an error: {result:?}"
        );
        let content = match result.structured_content {
            Some(content) => content,
            None => panic!(
                "{tool} returned no structured content: {:?}",
                result.content
            ),
        };
        // Harnesses check replies against the tool's output schema and refuse any that don't
        // fit, so every reply a test gets must fit too.
        let tools = self.service.list_all_tools().await.expect("list tools");
        let schema = tools
            .iter()
            .find(|listed| listed.name == tool)
            .and_then(|listed| listed.output_schema.as_ref())
            .map(|schema| serde_json::to_value(schema.as_ref()).expect("a JSON schema"));
        if let Some(schema) = schema {
            let validator = jsonschema::validator_for(&schema).expect("compile the output schema");
            if let Err(error) = validator.validate(&content) {
                panic!("{tool} replied outside its output schema: {error}: {content}");
            }
        }
        content
    }

    /// Calls a tool that must fail and returns its error text.
    pub async fn call_error(&self, tool: &'static str, arguments: Value) -> String {
        let result = self.call(tool, arguments).await;
        assert_eq!(
            result.is_error,
            Some(true),
            "{tool} should have failed: {result:?}"
        );
        result
            .content
            .iter()
            .filter_map(|content| content.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Checks the program the way an agent does after writing its spec from the user's
    /// request: with their words.
    pub async fn record(&self, program: &Program) -> Value {
        self.call_ok("check", program.request(REQUEST)).await
    }

    /// Checks the program once.
    pub async fn check(&self, program: &Program) -> Value {
        self.call_ok("check", program.arguments()).await
    }

    /// Checks the program until its tests and proofs have ended, as an agent that wants the
    /// final proofs keeps calling while check says they're running, and returns the last reply.
    pub async fn settle(&self, program: &Program) -> Value {
        self.settle_with(program, program.arguments()).await
    }

    /// [`Self::settle`], the first check with `arguments`, such as `prove_further`.
    pub async fn settle_with(&self, program: &Program, arguments: Value) -> Value {
        let mut arguments = Some(arguments);
        loop {
            let reply = match arguments.take() {
                Some(arguments) => self.call_ok("check", arguments).await,
                None => self.check(program).await,
            };
            let running = reply["proofs"].as_array().is_some_and(|proofs| {
                proofs.iter().filter_map(Value::as_str).any(|line| {
                    line.contains(": proving") || line.ends_with("waiting for its turn")
                })
            });
            if !running && !matches!(reply["state"].as_str(), Some("testing" | "proving")) {
                return reply;
            }
        }
    }
}

fn params(tool: &'static str, arguments: Value) -> CallToolRequestParams {
    let arguments = match arguments {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    CallToolRequestParams::new(tool).with_arguments(arguments)
}

/// A temporary program folder.
pub struct Program {
    dir: TempDir,
}

impl Program {
    /// An empty program folder.
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create a temporary program folder"),
        }
    }

    /// An empty program folder named for `name` inside `parent`, beside the others there, as the
    /// programs a graph links stand.
    pub fn new_in(parent: &Path, name: &str) -> Self {
        Self {
            dir: tempfile::Builder::new()
                .prefix(name)
                .tempdir_in(parent)
                .expect("create a program folder"),
        }
    }

    /// The folder's own name.
    pub fn name(&self) -> String {
        self.path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 name")
            .to_owned()
    }

    /// A program folder holding the add spec, its tests included.
    pub fn with_add_spec() -> Self {
        let program = Self::new();
        program.write("spec.md", ADD_SPEC);
        program
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The folder as a tool argument.
    pub fn arguments(&self) -> Value {
        serde_json::json!({ "path": self.path().to_str().expect("UTF-8 path") })
    }

    /// Arguments for `check` the first time: the folder and the user's words asking for the
    /// program.
    pub fn request(&self, words: &str) -> Value {
        serde_json::json!({
            "path": self.path().to_str().expect("UTF-8 path"),
            "request": words,
        })
    }

    /// A file in the folder.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path().join(name)
    }

    /// Writes a file in the folder.
    pub fn write(&self, name: &str, text: &str) {
        std::fs::write(self.file(name), text).expect("write a program file");
    }

    /// Reads a file in the folder, if it exists.
    pub fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.file(name)).ok()
    }
}
