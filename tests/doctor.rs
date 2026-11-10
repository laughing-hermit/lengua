//! `lengua-mcp doctor`, the same on every system: one line for each thing it checks, in order,
//! a sentence under each problem saying what to do, the verdict last, and exit code 0 only when
//! nothing is wrong. The engine runs WebAssembly on every runner. A data folder or `PLUGIN_DATA`
//! that can't be written is a problem. The solvers are found where the prover finds them: in the
//! solvers folder beside the server first, as a release archive keeps them, then on PATH; and a
//! WebAssembly z3 before either, which doctor compiles where its host finds it.

use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::process::Command;

/// What doctor checks, in the order of its lines.
const CHECKED: [&str; 8] = [
    "server: ",
    "engine: ",
    "cpu: ",
    "z3: ",
    "cvc5: ",
    "data folder: ",
    "PLUGIN_DATA: ",
    "clock: ",
];

/// `server doctor` with the data folder `data`, `PLUGIN_DATA` as `plugin_data` gives it (unset
/// when `None`) and PATH as `path` gives it (as this process has it when `None`): its exit code,
/// and what it printed.
fn doctor_of(
    server: &Path,
    data: &Path,
    plugin_data: Option<&Path>,
    path: Option<&str>,
) -> (Option<i32>, String) {
    let mut command = Command::new(server);
    command
        .arg("--data-dir")
        .arg(data)
        .arg("doctor")
        .env_remove("CLAUDE_PLUGIN_DATA");
    match plugin_data {
        Some(folder) => command.env("PLUGIN_DATA", folder),
        None => command.env_remove("PLUGIN_DATA"),
    };
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let output = command.output().expect("lengua-mcp runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// The built `lengua-mcp`'s doctor.
fn doctor(data: &Path, plugin_data: Option<&Path>) -> (Option<i32>, String) {
    doctor_of(
        Path::new(env!("CARGO_BIN_EXE_lengua-mcp")),
        data,
        plugin_data,
        None,
    )
}

/// The line of `said` that starts with `start`, and the problem's sentence under it, if any.
fn line_of<'a>(said: &'a str, start: &str) -> (&'a str, Option<&'a str>) {
    let lines: Vec<&str> = said.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("no line starts with {start:?}:\n{said}"));
    let problem = lines
        .get(at + 1)
        .copied()
        .and_then(|next| next.strip_prefix("  "));
    (lines[at], problem)
}

/// Runs the program just written at `path` once, so that it can run: a process another test
/// forked while the file was open for writing keeps it busy (Linux's "text file busy") until
/// that process starts its own program, and once one run starts, none can.
fn ready(path: &Path) {
    for _ in 0..100 {
        match Command::new(path).arg("--version").output() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            _ => break,
        }
    }
}

/// Each thing doctor checks has its line, in order; each problem's sentence is under its line;
/// the verdict, last, counts the problems, and the exit code is 0 exactly when there are none.
/// The engine compiles and runs WebAssembly here, the data folder is written, and an unset
/// `PLUGIN_DATA` is no problem.
#[test]
fn doctor_says_each_thing_on_its_line() {
    let root = tempfile::tempdir().expect("a folder");
    let data = root.path().join("data");
    let (code, said) = doctor(&data, None);
    let lines: Vec<&str> = said.lines().collect();
    let checks: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.starts_with("  "))
        .collect();
    assert_eq!(checks.len(), CHECKED.len() + 1, "{said}");
    for (line, start) in checks.iter().zip(CHECKED) {
        assert!(line.starts_with(start), "{start:?} isn't next:\n{said}");
    }
    let problems = lines.iter().filter(|line| line.starts_with("  ")).count();
    let verdict = *lines.last().expect("a verdict");
    match problems {
        0 => assert_eq!(verdict, "nothing is wrong", "{said}"),
        1 => assert_eq!(verdict, "1 problem", "{said}"),
        many => assert_eq!(verdict, format!("{many} problems"), "{said}"),
    }
    assert_eq!(code, Some(i32::from(problems != 0)), "{said}");

    let (server, problem) = line_of(&said, "server: ");
    let version = format!("server: lengua-mcp {}, ", env!("CARGO_PKG_VERSION"));
    assert!(server.starts_with(&version), "{said}");
    assert_eq!(problem, None, "{said}");
    let (engine, problem) = line_of(&said, "engine: ");
    let ran = format!(
        "engine: Wasmtime {} with Cranelift compiled and ran a module here",
        lengua::wasm::engine::ENGINE_VERSION
    );
    assert!(engine.starts_with(&ran), "{said}");
    assert_eq!(problem, None, "{said}");
    let (cpu, _) = line_of(&said, "cpu: ");
    let target = format!("cpu: {}-{}, ", std::env::consts::OS, std::env::consts::ARCH);
    assert!(
        cpu.starts_with(&target) && cpu.contains(", vectors up to "),
        "{said}"
    );
    let (folder, problem) = line_of(&said, "data folder: ");
    assert_eq!(
        folder,
        format!("data folder: {}, writable", data.display()),
        "{said}"
    );
    assert_eq!(problem, None, "{said}");
    assert!(data.is_dir(), "doctor didn't make the data folder");
    let (plugin_data, problem) = line_of(&said, "PLUGIN_DATA: ");
    let unset = "PLUGIN_DATA: not set, so compiled modules are kept in the data folder";
    assert_eq!(plugin_data, unset, "{said}");
    assert_eq!(problem, None, "{said}");
}

/// A data folder or a `PLUGIN_DATA` that can't be written, here because a file stands where a
/// folder would go, is a problem whose sentence says what to do, and doctor exits 1. A relative
/// `PLUGIN_DATA`, which lengua passes over, isn't one.
#[test]
fn a_folder_that_can_t_be_written_is_a_problem() {
    let root = tempfile::tempdir().expect("a folder");
    let file = root.path().join("a file");
    std::fs::write(&file, "").expect("a file");
    let (code, said) = doctor(&file.join("data"), Some(file.join("plugin").as_path()));
    assert_eq!(code, Some(1), "{said}");
    let (folder, problem) = line_of(&said, "data folder: ");
    assert!(folder.contains("which lengua can't write"), "{said}");
    assert!(
        problem.is_some_and(|problem| problem.contains("--data-dir")),
        "{said}"
    );
    let (plugin_data, problem) = line_of(&said, "PLUGIN_DATA: ");
    assert!(plugin_data.contains("which lengua can't write"), "{said}");
    assert!(
        problem.is_some_and(|problem| problem.contains("agent harness")),
        "{said}"
    );
    let (_, said) = doctor(&root.path().join("data"), Some(Path::new("relative")));
    let (plugin_data, problem) = line_of(&said, "PLUGIN_DATA: ");
    assert!(plugin_data.contains("not an absolute path"), "{said}");
    assert_eq!(problem, None, "{said}");
}

/// `lengua-mcp` copied into `<plugin>/server`, as a release archive lays it out, with an empty
/// `<plugin>/solvers` beside it.
fn archived_server(plugin: &Path) -> PathBuf {
    let server = plugin
        .join("server")
        .join(format!("lengua-mcp{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(server.parent().expect("the server folder")).expect("a folder");
    std::fs::copy(env!("CARGO_BIN_EXE_lengua-mcp"), &server).expect("copy lengua-mcp");
    ready(&server);
    std::fs::create_dir_all(plugin.join("solvers")).expect("the solvers folder");
    server
}

/// A solver in the solvers folder beside the server is the one doctor names, before PATH, as
/// the prover takes it; with neither, doctor says where it looked and what to do. Here the z3
/// beside the server is lengua-mcp itself, which prints no solver's version: a problem.
#[test]
fn the_solvers_beside_the_server_come_first() {
    let root = tempfile::tempdir().expect("a folder");
    let plugin = root.path().join("plugin");
    let server = archived_server(&plugin);
    let z3 = format!("z3{}", std::env::consts::EXE_SUFFIX);
    std::fs::copy(&server, plugin.join("solvers").join(&z3)).expect("a z3 that isn't one");
    ready(&plugin.join("solvers").join(&z3));
    let (code, said) = doctor_of(&server, &root.path().join("data"), None, Some(""));
    assert_eq!(code, Some(1), "{said}");
    let (line, problem) = line_of(&said, "z3: ");
    let beside = format!("{MAIN_SEPARATOR}plugin{MAIN_SEPARATOR}solvers{MAIN_SEPARATOR}{z3}");
    assert!(
        line.starts_with("z3: beside the server, ")
            && line.ends_with(&format!("{beside}, which printed no version")),
        "{said}"
    );
    assert!(
        problem.is_some_and(|problem| problem.contains("4.16.0")),
        "{said}"
    );
    let (line, problem) = line_of(&said, "cvc5: ");
    assert!(
        line.starts_with("cvc5: neither in ") && line.ends_with(" nor on PATH"),
        "{said}"
    );
    assert!(
        problem.is_some_and(|problem| problem.contains("release archive")),
        "{said}"
    );
}

/// Writes a stand-in for a solver at `path` that prints `version` as the solver does: a shell
/// script, so Unix's alone.
#[cfg(unix)]
fn stand_in(path: &Path, version: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(path, format!("#!/bin/sh\necho '{version}'\n")).expect("a stand-in");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("runnable");
    ready(path);
}

/// With the pinned z3 and cvc5 beside the server, as a release archive ships them, doctor finds
/// nothing wrong, PATH left out.
#[cfg(unix)]
#[test]
fn the_archive_s_layout_is_healthy() {
    let root = tempfile::tempdir().expect("a folder");
    let plugin = root.path().join("plugin");
    let server = archived_server(&plugin);
    stand_in(
        &plugin.join("solvers").join("z3"),
        "Z3 version 4.16.0 - 64 bit",
    );
    stand_in(
        &plugin.join("solvers").join("cvc5"),
        "This is cvc5 version 1.3.4 [git tag 1.3.4 branch HEAD]",
    );
    let (code, said) = doctor_of(&server, &root.path().join("data"), None, Some(""));
    for solver in ["z3", "cvc5"] {
        let (line, problem) = line_of(&said, &format!("{solver}: "));
        assert!(
            line.contains("the version lengua pins, beside the server, "),
            "{said}"
        );
        assert_eq!(problem, None, "{said}");
    }
    assert_eq!(said.lines().last(), Some("nothing is wrong"), "{said}");
    assert_eq!(code, Some(0), "{said}");
}

/// The stand-in solver of tests/fixtures/solver-wasm, a WebAssembly module with exceptions that
/// prints a version as z3 does, as wast encodes it.
fn stand_in_module() -> Vec<u8> {
    let text = include_str!("fixtures/solver-wasm/stand-in.wat");
    let buffer = wast::parser::ParseBuffer::new(text).expect("WebAssembly text");
    let wast::Wat::Module(mut module) =
        wast::parser::parse::<wast::Wat>(&buffer).expect("a module")
    else {
        panic!("the stand-in isn't a module");
    };
    module.encode().expect("the module's bytes")
}

/// How many files there are at any depth of `folder`.
fn files_under(folder: &Path) -> usize {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() { files_under(&path) } else { 1 }
        })
        .sum()
}

/// A WebAssembly z3 comes first, here the stand-in that `LENGUA_Z3_WASM` names: doctor names it
/// as WebAssembly, compiles it into the cache of lengua's own data folder, which the solver's host
/// reads, and reads its version through that host, as the prover starts it.
#[test]
fn a_webassembly_z3_is_compiled_where_its_host_finds_it() {
    let root = tempfile::tempdir().expect("a folder");
    let module = root.path().join("stand-in.wasm");
    std::fs::write(&module, stand_in_module()).expect("the stand-in");
    // lengua's own data folder, as each system names it, inside this test's folder.
    let home = root.path().join("home");
    let output = Command::new(env!("CARGO_BIN_EXE_lengua-mcp"))
        .arg("--data-dir")
        .arg(root.path().join("data"))
        .arg("doctor")
        .env(lengua::prove::smt::Z3_WASM, &module)
        .env_remove(lengua::prove::smt::SOLVER_HOST)
        .env("HOME", &home)
        .env("XDG_DATA_HOME", home.join("share"))
        .env("LOCALAPPDATA", &home)
        .env_remove("CLAUDE_PLUGIN_DATA")
        .env_remove("PLUGIN_DATA")
        .output()
        .expect("lengua-mcp runs");
    let said = String::from_utf8_lossy(&output.stdout).into_owned();
    let (line, problem) = line_of(&said, "z3: ");
    let named = format!(
        "z3: 4.16.0, the version lengua pins, WebAssembly that LENGUA_Z3_WASM names, {}, compiled \
         for this computer and kept in ",
        module.display()
    );
    assert!(line.starts_with(&named), "{said}");
    assert_eq!(problem, None, "{said}");
    assert!(
        files_under(&home) > 0,
        "nothing was kept in {}:\n{said}",
        home.display()
    );
}
