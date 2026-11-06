//! The whole workflow through the MCP server on every target, in both protocol generations, on
//! the `add` program: from the request to done through check alone, then run, from a shell, and
//! every way lengua refuses or recovers. Each target runs its own twin of `add` (one
//! byte-identical spec.md, a main.s in its CPU's instructions) and the same assertions, on its
//! own CPU's runner; what differs between the targets is a cell of a `PerCpu` table in
//! `tests/flow/mod.rs`.

mod common;
mod flow;
mod per_target;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{Program, Protocol, REQUEST, Session};
use flow::{any_line, text, with_args};
use lengua::hash::Sha256Hex;
use lengua::platform::Profile;
use per_target::Way;
use serde_json::json;

/// lengua's line in main.s once a check has passed: what it was compiled from and for.
fn compiled_from(target: Profile, spec: &str) -> String {
    format!(
        "{} lengua: compiled from spec.md sha256:{} for {}",
        flow::kit(target).comment,
        Sha256Hex::of(spec.as_bytes()),
        target.name()
    )
}

/// The line of main.s in which lengua says what it was compiled from and for.
fn header_of(program: &Program) -> String {
    let code = program.read("main.s").expect("main.s");
    code.lines().nth(1).unwrap_or_default().to_owned()
}

/// `lengua-mcp run` from a shell, with the session's data folder and so its key, aimed at the
/// target the session has.
fn from_shell(
    session: &Session,
    target: Profile,
    way: Way,
    folder: &Path,
    args: &[&str],
    stdin: &[u8],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
    command
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(folder)
        .arg("--")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    flow::aim(&mut command, target, way);
    let mut child = command.spawn().expect("lengua-mcp starts");
    let mut input = child.stdin.take().expect("its standard input");
    input.write_all(stdin).expect("write standard input");
    drop(input);
    child.wait_with_output().expect("lengua-mcp ends")
}

/// A checked program runs from a shell, script or service as well as through `run`: the same
/// checks and confinement, the shell's own standard input, output and error, no time limit, and
/// the program's exit code; 125 and the reason when lengua won't run it.
#[tokio::test]
async fn a_checked_program_runs_from_a_shell() {
    for target in Profile::ALL {
        let Some((session, way)) = per_target::session_with_way(target, Protocol::Handshake).await
        else {
            continue;
        };
        let name = target.name();
        let program = Program::new();
        program.write("spec.md", &flow::program_file(target, "add", "spec.md"));
        session.record(&program).await;
        program.write("main.s", &flow::program_file(target, "add", "main.s"));
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{name}: {done}");
        // The done reply's command, as the user pastes it, with the program's arguments after it.
        let shell = text(&done["shell"]);
        assert!(shell.ends_with(" --"), "{name}: {done}");
        let mut sh = Command::new("sh");
        sh.arg("-c").arg(format!("{shell} 2 3"));
        flow::aim(&mut sh, target, way);
        let sum = sh.output().expect("sh runs");
        assert_eq!(sum.stdout, b"5\n", "{name}: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{name}: {sum:?}");
        let usage = from_shell(&session, target, way, program.path(), &[], b"");
        assert_eq!(usage.stderr, b"usage: add <a> <b>\n", "{name}: {usage:?}");
        assert_eq!(usage.status.code(), Some(1), "{name}: {usage:?}");

        // Standard input streams through, however long.
        let cat = Program::new();
        cat.write("spec.md", &flow::program_file(target, "cat", "spec.md"));
        session.record(&cat).await;
        cat.write("main.s", &flow::program_file(target, "cat", "main.s"));
        let done = session.settle(&cat).await;
        assert_eq!(done["state"], "done", "{name}: {done}");
        let input = "hello abe\n".repeat(400);
        let copied = from_shell(&session, target, way, cat.path(), &[], input.as_bytes());
        assert_eq!(
            copied.stdout,
            input.as_bytes(),
            "{name}: {:?}",
            copied.stderr
        );
        assert_eq!(copied.status.code(), Some(0), "{name}: {copied:?}");

        // Code that isn't what check tested doesn't run.
        let code = program.read("main.s").expect("main.s");
        let comment = flow::kit(target).comment;
        program.write("main.s", &format!("{code}{comment} changed\n"));
        let refused = from_shell(&session, target, way, program.path(), &["2", "3"], b"");
        assert_eq!(refused.status.code(), Some(125), "{name}: {refused:?}");
        let said = String::from_utf8_lossy(&refused.stderr);
        assert!(
            said.contains("lengua runs a program only when its spec and code match lengua.lock"),
            "{name}: {said}"
        );
        assert!(refused.stdout.is_empty(), "{name}: {refused:?}");
        session.service.cancel().await.expect("close the session");
    }
}

/// A server runs from a shell until something stops it: on its spec's port, and a stop that
/// reaches lengua-mcp, as a service manager sends one, stops the program too.
#[cfg(unix)]
#[tokio::test]
async fn a_server_from_a_shell_serves_until_it_is_stopped() {
    use std::io::{BufRead as _, Read as _};
    for target in Profile::ALL {
        let Some((session, way)) = per_target::session_with_way(target, Protocol::Handshake).await
        else {
            continue;
        };
        let name = target.name();
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let program = Program::new();
        program.write(
            "spec.md",
            &flow::program_file(target, "echo", "spec.md").replace("7007", &port.to_string()),
        );
        session.record(&program).await;
        program.write("main.s", &flow::program_file(target, "echo", "main.s"));
        let done = session.settle(&program).await;
        assert_eq!(done["state"], "done", "{name}: {done}");

        let mut command = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"));
        command
            .arg("--data-dir")
            .arg(session.data_dir.path())
            .arg("run")
            .arg(program.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        flow::aim(&mut command, target, way);
        let mut child = command.spawn().expect("lengua-mcp starts");
        let mut errors = std::io::BufReader::new(child.stderr.take().expect("its standard error"));
        let mut said = String::new();
        errors.read_line(&mut said).expect("a line");
        assert_eq!(
            said,
            format!("lengua-mcp: serving on 127.0.0.1:{port}\n"),
            "{name}"
        );
        for line in ["hi\n", "again\n"] {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
            stream.write_all(line.as_bytes()).expect("send");
            let mut back = String::new();
            stream.read_to_string(&mut back).expect("receive");
            assert_eq!(back, line, "{name}");
        }
        let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid"))
            .expect("a pid");
        rustix::process::kill_process(pid, rustix::process::Signal::TERM).expect("stop it");
        let status = child.wait().expect("lengua-mcp ends");
        assert_eq!(status.code(), Some(128 + 15), "{name}: {status:?}");
        // The program has gone with it: nothing serves on the port.
        assert!(
            std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
            "{name}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// lengua writes its line where main.s is checked; the example in the repository has none, and
/// starts with the line that says which CPU's code it is.
#[test]
fn the_example_is_plain_assembly() {
    for target in Profile::ALL {
        let name = target.name();
        let kit = flow::kit(target);
        let code = flow::program_file(target, "add", "main.s");
        assert_eq!(code.lines().next(), Some(kit.first_line), "{name}");
        assert!(
            !code.contains(&format!("{} lengua:", kit.comment)),
            "{name}: {code}"
        );
    }
}

/// The add workflow in both protocol generations: from no files to a spec to code to done, the
/// runs, edited code, tests fixed after code ran, a lock edited by hand, an edited spec and a
/// new request.
#[tokio::test]
async fn add_goes_from_request_to_done_in_both_protocols() {
    for target in Profile::ALL {
        let Some(way) = per_target::way(target) else {
            continue;
        };
        let name = target.name();
        let spec = flow::program_file(target, "add", "spec.md");
        let code = flow::program_file(target, "add", "main.s");
        for protocol in Protocol::ALL {
            let what = format!("{name} with {protocol:?}");
            let session = flow::plain_in(target, way, protocol).await;
            let program = Program::new();
            let report = session.check(&program).await;
            assert_eq!(report["state"], "write_spec", "{what}: {report}");

            // One call after each file: the spec, tests included, with the user's words, then
            // the code.
            program.write("spec.md", &spec);
            let report = session.record(&program).await;
            assert_eq!(report["state"], "write_code", "{what}: {report}");
            program.write("main.s", &code);
            let done = session.settle(&program).await;
            assert_eq!(done["state"], "done", "{what}: {done}");
            // Code that passed carries lengua's line, which leaves what was checked as it was.
            assert_eq!(header_of(&program), compiled_from(target, &spec), "{what}");
            assert_eq!(
                program.read("main.s").expect("main.s").lines().count(),
                code.lines().count() + 1,
                "{what}"
            );
            flow::assert_passed(&done, 274, way, &what);
            assert!(
                done["memory"]
                    .as_str()
                    .is_some_and(|memory| memory.starts_with("the rules (M1 to M8) hold")),
                "{what}: {done}"
            );
            assert_eq!(
                done["example"],
                "example B1: given arguments [\"2\", \"3\"], the program printed \"5\\n\", \
                 exited with code 0",
                "{what}: {done}"
            );
            assert!(done["proofs"].is_array(), "{what}: {done}");
            // Through QEMU the runs are not timed.
            if way == Way::Native {
                assert!(
                    done["performance"][0]
                        .as_str()
                        .is_some_and(|line| line.starts_with("example B1: 1 run")),
                    "{what}: {done}"
                );
            }
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
            assert!(sum.get("fingerprint").is_none(), "{what}: {sum}");
            let usage = session.call_ok("run", with_args(&program, &[])).await;
            assert_eq!(usage["stderr"], "usage: add <a> <b>\n", "{what}: {usage}");
            assert_eq!(usage["exit_code"], 1, "{what}: {usage}");
            let invalid = session
                .call_ok("run", with_args(&program, &["2", "x"]))
                .await;
            assert_eq!(
                invalid["stderr"], "error: not a number\n",
                "{what}: {invalid}"
            );
            assert_eq!(invalid["exit_code"], 2, "{what}: {invalid}");
            let long = session
                .call_error(
                    "run",
                    json!({ "path": program.arguments()["path"], "timeout_seconds": 600 }),
                )
                .await;
            assert!(long.contains("goes from 1 to 40"), "{what}: {long}");

            // Edited code doesn't run until it's checked again.
            program.write("main.s", &format!("{code}\n"));
            let error = session
                .call_error("run", with_args(&program, &["2", "3"]))
                .await;
            assert!(error.contains("call check"), "{what}: {error}");
            program.write("main.s", &code);
            assert_eq!(session.settle(&program).await["state"], "done", "{what}");

            // Without its line, the checked code is still the checked code, and gets it back.
            assert_eq!(header_of(&program), compiled_from(target, &spec), "{what}");

            // Tests can be fixed after code ran: the change is shown, lengua marks what tests
            // less, and main.s's line names the spec it now passed.
            let fewer = spec.replace("| 200 |", "| 20 |");
            program.write("spec.md", &fewer);
            let done = session.settle(&program).await;
            assert_eq!(done["state"], "done", "{what}: {done}");
            assert!(
                text(&done["changes"]["spec_changed"]).contains("| 20 |"),
                "{what}: {done}"
            );
            assert_eq!(header_of(&program), compiled_from(target, &fewer), "{what}");
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
            program.write("spec.md", &spec);
            let done = session.settle(&program).await;
            assert!(done.get("changes").is_none(), "{what}: {done}");

            // A lock edited by hand can't be trusted; the user's words record the spec again.
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

            // An edited spec is recorded under the same words by the next check, and shown.
            let edited = spec.replace("Adds two numbers", "Adds two integers");
            program.write("spec.md", &edited);
            let error = session
                .call_error("run", with_args(&program, &["2", "3"]))
                .await;
            assert!(error.contains("call check"), "{what}: {error}");
            let done = session.settle(&program).await;
            assert_eq!(done["state"], "done", "{what}: {done}");
            let changed = text(&done["changes"]["spec_changed"]);
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
}

/// Code that breaks a rule runs no tests: the report names the rule, the compile skill is the
/// next step, and nothing is locked.
#[tokio::test]
async fn code_that_breaks_a_rule_runs_no_tests() {
    for target in Profile::ALL {
        let Some(way) = per_target::way(target) else {
            continue;
        };
        let name = target.name();
        let offence = flow::MOVES_THE_STACK_POINTER_IN_EDGE_FORM.get(target);
        for protocol in Protocol::ALL {
            let what = format!("{name} with {protocol:?}");
            let session = flow::plain_in(target, way, protocol).await;
            let program = Program::new();
            program.write("spec.md", &flow::program_file(target, "add", "spec.md"));
            program.write(
                "main.s",
                &offence.applied_to(&flow::program_file(target, "add", "main.s")),
            );
            let report = session.record(&program).await;
            assert_eq!(report["state"], "code_problems", "{what}: {report}");
            assert!(
                any_line(&report["problems"], "rule 3 (fixed resources)"),
                "{what}: {report}"
            );
            assert!(
                text(&report["next_step"]).contains("compile skill"),
                "{what}: {report}"
            );
            let lock = program.read("lengua.lock").expect("the lock");
            assert!(!lock.contains("[code]"), "{what}: {lock}");
            session.service.cancel().await.expect("close the session");
        }
    }
}

/// Every call returns within the window, however long the job takes, and the job goes on: later
/// calls wait on the same job, and nothing needs to change.
#[tokio::test]
async fn every_call_returns_within_the_window_and_the_job_goes_on() {
    for target in Profile::ALL {
        if !flow::natively(target) {
            continue;
        }
        let name = target.name();
        let session = Session::start_waiting(Protocol::Discover, 1).await;
        let program = flow::folder_of(flow::program_files(target, "add"));
        let began = Instant::now();
        let report = session.record(&program).await;
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "{name}: {:?}: {report}",
            began.elapsed()
        );
        // The same files: later calls wait on the same job, and nothing needs to change.
        let mut replies = vec![report];
        while !matches!(
            replies.last().and_then(|reply| reply["state"].as_str()),
            Some("done")
        ) {
            let began = Instant::now();
            replies.push(session.check(&program).await);
            assert!(began.elapsed() < Duration::from_secs(5), "{name}");
            assert!(replies.len() < 400, "{name}: {replies:?}");
        }
        assert!(
            replies.iter().any(|reply| reply["state"] == "testing"),
            "{name}: {replies:?}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// A shared program is checked again where it lands: one user's lengua builds add; another user,
/// with their own key, gets its two files, and lengua there checks them from scratch.
#[tokio::test]
async fn a_shared_program_is_checked_again_where_it_lands() {
    for target in Profile::ALL {
        let Some(way) = per_target::way(target) else {
            continue;
        };
        let name = target.name();
        let first = flow::plain_in(target, way, Protocol::Discover).await;
        let program = flow::folder_of(flow::program_files(target, "add"));
        first.record(&program).await;
        assert_eq!(first.settle(&program).await["state"], "done", "{name}");
        let shared = Program::new();
        for file in ["spec.md", "main.s"] {
            shared.write(file, &program.read(file).expect("a file"));
        }
        assert!(
            shared.read("lengua.lock").is_none(),
            "{name}: the lock stays behind"
        );
        let second = flow::plain_in(target, way, Protocol::Discover).await;
        let refused = second
            .call_error("run", with_args(&shared, &["2", "3"]))
            .await;
        assert!(refused.contains("call check"), "{name}: {refused}");
        // Checked from scratch there, with that user's words, it runs.
        second.record(&shared).await;
        assert_eq!(second.settle(&shared).await["state"], "done", "{name}");
        let sum = second.call_ok("run", with_args(&shared, &["2", "3"])).await;
        assert_eq!(sum["stdout"], "5\n", "{name}: {sum}");
        first.service.cancel().await.expect("close the session");
        second.service.cancel().await.expect("close the session");
    }
}

/// The programs the workflow runs are shipped for every target, and each target's rule-breaking
/// instruction goes before text its add program has: a mistake in the cells of a target this
/// computer cannot run shows on every runner.
#[test]
fn the_workflow_has_its_programs_and_cells_for_every_target() {
    for target in Profile::ALL {
        let name = target.name();
        let offence = flow::MOVES_THE_STACK_POINTER_IN_EDGE_FORM.get(target);
        let code = flow::program_file(target, "add", "main.s");
        assert_ne!(offence.applied_to(&code), code, "{name}");
        let spec = flow::program_file(target, "add", "spec.md");
        for needle in ["| 200 |", "Adds two numbers"] {
            assert!(
                spec.contains(needle),
                "{name}: add's spec has no {needle:?}"
            );
        }
        let echo = flow::program_file(target, "echo", "spec.md");
        assert!(echo.contains("7007"), "{name}: echo's spec names no port");
        assert!(
            !flow::program_file(target, "cat", "main.s").is_empty(),
            "{name}"
        );
        assert!(
            compiled_from(target, &spec).ends_with(name),
            "{name}: lengua's line names the target last"
        );
    }
}
