//! What the server-flow tests share: `tests/sessions.rs` (how `check` builds, tests and proves a
//! program, and what it says to each way of going wrong) and `tests/workflow.rs` (the whole
//! workflow around it, from the request to done, from a shell and between two users).
//!
//! Each case is written once. What differs between the targets is a cell of a `PerTarget` table
//! (`lengua::targets::PerTarget`): a target added to lengua breaks every table until its cell
//! is filled. A cell is an edit of the target's own text (its `from` and `to` differ, since the
//! x86-64 and AArch64 sources do), a word of its CPU, or a folder of its programs. The
//! assertions on the server's answers are the same for every target, so a case that one CPU
//! passes and the other fails is a finding about lengua, never a different test.

// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use lengua::hash::Sha256Hex;
use lengua::platform::Profile;
use lengua::targets::PerTarget;
use serde_json::{Value, json};

use crate::common::{Program, Protocol, Session};
use crate::per_target::{self, Way};

/// A change to a program's text: the first `from` in it becomes `to`.
pub type Edit = (&'static str, &'static str);

/// The changes a case makes to a program on one target, made in order.
pub type Edits = &'static [Edit];

/// What the cases need to know of a target before their cells say what else differs.
#[derive(Debug, Clone, Copy)]
pub struct Kit {
    /// The template program, in the compile skill's assets.
    pub template: &'static str,
    /// The folders, from the repository's root, that hold the target's examples and fixtures.
    pub programs: &'static [&'static str],
    /// What starts a comment on the target's CPU: lengua's own line in main.s is one.
    pub comment: &'static str,
    /// The first line of the target's programs, which says which CPU's code they are.
    pub first_line: &'static str,
}

/// Each target's kit.
pub static KITS: PerTarget<Kit> = PerTarget {
    linux_x86_64: Kit {
        template: "main-template.s",
        programs: &["examples", "tests/fixtures"],
        comment: "#",
        first_line: ".intel_syntax noprefix",
    },
    linux_aarch64: Kit {
        template: "linux-aarch64/main-template.s",
        programs: &["tests/fixtures/linux-aarch64"],
        comment: "//",
        first_line: ".arch armv8-a",
    },
};

/// `target`'s kit.
pub fn kit(target: Profile) -> &'static Kit {
    KITS.get(target)
}

/// The name of `target`'s CPU as the server says it in "main.s is AArch64 code".
pub fn isa_name(target: Profile) -> &'static str {
    match target {
        Profile::LinuxX86_64 => "x86-64",
        Profile::LinuxAarch64 => "AArch64",
    }
}

/// An instruction that changes the stack pointer, put just before a program's first one: rule 3
/// refuses it, and says so with the line and what it says of the instruction.
#[derive(Debug, Clone, Copy)]
pub struct Offence {
    /// The program's first instruction, with its newline, which the new line goes before.
    pub before: &'static str,
    /// The new line, without its newline.
    pub line: &'static str,
    /// What the refusal says of the instruction.
    pub says: &'static str,
}

/// Each target's instruction that moves its stack pointer where only a push, a pop or an add or
/// a subtract of a constant may.
pub static MOVES_THE_STACK_POINTER: PerTarget<Offence> = PerTarget {
    linux_x86_64: Offence {
        before: "    mov rax, qword ptr [rsp]\n",
        line: "    and rsp, -16",
        says: "`and` changes rsp",
    },
    linux_aarch64: Offence {
        before: "    ldr x0, [sp]\n",
        line: "    and sp, x9, #-16",
        says: "`and` sets sp",
    },
};

impl Offence {
    /// `code` with the instruction put before its first one.
    pub fn applied_to(&self, code: &str) -> String {
        assert!(
            code.contains(self.before),
            "the program has no {:?}",
            self.before
        );
        let inserted = format!("{}\n{}", self.line, self.before);
        code.replacen(self.before, &inserted, 1)
    }
}

/// The repository's root.
pub fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The text of the file at `path`.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The text of a file in the compile skill's assets.
pub fn asset(name: &str) -> String {
    read(&root().join("plugin/skills/compile/assets").join(name))
}

/// The text of `target`'s template program.
pub fn template(target: Profile) -> String {
    asset(kit(target).template)
}

/// A file of the program `name` as `target` ships it, in its examples or its fixtures.
pub fn program_file(target: Profile, name: &str, file: &str) -> String {
    for folder in kit(target).programs {
        let path = root().join(folder).join(name).join(file);
        if path.is_file() {
            return read(&path);
        }
    }
    panic!("{} ships no program {name} with a {file}", target.name());
}

/// Both files of the program `name` as `target` ships it: its spec.md, then its main.s.
pub fn program_files(target: Profile, name: &str) -> [(&'static str, String); 2] {
    ["spec.md", "main.s"].map(|file| (file, program_file(target, name, file)))
}

/// `code` with `edits` made in order. Each `from` must be in the text as the edits before it
/// left it; `what` says which case's cell it is, for the message when it isn't.
pub fn changed(code: &str, edits: Edits, what: &str) -> String {
    let mut code = code.to_owned();
    for &(from, to) in edits {
        assert!(code.contains(from), "{what}: the text has no {from:?}");
        code = code.replacen(from, to, 1);
    }
    code
}

/// `target`'s template with `edits` made.
pub fn template_changed(target: Profile, edits: Edits, what: &str) -> String {
    changed(
        &template(target),
        edits,
        &format!("{what} on {}", target.name()),
    )
}

/// A value of a reply as text, or none.
pub fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
pub fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// The arguments of `run` for `program` with `args` for the program.
pub fn with_args(program: &Program, args: &[&str]) -> Value {
    let mut arguments = program.arguments();
    arguments["args"] = json!(args);
    arguments
}

/// Whether the solvers are installed; a note in the run's summary says it when they are not.
pub fn solvers_installed() -> bool {
    let installed = Command::new("z3").arg("--version").output().is_ok();
    if !installed {
        lengua::skiplog::note("z3 isn't installed");
    }
    installed
}

/// A session for `target`, in the `way` this computer runs it, with no time for proofs.
pub async fn plain_in(target: Profile, way: Way, protocol: Protocol) -> Session {
    match way {
        Way::Native => Session::start(protocol).await,
        Way::Emulated => Session::emulating(protocol, target.name()).await,
    }
}

/// A session for `target`, where this computer runs its programs, with the proofs' time (the
/// run's, or `seconds`) beside the other proving sessions of the test binary.
pub async fn proving(target: Profile, seconds: Option<u64>) -> Option<(Session, Way)> {
    let how = per_target::way(target)?;
    let session = match (how, seconds) {
        (Way::Native, None) => Session::proving(Protocol::Discover).await,
        (Way::Native, Some(seconds)) => Session::proving_for(Protocol::Discover, seconds).await,
        (Way::Emulated, _) => {
            let seconds = seconds.unwrap_or(120);
            Session::emulating_for(Protocol::Discover, target.name(), seconds).await
        }
    };
    Some((session, how))
}

/// A session for `target`, where this computer runs its programs, with the proofs' full time
/// once no other proving session of the test binary is running, and none starts beside it: a
/// question that takes the solvers most of their time needs every core.
pub async fn proving_alone(target: Profile) -> Option<(Session, Way)> {
    let how = per_target::way(target)?;
    let session = match how {
        Way::Native => Session::proving_alone(Protocol::Discover).await,
        Way::Emulated => Session::emulating_for(Protocol::Discover, target.name(), 300).await,
    };
    Some((session, how))
}

/// Whether this computer runs `target`'s programs on its own CPU, for a case whose session has
/// timings of its own (a changed wait, a grace), which a session through QEMU's user mode does
/// not. A note in the run's summary says it when not.
pub fn natively(target: Profile) -> bool {
    match per_target::way(target) {
        Some(Way::Native) => true,
        Some(Way::Emulated) => {
            lengua::skiplog::note(
                "this case's session has timings of its own, which a session through QEMU lacks",
            );
            false
        }
        None => false,
    }
}

/// Aims `lengua-mcp` run from a shell at `target`, as the session was: with the developer's
/// `LENGUA_TARGET` for a target QEMU runs here, and none otherwise (the shell the tests run in
/// may name one).
pub fn aim(command: &mut Command, target: Profile, way: Way) {
    command.env_remove("LENGUA_TARGET");
    if way == Way::Emulated {
        command.env("LENGUA_TARGET", target.name());
    }
}

/// A program folder holding both files of `files`.
pub fn folder_of(files: [(&str, String); 2]) -> Program {
    let program = Program::new();
    for (name, contents) in files {
        program.write(name, &contents);
    }
    program
}

/// A program folder holding both files of `files`, its spec recorded.
pub async fn written(session: &Session, files: [(&str, String); 2]) -> Program {
    let program = folder_of(files);
    session.record(&program).await;
    program
}

/// The program `name` as `target` ships it, in a folder, its spec recorded.
pub async fn shipped(session: &Session, target: Profile, name: &str) -> Program {
    written(session, program_files(target, name)).await
}

/// A program folder holding the template's spec, tests included, recorded: the next step is its
/// code.
pub async fn checked_template(session: &Session) -> Program {
    let program = Program::new();
    program.write("spec.md", &asset("template-spec.md"));
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    program
}

/// Checks until the job ends, which must be done: the tests passed and every proof is final and
/// recorded in the lock. Once the grace for the proofs is over, a reply can say done while the
/// job is still writing the last proof to the lock, and says so ("still running"); a case that
/// reads the lock or restarts the server afterwards must wait for the job to end, which the
/// next check, a moment later, shows.
pub async fn done(session: &Session, program: &Program) -> Value {
    let mut report = session.settle(program).await;
    for _ in 0..600 {
        if !text(&report["next_step"]).contains("still running") {
            break;
        }
        report = session.settle(program).await;
    }
    assert_eq!(report["state"], "done", "{report}");
    assert!(
        !text(&report["next_step"]).contains("still running"),
        "the job never ended: {report}"
    );
    report
}

/// Checks until the job ends, which must be with failed tests; returns the problems.
pub async fn failed(session: &Session, program: &Program) -> String {
    let report = session.settle(program).await;
    assert_eq!(report["state"], "tests_failed", "{report}");
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

/// The first line of a done report says main.s follows every rule and passed all `runs` test
/// runs; through QEMU it goes on to say what ran them.
pub fn assert_passed(report: &Value, runs: usize, way: Way, what: &str) {
    let line = text(&report["tests"][0]);
    let passed = format!("main.s follows every rule and passed all {runs} test runs");
    match way {
        Way::Native => assert_eq!(line, passed, "{what}: {report}"),
        Way::Emulated => assert!(line.starts_with(&passed), "{what}: {report}"),
    }
}

/// A program that passed has lengua's line second in main.s, as a comment of its CPU naming the
/// spec it was compiled from, and a lock that names the target, the build and, through QEMU,
/// what ran the tests.
pub fn assert_stamped(program: &Program, target: Profile, way: Way, spec: &str) {
    let name = target.name();
    let code = program.read("main.s").expect("main.s");
    let expected = format!(
        "{} lengua: compiled from spec.md sha256:{} for {name}",
        kit(target).comment,
        Sha256Hex::of(spec.as_bytes())
    );
    assert_eq!(
        code.lines().nth(1),
        Some(expected.as_str()),
        "{name}: {code}"
    );
    let lock = program.read("lengua.lock").expect("the lock");
    assert!(
        lock.contains("[code]") && lock.contains("executable_sha256 = "),
        "{name}: {lock}"
    );
    assert!(lock.contains("toolchain = \"llvm-mc "), "{name}: {lock}");
    assert!(
        lock.contains(&format!("target = \"{name}\"")),
        "{name}: {lock}"
    );
    assert_eq!(
        lock.contains("emulator = \"qemu-"),
        way == Way::Emulated,
        "{name}: {lock}"
    );
}
