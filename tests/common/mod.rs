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

/// A valid idea, tests included, for a program that adds two numbers: an Attested Computation.
pub const ADD_IDEA: &str = include_str!("../fixtures/add/idea.md");

/// The main.wat compiled from [`ADD_IDEA`], which follows every rule and passes every test.
pub const ADD_CODE: &str = include_str!("../fixtures/add/main.wat");

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
        Self::connect(protocol, data_dir, knobs, hold).await
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
        Self::connect(protocol, data_dir, knobs, hold).await
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
        Self::connect(protocol, data_dir, knobs, hold).await
    }

    /// Starts `lengua-mcp` with its full time for proofs, once no other session in the test
    /// binary is proving, and keeps the others from proving until it ends.
    pub async fn proving_alone(protocol: Protocol) -> Self {
        let hold = Some(Proving::Alone(proving().write_owned().await));
        let data_dir = tempfile::tempdir().expect("create a temporary data folder");
        Self::connect(protocol, data_dir, Knobs::default(), hold).await
    }

    /// Ends this server and starts a new one on the same data folder, as when a harness restarts
    /// it: whatever the old one kept in memory is gone.
    pub async fn restart(self) -> Self {
        let Self {
            service,
            data_dir,
            protocol,
            knobs,
            _proving: hold,
        } = self;
        service.cancel().await.expect("close the session");
        Self::connect(protocol, data_dir, knobs, hold).await
    }

    async fn connect(
        protocol: Protocol,
        data_dir: TempDir,
        knobs: Knobs,
        hold: Option<Proving>,
    ) -> Self {
        let command =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_lengua-mcp")).configure(|command| {
                command.arg("--data-dir").arg(data_dir.path());
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
    /// one of its own; none for a folder beside another program, whose bundle it is in.
    _dir: Option<TempDir>,
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
        Self {
            _dir: Some(dir),
            path,
        }
    }

    /// An empty program folder named for `name` inside `parent`, beside the others there, as the
    /// programs a graph links stand.
    pub fn new_in(parent: &Path, name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(name)
            .tempdir_in(parent)
            .expect("create a program folder");
        let path = dir.path().to_path_buf();
        Self {
            _dir: Some(dir),
            path,
        }
    }

    /// An empty program folder named `name` beside this one, in its bundle, as a part its idea
    /// links to with `../name/idea.md` stands. It lasts as long as this program's bundle does.
    pub fn beside(&self, name: &str) -> Self {
        let path = self
            .path()
            .parent()
            .expect("a program folder stands in a bundle")
            .join(name);
        std::fs::create_dir_all(&path).expect("create a program folder");
        Self { _dir: None, path }
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

/// The folders of the parts that the program in `folder` builds on, each before the programs
/// that build on it, none twice: the folders its idea's `- Builds on:` links name, and theirs.
/// A folder whose idea doesn't parse has none.
pub fn parts_of(folder: &Path) -> Vec<PathBuf> {
    fn walk(folder: &Path, found: &mut Vec<PathBuf>) {
        let Ok(idea) = std::fs::read_to_string(folder.join("idea.md")) else {
            return;
        };
        let Ok(idea) = lengua::idea::parse(&idea) else {
            return;
        };
        for part in &idea.parts {
            let joined = folder.join(part.folder());
            let part = std::fs::canonicalize(&joined).unwrap_or(joined);
            if !found.contains(&part) {
                walk(&part, found);
                found.push(part);
            }
        }
    }
    let mut found = Vec::new();
    walk(folder, &mut found);
    found
}

impl Session {
    /// Lays out beside `program` each part that the program in `folder` builds on, a folder of
    /// the repository with its idea.md and main.wat, under the part's own folder name, and takes
    /// each to done, the parts a part builds on first: what an agent does before it builds a
    /// program on parts. Gives the first part that isn't done, with check's reply.
    pub async fn settle_parts(&self, program: &Program, folder: &Path) -> Result<(), String> {
        for part in parts_of(folder) {
            let name = part
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a part's folder has a UTF-8 name");
            let beside = program.beside(name);
            for file in ["idea.md", "main.wat"] {
                let text = std::fs::read_to_string(part.join(file))
                    .map_err(|error| format!("{}/{file}: {error}", part.display()))?;
                beside.write(file, &text);
            }
            self.record(&beside).await;
            let done = self.settle(&beside).await;
            if done["state"] != "done" {
                return Err(format!("the part {name} isn't done: {done}"));
            }
        }
        Ok(())
    }
}

/// Where CI's outside-wasm job put the modules it built from the repository's Rust and C, when
/// this run has them (`LENGUA_OUTSIDE_WASM`): `<name>.wasm` for each. A run on GitHub's runners
/// without them fails, so no such run passes having skipped the compilers' output.
pub fn compiled(name: &str) -> Option<Vec<u8>> {
    let Some(folder) = std::env::var_os("LENGUA_OUTSIDE_WASM").filter(|folder| !folder.is_empty())
    else {
        assert!(
            std::env::var_os("GITHUB_ACTIONS").is_none(),
            "LENGUA_OUTSIDE_WASM names no folder of the modules built from Rust and C"
        );
        lengua::skiplog::note(
            "LENGUA_OUTSIDE_WASM names no folder, so no module a compiler made from Rust or C \
             was checked or run (cargo xtask outside-wasm builds them)",
        );
        return None;
    };
    let file = Path::new(&folder).join(format!("{name}.wasm"));
    Some(std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display())))
}

/// The modules that the program in `folder` calls, as its idea's `- Calls:` lines name them. A
/// folder whose idea doesn't parse has none.
pub fn calls_of(folder: &Path) -> Vec<lengua::call::Called> {
    std::fs::read_to_string(folder.join("idea.md"))
        .ok()
        .and_then(|idea| lengua::idea::parse(&idea).ok())
        .map(|idea| idea.network.reach.calls)
        .unwrap_or_default()
}

impl Session {
    /// Lays out for `program` each module that the program in `folder`, a folder of the
    /// repository, calls: a lengua program beside it, under its own folder name, taken to done
    /// with its own parts first; and an outside module in `program`'s folder, as the module
    /// CI's outside-wasm job built from the source beside the example, `<example>.<module>`.
    /// Gives false when this run has no such modules, which only a run off GitHub's runners
    /// may; and the first called program that isn't done, with check's reply.
    pub async fn settle_calls(&self, program: &Program, folder: &Path) -> Result<bool, String> {
        for called in calls_of(folder) {
            match &called.kind {
                lengua::call::Kind::Program { .. } => {
                    let joined = folder.join(called.place());
                    let theirs = std::fs::canonicalize(&joined).unwrap_or(joined);
                    let beside = program.beside(&called.name);
                    for file in ["idea.md", "main.wat"] {
                        let text = std::fs::read_to_string(theirs.join(file))
                            .map_err(|error| format!("{}/{file}: {error}", theirs.display()))?;
                        beside.write(file, &text);
                    }
                    self.settle_parts(&beside, &theirs).await?;
                    self.record(&beside).await;
                    let done = self.settle(&beside).await;
                    if done["state"] != "done" {
                        return Err(format!(
                            "the called program {} isn't done: {done}",
                            called.name
                        ));
                    }
                }
                lengua::call::Kind::Outside { .. } => {
                    let example = folder
                        .file_name()
                        .and_then(|name| name.to_str())
                        .expect("an example's folder has a UTF-8 name");
                    let Some(module) = compiled(&format!("{example}.{}", called.name)) else {
                        return Ok(false);
                    };
                    std::fs::write(program.file(&called.place()), module)
                        .map_err(|error| format!("{}: {error}", called.place()))?;
                }
            }
        }
        Ok(true)
    }
}
