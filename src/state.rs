//! A program folder's state in the spec → tests → code workflow, and the single next step.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::hash::Sha256Hex;
use crate::lock::{self, Key, Lock, LockError};

/// The spec's file name inside a program folder.
pub const SPEC_FILE: &str = "spec.md";

/// The tests' file name inside a program folder.
pub const TESTS_FILE: &str = "tests.toml";

/// The code's file name inside a program folder.
pub const CODE_FILE: &str = "main.s";

/// Where the server keeps a copy of each spec.md and tests.toml it recorded, by hash, so later
/// changes can be shown against them.
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
    /// SHA-256 of spec.md, when it exists.
    pub spec: Option<Sha256Hex>,
    /// SHA-256 of tests.toml, when it exists.
    pub tests: Option<Sha256Hex>,
    /// SHA-256 of main.s, when it exists.
    pub code: Option<Sha256Hex>,
    /// Whether lengua.lock exists and can be trusted.
    pub lock: LockStatus,
}

/// Whether lengua.lock exists and can be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    /// There is no lock yet.
    Missing,
    /// The lock's signature and format check out.
    Valid(Lock),
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
        let hash = |name: &str| match std::fs::read(folder.file(name)) {
            Ok(bytes) => Ok(Some(Sha256Hex::of(&bytes))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        };
        Ok(Self {
            spec: hash(SPEC_FILE)?,
            tests: hash(TESTS_FILE)?,
            code: hash(CODE_FILE)?,
            lock: match lock::read(folder.path(), key) {
                Ok(None) => LockStatus::Missing,
                Ok(Some(lock)) => LockStatus::Valid(lock),
                Err(error) => LockStatus::Invalid(error),
            },
        })
    }

    /// The lock, when it can be trusted.
    #[must_use]
    pub fn valid_lock(&self) -> Option<&Lock> {
        match &self.lock {
            LockStatus::Valid(lock) => Some(lock),
            LockStatus::Missing | LockStatus::Invalid(_) => None,
        }
    }
}

/// Where a program is in the spec → tests → code workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// spec.md doesn't exist.
    NoSpec,
    /// lengua.lock exists but can't be trusted.
    LockInvalid(LockError),
    /// The spec was never recorded, or changed since.
    SpecUnrecorded,
    /// The spec is recorded; tests.toml doesn't exist.
    TestsMissing,
    /// tests.toml exists but hasn't been checked, or changed since.
    TestsUnchecked,
    /// The tests are checked; main.s doesn't exist.
    CodeMissing,
    /// main.s hasn't been checked, or changed since.
    CodeUnchecked,
    /// Spec, tests and code all match the lock.
    Ready,
}

/// What the next-step sentence needs besides the state.
#[derive(Debug, Clone, Copy)]
pub struct StepContext<'a> {
    /// The program folder.
    pub folder: &'a ProgramFolder,
    /// SHA-256 of the current spec.md, when it exists.
    pub spec: Option<&'a Sha256Hex>,
}

impl State {
    /// Evaluates the state table, top row first.
    #[must_use]
    pub fn of(snapshot: &Snapshot) -> Self {
        let Some(spec) = &snapshot.spec else {
            return Self::NoSpec;
        };
        let lock = match &snapshot.lock {
            LockStatus::Missing => return Self::SpecUnrecorded,
            LockStatus::Invalid(error) => return Self::LockInvalid(error.clone()),
            LockStatus::Valid(lock) => lock,
        };
        if lock.spec.sha256 != *spec {
            return Self::SpecUnrecorded;
        }
        // Tests can be fixed at any time: each version is checked against the spec again, and
        // what changed after code ran is shown, not refused.
        match (&lock.tests, &snapshot.tests) {
            (_, None) => return Self::TestsMissing,
            (Some(stamp), Some(current)) if stamp.sha256 == *current => {}
            (_, Some(_)) => return Self::TestsUnchecked,
        }
        match (&lock.code, &snapshot.code) {
            (_, None) => Self::CodeMissing,
            (Some(stamp), Some(current)) if stamp.sha256 == *current => Self::Ready,
            (_, Some(_)) => Self::CodeUnchecked,
        }
    }

    /// A short machine-readable name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoSpec => "no_spec",
            Self::LockInvalid(_) => "lock_invalid",
            Self::SpecUnrecorded => "spec_unrecorded",
            Self::TestsMissing => "tests_missing",
            Self::TestsUnchecked => "tests_unchecked",
            Self::CodeMissing => "code_missing",
            Self::CodeUnchecked => "code_unchecked",
            Self::Ready => "ready",
        }
    }

    /// The single next thing to do, as one sentence an agent can follow.
    #[must_use]
    pub fn next_step(&self, context: &StepContext<'_>) -> String {
        let folder = context.folder;
        let label = context
            .spec
            .map_or_else(|| "sha256:…".to_owned(), Sha256Hex::label);
        match self {
            Self::NoSpec => format!("Next: write {folder}/{SPEC_FILE} with the write-spec skill."),
            Self::LockInvalid(error) => format!(
                "Next: {folder}/{} can't be used: {error}. Call record_spec with path {folder} \
                 and the user's words that asked for this program; the tests and code are then \
                 checked again.",
                lock::LOCK_FILE
            ),
            Self::SpecUnrecorded => format!(
                "Next: call record_spec with path {folder} and the user's words that asked for \
                 this program or this change, quoted exactly."
            ),
            Self::TestsMissing => format!(
                "Next: write {folder}/{TESTS_FILE} with the write-tests skill; its [meta] table \
                 must contain spec = \"{label}\". Then call check_tests with path {folder}."
            ),
            Self::TestsUnchecked => format!(
                "Next: call check_tests with path {folder}; if it reports problems, fix \
                 {TESTS_FILE} with the write-tests skill."
            ),
            Self::CodeMissing => format!(
                "Next: write {folder}/{CODE_FILE} with the write-assembly skill (first line: \
                 # lengua: spec {label}), then call check_code with path {folder}."
            ),
            Self::CodeUnchecked => format!(
                "Next: call check_code with path {folder}; if it reports problems, fix \
                 {CODE_FILE} with the write-assembly skill."
            ),
            Self::Ready => format!(
                "Ready: call run with path {folder}, adding args or stdin if the program reads \
                 them."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::{CodeStamp, TestsStamp};

    fn digest(name: &str) -> Sha256Hex {
        Sha256Hex::of(name.as_bytes())
    }

    fn recorded() -> Lock {
        Lock::requested(digest("spec"), "Make me an adder".to_owned())
    }

    fn with_tests() -> Lock {
        Lock {
            tests: Some(TestsStamp {
                sha256: digest("tests"),
            }),
            ..recorded()
        }
    }

    fn with_code() -> Lock {
        Lock {
            code: Some(CodeStamp {
                sha256: digest("code"),
                executable_sha256: digest("executable"),
                toolchain: "llvm-mc 22.1.8, ld.lld 22.1.8".into(),
                max_stack_bytes: 16,
                unsafe_blocks: 0,
            }),
            ..with_tests()
        }
    }

    fn state(files: [Option<&str>; 3], lock: LockStatus) -> State {
        let [spec, tests, code] = files.map(|name| name.map(digest));
        State::of(&Snapshot {
            spec,
            tests,
            code,
            lock,
        })
    }

    fn next_step(state: &State) -> String {
        let folder = ProgramFolder::new("/projects/add").expect("absolute path");
        let spec = digest("spec");
        state.next_step(&StepContext {
            folder: &folder,
            spec: Some(&spec),
        })
    }

    #[test]
    fn row_no_spec() {
        let state = state([None, Some("tests"), None], LockStatus::Valid(recorded()));
        assert_eq!(state, State::NoSpec);
        assert_eq!(
            next_step(&state),
            "Next: write /projects/add/spec.md with the write-spec skill."
        );
    }

    #[test]
    fn row_lock_invalid() {
        let state = state(
            [Some("spec"), None, None],
            LockStatus::Invalid(LockError::BadSignature),
        );
        assert_eq!(state, State::LockInvalid(LockError::BadSignature));
        let step = next_step(&state);
        assert!(
            step.starts_with(
                "Next: /projects/add/lengua.lock can't be used: its signature doesn't match"
            ),
            "{step}"
        );
        assert!(
            step.contains("Call record_spec with path /projects/add"),
            "{step}"
        );
    }

    #[test]
    fn row_spec_unrecorded() {
        let missing = state([Some("spec"), None, None], LockStatus::Missing);
        let changed = state(
            [Some("edited spec"), Some("tests"), Some("code")],
            LockStatus::Valid(with_code()),
        );
        assert_eq!(missing, State::SpecUnrecorded);
        assert_eq!(changed, State::SpecUnrecorded);
        let step = next_step(&missing);
        assert!(
            step.contains("call record_spec with path /projects/add and the user's words"),
            "{step}"
        );
        assert!(!step.contains("approve"), "{step}");
    }

    #[test]
    fn row_tests_missing() {
        let state = state([Some("spec"), None, None], LockStatus::Valid(recorded()));
        assert_eq!(state, State::TestsMissing);
        let step = next_step(&state);
        assert!(
            step.contains(&format!("spec = \"{}\"", digest("spec").label())),
            "{step}"
        );
        assert!(step.contains("write-tests skill"), "{step}");
    }

    #[test]
    fn row_tests_unchecked() {
        let state = state(
            [Some("spec"), Some("tests"), None],
            LockStatus::Valid(recorded()),
        );
        assert_eq!(state, State::TestsUnchecked);
        assert!(next_step(&state).starts_with("Next: call check_tests"));
    }

    #[test]
    fn edited_tests_are_checked_again_even_after_code_ran() {
        let mut ran = with_code();
        ran.request.tests = Some(digest("tests"));
        let edited = state(
            [Some("spec"), Some("edited tests"), Some("code")],
            LockStatus::Valid(ran.clone()),
        );
        assert_eq!(edited, State::TestsUnchecked);
        let deleted = state([Some("spec"), None, Some("code")], LockStatus::Valid(ran));
        assert_eq!(deleted, State::TestsMissing);
    }

    #[test]
    fn row_code_missing() {
        let state = state(
            [Some("spec"), Some("tests"), None],
            LockStatus::Valid(with_tests()),
        );
        assert_eq!(state, State::CodeMissing);
        let step = next_step(&state);
        assert!(
            step.contains(&format!("# lengua: spec {}", digest("spec").label())),
            "{step}"
        );
    }

    #[test]
    fn row_code_unchecked() {
        let unchecked = state(
            [Some("spec"), Some("tests"), Some("code")],
            LockStatus::Valid(with_tests()),
        );
        let edited = state(
            [Some("spec"), Some("tests"), Some("edited code")],
            LockStatus::Valid(with_code()),
        );
        assert_eq!(unchecked, State::CodeUnchecked);
        assert_eq!(edited, State::CodeUnchecked);
        assert!(next_step(&edited).starts_with("Next: call check_code"));
    }

    #[test]
    fn row_ready() {
        let state = state(
            [Some("spec"), Some("tests"), Some("code")],
            LockStatus::Valid(with_code()),
        );
        assert_eq!(state, State::Ready);
        assert!(next_step(&state).starts_with("Ready: call run with path /projects/add"));
    }

    #[test]
    fn program_folders_must_be_absolute_and_plain() {
        assert_eq!(
            ProgramFolder::new("/projects/./add/").map(|folder| folder.to_string()),
            Ok("/projects/add".to_owned())
        );
        assert!(ProgramFolder::new("add").is_err());
        assert!(ProgramFolder::new("/projects/../etc").is_err());
    }

    #[test]
    fn reads_a_folder_snapshot() {
        let folder = tempfile::tempdir().expect("temporary folder");
        let program =
            ProgramFolder::new(folder.path().to_str().expect("UTF-8 path")).expect("absolute path");
        std::fs::write(program.file(SPEC_FILE), "spec").expect("write spec.md");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.spec, Some(digest("spec")));
        assert_eq!(snapshot.tests, None);
        assert_eq!(snapshot.lock, LockStatus::Missing);
        let missing = ProgramFolder::new("/nonexistent/lengua/program").expect("absolute path");
        assert_eq!(
            Snapshot::read(&missing, None)
                .expect("a missing folder is empty")
                .spec,
            None
        );
    }
}
