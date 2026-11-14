use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

struct Server {
    child: Child,
    input: ChildStdin,
    lines: Receiver<String>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
            .env(
                "LENGUA_HOME",
                Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-home"),
            )
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
        server.call(
            1,
            "initialize",
            &json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "lengua-wiki", "version": "0.1.0"}
            }),
        );
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

fn folders(root: &Path, group: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(root.join(group))
        .expect("the folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("idea.md").is_file())
        .collect();
    out.sort();
    out
}

#[test]
#[ignore = "writes the wiki of libraries/ and examples/ in place; CI runs it in the mode `wiki`"]
fn the_wiki_of_every_folder_is_written_in_place() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository");
    let mut server = Server::start();
    let mut id = 10;
    let mut summary = Vec::new();
    let mut all = folders(&root, "libraries");
    all.extend(folders(&root, "examples"));
    all.sort_by_key(|f| {
        let text = fs::read_to_string(f.join("idea.md")).unwrap_or_default();
        (
            !text.contains("\n- Exports:"),
            text.contains("\n- Uses: `../"),
        )
    });
    for folder in all {
        let mut reply = Value::Null;
        for _ in 0..5000 {
            id += 1;
            reply = server.check(id, &folder);
            let state = reply["build"]["state"].as_str().unwrap_or("none");
            if !["building", "testing", "mutating"].contains(&state) {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
        let line = format!(
            "{}: {} {} {}",
            folder.strip_prefix(&root).unwrap_or(&folder).display(),
            reply["build"]["state"],
            reply["label"],
            reply["findings"]
        );
        println!("{line}");
        summary.push(line);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("wiki")
        .arg(&root)
        .output()
        .expect("lengua-mcp wiki runs");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(output.status.success(), "{}", summary.join("\n"));
}
