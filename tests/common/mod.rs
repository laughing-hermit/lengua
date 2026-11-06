//! Shared test harness: drives the real `lengua-mcp` binary over stdio, the way an agent harness
//! does, in either MCP protocol generation.

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    ProtocolVersion,
};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceExt};
use serde_json::{Map, Value};
use tempfile::TempDir;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// Sessions that prove share this lock, and one whose test needs a hard query answered within the
/// solvers' time takes it alone: each proof uses every core, so beside the others its solvers
/// would run out of time. The tests that don't prove run beside it either way.
fn proving() -> Arc<RwLock<()>> {
    static PROVING: OnceLock<Arc<RwLock<()>>> = OnceLock::new();
    PROVING.get_or_init(|| Arc::new(RwLock::new(()))).clone()
}

/// A proving session's hold on [`proving`].
enum Proving {
    Shared(OwnedRwLockReadGuard<()>),
    Alone(OwnedRwLockWriteGuard<()>),
}

/// A valid idea, tests included, for a program that adds two numbers.
pub const ADD_IDEA: &str = r#"---
type: lengua/program-idea
title: add
description: Adds the two whole numbers in its arguments and prints the sum.
---
# Purpose
The program adds two whole numbers between `-1000000000` and `1000000000`.

# Behaviors
## B1: Adds two numbers
- Given the arguments `2` and `3`
- When the program runs
- Then it prints `5` and a newline
- And it exits with code `0`

### For any `a` and `b`
- Given the arguments `{a}` and `{b}`
- When the program runs
- Then it prints `{a + b}` and a newline
- And it exits with code `0`

| a | b | runs |
|---|---|---|
| `int(-1000000000, 1000000000)` | `int(-1000000000, 1000000000)` | 200 |

## B2: Shows the usage line without arguments
- Given no arguments
- When the program runs
- Then it prints `usage: add <a> <b>` and a newline to standard error
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

/// Whether this computer runs AArch64 programs only through QEMU's user mode, registered with
/// binfmt_misc's flag F as lengua needs: then a session for linux-aarch64 is emulated here.
#[must_use]
pub fn qemu_runs_aarch64() -> bool {
    if cfg!(target_arch = "aarch64") {
        return false;
    }
    let entry =
        std::fs::read_to_string("/proc/sys/fs/binfmt_misc/qemu-aarch64").unwrap_or_default();
    let ready = entry.lines().next() == Some("enabled")
        && entry.lines().any(|line| {
            line.strip_prefix("flags: ")
                .is_some_and(|flags| flags.contains('F'))
        });
    if !ready {
        lengua::skiplog::note("QEMU doesn't run AArch64 programs here (install qemu-user-static)");
    }
    ready
}

/// The timings a session sets on its server, for tests: each `None` leaves the server's own.
#[derive(Debug, Clone, Copy, Default)]
struct Knobs {
    /// `LENGUA_PROOF_SECONDS`: the time each version's proofs get in all.
    proof_seconds: Option<u64>,
    /// `LENGUA_CHECK_WAIT_MS`: how long `check` waits.
    wait_ms: Option<u64>,
    /// `LENGUA_GREEN_GRACE_MS`: how long `check` waits for the proofs once the tests pass.
    grace_ms: Option<u64>,
    /// `LENGUA_STAMP_DELAY_MS`: how long each write of the proofs to lengua.lock waits first.
    stamp_ms: Option<u64>,
}

/// A running `lengua-mcp` with a private, temporary data folder.
pub struct Session {
    /// The connected client.
    pub service: RunningService<RoleClient, TestClient>,
    /// The server's data folder; deleted when the session is dropped.
    pub data_dir: TempDir,
    protocol: Protocol,
    /// The timings it set on the server.
    knobs: Knobs,
    /// The target a developer's `LENGUA_TARGET` names, when the session has one.
    target: Option<&'static str>,
    /// While the session may prove, its hold on the proving lock.
    _proving: Option<Proving>,
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
        let hold = match proof_seconds {
            Some(0) => None,
            _ => Some(Proving::Shared(proving().read_owned().await)),
        };
        let knobs = Knobs {
            proof_seconds,
            wait_ms,
            grace_ms,
            stamp_ms: None,
        };
        Self::connect(protocol, data_dir, knobs, None, hold).await
    }

    /// Starts `lengua-mcp` with its full time for proofs, `check` waiting at most `wait_ms` for
    /// them, and each write of the proofs to lengua.lock waiting `stamp_ms` first: the moment
    /// after the last proof is long enough to look at.
    pub async fn proving_slow_to_record(protocol: Protocol, wait_ms: u64, stamp_ms: u64) -> Self {
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        let hold = Some(Proving::Shared(proving().read_owned().await));
        let knobs = Knobs {
            wait_ms: Some(wait_ms),
            stamp_ms: Some(stamp_ms),
            ..Knobs::default()
        };
        Self::connect(protocol, data_dir, knobs, None, hold).await
    }

    /// Starts `lengua-mcp` for `target`, as a developer's `LENGUA_TARGET` asks: a target this
    /// computer runs only through QEMU's user mode. Its proofs get no time.
    pub async fn emulating(protocol: Protocol, target: &'static str) -> Self {
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        let knobs = Knobs {
            proof_seconds: Some(0),
            ..Knobs::default()
        };
        Self::connect(protocol, data_dir, knobs, Some(target), None).await
    }

    /// Starts `lengua-mcp` for `target` through QEMU's user mode, as [`Self::emulating`] does,
    /// giving each version's proofs `seconds` in all.
    pub async fn emulating_for(protocol: Protocol, target: &'static str, seconds: u64) -> Self {
        let hold = Some(Proving::Shared(proving().read_owned().await));
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        let knobs = Knobs {
            proof_seconds: Some(seconds),
            ..Knobs::default()
        };
        Self::connect(protocol, data_dir, knobs, Some(target), hold).await
    }

    /// Starts `lengua-mcp` giving each version's proofs `seconds` in all, once no other session
    /// in the test binary is proving, and keeps the others from proving until it ends: a proof
    /// that must end within its time, as an induction's, would run out of it beside theirs.
    pub async fn proving_alone_for(protocol: Protocol, seconds: u64) -> Self {
        let hold = Some(Proving::Alone(proving().write_owned().await));
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        let knobs = Knobs {
            proof_seconds: Some(seconds),
            ..Knobs::default()
        };
        Self::connect(protocol, data_dir, knobs, None, hold).await
    }

    /// Starts `lengua-mcp` with its full time for proofs, once no other session in the test
    /// binary is proving, and keeps the others from proving until it ends.
    pub async fn proving_alone(protocol: Protocol) -> Self {
        let hold = Some(Proving::Alone(proving().write_owned().await));
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        Self::connect(protocol, data_dir, Knobs::default(), None, hold).await
    }

    /// Ends this server and starts a new one on the same data folder, as when a harness restarts
    /// it: whatever the old one kept in memory is gone.
    pub async fn restart(self) -> Self {
        let target = self.target;
        self.restart_for(target).await
    }

    /// Restarts it as [`Session::restart`] does, for `target` (`None`: this computer's own).
    pub async fn restart_for(self, target: Option<&'static str>) -> Self {
        let Self {
            service,
            data_dir,
            protocol,
            knobs,
            _proving: hold,
            ..
        } = self;
        service.cancel().await.expect("close the session");
        Self::connect(protocol, data_dir, knobs, target, hold).await
    }

    async fn connect(
        protocol: Protocol,
        data_dir: TempDir,
        knobs: Knobs,
        target: Option<&'static str>,
        hold: Option<Proving>,
    ) -> Self {
        let command =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_lengua-mcp")).configure(|command| {
                command.arg("--data-dir").arg(data_dir.path());
                // The session's own target, never one the developer's shell happens to name.
                command.env_remove("LENGUA_TARGET");
                if let Some(target) = target {
                    command.env("LENGUA_TARGET", target);
                }
                if let Some(seconds) = knobs.proof_seconds {
                    command.env("LENGUA_PROOF_SECONDS", seconds.to_string());
                }
                if let Some(milliseconds) = knobs.wait_ms {
                    command.env("LENGUA_CHECK_WAIT_MS", milliseconds.to_string());
                }
                if let Some(milliseconds) = knobs.grace_ms {
                    command.env("LENGUA_GREEN_GRACE_MS", milliseconds.to_string());
                }
                if let Some(milliseconds) = knobs.stamp_ms {
                    command.env("LENGUA_STAMP_DELAY_MS", milliseconds.to_string());
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
            knobs,
            target,
            _proving: hold,
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

    /// Checks the program the way an agent does after writing its idea from the user's
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
    /// Deletes the folder when the program is dropped, with the bundle it stands in when it has
    /// one of its own.
    _dir: TempDir,
    /// The folder.
    path: PathBuf,
}

impl Program {
    /// An empty program folder, `<tmp>/bundle/program`: alone in a bundle of its own, so the
    /// index.md and log.md that check keeps land there, never in the temporary folder itself.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create a temporary folder");
        let path = dir.path().join("bundle").join("program");
        std::fs::create_dir_all(&path).expect("create a program folder");
        Self { _dir: dir, path }
    }

    /// An empty program folder named for `name` inside `parent`, beside the others there, as the
    /// programs a graph links stand.
    pub fn new_in(parent: &Path, name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(name)
            .tempdir_in(parent)
            .expect("create a program folder");
        let path = dir.path().to_path_buf();
        Self { _dir: dir, path }
    }

    /// The folder's own name.
    pub fn name(&self) -> String {
        self.path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 name")
            .to_owned()
    }

    /// A program folder holding the add idea, its tests included.
    pub fn with_add_idea() -> Self {
        let program = Self::new();
        program.write("idea.md", ADD_IDEA);
        program
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
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
