//! Shared test harness: drives the real `lengua-mcp` binary over stdio, the way an agent harness
//! does, in either MCP protocol generation and with scripted answers to approval dialogs.

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationAction, ErrorData, Implementation, ProtocolVersion,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Map, Value};
use tempfile::TempDir;

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
}

impl TestClient {
    /// A client that can show approval dialogs and answers them with `answer`.
    pub fn with_elicitation(answer: Answer) -> Self {
        Self {
            elicitation: true,
            answer: Arc::new(Mutex::new(answer)),
            prompts: Arc::default(),
        }
    }

    /// A client that can't show approval dialogs.
    pub fn without_elicitation() -> Self {
        Self {
            elicitation: false,
            answer: Arc::new(Mutex::new(Answer::Decline)),
            prompts: Arc::default(),
        }
    }

    /// Changes the scripted answer for the next dialogs.
    pub fn set_answer(&self, answer: Answer) {
        *self.answer.lock().expect("answer lock") = answer;
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

    /// Calls a tool with JSON object arguments and returns its result.
    pub async fn call(&self, tool: &'static str, arguments: Value) -> CallToolResult {
        let mut params = CallToolRequestParams::new(tool);
        if let Value::Object(map) = arguments {
            params = params.with_arguments(map);
        } else {
            params = params.with_arguments(Map::new());
        }
        self.service
            .call_tool(params)
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
}
