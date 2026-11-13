use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

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

fn home() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-home")
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
        if !command.get_envs().any(|(name, _)| name == "LENGUA_HOME") {
            command.env("LENGUA_HOME", home());
        }
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

    fn run(&mut self, id: u64, folder: &Path, input: &Value) -> Value {
        let mut arguments = input.clone();
        arguments["folder"] = json!(folder.to_string_lossy());
        let reply = self.call(
            id,
            "tools/call",
            &json!({"name": "run", "arguments": arguments}),
        );
        reply["result"]["structuredContent"].clone()
    }

    fn run_to_end(&mut self, base: u64, folder: &Path, input: &Value) -> Value {
        for id in base..base + 100 {
            let reply = self.run(id, folder, input);
            if reply["state"] != "running" {
                return reply;
            }
        }
        panic!("the run did not end after 100 calls");
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
    assert_eq!(tools.len(), 2);
    let check = tools
        .iter()
        .find(|t| t["name"] == "check")
        .expect("the check tool");
    assert!(check["inputSchema"]["properties"]["folder"].is_object());
    assert!(check["outputSchema"]["properties"]["findings"].is_object());
    let run = tools
        .iter()
        .find(|t| t["name"] == "run")
        .expect("the run tool");
    for name in ["folder", "arguments", "stdin", "fixed"] {
        assert!(
            run["inputSchema"]["properties"][name].is_object(),
            "{run:#}"
        );
    }
    for name in ["state", "exit_code", "stdout", "stderr", "message"] {
        assert!(
            run["outputSchema"]["properties"][name].is_object(),
            "{run:#}"
        );
    }
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
    example_named("hello", test)
}

fn example_named(name: &str, test: &str) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    let to = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("lengua-mcp-{test}"))
        .join(name);
    if to.exists() {
        fs::remove_dir_all(&to).expect("an old copy removed");
    }
    copy(&from, &to);
    to
}

fn until_built(server: &mut Server, folder: &Path, base: u64) -> Value {
    until_built_within(server, folder, base, 5000)
}

fn until_built_within(server: &mut Server, folder: &Path, base: u64, polls: u64) -> Value {
    let started = Instant::now();
    for id in base..base + polls {
        let reply = server.check(id, folder);
        let state = reply["build"]["state"].as_str().unwrap_or("").to_string();
        if !["building", "testing", "mutating"].contains(&state.as_str()) {
            let line = format!(
                "{} {}: {state} in {:.1} s from the first check, the job's own {} s; tests {}, mutants {}\n",
                folder
                    .parent()
                    .and_then(Path::file_name)
                    .map_or(String::new(), |n| n.to_string_lossy().to_string()),
                folder
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().to_string()),
                started.elapsed().as_secs_f64(),
                reply["build"]["seconds"],
                reply["build"]["tests"],
                reply["build"]["mutants"]
            );
            let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-timings.txt");
            if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log) {
                let _ = file.write_all(line.as_bytes());
            }
            return reply;
        }
        assert!(reply["build"]["seconds"].is_u64(), "{reply:#}");
        thread::sleep(Duration::from_millis(200));
    }
    panic!("the job did not end in {polls} polls of 200 ms or more");
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
    assert!(!folder.join("lengua.lock").exists(), "{first:#}");
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert!(folder.join("lengua.lock").is_file(), "{reply:#}");
    let written = reply["written"].as_array().expect("a list of files");
    assert!(written.iter().any(|f| f == "lengua.lock"), "{reply:#}");
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
    assert_eq!(again["build"]["state"], "done", "{again:#}");
    assert_eq!(again["label"], "checked", "{again:#}");
    assert!(again["score"].is_null(), "{again:#}");
    assert_eq!(again["build"]["tests"]["total"], 1, "{again:#}");
    assert_eq!(again["build"]["mutants"]["total"], 0, "{again:#}");
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
    assert_eq!(built["build"]["state"], "done", "{built:#}");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    let text = \"Hello, world!\";\n    println!(\"{text}\");\n}\n",
    )
    .expect("a file");
    let reply = server.check(3000, &folder);
    assert_eq!(reply["build"]["state"], "building", "{reply:#}");
    let rebuilt = until_built(&mut server, &folder, 4000);
    assert_eq!(rebuilt["build"]["state"], "done", "{rebuilt:#}");
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

fn built(name: &str, test: &str) -> (Server, PathBuf, Value) {
    let folder = example_named(name, test);
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    (server, folder, reply)
}

#[test]
fn hello_world_runs_under_its_grants_and_gets_no_input() {
    let (mut server, folder, _) = built("hello", "run-hello");
    let reply = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(reply["state"], "exited", "{reply:#}");
    assert_eq!(reply["exit_code"], 0, "{reply:#}");
    assert_eq!(reply["stdout"], "Hello, world!\n", "{reply:#}");
    assert_eq!(reply["stderr"], "", "{reply:#}");
    let arguments = server.run(6000, &folder, &json!({"arguments": ["one"]}));
    assert_eq!(arguments["state"], "refused", "{arguments:#}");
    assert!(
        arguments["message"]
            .as_str()
            .is_some_and(|m| m.contains("`- Reads: arguments`")),
        "{arguments:#}"
    );
    let stdin = server.run(6001, &folder, &json!({"stdin": "text"}));
    assert_eq!(stdin["state"], "refused", "{stdin:#}");
    assert!(
        stdin["message"]
            .as_str()
            .is_some_and(|m| m.contains("`- Reads: standard input`")),
        "{stdin:#}"
    );
}

#[test]
fn granted_arguments_reach_the_program() {
    let (mut server, folder, _) = built("echo", "run-echo");
    let reply = server.run_to_end(5000, &folder, &json!({"arguments": ["one", "two words"]}));
    assert_eq!(reply["state"], "exited", "{reply:#}");
    assert_eq!(reply["stdout"], "one\ntwo words\n", "{reply:#}");
    let none = server.run_to_end(6000, &folder, &json!({}));
    assert_eq!(none["stdout"], "", "{none:#}");
}

#[test]
fn granted_standard_input_reaches_the_program() {
    let (mut server, folder, _) = built("count", "run-count");
    let reply = server.run_to_end(5000, &folder, &json!({"stdin": "hello world\n"}));
    assert_eq!(reply["state"], "exited", "{reply:#}");
    assert_eq!(reply["stdout"], "2\n", "{reply:#}");
    let empty = server.run_to_end(6000, &folder, &json!({}));
    assert_eq!(empty["stdout"], "0\n", "{empty:#}");
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(windows)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, at)
}

#[test]
fn a_granted_folder_is_read_and_written_and_nothing_outside_it_is() {
    let (mut server, folder, _) = built("files", "run-files");
    fs::create_dir_all(folder.join("data")).expect("a folder");
    fs::write(folder.join("data/note.txt"), "a note\n").expect("a file");
    link(&Path::new("..").join("idea.md"), &folder.join("data/link")).expect("a relative link");
    link(&folder.join("idea.md"), &folder.join("data/far")).expect("an absolute link");
    let absolute = folder.join("idea.md").to_string_lossy().to_string();
    let paths = [
        "data/note.txt",
        "../idea.md",
        "data/../idea.md",
        absolute.as_str(),
        "data/link",
        "data/far",
    ];
    let reply = server.run_to_end(5000, &folder, &json!({"arguments": paths}));
    assert_eq!(reply["state"], "exited", "{reply:#}");
    let expected = format!(
        "a note\ncannot read ../idea.md\ncannot read data/../idea.md\ncannot read {absolute}\ncannot read data/link\ncannot read data/far\nwrite in data: no\n"
    );
    assert_eq!(reply["stdout"], expected, "{reply:#}");
    assert_eq!(
        fs::read_to_string(folder.join("out/count.txt")).expect("the count"),
        "1\n"
    );
    assert!(!folder.join("data/new.txt").exists());
}

#[test]
fn an_ungranted_import_is_refused_at_check_and_at_run() {
    let folder = example_named("ungranted", "run-ungranted");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "build_failed", "{reply:#}");
    let found = codes(&reply);
    assert!(!found.is_empty(), "{reply:#}");
    assert!(found.iter().all(|c| c == "import-ungranted"), "{reply:#}");
    let findings = reply["findings"].as_array().expect("a list of findings");
    assert!(
        findings.iter().any(|f| f["message"]
            .as_str()
            .is_some_and(|m| m.contains("`wasi:filesystem/") && m.contains("`- Writes: files in"))),
        "{reply:#}"
    );
    let world = fs::read_to_string(folder.join("wit/world.wit")).expect("the world");
    assert!(!world.contains("wasi:filesystem"), "{world}");
    let run = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(run["state"], "refused", "{run:#}");
    assert!(
        run["message"].as_str().is_some_and(
            |m| m.contains("there is no lock `lengua.lock`") && m.contains("run `check`")
        ),
        "{run:#}"
    );
    assert_eq!(run["stdout"], "", "{run:#}");
    assert!(!folder.join("note.txt").exists());
}

#[test]
fn an_endless_loop_stops_at_the_time_limit() {
    let (mut server, folder, checked) = built("forever", "run-forever");
    assert_eq!(checked["ok"], true, "{checked:#}");
    assert_eq!(checked["build"]["tests"]["failed"], 0, "{checked:#}");
    assert_ne!(checked["label"], "unchecked", "{checked:#}");
    let reply = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(reply["state"], "out_of_time", "{reply:#}");
    let taken = reply["milliseconds"].as_u64().expect("a time");
    assert!((200..20_000).contains(&taken), "{reply:#}");
}

#[test]
fn memory_past_the_limit_stops_the_program() {
    let (mut server, folder, _) = built("greedy", "run-greedy");
    let reply = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(reply["state"], "out_of_memory", "{reply:#}");
    assert_eq!(reply["stdout"], "", "{reply:#}");
}

#[test]
fn the_clock_and_random_numbers_are_fixed_for_tests_and_real_when_granted() {
    let (mut server, folder, _) = built("clock", "run-clock");
    let first = server.run_to_end(5000, &folder, &json!({"fixed": true}));
    assert_eq!(first["state"], "exited", "{first:#}");
    let second = server.run_to_end(6000, &folder, &json!({"fixed": true}));
    assert_eq!(first["stdout"], second["stdout"], "{first:#} {second:#}");
    let text = first["stdout"].as_str().expect("a text");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines.first().copied(), Some("1767225600"), "{text}");
    assert!(
        lines.get(1).is_some_and(|l| l.parse::<u64>().is_ok()),
        "{text}"
    );
    let live = server.run_to_end(7000, &folder, &json!({}));
    let seconds: u64 = live["stdout"]
        .as_str()
        .and_then(|t| t.lines().next())
        .and_then(|l| l.parse().ok())
        .expect("the seconds of the clock");
    assert!(seconds > 1_767_225_600, "{live:#}");
}

#[test]
fn code_that_breaks_a_rule_of_lengua_s_lint_is_refused_before_the_build() {
    let folder = example("lint");
    fs::write(
        folder.join("src/main.rs"),
        "// a note\nfn main() {\n    println!(\"Hello, world!\");\n}\n",
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = server.check(2, &folder);
    assert_eq!(reply["ok"], false, "{reply:#}");
    assert_eq!(codes(&reply), ["rule-11-comment"], "{reply:#}");
    assert_eq!(reply["findings"][0]["file"], "src/main.rs");
    assert_eq!(reply["findings"][0]["line"], 1);
    assert!(reply["build"].is_null(), "{reply:#}");
}

#[test]
fn code_that_rustfmt_would_change_is_a_finding() {
    let folder = example("format");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello, world!\") ;\n}\n",
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "build_failed", "{reply:#}");
    assert_eq!(codes(&reply), ["rust-format"], "{reply:#}");
    assert_eq!(reply["findings"][0]["file"], "src/main.rs", "{reply:#}");
}

#[test]
fn clippy_holds_rule_7_and_its_errors_are_findings() {
    let folder = example("clippy");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    let text = std::env::args().next();\n    println!(\"{}\", text.unwrap());\n}\n",
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "build_failed", "{reply:#}");
    let findings = reply["findings"].as_array().expect("a list of findings");
    assert!(
        findings.iter().any(|f| f["file"] == "src/main.rs"
            && f["line"] == 3
            && f["code"] == "rust-error"
            && f["message"].as_str().is_some_and(|m| m.contains("unwrap"))),
        "{reply:#}"
    );
}

fn line_of(folder: &Path, file: &str, text: &str) -> usize {
    fs::read_to_string(folder.join(file))
        .expect("the file")
        .lines()
        .position(|l| l == text)
        .expect("the line")
        + 1
}

fn notes_with(reply: &Value, part: &str) -> Vec<Value> {
    reply["notes"]
        .as_array()
        .expect("a list of notes")
        .iter()
        .filter(|n| n["message"].as_str().is_some_and(|m| m.contains(part)))
        .cloned()
        .collect()
}

#[test]
fn a_wrong_output_fails_its_example_with_what_was_expected_and_what_came() {
    let folder = example("wrong");
    fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello, World!\");\n}\n",
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "tests_failed", "{reply:#}");
    assert_eq!(reply["ok"], false, "{reply:#}");
    assert_eq!(reply["label"], "unchecked", "{reply:#}");
    assert_eq!(codes(&reply), ["test-failed"], "{reply:#}");
    let finding = &reply["findings"][0];
    assert_eq!(finding["file"], "idea.md");
    assert_eq!(
        finding["line"],
        line_of(&folder, "idea.md", "### Example: no input")
    );
    let message = finding["message"].as_str().expect("a message");
    assert!(
        message.contains("the example `no input` of the behaviour `Write the message` failed"),
        "{message}"
    );
    assert!(
        message.contains("standard output: expected \"Hello, world!\\n\" and got \"Hello, World!\\n\" (they differ at character 8)"),
        "{message}"
    );
    assert_eq!(reply["build"]["tests"]["failed"], 1, "{reply:#}");
    assert!(reply["build"]["sha256"].is_string(), "{reply:#}");
    assert!(reply["build"]["mutants"].is_null(), "{reply:#}");
    let index = fs::read_to_string(folder.join("index.md")).expect("the index");
    assert!(index.contains("Label: `unchecked`."), "{index}");
    assert!(!folder.join("lengua.lock").exists(), "{reply:#}");
}

#[test]
fn a_case_at_a_boundary_fails_with_its_drawn_value() {
    let folder = example_named("small", "boundary");
    let code = fs::read_to_string(folder.join("src/main.rs")).expect("the code");
    fs::write(
        folder.join("src/main.rs"),
        code.replace("number <= 9", "number < 9"),
    )
    .expect("a file");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "tests_failed", "{reply:#}");
    assert_eq!(codes(&reply), ["case-failed"], "{reply:#}");
    let finding = &reply["findings"][0];
    assert_eq!(
        finding["line"],
        line_of(&folder, "idea.md", "### Cases: numbers of one digit")
    );
    let message = finding["message"].as_str().expect("a message");
    assert!(
        message.contains("the case with `n` = 9 of `numbers of one digit`"),
        "{message}"
    );
    assert!(
        message.contains("expected \"small\\n\" and got \"large\\n\""),
        "{message}"
    );
}

#[test]
fn a_well_tested_program_is_checked_and_its_label_goes_into_the_wiki() {
    let folder = example_named("small", "checked");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert_eq!(reply["label"], "checked", "{reply:#}");
    assert_eq!(reply["score"], 100, "{reply:#}");
    let mutants = &reply["build"]["mutants"];
    assert!(
        mutants["total"].as_u64().is_some_and(|n| n >= 4),
        "{reply:#}"
    );
    assert_eq!(mutants["survived"], 0, "{reply:#}");
    assert_eq!(mutants["done"], mutants["total"], "{reply:#}");
    assert!(
        notes_with(&reply, "a mutant survived").is_empty(),
        "{reply:#}"
    );
    assert!(
        reply["build"]["tests"]["total"]
            .as_u64()
            .is_some_and(|n| n >= 6),
        "{reply:#}"
    );
    let index = fs::read_to_string(folder.join("index.md")).expect("the index");
    assert!(index.contains("Label: `checked`."), "{index}");
    let idea = fs::read_to_string(folder.join("idea.md")).expect("the idea");
    assert!(idea.contains("\nlabel: checked\n"), "{idea}");
    let log = fs::read_to_string(folder.join("log.md")).expect("the log");
    assert!(
        log.contains("no problems; label `checked`, mutation score "),
        "{log}"
    );
    let code = fs::read_to_string(folder.join("src/main.rs")).expect("the code");
    fs::write(
        folder.join("src/main.rs"),
        code.replace(
            "    let argument = std::env::args().nth(1).unwrap_or_default();",
            "    let argument: String = std::env::args().nth(1).unwrap_or_default();",
        ),
    )
    .expect("a file");
    let again = until_built(&mut server, &folder, 6000);
    assert_eq!(again["build"]["state"], "done", "{again:#}");
    assert_eq!(again["label"], "checked", "{again:#}");
    let cached = again["build"]["mutants"]["cached"]
        .as_u64()
        .expect("a count");
    let total = again["build"]["mutants"]["total"]
        .as_u64()
        .expect("a count");
    assert!(cached > 0 && cached < total, "{again:#}");
}

const WEAK: &str = "# Purpose

The program tells if a number is small. It reads the number from its first argument.

# Behaviours

## Write small for a small number

Given the argument 3,
When the program runs,
Then it writes `small` and a line end to the standard output.

### Example: the number 3

- Arguments: \"3\"
- Expect standard output: \"small\\n\"
- Expect exit code: 0

# Constraints

- Reads: arguments
- Memory: 16 MiB
- Time: 2 s

# Structure

- `main`: reads the first argument as a number and writes its size.
- `size`: gives `small` for a number of 9 or less, and `large` for a number that is more than 9.
";

#[test]
fn a_weakly_tested_program_is_tested_and_its_surviving_mutants_are_notes() {
    let folder = example_named("small", "weak");
    fs::write(folder.join("idea.md"), WEAK).expect("the idea");
    let mut server = Server::start();
    let reply = until_built(&mut server, &folder, 10);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert_eq!(reply["label"], "tested", "{reply:#}");
    assert!(reply["score"].as_u64().is_some_and(|s| s < 80), "{reply:#}");
    let survivors = notes_with(&reply, "a mutant survived");
    assert!(!survivors.is_empty(), "{reply:#}");
    let line = line_of(
        &folder,
        "src/main.rs",
        "    let name = if number <= 9 { \"small\" } else { \"large\" };",
    );
    assert!(
        survivors.iter().any(|n| n["file"] == "src/main.rs"
            && n["line"] == line
            && n["message"]
                .as_str()
                .is_some_and(|m| m.contains("in `size`, `<=` became `<`"))),
        "{reply:#}"
    );
    let index = fs::read_to_string(folder.join("index.md")).expect("the index");
    assert!(index.contains("Label: `tested`."), "{index}");
}

#[test]
fn a_hash_map_works_without_a_grant_of_random_numbers() {
    let (mut server, folder, reply) = built("words", "run-words");
    assert_eq!(reply["ok"], true, "{reply:#}");
    let world = fs::read_to_string(folder.join("wit/world.wit")).expect("the world");
    assert!(world.contains("wasi:random/insecure-seed"), "{world}");
    assert!(!world.contains("wasi:random/random"), "{world}");
    let first = server.run_to_end(5000, &folder, &json!({"stdin": "b a b c\n"}));
    assert_eq!(first["state"], "exited", "{first:#}");
    assert_eq!(first["stdout"], "3\n", "{first:#}");
}

fn lengua_run() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .with_file_name(format!("lengua-run{}", std::env::consts::EXE_SUFFIX))
}

fn run_lengua_run(folder: &Path, args: &[&str], home_folder: &Path) -> (std::process::Output, f64) {
    let started = Instant::now();
    let output = Command::new(lengua_run())
        .arg(folder)
        .args(args)
        .env("LENGUA_HOME", home_folder)
        .stdin(Stdio::null())
        .output()
        .expect("lengua-run starts");
    (output, started.elapsed().as_secs_f64())
}

fn sealed_file(folder: &Path) -> String {
    fs::read_to_string(folder.join("lengua.lock"))
        .expect("the lock")
        .lines()
        .find_map(|l| l.strip_prefix("sealed "))
        .and_then(|rest| rest.split_once(' '))
        .map(|(_, path)| path.to_string())
        .expect("a sealed line")
}

fn refused_for(reply: &Value, part: &str) {
    assert_eq!(reply["state"], "refused", "{reply:#}");
    let message = reply["message"].as_str().expect("a message");
    assert!(message.contains(part), "{part} in {message}");
    assert!(message.contains("run `check`"), "{message}");
}

#[test]
fn a_changed_byte_in_each_locked_file_is_refused_at_run() {
    let (mut server, folder, _) = built("hello", "lock-files");
    let lock = fs::read_to_string(folder.join("lengua.lock")).expect("the lock");
    assert!(
        lock.starts_with("lengua-lock 1\nkind program\nlengua "),
        "{lock}"
    );
    assert!(lock.contains("\nlabel checked\nscore 0 of 0\n"), "{lock}");
    assert!(lock.contains("\ntoolchain rustc "), "{lock}");
    let good = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(good["stdout"], "Hello, world!\n", "{good:#}");
    let sealed = sealed_file(&folder);
    assert!(sealed.starts_with("target/lengua/sealed/"), "{sealed}");
    let files = [
        "idea.md",
        "index.md",
        "log.md",
        "src/main.rs",
        "Cargo.toml",
        "wit/world.wit",
        "Cargo.lock",
        "target/wasm32-wasip2/release/hello.wasm",
        sealed.as_str(),
    ];
    let mut id = 6000;
    for file in files {
        let path = folder.join(file);
        let bytes = fs::read(&path).expect("a locked file");
        let mut changed = bytes.clone();
        let middle = changed.len() / 2;
        changed[middle] ^= 1;
        fs::write(&path, &changed).expect("a changed byte");
        let reply = server.run(id, &folder, &json!({}));
        refused_for(&reply, &format!("`{file}` changed"));
        fs::write(&path, &bytes).expect("the file back");
        let again = server.run_to_end(id + 1, &folder, &json!({}));
        assert_eq!(again["state"], "exited", "{file}: {again:#}");
        id += 10;
    }
    fs::write(folder.join("src/extra.rs"), "\n").expect("a new file");
    let reply = server.run(7000, &folder, &json!({}));
    refused_for(&reply, "`src/extra.rs` is new");
    fs::remove_file(folder.join("src/extra.rs")).expect("removed");
    let (output, _) = run_lengua_run(&folder, &[], &home());
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    fs::write(folder.join("idea.md"), "# Purpose\n").expect("a changed idea");
    let (output, _) = run_lengua_run(&folder, &[], &home());
    assert_eq!(output.status.code(), Some(125), "{output:?}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains("`idea.md` changed") && message.contains("run `check`"),
        "{message}"
    );
}

#[test]
fn a_lock_signed_with_another_key_is_refused() {
    let (_, folder, _) = built("hello", "lock-key");
    let other = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-other-home");
    fs::create_dir_all(&other).expect("a folder");
    fs::write(other.join("key"), format!("{}\n", "ab".repeat(32))).expect("another key");
    let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
    command.env("LENGUA_HOME", &other);
    let mut server = Server::start_with(command);
    let reply = server.run(2, &folder, &json!({}));
    refused_for(&reply, "not signed with this user's key");
    let (output, _) = run_lengua_run(&folder, &[], &other);
    assert_eq!(output.status.code(), Some(125), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("not signed with this user's key"));
}

#[test]
fn lengua_run_runs_a_checked_folder_with_arguments_and_a_granted_folder() {
    let (_, hello, _) = built("hello", "lengua-run-hello");
    let (output, _) = run_lengua_run(&hello, &[], &home());
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, world!\n");
    let (output, _) = run_lengua_run(&hello, &["one"], &home());
    assert_eq!(output.status.code(), Some(125), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("`- Reads: arguments`"));
    let (_, files, _) = built("files", "lengua-run-files");
    fs::create_dir_all(files.join("data")).expect("a folder");
    fs::write(files.join("data/note.txt"), "a note\n").expect("a file");
    let (output, _) = run_lengua_run(&files, &["data/note.txt", "../idea.md"], &home());
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a note\ncannot read ../idea.md\nwrite in data: no\n"
    );
    assert_eq!(
        fs::read_to_string(files.join("out/count.txt")).expect("the count"),
        "1\n"
    );
    let (_, count, _) = built("count", "lengua-run-count");
    let output = Command::new(lengua_run())
        .arg(&count)
        .env("LENGUA_HOME", home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut input) = child.stdin.take() {
                input.write_all(b"one two three\n")?;
            }
            child.wait_with_output()
        })
        .expect("lengua-run runs");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3\n");
}

#[test]
fn a_program_that_waits_in_the_host_is_stopped_at_its_time() {
    let (mut server, folder, checked) = built("sleepy", "sleepy");
    assert_eq!(checked["ok"], true, "{checked:#}");
    assert_eq!(checked["build"]["tests"]["failed"], 0, "{checked:#}");
    assert_ne!(checked["label"], "unchecked", "{checked:#}");
    let reply = server.run_to_end(5000, &folder, &json!({}));
    assert_eq!(reply["state"], "out_of_time", "{reply:#}");
    assert_eq!(reply["stdout"], "", "{reply:#}");
    let taken = reply["milliseconds"].as_u64().expect("a time");
    assert!((1000..10_000).contains(&taken), "{reply:#}");
    let (output, seconds) = run_lengua_run(&folder, &[], &home());
    assert_eq!(output.status.code(), Some(124), "{output:?}");
    assert!(seconds < 10.0, "{seconds}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("out of time"));
}

#[test]
fn an_application_of_three_modules_is_checked_as_one_file() {
    let (mut server, folder, reply) = built("tally", "tally");
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert_eq!(reply["label"], "checked", "{reply:#}");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    assert!(
        reply["build"]["tests"]["total"]
            .as_u64()
            .is_some_and(|n| n >= 6),
        "{reply:#}"
    );
    let lock = fs::read_to_string(folder.join("lengua.lock")).expect("the lock");
    for file in [
        "idea.md",
        "ideas/lines.md",
        "ideas/report.md",
        "ideas/words.md",
        "src/main.rs",
        "src/lines.rs",
        "src/report.rs",
        "src/words.rs",
    ] {
        assert!(lock.contains(&format!(" {file}\n")), "{file} in {lock}");
    }
    assert_eq!(
        lock.matches("\ncomponent ").count(),
        1,
        "one component: {lock}"
    );
    let run = server.run_to_end(5000, &folder, &json!({"stdin": "one two\nthree\n"}));
    assert_eq!(run["stdout"], "lines 2, words 3, maximum 5\n", "{run:#}");
    fs::write(folder.join("src/extra.rs"), "\n").expect("a file of no idea");
    let refused = server.check(6000, &folder);
    assert_eq!(codes(&refused), ["rule-12-source-unlisted"], "{refused:#}");
    let run = server.run(6001, &folder, &json!({"stdin": "a\n"}));
    refused_for(&run, "`src/extra.rs` is new");
}

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn bundle(test: &str, parts: &[&str]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lengua-mcp-{test}"));
    if root.exists() {
        fs::remove_dir_all(&root).expect("an old copy removed");
    }
    for part in parts {
        copy(&repo(part), &root.join(part));
    }
    root
}

fn done(server: &mut Server, folder: &Path, base: u64) -> Value {
    let reply = until_built(server, folder, base);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    reply
}

fn shared_library(name: &str) {
    let path = format!("libraries/{name}");
    let root = bundle(&format!("lib-{name}"), &[path.as_str()]);
    let folder = root.join(&path);
    let mut server = Server::start();
    let reply = done(&mut server, &folder, 10);
    assert_eq!(reply["label"], "checked", "{reply:#}");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    assert!(
        reply["build"]["component"]
            .as_str()
            .is_some_and(|c| Path::new(c)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("rlib"))),
        "{reply:#}"
    );
    let lock = fs::read_to_string(folder.join("lengua.lock")).expect("the lock");
    assert!(lock.contains("\nkind library\n"), "{lock}");
    assert!(lock.contains("\nlabel checked\n"), "{lock}");
    assert!(!lock.contains("\nsealed "), "{lock}");
    let cargo = fs::read_to_string(folder.join("Cargo.toml")).expect("Cargo.toml");
    assert!(cargo.contains("[lib]\n"), "{cargo}");
    assert!(!cargo.contains("[dependencies]"), "{cargo}");
}

#[test]
fn the_shared_library_json_is_checked() {
    shared_library("json");
}

#[test]
fn the_shared_library_http_is_checked() {
    shared_library("http");
}

#[test]
fn the_shared_library_decimal_is_checked() {
    shared_library("decimal");
}

#[test]
fn the_shared_library_text_is_checked() {
    shared_library("text");
}

#[test]
fn the_shared_library_sort_is_checked() {
    shared_library("sort");
}

#[test]
fn the_shared_library_hashes_is_checked() {
    shared_library("hashes");
}

#[test]
fn the_shared_library_dates_is_checked() {
    shared_library("dates");
}

#[test]
fn a_program_uses_a_checked_library_and_refuses_one_that_changed() {
    let root = bundle("uses", &["libraries/decimal", "examples/prices"]);
    let library = root.join("libraries/decimal");
    let program = root.join("examples/prices");
    let mut server = Server::start();
    let early = server.check(2, &program);
    assert_eq!(codes(&early), ["library-lock"], "{early:#}");
    done(&mut server, &library, 10);
    let reply = done(&mut server, &program, 3000);
    assert_eq!(reply["label"], "checked", "{reply:#}");
    assert_eq!(
        reply["libraries"],
        json!([{"path": "../../libraries/decimal", "label": "checked"}]),
        "{reply:#}"
    );
    let lock = fs::read_to_string(program.join("lengua.lock")).expect("the lock");
    assert!(
        lock.contains(" ../../libraries/decimal/lengua.lock\n"),
        "{lock}"
    );
    let index = fs::read_to_string(program.join("index.md")).expect("the index");
    assert!(
        index.contains("\n# Libraries\n\n* [decimal](../../libraries/decimal/idea.md) - ")
            && index.contains("Label: `checked`."),
        "{index}"
    );
    let run = server.run_to_end(6000, &program, &json!({"stdin": "1.50\n2.25\n"}));
    assert_eq!(run["stdout"], "3.75\n", "{run:#}");
    let code = fs::read_to_string(library.join("src/main.rs")).expect("the code");
    fs::write(library.join("src/main.rs"), format!("{code}\n")).expect("a change");
    let refused = server.check(7000, &program);
    assert!(
        codes(&refused).contains(&"library-lock".to_string()),
        "{refused:#}"
    );
    let run = server.run(7001, &program, &json!({"stdin": "1\n"}));
    refused_for(&run, "the library of `../../libraries/decimal/lengua.lock`");
}

#[test]
fn a_connection_reaches_only_the_granted_address() {
    let (mut server, folder, reply) = built("fetch", "fetch");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    assert_ne!(reply["label"], "unchecked", "{reply:#}");
    let world = fs::read_to_string(folder.join("wit/world.wit")).expect("the world");
    assert!(world.contains("wasi:sockets/tcp@"), "{world}");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    let run = server.run_to_end(
        5000,
        &folder,
        &json!({"arguments": [format!("127.0.0.1:{port}")]}),
    );
    assert_eq!(run["stdout"], "refused\n", "{run:#}");
    assert_eq!(run["exit_code"], 1, "{run:#}");
}

#[test]
fn a_program_listens_only_on_its_granted_port() {
    let (_, _, reply) = built("upper", "upper");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    assert_ne!(reply["label"], "unchecked", "{reply:#}");
}

fn http_get(port: u16, path: &str) -> String {
    let started = Instant::now();
    loop {
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
            stream.write_all(request.as_bytes()).expect("a request");
            let mut answer = String::new();
            let _ = std::io::Read::read_to_string(&mut stream, &mut answer);
            return answer;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "no server");
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_server_answers_in_its_tests_with_run_and_with_lengua_run() {
    let (mut server, folder, reply) = built("greet", "greet");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    assert_ne!(reply["label"], "unchecked", "{reply:#}");
    let world = fs::read_to_string(folder.join("wit/world.wit")).expect("the world");
    assert!(
        world.contains("export wasi:http/incoming-handler@"),
        "{world}"
    );
    let run = server.run_to_end(
        5000,
        &folder,
        &json!({"stdin": "GET /hello?name=Lin HTTP/1.1\r\nhost: x\r\n\r\n"}),
    );
    assert_eq!(run["state"], "exited", "{run:#}");
    let answer = run["stdout"].as_str().expect("an answer");
    assert!(
        answer.starts_with("HTTP/1.1 200 OK\r\n") && answer.ends_with("\r\n\r\nHello, Lin!"),
        "{answer}"
    );
    let mut child = Command::new(lengua_run())
        .arg(&folder)
        .env("LENGUA_HOME", home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("lengua-run starts");
    let answer = http_get(7810, "/hello?name=Ada");
    let missing = http_get(7810, "/other");
    let _ = child.kill();
    let _ = child.wait();
    assert!(answer.ends_with("Hello, Ada!"), "{answer}");
    assert!(missing.starts_with("HTTP/1.1 404 "), "{missing}");
}

#[test]
fn a_shared_library_that_breaks_a_rule_is_refused() {
    let root = bundle("lib-broken", &["libraries/text"]);
    let folder = root.join("libraries/text");
    let code = fs::read_to_string(folder.join("src/main.rs")).expect("the code");
    fs::write(folder.join("src/main.rs"), format!("// a note\n{code}")).expect("a comment");
    let mut server = Server::start();
    let reply = server.check(2, &folder);
    assert_eq!(codes(&reply), ["rule-11-comment"], "{reply:#}");
    assert!(reply["build"].is_null(), "{reply:#}");
}

#[test]
fn the_shop_is_one_file_of_three_modules_two_libraries_and_a_c_component() {
    let root = bundle(
        "shop",
        &["libraries/json", "libraries/decimal", "examples/shop"],
    );
    let shop = root.join("examples/shop");
    assert!(
        shop.join("components/tax.wasm").is_file(),
        "the CI job `component` builds components/tax.wasm with wasi-sdk"
    );
    let mut server = Server::start();
    done(&mut server, &root.join("libraries/json"), 10);
    done(&mut server, &root.join("libraries/decimal"), 3000);
    let reply = done(&mut server, &shop, 6000);
    assert_eq!(reply["label"], "checked", "{reply:#}");
    assert_eq!(
        reply["libraries"],
        json!([
            {"path": "../../libraries/json", "label": "checked"},
            {"path": "../../libraries/decimal", "label": "checked"}
        ]),
        "{reply:#}"
    );
    assert_eq!(
        reply["components"],
        json!([{"path": "components/tax.wasm", "label": "trusted"}]),
        "{reply:#}"
    );
    assert!(
        !notes_with(&reply, "uses a trusted component").is_empty(),
        "{reply:#}"
    );
    let lock = fs::read_to_string(shop.join("lengua.lock")).expect("the lock");
    for line in [
        "\ncomponent ",
        "\nsealed ",
        "\ntrusted ",
        "\nsealed-trusted ",
        " ../../libraries/json/lengua.lock\n",
        " ../../libraries/decimal/lengua.lock\n",
        " ideas/basket.md\n",
        " src/pages.rs\n",
    ] {
        assert!(lock.contains(line), "{line} in {lock}");
    }
    assert_eq!(lock.matches("\ncomponent ").count(), 1, "{lock}");
    let index = fs::read_to_string(shop.join("index.md")).expect("the index");
    for part in [
        "# Program",
        "# Modules",
        "# Libraries",
        "# Components",
        "Label: `trusted`.",
        "Label: `checked`.",
    ] {
        assert!(index.contains(part), "{part} in {index}");
    }
    let run = server.run_to_end(
        9000,
        &shop,
        &json!({"stdin": "GET /total?tea=2&cake=1 HTTP/1.1\r\nhost: x\r\n\r\n"}),
    );
    let answer = run["stdout"].as_str().expect("an answer");
    assert!(
        answer.ends_with("{\"net\":\"8.20\",\"tax\":\"0.66\",\"total\":\"8.86\"}"),
        "{run:#}"
    );
    let mut child = Command::new(lengua_run())
        .arg(&shop)
        .env("LENGUA_HOME", home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("lengua-run starts");
    let started = Instant::now();
    let answer_of_lengua_run = http_get(7811, "/total?tea=1");
    let milliseconds = started.elapsed().as_millis();
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        answer_of_lengua_run.ends_with("{\"net\":\"2.50\",\"tax\":\"0.20\",\"total\":\"2.70\"}"),
        "{answer_of_lengua_run}"
    );
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-timings.txt");
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log) {
        let size = reply["build"]["size"].as_u64().unwrap_or(0);
        let _ = writeln!(
            file,
            "shop: one file of {size} bytes; lengua-run's first answer {milliseconds} ms after its start"
        );
    }
    fs::remove_file(shop.join("components/tax.wasm")).expect("the component removed");
    let refused = server.check(10_000, &shop);
    assert_eq!(codes(&refused), ["component-missing"], "{refused:#}");
}

fn http_bytes(port: u16, path: &str) -> Vec<u8> {
    let started = Instant::now();
    loop {
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
            stream.write_all(request.as_bytes()).expect("a request");
            let mut answer = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stream, &mut answer);
            return answer;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "no server");
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_notes_are_one_file_with_gpui_screens_built_for_the_browser_and_tested() {
    let root = bundle("notes", &["libraries/json", "examples/notes"]);
    let notes = root.join("examples/notes");
    let mut server = Server::start();
    done(&mut server, &root.join("libraries/json"), 10);
    let reply = until_built_within(&mut server, &notes, 20_000, 60_000);
    assert_eq!(reply["build"]["state"], "done", "{reply:#}");
    assert_eq!(reply["ok"], true, "{reply:#}");
    assert_ne!(reply["label"], "unchecked", "{reply:#}");
    assert_eq!(reply["build"]["tests"]["failed"], 0, "{reply:#}");
    let front = &reply["build"]["front"];
    assert_eq!(front["target"], "wasm32-unknown-unknown", "{reply:#}");
    assert_eq!(
        front["screens"],
        json!(["list", "note", "edit"]),
        "{reply:#}"
    );
    let front_size = front["size"].as_u64().unwrap_or(0);
    let size = reply["build"]["size"].as_u64().unwrap_or(0);
    assert!(front_size > 1 << 20, "{reply:#}");
    assert!(
        size > front_size,
        "the one file holds the front end: {reply:#}"
    );
    for file in [
        "web/app/Cargo.toml",
        "web/app/lib.rs",
        "web/files/Cargo.toml",
        "web/files/lib.rs",
    ] {
        assert!(notes.join(file).is_file(), "{file}");
    }
    let lock = fs::read_to_string(notes.join("lengua.lock")).expect("the lock");
    for line in [
        " target/lengua/web/app_bg.wasm\n",
        " target/lengua/web/app.js\n",
        " web/app/lib.rs\n",
        " src/ui/list.rs\n",
        " ideas/ui/edit.md\n",
        " assets/fonts/IBMPlexSans-Regular.ttf\n",
        "\ncomponent ",
        "\nsealed ",
    ] {
        assert!(lock.contains(line), "{line} in {lock}");
    }
    assert_eq!(lock.matches("\nfront ").count(), 2, "{lock}");
    let index = fs::read_to_string(notes.join("index.md")).expect("the index");
    assert!(index.contains("\n# Screens\n"), "{index}");
    let mut child = Command::new(lengua_run())
        .arg(&notes)
        .env("LENGUA_HOME", home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("lengua-run starts");
    let started = Instant::now();
    let page = http_get(7812, "/");
    let page_ms = started.elapsed().as_millis();
    let list = http_get(7812, "/api/notes");
    let code = http_bytes(7812, "/app_bg.wasm");
    let code_ms = started.elapsed().as_millis();
    let _ = child.kill();
    let _ = child.wait();
    assert!(page.contains("<title>notes</title>"), "{page}");
    assert!(
        list.ends_with("\"text\":\"Call the shop at noon.\"}]"),
        "{list}"
    );
    let body = code
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(0, |at| code.len() - at - 4);
    assert_eq!(
        u64::try_from(body).unwrap_or(0),
        front_size,
        "the front end as served"
    );
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lengua-timings.txt");
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = writeln!(
            file,
            "notes: one file of {size} bytes, the front end {front_size} bytes; lengua-run's page {page_ms} ms and the front end {code_ms} ms after its start; mutants {}, score {}",
            reply["build"]["mutants"], reply["score"]
        );
    }
}
