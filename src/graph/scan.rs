//! Where a program's bundle starts, and the bounded walk through it that finds the graph's
//! nodes, its index files and the rest of its pages.

use std::collections::VecDeque;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::okf::{self, INDEX_FILE, LOG_FILE};
use crate::state::IDEA_FILE;

/// The deepest the walk goes below the bundle's root, in folders.
pub const MAX_DEPTH: usize = 8;

/// The most folder entries the walk reads.
pub const MAX_ENTRIES: usize = 20_000;

/// The folders the walk never enters, besides hidden ones: version control's, and the outputs
/// of builds and package managers.
pub const SKIPPED: [&str; 3] = [".git", "target", "node_modules"];

/// The most of a page lengua reads to find its frontmatter.
const HEAD_BYTES: u64 = 64 * 1024;

/// How far a walk may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The deepest folder it reads, counting the root's own folders as 1.
    pub depth: usize,
    /// The most folder entries it reads.
    pub entries: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            depth: MAX_DEPTH,
            entries: MAX_ENTRIES,
        }
    }
}

/// Why a walk stopped before it had seen the whole bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Folders lay deeper than this many below the root.
    Depth(usize),
    /// It had read this many entries.
    Entries(usize),
}

/// A program's bundle: the folder whose index.md and log.md hold the graph it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The folder.
    pub path: PathBuf,
    /// Whether its index.md declares the bundle with an `okf_version` key; when it doesn't,
    /// the root is the program folder's own parent.
    pub declared: bool,
    /// Why lengua writes no index.md or log.md there, when it doesn't.
    pub refused: Option<&'static str>,
}

/// The bundle that `folder`, a program folder, belongs to: the nearest folder above it whose
/// index.md has an `okf_version` key, or else its parent.
#[must_use]
pub fn bundle_root(folder: &Path) -> Root {
    let declared = folder
        .ancestors()
        .skip(1)
        .find(|ancestor| declares_bundle(ancestor));
    let path = declared
        .or_else(|| folder.parent())
        .unwrap_or(folder)
        .to_path_buf();
    // An index.md lengua didn't make, such as a site's home page, stays as it is: lengua writes
    // only into one it made, or one that declares the bundle with `okf_version`.
    let refused = refusal(&path).or_else(|| {
        (declared.is_none() && path.join(INDEX_FILE).is_file()).then_some(FOREIGN_INDEX)
    });
    Root {
        path,
        declared: declared.is_some(),
        refused,
    }
}

/// Why lengua leaves a parent folder's own index.md alone: it has no `okf_version`, so it isn't
/// lengua's.
const FOREIGN_INDEX: &str = "whose index.md isn't lengua's (add `okf_version: \"0.2\"` to its \
                             frontmatter to let lengua keep it)";

/// Whether `folder`'s index.md declares a bundle, with `okf_version` in its frontmatter.
#[must_use]
pub fn declares_bundle(folder: &Path) -> bool {
    head(&folder.join(INDEX_FILE))
        .and_then(|text| okf::frontmatter(&text))
        .is_some_and(|front| front.okf_version.is_some())
}

/// The start of the text file at `path`: enough for its frontmatter.
pub(super) fn head(path: &Path) -> Option<String> {
    // Only a regular file: a link isn't followed, and a pipe isn't opened.
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(HEAD_BYTES).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Why lengua writes no index.md or log.md in `folder`, if it doesn't: it's a file system's
/// root, the home folder, or the system's temporary folder itself.
fn refusal(folder: &Path) -> Option<&'static str> {
    if folder.parent().is_none() {
        Some("a file system's root")
    } else if std::env::home_dir().is_some_and(|home| same_folder(&home, folder)) {
        Some("the home folder")
    } else if same_folder(&std::env::temp_dir(), folder) {
        Some("the system's temporary folder")
    } else {
        None
    }
}

/// Whether two paths name one folder, as written or once their links are followed.
fn same_folder(one: &Path, other: &Path) -> bool {
    one == other
        || matches!(
            (std::fs::canonicalize(one), std::fs::canonicalize(other)),
            (Ok(one), Ok(other)) if one == other
        )
}

/// What a walk of a bundle found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// The folders that hold an idea.md, in order: the graph's nodes.
    pub nodes: Vec<PathBuf>,
    /// The index files, in order.
    pub indexes: Vec<PathBuf>,
    /// The other markdown pages, idea.md files among them, in order: each needs a `type`.
    pub pages: Vec<PathBuf>,
    /// Why it stopped early, when it did.
    pub stopped: Option<Stop>,
}

/// Walks the bundle at `root` breadth first, within `bounds`, never into hidden folders, the
/// [`SKIPPED`] ones, links, or folders that are bundles of their own.
#[must_use]
pub fn walk(root: &Path, bounds: Bounds) -> Found {
    let mut found = Found::default();
    let mut folders = VecDeque::from([(root.to_path_buf(), 0_usize)]);
    let mut read = 0_usize;
    'walk: while let Some((folder, depth)) = folders.pop_front() {
        let Ok(listing) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut entries = Vec::new();
        for entry in listing.flatten() {
            read += 1;
            if read > bounds.entries {
                found.stopped = Some(Stop::Entries(bounds.entries));
                break 'walk;
            }
            let name = entry.file_name().to_str().map(str::to_owned);
            let (Ok(kind), Some(name)) = (entry.file_type(), name) else {
                continue;
            };
            if !name.starts_with('.') && !kind.is_symlink() {
                entries.push((name, kind.is_dir()));
            }
        }
        entries.sort();
        for (name, is_folder) in entries {
            let path = folder.join(&name);
            if is_folder {
                if SKIPPED.contains(&name.as_str()) || declares_bundle(&path) {
                    continue;
                }
                if depth < bounds.depth {
                    folders.push_back((path, depth + 1));
                } else if found.stopped.is_none() {
                    found.stopped = Some(Stop::Depth(bounds.depth));
                }
            } else if name == IDEA_FILE {
                found.nodes.push(folder.clone());
                found.pages.push(path);
            } else if name == INDEX_FILE {
                found.indexes.push(path);
            } else if name != LOG_FILE && is_markdown(&name) {
                found.pages.push(path);
            }
        }
    }
    found.nodes.sort();
    found.indexes.sort();
    found.pages.sort();
    found
}

/// Whether a file named `name` is a markdown page.
fn is_markdown(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_starts_at_the_nearest_declared_index_or_the_parent() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let outer = folder.path().join("outer");
        let program = outer.join("group").join("add");
        std::fs::create_dir_all(&program).expect("folders");
        let parent = bundle_root(&program);
        assert_eq!(parent.path, outer.join("group"));
        assert!(!parent.declared);
        assert_eq!(parent.refused, None);
        std::fs::write(
            outer.join(INDEX_FILE),
            "---\nokf_version: \"0.2\"\n---\n# Programs\n",
        )
        .expect("an index");
        let declared = bundle_root(&program);
        assert_eq!(declared.path, outer);
        assert!(declared.declared);
        // An index with no version declares nothing.
        std::fs::write(outer.join(INDEX_FILE), "# Programs\n").expect("an index");
        assert_eq!(bundle_root(&program).path, outer.join("group"));
        // And lengua leaves one alone in the parent it falls back to.
        std::fs::write(outer.join("group").join(INDEX_FILE), "# Home\n").expect("an index");
        let foreign = bundle_root(&program);
        assert_eq!(foreign.path, outer.join("group"));
        assert_eq!(foreign.refused, Some(FOREIGN_INDEX));
    }

    #[test]
    fn lengua_writes_nothing_in_a_file_systems_root_or_the_temporary_folder() {
        let temporary = std::env::temp_dir();
        let top = temporary
            .ancestors()
            .last()
            .unwrap_or(&temporary)
            .to_path_buf();
        assert_eq!(
            bundle_root(&top.join("lengua-program")).refused,
            Some("a file system's root")
        );
        assert_eq!(
            bundle_root(&temporary.join("lengua-program")).refused,
            Some("the system's temporary folder")
        );
    }

    #[test]
    fn the_walk_skips_hidden_built_and_linked_folders() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let root = folder.path();
        for name in ["add", ".hidden/x", "target/y", "node_modules/z", "deep/er"] {
            std::fs::create_dir_all(root.join(name)).expect("a folder");
            std::fs::write(root.join(name).join(IDEA_FILE), "---\n---\n").expect("an idea");
        }
        std::fs::write(root.join("notes.md"), "notes").expect("a page");
        std::fs::write(root.join(INDEX_FILE), "# Programs\n").expect("an index");
        std::fs::write(root.join(LOG_FILE), "# Update log\n").expect("a log");
        let found = walk(root, Bounds::default());
        assert_eq!(found.nodes, [root.join("add"), root.join("deep/er")]);
        assert_eq!(found.indexes, [root.join(INDEX_FILE)]);
        assert_eq!(
            found.pages,
            [
                root.join("add").join(IDEA_FILE),
                root.join("deep/er").join(IDEA_FILE),
                root.join("notes.md"),
            ]
        );
        assert_eq!(found.stopped, None);
        let shallow = walk(
            root,
            Bounds {
                depth: 1,
                ..Bounds::default()
            },
        );
        assert_eq!(shallow.nodes, [root.join("add")]);
        assert_eq!(shallow.stopped, Some(Stop::Depth(1)));
        let few = walk(
            root,
            Bounds {
                entries: 3,
                ..Bounds::default()
            },
        );
        assert_eq!(few.stopped, Some(Stop::Entries(3)));
    }
}
