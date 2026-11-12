//! The compile skill's WebAssembly templates, each a complete program, on every system: for
//! `main-template.wat` and each `*-template.wat` of the skill's assets, its idea parses and its
//! tests check, lengua builds its module, the WebAssembly dialect's rules find no violation and
//! no unsafe block, and every test of its idea passes, run in lengua's pool of workers as `check`
//! runs a program's tests. A template that breaks a rule, or fails a test, is one an agent would
//! copy the break or the bug from.

use std::path::PathBuf;
use std::sync::Arc;

use lengua::checker::wasm::{Context, check};
use lengua::wasm::{Pool, Program};

/// Each WebAssembly template, with its idea: the program, then the idea it's made from.
const TEMPLATES: [(&str, &str); 9] = [
    ("main-template.wat", "main-template-idea.md"),
    ("stream-template.wat", "stream-template-wat-idea.md"),
    ("vector-template.wat", "vector-template-wat-idea.md"),
    ("server-template.wat", "server-template-wat-idea.md"),
    ("tally-template.wat", "tally-template-wat-idea.md"),
    ("client-template.wat", "client-template-wat-idea.md"),
    ("files-template.wat", "files-template-wat-idea.md"),
    ("program-template.wat", "program-template-wat-idea.md"),
    ("terminal-template.wat", "terminal-template-wat-idea.md"),
];

/// This binary, which the pool's workers are.
fn launcher() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lengua-mcp"))
}

/// The file `name` of the compile skill's assets.
fn asset(name: &str) -> Result<String, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
}

/// The seed the generated tests draw from: the one `LENGUA_RANDOM_SEED` names, to run a failure
/// again, or else one drawn afresh, as `check` draws one for each check. A failure names it.
fn seed() -> u64 {
    std::env::var("LENGUA_RANDOM_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).expect("random bytes for a seed");
            u64::from_le_bytes(bytes)
        })
}

/// Checks the template `code`, made from the idea `idea_file`, from its idea to its tests, and
/// gives how many test runs it passed.
async fn template(pool: &Pool, code: &str, idea_file: &str) -> Result<usize, String> {
    let idea = lengua::idea::parse(&asset(idea_file)?).map_err(|problems| {
        let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
        format!("{idea_file} doesn't parse: {}", shown.join("; "))
    })?;
    let suite = lengua::testfile::check(&idea).map_err(|problems| {
        let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
        format!("{idea_file}'s tests don't check: {}", shown.join("; "))
    })?;
    let text = asset(code)?;
    let built = lengua::wasm::text::build(&text).map_err(|problem| problem.to_string())?;
    let report = check(
        &text,
        &built,
        &Context {
            uses: &idea.uses,
            network: &idea.network,
            secrets: &idea.secrets,
        },
    );
    if !report.passed() {
        return Err(format!(
            "it breaks the rules: {}",
            report.lines().join("; ")
        ));
    }
    if !report.unsafe_blocks.is_empty() {
        return Err(format!(
            "it has unsafe blocks, which a template shouldn't teach: {:?}",
            report.unsafe_blocks
        ));
    }
    let program = Program::new(built).map_err(|error| format!("its module isn't kept: {error}"))?;
    let seed = seed();
    let outcome = lengua::testrun::run_wasm_tests(
        &suite,
        pool,
        Arc::new(program),
        seed,
        &idea.uses,
        &idea.network,
        lengua::testrun::workers(),
    )
    .await
    .map_err(|error| format!("its tests couldn't run (seed {seed}): {error}"))?;
    if !outcome.passed() {
        return Err(format!(
            "its tests failed with seed {seed}, which LENGUA_RANDOM_SEED={seed} repeats: {}",
            outcome.failures.join("; ")
        ));
    }
    if outcome.runs == 0 {
        return Err(format!("{idea_file} gave it no test to run"));
    }
    Ok(outcome.runs)
}

/// One template after another, each with every worker the machine has, so no template's tests
/// wait on another's.
#[tokio::test(flavor = "multi_thread")]
async fn every_webassembly_template_follows_the_rules_and_passes_its_tests() {
    let pool = Pool::new(launcher(), None);
    let mut failures = Vec::new();
    for (code, idea_file) in TEMPLATES {
        match template(&pool, code, idea_file).await {
            Ok(runs) => eprintln!("{code}: every rule passes, and all {runs} test runs"),
            Err(problem) => failures.push(format!("{code}: {problem}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The templates are every WebAssembly program in the compile skill's assets, each with an
/// idea of its own: a template added without its line above would go unchecked.
#[test]
fn the_list_holds_every_webassembly_template() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin/skills/compile/assets");
    let mut found: Vec<String> = std::fs::read_dir(&assets)
        .expect("the compile skill's assets")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".wat"))
        .collect();
    found.sort();
    let mut listed: Vec<String> = TEMPLATES
        .iter()
        .map(|(code, _)| (*code).to_owned())
        .collect();
    listed.sort();
    assert_eq!(found, listed);
    for (_, idea_file) in TEMPLATES {
        assert!(assets.join(idea_file).is_file(), "{idea_file} is missing");
    }
}
