use std::fs;
use std::path::Path;

use std::fmt::Write as _;

use lengua_idea::Case;

use crate::build::Problem;

pub(crate) const FOLDER: &str = "target/lengua/harness";
pub(crate) const MUTANT_FOLDER: &str = "target/lengua/mutant-harness";
const FIRST_ARM: usize = 7;

pub(crate) struct Library<'a> {
    pub package: &'a str,
    pub dependency: &'a str,
    pub path: &'a str,
    pub harness: &'a str,
}

pub(crate) fn manifest(library: &Library) -> String {
    let crate_name = library.package.replace('-', "_");
    format!(
        "[package]\nname = \"{harness}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[[bin]]\nname = \"{harness}\"\npath = \"main.rs\"\n\n[dependencies]\n{crate_name} = {{ package = \"{dependency}\", path = \"{path}\" }}\n\n[profile.release]\nopt-level = 3\npanic = \"abort\"\nlto = true\ncodegen-units = 1\n\n[workspace]\n",
        harness = library.harness,
        dependency = library.dependency,
        path = library.path,
    )
}

pub(crate) fn main_text(package: &str, cases: &[Case]) -> String {
    let crate_name = package.replace('-', "_");
    let mut out = format!(
        "#![allow(unused_imports, unused_braces, clippy::all)]\nuse {crate_name}::*;\n\nfn main() {{\n    let case = std::env::args().nth(1).unwrap_or_default();\n    match case.as_str() {{\n"
    );
    for (index, case) in cases.iter().enumerate() {
        let call = case.test.call.as_deref().unwrap_or("()");
        let _ = writeln!(
            out,
            "        \"{index}\" => println!(\"{{:?}}\", {call}),"
        );
    }
    out.push_str("        _ => {}\n    }\n}\n");
    out
}

pub(crate) fn write(
    folder: &Path,
    dir: &str,
    library: &Library,
    cases: &[Case],
) -> Result<(), String> {
    let place = folder.join(dir);
    let fail = |e: std::io::Error| format!("lengua could not write the harness `{dir}`: {e}");
    fs::create_dir_all(&place).map_err(fail)?;
    fs::write(place.join("Cargo.toml"), manifest(library)).map_err(fail)?;
    fs::write(place.join("main.rs"), main_text(library.package, cases)).map_err(fail)?;
    if let Ok(locked) = fs::read(folder.join("Cargo.lock")) {
        fs::write(place.join("Cargo.lock"), locked).map_err(fail)?;
    }
    Ok(())
}

pub(crate) fn mapped(problems: Vec<Problem>, cases: &[Case]) -> Vec<Problem> {
    problems
        .into_iter()
        .map(|p| {
            let index = p.line.checked_sub(FIRST_ARM);
            let case = index.and_then(|i| cases.get(i)).filter(|_| p.file.ends_with("main.rs"));
            match case {
                Some(case) => Problem {
                    file: case.test.file.clone(),
                    line: case.test.line,
                    code: "harness-error".to_string(),
                    message: format!(
                        "the call of the example `{}` of the behaviour `{}` does not build against the library: {}",
                        case.test.name, case.test.behaviour, p.message
                    ),
                },
                None => p,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{FIRST_ARM, Library, main_text, manifest, mapped};
    use crate::build::problem;
    use lengua_idea::{Case, Test};

    fn case(call: &str) -> Case {
        Case {
            values: Vec::new(),
            test: Test {
                file: "ideas/parse.md".to_string(),
                line: 12,
                name: "one".to_string(),
                behaviour: "Read it".to_string(),
                call: Some(call.to_string()),
                ..Test::default()
            },
        }
    }

    #[test]
    fn the_harness_calls_each_case_on_its_own_line() {
        let text = main_text(
            "my-json",
            &[case("parse(\"1\")"), case("write(&Value::Null)")],
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "use my_json::*;");
        assert_eq!(
            lines[FIRST_ARM - 1],
            "        \"0\" => println!(\"{:?}\", parse(\"1\")),"
        );
        assert_eq!(
            lines[FIRST_ARM],
            "        \"1\" => println!(\"{:?}\", write(&Value::Null)),"
        );
        let toml = manifest(&Library {
            package: "my-json",
            dependency: "my-json-mutant",
            path: "../mutant",
            harness: "my-json-mutant-harness",
        });
        assert!(toml.contains("my_json = { package = \"my-json-mutant\", path = \"../mutant\" }"));
        assert!(toml.contains("name = \"my-json-mutant-harness\""));
        let found = mapped(
            vec![problem(
                "main.rs",
                FIRST_ARM + 1,
                "rust-error",
                "no".to_string(),
            )],
            &[case("a()"), case("b()")],
        );
        assert_eq!(found[0].file, "ideas/parse.md");
        assert_eq!(found[0].line, 12);
        assert_eq!(found[0].code, "harness-error");
    }
}
