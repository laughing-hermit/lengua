use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use lengua_idea::{Build, Case, End, Range, Reach, Test};

use crate::build::Problem;
use crate::fixtures::Fixtures;
use crate::runs::GRACE;
use crate::worker::{self, Request};
use lengua_host::{Ending, Input, Outcome};
use lengua_idea::lock::Kind;

const SHOWN: usize = 60;
const BEFORE: usize = 20;

static RUNS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn cases(build: &Build) -> Vec<Case> {
    build
        .tests
        .iter()
        .filter(|t| !t.screen)
        .flat_map(lengua_idea::cases)
        .collect()
}

fn shown(text: &str, at: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = at.saturating_sub(BEFORE);
    let window: String = chars.iter().skip(start).take(SHOWN).collect();
    let before = if start > 0 { "..." } else { "" };
    let after = if start + SHOWN < chars.len() {
        "..."
    } else {
        ""
    };
    format!("{before}{window:?}{after}")
}

pub(crate) fn diff(what: &str, expected: &str, got: &str) -> Option<String> {
    if expected == got {
        return None;
    }
    let at = expected
        .chars()
        .zip(got.chars())
        .take_while(|(a, b)| a == b)
        .count();
    Some(format!(
        "{what}: expected {} and got {} (they differ at character {})",
        shown(expected, at),
        shown(got, at),
        at + 1
    ))
}

pub(crate) fn request_bytes(test: &Test) -> Vec<u8> {
    let line = test.request.as_deref().unwrap_or("GET /");
    let body = test.request_body.clone().unwrap_or_default();
    let mut out = format!("{line} HTTP/1.1\r\n");
    let mut host = false;
    for (name, value) in &test.request_headers {
        host |= name.eq_ignore_ascii_case("host");
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    if !host {
        out.push_str("host: localhost\r\n");
    }
    if !body.is_empty() || test.request_body.is_some() {
        out.push_str("content-length: ");
        out.push_str(&body.len().to_string());
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    out.push_str(&body);
    out.into_bytes()
}

type Answer = (u16, Vec<(String, String)>, String);

pub(crate) fn parse_response(bytes: &[u8]) -> Option<Answer> {
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(bytes.get(..end)?).to_string();
    let body = String::from_utf8_lossy(bytes.get(end + 4..)?).to_string();
    let mut lines = head.split("\r\n");
    let status: u16 = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    Some((status, headers, body))
}

fn compare_http(test: &Test, outcome: &Outcome, out: &mut Vec<String>) {
    let Some((status, headers, body)) = parse_response(&outcome.stdout) else {
        out.push(format!(
            "the server's answer is not an HTTP answer: {:?}",
            String::from_utf8_lossy(&outcome.stdout)
        ));
        return;
    };
    if let Some(wanted) = test.status
        && wanted != status
    {
        out.push(format!(
            "status: expected {wanted} and got {status} (the body: {body:?})"
        ));
    }
    for (name, wanted) in &test.expect_headers {
        match headers.iter().find(|(n, _)| n == name) {
            Some((_, got)) => out.extend(diff(&format!("the header `{name}`"), wanted, got)),
            None => out.push(format!("the answer has no header `{name}`")),
        }
    }
    if let Some(wanted) = &test.expect_body {
        out.extend(diff("body", wanted, &body));
    }
}

fn compare(test: &Test, outcome: &Outcome, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let (end, detail) = match &outcome.ending {
        Ending::Exited(_) => (Some(End::Exited), None),
        Ending::OutOfTime(message) | Ending::Trapped(message) => {
            let end = if matches!(outcome.ending, Ending::Trapped(_)) {
                End::Trapped
            } else {
                End::OutOfTime
            };
            (Some(end), Some(message.as_str()))
        }
        Ending::OutOfMemory => (Some(End::OutOfMemory), None),
        Ending::Refused(message) => (None, Some(message.as_str())),
    };
    let Some(end) = end else {
        out.push(format!(
            "the host did not start the program: {}",
            detail.unwrap_or("")
        ));
        return out;
    };
    if end != test.end {
        out.push(format!(
            "the run ended `{}`, and the example expects `{}`{}",
            end.name(),
            test.end.name(),
            detail.map_or_else(String::new, |m| format!(" ({m})"))
        ));
    }
    if let (Ending::Exited(code), Some(wanted)) = (&outcome.ending, test.exit_code)
        && *code != i32::from(wanted)
    {
        out.push(format!("exit code: expected {wanted} and got {code}"));
    }
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    if test.request.is_some() {
        compare_http(test, outcome, &mut out);
    } else if let Some(value) = &test.value {
        let got = stdout.strip_suffix('\n').unwrap_or(&stdout);
        out.extend(diff("value", value, got));
    } else {
        out.extend(diff(
            "standard output",
            test.stdout.as_deref().unwrap_or(""),
            &stdout,
        ));
    }
    if let Some(wanted) = &test.stderr {
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        out.extend(diff("standard error", wanted, &stderr));
    }
    for (path, wanted) in &test.expect_files {
        match fs::read(root.join(path)) {
            Ok(bytes) => out.extend(diff(
                &format!("the file `{path}`"),
                wanted,
                &String::from_utf8_lossy(&bytes),
            )),
            Err(_) => out.push(format!("the file `{path}` is not there after the run")),
        }
    }
    out
}

fn place(root: &Path, build: &Build, test: &Test) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("lengua could not make the files of the test: {e}");
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root).map_err(fail)?;
    for folder in &build.grants.folders {
        fs::create_dir_all(root.join(&folder.path)).map_err(fail)?;
    }
    for (path, content) in &test.files {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(fail)?;
        }
        fs::write(&file, content).map_err(fail)?;
    }
    Ok(())
}

fn ungranted(build: &Build, test: &Test) -> Option<String> {
    let arguments = test.arguments.as_ref().is_some_and(|a| !a.is_empty());
    if arguments && !build.grants.reach(Reach::Arguments) {
        return Some(
            "the example gives arguments, and the idea does not grant `- Reads: arguments`"
                .to_string(),
        );
    }
    if test.stdin.is_some() && !build.grants.reach(Reach::StandardInput) {
        return Some(
            "the example gives a standard input, and the idea does not grant `- Reads: standard input`"
                .to_string(),
        );
    }
    None
}

fn input_of(test: &Test, index: usize, root: &Path) -> Input {
    let (arguments, stdin) = if test.call.is_some() {
        (vec![index.to_string()], Vec::new())
    } else if test.request.is_some() {
        (Vec::new(), request_bytes(test))
    } else {
        (
            test.arguments.clone().unwrap_or_default(),
            test.stdin.clone().unwrap_or_default().into_bytes(),
        )
    };
    Input {
        arguments,
        stdin,
        fixed: true,
        root: Some(root.to_path_buf()),
        model: test.model.clone(),
    }
}

fn answers(test: &Test, got: &[Option<Vec<u8>>], out: &mut Vec<String>) {
    let Some(wanted) = &test.answer else {
        return;
    };
    match got.first() {
        Some(Some(bytes)) => {
            out.extend(diff("the answer", wanted, &String::from_utf8_lossy(bytes)));
        }
        _ => out.push("the client did not reach the program".to_string()),
    }
}

pub(crate) fn run_case(
    folder: &Path,
    build: &Build,
    lock: &Path,
    (index, case): (usize, &Case),
) -> Option<String> {
    let test = &case.test;
    if let Some(message) = ungranted(build, test) {
        return Some(message);
    }
    let root = std::env::temp_dir().join(format!(
        "lengua-test-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    let limit = Duration::from_millis(build.grants.time_ms) + GRACE;
    let result = place(&root, build, test).and_then(|()| {
        let input = input_of(test, index, &root);
        let fixtures = Fixtures::start(test, limit)?;
        let outcome = worker::run(&Request {
            kind: Kind::Test,
            lock,
            folder,
            input: &input,
            limit,
        });
        let got = fixtures.finish();
        let mut found = compare(test, &outcome, &root);
        answers(test, &got, &mut found);
        Ok(found)
    });
    let _ = fs::remove_dir_all(&root);
    match result {
        Ok(found) if found.is_empty() => None,
        Ok(found) => Some(found.join("; ")),
        Err(message) => Some(message),
    }
}

fn values_text(case: &Case) -> String {
    case.values
        .iter()
        .map(|(name, value)| {
            let integer = case
                .test
                .ranges
                .iter()
                .any(|r| matches!(r, Range::Integers { .. }) && r.name() == name.as_str());
            if integer {
                format!("`{name}` = {value}")
            } else {
                format!("`{name}` = {value:?}")
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}

pub(crate) fn problem(case: &Case, detail: &str) -> Problem {
    let test = &case.test;
    let (code, message) = if case.values.is_empty() {
        (
            "test-failed",
            format!(
                "the example `{}` of the behaviour `{}` failed: {detail}",
                test.name, test.behaviour
            ),
        )
    } else {
        (
            "case-failed",
            format!(
                "the case with {} of `{}` in the behaviour `{}` failed: {detail}",
                values_text(case),
                test.name,
                test.behaviour
            ),
        )
    };
    Problem {
        file: test.file.clone(),
        line: test.line,
        code: code.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::diff;

    #[test]
    fn a_difference_shows_both_texts_from_where_they_differ() {
        assert_eq!(diff("standard output", "2\n", "2\n"), None);
        assert_eq!(
            diff("standard output", "2\n", "3\n"),
            Some(
                "standard output: expected \"2\\n\" and got \"3\\n\" (they differ at character 1)"
                    .to_string()
            )
        );
        let long = "a".repeat(100);
        let other = format!("{}b{}", "a".repeat(50), "a".repeat(49));
        let found = diff("x", &long, &other).expect("a difference");
        assert!(found.contains("(they differ at character 51)"), "{found}");
        assert!(found.contains("expected ...\""), "{found}");
        assert!(found.contains("ab"), "{found}");
    }
}
