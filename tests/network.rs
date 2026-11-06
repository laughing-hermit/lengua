//! The network, the terminal and standard input and output through the server, on every target: a
//! server whose clients the tests bring and a client whose server the tests stand in for, a
//! client over TLS and one that connects to the computer an argument names, a server that waits
//! for its client to close, the event-loop server and the key reader, standard input that
//! arrives in pieces, a cut only the proof tries, and standard output that is closed.
//!
//! Each case is written once, with a cell for each target (`PerCpu`): the program's folder, or
//! the edit of that target's code that the case makes. The specs of the two targets' programs are
//! the same bytes, so an edit of a spec is one for both. A target added to lengua breaks this
//! file until its cells are written; every target's edits are checked against its programs on
//! any computer, and its cases run where this computer runs its programs, natively or through
//! QEMU's user mode, and note in the run's summary where it cannot.

mod common;
mod per_target;

use std::path::PathBuf;

use common::{Program, Protocol, Session};
use lengua::platform::Profile;
use lengua::targets::PerCpu;
use serde_json::{Value, json};

/// How long each program's proofs may take, in seconds, in the cases that wait for them.
const PROOF_SECONDS: u64 = 120;

/// How long the proofs may take in the case whose proof one target cannot finish: a minute, as
/// the runners give by default, which is all x86-64 needs.
const SHORT_PROOF_SECONDS: u64 = 60;

/// A program's folder on each target, relative to the repository: its spec.md, the same bytes on
/// every target, and its main.s.
type Folders = PerCpu<&'static str>;

const ECHO: Folders = PerCpu {
    x86_64: "examples/echo",
    aarch64: "tests/fixtures/linux-aarch64/echo",
};

const FETCH: Folders = PerCpu {
    x86_64: "examples/fetch",
    aarch64: "tests/fixtures/linux-aarch64/fetch",
};

const SECOND: Folders = PerCpu {
    x86_64: "tests/fixtures/second",
    aarch64: "tests/fixtures/linux-aarch64/second",
};

const ADD: Folders = PerCpu {
    x86_64: "examples/add",
    aarch64: "tests/fixtures/linux-aarch64/add",
};

/// A change to a program's code: the first `from` becomes `to`.
type Edit = (&'static str, &'static str);

/// Where the programs call fill with a newline as its end byte, the call that reads to the end of
/// what arrives instead: the end byte is -1.
const READS_TO_THE_END: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov ecx, NEWLINE\n    call fill\n",
        "    mov ecx, -1\n    call fill\n",
    ),
    aarch64: (
        "    mov w3, #NEWLINE\n    bl fill\n",
        "    mov w3, #-1\n    bl fill\n",
    ),
};

/// Where the programs call fill, one read of their own instead: a call of the operation, which
/// takes the same first three arguments.
const READS_ONCE: PerCpu<Edit> = PerCpu {
    x86_64: (
        "    mov ecx, NEWLINE\n    call fill\n",
        "    call os_read\n",
    ),
    aarch64: ("    mov w3, #NEWLINE\n    bl fill\n", "    bl os_read\n"),
};

/// The echo server made to exit after its first connection, where its spec says it keeps
/// serving: an exit and the constants it names.
const STOPS_AFTER_ONE: PerCpu<[Edit; 2]> = PerCpu {
    x86_64: [
        (".equ LISTENING, 3\n", ".equ LISTENING, 3\n.equ EXIT_DONE, 0\n"),
        (
            ".Lmain_next:\n    jmp .Lmain_serve",
            ".Lmain_next:\n    mov edi, EXIT_DONE\n    call os_exit",
        ),
    ],
    aarch64: [
        (".equ LISTENING, 3\n", ".equ LISTENING, 3\n.equ EXIT_DONE, 0\n"),
        (
            ".Lmain_next:\n    b .Lmain_serve",
            ".Lmain_next:\n    mov w0, #EXIT_DONE\n    bl os_exit",
        ),
    ],
};

/// The exit code add promises when writing its output fails, made another, in both targets' code.
const WRONG_EXIT: Edit = (".equ EXIT_WRITE_FAILED, 3", ".equ EXIT_WRITE_FAILED, 4");

/// The event-loop server and the key reader of each target: the spec of each in the compile
/// skill's assets, and its template.
const EVENT_LOOPS: PerCpu<[(&str, &str); 2]> = PerCpu {
    x86_64: [
        ("server-template-spec.md", "server-template.s"),
        ("terminal-template-spec.md", "terminal-template.s"),
    ],
    aarch64: [
        ("server-template-spec.md", "linux-aarch64/server-template.s"),
        (
            "terminal-template-spec.md",
            "linux-aarch64/terminal-template.s",
        ),
    ],
};

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// Whether the tests' reply says every rule held and every test passed.
fn passed(report: &Value) -> bool {
    any_line(&report["tests"], "follows every rule and passed all")
}

/// A file of the repository, by its path from the repository's root.
fn read(path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A file of the compile skill's assets.
fn asset(name: &str) -> String {
    read(&format!("plugin/skills/compile/assets/{name}"))
}

/// A program's spec.md and main.s on `target`.
fn files(folders: &Folders, target: Profile) -> (String, String) {
    let folder = folders.get(target);
    (
        read(&format!("{folder}/spec.md")),
        read(&format!("{folder}/main.s")),
    )
}

/// `code` with the first `from` of the edit made `to`, which must change it.
fn edited(code: &str, (from, to): Edit) -> String {
    assert!(code.contains(from), "{from:?} isn't in the program");
    code.replacen(from, to, 1)
}

/// `spec` with every `from` made `to`, which must change it.
fn respelled(spec: &str, from: &str, to: &str) -> String {
    assert!(spec.contains(from), "{from:?} isn't in the spec");
    spec.replace(from, to)
}

/// The lines of the report's proofs.
fn proof_lines(report: &Value) -> Vec<&str> {
    report["proofs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(text)
        .collect()
}

/// What a proof line says of a block proven for every input in its range.
const PROVEN: &str = "proven for every input in range";

/// Whether the report has proofs and each says its block is proven for every input in its range.
fn all_proven(report: &Value) -> bool {
    let proofs = proof_lines(report);
    !proofs.is_empty() && proofs.iter().all(|line| line.contains(PROVEN))
}

/// A program folder holding `spec` and `code`, recorded as the user's request.
async fn written(session: &Session, spec: &str, code: &str) -> Program {
    let program = Program::new();
    program.write("spec.md", spec);
    program.write("main.s", code);
    session.record(&program).await;
    program
}

/// Checks until the job ends, which must be done.
async fn done(session: &Session, program: &Program, what: &str) -> Value {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "done", "{what}: {report}");
    report
}

/// Checks until the job ends, which must be with failed tests; returns the problems.
async fn failed(session: &Session, program: &Program, what: &str) -> String {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "tests_failed", "{what}: {report}");
    report["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .map(|problem| text(problem).to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// Whether the solvers the proofs need are installed here, noted where they are not.
fn solvers_installed() -> bool {
    let installed = std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
    }
    installed
}

/// A session for `target` with `seconds` for proofs, once no other session in this test binary is
/// proving, where this computer runs the target's programs: natively, or through QEMU's user
/// mode.
async fn proving(target: Profile, seconds: u64) -> Option<Session> {
    Some(match per_target::way(target)? {
        per_target::Way::Native => Session::proving_alone_for(Protocol::Discover, seconds).await,
        per_target::Way::Emulated => {
            Session::emulating_for(Protocol::Discover, target.name(), seconds).await
        }
    })
}

/// A server: lengua listens for it, hands it the socket as file descriptor 3, has the test's
/// client talk to it, and stops it when it keeps serving.
#[tokio::test]
async fn the_echo_server_answers_the_tests_client() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        // Only where nothing else holds the port here.
        if std::net::TcpListener::bind("127.0.0.1:7007").is_err() {
            lengua::skiplog::note("something on this computer holds port 7007");
            session.service.cancel().await.expect("close the session");
            continue;
        }
        let (spec, code) = files(&ECHO, target);
        let program = written(&session, &spec, &code).await;
        let report = done(&session, &program, name).await;
        assert!(
            text(&report["example"]).contains("a client that sent"),
            "{name}: {report}"
        );
        assert!(passed(&report), "{name}: {report}");

        // run serves in the background on the spec's port until it's stopped.
        let serving = session.call_ok("run", program.arguments()).await;
        assert_eq!(serving["serving"], "127.0.0.1:7007", "{name}: {serving}");
        let again = session.call_error("run", program.arguments()).await;
        assert!(again.contains("already serving"), "{name}: {again}");
        {
            use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
            let mut stream = tokio::net::TcpStream::connect("127.0.0.1:7007")
                .await
                .expect("connect");
            stream.write_all(b"ping\n").await.expect("send");
            let mut answer = Vec::new();
            stream.read_to_end(&mut answer).await.expect("read");
            assert_eq!(answer, b"ping\n", "{name}");
        }
        let mut stop = program.arguments();
        stop["stop"] = json!(true);
        let stopped = session.call_ok("run", stop.clone()).await;
        assert!(
            text(&stopped["ended"]).contains("stopped by lengua"),
            "{name}: {stopped}"
        );
        let nothing = session.call_error("run", stop).await;
        assert!(nothing.contains("no server"), "{name}: {nothing}");

        // A server that stops after one connection fails: the spec says it keeps serving.
        let [constants, exit] = *STOPS_AFTER_ONE.get(target);
        program.write("main.s", &edited(&edited(&code, constants), exit));
        let problems = failed(&session, &program, name).await;
        assert!(
            problems.contains("while the test expects it to keep serving")
                || problems.contains("wasn't waiting for the next one"),
            "{name}: {problems}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// A client: lengua makes its connection before it starts, to the test's own server in its tests
/// and to this computer's port 8080 in a run, and hands it over as descriptor 4.
#[tokio::test]
async fn the_fetch_client_talks_to_the_tests_server() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&FETCH, target);
        let program = written(&session, &spec, &code).await;
        let report = done(&session, &program, name).await;
        assert!(passed(&report), "{name}: {report}");
        // Only where nothing else holds port 8080 here.
        if std::net::TcpListener::bind("127.0.0.1:8080").is_err() {
            lengua::skiplog::note("something on this computer holds port 8080");
            session.service.cancel().await.expect("close the session");
            continue;
        }
        // With nothing there, the program gets its connection already ended, and the report says
        // why.
        let refused = session.call_ok("run", with_args(&program, &["/"])).await;
        assert_eq!(refused["exit_code"], 3, "{name}: {refused}");
        assert_eq!(
            refused["connections"][0]["place"], "port 8080 of this computer",
            "{name}: {refused}"
        );
        assert!(
            refused["connections"][0]["problem"].is_string(),
            "{name}: {refused}"
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("port 8080");
        let server = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 64];
            while !request.ends_with(b"\r\n\r\n") {
                let read = stream.read(&mut chunk).await.expect("read");
                assert!(read > 0, "the request ended early: {request:?}");
                request.extend_from_slice(&chunk[..read]);
            }
            stream
                .write_all(b"HTTP/1.0 200 OK\r\n\r\nfrom 8080")
                .await
                .expect("reply");
            request
        });
        let fetched = session.call_ok("run", with_args(&program, &["/hi"])).await;
        assert_eq!(fetched["stdout"], "from 8080", "{name}: {fetched}");
        assert!(
            fetched["connections"][0]["problem"].is_null(),
            "{name}: {fetched}"
        );
        assert_eq!(
            server.await.expect("served"),
            b"GET /hi HTTP/1.0\r\n\r\n",
            "{name}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// fetch's spec pointed at a named computer over TLS: lengua would do the TLS in a run, so its
/// tests' stand-in server speaks the plain bytes the program sends and reads.
fn over_tls(spec: &str) -> String {
    let spec = respelled(
        spec,
        "- Network: connects to TCP port 8080",
        "- Network: connects to TCP port 443 on example.com over TLS",
    );
    respelled(&spec, "on port `8080`", "on port `443`")
}

#[tokio::test]
async fn a_client_over_tls_is_tested_in_plain_bytes() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let (spec, code) = files(&FETCH, target);
        let program = written(&session, &over_tls(&spec), &code).await;
        done(&session, &program, target.name()).await;
        session.service.cancel().await.expect("close the session");
    }
}

/// fetch's spec with its server's computer named by its first argument, as a curl-like tool's
/// user types the host.
fn named_by_an_argument(spec: &str) -> String {
    respelled(
        spec,
        "- Network: connects to TCP port 8080",
        "- Network: connects to TCP port 8080 on the computer its first argument names",
    )
}

/// fetch with its server's computer named by its first argument: its tests' server stands in for
/// any value the argument takes, so it passes, and a real run reads the argument before the
/// program starts. `/hello` names no computer, so the program gets its connection already ended
/// and says it can't reach the server, and the run says why.
#[tokio::test]
async fn a_connection_to_the_computer_an_argument_names() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&FETCH, target);
        let program = written(&session, &named_by_an_argument(&spec), &code).await;
        done(&session, &program, name).await;
        let report = session
            .call_ok("run", with_args(&program, &["/hello"]))
            .await;
        assert_eq!(report["exit_code"], 3, "{name}: {report}");
        assert!(
            text(&report["connections"][0]["problem"]).contains("isn't a computer's name"),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// curl and browsers keep their side of the connection open until they have the answer: a server
/// that reads to the end of the request before answering waits on a client that waits for it, and
/// the test's client does as they do.
#[tokio::test]
async fn a_server_that_waits_for_its_client_to_close_fails_as_with_curl() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&ECHO, target);
        let waits = edited(&code, *READS_TO_THE_END.get(target));
        let program = written(&session, &spec, &waits).await;
        let problems = failed(&session, &program, name).await;
        assert!(
            problems.contains("curl and browsers keep it open until they have the whole answer"),
            "{name}: {problems}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// fetch's spec with short paths and bodies, so that the proof covers them all.
fn with_short_paths_and_bodies(spec: &str) -> String {
    let spec = respelled(
        spec,
        "path holds `1` to `100` characters",
        "path holds `1` to `100` characters (tests: `6` and `11`)",
    );
    let spec = respelled(&spec, "`string(1, 100, ", "`string(1, 6, ");
    let spec = respelled(&spec, "`string(0, 65000, ", "`string(0, 11, ");
    respelled(&spec, "`string(1, 65000, ", "`string(1, 11, ")
}

#[tokio::test]
async fn programs_on_the_network_are_proven() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some(session) = proving(target, PROOF_SECONDS).await else {
            continue;
        };
        let name = target.name();
        // A server: the echo server, as it is.
        let (spec, code) = files(&ECHO, target);
        let echo = written(&session, &spec, &code).await;
        let report = done(&session, &echo, name).await;
        assert!(all_proven(&report), "{name}: {report}");
        // It reads its line with fill, so the proof covers every way the line can be cut.
        let first = proof_lines(&report).first().copied().unwrap_or_default();
        assert!(
            first.ends_with("arriving cut into pieces any way"),
            "{name}: {report}"
        );
        // A client: fetch, with short paths and bodies so the proof covers them all.
        let (spec, code) = files(&FETCH, target);
        let fetch = written(&session, &with_short_paths_and_bodies(&spec), &code).await;
        let report = done(&session, &fetch, name).await;
        assert!(all_proven(&report), "{name}: {report}");
        session.service.cancel().await.expect("close the session");
    }
}

/// The event-loop server, for clients at once on the listening socket lengua hands it, and the
/// key reader, for keys from standard input as from a pipe, pass their tests through the server.
#[tokio::test]
async fn the_event_loop_server_and_the_key_reader_pass_their_tests() {
    for target in Profile::ALL {
        let Some(session) = proving(target, PROOF_SECONDS).await else {
            continue;
        };
        for (spec, code) in EVENT_LOOPS.get(target) {
            let what = format!("{code} on {}", target.name());
            let program = written(&session, &asset(spec), &asset(code)).await;
            let report = done(&session, &program, &what).await;
            assert!(passed(&report), "{what}: {report}");
        }
        session.service.cancel().await.expect("close the session");
    }
}

/// second with one read of its own in place of fill: right only when both lines arrive in one
/// read, as a pipe doesn't promise. Every other generated run sends standard input in pieces, the
/// first byte alone, so the tests find it (the proof's every count of a read is held in the
/// prover's own tests).
#[tokio::test]
async fn a_raw_read_fails_when_standard_input_arrives_in_pieces() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&SECOND, target);
        let program = written(&session, &spec, &edited(&code, *READS_ONCE.get(target))).await;
        let problems = failed(&session, &program, name).await;
        assert!(
            problems.contains(
                "standard input arriving in three pieces: the first byte, up to the middle, and \
                 the rest:"
            ),
            "{name}: {problems}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// What each target's proof says of add's block B4, where standard output is closed: proven for
/// every input in range within a minute. AArch64's reached only part of the block's 400 input
/// combinations in that minute and said "tested, not proven" (it needed "about 8 minutes", run
/// 41912035484 on the ARM64 runner) until the prover's model of AArch64 read the overflow flag,
/// `tbnz` on the top bit and `msub` as x86-64's reads them (the same gaps that `amounts` showed
/// in todo.md); now it settles the block by its terms alone (400 input combinations, 1014 paths,
/// run 41942392800). Both catch an exit code that is not the spec's, by the test that closes
/// standard output.
const B4_PROOF: PerCpu<&str> = PerCpu {
    x86_64: "generated B4: proven for every input in range",
    aarch64: "generated B4: proven for every input in range",
};

/// add's spec promises code 3 when writing its output fails, and its B4 shows it with
/// `- And standard output is closed`: a pipe nobody reads, as after `prog | head -1`. Every write
/// then fails with EPIPE, never a signal, so the program passes, proven where the prover reaches
/// it, and one that exits with another code fails the test that closes its output.
#[tokio::test]
async fn a_closed_standard_output_fails_every_write() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some(session) = proving(target, SHORT_PROOF_SECONDS).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&ADD, target);
        assert!(spec.contains("- And standard output is closed"), "add's B4");
        let program = written(&session, &spec, &code).await;
        let report = done(&session, &program, name).await;
        let proofs = proof_lines(&report);
        let b4 = *B4_PROOF.get(target);
        assert!(
            proofs.iter().any(|line| line.starts_with(b4)),
            "{name}: {report}"
        );
        program.write("main.s", &edited(&code, WRONG_EXIT));
        let problems = failed(&session, &program, name).await;
        assert!(problems.contains("B4"), "{name}: {problems}");
        session.service.cancel().await.expect("close the session");
    }
}

/// second's spec with a short first line and a long second one, so the tests' own pieces of
/// standard input, cut after its first byte and at its middle, never end a read right after the
/// first line: only the proof's cuts do.
fn with_lines_the_tests_cut_past(spec: &str) -> String {
    let changed = respelled(
        spec,
        "Each line is `1` to `20`\nlowercase letters followed by a newline.",
        "The first line is `1` to `5`\nlowercase letters and the second `12` to `20`, each \
         followed by a newline.",
    );
    let changed = respelled(&changed, "`ab\\ncd\\n`", "`ab\\nijklmnopqrst\\n`");
    let changed = respelled(
        &changed,
        "prints `cd` followed",
        "prints `ijklmnopqrst` followed",
    );
    let changed = changed
        .replacen("string(1, 20,", "string(1, 5,", 1)
        .replacen("string(1, 20,", "string(12, 20,", 1);
    assert!(changed.contains("string(12, 20,"), "{changed}");
    assert!(changed.contains("`ab\\nijklmnopqrst\\n`"), "{changed}");
    changed
}

/// second reads both lines with one fill, which stops at the first newline a read ends with:
/// right whenever no read ends right after the first line, as none of the tests' own pieces do
/// with these lengths. The proof takes every cut and finds the one after the first line, and a
/// real run with standard input cut there confirms it.
#[tokio::test]
async fn a_cut_only_the_proof_tries_is_found_and_run_for_real() {
    if !solvers_installed() {
        return;
    }
    for target in Profile::ALL {
        let Some(session) = proving(target, PROOF_SECONDS).await else {
            continue;
        };
        let name = target.name();
        let (spec, code) = files(&SECOND, target);
        let program = written(&session, &with_lines_the_tests_cut_past(&spec), &code).await;
        let problems = failed(&session, &program, name).await;
        assert!(
            problems.contains("the proof found inputs the tests didn't try where main.s is wrong"),
            "{name}: {problems}"
        );
        assert!(
            problems.contains("standard input arriving in two pieces, cut after byte"),
            "{name}: {problems}"
        );
        // Reading to the end instead: proven however standard input arrives.
        program.write("main.s", &edited(&code, *READS_TO_THE_END.get(target)));
        let report = done(&session, &program, name).await;
        assert!(
            text(&report["proofs"][0]).ends_with("standard input arriving cut into pieces any way"),
            "{name}: {report}"
        );
        session.service.cancel().await.expect("close the session");
    }
}

/// Every edit and every spec change above is made to text that its program has, for every
/// target, on any computer: a cell for a CPU this computer cannot run is still held to its
/// program, and a change that changes nothing fails.
#[test]
fn every_edit_is_made_to_text_its_program_has() {
    for target in Profile::ALL {
        let (_, echo) = files(&ECHO, target);
        let (fetch_spec, _) = files(&FETCH, target);
        let (second_spec, second) = files(&SECOND, target);
        let (_, add) = files(&ADD, target);
        assert_ne!(edited(&echo, *READS_TO_THE_END.get(target)), echo);
        assert_ne!(edited(&second, *READS_TO_THE_END.get(target)), second);
        assert_ne!(edited(&second, *READS_ONCE.get(target)), second);
        assert_ne!(edited(&add, WRONG_EXIT), add);
        let [constants, exit] = *STOPS_AFTER_ONE.get(target);
        assert_ne!(edited(&edited(&echo, constants), exit), echo);
        for changed in [
            over_tls(&fetch_spec),
            named_by_an_argument(&fetch_spec),
            with_short_paths_and_bodies(&fetch_spec),
        ] {
            assert_ne!(changed, fetch_spec);
        }
        assert_ne!(with_lines_the_tests_cut_past(&second_spec), second_spec);
        for (spec, code) in EVENT_LOOPS.get(target) {
            assert!(!asset(spec).is_empty() && !asset(code).is_empty());
        }
    }
}
