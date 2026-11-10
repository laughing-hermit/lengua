//! The knowledge graph of program folders. Each folder that holds an idea.md is a node, and the
//! idea's `- Builds on:` links and its plain markdown links to other ideas are the edges.
//!
//! A bundle's root keeps the graph's `index.md` (OKF v0.2 §8) and `log.md` (OKF §9): the nearest
//! folder above a program whose index.md declares `okf_version`, or else the program's parent.
//! `check` builds the graph on every call and says how it stands; it writes both files after it
//! records an idea, once a program is done, and when a proof finds a program wrong. The files
//! are the graph: nothing else holds it.

mod health;
pub mod okf;
pub mod scan;
pub mod write;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::data_dir::DataDir;
use crate::lock::{self, Key};
use crate::state::{CODE_FILE, IDEA_FILE};

pub use okf::Day;
pub use scan::{Bounds, Root, Stop};
pub use write::{Event, Written, write};

/// The most findings a reply lists; it counts the rest.
const MAX_FINDINGS: usize = 12;

/// A link from an idea to a file, as the graph follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The line of the idea it's on.
    pub line: usize,
    /// Where it leads, as written.
    pub target: String,
    /// The path it leads to, `..` resolved; `None` for a web address or the like.
    pub path: Option<PathBuf>,
}

/// What changed in a program folder since its lengua.lock recorded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stale {
    /// Its idea.md.
    Idea,
    /// Its code.
    Code,
}

/// A node of the graph: a program folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The folder.
    pub folder: PathBuf,
    /// The folder from the bundle's root, with `/` between names; empty for the root itself.
    pub path: String,
    /// Its idea.md.
    pub idea: PathBuf,
    /// Its code file's name, [`CODE_FILE`], when it holds one.
    pub code: Option<&'static str>,
    /// The idea's title, or the folder's name when it has none.
    pub title: String,
    /// The idea's description, when it has one.
    pub description: Option<String>,
    /// The idea's `- Builds on:` links.
    pub parts: Vec<Link>,
    /// The idea's other links.
    pub links: Vec<Link>,
    /// What changed since its lengua.lock recorded it, when something did.
    pub stale: Option<Stale>,
}

/// The graph of one bundle's program folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    /// The bundle's root.
    pub root: Root,
    /// The nodes, in the order of their paths.
    pub nodes: Vec<Node>,
    /// The bundle's index files.
    pub indexes: Vec<PathBuf>,
    /// The pages with no frontmatter `type`, but index.md and log.md, which need none.
    pub untyped: Vec<PathBuf>,
    /// The nodes some index file lists.
    pub listed: BTreeSet<PathBuf>,
    /// Why the walk stopped early, when it did.
    pub stopped: Option<Stop>,
    /// Each node's place in `nodes`, by its folder.
    places: BTreeMap<PathBuf, usize>,
}

/// How much a finding about the graph matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// It keeps lengua from recording the program.
    Blocks,
    /// Something to fix, which stops nothing.
    Warning,
    /// Something to know.
    Note,
}

impl Severity {
    /// Its word at the start of a reply's line.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// One finding about the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// How much it matters.
    pub severity: Severity,
    /// The program folder it's about, when it's about one.
    pub folder: Option<PathBuf>,
    /// What it is, and what to do about it.
    pub message: String,
}

/// How the graph stands, seen from one program folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Health {
    /// Every finding, the most serious first, and the folder's own before the rest.
    pub findings: Vec<Finding>,
}

impl Graph {
    /// The graph of the bundle that `folder` belongs to, within the walk's usual bounds.
    #[must_use]
    pub fn build(folder: &Path, data_dir: &DataDir) -> Self {
        Self::build_within(folder, data_dir, Bounds::default())
    }

    /// [`Graph::build`], within `bounds`.
    #[must_use]
    pub fn build_within(folder: &Path, data_dir: &DataDir, bounds: Bounds) -> Self {
        Self::of(scan::bundle_root(folder), folder, data_dir, bounds)
    }

    /// The graph of the bundle at `root`, with `folder` among its nodes when it holds an idea:
    /// a walk that stopped early still has the program it was asked about.
    fn of(root: Root, folder: &Path, data_dir: &DataDir, bounds: Bounds) -> Self {
        let mut found = scan::walk(&root.path, bounds);
        if folder.join(IDEA_FILE).is_file() {
            found.nodes.push(folder.to_path_buf());
            found.nodes.sort();
            found.nodes.dedup();
        }
        let key = Key::load(data_dir).ok().flatten();
        let mut nodes: Vec<Node> = found
            .nodes
            .iter()
            .map(|node| Node::read(&root.path, node, key.as_ref()))
            .collect();
        nodes.sort_by_cached_key(|node| node.path.clone());
        let places = nodes
            .iter()
            .enumerate()
            .map(|(place, node)| (node.folder.clone(), place))
            .collect();
        let untyped = found
            .pages
            .into_iter()
            .filter(|page| !typed(page))
            .collect();
        let mut graph = Self {
            root,
            nodes,
            indexes: found.indexes,
            untyped,
            listed: BTreeSet::new(),
            stopped: found.stopped,
            places,
        };
        graph.listed = graph.listed_nodes();
        graph
    }

    /// The nodes that some index file of the bundle links to.
    fn listed_nodes(&self) -> BTreeSet<PathBuf> {
        let mut listed = BTreeSet::new();
        for index in &self.indexes {
            let (Some(base), Ok(Some(text))) = (index.parent(), crate::state::read_text(index)) else {
                continue;
            };
            listed.extend(
                text.lines()
                    .flat_map(okf::links)
                    .filter_map(|(_, target)| resolve(&self.root.path, base, &target))
                    .filter_map(|path| self.node_at(&path))
                    .map(|node| node.folder.clone()),
            );
        }
        listed
    }

    /// The node that `path` leads to: its folder, its idea or its code.
    #[must_use]
    pub fn node_at(&self, path: &Path) -> Option<&Node> {
        let place = self
            .places
            .get(path)
            .or_else(|| self.places.get(folder_of(path)))?;
        self.nodes.get(*place)
    }

    /// The folders `node` builds on that are nodes of the graph.
    fn parts_of(&self, node: &Node) -> Vec<PathBuf> {
        node.parts
            .iter()
            .filter_map(|part| part.path.as_deref())
            .filter_map(|path| self.node_at(path))
            .map(|part| part.folder.clone())
            .collect()
    }

    /// How many links lead from one node to another, each pair counted once.
    #[must_use]
    pub fn edges(&self) -> usize {
        let mut pairs = BTreeSet::new();
        for node in &self.nodes {
            for link in node.parts.iter().chain(&node.links) {
                if let Some(to) = link.path.as_deref().and_then(|path| self.node_at(path))
                    && to.folder != node.folder
                {
                    pairs.insert((&node.folder, &to.folder));
                }
            }
        }
        pairs.len()
    }

    /// A folder or file of the bundle as a reply names it: from the root, with `/` between
    /// names, or whole when it lies outside.
    #[must_use]
    pub fn shown(&self, path: &Path) -> String {
        match relative(&self.root.path, path) {
            Some(shown) if !shown.is_empty() => shown,
            _ => path.display().to_string(),
        }
    }

    /// How the graph stands, seen from `folder`.
    #[must_use]
    pub fn health(&self, folder: &Path) -> Health {
        health::of(self, folder)
    }

    /// The lines of a reply about the graph: the bundle, what was just written, and the
    /// findings, the most serious first.
    #[must_use]
    pub fn lines(&self, folder: &Path, written: Option<&Written>) -> Vec<String> {
        let mut lines = vec![self.summary()];
        if let Some(written) = written {
            lines.extend(written.lines());
        }
        let findings = self.health(folder).findings;
        lines.extend(
            findings
                .iter()
                .take(MAX_FINDINGS)
                .map(|finding| format!("{}: {}", finding.severity.word(), finding.message)),
        );
        if findings.len() > MAX_FINDINGS {
            lines.push(format!("…and {} more", findings.len() - MAX_FINDINGS));
        }
        lines
    }

    /// The bundle in a line: where it is, its programs and their links.
    fn summary(&self) -> String {
        let root = self.root.path.display();
        let holds = format!(
            "holds {}, with {} between them",
            counted(self.nodes.len(), "program"),
            counted(self.edges(), "link")
        );
        match self.root.refused {
            Some(place) => format!(
                "The bundle at {root}, {place}, {holds}; lengua writes no index.md or log.md there."
            ),
            None => format!("The bundle at {root} {holds}."),
        }
    }
}

impl Node {
    /// Reads the node at `folder` of the bundle at `root`, its lock checked with `key`.
    fn read(root: &Path, folder: &Path, key: Option<&Key>) -> Self {
        let idea = folder.join(IDEA_FILE);
        let bytes = crate::state::read_file(&idea)
            .ok()
            .flatten()
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let front = okf::frontmatter(&text).unwrap_or_default();
        let code = folder.join(CODE_FILE).is_file().then_some(CODE_FILE);
        let links = okf::idea_links(&text);
        let follow = |(line, target): (usize, String)| Link {
            path: resolve(root, folder, &target),
            line,
            target,
        };
        Self {
            path: relative(root, folder).unwrap_or_default(),
            title: filled(front.title).unwrap_or_else(|| name_of(folder)),
            description: filled(front.description),
            parts: links.parts.into_iter().map(follow).collect(),
            links: links.links.into_iter().map(follow).collect(),
            stale: stale(folder, &bytes, code, key),
            folder: folder.to_path_buf(),
            idea,
            code,
        }
    }
}

/// Where `target`, a link in a file of the folder `from`, leads: a path of this computer with
/// `..` resolved, or `None` for a web address or anything else outside its files. A target
/// that starts with `/` starts at the bundle's root, as OKF's links do.
#[must_use]
pub fn resolve(root: &Path, from: &Path, target: &str) -> Option<PathBuf> {
    let target = target.split(['#', '?']).next().unwrap_or_default();
    if target.is_empty() || target.starts_with("//") || target.contains('\\') || has_scheme(target)
    {
        return None;
    }
    let (mut path, rest) = match target.strip_prefix('/') {
        Some(rest) => (root.to_path_buf(), rest),
        None => (from.to_path_buf(), target),
    };
    for segment in rest.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if !path.pop() {
                    return None;
                }
            }
            name => path.push(name),
        }
    }
    Some(path)
}

/// Whether `target` starts with a scheme, as `https:` or `mailto:` do.
fn has_scheme(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|first: char| first.is_ascii_alphabetic())
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    })
}

/// The program folder a path names: the folder of an idea or a code file, or the path itself.
fn folder_of(path: &Path) -> &Path {
    let names_a_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == IDEA_FILE || name == CODE_FILE);
    match path.parent() {
        Some(folder) if names_a_file => folder,
        _ => path,
    }
}

/// `path` from `root`, with `/` between names: empty for `root` itself, `None` outside it.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    Some(
        rest.components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// A folder's own name.
fn name_of(folder: &Path) -> String {
    folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// A frontmatter value trimmed, unless it's empty.
fn filled(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Whether the page at `path` has a frontmatter `type`.
fn typed(path: &Path) -> bool {
    scan::head(path)
        .and_then(|text| okf::frontmatter(&text))
        .and_then(|front| filled(front.kind))
        .is_some()
}

/// What changed in `folder` since its lengua.lock recorded it, as `key` reads the lock: its idea,
/// `idea` now, but for its trust record, or its code.
fn stale(folder: &Path, idea: &[u8], code: Option<&str>, key: Option<&Key>) -> Option<Stale> {
    let recorded = lock::read(folder, key).ok().flatten()?;
    if recorded.idea.sha256 != crate::trust::idea_hash(idea) {
        return Some(Stale::Idea);
    }
    let stamp = recorded.code?;
    let name = code?;
    let bytes = crate::state::read_file(&folder.join(name)).ok()??;
    (stamp.sha256 != crate::hash::Sha256Hex::of(&bytes)).then_some(Stale::Code)
}

/// `count` things, in the plural unless there's one.
fn counted(count: usize, thing: &str) -> String {
    if count == 1 {
        format!("1 {thing}")
    } else {
        format!("{count} {thing}s")
    }
}

/// Why `folder` can't be recorded when its `- Builds on:` links lead back to it, read from the
/// ideas themselves: the cycle, in words, and how to break it.
#[must_use]
pub fn builds_on_cycle(folder: &Path) -> Option<String> {
    let root = scan::bundle_root(folder).path;
    let parts_of = |at: &Path| -> Vec<PathBuf> {
        let text = crate::state::read_text(&at.join(IDEA_FILE))
            .ok()
            .flatten()
            .unwrap_or_default();
        okf::idea_links(&text)
            .parts
            .into_iter()
            .filter_map(|(_, target)| resolve(&root, at, &target))
            .map(|path| folder_of(&path).to_path_buf())
            .collect()
    };
    let cycle = health::cycle_through(folder, parts_of)?;
    let names: Vec<String> = cycle
        .iter()
        .map(|at| match relative(&root, at) {
            Some(name) if !name.is_empty() => name,
            _ => name_of(at),
        })
        .collect();
    Some(format!(
        "the `- Builds on:` links make a cycle: {}. A program can't build on itself, even \
         through others: drop one of these links",
        health::chain(&names)
    ))
}

/// The graph pass of a check of `folder`: it writes the bundle's index.md and log.md for
/// `events`, the check's doings, when there are any, and returns the reply's lines.
#[must_use]
pub fn pass(data_dir: &DataDir, folder: &Path, events: &[Event], day: Day) -> Vec<String> {
    if events.is_empty() {
        return Graph::build(folder, data_dir).lines(folder, None);
    }
    match write(data_dir, folder, events, day) {
        Ok((graph, written)) => graph.lines(folder, Some(&written)),
        Err(error) => {
            let mut lines = Graph::build(folder, data_dir).lines(folder, None);
            let problem = format!("lengua couldn't write the bundle's index.md or log.md: {error}");
            lines.insert(lines.len().min(1), problem);
            lines
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_resolve_from_their_idea_or_the_bundles_root() {
        let root = Path::new("bundle");
        let from = root.join("add");
        assert_eq!(
            resolve(root, &from, "../sort/idea.md#b1"),
            Some(root.join("sort").join("idea.md"))
        );
        assert_eq!(
            resolve(root, &from, "/tools/./x/"),
            Some(root.join("tools").join("x"))
        );
        for outside in [
            "https://example.com/idea.md",
            "mailto:me@example.com",
            "#b2",
            "",
        ] {
            assert_eq!(resolve(root, &from, outside), None, "{outside}");
        }
        assert_eq!(folder_of(&from.join("idea.md")), from.as_path());
        assert_eq!(folder_of(&from.join("main.wat")), from.as_path());
        assert_eq!(folder_of(&from), from.as_path());
    }
}
