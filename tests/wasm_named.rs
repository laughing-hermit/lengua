//! What a program reaches by itself beyond the network (W18, second part), from its idea to its
//! runs, with the three examples that show it: `copy` opens the files its arguments name,
//! anywhere the user says and never among lengua's own; `computer` starts the two programs its
//! idea names; `key` asks for the terminal's mode, which a run with no terminal doesn't have.
//! Each goes through `check` to done first, then runs from a shell and through the `run` tool.
//! docs/reviews/reach-2-2026-11-11.md has the threat cases these hold.

mod common;

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use common::{Program as Folder, Protocol, Session};
use serde_json::{Value, json};

/// A file of the example `name`, in examples.
fn example(name: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A program folder holding the example `name`, checked to done in `session`.
async fn checked(session: &Session, name: &str) -> Folder {
    let program = Folder::new();
    for file in ["idea.md", "main.wat"] {
        program.write(file, &example(name, file));
    }
    session.record(&program).await;
    let done = session.settle(&program).await;
    assert_eq!(done["state"], "done", "{done}");
    program
}

/// `lengua-mcp run` of `program` with `args`, from a shell whose current folder is `folder`,
/// with the session's data folder, and so its key.
fn from_shell(session: &Session, folder: &Path, program: &Folder, args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(program.path())
        .arg("--")
        .args(args)
        .current_dir(folder)
        .stdin(Stdio::null())
        .env_remove("PLUGIN_DATA")
        .env_remove("CLAUDE_PLUGIN_DATA")
        .output()
        .expect("lengua-mcp runs")
}

/// What `ran` said on standard error.
fn said(ran: &Output) -> String {
    String::from_utf8_lossy(&ran.stderr).into_owned()
}

/// `run`'s arguments for `program` given `args`.
fn with_args(program: &Folder, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// The text of `path`, which the test names.
fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Whether `report`'s `may_reach` has a line that contains `words`.
fn may(report: &Value, words: &str) -> bool {
    report["may_reach"].as_array().is_some_and(|lines| {
        lines
            .iter()
            .any(|line| line.as_str().is_some_and(|line| line.contains(words)))
    })
}

/// Files anywhere the user names: `copy` reads the file its first argument names and writes
/// the one its second names, in any folder of the user's, by a path from the shell's folder or
/// a whole one. The threat: the user, or an agent, names lengua's own. A run opens no file of
/// the data folder, to read or to write, from a shell or through the `run` tool, and says why.
#[tokio::test]
async fn a_run_opens_the_files_its_arguments_name_and_no_file_of_lenguas_own() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "copy").await;
    let report = session.check(&program).await;
    assert!(
        may(&report, "it may read the file its first argument names"),
        "{report}"
    );
    assert!(
        may(&report, "it may write the file its second argument names"),
        "{report}"
    );
    let notes = tempfile::tempdir().expect("a folder of the user's");
    let elsewhere = tempfile::tempdir().expect("another folder of the user's");
    std::fs::write(notes.path().join("in.txt"), "hello\n").expect("a file");
    // From a shell: a path from the shell's folder, and a whole path in another folder.
    let out = elsewhere.path().join("out.txt");
    let ran = from_shell(
        &session,
        notes.path(),
        &program,
        &[OsStr::new("in.txt"), out.as_os_str()],
    );
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    assert_eq!(std::fs::read(&out).expect("the copy"), b"hello\n");
    let words = said(&ran);
    assert!(
        words.contains("it may read the file its first argument names"),
        "{words}"
    );
    assert!(words.contains("its argument 1 names"), "{words}");
    // lengua's own: the key that signs every lock, as the input and as the output.
    let key = session.data_dir.path().join("key");
    let before = std::fs::read(&key).expect("check made the key");
    let stolen = elsewhere.path().join("stolen.txt");
    let ran = from_shell(
        &session,
        notes.path(),
        &program,
        &[key.as_os_str(), stolen.as_os_str()],
    );
    assert_eq!(ran.status.code(), Some(2), "{ran:?}");
    assert!(!stolen.exists(), "the key was copied");
    let words = said(&ran);
    assert!(
        words.contains("copy: cannot open the input file"),
        "{words}"
    );
    assert!(words.contains("lengua's own"), "{words}");
    let ran = from_shell(
        &session,
        notes.path(),
        &program,
        &[OsStr::new("in.txt"), key.as_os_str()],
    );
    assert_eq!(ran.status.code(), Some(2), "{ran:?}");
    assert!(
        said(&ran).contains("copy: cannot open the output file"),
        "{ran:?}"
    );
    assert_eq!(std::fs::read(&key).expect("the key"), before);
    // The system's own, where there is one to name.
    #[cfg(unix)]
    {
        let ran = from_shell(
            &session,
            notes.path(),
            &program,
            &[OsStr::new("/dev/null"), stolen.as_os_str()],
        );
        assert_eq!(ran.status.code(), Some(2), "{ran:?}");
        assert!(said(&ran).contains("the system's own"), "{ran:?}");
    }
    // Through the `run` tool: whole paths, and paths from `folder`.
    let again = elsewhere.path().join("again.txt");
    let ran = session
        .call_ok(
            "run",
            with_args(
                &program,
                &[text(&notes.path().join("in.txt")), text(&again)],
            ),
        )
        .await;
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert_eq!(std::fs::read(&again).expect("the copy"), b"hello\n");
    assert!(
        may(&ran, "it may read the file its first argument names"),
        "{ran}"
    );
    let mut near = with_args(&program, &["in.txt", "near.txt"]);
    near["folder"] = json!(text(notes.path()));
    let ran = session.call_ok("run", near).await;
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert_eq!(
        std::fs::read(notes.path().join("near.txt")).expect("the copy"),
        b"hello\n"
    );
    let ran = session
        .call_ok("run", with_args(&program, &[text(&key), text(&stolen)]))
        .await;
    assert_eq!(ran["exit_code"], 2, "{ran}");
    assert!(!stolen.exists(), "the key was copied");
    assert!(ran.to_string().contains("lengua's own"), "{ran}");
    assert_eq!(std::fs::read(&key).expect("the key"), before);
    session.service.cancel().await.expect("close the session");
}

/// Starting named programs: `computer` starts `hostname` and `whoami`, which every runner has,
/// and prints what each printed. Its idea's test started neither: `check` came to done with what
/// a step says `hostname` gives, and with no `whoami` at all. A run says each grant, and the
/// file each program is, before the program starts.
#[tokio::test]
async fn a_run_starts_the_programs_its_idea_names() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "computer").await;
    let here = tempfile::tempdir().expect("a folder");
    let ran = from_shell(&session, here.path(), &program, &[]);
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
    let lines: Vec<&str> = printed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 2, "{ran:?}");
    assert!(
        !printed.contains("not found") && !printed.contains("failed"),
        "{ran:?}"
    );
    let words = said(&ran);
    assert!(
        words.contains("it may start the program `hostname` with no arguments"),
        "{words}"
    );
    assert!(words.contains("`whoami` is "), "{words}");
    // Through the `run` tool, the same two lines.
    let ran = session.call_ok("run", program.arguments()).await;
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert_eq!(ran["stdout"], printed.as_str(), "{ran}");
    assert!(
        may(&ran, "it may start the program `whoami` with no arguments"),
        "{ran}"
    );
    session.service.cancel().await.expect("close the session");
}

/// The terminal: `key` asks for keys as they're pressed. A run with no terminal, a shell's with
/// its input from nowhere or the `run` tool's, hears that there is none and reads its input as
/// it comes; the grant is said all the same.
#[tokio::test]
async fn a_run_with_no_terminal_reads_its_input_as_it_comes() {
    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "key").await;
    let mut given = program.arguments();
    given["stdin"] = json!("x");
    let ran = session.call_ok("run", given).await;
    assert_eq!(ran["exit_code"], 0, "{ran}");
    assert_eq!(ran["stdout"], "Press a key.\nYou pressed a key.\n", "{ran}");
    assert!(may(&ran, "it may read the terminal's size"), "{ran}");
    let here = tempfile::tempdir().expect("a folder");
    let ran = from_shell(&session, here.path(), &program, &[]);
    assert_eq!(ran.status.code(), Some(1), "{ran:?}");
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout),
        "Press a key.\n",
        "{ran:?}"
    );
    let words = said(&ran);
    assert!(words.contains("key: no key came"), "{words}");
    assert!(words.contains("it may read the terminal's size"), "{words}");
    session.service.cancel().await.expect("close the session");
}

/// The terminal, for real, on the systems where a test can make one: `key` runs from a shell
/// whose standard input is a pseudo-terminal. It makes the terminal raw, gets one key with no
/// Enter after it, and the terminal is afterwards as it was before the run.
#[cfg(unix)]
#[tokio::test]
async fn a_run_at_a_terminal_reads_a_key_as_it_is_pressed_and_puts_the_terminal_back() {
    use std::io::Write as _;
    use std::os::fd::AsFd as _;
    use std::time::{Duration, Instant};

    let session = Session::start(Protocol::Discover).await;
    let program = checked(&session, "key").await;
    let (terminal, keyboard) = lengua_os::terminal::pseudo().expect("a pseudo-terminal");
    let mode = || lengua_os::terminal::mode_of(terminal.as_fd()).expect("the terminal's mode");
    let before = mode();
    let here = tempfile::tempdir().expect("a folder");
    let child = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(session.data_dir.path())
        .arg("run")
        .arg(program.path())
        .current_dir(here.path())
        .stdin(Stdio::from(terminal.try_clone().expect("a second descriptor")))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("PLUGIN_DATA")
        .env_remove("CLAUDE_PLUGIN_DATA")
        .spawn()
        .expect("lengua-mcp starts");
    // The program makes the terminal raw before it reads. Then one key is pressed, and no Enter.
    let started = Instant::now();
    while mode() == before {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the program never made the terminal raw"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut keyboard = std::fs::File::from(keyboard);
    keyboard.write_all(b"x").expect("a key");
    let ran = child.wait_with_output().expect("the run ends");
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout),
        "Press a key.\nYou pressed a key.\n",
        "{ran:?}"
    );
    assert_eq!(mode(), before);
    session.service.cancel().await.expect("close the session");
}
