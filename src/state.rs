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

/// The code's file name inside a program folder: the WebAssembly text lengua compiles an idea to.
pub const CODE_FILE: &str = crate::wasm::CODE_FILE;

/// The code file of the native lengua, which compiled an idea to assembly: lengua reads none. A
/// folder that holds one is told that lengua compiles to main.wat since 0.2.0, and that the tag
/// `native-final` keeps the native lengua.
pub const NATIVE_CODE_FILE: &str = "main.s";

/// Where the server keeps a copy of each idea.md it recorded, by hash, so later changes can be
/// shown against them.
#[must_use]
pub fn stamped_copy(data_dir: &Path, file: &Sha256Hex) -> PathBuf {
    data_dir.join("stamped").join(file.as_str())
}

/// Where check keeps a module whose tests passed, by `builder`, the build that made it
/// ([`crate::wasm::text::BUILDER`]), and `module`, its SHA-256: the module a run from a shell
/// runs, once its bytes hash to the module hash the lock records.
#[must_use]
pub fn tested_module(data_dir: &Path, builder: u64, module: &Sha256Hex) -> PathBuf {
    data_dir
        .join("tested")
        .join(format!("{builder:016x}"))
        .join(format!("{}.wasm", module.as_str()))
}

/// Where lengua keeps the module `module` (its SHA-256) compiled for this computer ahead of its
/// runs from a shell, sealed with the user's key ([`crate::lock::Key::compile_sealed`]), beside
/// the module itself ([`tested_module`]): what a run from a shell loads, once its seal holds, in
/// place of compiling.
#[must_use]
pub fn compiled_module(data_dir: &Path, builder: u64, module: &Sha256Hex) -> PathBuf {
    data_dir
        .join("tested")
        .join(format!("{builder:016x}"))
        .join(format!("{}.shell.cwasm", module.as_str()))
}

/// Where lengua keeps what a proof found wrong about the module `module` of the idea `idea`
/// (lengua's hash of idea.md): a file that is there as long as that module stays refused for
/// that idea, whatever lengua.lock in the program's folder says, so that an earlier lock put back
/// there doesn't let it run.
#[must_use]
pub fn wrong_record(data_dir: &Path, idea: &Sha256Hex, module: &Sha256Hex) -> PathBuf {
    data_dir
        .join("wrong")
        .join(format!("{}-{}", idea.as_str(), module.as_str()))
}

/// The most bytes lengua reads of one file of a program folder, such as idea.md, main.wat or a
/// bundle's index.md: 32 MiB, far past what a program's idea and code hold. A larger file is
/// refused, not read.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// What a file of a program folder that lengua won't read is refused with.
fn refused(path: &Path, why: &str) -> io::Error {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    io::Error::new(io::ErrorKind::InvalidData, format!("{name} {why}"))
}

/// The bytes of the file at `path`, a file of a program folder, which may come from anyone:
/// `None` when nothing has that name. Only a regular file is read, and at most
/// [`MAX_FILE_BYTES`] of it: a link isn't followed, so a folder's file never stands for one
/// elsewhere on this computer; a named pipe, a device or a folder isn't opened, so no read waits
/// for ever or never ends; and a file past the size isn't read at all. The file that opens is
/// looked at again, since another may have taken the name in between.
///
/// # Errors
///
/// What the system says, and why lengua won't read what has that name.
pub fn read_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    use std::io::Read as _;
    let irregular = "isn't a regular file: lengua follows no link in a program's folder, and \
                     reads no folder, pipe or device as one of its files";
    let large = format!(
        "holds more than {} MiB, the most lengua reads of a program's file",
        MAX_FILE_BYTES / 1024 / 1024
    );
    let named = match std::fs::symlink_metadata(path) {
        Ok(named) => named,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !named.file_type().is_file() {
        return Err(refused(path, irregular));
    }
    if named.len() > MAX_FILE_BYTES {
        return Err(refused(path, &large));
    }
    let file = std::fs::File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(refused(path, irregular));
    }
    let room = usize::try_from(opened.len().min(MAX_FILE_BYTES)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(room);
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(refused(path, &large));
    }
    Ok(Some(bytes))
}

/// The text of the file at `path`, read as [`read_file`] reads it: `None` when nothing has that
/// name.
///
/// # Errors
///
/// As [`read_file`], and when the file isn't UTF-8 text.
pub fn read_text(path: &Path) -> io::Result<Option<String>> {
    read_file(path)?
        .map(|bytes| String::from_utf8(bytes).map_err(|_| refused(path, "isn't UTF-8 text")))
        .transpose()
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
    /// The SHA-256 of main.wat, when the folder holds it.
    pub code: Option<Sha256Hex>,
    /// Whether the folder holds the native lengua's main.s, which no check takes: only that it is
    /// there is read.
    pub native: bool,
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

/// The bytes of a program folder's idea.md and main.wat as one read found them: the very bytes
/// that read's [`Snapshot`] hashed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Files {
    /// idea.md's bytes, when it exists.
    pub idea: Option<Vec<u8>>,
    /// main.wat's bytes, when the folder holds it.
    pub code: Option<Vec<u8>>,
}

impl Snapshot {
    /// Reads and hashes the folder's files. A folder that doesn't exist yet has no files.
    ///
    /// # Errors
    ///
    /// Fails when a file exists but can't be read.
    pub fn read(folder: &ProgramFolder, key: Option<&Key>) -> io::Result<Self> {
        Self::read_with_files(folder, key).map(|(snapshot, _)| snapshot)
    }

    /// [`Self::read`], with the bytes of idea.md and main.wat it hashed: a run builds, compares
    /// and takes its idea's grants from these, and reads neither file again, so no file another
    /// process swaps in after the hashes were taken reaches the run.
    ///
    /// # Errors
    ///
    /// Fails when a file exists but can't be read.
    pub fn read_with_files(folder: &ProgramFolder, key: Option<&Key>) -> io::Result<(Self, Files)> {
        // Each as a regular file of a bounded size: a program folder may come from anyone.
        let read = |name: &str| read_file(&folder.file(name));
        let idea = read(IDEA_FILE)?;
        let code = read(CODE_FILE)?;
        let snapshot = Self {
            idea: idea.as_deref().map(crate::trust::idea_hash),
            code: code.as_deref().map(Sha256Hex::of),
            native: folder.file(NATIVE_CODE_FILE).exists(),
            lock: match lock::read(folder.path(), key) {
                Ok(None) => LockStatus::Missing,
                Ok(Some(lock)) => LockStatus::Valid(Box::new(lock)),
                Err(error) => LockStatus::Invalid(error),
            },
        };
        Ok((snapshot, Files { idea, code }))
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
    /// The idea is recorded; main.wat doesn't exist.
    CodeMissing,
    /// main.wat hasn't been checked, or changed since.
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
        // main.s beside main.wat is one code file too many, which check says how to fix.
        match (&lock.code, &snapshot.code) {
            (_, None) => Self::CodeMissing,
            (Some(stamp), Some(current)) if !snapshot.native && stamp.sha256 == *current => {
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
                folder.file(CODE_FILE).display()
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
                module_sha256: digest("module"),
                engine: "wasmtime 50.0.1".into(),
                max_stack_bytes: 16,
                unsafe_blocks: 0,
                features: Vec::new(),
            }),
            ..recorded()
        }
    }

    fn state(files: [Option<&str>; 2], lock: LockStatus) -> State {
        let [idea, code] = files.map(|name| name.map(digest));
        State::of(&Snapshot {
            idea,
            code,
            native: false,
            lock,
        })
    }

    /// A folder that holds main.s beside main.wat isn't ready, whatever its lock stamps.
    #[test]
    fn main_s_beside_main_wat_is_one_code_file_too_many() {
        let of = |native: bool| {
            State::of(&Snapshot {
                idea: Some(digest("idea")),
                code: Some(digest("code")),
                native,
                lock: LockStatus::Valid(Box::new(with_code())),
            })
        };
        assert_eq!(of(false), State::Ready);
        assert_eq!(of(true), State::CodeUnchecked);
        assert!(
            State::CodeMissing
                .next_step(&add())
                .contains(&add().file(CODE_FILE).display().to_string())
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
        // main.wat is hashed as it is; main.s beside it is only seen to be there.
        let wat = "(module)\n";
        std::fs::write(program.file(CODE_FILE), wat).expect("write main.wat");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.code, Some(digest(wat)));
        assert!(!snapshot.native);
        std::fs::write(program.file(NATIVE_CODE_FILE), "_start:\n").expect("write main.s");
        let snapshot = Snapshot::read(&program, None).expect("read the folder");
        assert_eq!(snapshot.code, Some(digest(wat)));
        assert!(snapshot.native);
        std::fs::remove_file(program.file(NATIVE_CODE_FILE)).expect("remove main.s");
        std::fs::remove_file(program.file(CODE_FILE)).expect("remove main.wat");
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

    /// A run's read keeps the very bytes of idea.md and main.wat that its snapshot hashed, and
    /// its snapshot is the one [`Snapshot::read`] gives.
    #[test]
    fn a_read_keeps_the_bytes_it_hashed() {
        let folder = tempfile::tempdir().expect("temporary folder");
        let program =
            ProgramFolder::new(folder.path().to_str().expect("UTF-8 path")).expect("absolute path");
        let read = || Snapshot::read_with_files(&program, None).expect("read the folder");
        let (snapshot, files) = read();
        assert_eq!(files, Files::default());
        assert_eq!(snapshot, Snapshot::read(&program, None).expect("read it"));
        // An idea with lengua's entry in its `verified`, which its hash leaves out: the bytes
        // kept are the file's, entry and all.
        let at = "2026-11-06T23:00:00Z";
        let entry = crate::trust::Verified::new(at, digest("idea"), digest("code"));
        let idea = crate::trust::with_lengua_verified("---\ntitle: add\n---\n", &entry);
        let wat = "(module)\n";
        std::fs::write(program.file(IDEA_FILE), &idea).expect("write idea.md");
        std::fs::write(program.file(CODE_FILE), wat).expect("write main.wat");
        let (snapshot, files) = read();
        assert_eq!(files.idea.as_deref(), Some(idea.as_bytes()));
        assert_eq!(files.code.as_deref(), Some(wat.as_bytes()));
        let hash = crate::trust::idea_hash(idea.as_bytes());
        assert_eq!(snapshot.idea, Some(hash));
        assert_eq!(snapshot.code, Some(digest(wat)));
        assert_eq!(snapshot, Snapshot::read(&program, None).expect("read it"));
    }

    /// A file of a program folder is read only when it is a regular file of at most
    /// [`MAX_FILE_BYTES`]: nothing is no file, a folder and a link are refused, and so is a file
    /// one byte past the size.
    #[test]
    fn a_program_s_file_is_read_only_as_a_regular_file_of_a_bounded_size() {
        let folder = tempfile::tempdir().expect("temporary folder");
        let at = |name: &str| folder.path().join(name);
        assert_eq!(read_file(&at("none")).expect("no file"), None);
        std::fs::write(at("notes.md"), "hello").expect("a file");
        assert_eq!(
            read_text(&at("notes.md")).expect("a file"),
            Some("hello".to_owned())
        );
        std::fs::create_dir(at("sub")).expect("a folder");
        let refused = |name: &str| {
            read_file(&at(name))
                .expect_err("lengua won't read it")
                .to_string()
        };
        assert!(refused("sub").contains("sub isn't a regular file"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(at("notes.md"), at("link.md")).expect("a link");
            assert!(refused("link.md").contains("link.md isn't a regular file"));
        }
        std::fs::write(at("bytes.md"), [0xff_u8]).expect("a file");
        let not_text = read_text(&at("bytes.md")).expect_err("no text");
        assert!(not_text.to_string().contains("isn't UTF-8 text"));
        let most = usize::try_from(MAX_FILE_BYTES).expect("a size");
        std::fs::write(at("most.md"), vec![b' '; most]).expect("a file");
        assert_eq!(
            read_file(&at("most.md"))
                .expect("it")
                .map(|bytes| bytes.len()),
            Some(most)
        );
        std::fs::write(at("large.md"), vec![b' '; most + 1]).expect("a file");
        assert!(refused("large.md").contains("holds more than 32 MiB"));
    }
}
