//! How a graph stands: cycles of `- Builds on:` links, links that lead nowhere, programs not
//! built yet or changed since their lock, orphans, pages with no type, and a walk that stopped
//! early.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use super::{Finding, Graph, Health, Node, Severity, Stale, Stop};
use crate::state::{CODE_FILE, CODE_FILES, IDEA_FILE};

/// The most folders a search for a cycle goes through.
const MAX_VISITS: usize = 20_000;

/// How `graph` stands, seen from the program folder `folder`.
pub(super) fn of(graph: &Graph, folder: &Path) -> Health {
    let mut findings = Vec::new();
    cycles(graph, folder, &mut findings);
    let linked = linked(graph);
    for node in &graph.nodes {
        about(graph, node, &linked, &mut findings);
    }
    for page in &graph.untyped {
        findings.push(Finding {
            severity: Severity::Warning,
            folder: None,
            message: format!(
                "{} has no frontmatter `type`, which every page of an Open Knowledge Format \
                 bundle but index.md and log.md needs: add one, such as `type: note`",
                graph.shown(page)
            ),
        });
    }
    if let Some(stop) = graph.stopped {
        let message = match stop {
            Stop::Entries(entries) => format!(
                "the walk through the bundle stopped after {entries} entries, so the graph may \
                 lack programs past them"
            ),
            Stop::Depth(depth) => format!(
                "the walk through the bundle goes {depth} folders deep, so the graph lacks the \
                 programs deeper down"
            ),
        };
        findings.push(Finding {
            severity: Severity::Note,
            folder: None,
            message,
        });
    }
    findings.sort_by_key(|finding| {
        (
            finding.severity,
            finding.folder.as_deref() != Some(folder),
            finding.folder.clone(),
        )
    });
    Health { findings }
}

/// What there is to say of one node: links that lead nowhere, no code yet, a stale lock, and
/// no index or idea that leads to it.
fn about(graph: &Graph, node: &Node, linked: &BTreeSet<PathBuf>, findings: &mut Vec<Finding>) {
    let shown = graph.shown(&node.folder);
    let mut say = |severity, message| {
        findings.push(Finding {
            severity,
            folder: Some(node.folder.clone()),
            message,
        });
    };
    for link in node.parts.iter().chain(&node.links) {
        if let Some(path) = &link.path
            && !path.exists()
        {
            say(
                Severity::Warning,
                format!(
                    "{shown}/{IDEA_FILE} line {} links to {}, which isn't there",
                    link.line, link.target
                ),
            );
        }
    }
    if node.code.is_none() {
        say(
            Severity::Note,
            format!(
                "{shown} is not built yet: it has no {}",
                CODE_FILES.join(" or ")
            ),
        );
    }
    if let Some(changed) = node.stale {
        let file = match changed {
            Stale::Idea => IDEA_FILE,
            Stale::Code => node.code.unwrap_or(CODE_FILE),
        };
        say(
            Severity::Note,
            format!(
                "{shown} is stale: its {file} changed since its lengua.lock recorded it; call \
                 check with path {}",
                node.folder.display()
            ),
        );
    }
    if !graph.listed.contains(&node.folder) && !linked.contains(&node.folder) {
        let fix = if graph.root.refused.is_some() {
            "list it in an index.md yourself, since lengua writes none here"
        } else {
            "lengua lists it in index.md when it next writes there"
        };
        say(
            Severity::Note,
            format!(
                "{shown} is an orphan: no index.md lists it, and no idea links to it; {fix}"
            ),
        );
    }
}

/// The nodes that another node's idea links to.
fn linked(graph: &Graph) -> BTreeSet<PathBuf> {
    let mut linked = BTreeSet::new();
    for node in &graph.nodes {
        for link in node.parts.iter().chain(&node.links) {
            if let Some(to) = link.path.as_deref().and_then(|path| graph.node_at(path))
                && to.folder != node.folder
            {
                linked.insert(to.folder.clone());
            }
        }
    }
    linked
}

/// Each cycle of `- Builds on:` links, once: it blocks `folder` when `folder` is in it, and is a
/// warning for every other program.
fn cycles(graph: &Graph, folder: &Path, findings: &mut Vec<Finding>) {
    let parts_of = |at: &Path| -> Vec<PathBuf> {
        graph
            .node_at(at)
            .map(|node| graph.parts_of(node))
            .unwrap_or_default()
    };
    let mut covered = BTreeSet::new();
    let shown = |cycle: &[PathBuf]| -> String {
        let names: Vec<String> = cycle.iter().map(|at| graph.shown(at)).collect();
        chain(&names)
    };
    if let Some(cycle) = cycle_through(folder, parts_of) {
        findings.push(Finding {
            severity: Severity::Blocks,
            folder: Some(folder.to_path_buf()),
            message: format!(
                "{}: a program can't build on itself, even through others, so lengua won't \
                 record it until one of these `- Builds on:` links goes",
                shown(&cycle)
            ),
        });
        covered.extend(cycle);
    }
    for node in &graph.nodes {
        if covered.contains(&node.folder) {
            continue;
        }
        if let Some(cycle) = cycle_through(&node.folder, parts_of) {
            findings.push(Finding {
                severity: Severity::Warning,
                folder: Some(node.folder.clone()),
                message: format!(
                    "{}: a cycle of `- Builds on:` links, which blocks those programs only",
                    shown(&cycle)
                ),
            });
            covered.extend(cycle);
        }
    }
}

/// The shortest path of `- Builds on:` links from `start` back to itself, when there is one:
/// `start`, the folders on the way, and `start` again. `parts_of` gives a folder's parts.
pub(super) fn cycle_through(
    start: &Path,
    mut parts_of: impl FnMut(&Path) -> Vec<PathBuf>,
) -> Option<Vec<PathBuf>> {
    let mut came_from: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut folders = VecDeque::from([start.to_path_buf()]);
    while let Some(folder) = folders.pop_front() {
        for part in parts_of(&folder) {
            if part == start {
                let mut path = vec![folder.clone()];
                let mut at = folder.clone();
                while at != start {
                    let Some(before) = came_from.get(&at) else {
                        break;
                    };
                    path.push(before.clone());
                    at = before.clone();
                }
                path.reverse();
                path.push(start.to_path_buf());
                return Some(path);
            }
            if came_from.len() < MAX_VISITS
                && let Entry::Vacant(entry) = came_from.entry(part.clone())
            {
                entry.insert(folder.clone());
                folders.push_back(part);
            }
        }
    }
    None
}

/// A cycle of programs in words: `a builds on b, which builds on a`.
pub(super) fn chain(names: &[String]) -> String {
    match names {
        [one, other] if one == other => format!("{one} builds on itself"),
        [first, rest @ ..] => {
            let mut text = first.clone();
            for (index, name) in rest.iter().enumerate() {
                text.push_str(if index == 0 {
                    " builds on "
                } else {
                    ", which builds on "
                });
                text.push_str(name);
            }
            text
        }
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_shortest_cycle_back() {
        let edges: BTreeMap<&str, Vec<&str>> = BTreeMap::from([
            ("a", vec!["b", "c"]),
            ("b", vec!["d"]),
            ("c", vec!["a"]),
            ("d", vec!["b"]),
            ("e", vec!["e"]),
        ]);
        let parts_of = |at: &Path| -> Vec<PathBuf> {
            let name = at.to_str().unwrap_or_default();
            edges
                .get(name)
                .map(|parts| parts.iter().map(PathBuf::from).collect())
                .unwrap_or_default()
        };
        let names = |start: &str| -> Option<Vec<String>> {
            cycle_through(Path::new(start), parts_of).map(|cycle| {
                cycle
                    .iter()
                    .map(|folder| folder.display().to_string())
                    .collect()
            })
        };
        assert_eq!(names("a").as_deref(), Some(&["a", "c", "a"].map(String::from)[..]));
        assert_eq!(names("e").as_deref(), Some(&["e", "e"].map(String::from)[..]));
        // b is in a cycle with d, not with a: from b, the cycle is b's own.
        assert_eq!(names("b").as_deref(), Some(&["b", "d", "b"].map(String::from)[..]));
        assert_eq!(names("f"), None);
        assert_eq!(
            chain(&["a", "c", "a"].map(String::from)),
            "a builds on c, which builds on a"
        );
        assert_eq!(chain(&["e", "e"].map(String::from)), "e builds on itself");
    }
}
