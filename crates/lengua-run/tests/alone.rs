//! lengua-run by itself, with no lengua-mcp beside it, the same on every system: it runs a
//! checked program from the module check compiled ahead of the run, once that carries the seal
//! of the user's key; it loads nothing else in that file's place; it makes every check a run
//! makes, and refuses as `lengua-mcp run` does; a program whose calls go too deep traps on the
//! thread that runs it; and what needs the compiler it leaves to lengua-mcp, with the same words.
//!
//! The program folders here are made with lengua's own parts and no server: idea.md and
//! main.wat, lengua.lock signed with a data folder's key as check signs it at done, and the
//! module compiled and sealed as check compiles it. tests/wasm_check.rs holds the same through
//! `check` itself, with lengua-mcp beside lengua-run.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lengua::data_dir::DataDir;
use lengua::hash::Sha256Hex;
use lengua::lock::{self, CodeStamp, IdeaStamp, Key, Lock, RequestStamp};
use lengua::wasm::engine::{self, Setup};

/// The add program's idea.
const IDEA: &str = include_str!("../../../tests/fixtures/add/idea.md");

/// Its code, in WebAssembly text.
const CODE: &str = include_str!("../../../tests/fixtures/add/main.wat");

/// A module of its own, valid, that prints 6 and a newline: another module than add's.
const OTHER: &str = r#"(module
  (memory (export "memory") 1)
  (data (i32.const 64) "6\n")
  (func (export "_start")
    (drop (call $write_all (i32.const 1) (i32.const 64) (i32.const 2)))))
"#;

/// A module whose `_start` calls a function that calls itself without end.
const DEEP: &str = r#"(module
  (memory (export "memory") 1)
  (func $deep (result i32)
    (i32.add (call $deep) (i32.const 1)))
  (func (export "_start")
    (drop (call $deep))))
"#;

/// An idea that names a secret, and its code.
const SECRET_IDEA: &str = include_str!("../../../examples/secret-equal/idea.md");

/// The code of [`SECRET_IDEA`].
const SECRET_CODE: &str = include_str!("../../../examples/secret-equal/main.wat");

/// A program folder as check leaves it at done, and lengua-run in a folder of its own.
struct Checked {
    /// Holds every folder below; deleted when dropped.
    root: tempfile::TempDir,
    /// The data folder, with the user's key.
    data: DataDir,
    /// The user's key.
    key: Key,
    /// The module main.wat builds.
    wasm: Vec<u8>,
    /// Its SHA-256.
    module: Sha256Hex,
}

impl Checked {
    /// The folder of the program `idea` and `code`, recorded and stamped in its lock as check
    /// stamps code whose tests passed, with no compiled module yet.
    fn new(idea: &str, code: &str) -> Self {
        let root = tempfile::tempdir().expect("a folder");
        let data = DataDir::new(root.path().join("data"));
        let key = Key::load_or_create(&data).expect("a key");
        let folder = root.path().join("program");
        std::fs::create_dir_all(&folder).expect("the program's folder");
        std::fs::create_dir_all(root.path().join("work")).expect("a working folder");
        std::fs::write(folder.join("idea.md"), idea).expect("idea.md");
        std::fs::write(folder.join("main.wat"), code).expect("main.wat");
        let wasm = lengua::wasm::text::build(code).expect("main.wat builds").wasm;
        let module = Sha256Hex::of(&wasm);
        let recorded = lengua::trust::idea_hash(idea.as_bytes());
        let lock = Lock {
            request: RequestStamp {
                words: vec!["a program".to_owned()],
                idea: recorded.clone(),
            },
            idea: IdeaStamp { sha256: recorded },
            code: Some(CodeStamp {
                sha256: Sha256Hex::of(code.as_bytes()),
                module_sha256: module.clone(),
                engine: engine::ENGINE.to_owned(),
                max_stack_bytes: 0,
                unsafe_blocks: 0,
                features: engine::features_used(&wasm),
            }),
            proofs: None,
        };
        lock::write(&folder, &lock, &key).expect("lengua.lock");
        // lengua-run in a folder with nothing else, so that a run it leaves to lengua-mcp fails.
        let alone = root.path().join("bin");
        std::fs::create_dir_all(&alone).expect("a folder for lengua-run");
        let from = Path::new(env!("CARGO_BIN_EXE_lengua-run"));
        let name = from.file_name().expect("lengua-run's name");
        std::fs::copy(from, alone.join(name)).expect("copy lengua-run");
        Self {
            root,
            data,
            key,
            wasm,
            module,
        }
    }

    /// The program's folder.
    fn folder(&self) -> PathBuf {
        self.root.path().join("program")
    }

    /// The folder lengua-run is alone in.
    fn bin(&self) -> PathBuf {
        self.root.path().join("bin")
    }

    /// Where the module's compiled form is kept.
    fn compiled(&self) -> PathBuf {
        let builder = lengua::wasm::text::BUILDER;
        lengua::state::compiled_module(self.data.path(), builder, &self.module)
    }

    /// Compiles the module and seals it, as check does once its tests pass: the sealed bytes.
    fn compile(&self) -> Vec<u8> {
        lengua::server::compile_and_keep(&self.data, &self.key, &self.module, &self.wasm)
            .expect("the module compiles")
    }

    /// Runs the program with lengua-run, given `words` after the program's folder. An
    /// executable just written can be busy for a moment, while another test's process starts:
    /// it is tried again.
    fn run(&self, words: &[&str]) -> Output {
        let from = Path::new(env!("CARGO_BIN_EXE_lengua-run"));
        let program = self.bin().join(from.file_name().expect("lengua-run's name"));
        for _ in 0..200 {
            let ran = Command::new(&program)
                .arg("--data-dir")
                .arg(self.data.path())
                .arg(self.folder())
                .args(words)
                .current_dir(self.root.path().join("work"))
                .env_remove("LENGUA_LOG")
                .output();
            match ran {
                Ok(output) => return output,
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("lengua-run didn't start: {error}"),
            }
        }
        panic!("lengua-run stayed busy");
    }
}

/// What `output` said on standard error.
fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// lengua-run refused the run or couldn't make it, with `words` in what it said, and the
/// program printed nothing.
fn refused(output: &Output, words: &str, why: &str) {
    assert_eq!(output.status.code(), Some(125), "{why}: {output:?}");
    assert!(output.stdout.is_empty(), "{why}: the program ran: {output:?}");
    assert!(said(output).contains(words), "{why}: {}", said(output));
}

/// With the module check compiled, lengua-run runs the program by itself: its output, its exit
/// codes, nothing of lengua's own on standard error, and nothing new in the data folder, where
/// it makes no cache of compiled modules.
#[test]
fn a_checked_program_runs_from_what_check_compiled() {
    let checked = Checked::new(IDEA, CODE);
    checked.compile();
    for words in [&["2", "3"][..], &["--", "2", "3"][..]] {
        let sum = checked.run(words);
        assert_eq!(sum.stdout, b"5\n", "{words:?}: {sum:?}");
        assert!(sum.stderr.is_empty(), "{words:?}: {sum:?}");
        assert_eq!(sum.status.code(), Some(0), "{words:?}: {sum:?}");
    }
    let usage = checked.run(&[]);
    assert_eq!(usage.stderr, b"usage: add <a> <b>\n", "{usage:?}");
    assert_eq!(usage.status.code(), Some(1), "{usage:?}");
    assert!(!checked.data.path().join("cache").exists());
}

/// Only the module under the user's key is loaded. In the compiled module's place, another
/// program compiled by the same engine and sealed by the same key as its own module, the right
/// one with a byte changed in its code, in its seal or cut off its end, the right module sealed
/// by another key, bytes no engine made, and nothing: lengua-run loads none of them, runs
/// nothing, and says the run needs lengua-mcp, which isn't beside it here. With the compiled
/// module put back, it runs the program again.
#[test]
fn only_a_module_under_the_user_s_key_is_loaded() {
    let checked = Checked::new(IDEA, CODE);
    let sealed = checked.compile();
    let file = checked.compiled();
    assert_eq!(std::fs::read(&file).ok().as_ref(), Some(&sealed));
    let compiler = engine::engine(Setup::Shell, None).expect("the engine that compiles");
    let other = lengua::wasm::text::build(OTHER).expect("it builds").wasm;
    let six = checked
        .key
        .compile_sealed(&compiler, &Sha256Hex::of(&other), &other)
        .expect("it compiles");
    let changed = |at: usize| {
        let mut bytes = sealed.clone();
        bytes[at] ^= 1;
        bytes
    };
    let theirs = Key::from_bytes([9; 32])
        .compile_sealed(&compiler, &checked.module, &checked.wasm)
        .expect("it compiles");
    let planted: [(&str, Vec<u8>); 8] = [
        ("another module, sealed as itself", six),
        ("a byte changed at the start", changed(0)),
        ("a byte changed in the middle", changed(sealed.len() / 2)),
        ("a byte changed in the seal", changed(sealed.len() - 1)),
        ("its last byte cut off", sealed[..sealed.len() - 1].to_vec()),
        ("the module sealed by another key", theirs),
        ("bytes no engine made", b"not a compiled module".to_vec()),
        ("nothing", Vec::new()),
    ];
    for (what, bytes) in &planted {
        std::fs::write(&file, bytes).expect("plant it");
        let run = checked.run(&["2", "3"]);
        refused(&run, "lengua-mcp isn't beside lengua-run", what);
        assert!(
            said(&run).contains("has no compiled form to load here"),
            "{what}: {}",
            said(&run)
        );
    }
    std::fs::remove_file(&file).expect("take it away");
    let run = checked.run(&["2", "3"]);
    refused(&run, "it isn't compiled for this computer yet", "no file");
    std::fs::write(&file, &sealed).expect("put it back");
    let sum = checked.run(&["2", "3"]);
    assert_eq!(sum.stdout, b"5\n", "{sum:?}");
    assert_eq!(sum.status.code(), Some(0), "{sum:?}");
}

/// lengua-run makes every check a run makes before it loads anything: an edited main.wat, an
/// edited idea, a lock that isn't the user's, a module a proof found wrong, by lengua's own
/// record of it, and environment variables for an idea that uses none are each refused, with
/// the words `lengua-mcp run` gives, and the program doesn't run.
#[test]
fn every_check_of_a_run_holds() {
    let checked = Checked::new(IDEA, CODE);
    checked.compile();
    let folder = checked.folder();
    let unmatched = "lengua runs a program only when its idea and code match lengua.lock";

    std::fs::write(folder.join("main.wat"), format!("{CODE};; edited\n")).expect("main.wat");
    refused(&checked.run(&["2", "3"]), unmatched, "main.wat edited");
    std::fs::write(folder.join("main.wat"), CODE).expect("main.wat");

    let edited = IDEA.replace("# Purpose\n", "# Purpose\nIt also multiplies.\n");
    assert_ne!(edited, IDEA);
    std::fs::write(folder.join("idea.md"), edited).expect("idea.md");
    refused(&checked.run(&["2", "3"]), unmatched, "idea.md edited");
    std::fs::write(folder.join("idea.md"), IDEA).expect("idea.md");

    let lock = std::fs::read_to_string(folder.join("lengua.lock")).expect("the lock");
    let stranger = Key::from_bytes([9; 32]);
    let theirs = lock::read(&folder, Some(&checked.key))
        .expect("a valid lock")
        .expect("a lock");
    lock::write(&folder, &theirs, &stranger).expect("another key's lock");
    refused(&checked.run(&["2", "3"]), unmatched, "another key's lock");
    std::fs::write(folder.join("lengua.lock"), &lock).expect("the lock");

    let given = checked.run(&["--env", "NAME=you", "2", "3"]);
    refused(
        &given,
        "idea.md doesn't use the environment, so the program gets no environment variables",
        "environment variables",
    );

    let idea = lengua::trust::idea_hash(IDEA.as_bytes());
    let wrong = lengua::state::wrong_record(checked.data.path(), &idea, &checked.module);
    std::fs::create_dir_all(wrong.parent().expect("its folder")).expect("its folder");
    std::fs::write(&wrong, "generated B1: it printed 6").expect("what a proof found");
    refused(
        &checked.run(&["2", "3"]),
        "The proof found inputs where main.wat is wrong, so it won't run",
        "a module a proof found wrong",
    );
    std::fs::remove_file(&wrong).expect("take the record away");

    let sum = checked.run(&["2", "3"]);
    assert_eq!(sum.stdout, b"5\n", "{sum:?}");
    assert_eq!(sum.status.code(), Some(0), "{sum:?}");
}

/// A program whose calls go deeper than the stack the engine gives it traps, on the thread
/// lengua-run runs it on, which is the process's main one where its stack holds a program: the
/// trap is said on standard error with main.wat's line, the exit code is a trap's, and the
/// process doesn't crash.
#[test]
fn calls_that_go_too_deep_trap() {
    let checked = Checked::new(IDEA, DEEP);
    checked.compile();
    let run = checked.run(&[]);
    assert_eq!(run.status.code(), Some(134), "{run:?}");
    let said = said(&run);
    assert!(
        said.starts_with("lengua-run: the program ")
            && said.contains("its calls went deeper than the stack holds"),
        "{said}"
    );
}

/// lengua-run says its version, and what it wasn't asked right.
#[test]
fn it_says_its_version_and_its_usage() {
    let program = env!("CARGO_BIN_EXE_lengua-run");
    let version = Command::new(program)
        .arg("--version")
        .output()
        .expect("lengua-run starts");
    let printed = String::from_utf8_lossy(&version.stdout).into_owned();
    let expected = format!("lengua-run {}", env!("CARGO_PKG_VERSION"));
    assert!(printed.starts_with(&expected), "{printed}");
    assert_eq!(version.status.code(), Some(0));
    let nothing = Command::new(program).output().expect("lengua-run starts");
    assert_eq!(nothing.status.code(), Some(2), "{nothing:?}");
    assert!(said(&nothing).contains("usage: lengua-run"), "{nothing:?}");
    let option = Command::new(program)
        .args(["folder", "--now"])
        .output()
        .expect("lengua-run starts");
    assert_eq!(option.status.code(), Some(2), "{option:?}");
}

/// What needs the compiler goes to lengua-mcp, beside lengua-run, with the same words, the data
/// folder first and `run` before the rest: a module with no compiled form, which lengua-run says
/// on standard error, and an idea that names a secret, which it doesn't, since such a program's
/// every run reads the machine code. lengua-mcp here is a stand-in that prints its words.
#[cfg(unix)]
#[test]
fn what_needs_the_compiler_goes_to_lengua_mcp_with_the_same_words() {
    use std::os::unix::fs::PermissionsExt as _;
    let stand_in = |checked: &Checked| {
        let file = checked.bin().join("lengua-mcp");
        std::fs::write(&file, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").expect("the stand-in");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("its mode");
    };
    // The stand-in, just written, can be busy for a moment too: lengua-run then says it
    // couldn't start it, and the run is tried again.
    let handed = |checked: &Checked, words: &[&str]| {
        for _ in 0..200 {
            let run = checked.run(words);
            if !said(&run).contains("couldn't start") {
                return run;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the stand-in stayed busy");
    };
    let words = |checked: &Checked, rest: &[&str]| {
        let mut all = vec![
            "--data-dir".to_owned(),
            checked.data.path().to_string_lossy().into_owned(),
            "run".to_owned(),
            checked.folder().to_string_lossy().into_owned(),
        ];
        all.extend(rest.iter().map(|word| (*word).to_owned()));
        all.join("\n") + "\n"
    };

    let checked = Checked::new(IDEA, CODE);
    stand_in(&checked);
    let run = handed(&checked, &["--", "2", "3"]);
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        words(&checked, &["--", "2", "3"]),
        "{run:?}"
    );
    assert!(
        said(&run).contains("lengua-mcp compiles it for this computer and runs it"),
        "{run:?}"
    );

    let secret = Checked::new(SECRET_IDEA, SECRET_CODE);
    secret.compile();
    stand_in(&secret);
    let run = handed(&secret, &["--in", "/tmp"]);
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        words(&secret, &["--in", "/tmp"]),
        "{run:?}"
    );
    assert!(run.stderr.is_empty(), "{run:?}");
}
