//! The unmodified main template of every target goes through `check` to `done`, the way an agent
//! that copies it for a spec like its own would see: the spec recorded, the template's rules, its
//! test runs, its record in lengua.lock and lengua's line in main.s, a comment of the CPU's own.
//! One case with a cell for each target; each runs on that target's own runner.

mod common;
mod per_target;

use std::path::PathBuf;

use common::{Program, Protocol};
use lengua::header::Header;
use lengua::platform::Profile;
use lengua::targets::PerTarget;
use serde_json::Value;

/// Each target's main template, in the compile skill's assets.
const TEMPLATES: PerTarget<&str> = PerTarget {
    linux_x86_64: "main-template.s",
    linux_aarch64: "linux-aarch64/main-template.s",
};

/// A file of the compile skill's assets.
fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugin/skills/compile/assets")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[tokio::test]
async fn the_template_passes_its_tests_and_is_stamped() {
    for target in Profile::ALL {
        let Some(session) = per_target::session(target, Protocol::Discover).await else {
            continue;
        };
        let name = target.name();
        let program = Program::new();
        program.write("spec.md", &asset("template-spec.md"));
        let report = session.record(&program).await;
        assert_eq!(report["state"], "write_code", "{name}: {report}");
        program.write("main.s", &asset(TEMPLATES.get(target)));
        let report = session.settle(&program).await;
        assert_eq!(report["state"], "done", "{name}: {report}");
        assert_eq!(
            report["tests"][0], "main.s follows every rule and passed all 274 test runs",
            "{name}: {report}"
        );
        let costs = report["performance"]
            .as_array()
            .expect("what each test cost");
        assert_eq!(
            costs.len(),
            7,
            "{name}: 4 examples and 3 generated blocks: {report}"
        );
        assert!(
            text(&costs[0]).starts_with("example B1: 1 run")
                && text(&costs[0]).contains("its input, run alone 5 times: median "),
            "{name}: {report}"
        );
        // The times come from the block's largest input, run alone: the minus sign and 19 digits.
        assert!(
            text(&costs[4]).starts_with("generated B1: 200 runs")
                && text(&costs[4]).contains("its largest input, 20 bytes, run alone 5 times"),
            "{name}: {report}"
        );
        assert_eq!(
            report["example"],
            "example B1: given arguments [\"7\"], the program printed \"-7\\n\", exited with code 0",
            "{name}: {report}"
        );
        let lock = program.read("lengua.lock").expect("the lock");
        assert!(
            lock.contains("[code]") && lock.contains("executable_sha256 = "),
            "{name}: {lock}"
        );
        assert!(lock.contains("toolchain = \"llvm-mc "), "{name}: {lock}");
        // lengua's line is the second of main.s, a comment of the target's own CPU, and names
        // the target the code was compiled for.
        let code = program.read("main.s").expect("main.s");
        let line = code.lines().nth(1).unwrap_or_default();
        let comment = target.isa().comment();
        assert!(
            line.starts_with(&format!("{comment} lengua: compiled from spec.md sha256:")),
            "{name}: {code}"
        );
        let header = Header::of(&code).expect("lengua's line");
        assert_eq!(header.target, name, "{name}: {code}");
        session.service.cancel().await.expect("close the session");
    }
}
