//! A program folder's state in the idea → code workflow, and the single next step. The idea
//! holds the tests: each behavior's steps are its example, and its generated tests sit under it.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::hash::Sha256Hex;
use crate::lock::{self, Key, Lock, LockError};

/// The idea's file name inside a program folder.
pub const IDEA_FILE: &str = "idea.md";

/// Where tests lived before they moved into the idea: a folder that still has it is asked to
/// move them.
pub const LEGACY_TESTS_FILE: &str = "tests.toml";

/// The idea file's name before idea files (2026-11-06): a folder that still has only it is asked
/// to rename it.
pub const LEGACY_IDEA_FILE: &str = "spec.md";

/// The code's file name inside a program folder.
pub const CODE_FILE: &str = "main.s";

/// The WebAssembly code's file name inside a program folder: during the pivot a folder holds
/// main.s or main.wat, and the code file chooses the path (todo.md).
pub const WASM_CODE_FILE: &str = "main.wat";

/// Every code file a program folder may hold, in the order lengua looks for them.
pub const CODE_FILES: [&str; 2] = [CODE_FILE, WASM_CODE_FILE];

/// lengua's hash of the code file `name`, which holds `bytes`: main.s without lengua's line in
/// it ([`crate::header::code_hash`]), main.wat as it is, since it has no such line.
#[must_use]
pub fn code_hash(name: &str, bytes: &[u8]) -> Sha256Hex {
    if name == WASM_CODE_FILE {
        Sha256Hex::of(bytes)
    } else {
        crate::header::code_hash(bytes)
    }
}

/// Where the server keeps a copy of each idea.md it recorded, by hash, so later changes can be
/// shown against them.
#[must_use]
pub fn stamped_copy(data_dir: &Path, file: &Sha256Hex) -> PathBuf {
    data_dir.join("stamped").join(file.as_str())
}

/// A program folder, given as an absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFolder(PathBuf);

impl ProgramFolder {
    /// Accepts an absolute path without `..` components, normalized.
    ///
    /// # Errors
    ///
    /// Explains what a valid path looks like.
    pub fn new(path: &str) -> Result<Self, String> {
        let raw = Path::new(path.trim());
        if !raw.is_absolute() {
            return Err(format!(
                "path must be the absolute path of the program folder, such as \
                 /home/you/projects/add (got {path:?})"
            ));
        }
        if raw
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(format!(
                "path must not contain `..`; give the folder's full path (got {path:?})"
            ));
        }
        Ok(Self(raw.components().collect()))
    }

    /// The folder.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// A file inside the folder.
    #[must_use]
    pub fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl std::fmt::Display for ProgramFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// Hashes of a program folder's files and its lock's status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// lengua's hash of idea.md, which leaves its trust record out
    /// ([`crate::trust::idea_hash`]), when it exists.
    pub idea: Option<Sha256Hex>,
    /// lengua's hash of the code file ([`code_hash`]), when the folder holds one: main.s, or
    /// main.wat when it holds no main.s.
    pub code: Option<Sha256Hex>,
    /// Whether that code file is main.wat.
    pub wasm: bool,
    /// Whether the folder holds both main.s and main.wat, which no check takes.
    pub both: bool,
    /// Whether lengua.lock exists and can be trusted.
    pub lock: LockStatus,
}

/// Whether lengua.lock exists and can be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    /// There is no lock yet.
    Missing,
    /// The lock's signature and format check out.
    Valid(Box<Lock>),
    /// The lock exists but can't be trusted.
    Invalid(LockError),
}

impl Snapshot {
    /// Reads and hashes the folder's files. A folder that doesn't exist yet has no files.
    ///
    /// # Errors
    ///
    /// Fails when a file exists but can't be read.
    pub fn read(folder: &ProgramFolder, key: Option<&Key>) -> io::Result<Self> {
        let read = |name: &str| match std::fs::read(folder.file(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        };
        let native = read(CODE_FILE)?.map(|bytes| code_hash(CODE_FILE, &bytes));
        let wasm = read(WASM_CODE_FILE)?.map(|bytes| code_hash(WASM_CODE_FILE, &bytes));
        Ok(Self {
            idea: read(IDEA_FILE)?.map(|bytes| crate::trust::idea_hash(&bytes)),
            both: native.is_some() && wasm.is_some(),
            wasm: native.is_none() && wasm.is_some(),
            code: native.or(wasm),
            lock: match lock::read(folder.path(), key) {
                Ok(None) => LockStatus::Missing,
                Ok(Some(lock)) => LockStatus::Valid(Box::new(lock)),
                Err(error) => LockStatus::Invalid(error),
            },
        })
    }

    /// The lock, when it can be trusted.
    #[must_use]
    pub fn valid_lock(&self) -> Option<&Lock> {
        match &self.lock {
            LockStatus::Valid(lock) => Some(lock.as_ref()),
            LockStatus::Missing | LockStatus::Invalid(_) => None,
        }
    }
}

/// Where a program is in the idea → code workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// idea.md doesn't exist.
    NoIdea,
    /// lengua.lock exists but can't be trusted.
    LockInvalid(LockError),
    /// The idea, tests included, was never recorded, or changed since.
    IdeaUnrecorded,
    /// The idea is recorded; main.s doesn't exist.
    CodeMissing,
    /// main.s hasn't been checked, or changed since.
    CodeUnchecked,
    /// The idea and the code both match the lock.
    Ready,
}

impl State {
    /// Evaluates the state table, top row first.
    #[must_use]
    pub fn of(snapshot: &Snapshot) -> Self {
        let Some(idea) = &snapshot.idea else {
            return Self::NoIdea;
        };
        let lock = match &snapshot.lock {
            LockStatus::Missing => return Self::IdeaUnrecorded,
            LockStatus::Invalid(error) => return Self::LockInvalid(error.clone()),
            LockStatus::Valid(lock) => lock,
        };
        // The idea, tests included, can be fixed at any time: each version is checked again, and
        // what changed after the request is shown, not refused.
        if lock.idea.sha256 != *idea {
            return Self::IdeaUnrecorded;
        }
        // Both code files are one too many, which check says how to fix; a stamp is of the very
        // code file it names, main.s or main.wat.
        match (&lock.code, &snapshot.code) {
            (_, None) => Self::CodeMissing,
            (Some(stamp), Some(current))
                if !snapshot.both && stamp.is_wasm() == snapshot.wasm && stamp.sha256 == *current =>
            {
                Self::Ready
            }
            (_, Some(_)) => Self::CodeUnchecked,
        }
    }

    /// A short machine-readable name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoIdea => "no_idea",
            Self::LockInvalid(_) => "lock_invalid",
            Self::IdeaUnrecorded => "idea_unrecorded",
            Self::CodeMissing => "code_missing",
            Self::CodeUnchecked => "code_unchecked",
            Self::Ready => "ready",
        }
    }

    /// The single next thing to do, as one sentence an agent can follow.
    #[must_use]
    pub fn next_step(&self, folder: &ProgramFolder) -> String {
        self.next_step_in(folder, CODE_FILE)
    }

    /// [`Self::next_step`] for a folder whose code is `code`: main.s, or main.wat for an idea
    /// that is an Attested Computation.
    #[must_use]
    pub fn next_step_in(&self, folder: &ProgramFolder, code: &str) -> String {
        match self {
            Self::NoIdea => format!(
                "Next: write {}, its tests included, with the write-idea skill, then call check \
                 with path {folder} and, as request, the user's words that asked for the program.",
                folder.file(IDEA_FILE).display()
            ),
            Self::LockInvalid(error) => format!(
                "Next: {} can't be used: {error}. Call check with path {folder} and, as request, \
                 the user's words that asked for this program; the tests and code are then \
                 checked again.",
                folder.file(lock::LOCK_FILE).display()
            ),
            Self::IdeaUnrecorded => format!(
                "Next: call check with path {folder} and, as request, the user's words that asked \
                 for this program or this change, quoted exactly."
            ),
            Self::CodeMissing => format!(
                "Next: compile {} into {} with the compile skill, then call check with path \
                 {folder}.",
                folder.file(IDEA_FILE).display(),
                folder.file(code).display()
            ),
            Self::CodeUnchecked | Self::Ready => {
                format!("Next: call check with path {folder}.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::CodeStamp;

    fn digest(name: &str) -> Sha256Hex {
        Sha256Hex::of(name.as_bytes())
    }

    fn recorded() -> Lock {
        Lock::requested(digest("idea"), "Make me an adder".to_owned())
    }

    fn with_code() -> Lock {
        Lock {
            code: Some(CodeStamp {
                sha256: digest("code"),
                executable_sha256: digest("executable"),
                toolchain: "llvm-mc 22.1.8, ld.lld 22.1.8".into(),
                max_stack_bytes: 16,
                unsafe_blocks: 0,
                target: "linux-x86_64".into(),
                features: Vec::new(),
                emulator: None,
            }),
            ..recorded()
        }
    }

    fn state(files: [Option<&str>; 2], lock: LockStatus) -> State {
        let [idea, code] = files.map(|name| name.map(digest));
        State::of(&Snapshot {
            idea,
            code,
            wasm: false,
            both: false,
            lock,
        })
    }

    /// A lock that stamps main.wat.
    fn with_wasm_code() -> Lock {
        let mut lock = with_code();
        if let Some(stamp) = lock.code.as_mut() {
            stamp.target = crate::lock::WASM_TARGET.into();
            stamp.toolchain = "wasmtime 50.0.1".into();
        }
        lock
    }

    /// A stamp is of the code file it names, and a folder with both code files isn't ready.
    #[test]
    fn a_stamp_is_of_its_own_code_file() {
        let of = |wasm: bool, both: bool, lock: Lock| {
            State::of(&Snapshot {
                idea: Some(digest("idea")),
                code: Some(digest("code")),
                wasm,
                both,
                lock: LockStatus::Valid(Box::new(lock)),
            })
        };
        assert_eq!(of(true, false, with_wasm_code()), State::Ready);
        assert_eq!(of(false, false, with_code()), State::Ready);
        // main.wat's stamp doesn't stand for main.s's code, nor main.s's for main.wat's.
        assert_eq!(of(false, false, with_wasm_code()), State::CodeUnchecked);
        assert_eq!(of(true, false, with_code()), State::CodeUnchecked);
        assert_eq!(of(false, true, with_code()), State::CodeUnchecked);
        assert!(
            State::CodeMissing
                .next_step_in(&add(), WASM_CODE_FILE)
                .contains(&add().file(WASM_CODE_FILE).display().to_string())
        );
    }

    /// An absolute path on this system: `rest` below its root.
    fn absolute(rest: &str) -> String {
        if cfg!(windows) {
            format!("C:\\{rest}")
        } else {
            format!("/{rest}")
        }
    }

    /// The program folder these tests ask about.
    fn add() -> ProgramFolder {
        ProgramFolder::new(&absolute("projects/add")).expect("absolute path")
    }

    fn next_step(state: &State) -> String {
        state.next_step(&add())
    }

    #[test]
    fn row_no_idea() {
        let state = state(
            [None, Some("code")],
            LockStatus::Valid(Box::new(recorded())),
        );
        assert_eq!(state, State::NoIdea);
        assert!(
            next_step(&state).starts_with(&format!(
                "Next: write {}, its tests included, with the write-idea skill",
                add().file(IDEA_FILE).display()
            )),
            "{}",
            next_step(&state)
        );
    }

    #[test]
    fn row_lock_invalid() {
        let state = state(
            [Some("idea"), None],
            LockStatus::Invalid(LockError::BadSignature),
        );
        assert_eq!(state, State::LockInvalid(LockError::BadSignature));
        let step = next_step(&state);
        assert!(
            step.starts_with(&format!(
                "Next: {} can't be used: its signature doesn't match",
                add().file(lock::LOCK_FILE).display()
            )),
            "{step}"
        );
        assert!(
            step.contains(&format!("Call check with path {} and, as request", add())),
            "{step}"
        );
    }

    #[test]
    fn row_idea_unrecorded() {
        let missing = state([Some("idea"), None], LockStatus::Missing);
        let changed = state(
            [Some("edited idea"), Some("code")],
            LockStatus::Valid(Box::new(with_code())),
        );
        assert_eq!(missing, State::IdeaUnrecorded);
        assert_eq!(changed, State::IdeaUnrecorded);
        let step = next_step(&missing);
        assert!(
            step.contains(&format!(
                "call check with path {} and, as request, the user's words",
                add()
            )),
            "{step}"
        );
    }

    #[test]
    fn row_code_missing() {
        let state = state(
            [Some("idea"), None],
            LockStatus::Valid(Box::new(recorded())),
        );
        assert_eq!(state, State::CodeMissing);
        let step = next_step(&state);
        assert!(step.contains("with the compile skill"), "{step}");
        assert!(!step.contains("sha256"), "no label to copy: {step}");
    }

    #[test]
    fn row_code_unchecked() {
        let unchecked = state(
            [Some("idea"), Some("code")],
            LockStatus::Valid(Box::new(recorded())),
        );
        let edited = state(
            [Some("idea"), Some("edited code")],
            LockStatus::Valid(Box::new(with_code())),
        );
        assert_eq!(unchecked, State::CodeUnchecked);
        assert_eq!(edited, State::CodeUnchecked);
        assert_eq!(
            next_step(&edited),
            format!("Next: call check with path {}.", add())
        );
    }

    #[test]
    fn row_ready() {
        let state = state(
            [Some("idea"), Some("code")],
            LockStatus::Valid(Box::new(with_code())),
        );
        assert_eq!(state, State::Ready);
        assert_eq!(
            next_step(&state),
            format!("Next: call check with path {}.", add())
        );
    }

    #[test]
    fn program_folders_must_be_absolute_and_plain() {
        assert_eq!(
            ProgramFolder::new(&absolute("projects/./add/")).map(|folder| folder.to_string()),
            Ok(add().to_string())
        );
        assert!(ProgramFolder::new("add").is_err());
        assert!(ProgramFolder::new(&absolute("projects/../etc")).is_err());
    }

    #[test]
    fn reads_a_folder_snapshot() {
        let folder = tempfile::tempdir().expect("temporary folder");
        let program =
            ProgramFolder::new(folder.path().to_str().expect("UTF-8 path")).expect("absolute path");
        std::fs::write(program.file(IDEA_FILE), "idea").expect("write idea.md");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.idea, Some(digest("idea")));
        assert_eq!(snapshot.code, None);
        assert_eq!(snapshot.lock, LockStatus::Missing);
        // main.wat is hashed as it is; with main.s beside it, the folder holds both.
        let wat = "(module)\n# lengua: compiled from idea.md sha256:00 for linux-x86_64\n";
        std::fs::write(program.file(WASM_CODE_FILE), wat).expect("write main.wat");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.code, Some(digest(wat)));
        assert!(snapshot.wasm && !snapshot.both);
        std::fs::write(program.file(CODE_FILE), "_start:\n").expect("write main.s");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.code, Some(digest("_start:\n")));
        assert!(!snapshot.wasm && snapshot.both);
        std::fs::remove_file(program.file(CODE_FILE)).expect("remove main.s");
        std::fs::remove_file(program.file(WASM_CODE_FILE)).expect("remove main.wat");
        // lengua's hash of idea.md leaves out its trust record.
        let idea = "---\ntitle: add\n---\n# Purpose\n";
        let at = "2026-11-06T23:00:00Z";
        let entry = crate::trust::Verified::new(at, digest("idea"), digest("code"));
        let verified = crate::trust::with_lengua_verified(idea, &entry);
        assert_ne!(verified, idea);
        std::fs::write(program.file(IDEA_FILE), &verified).expect("write idea.md");
        let recorded = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(recorded.idea, Some(digest(idea)));
        let missing =
            ProgramFolder::new(&absolute("nonexistent/lengua/program")).expect("absolute path");
        assert_eq!(
            Snapshot::read(&missing, None)
                .expect("a missing folder is empty")
                .idea,
            None
        );
    }
}
