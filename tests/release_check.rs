//! The release check: lengua's release archive for this computer, installed as a user installs
//! it, checked through a plain MCP client. CI's `release` mode builds the archive and this check
//! on each of the five runners; its install job downloads both onto a clean runner of the same
//! system and CPU, which builds nothing, and runs this check there with `LENGUA_RELEASE` naming
//! the folder they were downloaded to. The check then does what a user does, in order:
//!
//! - checks the archive against the SHA-256 beside it, and unpacks it in a fresh folder with the
//!   system's own tar (PowerShell's Expand-Archive on Windows);
//! - has the archive's own `lengua-mcp install --prefix` refuse to replace a lengua folder that
//!   isn't lengua's, which stays as it was;
//! - installs it with `lengua-mcp install`, where it installs for any user, which ends with
//!   doctor, whose exit code it keeps;
//! - and runs the installed `lengua-mcp doctor`, and all that follows, with PATH cut to the
//!   system's own folders, so that nothing the runner's image adds there (Git for Windows' MinGW
//!   libraries, say) stands in for a library the archive should ship.
//!
//! `LENGUA_RELEASE` may name an installed or unpacked archive's plugin folder instead, which the
//! check checks as it is. Of the installed plugin, it:
//!
//! - finds what the archive ships: `TARGET`, which names this computer's system and CPU; the
//!   manifests, at the version this check was built at, which start `server/lengua-mcp`; both
//!   skills; z3 and cvc5 in `solvers/`; and the licences;
//! - runs `lengua-mcp --version`, and `lengua-mcp doctor`, which must find nothing wrong and the
//!   solvers beside the server, where the archive keeps them;
//! - starts the archive's own lengua-mcp as an MCP server over stdio with rmcp's client, as an
//!   agent harness does, giving it a `PLUGIN_DATA`, lists its tools, `check` and `run`, and takes
//!   add (tests/fixtures/add) from the user's request to done through `check`: its idea.md,
//!   then its main.wat, with the archive's solvers proving it in the background;
//! - runs add through `run`, through `lengua-mcp run` from a shell, and on Unix through the done
//!   reply's own shell command, given to `sh`;
//! - and attests it with `lengua-mcp check`.
//!
//! The server runs as a user's does, but for a data folder of its own, so the check leaves
//! nothing in the user's. The attester's proofs get a short time: add's whole range takes far
//! longer, which the slow ignored tests prove. This replaces scripts/release-check.py.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lengua::data_dir::DataDir;
use lengua::hash::Sha256Hex;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Value, json};

/// The check CI's install job runs, by its name.
const CHECK: &str = "the_archive_works_as_a_user_gets_it";

/// add's idea, an Attested Computation.
const IDEA: &str = include_str!("fixtures/add/idea.md");

/// add's main.wat.
const CODE: &str = include_str!("fixtures/add/main.wat");

/// What a user says to ask for add.
const REQUEST: &str = "Make me a program that adds two numbers";

/// The attester's time for its proofs, in seconds: enough for the archive's solvers to start
/// and answer, where add's whole range takes far longer.
const ATTESTER_PROOF_SECONDS: &str = "20";

/// What a proof's line says when no solver could start.
const NO_SOLVER: [&str; 3] = ["isn't installed", "couldn't start", "no SMT solver"];

/// The knobs lengua's own tests turn on a server, which a user's server runs without.
const KNOBS: [&str; 4] = [
    "LENGUA_PROOF_SECONDS",
    "LENGUA_CHECK_WAIT_MS",
    "LENGUA_GREEN_GRACE_MS",
    "LENGUA_STAMP_DELAY_MS",
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

/// `command` with PATH cut to `path`, when the check runs where it installed the archive.
fn with_path<'a>(command: &'a mut Command, path: Option<&OsStr>) -> &'a mut Command {
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
}

/// The PATH of the system's own folders and no others, for the programs the archive ships.
fn bare_path() -> OsString {
    if cfg!(windows) {
        let root = std::env::var_os("SYSTEMROOT").unwrap_or_else(|| OsString::from(r"C:\Windows"));
        let root = PathBuf::from(root);
        std::env::join_paths([root.join("System32"), root]).expect("a PATH")
    } else {
        OsString::from("/usr/bin:/bin")
    }
}

/// The archive downloaded into `folder`, its .tar.gz or else its .zip, with its .sha256 beside
/// it; `None` when `folder` holds none.
fn downloaded_archive(folder: &Path) -> Option<PathBuf> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let archive = |end: &str| {
        names
            .iter()
            .find(|name| name.starts_with("lengua-") && name.ends_with(end))
            .cloned()
    };
    let found = archive(".tar.gz").or_else(|| archive(".zip"))?;
    for name in &names {
        println!("downloaded: {name}");
    }
    Some(folder.join(found))
}

/// Checks `archive` against the SHA-256 its .sha256 names, as `sha256sum -c` does, and unpacks
/// it in `into`, as a user does: with tar, or on Windows with PowerShell's Expand-Archive. Gives
/// the folder it unpacked, named as the archive is.
fn checked_and_unpacked(archive: &Path, into: &Path) -> PathBuf {
    let name = archive
        .file_name()
        .and_then(OsStr::to_str)
        .expect("an archive's name in UTF-8");
    let bytes = std::fs::read(archive).expect("the archive");
    let sums = archive.with_file_name(format!("{name}.sha256"));
    let sums = std::fs::read_to_string(&sums).expect("the archive's .sha256");
    let mut fields = sums.split_whitespace();
    let (sum, named) = (fields.next(), fields.next());
    let digest = Sha256Hex::of(&bytes);
    assert_eq!(named, Some(name), "the .sha256 names another file");
    assert_eq!(sum, Some(digest.as_str()), "{name}'s SHA-256 differs");
    println!("{name}: OK");
    let unpacked = if cfg!(windows) {
        let script = format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}'",
            archive.display(),
            into.display()
        );
        ran(Command::new("powershell").args(["-NoProfile", "-Command", &script]))
    } else {
        ran(Command::new("tar").arg("-C").arg(into).arg("-xzf").arg(archive))
    };
    assert_eq!(unpacked.status.code(), Some(0), "{name}: {unpacked:?}");
    let stem = name
        .strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".zip"))
        .unwrap_or(name);
    into.join(stem)
}

/// A lengua folder in someone else's marketplace folder stays as it was: the archive's
/// `server`'s install, given that folder, refuses to replace it.
fn a_folder_that_isn_t_lengua_s_is_kept(server: &Path) {
    let someone = tempfile::tempdir().expect("someone's folder");
    let theirs = someone.path().join("lengua");
    std::fs::create_dir_all(&theirs).expect("their lengua folder");
    std::fs::write(theirs.join("notes.txt"), "mine\n").expect("their notes");
    let mut install = Command::new(server);
    install.arg("install").arg("--prefix").arg(someone.path());
    let refused = ran(&mut install);
    assert_ne!(
        refused.status.code(),
        Some(0),
        "lengua-mcp install replaced a lengua folder that wasn't lengua's: {refused:?}"
    );
    let notes = std::fs::read_to_string(theirs.join("notes.txt")).expect("their notes");
    assert_eq!(notes, "mine\n");
    let said = text(&refused.stderr);
    let said = said.trim();
    println!("release check: a folder that isn't lengua's is kept: {said}");
}

/// Installs the archive with its `server`'s `lengua-mcp install`, as a user does, where it
/// installs for any user: its exit code is doctor's, which must find nothing wrong. Gives the
/// installed plugin's folder.
fn installed_by(server: &Path) -> PathBuf {
    let status = Command::new(server)
        .arg("install")
        .status()
        .expect("lengua-mcp install starts");
    assert_eq!(status.code(), Some(0), "the install failed: {status}");
    let data = DataDir::from_env().expect("a data folder");
    let installed = data.path().join("plugins").join("lengua");
    let target = installed.join("TARGET");
    assert!(target.is_file(), "{} isn't there", target.display());
    installed
}

/// The installed lengua-mcp's doctor, with PATH cut to `path`, finds nothing wrong.
fn installed_doctor_is_healthy(installed: &Path, path: &OsStr) {
    let exe = std::env::consts::EXE_SUFFIX;
    let mut doctor = Command::new(installed.join("server").join(format!("lengua-mcp{exe}")));
    doctor.arg("doctor").env("PATH", path);
    let said = ran(&mut doctor);
    println!("{}", text(&said.stdout));
    assert_eq!(said.status.code(), Some(0), "doctor found a problem");
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
/// with its licences, cvc5's with those of the parts it links in; and lengua's licence and the
/// crates'. Gives the server.
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
    let mut parts = vec![
        "cvc5/AUTHORS",
        "cvc5/cadical/LICENSE",
        "cvc5/symfpu/LICENSE-BSD",
        "cvc5/libpoly/LICENCE",
    ];
    if cfg!(target_os = "linux") {
        parts.extend(["cvc5/ubuntu/libc6/copyright", "cvc5/ubuntu/LGPL-2.1"]);
    }
    if cfg!(windows) {
        parts.push("cvc5/mingw-w64/COPYING.MinGW-w64-runtime.txt");
    }
    for part in parts {
        assert!(licences.join(part).is_file(), "{part} isn't in the archive");
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
/// folder `data` and PATH cut to `path` when it is given, finds nothing wrong and both solvers
/// beside the server, where the prover finds them first.
fn version_and_doctor(server: &Path, data: &Path, path: Option<&OsStr>) {
    let version = ran(with_path(Command::new(server).arg("--version"), path));
    assert_eq!(
        text(&version.stdout).trim(),
        format!("lengua-mcp {}", env!("CARGO_PKG_VERSION"))
    );
    let mut doctor = Command::new(server);
    doctor.arg("--data-dir").arg(data).arg("doctor");
    let doctor = ran(with_path(&mut doctor, path));
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

/// CI's install job runs this check, by its name, on the archive it downloaded, which
/// `LENGUA_RELEASE` names: with the test or the variable renamed, the job would pass having
/// checked nothing.
#[test]
fn the_install_job_runs_this_check() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let workflow = std::fs::read_to_string(&path).expect("ci.yml");
    let ci: Value = serde_saphyr::from_str(&workflow).expect("ci.yml is YAML");
    let steps = ci["jobs"]["install"]["steps"]
        .as_array()
        .expect("the install job's steps");
    let exact = format!("--exact {CHECK} ");
    let runs = steps.iter().any(|step| {
        let named = step["run"].as_str().is_some_and(|run| run.contains(&exact));
        named && step["env"]["LENGUA_RELEASE"].is_string()
    });
    assert!(runs, "the install job doesn't run {CHECK}");
    let source = include_str!("release_check.rs");
    let defined = format!("async fn {CHECK}()");
    assert!(source.contains(&defined), "no test here is {CHECK}");
}

#[tokio::test]
#[ignore = "the release check: CI's release mode runs it where it downloaded the archive, which \
            LENGUA_RELEASE names"]
async fn the_archive_works_as_a_user_gets_it() {
    let Some(release) = std::env::var_os("LENGUA_RELEASE").map(PathBuf::from) else {
        lengua::skiplog::note(
            "the release check needs LENGUA_RELEASE, the folder an archive was downloaded to or \
             an installed or unpacked archive's plugin folder",
        );
        return;
    };
    // A downloaded archive is checked, unpacked in a fresh folder and installed as a user does;
    // a plugin folder is checked as it is.
    let fresh = tempfile::tempdir().expect("a fresh folder to unpack in");
    let (plugin, path) = match downloaded_archive(&release) {
        Some(archive) => {
            let unpacked = checked_and_unpacked(&archive, fresh.path());
            let exe = std::env::consts::EXE_SUFFIX;
            let server = unpacked.join("server").join(format!("lengua-mcp{exe}"));
            a_folder_that_isn_t_lengua_s_is_kept(&server);
            let installed = installed_by(&server);
            let path = bare_path();
            installed_doctor_is_healthy(&installed, &path);
            (installed, Some(path))
        }
        None => (release, None),
    };
    let path = path.as_deref();
    let server = shipped(&plugin);
    let data = tempfile::tempdir().expect("a data folder");
    version_and_doctor(&server, data.path(), path);

    // The archive's server over stdio, as an agent harness starts it, with the folder the
    // harness gives the plugin.
    let plugin_data = tempfile::tempdir().expect("a PLUGIN_DATA");
    let command = tokio::process::Command::new(&server).configure(|command| {
        command
            .arg("--data-dir")
            .arg(data.path())
            .env("PLUGIN_DATA", plugin_data.path());
        if let Some(path) = path {
            command.env("PATH", path);
        }
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
    let at = folder.to_str().expect("a path in UTF-8").to_owned();
    std::fs::write(folder.join("idea.md"), IDEA).expect("write idea.md");
    let recorded = call(
        &service,
        "check",
        json!({ "path": at.as_str(), "request": REQUEST }),
    )
    .await;
    assert_eq!(recorded["state"], "write_code", "{recorded}");
    std::fs::write(folder.join("main.wat"), CODE).expect("write main.wat");
    let done = loop {
        let reply = call(&service, "check", json!({ "path": at.as_str() })).await;
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
        json!({ "path": at.as_str(), "args": ["2", "3"] }),
    )
    .await;
    assert_eq!(sum["stdout"], "5\n", "{sum}");
    assert_eq!(sum["exit_code"], 0, "{sum}");
    let mut shell = Command::new(&server);
    shell.arg("--data-dir").arg(data.path()).arg("run");
    let shell = ran(with_path(shell.arg(&folder).args(["2", "3"]), path));
    assert_eq!(text(&shell.stdout), "5\n", "{shell:?}");
    assert_eq!(shell.status.code(), Some(0), "{shell:?}");
    #[cfg(unix)]
    {
        let command = done["shell"]
            .as_str()
            .expect("the done reply's shell command");
        let mut sh = Command::new("sh");
        sh.arg("-c").arg(format!("{command} 2 3"));
        let given = ran(with_path(&mut sh, path));
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
    let attested = ran(with_path(
        attest.env("LENGUA_PROOF_SECONDS", ATTESTER_PROOF_SECONDS),
        path,
    ));
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
