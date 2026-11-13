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
        let mut child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
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
            json!({
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

    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
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
            json!({"name": "check", "arguments": {"folder": folder.to_string_lossy()}}),
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
    let reply = server.call(2, "tools/list", json!({}));
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
    assert_eq!(written.len(), 5);
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
        json!({"name": "check", "arguments": {"folder": "/no/such/lengua/folder"}}),
    );
    assert_eq!(reply["result"]["isError"], true, "{reply:#}");
}
