//! Shared test harness: drives the real `lengua-mcp` binary over stdio, the way an agent harness
//! does, in either MCP protocol generation and with scripted answers to approval dialogs.

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ClientCapabilities, ClientConfig,
    ElicitRequestParams, ElicitResult, ElicitationAction, ErrorData, Implementation,
    ProtocolVersion,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Map, Value};
use tempfile::TempDir;

/// A valid spec for a program that adds two numbers.
pub const ADD_SPEC: &str = "---
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

## B2: Missing arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` followed by a newline to standard error
- And it exits with code `1`

# Constraints
- Allowed system calls: read, write, exit
";

/// Valid tests for the add spec, with `{label}` standing for the spec's label.
pub const ADD_TESTS: &str = r#"[meta]
spec = "{label}"

[[example]]
id = "B1"
args = ["2", "3"]
stdout = "5\n"
exit_code = 0

[[example]]
id = "B2"
stderr = "usage: add <a> <b>\n"
exit_code = 1

[[generated]]
id = "B1"
runs = 200
vars = { a = "int(-1000000000, 1000000000)", b = "int(-1000000000, 1000000000)" }
args = ["{a}", "{b}"]
stdout = "{a + b}\n"
exit_code = 0
"#;

/// The label tests and code carry for a spec: `sha256:<hex>`.
pub fn label(spec: &str) -> String {
    lengua::hash::Sha256Hex::of(spec.as_bytes()).label()
}

/// The add tests, labeled for `spec`.
pub fn add_tests(spec: &str) -> String {
    ADD_TESTS.replacen("{label}", &label(spec), 1)
}

/// Which MCP protocol generation the client speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// `initialize` handshake (protocol 2025-11-25 and earlier); approval via `elicitation/create`.
    Handshake,
    /// `server/discover` with per-request metadata (protocol 2026-07-28); approval via
    /// input-required results.
    Discover,
}

impl Protocol {
    /// Both generations, for tests that must pass in each.
    pub const ALL: [Self; 2] = [Self::Handshake, Self::Discover];
}

/// How the scripted user answers an approval dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Accepts the form with `approve = true`.
    Approve,
    /// Accepts the form with `approve = false`.
    Reject,
    /// Presses "Decline".
    Decline,
    /// Dismisses the dialog.
    Cancel,
}

/// A test MCP client with optional elicitation support and a scripted answer.
#[derive(Debug, Clone)]
pub struct TestClient {
    elicitation: bool,
    answer: Arc<Mutex<Answer>>,
    prompts: Arc<Mutex<Vec<String>>>,
    /// A file to overwrite while the next dialog is open, as if the user edited it.
    edit: Arc<Mutex<Option<(PathBuf, String)>>>,
}

impl TestClient {
    /// A client that can show approval dialogs and answers them with `answer`.
    pub fn with_elicitation(answer: Answer) -> Self {
        Self {
            elicitation: true,
            answer: Arc::new(Mutex::new(answer)),
            prompts: Arc::default(),
            edit: Arc::default(),
        }
    }

    /// A client that can't show approval dialogs.
    pub fn without_elicitation() -> Self {
        Self {
            elicitation: false,
            answer: Arc::new(Mutex::new(Answer::Decline)),
            prompts: Arc::default(),
            edit: Arc::default(),
        }
    }

    /// Changes the scripted answer for the next dialogs.
    pub fn set_answer(&self, answer: Answer) {
        *self.answer.lock().expect("answer lock") = answer;
    }

    /// Overwrites `path` with `text` while the next dialog is open.
    pub fn edit_during_next_dialog(&self, path: PathBuf, text: String) {
        *self.edit.lock().expect("edit lock") = Some((path, text));
    }

    /// The messages of every dialog shown so far.
    pub fn prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("prompts lock").clone()
    }
}

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientConfig {
        let capabilities = if self.elicitation {
            ClientCapabilities::builder().enable_elicitation().build()
        } else {
            ClientCapabilities::default()
        };
        ClientConfig::new(
            capabilities,
            Implementation::new("lengua-test-client", "0.0.0"),
        )
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, ErrorData> {
        if let ElicitRequestParams::FormElicitationParams { message, .. } = &request {
            self.prompts
                .lock()
                .expect("prompts lock")
                .push(message.clone());
        }
        if let Some((path, text)) = self.edit.lock().expect("edit lock").take() {
            std::fs::write(path, text).expect("edit the file during the dialog");
        }
        let answer = *self.answer.lock().expect("answer lock");
        Ok(match answer {
            Answer::Approve => ElicitResult::new(ElicitationAction::Accept)
                .with_content(serde_json::json!({ "approve": true })),
            Answer::Reject => ElicitResult::new(ElicitationAction::Accept)
                .with_content(serde_json::json!({ "approve": false })),
            Answer::Decline => ElicitResult::new(ElicitationAction::Decline),
            Answer::Cancel => ElicitResult::new(ElicitationAction::Cancel),
        })
    }
}

/// A running `lengua-mcp` with a private, temporary data folder.
pub struct Session {
    /// The connected client.
    pub service: RunningService<RoleClient, TestClient>,
    /// The scripted client, for changing answers and reading prompts.
    pub client: TestClient,
    /// The server's data folder; deleted when the session is dropped.
    pub data_dir: TempDir,
}

impl Session {
    /// Starts `lengua-mcp` and connects `client` to it using `protocol`.
    pub async fn start(protocol: Protocol, client: TestClient) -> Self {
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        let command =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_lengua-mcp")).configure(|command| {
                command.arg("--data-dir").arg(data_dir.path());
            });
        let transport = TokioChildProcess::new(command).expect("spawn lengua-mcp");
        let service = match protocol {
            Protocol::Handshake => client.clone().serve(transport).await,
            Protocol::Discover => {
                client
                    .clone()
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
            client,
            data_dir,
        }
    }

    /// Calls a tool with JSON object arguments and returns its result, answering any dialogs.
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
        match result.structured_content {
            Some(content) => content,
            None => panic!(
                "{tool} returned no structured content: {:?}",
                result.content
            ),
        }
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

    /// Sends one `tools/call` without answering dialogs, optionally as a retry that carries
    /// `request_state` and `responses`.
    pub async fn call_once(
        &self,
        tool: &'static str,
        arguments: Value,
        request_state: Option<&str>,
        responses: Option<BTreeMap<String, Value>>,
    ) -> CallToolResponse {
        let mut params = params(tool, arguments);
        if let Some(state) = request_state {
            params = params.with_request_state(state);
        }
        if let Some(responses) = responses {
            params = params.with_input_responses(responses);
        }
        self.service
            .call_tool_once(params)
            .await
            .unwrap_or_else(|error| panic!("calling {tool} failed: {error}"))
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

    /// A program folder holding the add spec.
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
