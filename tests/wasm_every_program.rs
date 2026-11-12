//! Every program of the repository, in examples/ and tests/fixtures, goes through the server from
//! the user's request to done, the same on every system: its idea, an Attested Computation, is
//! recorded with the user's words; its main.wat follows every rule of the WebAssembly dialect and
//! passes every test of its idea; and lengua's entry in the idea's `verified` names the main.wat
//! it checked. The test takes every folder of both, so a program added to either is checked with
//! the rest, and a program that fails is named while the others still go through.

mod common;

use std::path::{Path, PathBuf};

use common::{Program, Protocol, Session};
use lengua::hash::Sha256Hex;
use lengua::trust;
use serde_json::Value;

/// The folders of the programs, relative to the repository: a program is a folder in one of them
/// with an idea.md and a main.wat.
const FOLDERS: [&str; 2] = ["examples", "tests/fixtures"];

/// The fewest programs the two folders hold: the 15 examples, and the 29 fixtures, the prover's
/// small add, echo and greet and the benchmark's ten among them.
const AT_LEAST: usize = 44;

/// A value of a reply as text, or none.
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

/// Whether any of `lines` contains `needle`.
fn any_line(lines: &Value, needle: &str) -> bool {
    lines
        .as_array()
        .is_some_and(|lines| lines.iter().any(|line| text(line).contains(needle)))
}

/// Every program folder of [`FOLDERS`], each with an idea.md and a main.wat: its name, its path
/// from the repository's root, such as `tests/fixtures/add` beside `examples/add`, and its path,
/// in order.
fn programs() -> Vec<(String, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for place in FOLDERS {
        for entry in std::fs::read_dir(root.join(place)).expect("a folder of programs") {
            let path = entry.expect("an entry").path();
            if path.join("idea.md").is_file()
                && path.join("main.wat").is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                found.push((format!("{place}/{name}"), path.clone()));
            }
        }
    }
    found.sort();
    found
}

/// The file `file` of the program `name`, in `folder`.
fn read(name: &str, folder: &Path, file: &str) -> Result<String, String> {
    std::fs::read_to_string(folder.join(file)).map_err(|error| format!("{name}/{file}: {error}"))
}

/// The program `name`, in `folder`, from the user's request to done in `session`, as an agent
/// takes it: its idea recorded, then its main.wat checked until its tests end. Gives what went
/// wrong, if anything.
async fn from_request_to_done(session: &Session, name: &str, folder: &Path) -> Result<(), String> {
    let idea = read(name, folder, "idea.md")?;
    let code = read(name, folder, "main.wat")?;
    let program = Program::new();
    program.write("idea.md", &idea);
    // The parts it builds on stand beside it, each checked first.
    session.settle_parts(&program, folder).await?;
    // The modules it calls stand ready too: a called program beside it, checked first, and an
    // outside module in its folder, as CI built it from the source beside the example.
    let calls = !common::calls_of(folder).is_empty();
    if !session.settle_calls(&program, folder).await? {
        return Ok(());
    }
    let recorded = session.record(&program).await;
    if recorded["state"] != "write_code" {
        return Err(format!("{name}: its idea wasn't recorded: {recorded}"));
    }
    program.write("main.wat", &code);
    let done = session.settle(&program).await;
    if done["state"] != "done" {
        return Err(format!("{name}: {done}"));
    }
    if !any_line(&done["tests"], "main.wat follows every rule and passed all") {
        return Err(format!("{name}: its tests didn't all pass: {done}"));
    }
    // A program that calls a module gets no entry: the entry names an idea and its main.wat,
    // and such a program rests on a third file. Its reply names each module it calls.
    if calls {
        let written = program.read("idea.md").unwrap_or_default();
        if !trust::lengua_entries(&written).is_empty() {
            return Err(format!("{name}: it calls a module and has an entry in `verified`"));
        }
        if done["calls"].as_array().is_none_or(Vec::is_empty) {
            return Err(format!("{name}: its reply names no module it calls: {done}"));
        }
        return Ok(());
    }
    // lengua's entry in `verified` names the main.wat it checked.
    let written = program.read("idea.md").unwrap_or_default();
    let checked = Sha256Hex::of(code.as_bytes());
    let entries = trust::lengua_entries(&written);
    if !entries.iter().any(|entry| entry.code == checked) {
        return Err(format!(
            "{name}: lengua's entry in `verified` doesn't name its main.wat: {written}"
        ));
    }
    Ok(())
}

/// Every program of examples/ and tests/fixtures goes from the user's request to done, one after
/// another in one session, whose pool gives each program's tests every worker of the machine.
#[tokio::test]
async fn every_webassembly_program_goes_from_its_request_to_done() {
    let found = programs();
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        found.len() >= AT_LEAST,
        "{FOLDERS:?} hold {} programs: {names:?}",
        found.len()
    );
    let session = Session::start(Protocol::Discover).await;
    let mut failures = Vec::new();
    for (name, folder) in &found {
        if let Err(problem) = from_request_to_done(&session, name, folder).await {
            failures.push(problem);
        }
    }
    session.service.cancel().await.expect("close the session");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
