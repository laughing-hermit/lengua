//! The whole workflow through the MCP server, in both protocol generations, on the `add`
//! program: from the request to done through check alone, then run, from a shell, and every way
//! lengua refuses or recovers. The programs are the WebAssembly fixtures' main.wat, which run the
//! same on every system.

mod common;

#[cfg(unix)]
use std::io::Write as _;
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{ADD_CODE, ADD_IDEA, Program, Protocol, REQUEST, Session};
use serde_json::{Value, json};

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// The arguments of `run` for `program` with `args` for the program.
fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// A file of the fixture `name`, in tests/fixtures.
fn fixture(name: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A program folder holding the WebAssembly fixture `name`'s idea.md and main.wat.
fn folder_of(name: &str) -> Program {
    let program = Program::new();
    for file in ["idea.md", "main.wat"] {
        program.write(file, &fixture(name, file));
    }
    program
}

/// `lengua-mcp run` from a shell, with the session's data folder and so its key.
#[cfg(unix)]
fn from_shell(session: &Session, folder: &Path, args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(folder)
        .arg("--")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lengua-mcp starts");
    let mut input = child.stdin.take().expect("its standard input");
    input.write_all(stdin).expect("write standard input");
    drop(input);
    child.wait_with_output().expect("lengua-mcp ends")
}

/// A checked program runs from a shell, script or service as well as through `run`: the done
/// reply's command, as the user pastes it with the program's arguments after it, and standard
/// input that streams through, however long. tests/wasm_check.rs holds the rest: the exit codes,
/// the arguments after `--` or without it, and 125 with the reason when lengua won't run it.
#[cfg(unix)]
#[tokio::test]
async fn a_checked_program_runs_from_a_shell() {
    let session = Session::start(Protocol::Handshake).await;
    let program = folder_of("add");
    session.record(&program).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    // The done reply's command, as the user pastes it, with the program's arguments after it.
    let shell = text(&done["shell"]);
    assert!(shell.ends_with(" --"), "{done}");
    let sum = Command::new("sh")
        .arg("-c")
        .arg(format!("{shell} 2 3"))
        .output()
        .expect("sh runs");
    assert_eq!(sum.stdout, b"5\n", "{sum:?}");
    assert_eq!(sum.status.code(), Some(0), "{sum:?}");

    // Standard input streams through, however long.
    let cat = folder_of("cat");
    session.record(&cat).await;
    let done = session.settle(&cat).await;
    assert_eq!(done["state"], "done", "{done}");
    let input = "hello abe\n".repeat(400);
    let copied = from_shell(&session, cat.path(), &[], input.as_bytes());
    assert_eq!(copied.stdout, input.as_bytes(), "{:?}", copied.stderr);
    assert_eq!(copied.status.code(), Some(0), "{copied:?}");
    session.service.cancel().await.expect("close the session");
}

/// A server runs from a shell until something stops it: on its idea's port, and a stop that
/// reaches lengua-mcp, as a service manager sends one, stops the program too, which runs in
/// lengua-mcp's own process.
#[cfg(unix)]
#[tokio::test]
async fn a_server_from_a_shell_serves_until_it_is_stopped() {
    use std::io::{BufRead as _, Read as _};
    use std::os::unix::process::ExitStatusExt as _;
    let session = Session::start(Protocol::Handshake).await;
    let port = std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    let program = Program::new();
    program.write(
        "idea.md",
        &fixture("echo", "idea.md").replace("7007", &port.to_string()),
    );
    session.record(&program).await;
    program.write("main.wat", &fixture("echo", "main.wat"));
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(program.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lengua-mcp starts");
    let mut errors = std::io::BufReader::new(child.stderr.take().expect("its standard error"));
    let mut said = String::new();
    errors.read_line(&mut said).expect("a line");
    assert_eq!(said, format!("lengua-mcp: serving on 127.0.0.1:{port}\n"));
    for line in ["hi\n", "again\n"] {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.write_all(line.as_bytes()).expect("send");
        let mut back = String::new();
        stream.read_to_string(&mut back).expect("receive");
        assert_eq!(back, line);
    }
    let pid =
        rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid")).expect("a pid");
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).expect("stop it");
    let status = child.wait().expect("lengua-mcp ends");
    assert_eq!(status.signal(), Some(15), "{status:?}");
    // The program has gone with it: nothing serves on the port.
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    session.service.cancel().await.expect("close the session");
}

/// The add workflow in both protocol generations: from no files to an idea to code to done, the
/// runs, edited code, tests fixed after code ran, a lock edited by hand, an edited idea and a
/// new request.
#[tokio::test]
async fn add_goes_from_request_to_done_in_both_protocols() {
    for protocol in Protocol::ALL {
        let what = format!("{protocol:?}");
        let session = Session::start(protocol).await;
        let program = Program::new();
        let report = session.check(&program).await;
        assert_eq!(report["state"], "write_idea", "{what}: {report}");

        // One call after each file: the idea, tests included, with the user's words, then the
        // code.
        program.write("idea.md", ADD_IDEA);
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{what}: {report}");
        program.write("main.wat", ADD_CODE);
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{what}: {done}");
        assert_eq!(
            done["tests"][0], "main.wat follows every rule and passed all 202 test runs",
            "{what}: {done}"
        );
        assert!(
            text(&done["memory"]).starts_with("the rules (M1 to M8) hold"),
            "{what}: {done}"
        );
        assert_eq!(
            done["example"],
            "example B1: given arguments [\"2\", \"3\"], the program printed \"5\\n\", exited \
             with code 0",
            "{what}: {done}"
        );
        assert!(done["proofs"].is_array(), "{what}: {done}");
        assert!(
            done["performance"][0]
                .as_str()
                .is_some_and(|line| line.starts_with("example B1: 1 run")),
            "{what}: {done}"
        );
        for gone in ["machine", "changes", "problems"] {
            assert!(done.get(gone).is_none(), "{what}: {gone}: {done}");
        }
        assert!(
            text(&done["next_step"]).starts_with("Done:"),
            "{what}: {done}"
        );

        let sum = session
            .call_ok("run", with_args(&program, &["2", "3"]))
            .await;
        assert_eq!(sum["stdout"], "5\n", "{what}: {sum}");
        assert_eq!(sum["exit_code"], 0, "{what}: {sum}");
        // A WebAssembly program's run counts no CPU's instructions, cycles or time.
        for gone in ["fingerprint", "instructions", "cycles", "cpu_time_us"] {
            assert!(sum.get(gone).is_none(), "{what}: {gone}: {sum}");
        }
        let usage = session.call_ok("run", with_args(&program, &[])).await;
        assert_eq!(usage["stderr"], "usage: add <a> <b>\n", "{what}: {usage}");
        assert_eq!(usage["exit_code"], 1, "{what}: {usage}");
        let long = session
            .call_error(
                "run",
                json!({ "path": program.arguments()["path"], "timeout_seconds": 600 }),
            )
            .await;
        assert!(long.contains("goes from 1 to 40"), "{what}: {long}");

        // Edited code doesn't run until it's checked again.
        program.write("main.wat", &format!("{ADD_CODE}\n"));
        let error = session
            .call_error("run", with_args(&program, &["2", "3"]))
            .await;
        assert!(error.contains("call check"), "{what}: {error}");
        program.write("main.wat", ADD_CODE);
        assert_eq!(session.settle(&program).await["state"], "done", "{what}");

        // Tests can be fixed after code ran: the change is shown, and lengua marks what tests
        // less.
        let fewer = ADD_IDEA.replace("| 200 |", "| 20 |");
        program.write("idea.md", &fewer);
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{what}: {done}");
        assert!(
            text(&done["changes"]["idea_changed"]).contains("| 20 |"),
            "{what}: {done}"
        );
        assert!(
            done["changes"]["tests_less"][0]
                .as_str()
                .is_some_and(|mark| mark.contains("fewer runs, 20 instead of 200")),
            "{what}: {done}"
        );
        assert!(
            text(&done["next_step"]).contains("changes lists"),
            "{what}: {done}"
        );
        program.write("idea.md", ADD_IDEA);
        let done = session.settle(&program).await;
        assert!(done.get("changes").is_none(), "{what}: {done}");

        // A lock edited by hand can't be trusted; the user's words record the idea again.
        let lock = program.read("lengua.lock").expect("the lock");
        program.write(
            "lengua.lock",
            &lock.replace("unsafe_blocks = 0", "unsafe_blocks = 1"),
        );
        let error = session
            .call_error("run", with_args(&program, &["2", "3"]))
            .await;
        assert!(error.contains("can't be used"), "{what}: {error}");
        let report = session.check(&program).await;
        assert_eq!(report["state"], "needs_request", "{what}: {report}");
        session.record(&program).await;
        assert_eq!(session.settle(&program).await["state"], "done", "{what}");

        // An edited idea is recorded under the same words by the next check, and shown.
        let edited = ADD_IDEA.replace("Adds two numbers", "Adds two integers");
        program.write("idea.md", &edited);
        let error = session
            .call_error("run", with_args(&program, &["2", "3"]))
            .await;
        assert!(error.contains("call check"), "{what}: {error}");
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{what}: {done}");
        let changed = text(&done["changes"]["idea_changed"]);
        assert!(
            changed.contains("Adds two numbers") && changed.contains("Adds two integers"),
            "{what}: {done}"
        );
        assert_eq!(
            done["changes"]["requests"],
            json!([REQUEST]),
            "{what}: {done}"
        );

        // A new request after a finished build starts a new one with the user's words.
        let done = session
            .call_ok("check", program.request("Call them integers"))
            .await;
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains("words = [\"Call them integers\"]"),
            "{what}: {lock}"
        );
        assert!(done["state"].is_string(), "{what}: {done}");
        session.service.cancel().await.expect("close the session");
    }
}

/// Every call returns within the window, however long the job takes, and the job goes on: later
/// calls wait on the same job, and nothing needs to change.
#[tokio::test]
async fn every_call_returns_within_the_window_and_the_job_goes_on() {
    let session = Session::start_waiting(Protocol::Discover, 1).await;
    let program = folder_of("add");
    let started = Instant::now();
    let report = session.record(&program).await;
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}: {report}",
        started.elapsed()
    );
    // The same files: later calls wait on the same job, and nothing needs to change.
    let mut replies = vec![report];
    while !matches!(
        replies.last().and_then(|reply| reply["state"].as_str()),
        Some("done")
    ) {
        let began = Instant::now();
        replies.push(session.check(&program).await);
        assert!(began.elapsed() < Duration::from_secs(5), "{replies:?}");
        assert!(
            started.elapsed() < Duration::from_secs(300),
            "the job never ended: {replies:?}"
        );
    }
    assert!(
        replies.iter().any(|reply| reply["state"] == "testing"),
        "{replies:?}"
    );
    session.service.cancel().await.expect("close the session");
}

/// A shared program is checked again where it lands: one user's lengua builds add; another user,
/// with their own key, gets its two files, and lengua there checks them from scratch.
#[tokio::test]
async fn a_shared_program_is_checked_again_where_it_lands() {
    let first = Session::start(Protocol::Discover).await;
    let program = folder_of("add");
    first.record(&program).await;
    assert_eq!(first.settle(&program).await["state"], "done");
    let shared = Program::new();
    for file in ["idea.md", "main.wat"] {
        shared.write(file, &program.read(file).expect("a file"));
    }
    assert!(
        shared.read("lengua.lock").is_none(),
        "the lock stays behind"
    );
    let second = Session::start(Protocol::Discover).await;
    let refused = second
        .call_error("run", with_args(&shared, &["2", "3"]))
        .await;
    assert!(refused.contains("call check"), "{refused}");
    // Checked from scratch there, with that user's words, it runs.
    second.record(&shared).await;
    assert_eq!(second.settle(&shared).await["state"], "done");
    let sum = second.call_ok("run", with_args(&shared, &["2", "3"])).await;
    assert_eq!(sum["stdout"], "5\n", "{sum}");
    first.service.cancel().await.expect("close the session");
    second.service.cancel().await.expect("close the session");
}
