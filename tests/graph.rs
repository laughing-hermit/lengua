//! The knowledge graph of program folders: `check` keeps each bundle's index.md (OKF v0.2 §8) and
//! log.md (OKF §9) and says how the graph stands. Most of these tests run on every system and run
//! no program: they read and write a bundle as `check` does, through lengua's own functions.

mod common;

use std::path::{Path, PathBuf};

use common::{ADD_IDEA, Program, Protocol, Session};
use lengua::data_dir::DataDir;
use lengua::graph::{self, Bounds, Day, Event, Finding, Graph, Severity, Stale, Stop, Written};
use lengua::hash::Sha256Hex;
use lengua::lock::{self, CodeStamp, Key, Lock};
use lengua::trust::{self, Verified};
use serde_json::Value;
use tempfile::TempDir;

/// The add idea's description, as its frontmatter writes it.
const ADD_DESCRIPTION: &str =
    "description: Adds the two whole numbers in its arguments and prints the sum.";

/// A bundle of program folders made for one test, with a data folder for lengua beside it.
struct Bundle {
    folder: TempDir,
    data: TempDir,
}

impl Bundle {
    fn new() -> Self {
        Self {
            folder: tempfile::tempdir().expect("a folder for the bundle"),
            data: tempfile::tempdir().expect("a data folder"),
        }
    }

    /// The bundle's root: the folder that holds its programs.
    fn root(&self) -> &Path {
        self.folder.path()
    }

    fn data_dir(&self) -> DataDir {
        DataDir::new(self.data.path().to_path_buf())
    }

    /// A program folder of the bundle, `name` from its root, holding `idea` and, with `code`, a
    /// main.wat.
    fn program(&self, name: &str, idea: &str, code: bool) -> PathBuf {
        let folder = self.root().join(name);
        std::fs::create_dir_all(&folder).expect("a program folder");
        std::fs::write(folder.join("idea.md"), idea).expect("write idea.md");
        if code {
            std::fs::write(folder.join("main.wat"), "(module)\n").expect("main.wat");
        }
        folder
    }

    /// A file of the bundle, `name` from its root; empty when it isn't there.
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.root().join(name)).unwrap_or_default()
    }

    /// Writes the bundle's files for what `events` did to the program at `folder`, on `on`.
    fn write(&self, folder: &Path, events: &[Event], on: &str) -> (Graph, Written) {
        graph::write(&self.data_dir(), folder, events, day(on)).expect("write the bundle's files")
    }

    /// The graph the program at `folder` belongs to.
    fn graph(&self, folder: &Path) -> Graph {
        Graph::build(folder, &self.data_dir())
    }
}

/// A day of the log.
fn day(text: &str) -> Day {
    Day::parse(text).expect("a day")
}

/// The add idea for the program `title`, which `description` describes.
fn idea(title: &str, description: &str) -> String {
    ADD_IDEA
        .replace("title: add", &format!("title: {title}"))
        .replace(ADD_DESCRIPTION, &format!("description: {description}"))
}

/// `idea`, building on the parts `links` names.
fn building_on(idea: &str, links: &str) -> String {
    idea.replace(
        "- Uses: nothing",
        &format!("- Uses: nothing\n- Builds on: {links}"),
    )
}

/// The add idea, with `link` in its purpose.
fn linking(link: &str) -> String {
    ADD_IDEA.replace("# Purpose\n", &format!("# Purpose\nLike {link}.\n"))
}

/// What `folder` holds, by name, in order.
fn listing(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("a folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A reply's lines.
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn lengua_makes_the_index_with_okf_version() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    let (_, written) = bundle.write(&add, &[Event::Recorded], "2026-11-06");
    assert!(written.made, "{written:?}");
    assert_eq!(written.listed, ["add"]);
    assert_eq!(
        bundle.read("index.md"),
        "---\nokf_version: \"0.2\"\n---\n# Programs\n\n* [add](add/idea.md) - Adds the two \
         whole numbers in its arguments and prints the sum. Code: [main.wat](add/main.wat)\n"
    );
    assert_eq!(
        bundle.read("log.md"),
        "# Update log\n\n## 2026-11-06\n\n* **Creation**: [add](add/idea.md) recorded.\n"
    );
    // The index declares the bundle now, and nothing else landed anywhere: not in the program's
    // own folder, and no scratch file beside the two.
    assert!(graph::scan::bundle_root(&add).declared);
    assert_eq!(listing(&add), ["idea.md", "main.wat"]);
    assert_eq!(listing(bundle.root()), ["add", "index.md", "log.md"]);
}

#[test]
fn the_index_keeps_every_line_lengua_doesnt_own() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    bundle.program("sort", &idea("sort", "Sorts lines."), false);
    let by_hand = "---\nokf_version: \"0.2\"\nowner: us\n---\n# Our programs\n\nWritten by \
                   hand.\n\n## Tools\n\n\
                   * [the manual](https://example.com/manual) - read it first\n\
                   * [add](add/idea.md) - an old description\n\nThe end.\r\n";
    std::fs::write(bundle.root().join("index.md"), by_hand).expect("an index by hand");
    let (_, written) = bundle.write(&add, &[Event::Recorded], "2026-11-06");
    let kept = "---\nokf_version: \"0.2\"\nowner: us\n---\n# Our programs\n\nWritten by \
                hand.\n\n## Tools\n\n\
                * [the manual](https://example.com/manual) - read it first\n\
                * [add](add/idea.md) - Adds the two whole numbers in its arguments and prints \
                the sum. Code: [main.wat](add/main.wat)\n\
                * [sort](sort/idea.md) - Sorts lines.\n\nThe end.\r\n";
    assert_eq!(bundle.read("index.md"), kept);
    assert_eq!(written.changed, ["add"]);
    assert_eq!(written.listed, ["sort"]);
    assert!(!written.made);
    // An index lengua didn't make keeps its own frontmatter, and add was listed before: an update.
    assert!(
        bundle
            .read("log.md")
            .contains("* **Update**: [add](add/idea.md) recorded.\n")
    );
    let (_, again) = bundle.write(&add, &[], "2026-11-06");
    assert_eq!(again, Written::default());
    assert_eq!(bundle.read("index.md"), kept);
}

#[test]
fn an_index_without_okf_version_stays_as_it_is() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    // Such as a site's home page: it isn't lengua's, so lengua writes neither file beside it.
    let home = "# Welcome\n\nOur site.\n\n* [add](add/idea.md) - our adder\n";
    std::fs::write(bundle.root().join("index.md"), home).expect("an index of someone else's");
    let (graph, written) = bundle.write(&add, &[Event::Recorded], "2026-11-06");
    assert_eq!(written, Written::default());
    assert_eq!(bundle.read("index.md"), home);
    assert_eq!(listing(bundle.root()), ["add", "index.md"]);
    assert!(!graph.root.declared);
    let lines = graph.lines(&add, Some(&written));
    let summary = lines.first().expect("the graph's summary");
    assert!(
        summary.contains("whose index.md isn't lengua's")
            && summary.contains("okf_version")
            && summary.ends_with("lengua writes no index.md or log.md there."),
        "{lines:?}"
    );
    // The index still lists the program, so it is no orphan.
    assert!(
        !lines.iter().any(|line| line.contains("orphan")),
        "{lines:?}"
    );
    // With `okf_version` it becomes lengua's to keep.
    let declared = format!("---\nokf_version: \"0.2\"\n---\n{home}");
    std::fs::write(bundle.root().join("index.md"), &declared).expect("declare the bundle");
    let (_, written) = bundle.write(&add, &[Event::Recorded], "2026-11-06");
    assert_eq!(written.changed, ["add"]);
    assert!(
        bundle
            .read("index.md")
            .starts_with("---\nokf_version: \"0.2\"\n---\n# Welcome\n")
    );
    assert!(
        bundle
            .read("log.md")
            .contains("[add](add/idea.md) recorded.")
    );
}

#[test]
fn a_changed_description_updates_its_entry() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    bundle.write(&add, &[Event::Recorded], "2026-11-06");
    std::fs::write(add.join("idea.md"), idea("add", "Adds two numbers")).expect("edit idea.md");
    let (_, written) = bundle.write(&add, &[Event::Recorded], "2026-11-06");
    assert_eq!(written.changed, ["add"]);
    let index = bundle.read("index.md");
    assert!(
        index.ends_with(
            "# Programs\n\n* [add](add/idea.md) - Adds two numbers. Code: \
             [main.wat](add/main.wat)\n"
        ),
        "{index}"
    );
}

#[test]
fn a_deleted_programs_entry_is_removed() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    let gone = bundle.program("gone", &idea("gone", "Goes away."), true);
    bundle.write(&add, &[Event::Recorded], "2026-11-06");
    assert!(bundle.read("index.md").contains("(gone/idea.md)"));
    std::fs::remove_dir_all(gone).expect("delete a program");
    let (graph, written) = bundle.write(&add, &[], "2026-11-06");
    assert_eq!(written.dropped, ["gone/idea.md"]);
    let index = bundle.read("index.md");
    assert!(!index.contains("gone"), "{index}");
    assert!(index.contains("* [add](add/idea.md) - "), "{index}");
    assert_eq!(graph.nodes.len(), 1);
}

#[test]
fn a_broken_link_is_a_warning_and_blocks_nothing() {
    let bundle = Bundle::new();
    let text = linking("[subtract](../subtract/idea.md)");
    let add = bundle.program("add", &text, true);
    let parsed = lengua::idea::parse(&text).expect("an idea with a broken link is an idea");
    assert_eq!(parsed.links.len(), 1);
    assert_eq!(graph::builds_on_cycle(&add), None);
    let findings = bundle.graph(&add).health(&add).findings;
    let broken: Vec<&Finding> = findings
        .iter()
        .filter(|finding| finding.message.contains("which isn't there"))
        .collect();
    assert_eq!(broken.len(), 1, "{findings:#?}");
    assert_eq!(broken[0].severity, Severity::Warning);
    // The link's line is where `linking` put it, in whatever add's idea looks like.
    let line = text
        .lines()
        .position(|line| line.contains("../subtract/idea.md"))
        .expect("the link is in the idea")
        + 1;
    assert_eq!(
        broken[0].message,
        format!("add/idea.md line {line} links to ../subtract/idea.md, which isn't there")
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding.severity != Severity::Blocks),
        "{findings:#?}"
    );
}

#[test]
fn a_cycle_blocks_only_its_programs_with_its_path() {
    let bundle = Bundle::new();
    let on = |name: &str, part: &str| {
        building_on(
            &idea(name, &format!("Builds on {part}.")),
            &format!("[{part}](../{part}/idea.md)"),
        )
    };
    let a = bundle.program("a", &on("a", "b"), true);
    let b = bundle.program("b", &on("b", "a"), true);
    let c = bundle.program("c", &on("c", "a"), true);
    let cycle = graph::builds_on_cycle(&a).expect("a is in a cycle");
    assert!(
        cycle.contains("a builds on b, which builds on a"),
        "{cycle}"
    );
    let from_b = graph::builds_on_cycle(&b).unwrap_or_default();
    assert!(
        from_b.contains("b builds on a, which builds on b"),
        "{from_b}"
    );
    assert_eq!(
        graph::builds_on_cycle(&c),
        None,
        "c builds on the cycle, outside it"
    );
    let graph = bundle.graph(&c);
    let seen_from = |folder: &Path| -> Vec<Finding> {
        graph
            .health(folder)
            .findings
            .into_iter()
            .filter(|finding| finding.message.contains("builds on"))
            .collect()
    };
    let in_it = seen_from(&a);
    assert_eq!(in_it.len(), 1, "one cycle, said once: {in_it:#?}");
    assert_eq!(in_it[0].severity, Severity::Blocks);
    assert!(
        in_it[0]
            .message
            .starts_with("a builds on b, which builds on a: ")
    );
    let outside = seen_from(&c);
    assert_eq!(outside.len(), 1, "{outside:#?}");
    assert_eq!(outside[0].severity, Severity::Warning);
    assert!(
        outside[0]
            .message
            .contains("which blocks those programs only"),
        "{outside:#?}"
    );
}

#[test]
fn a_stale_program_is_reported() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    let key = Key::load_or_create(&bundle.data_dir()).expect("a signing key");
    let code = std::fs::read(add.join("main.wat")).expect("main.wat");
    let recorded = Lock {
        code: Some(CodeStamp {
            sha256: Sha256Hex::of(&code),
            module_sha256: Sha256Hex::of(b"module"),
            engine: lengua::wasm::engine::ENGINE.into(),
            max_stack_bytes: 16,
            unsafe_blocks: 0,
            features: Vec::new(),
            parts: Vec::new(),
            calls: Vec::new(),
        }),
        ..Lock::requested(
            Sha256Hex::of(ADD_IDEA.as_bytes()),
            "Make me an adder".into(),
        )
    };
    lock::write(&add, &recorded, &key).expect("write the lock");
    let stale = |graph: &Graph| graph.nodes[0].stale;
    assert_eq!(stale(&bundle.graph(&add)), None);
    // lengua's entry in `verified` is no change to the idea.
    let idea_hash = Sha256Hex::of(ADD_IDEA.as_bytes());
    let entry = Verified::new("2026-11-06T23:00:00Z", idea_hash, Sha256Hex::of(&code));
    let verified = trust::with_lengua_verified(ADD_IDEA, &entry);
    assert_ne!(verified, ADD_IDEA);
    std::fs::write(add.join("idea.md"), &verified).expect("write lengua's entry");
    assert_eq!(stale(&bundle.graph(&add)), None);
    std::fs::write(add.join("main.wat"), "(module (func))\n").expect("edit main.wat");
    assert_eq!(stale(&bundle.graph(&add)), Some(Stale::Code));
    std::fs::write(add.join("idea.md"), idea("add", "Adds.")).expect("edit idea.md");
    let graph = bundle.graph(&add);
    assert_eq!(stale(&graph), Some(Stale::Idea));
    let lines = graph.lines(&add, None);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("note: add is stale: its idea.md changed")),
        "{lines:#?}"
    );
}

#[test]
fn the_log_is_newest_first() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    bundle.write(&add, &[Event::Recorded], "2026-11-06");
    bundle.write(&add, &[Event::Done], "2026-11-06");
    bundle.write(&add, &[Event::Recorded, Event::Done], "2026-11-07");
    let (_, written) = bundle.write(&add, &[Event::Done], "2026-11-07");
    assert!(written.logged.is_empty(), "said once: {written:?}");
    assert_eq!(
        bundle.read("log.md"),
        "# Update log\n\n## 2026-11-07\n\n* **Update**: [add](add/idea.md) checked: done.\n\
         * **Update**: [add](add/idea.md) recorded.\n\n## 2026-11-06\n\n\
         * **Update**: [add](add/idea.md) checked: done.\n\
         * **Creation**: [add](add/idea.md) recorded.\n"
    );
}

#[test]
fn nothing_is_written_in_the_home_or_temporary_folder_itself() {
    // A program folder right in the system's temporary folder.
    let program = tempfile::Builder::new()
        .prefix("lengua-graph-")
        .tempdir()
        .expect("a program folder in the temporary folder");
    std::fs::write(program.path().join("idea.md"), ADD_IDEA).expect("write idea.md");
    let temporary = std::env::temp_dir();
    let files =
        || ["index.md", "log.md"].map(|name| std::fs::read_to_string(temporary.join(name)).ok());
    let before = files();
    let data = tempfile::tempdir().expect("a data folder");
    let (graph, written) = graph::write(
        &DataDir::new(data.path().to_path_buf()),
        program.path(),
        &[Event::Recorded],
        day("2026-11-06"),
    )
    .expect("writing nothing is no failure");
    assert_eq!(graph.root.refused, Some("the system's temporary folder"));
    assert_eq!(written, Written::default());
    assert_eq!(files(), before, "the temporary folder is as it was");
    let lines = graph.lines(program.path(), None);
    assert!(
        lines[0].contains("lengua writes no index.md or log.md there"),
        "{lines:#?}"
    );
    // The home folder and a file system's root take none either.
    match std::env::home_dir() {
        Some(home) => assert_eq!(
            graph::scan::bundle_root(&home.join("lengua-graph-program")).refused,
            Some("the home folder")
        ),
        None => lengua::skiplog::note("this computer names no home folder"),
    }
    let top = temporary
        .ancestors()
        .last()
        .unwrap_or(&temporary)
        .to_path_buf();
    assert_eq!(
        graph::scan::bundle_root(&top.join("lengua-graph-program")).refused,
        Some("a file system's root")
    );
}

#[test]
fn a_page_without_a_type_is_reported() {
    let bundle = Bundle::new();
    let add = bundle.program("add", ADD_IDEA, true);
    let page = |name: &str, text: &str| {
        std::fs::write(bundle.root().join(name), text).expect("a page");
    };
    page("notes.md", "# Notes\nBy hand.\n");
    page("typed.md", "---\ntype: note\n---\n# Typed\n");
    page("add/plan.md", "---\ntitle: a plan\n---\n");
    bundle.write(&add, &[Event::Recorded], "2026-11-06");
    let graph = bundle.graph(&add);
    let untyped: Vec<String> = graph.untyped.iter().map(|page| graph.shown(page)).collect();
    assert_eq!(untyped, ["add/plan.md", "notes.md"]);
    let lines = graph.lines(&add, None);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("warning: notes.md has no frontmatter `type`")),
        "{lines:#?}"
    );
}

#[test]
fn the_walk_through_a_bundle_is_bounded() {
    let bundle = Bundle::new();
    let top = bundle.program("top", ADD_IDEA, true);
    for skipped in [".hidden/x", "target/x", "node_modules/x"] {
        bundle.program(skipped, ADD_IDEA, true);
    }
    // A program nine folders below the root, one deeper than the walk goes.
    let deep = (1..=9).fold(bundle.root().to_path_buf(), |folder, level| {
        folder.join(format!("d{level}"))
    });
    std::fs::create_dir_all(&deep).expect("deep folders");
    std::fs::write(deep.join("idea.md"), ADD_IDEA).expect("a deep idea");
    let graph = bundle.graph(&top);
    assert_eq!(graph.stopped, Some(Stop::Depth(8)));
    let paths: Vec<&str> = graph.nodes.iter().map(|node| node.path.as_str()).collect();
    assert_eq!(
        paths,
        ["top"],
        "nothing hidden, built or past 8 folders down"
    );
    let lines = graph.lines(&top, None);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("goes 8 folders deep")),
        "{lines:#?}"
    );
    // The program checked is in its graph, however deep it lies.
    let declared = "---\nokf_version: \"0.2\"\n---\n# Programs\n";
    std::fs::write(bundle.root().join("index.md"), declared).expect("a declared index");
    let from_deep = bundle.graph(&deep);
    assert_eq!(from_deep.root.path, bundle.root());
    assert!(from_deep.nodes.iter().any(|node| node.folder == deep));
    // And the walk stops after so many entries.
    for number in 0..10 {
        let name = format!("file{number}.txt");
        std::fs::write(bundle.root().join(name), "").expect("a file");
    }
    let few = Bounds {
        entries: 5,
        ..Bounds::default()
    };
    let short = Graph::build_within(&top, &bundle.data_dir(), few);
    assert_eq!(short.stopped, Some(Stop::Entries(5)));
    assert!(
        short
            .lines(&top, None)
            .iter()
            .any(|line| line.contains("stopped after 5 entries")),
        "{:#?}",
        short.lines(&top, None)
    );
}

#[test]
fn two_writers_at_once_leave_a_consistent_index() {
    let bundle = Bundle::new();
    let names: Vec<String> = (1..=8).map(|number| format!("p{number}")).collect();
    let folders: Vec<PathBuf> = names
        .iter()
        .map(|name| bundle.program(name, &idea(name, &format!("Program {name}.")), true))
        .collect();
    let start = std::sync::Barrier::new(folders.len());
    std::thread::scope(|scope| {
        for folder in &folders {
            let (bundle, start) = (&bundle, &start);
            scope.spawn(move || {
                start.wait();
                bundle.write(folder, &[Event::Recorded], "2026-11-06");
            });
        }
    });
    let index = bundle.read("index.md");
    let entries: Vec<&str> = index
        .lines()
        .filter(|line| line.starts_with("* ["))
        .collect();
    let expected: Vec<String> = names
        .iter()
        .map(|name| {
            format!(
                "* [{name}]({name}/idea.md) - Program {name}. Code: [main.wat]({name}/main.wat)"
            )
        })
        .collect();
    assert_eq!(entries, expected, "{index}");
    let log = bundle.read("log.md");
    for name in &names {
        let entry = format!("[{name}]({name}/idea.md) recorded.");
        assert_eq!(log.matches(&entry).count(), 1, "{log}");
    }
    let mut left = vec!["index.md".to_owned(), "log.md".to_owned()];
    left.extend(names);
    assert_eq!(listing(bundle.root()), left, "no scratch file stays");
}

/// lengua's writer makes the committed examples/index.md, the examples' bundle, byte for byte.
#[test]
fn examples_index_is_lengua_s() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let committed = std::fs::read_to_string(examples.join("index.md")).expect("examples/index.md");
    let data = tempfile::tempdir().expect("a data folder");
    let graph = Graph::build(
        &examples.join("add"),
        &DataDir::new(data.path().to_path_buf()),
    );
    let programs = std::fs::read_dir(examples.as_path())
        .expect("the examples")
        .flatten()
        .filter(|entry| entry.path().join("idea.md").is_file())
        .count();
    assert_eq!(graph.root.path, examples);
    assert!(graph.root.declared, "examples/index.md declares the bundle");
    assert_eq!(graph.nodes.len(), programs);
    assert_eq!(
        graph::write::index_text(None, &graph),
        committed,
        "lengua makes examples/index.md just so"
    );
    assert_eq!(
        graph::write::index_text(Some(&committed), &graph),
        committed,
        "and keeps it as it is"
    );
}

/// Every check, on every system, says how the graph stands: first, where its bundle is.
#[tokio::test]
async fn every_check_reports_the_graph() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    let report = session.record(&program).await;
    let lines = strings(&report["graph"]);
    let bundle = program.path().parent().expect("the program's bundle");
    let first = format!("The bundle at {} ", bundle.display());
    assert!(
        lines.first().is_some_and(|line| line.starts_with(&first)),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn check_keeps_the_bundles_index_and_log() {
    let session = Session::start(Protocol::Discover).await;
    let program = Program::with_add_idea();
    let report = session.record(&program).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lines = strings(&report["graph"]);
    for said in [
        "lengua made index.md, listing program",
        "log.md: add recorded",
    ] {
        assert!(lines.iter().any(|line| line == said), "{said}: {report}");
    }
    let bundle = program.path().parent().expect("the program's bundle");
    let read = |name: &str| std::fs::read_to_string(bundle.join(name)).unwrap_or_default();
    assert_eq!(
        read("index.md"),
        "---\nokf_version: \"0.2\"\n---\n# Programs\n\n* [add](program/idea.md) - Adds the \
         two whole numbers in its arguments and prints the sum.\n"
    );
    let log = read("log.md");
    assert!(
        log.ends_with("* **Creation**: [add](program/idea.md) recorded.\n"),
        "{log}"
    );
    // Checking again records nothing new, and the log says nothing twice.
    let again = session.check(&program).await;
    assert_eq!(again["state"], "write_code", "{again}");
    assert_eq!(read("log.md"), log);
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn check_blocks_a_cycle_and_not_a_broken_link() {
    let session = Session::start(Protocol::Discover).await;
    let parent = tempfile::tempdir().expect("a folder for the bundle");
    let a = Program::new_in(parent.path(), "a");
    let b = Program::new_in(parent.path(), "b");
    let on = |name: &str, part: &Program| {
        let link = format!("[{name}](../{}/idea.md)", part.name());
        building_on(&idea(name, "Builds on another."), &link)
    };
    a.write("idea.md", &on("a", &b));
    b.write("idea.md", &on("b", &a));
    let report = session.record(&a).await;
    assert_eq!(report["state"], "idea_problems", "{report}");
    let problems = report["problems"].to_string();
    assert!(problems.contains("make a cycle"), "{report}");
    let lines = strings(&report["graph"]);
    assert!(
        lines.iter().any(|line| line.starts_with("blocks: ")),
        "{report}"
    );
    // A link that leads nowhere is a warning, and its program goes on.
    let linked = Program::new_in(parent.path(), "linked");
    linked.write("idea.md", &linking("[subtract](../subtract/idea.md)"));
    let report = session.record(&linked).await;
    assert_eq!(report["state"], "write_code", "{report}");
    let lines = strings(&report["graph"]);
    let broken = "links to ../subtract/idea.md, which isn't there";
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("warning: ") && line.ends_with(broken)),
        "{report}"
    );
    session.service.cancel().await.expect("close the session");
}

#[tokio::test]
async fn two_checks_at_once_leave_a_consistent_index() {
    let session = Session::start(Protocol::Discover).await;
    let parent = tempfile::tempdir().expect("a folder for the bundle");
    let one = Program::new_in(parent.path(), "one");
    let two = Program::new_in(parent.path(), "two");
    for program in [&one, &two] {
        program.write("idea.md", ADD_IDEA);
    }
    let (first, second) = tokio::join!(session.record(&one), session.record(&two));
    for report in [&first, &second] {
        assert_eq!(report["state"], "write_code", "{report}");
    }
    let index = std::fs::read_to_string(parent.path().join("index.md")).expect("index.md");
    let entries: Vec<&str> = index
        .lines()
        .filter(|line| line.starts_with("* ["))
        .collect();
    let mut expected: Vec<String> = [&one, &two]
        .iter()
        .map(|program| {
            format!(
                "* [add]({}/idea.md) - Adds the two whole numbers in its arguments and prints \
                 the sum.",
                program.name()
            )
        })
        .collect();
    expected.sort();
    assert_eq!(entries, expected, "{index}");
    let log = std::fs::read_to_string(parent.path().join("log.md")).expect("log.md");
    assert_eq!(log.matches(") recorded.").count(), 2, "{log}");
    session.service.cancel().await.expect("close the session");
}
