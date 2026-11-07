//! The release check: lengua's release archive for this computer, installed as a user installs
//! it, checked through a plain MCP client. CI's `release` mode builds the archive and this check
//! on each of the five runners, installs the archive on a clean runner of the same system and
//! CPU with its own install script, and runs this check there with `LENGUA_RELEASE` naming the
//! installed plugin folder (an unpacked archive's folder does as well). It:
//!
//! - finds what the archive ships: `TARGET`, which names this computer's system and CPU; the
//!   manifests, at the version this check was built at, which start `server/lengua-mcp`; both
//!   skills; z3 and cvc5 in `solvers/`; and the licences;
//! - runs `lengua-mcp --version`, and `lengua-mcp doctor`, which must find nothing wrong and the
//!   solvers beside the server, where the archive keeps them;
//! - starts the archive's own lengua-mcp as an MCP server over stdio with rmcp's client, as an
//!   agent harness does, giving it a `PLUGIN_DATA`, lists its tools, `check` and `run`, and takes
//!   add (tests/fixtures-wasm/add) from the user's request to done through `check`: its idea.md,
//!   then its main.wat, with the archive's solvers proving it in the background;
//! - runs add through `run`, through `lengua-mcp run` from a shell, and on Unix through the done
//!   reply's own shell command, given to `sh`;
//! - and attests it with `lengua-mcp check`.
//!
//! The server runs as a user's does, but for a data folder of its own, so the check leaves
//! nothing in the user's. The attester's proofs get a short time: add's whole range takes far
//! longer, which the slow ignored tests prove. This replaces scripts/release-check.py.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Value, json};

/// add's idea, an Attested Computation.
const IDEA: &str = include_str!("fixtures-wasm/add/idea.md");

/// add's main.wat.
const CODE: &str = include_str!("fixtures-wasm/add/main.wat");

/// What a user says to ask for add.
const REQUEST: &str = "Make me a program that adds two numbers";

/// The attester's time for its proofs, in seconds: enough for the archive's solvers to start
/// and answer, where add's whole range takes far longer.
const ATTESTER_PROOF_SECONDS: &str = "20";

/// What a proof's line says when no solver could start.
const NO_SOLVER: [&str; 3] = ["isn't installed", "couldn't start", "no SMT solver"];

/// The knobs lengua's own tests turn on a server, which a user's server runs without.
const KNOBS: [&str; 5] = [
    "LENGUA_PROOF_SECONDS",
    "LENGUA_CHECK_WAIT_MS",
    "LENGUA_GREEN_GRACE_MS",
    "LENGUA_STAMP_DELAY_MS",
    "LENGUA_TARGET",
];

/// A plain MCP client: lengua needs nothing from it but tool calls.
#[derive(Debug, Clone, Copy, Default)]
struct Client;

impl ClientHandler for Client {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("lengua-release-check", env!("CARGO_PKG_VERSION")),
        )
    }
}

/// This computer's system and CPU, as an archive's TARGET names them.
fn target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// How `command` ended, and what it printed.
fn ran(command: &mut Command) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("{command:?} didn't start: {error}"))
}

/// A program's output as text.
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether `folder` holds a file, at any depth.
fn has_a_file(folder: &Path) -> bool {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let path = entry.path();
            path.is_file() || (path.is_dir() && has_a_file(&path))
        })
}

/// The manifest `name` in `plugin`, read.
fn manifest(plugin: &Path, name: &str) -> Value {
    let path = plugin.join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// What the archive ships, in `plugin`: its target, which must be this computer's; the
/// manifests, at this check's version, which start the server; both skills; both solvers, each
/// with its licences; and lengua's licence and the crates'. Gives the server.
fn shipped(plugin: &Path) -> PathBuf {
    let exe = std::env::consts::EXE_SUFFIX;
    let target = std::fs::read_to_string(plugin.join("TARGET"))
        .unwrap_or_else(|error| panic!("{} has no TARGET: {error}", plugin.display()));
    assert_eq!(
        target.trim(),
        self::target(),
        "the archive is for another system or CPU"
    );
    for name in ["plugin.json", ".claude-plugin/plugin.json"] {
        let read = manifest(plugin, name);
        assert_eq!(read["name"], "lengua", "{name}");
        assert_eq!(read["version"], env!("CARGO_PKG_VERSION"), "{name}");
    }
    let started = format!("server/lengua-mcp{exe}");
    assert_eq!(
        manifest(plugin, "mcp.json")["mcpServers"]["lengua"]["command"],
        format!("./{started}")
    );
    assert_eq!(
        manifest(plugin, ".mcp.json")["mcpServers"]["lengua"]["command"],
        format!("${{CLAUDE_PLUGIN_ROOT}}/{started}")
    );
    let server = plugin.join("server").join(format!("lengua-mcp{exe}"));
    assert!(
        server.is_file(),
        "{} isn't in the archive",
        server.display()
    );
    for skill in ["write-idea", "compile"] {
        let file = plugin.join("skills").join(skill).join("SKILL.md");
        assert!(file.is_file(), "{} isn't in the archive", file.display());
    }
    let licences = plugin.join("licenses");
    for solver in ["z3", "cvc5"] {
        let program = plugin.join("solvers").join(format!("{solver}{exe}"));
        assert!(
            program.is_file(),
            "{} isn't in the archive",
            program.display()
        );
        assert!(
            has_a_file(&licences.join(solver)),
            "{solver}'s licences aren't in {}",
            licences.display()
        );
    }
    for file in ["LICENSE", "licenses/README.md"] {
        assert!(plugin.join(file).is_file(), "{file} isn't in the archive");
    }
    let crates =
        std::fs::read_to_string(licences.join("rust-crates.txt")).expect("the crates' licences");
    assert!(
        crates.contains("\n== wasmtime "),
        "the crates' licences don't name wasmtime"
    );
    server
}

/// `lengua-mcp --version` names this check's version, and `lengua-mcp doctor`, with the data
/// folder `data`, finds nothing wrong and both solvers beside the server, where the prover
/// finds them first.
fn version_and_doctor(server: &Path, data: &Path) {
    let version = ran(Command::new(server).arg("--version"));
    assert_eq!(
        text(&version.stdout).trim(),
        format!("lengua-mcp {}", env!("CARGO_PKG_VERSION"))
    );
    let mut doctor = Command::new(server);
    doctor.arg("--data-dir").arg(data).arg("doctor");
    let doctor = ran(&mut doctor);
    let said = text(&doctor.stdout);
    println!("{said}");
    assert_eq!(
        doctor.status.code(),
        Some(0),
        "doctor found something wrong:\n{said}"
    );
    assert_eq!(said.lines().last(), Some("nothing is wrong"), "{said}");
    for solver in ["z3", "cvc5"] {
        let beside = said.lines().any(|line| {
            line.starts_with(&format!("{solver}: "))
                && line.contains("the version lengua pins, beside the server, ")
        });
        assert!(
            beside,
            "the archive's {solver} isn't the one found:\n{said}"
        );
    }
}

/// Calls `tool` with `arguments`, which must succeed, and gives its structured reply.
async fn call(
    service: &RunningService<RoleClient, Client>,
    tool: &'static str,
    arguments: Value,
) -> Value {
    let Value::Object(arguments) = arguments else {
        panic!("{tool}'s arguments aren't an object");
    };
    let result = service
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap_or_else(|error| panic!("calling {tool} failed: {error}"));
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

/// Whether any of `lines`, an array of text, contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines.as_array().is_some_and(|lines| {
        lines
            .iter()
            .any(|line| line.as_str().is_some_and(|line| line.contains(needle)))
    })
}

#[tokio::test]
#[ignore = "the release check: CI's release mode runs it where it installed the archive, which \
            LENGUA_RELEASE names"]
async fn the_archive_works_as_a_user_gets_it() {
    let Some(plugin) = std::env::var_os("LENGUA_RELEASE").map(PathBuf::from) else {
        lengua::skiplog::note(
            "the release check needs LENGUA_RELEASE, an installed or unpacked archive's plugin \
             folder",
        );
        return;
    };
    let server = shipped(&plugin);
    let data = tempfile::tempdir().expect("a data folder");
    version_and_doctor(&server, data.path());

    // The archive's server over stdio, as an agent harness starts it, with the folder the
    // harness gives the plugin.
    let plugin_data = tempfile::tempdir().expect("a PLUGIN_DATA");
    let command = tokio::process::Command::new(&server).configure(|command| {
        command
            .arg("--data-dir")
            .arg(data.path())
            .env("PLUGIN_DATA", plugin_data.path());
        for knob in KNOBS {
            command.env_remove(knob);
        }
    });
    let transport = TokioChildProcess::new(command).expect("start lengua-mcp");
    let service = Client
        .serve(transport)
        .await
        .expect("connect to the archive's lengua-mcp");
    let tools: BTreeSet<String> = service
        .list_all_tools()
        .await
        .expect("list the tools")
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(
        tools,
        BTreeSet::from(["check".to_owned(), "run".to_owned()])
    );

    // add from the user's request to done: its idea first, recorded with their words, then
    // its main.wat, checked until its tests have run.
    let work = tempfile::tempdir().expect("a folder");
    let folder = work.path().join("bundle").join("add");
    std::fs::create_dir_all(&folder).expect("add's folder");
    let path = folder.to_str().expect("a path in UTF-8").to_owned();
    std::fs::write(folder.join("idea.md"), IDEA).expect("write idea.md");
    let recorded = call(
        &service,
        "check",
        json!({ "path": path.as_str(), "request": REQUEST }),
    )
    .await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    std::fs::write(folder.join("main.wat"), CODE).expect("write main.wat");
    let done = loop {
        let reply = call(&service, "check", json!({ "path": path.as_str() })).await;
        if !matches!(reply["state"].as_str(), Some("testing" | "proving")) {
            break reply;
        }
    };
    assert_eq!(done["state"], "done", "{done}");
    assert!(
        any_line(&done["tests"], "passed all 202 test runs"),
        "{done}"
    );
    for words in NO_SOLVER {
        assert!(
            !any_line(&done["proofs"], words),
            "the server's proofs found no solver: {done}"
        );
    }
    println!("release check: add is done; its proofs: {}", done["proofs"]);

    // add runs through run, and from a shell.
    let sum = call(
        &service,
        "run",
        json!({ "path": path.as_str(), "args": ["2", "3"] }),
    )
    .await;
    assert_eq!(sum["stdout"], "5\n", "{sum}");
    assert_eq!(sum["exit_code"], 0, "{sum}");
    let mut shell = Command::new(&server);
    shell.arg("--data-dir").arg(data.path()).arg("run");
    let shell = ran(shell.arg(&folder).args(["2", "3"]));
    assert_eq!(text(&shell.stdout), "5\n", "{shell:?}");
    assert_eq!(shell.status.code(), Some(0), "{shell:?}");
    #[cfg(unix)]
    {
        let command = done["shell"]
            .as_str()
            .expect("the done reply's shell command");
        let given = ran(Command::new("sh").arg("-c").arg(format!("{command} 2 3")));
        assert_eq!(text(&given.stdout), "5\n", "{command}: {given:?}");
        assert_eq!(given.status.code(), Some(0), "{command}: {given:?}");
    }
    service.cancel().await.expect("close the session");
    assert!(
        plugin_data.path().join("cache").is_dir(),
        "the server kept no compiled module in PLUGIN_DATA"
    );

    // The attester, as a consumer of add would run it, with no data folder.
    let mut attest = Command::new(&server);
    attest.arg("check").arg(&folder);
    let attested = ran(attest.env("LENGUA_PROOF_SECONDS", ATTESTER_PROOF_SECONDS));
    let said = text(&attested.stdout);
    println!("{said}");
    assert_eq!(attested.status.code(), Some(0), "{said}");
    assert_eq!(said.lines().last(), Some("attested"), "{said}");
    assert!(said.contains("tests: passed all 202 test runs"), "{said}");
    for words in NO_SOLVER {
        assert!(
            !said.contains(words),
            "the attester's proofs found no solver:\n{said}"
        );
    }
    println!("release check: {} passed", target());
}
