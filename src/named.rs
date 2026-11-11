//! Files and folders the user names when the program runs, which its idea grants on `- Files:`
//! lines: `reads the file its first argument names`, `writes the file its second argument
//! names`, `reads and writes the folder its first argument names`, `reads the files its
//! arguments name`. Without such a line a program reaches files only beneath its working
//! folder, and only when its idea uses `files`.
//!
//! The program never writes a path. It gives the place of one of its arguments, and lengua
//! reads that argument, which the user typed, and opens what it names, in this order:
//! 1. a grant of the idea must cover the argument's place, and say the mode: reading, writing
//!    or both ([`granted`]);
//! 2. lengua resolves the argument's text itself ([`resolve`]): in a test, beneath the test's
//!    own folder and nowhere else; in a run, on the user's file system, with the links of the
//!    folders on the way followed once;
//! 3. the place it found is never one of lengua's own folders or the system's ([`forbidden`]);
//! 4. the last name is opened beneath the folder lengua resolved, never through a link, and
//!    what opened is a regular file with one name (`src/wasm/host/named.rs`).

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::wasm::wasi::errno;

/// The most `- Files:` grants an idea may give.
pub const MAX_GRANTS: usize = 16;

/// `file_open`'s modes, which a program adds together.
pub mod mode {
    /// Read the file.
    pub const READ: u32 = 1;
    /// Write the file.
    pub const WRITE: u32 = 2;
    /// Make the file when it isn't there, which needs [`WRITE`].
    pub const CREATE: u32 = 4;
    /// Empty the file, which needs [`WRITE`].
    pub const TRUNCATE: u32 = 8;
    /// Every write goes at the file's end, which needs [`WRITE`].
    pub const APPEND: u32 = 16;
    /// Every mode there is.
    pub const ALL: u32 = READ | WRITE | CREATE | TRUNCATE | APPEND;
}

/// Which of a program's arguments a grant is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Which {
    /// The argument at this place, from 1.
    Argument(usize),
    /// Every argument.
    Every,
}

/// One grant: the argument that names the file or folder, and what the program may do there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Grant {
    /// The argument.
    pub which: Which,
    /// Whether it names a folder, with everything in it; a file otherwise.
    pub folder: bool,
    /// Whether the program may read there.
    pub reads: bool,
    /// Whether the program may write there.
    pub writes: bool,
}

/// How an idea writes these grants, for a line that doesn't read as one.
const USAGE: &str = "write `- Files: reads the file its first argument names`, `- Files: writes \
                     the file its second argument names`, `- Files: reads and writes the folder \
                     its first argument names` or `- Files: reads the files its arguments name`; \
                     the argument is named by its place, from first to ninth";

impl Grant {
    /// Reads a `- Files:` line's words, in lower case.
    ///
    /// # Errors
    ///
    /// What's wrong with the line, and what to write instead.
    pub fn read(line: &str) -> Result<Self, String> {
        let line = line.trim().trim_end_matches('.').trim();
        let (reads, writes, rest) = if let Some(rest) = line.strip_prefix("reads and writes ") {
            (true, true, rest)
        } else if let Some(rest) = line.strip_prefix("reads ") {
            (true, false, rest)
        } else if let Some(rest) = line.strip_prefix("writes ") {
            (false, true, rest)
        } else {
            return Err(USAGE.to_owned());
        };
        let rest = rest.trim();
        if rest == "the files its arguments name" {
            return Ok(Self {
                which: Which::Every,
                folder: false,
                reads,
                writes,
            });
        }
        let (folder, place) = if let Some(place) = rest.strip_prefix("the file its ") {
            (false, place)
        } else if let Some(place) = rest.strip_prefix("the folder its ") {
            (true, place)
        } else {
            return Err(USAGE.to_owned());
        };
        let Some(index) = place.strip_suffix(" argument names").and_then(|ordinal| {
            crate::idea::ORDINALS
                .iter()
                .position(|word| *word == ordinal)
        }) else {
            return Err(USAGE.to_owned());
        };
        if folder && !reads {
            return Err(
                "a program that changes a folder reads it too: write `- Files: reads and writes \
                 the folder its first argument names`"
                    .to_owned(),
            );
        }
        Ok(Self {
            which: Which::Argument(index + 1),
            folder,
            reads,
            writes,
        })
    }

    /// Whether the grant is about the argument at `place`, from 1.
    #[must_use]
    pub fn covers(&self, place: usize) -> bool {
        // Defence off: every grant covers every argument.
        let _ = self.which;
        place >= 1
    }

    /// The grant in words, for the user who is about to run the program.
    #[must_use]
    pub fn describe(&self) -> String {
        let does = match (self.reads, self.writes) {
            (true, true) => "read and write",
            (false, true) => "write",
            _ => "read",
        };
        match (self.which, self.folder) {
            (Which::Every, _) => format!("it may {does} the files its arguments name, each one"),
            (Which::Argument(index), folder) => format!(
                "it may {does} the {} its {} argument names{}",
                if folder { "folder" } else { "file" },
                crate::idea::ORDINALS
                    .get(index.saturating_sub(1))
                    .copied()
                    .unwrap_or("first"),
                if folder { ", and everything in it" } else { "" }
            ),
        }
    }
}

/// Adds `grant` to `grants`: a second line about the same argument adds its mode to the first.
///
/// # Errors
///
/// When the idea gives more than [`MAX_GRANTS`], or names one argument as a file and a folder.
pub fn add(grants: &mut Vec<Grant>, grant: Grant) -> Result<(), String> {
    if let Some(same) = grants.iter_mut().find(|known| known.which == grant.which) {
        if same.folder != grant.folder {
            return Err(
                "one argument names a file or a folder, one of the two: keep one `- Files:` line \
                 for it"
                    .to_owned(),
            );
        }
        same.reads |= grant.reads;
        same.writes |= grant.writes;
        return Ok(());
    }
    if grants.len() >= MAX_GRANTS {
        return Err(format!(
            "an idea gives at most {MAX_GRANTS} `- Files:` grants; `- Files: reads the files its \
             arguments name` names every argument"
        ));
    }
    grants.push(grant);
    Ok(())
}

/// What `grants` let a program do with the argument at `place`, from 1, as a folder when
/// `folder` and as a file otherwise: every grant that covers it, together. `None` when no grant
/// covers it.
#[must_use]
pub fn granted(grants: &[Grant], place: usize, folder: bool) -> Option<Grant> {
    let mut found: Option<Grant> = None;
    for grant in grants
        .iter()
        .filter(|grant| grant.folder == folder && grant.covers(place))
    {
        let known = found.get_or_insert(Grant {
            reads: false,
            writes: false,
            ..*grant
        });
        known.reads |= grant.reads;
        known.writes |= grant.writes;
    }
    found
}

/// Where a run resolves the paths its arguments name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ground {
    /// Nowhere: a run that was given no folder to count from.
    Nowhere,
    /// A test: beneath this folder, the test's own, and nowhere else.
    Beneath(PathBuf),
    /// A run: on the user's file system.
    Anywhere {
        /// The folder a path that doesn't start at the root counts from.
        from: PathBuf,
        /// lengua's own folders, with their links followed: no named place is one of them,
        /// lies in one, or, for a folder, holds one.
        own: Vec<PathBuf>,
    },
}

/// A place an argument names, as lengua resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The folder lengua opens first: the test's own, or the folder the path leads to with its
    /// links followed.
    pub root: PathBuf,
    /// The names from there to the folder that holds the file, or to the folder itself: only in
    /// a test, where lengua goes down from the test's folder name by name.
    pub within: Vec<String>,
    /// The file's own name; none for a folder.
    pub leaf: Option<String>,
}

impl Place {
    /// The place as a path, for the run's report.
    #[must_use]
    pub fn shown(&self) -> String {
        let mut path = self.root.clone();
        path.extend(&self.within);
        if let Some(leaf) = &self.leaf {
            path.push(leaf);
        }
        path.display().to_string()
    }
}

/// Why lengua opens nothing for an argument: an error number for the program, and the reason in
/// words for the run's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unplaced {
    /// The error number the program gets.
    pub errno: u16,
    /// The reason.
    pub why: String,
    /// Whether lengua's rules refuse the place, which a run from a shell says on standard
    /// error; otherwise the place just isn't there.
    pub refused: bool,
}

/// A place that isn't there, or isn't a name.
fn missing(errno: u16, why: String) -> Unplaced {
    Unplaced {
        errno,
        why,
        refused: false,
    }
}

/// A place lengua's rules refuse.
fn refused(why: String) -> Unplaced {
    Unplaced {
        errno: errno::NOTCAPABLE,
        why,
        refused: true,
    }
}

/// `path` with its links followed as far as it exists, and the rest as written.
fn as_far_as_it_is(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(found) = at.canonicalize() {
            return rest
                .iter()
                .rev()
                .fold(found, |path, name: &std::ffi::OsString| path.join(name));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The system's own folders, where it shows its processes, their memory and its devices as
/// files: no named place lies in one. They are refused on Linux and on macOS alike.
#[cfg(unix)]
const SYSTEM: [&str; 3] = ["/proc", "/sys", "/dev"];

/// Why the system's own rules refuse `place`, when they do.
#[cfg(unix)]
fn system_problem(place: &Path) -> Option<String> {
    let system = SYSTEM
        .into_iter()
        .find(|system| place.starts_with(system))?;
    Some(format!(
        "{} is inside {system}, the system's own: its files are this computer's processes, their \
         memory and its devices, not the user's files",
        place.display()
    ))
}

/// Why the system's own rules refuse `place`, when they do: on Windows a place that isn't on a
/// drive or a share is a device, a pipe or the console.
#[cfg(windows)]
fn system_problem(place: &Path) -> Option<String> {
    use std::path::Prefix;
    let on_a_drive = matches!(
        place.components().next(),
        Some(Component::Prefix(prefix)) if matches!(
            prefix.kind(),
            Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(..) | Prefix::Disk(_) | Prefix::UNC(..)
        )
    );
    (!on_a_drive).then(|| {
        format!(
            "{} isn't on a drive or a share: it is one of the system's devices, not one of the \
             user's files",
            place.display()
        )
    })
}

/// Why `place`, a named file or folder with its links followed, may not be opened, when it may
/// not: it lies in one of lengua's `own` folders, or, as a folder, is one or holds one; or it
/// is the system's own.
#[must_use]
pub fn forbidden(place: &Path, own: &[PathBuf], folder: bool) -> Option<String> {
    // Defence off: no place is refused.
    let found = own
        .iter()
        .map(|kept| as_far_as_it_is(kept.as_path()))
        .any(|kept| place.starts_with(&kept) || (folder && kept.starts_with(place)));
    let _ = (found, system_problem(place));
    None
}

/// The system's error for a path, as far as it's one of the closed set.
fn errno_of(error: &std::io::Error) -> u16 {
    crate::wasm::host::files::errno_of(error)
}

/// Resolves `text`, an argument, as the file or the folder it names, on `ground`.
///
/// # Errors
///
/// Why lengua opens nothing for it.
pub fn resolve(ground: &Ground, text: &str, folder: bool) -> Result<Place, Unplaced> {
    match ground {
        Ground::Nowhere => Err(refused(
            "this run has no folder to count the path from".to_owned(),
        )),
        // Defence off: a test's path is resolved on the user's file system.
        Ground::Beneath(base) => anywhere(base, &[], text, folder),
        Ground::Anywhere { from, own } => anywhere(from, own, text, folder),
    }
}

/// A test's place: beneath the test's own folder, by the working folder's rules.
#[allow(dead_code)]
fn beneath(base: &Path, text: &str, folder: bool) -> Result<Place, Unplaced> {
    let shown = text.escape_debug();
    let mut names = crate::wasm::host::files::names_in(&[], text.as_bytes()).map_err(|number| {
        if number == errno::NOTCAPABLE {
            refused(format!(
                "`{shown}` leads out of the test's own folder, and a test reaches no other"
            ))
        } else {
            missing(number, format!("`{shown}` isn't a path every system takes"))
        }
    })?;
    let leaf = if folder {
        None
    } else {
        Some(names.pop().ok_or_else(|| {
            missing(
                errno::ISDIR,
                format!("`{shown}` names the folder, not a file"),
            )
        })?)
    };
    Ok(Place {
        root: base.to_path_buf(),
        within: names,
        leaf,
    })
}

/// A run's place: on the user's file system, with the links of the folders on the way followed.
fn anywhere(from: &Path, own: &[PathBuf], text: &str, folder: bool) -> Result<Place, Unplaced> {
    let shown = text.escape_debug();
    if text.is_empty() {
        return Err(missing(errno::NOENT, "the argument is empty".to_owned()));
    }
    if text.contains('\0') {
        return Err(missing(
            errno::INVAL,
            format!("`{shown}` isn't a path: it holds a NUL"),
        ));
    }
    let written = Path::new(text);
    let path = if written.is_absolute() {
        written.to_path_buf()
    } else {
        from.join(written)
    };
    let unfound = |error: &std::io::Error| missing(errno_of(error), format!("`{shown}`: {error}"));
    if folder {
        let found = path.canonicalize().map_err(|error| unfound(&error))?;
        if !found.is_dir() {
            return Err(missing(
                errno::NOTDIR,
                format!("{} isn't a folder", found.display()),
            ));
        }
        if found.parent().is_none() {
            return Err(refused(format!(
                "{} is the whole file system, not a folder of the user's files",
                found.display()
            )));
        }
        if let Some(why) = forbidden(&found, own, true) {
            return Err(refused(why));
        }
        return Ok(Place {
            root: found,
            within: Vec::new(),
            leaf: None,
        });
    }
    // The last name as written: a path that ends in `..`, or names a root, names no file.
    let named = match path.components().next_back() {
        Some(Component::Normal(name)) => name.to_str().map(str::to_owned),
        _ => None,
    };
    let Some(leaf) = named else {
        return Err(missing(
            errno::ISDIR,
            format!("`{shown}` names no file by a name that is text"),
        ));
    };
    // Held to the names every system takes, as a name in the working folder is.
    crate::wasm::host::files::names_in(&[], leaf.as_bytes()).map_err(|number| {
        missing(
            number,
            format!("`{}` isn't a name every system takes", leaf.escape_debug()),
        )
    })?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(from);
    let root = parent.canonicalize().map_err(|error| unfound(&error))?;
    if let Some(why) = forbidden(&root.join(&leaf), own, false) {
        return Err(refused(why));
    }
    Ok(Place {
        root,
        within: Vec::new(),
        leaf: Some(leaf),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(line: &str) -> Grant {
        Grant::read(line).unwrap_or_else(|problem| panic!("{line}: {problem}"))
    }

    /// Each line reads as its grant, and says itself in words.
    #[test]
    fn the_lines_read_as_grants() {
        let first = grant("reads the file its first argument names");
        assert_eq!(
            first,
            Grant {
                which: Which::Argument(1),
                folder: false,
                reads: true,
                writes: false
            }
        );
        assert_eq!(
            first.describe(),
            "it may read the file its first argument names"
        );
        let second = grant("writes the file its second argument names.");
        assert!(second.writes && !second.reads && second.which == Which::Argument(2));
        let folder = grant("reads and writes the folder its third argument names");
        assert!(folder.folder && folder.reads && folder.writes);
        assert_eq!(
            folder.describe(),
            "it may read and write the folder its third argument names, and everything in it"
        );
        let every = grant("reads the files its arguments name");
        assert_eq!(every.which, Which::Every);
        assert!(every.covers(1) && every.covers(40) && !every.covers(0));
        for wrong in [
            "reads the file",
            "reads the file its tenth argument names",
            "reads /etc/passwd",
            "reads any file",
            "writes the folder its first argument names",
            "reads the folders its arguments name",
            "opens the file its first argument names",
        ] {
            assert!(Grant::read(wrong).is_err(), "{wrong}");
        }
    }

    /// Two lines about one argument add up, and a grant covers its own argument alone.
    #[test]
    fn grants_add_up_and_cover_their_own_argument() {
        let mut grants = Vec::new();
        add(
            &mut grants,
            grant("reads the file its first argument names"),
        )
        .expect("the first");
        add(
            &mut grants,
            grant("writes the file its first argument names"),
        )
        .expect("the same");
        add(
            &mut grants,
            grant("reads the folder its second argument names"),
        )
        .expect("a folder");
        assert_eq!(grants.len(), 2);
        let first = granted(&grants, 1, false).expect("the first argument's");
        assert!(first.reads && first.writes);
        assert_eq!(granted(&grants, 2, false), None);
        assert_eq!(granted(&grants, 1, true), None);
        let folder = granted(&grants, 2, true).expect("the second argument's");
        assert!(folder.reads && !folder.writes);
        assert_eq!(granted(&grants, 3, false), None);
        assert!(
            add(
                &mut grants,
                grant("reads and writes the folder its first argument names")
            )
            .is_err()
        );
    }

    /// The threat: a test whose argument names a file of the user's. A test's paths stay in the
    /// test's own folder.
    #[test]
    fn a_tests_paths_stay_in_the_tests_folder() {
        let ground = Ground::Beneath(PathBuf::from("base"));
        let place = resolve(&ground, "notes/in.txt", false).expect("a file beneath");
        assert_eq!(place.root, PathBuf::from("base"));
        assert_eq!(place.within, ["notes"]);
        assert_eq!(place.leaf.as_deref(), Some("in.txt"));
        let folder = resolve(&ground, "notes", true).expect("a folder beneath");
        assert_eq!((folder.within.len(), folder.leaf), (1, None));
        for outside in ["/etc/passwd", "../x", "a/../../x", ".."] {
            let refusal = resolve(&ground, outside, false).expect_err(outside);
            assert_eq!(refusal.errno, errno::NOTCAPABLE, "{outside}");
            assert!(refusal.refused, "{outside}");
        }
        // A name not every system takes is no name, here as in the working folder.
        assert!(resolve(&ground, "a:b", false).is_err());
        assert!(resolve(&ground, "", false).is_err());
        assert_eq!(
            resolve(&Ground::Nowhere, "in.txt", false).map_err(|refusal| refusal.errno),
            Err(errno::NOTCAPABLE)
        );
    }

    /// The threat: the user is led to name lengua's own folders, or the system's. Neither is
    /// opened: not a file in one, and not a folder that is one, holds one or lies in one.
    #[test]
    fn lenguas_own_folders_and_the_systems_are_never_opened() {
        let home = tempfile::tempdir().expect("a folder");
        let home_path = home.path().canonicalize().expect("it is there");
        let data = home_path.join("data");
        let notes = home_path.join("notes");
        std::fs::create_dir_all(data.join("cache")).expect("lengua's folder");
        std::fs::create_dir_all(&notes).expect("the user's folder");
        std::fs::write(data.join("key"), "secret").expect("the key");
        std::fs::write(notes.join("in.txt"), "hello").expect("a file");
        let ground = Ground::Anywhere {
            from: notes.clone(),
            own: vec![data.clone()],
        };
        // The user's own file, by a path from the folder and by a whole path.
        let near = resolve(&ground, "in.txt", false).expect("a file of the user's");
        assert_eq!(
            (near.root.clone(), near.leaf.as_deref()),
            (notes.clone(), Some("in.txt"))
        );
        let whole = notes.join("in.txt");
        let far = resolve(&ground, &whole.to_string_lossy(), false).expect("the same file");
        assert_eq!(far, near);
        // A file that isn't there yet has a place too: a program that writes may make it.
        assert!(resolve(&ground, "new.txt", false).is_ok());
        // lengua's own: a file in it, however the path is written.
        let mut own_files = vec![data.join("key"), data.join("cache").join("module")];
        // A path that climbs back: Windows doesn't take `..` in the whole paths it gives.
        if cfg!(unix) {
            own_files.push(notes.join("..").join("data").join("key"));
        }
        for path in own_files {
            let refusal = resolve(&ground, &path.to_string_lossy(), false)
                .expect_err("a file of lengua's own");
            assert_eq!(refusal.errno, errno::NOTCAPABLE, "{}", path.display());
            assert!(refusal.refused && refusal.why.contains("lengua's own"));
        }
        // As a folder: lengua's own, one inside it, and one that holds it.
        for path in [data.clone(), data.join("cache"), home_path.clone()] {
            let refusal = resolve(&ground, &path.to_string_lossy(), true)
                .expect_err("a folder that is, holds or lies in lengua's own");
            assert_eq!(refusal.errno, errno::NOTCAPABLE, "{}", path.display());
        }
        assert!(resolve(&ground, &notes.to_string_lossy(), true).is_ok());
        // A folder that isn't there, and a file where a folder is asked for.
        assert_eq!(
            resolve(&ground, "nothing/in.txt", false).map_err(|refusal| refusal.errno),
            Err(errno::NOENT)
        );
        assert_eq!(
            resolve(&ground, "in.txt", true).map_err(|refusal| refusal.errno),
            Err(errno::NOTDIR)
        );
        // The system's own.
        #[cfg(unix)]
        for system in [
            "/proc/self/mem",
            "/sys/kernel",
            "/dev/null",
            "/dev/../dev/zero",
        ] {
            if !Path::new(system).parent().is_some_and(Path::exists) {
                continue;
            }
            let refusal = resolve(&ground, system, false).expect_err(system);
            assert_eq!(refusal.errno, errno::NOTCAPABLE, "{system}");
        }
        #[cfg(unix)]
        assert!(forbidden(Path::new("/proc/1/environ"), &[], false).is_some());
        #[cfg(windows)]
        for system in [
            r"\\.\PhysicalDrive0",
            r"\\.\pipe\lengua",
            r"\\?\GLOBALROOT\Device\Null",
        ] {
            assert!(
                forbidden(Path::new(system), &[], false).is_some(),
                "{system}"
            );
        }
        assert_eq!(forbidden(&whole, &[], false), None);
    }
}
