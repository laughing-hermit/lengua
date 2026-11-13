use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a folder");
    for entry in fs::read_dir(from).expect("the fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("a copy");
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../lengua-idea/tests/fixtures")
        .join(name);
    let to = std::env::temp_dir().join(format!("lengua-mcp-{}-{name}", std::process::id()));
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    to
}

struct Server {
    child: Child,
    input: ChildStdin,
    lines: Receiver<String>,
}

impl Server {
    fn start() -> Self {
        Self::start_with(Command::new(env!("CARGO_BIN_EXE_lengua-mcp")))
    }

    fn start_with(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("lengua-mcp starts");
        let input = child.stdin.take().expect("a standard input");
        let output = child.stdout.take().expect("a standard output");
        let (send, lines) = channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            input,
            lines,
        };
        let reply = server.call(
            1,
            "initialize",
            &json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "lengua-test", "version": "0.1.0"}
            }),
        );
        assert_eq!(reply["result"]["serverInfo"]["name"], "lengua-mcp");
        server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        server
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").expect("a message sent");
        self.input.flush().expect("a flush");
    }

    fn call(&mut self, id: u64, method: &str, params: &Value) -> Value {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(60))
                .expect("a reply within 60 seconds");
            let message: Value = serde_json::from_str(&line).expect("a JSON line");
            if message["id"] == id {
                return message;
            }
        }
    }

    fn check(&mut self, id: u64, folder: &Path) -> Value {
        let reply = self.call(
            id,
            "tools/call",
            &json!({"name": "check", "arguments": {"folder": folder.to_string_lossy()}}),
        );
        reply["result"]["structuredContent"].clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn codes(reply: &Value) -> Vec<String> {
    reply["findings"]
        .as_array()
        .expect("a list of findings")
        .iter()
        .map(|f| f["code"].as_str().expect("a code").to_string())
        .collect()
}

#[test]
fn the_server_lists_the_check_tool() {
    let mut server = Server::start();
    let reply = server.call(2, "tools/list", &json!({}));
    let tools = reply["result"]["tools"]
        .as_array()
        .expect("a list of tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "check");
    assert!(tools[0]["inputSchema"]["properties"]["folder"].is_object());
    assert!(tools[0]["outputSchema"]["properties"]["findings"].is_object());
}

#[test]
fn a_good_folder_is_ok_and_gets_its_wiki() {
    let folder = fixture("good");
    let mut server = Server::start();
    let reply = server.check(2, &folder);
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert_eq!(codes(&reply), Vec::<String>::new());
    assert!(
        reply["checked_by"]
            .as_str()
            .is_some_and(|s| s.starts_with("lengua-mcp "))
    );
    assert!(folder.join("index.md").is_file());
    assert!(folder.join("log.md").is_file());
    let written = reply["written"].as_array().expect("a list of files");
    assert_eq!(written.len(), 7);
    assert_eq!(reply["build"]["state"], "no_code", "{reply:#}");
    assert!(folder.join("Cargo.toml").is_file());
    assert!(folder.join("wit/world.wit").is_file());
}

#[test]
fn a_bad_folder_gives_every_problem_in_one_reply() {
    let folder = fixture("bad");
    let mut server = Server::start();
    let reply = server.check(2, &folder);
    assert_eq!(reply["ok"], false);
    let found = codes(&reply);
    for code in [
        "frontmatter-line",
        "section-unknown",
        "purpose-empty",
        "behaviour-then-missing",
        "behaviour-order",
        "example-string",
        "example-exit-code-missing",
        "case-variable-unknown",
        "constraint-memory-missing",
        "structure-main-missing",
        "module-missing",
        "module-unlisted",
        "uses-unknown",
        "idea-name",
        "ste-word",
        "ste-passive",
        "ste-sentence-length",
    ] {
        assert!(found.iter().any(|c| c == code), "{code} in {found:?}");
    }
    let findings = reply["findings"].as_array().expect("a list of findings");
    let places: Vec<(String, u64)> = findings
        .iter()
        .map(|f| {
            (
                f["file"].as_str().expect("a file").to_string(),
                f["line"].as_u64().expect("a line"),
            )
        })
        .collect();
    let mut sorted = places.clone();
    sorted.sort();
    assert_eq!(places, sorted);
    assert!(!folder.join("index.md").exists());
}

#[test]
fn a_folder_that_is_not_there_is_a_tool_error() {
    let mut server = Server::start();
    let reply = server.call(
        2,
        "tools/call",
        &json!({"name": "check", "arguments": {"folder": "/no/such/lengua/folder"}}),
    );
    assert_eq!(reply["result"]["isError"], true, "{reply:#}");
}

fn example(test: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello");
    let to = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("lengua-mcp-{test}"))
        .join("hello");
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    to
}

fn until_built(server: &mut Server, folder: &Path, base: u64) -> Value {
    for id in base..base + 1000 {
        let reply = server.check(id, folder);
        let state = reply["build"]["state"].as_str().unwrap_or("").to_string();
        if state != "building" {
            return reply;
        }
        assert!(reply["build"]["seconds"].is_u64(), "{reply:#}");
        thread::sleep(Duration::from_secs(1));
    }
    panic!("the build did not end in about 1000 seconds");
}

#[test]
fn hello_world_builds_to_a_component() {
    let folder = example("hello");
    let mut server = Server::start();
    let first = server.check(2, &folder);
    assert_eq!(first["ok"], true, "{first:#}");
    assert_eq!(first["build"]["state"], "building", "{first:#}");
    assert_eq!(first["build"]["target"], "wasm32-wasip2");
    let written = first["written"].as_array().expect("a list of files");
    assert!(written.iter().any(|f| f == "Cargo.toml"), "{first:#}");
    assert!(written.iter().any(|f| f == "wit/world.wit"), "{first:#}");
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "built", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    let component = PathBuf::from(reply["build"]["component"].as_str().expect("a path"));
    assert!(component.ends_with("target/wasm32-wasip2/release/hello.wasm"));
    let bytes = fs::read(&component).expect("the component");
    assert_eq!(
        bytes.get(..8),
        Some(&[0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00][..])
    );
    assert_eq!(
        reply["build"]["size"].as_u64(),
        u64::try_from(bytes.len()).ok()
    );
    let sha = reply["build"]["sha256"].as_str().expect("a hash");
    assert_eq!(sha, lengua_idea::sha256_hex(&bytes));
    let again = server.check(2000, &folder);
    assert_eq!(again["build"]["state"], "built", "{again:#}");
    assert_eq!(again["build"]["sha256"], sha);
}

#[test]
fn a_compile_error_comes_back_as_findings_with_file_and_line() {
    let folder = example("error");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    let count: u32 = \"one\";\n    println!(\"{count}\");\n}\n",
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "build_failed", "{reply:#}");
    assert_eq!(reply["ok"], false);
    let findings = reply["findings"].as_array().expect("a list of findings");
    assert!(
        findings.iter().any(|f| f["file"] == "src/main.rs"
            && f["line"] == 2
            && f["code"] == "rust-error"
            && f["message"].as_str().is_some_and(|m| m.contains("E0308"))),
        "{reply:#}"
    );
}

#[test]
fn a_change_after_a_build_starts_a_new_build() {
    let folder = example("change");
    let mut server = Server::start();
    let built = until_built(&mut server, &folder, 10);
    assert_eq!(built["build"]["state"], "built", "{built:#}");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello, world!\");\n    println!();\n}\n",
    )
    .expect("a file");
    let reply = server.check(3000, &folder);
    assert_eq!(reply["build"]["state"], "building", "{reply:#}");
    let rebuilt = until_built(&mut server, &folder, 4000);
    assert_eq!(rebuilt["build"]["state"], "built", "{rebuilt:#}");
    assert_ne!(rebuilt["build"]["sha256"], built["build"]["sha256"]);
}

#[test]
fn a_hand_edited_cargo_toml_is_a_finding_and_nothing_is_built() {
    let folder = example("edited");
    fs::remove_file(folder.join("src/main.rs")).expect("the code removed");
    let mut server = Server::start();
    let first = server.check(2, &folder);
    assert_eq!(first["build"]["state"], "no_code", "{first:#}");
    let cargo = fs::read_to_string(folder.join("Cargo.toml")).expect("Cargo.toml");
    fs::write(
        folder.join("Cargo.toml"),
        cargo.replace("lto = true", "lto = false"),
    )
    .expect("an edit");
    let reply = server.check(3, &folder);
    assert_eq!(reply["ok"], false);
    assert_eq!(codes(&reply), ["generated-edited"]);
    assert_eq!(reply["findings"][0]["file"], "Cargo.toml");
    assert!(reply["build"].is_null(), "{reply:#}");
}

#[test]
fn a_missing_toolchain_gives_the_doctor_s_message() {
    let folder = example("toolchain");
    let empty = folder.parent().expect("a parent").join("empty-path");
    fs::create_dir_all(&empty).expect("a folder");
    let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
    command.env("PATH", &empty);
    let mut server = Server::start_with(command);
    let reply = server.check(2, &folder);
    assert_eq!(reply["build"]["state"], "build_failed", "{reply:#}");
    assert_eq!(codes(&reply), ["toolchain-missing"]);
    let message = reply["findings"][0]["message"].as_str().expect("a message");
    assert!(message.contains("missing  cargo: not found"), "{message}");
}
