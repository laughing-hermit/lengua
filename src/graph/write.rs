//! Writing a bundle's index.md and log.md. lengua keeps only its own entries in the index, one
//! per program, and every other line byte for byte; it adds the log's entries newest first. Each
//! file is written whole or not at all, by one writer at a time: a mutex per bundle in this
//! process, and a lock file in lengua's data folder between processes.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use super::okf::{self, Change, Day, INDEX_FILE, INDEX_HEADING, LOG_FILE, OKF_VERSION};
use super::{Bounds, Graph, Node, resolve, scan};
use crate::data_dir::DataDir;
use crate::idea::folder_of_link;

/// What a check did to a program that the bundle's log records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// It recorded the program's idea.
    Recorded,
    /// It said done: the program passed every test.
    Done,
    /// A proof found inputs where the program's code is wrong.
    ProvenWrong,
}

impl Event {
    /// What the log says of it.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Done => "checked: done",
            Self::ProvenWrong => "checked: proven wrong",
        }
    }
}

/// What a write changed in the bundle's files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    /// Whether lengua made index.md.
    pub made: bool,
    /// The programs index.md lists now and didn't before, by their paths.
    pub listed: Vec<String>,
    /// The programs whose entries changed, by their paths.
    pub changed: Vec<String>,
    /// What the entries index.md no longer has linked to: ideas that aren't there any more.
    pub dropped: Vec<String>,
    /// What log.md says now that it didn't, one entry each, in words.
    pub logged: Vec<String>,
}

impl Written {
    /// A reply's lines about it.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.made {
            lines.push(format!("lengua made index.md, listing {}", names(&self.listed)));
        } else {
            if !self.listed.is_empty() {
                lines.push(format!("index.md lists {} now", names(&self.listed)));
            }
            if !self.changed.is_empty() {
                lines.push(format!(
                    "index.md's entries for {} changed",
                    names(&self.changed)
                ));
            }
            if !self.dropped.is_empty() {
                lines.push(format!(
                    "index.md no longer lists {}, which isn't there",
                    names(&self.dropped)
                ));
            }
        }
        lines.extend(self.logged.iter().map(|entry| format!("log.md: {entry}")));
        lines
    }
}

/// Some names, a few of them in full.
fn names(names: &[String]) -> String {
    const SHOWN: usize = 5;
    match names.len() {
        0 => "nothing".to_owned(),
        count if count <= SHOWN => names.join(", "),
        count => format!("{} and {} more", names[..SHOWN].join(", "), count - SHOWN),
    }
}

/// Writes the index.md and log.md of the bundle that `folder` belongs to, for `events`, what
/// the check did to `folder`'s program on `day`. Returns the graph as it stands then, with what
/// changed; where the bundle's root refuses the files, it writes nothing.
///
/// # Errors
///
/// Fails when a file can't be read or written, or the lock can't be taken.
pub fn write(
    data_dir: &DataDir,
    folder: &Path,
    events: &[Event],
    day: Day,
) -> Result<(Graph, Written), String> {
    write_within(data_dir, folder, events, day, Bounds::default())
}

/// [`write`], its walk within `bounds`.
///
/// # Errors
///
/// As [`write`].
pub fn write_within(
    data_dir: &DataDir,
    folder: &Path,
    events: &[Event],
    day: Day,
    bounds: Bounds,
) -> Result<(Graph, Written), String> {
    let root = scan::bundle_root(folder);
    if root.refused.is_some() {
        return Ok((Graph::of(root, folder, data_dir, bounds), Written::default()));
    }
    let key = std::fs::canonicalize(&root.path).unwrap_or_else(|_| root.path.clone());
    let writers = bundle_mutex(&key);
    let _one_at_a_time = writers.lock().unwrap_or_else(PoisonError::into_inner);
    let _across_processes = lock_file(data_dir, &key)
        .map_err(|error| format!("couldn't take the lock on the bundle's files: {error}"))?;
    let graph = Graph::of(root, folder, data_dir, bounds);
    let index_path = graph.root.path.join(INDEX_FILE);
    let before = read_optional(&index_path)?;
    let (index, mut written) = kept(before.as_deref(), &graph);
    if before.as_deref() != Some(index.as_str()) {
        replace(&index_path, &index)?;
    }
    if let Some(node) = graph.node_at(folder) {
        let log_path = graph.root.path.join(LOG_FILE);
        let mut log = read_optional(&log_path)?;
        let mut change = if written.listed.contains(&node.path) {
            Change::Creation
        } else {
            Change::Update
        };
        for event in events {
            let entry = okf::log_entry(change, &node.title, &node.path, event.words());
            if let Some(text) = okf::logged(log.as_deref(), day, &entry) {
                log = Some(text);
                written
                    .logged
                    .push(format!("{} {}", node.title, event.words()));
            }
            change = Change::Update;
        }
        if !written.logged.is_empty()
            && let Some(text) = &log
        {
            replace(&log_path, text)?;
        }
    }
    Ok((graph, written))
}

/// The index lengua would write for `graph`, from the index there was, `existing`: what
/// [`write`] puts in index.md.
#[must_use]
pub fn index_text(existing: Option<&str>, graph: &Graph) -> String {
    kept(existing, graph).0
}

/// What a line of an index is to lengua.
enum Line<'graph> {
    /// Not one of lengua's entries: it stays as it is.
    Other,
    /// The entry of a node of the graph.
    Node(&'graph Node),
    /// The entry of an idea that's there but not in the graph, sorted as the folder it names:
    /// it stays as it is.
    Elsewhere(String),
    /// The entry of an idea that isn't there any more, which it linked to: it goes.
    Gone(String),
}

/// What `line` of the bundle's root index is to lengua.
fn line_of<'graph>(graph: &'graph Graph, line: &str) -> Line<'graph> {
    let Some(target) = okf::entry_target(line.trim_end()) else {
        return Line::Other;
    };
    let Some(path) = resolve(&graph.root.path, &graph.root.path, &target) else {
        return Line::Other;
    };
    match graph.node_at(&path) {
        Some(node) => Line::Node(node),
        None if path.is_file() => Line::Elsewhere(folder_of_link(&target)),
        None => Line::Gone(target),
    }
}

/// The text of the index with lengua's entries kept, and what changed. A new index declares
/// the bundle, with `okf_version`; in an index there was, each run of lengua's entries keeps its
/// place, sorted by folder, the first run takes the programs no entry lists yet, and every
/// other line stays byte for byte.
fn kept(existing: Option<&str>, graph: &Graph) -> (String, Written) {
    let entries: Vec<(&Node, String)> = graph
        .nodes
        .iter()
        .map(|node| (node, entry_of(node)))
        .collect();
    let Some(existing) = existing else {
        let mut text = format!("---\nokf_version: \"{OKF_VERSION}\"\n---\n{INDEX_HEADING}\n");
        if !entries.is_empty() {
            text.push('\n');
        }
        for (_, entry) in &entries {
            text.push_str(entry);
            text.push('\n');
        }
        let listed = entries.iter().map(|(node, _)| node.path.clone()).collect();
        return (
            text,
            Written {
                made: true,
                listed,
                ..Written::default()
            },
        );
    };
    let lines: Vec<&str> = existing.split_inclusive('\n').collect();
    let kinds: Vec<Line<'_>> = lines.iter().map(|line| line_of(graph, line)).collect();
    let mut written = Written::default();
    let present: BTreeSet<&Path> = kinds
        .iter()
        .filter_map(|kind| match kind {
            Line::Node(node) => Some(node.folder.as_path()),
            Line::Other | Line::Elsewhere(_) | Line::Gone(_) => None,
        })
        .collect();
    let mut placed: BTreeSet<&Path> = BTreeSet::new();
    let mut text = String::with_capacity(existing.len() + 256);
    let mut first = true;
    let mut at = 0;
    while at < lines.len() {
        if matches!(kinds[at], Line::Other) {
            text.push_str(lines[at]);
            at += 1;
            continue;
        }
        let end = (at..lines.len())
            .find(|&next| matches!(kinds[next], Line::Other))
            .unwrap_or(lines.len());
        let mut run: Vec<(String, String)> = Vec::new();
        for (line, kind) in lines[at..end].iter().zip(&kinds[at..end]) {
            match kind {
                Line::Node(node) => {
                    if placed.insert(node.folder.as_path()) {
                        let entry = entry_of(node);
                        if line.trim_end() != entry {
                            written.changed.push(node.path.clone());
                        }
                        run.push((node.path.clone(), entry));
                    }
                }
                Line::Elsewhere(folder) => run.push((folder.clone(), line.trim_end().to_owned())),
                Line::Gone(target) => written.dropped.push(target.clone()),
                Line::Other => {}
            }
        }
        if first {
            first = false;
            for (node, entry) in &entries {
                if !present.contains(node.folder.as_path())
                    && placed.insert(node.folder.as_path())
                {
                    written.listed.push(node.path.clone());
                    run.push((node.path.clone(), entry.clone()));
                }
            }
        }
        run.sort();
        for (_, line) in run {
            text.push_str(&line);
            text.push('\n');
        }
        at = end;
    }
    if first && !entries.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        if !text.is_empty() && !text.ends_with("\n\n") {
            text.push('\n');
        }
        for (node, entry) in &entries {
            written.listed.push(node.path.clone());
            text.push_str(entry);
            text.push('\n');
        }
    }
    (text, written)
}

/// The entry lengua keeps for `node` in the bundle's root index.
fn entry_of(node: &Node) -> String {
    okf::index_entry(
        &node.title,
        &node.path,
        node.description.as_deref(),
        node.code,
    )
}

/// The writers in this process of each bundle, by its root.
type Writers = BTreeMap<PathBuf, Arc<Mutex<()>>>;

/// The mutex this process's writers of the bundle at `root` share.
fn bundle_mutex(root: &Path) -> Arc<Mutex<()>> {
    static WRITERS: OnceLock<Mutex<Writers>> = OnceLock::new();
    let mut writers = WRITERS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    Arc::clone(writers.entry(root.to_path_buf()).or_default())
}

/// Takes the lock that keeps lengua's processes from writing the bundle at `root` at once: a
/// file in the data folder, never beside the user's files, released when it closes.
fn lock_file(data_dir: &DataDir, root: &Path) -> io::Result<std::fs::File> {
    let path = data_dir.graph_lock(root);
    if let Some(folder) = path.parent() {
        crate::lock::create_private_dir(folder)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

/// A file's text, or `None` when there's no file.
fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("couldn't read {}: {error}", path.display())),
    }
}

/// Writes `text` to `path` whole or not at all: into a hidden file beside it, renamed over it.
fn replace(path: &Path, text: &str) -> Result<(), String> {
    static SCRATCHES: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let scratch = path.with_file_name(format!(
        ".{name}.{}.{}.lengua",
        std::process::id(),
        SCRATCHES.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&scratch)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        rename(&scratch, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(scratch.as_path());
    }
    written.map_err(|error| format!("couldn't write {}: {error}", path.display()))
}

/// Renames `from` over `to`, trying again a few times when the system refuses for a moment, as
/// Windows does while another program, such as a virus scanner, has the file open.
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut tries = 0_u32;
    loop {
        match std::fs::rename(from, to) {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied && tries < 5 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20 << tries));
            }
            done => return done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_few_in_full() {
        let some = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(names(&some), "a, b");
        let many: Vec<String> = (1..=7).map(|n| format!("p{n}")).collect();
        assert_eq!(names(&many), "p1, p2, p3, p4, p5 and 2 more");
        assert_eq!(names(&[]), "nothing");
        assert_eq!(Event::ProvenWrong.words(), "checked: proven wrong");
    }
}
